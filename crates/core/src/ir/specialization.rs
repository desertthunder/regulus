use std::collections::{HashMap, HashSet};

use crate::ast::{self, Expression};
use crate::resolve::{ReferenceTarget, SymbolKind};
use crate::source::Span;
use crate::types::{Type, TypedModule, TypedProject};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencySpecialization {
    pub key: DependencySpecializationKey,
    pub source_span: Span,
    pub instantiated_type: Type,
    pub substitutions: HashMap<String, Type>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencySpecializationKey {
    pub package: String,
    pub module: String,
    pub function: String,
    pub params: Vec<Type>,
    pub return_type: Type,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct FunctionId {
    package: String,
    module: String,
    function: String,
}

#[derive(Debug, Clone)]
struct WorkItem {
    function: FunctionId,
    substitutions: HashMap<String, Type>,
}

impl WorkItem {
    fn key(&self) -> String {
        let mut substitutions = self.substitutions.iter().collect::<Vec<_>>();
        substitutions.sort_by_key(|(name, _)| name.as_str());
        let substitutions = substitutions
            .into_iter()
            .map(|(name, type_)| format!("{name}={}", type_.display()))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{}:{}.{}/{}",
            self.function.package, self.function.module, self.function.function, substitutions
        )
    }
}

trait ExpressionSpan {
    fn span(&self) -> Span;
}

impl ExpressionSpan for Expression {
    fn span(&self) -> Span {
        Span::from(self)
    }
}

struct FunctionSource<'a> {
    module: &'a TypedModule,
    function: &'a ast::Function,
}

impl TypedProject {
    pub fn collect_dependency_specializations(&self) -> Vec<DependencySpecialization> {
        let source_functions = self.source_functions();
        let dependency_functions = source_functions
            .keys()
            .filter(|id| id.package != self.package_name)
            .cloned()
            .collect::<HashSet<_>>();
        let mut worklist = self
            .project_export_roots()
            .into_iter()
            .map(|function| WorkItem { function, substitutions: HashMap::new() })
            .collect::<Vec<_>>();
        let mut visited = HashSet::new();
        let mut specializations = Vec::new();

        while let Some(item) = worklist.pop() {
            if !visited.insert(item.key()) {
                continue;
            }
            let Some(source) = source_functions.get(&item.function) else {
                continue;
            };
            for call in source.function.body.fn_calls() {
                let Some(callee) = source.module.call_target(&call.function) else {
                    continue;
                };
                let callee_is_source = source_functions.contains_key(&callee);
                let Some(interface_type) = self
                    .interfaces
                    .get(&callee.module)
                    .and_then(|entry| entry.interface.functions.get(&callee.function))
                else {
                    if callee_is_source {
                        worklist.push(WorkItem { function: callee, substitutions: item.substitutions.clone() });
                    }
                    continue;
                };
                let Some(call_return_type) = source.module.typed_expression(call.span) else {
                    continue;
                };
                let Some(call_param_types) = call
                    .arguments
                    .iter()
                    .map(|argument| source.module.typed_expression(argument.value.span()))
                    .collect::<Option<Vec<_>>>()
                else {
                    continue;
                };
                let call_site_type = Type::Function {
                    params: call_param_types
                        .into_iter()
                        .map(|type_| type_.substitute(&item.substitutions))
                        .collect(),
                    return_type: Box::new(call_return_type.substitute(&item.substitutions)),
                };
                let substitutions = interface_type.substitutions_to(&call_site_type);
                let instantiated_type = interface_type.substitute(&substitutions);
                let Type::Function { params, return_type } = instantiated_type.clone() else {
                    continue;
                };
                if callee_is_source {
                    worklist.push(WorkItem { function: callee.clone(), substitutions: substitutions.clone() });
                }
                if !dependency_functions.contains(&callee) {
                    continue;
                }
                let key = DependencySpecializationKey {
                    package: callee.package,
                    module: callee.module,
                    function: callee.function,
                    params,
                    return_type: *return_type,
                };
                push_unique_specialization(
                    &mut specializations,
                    DependencySpecialization {
                        key,
                        source_span: call.span,
                        instantiated_type: instantiated_type.clone(),
                        substitutions,
                    },
                );
            }
        }

        specializations
    }

    fn source_functions(&self) -> HashMap<FunctionId, FunctionSource<'_>> {
        let mut functions = HashMap::new();
        for module in &self.modules {
            let (Some(package), Some(module_name)) = (&module.package_name, &module.module_name) else {
                continue;
            };
            for function in &module.resolved.ast.functions {
                functions.insert(
                    FunctionId {
                        package: package.clone(),
                        module: module_name.clone(),
                        function: function.name.text.clone(),
                    },
                    FunctionSource { module, function },
                );
            }
        }
        functions
    }

    fn project_export_roots(&self) -> Vec<FunctionId> {
        self.modules
            .iter()
            .filter(|module| module.package_name.as_deref() == Some(self.package_name.as_str()))
            .flat_map(|module| {
                let package = module.package_name.clone().unwrap_or_default();
                let module_name = module.module_name.clone().unwrap_or_default();
                module
                    .resolved
                    .ast
                    .functions
                    .iter()
                    .filter(|function| function.public)
                    .map(move |function| FunctionId {
                        package: package.clone(),
                        module: module_name.clone(),
                        function: function.name.text.clone(),
                    })
            })
            .collect()
    }
}

impl TypedModule {
    fn typed_expression(&self, span: Span) -> Option<Type> {
        self.expressions
            .iter()
            .find(|expression| expression.span == span)
            .map(|expression| expression.type_.clone())
    }

    fn call_target(&self, function: &Expression) -> Option<FunctionId> {
        match function {
            Expression::Variable(name) => self.resolved.references.iter().find_map(|reference| {
                if reference.name.span != name.span {
                    return None;
                }
                let ReferenceTarget::Symbol(symbol_id) = reference.target else {
                    return None;
                };
                let symbol = self.resolved.symbols.symbol(symbol_id);
                match &symbol.kind {
                    SymbolKind::Function { .. } | SymbolKind::ExternalFunction { .. } => Some(FunctionId {
                        package: self.package_name.clone()?,
                        module: self.module_name.clone()?,
                        function: symbol.name.clone(),
                    }),
                    SymbolKind::Imported { package, module: imported_module, member } => Some(FunctionId {
                        package: package.clone().or_else(|| self.package_name.clone())?,
                        module: imported_module.clone(),
                        function: member.clone(),
                    }),
                    _ => None,
                }
            }),
            Expression::FieldAccess(access) => self.resolved.references.iter().find_map(|reference| {
                let ReferenceTarget::QualifiedMember { module: module_symbol, member, .. } = &reference.target else {
                    return None;
                };
                if member.span != access.field.span {
                    return None;
                }
                let symbol = self.resolved.symbols.symbol(*module_symbol);
                let SymbolKind::Import { package, module: imported_module } = &symbol.kind else {
                    return None;
                };
                Some(FunctionId {
                    package: package.clone().or_else(|| self.package_name.clone())?,
                    module: imported_module.clone(),
                    function: member.text.clone(),
                })
            }),
            _ => None,
        }
    }
}

fn push_unique_specialization(
    specializations: &mut Vec<DependencySpecialization>, specialization: DependencySpecialization,
) {
    if !specializations
        .iter()
        .any(|existing| existing.key == specialization.key)
    {
        specializations.push(specialization);
    }
}

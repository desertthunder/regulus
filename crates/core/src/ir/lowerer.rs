use std::collections::{HashMap, HashSet};

use super::*;
use crate::abi::{StdlibHostAdapter, stdlib_host_adapter, validate_extern_function_abi, validate_external_info_abi};
use crate::ast::{self, Pattern, Statement};
use crate::diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, Label};
use crate::labels::{FunctionLabelMap, call_argument_order, function_label_map, use_callback_placement};
use crate::resolve::ReferenceTarget;
use crate::shared::unquote;
use crate::stdlib::StdlibRegistry;
use crate::types::{ConstructorInfo, ExternalFunctionInfo, FieldInfo, InterfaceEntry, TypedModule};
use bit_slices::{ast_bit_array_literal, bit_array_literal, bit_string_pattern_segments};

pub fn lower(module: TypedModule) -> Result<Module, Diagnostics> {
    Lowerer::new(module).lower()
}

pub fn lower_with_project_interfaces(
    module: TypedModule, interfaces: &HashMap<String, InterfaceEntry>,
) -> Result<Module, Diagnostics> {
    Lowerer::new(module).with_project_interfaces(interfaces).lower()
}

pub struct Lowerer {
    module: TypedModule,
    function_types: HashMap<String, Type>,
    function_labels: FunctionLabelMap,
    imported_functions: HashMap<String, String>,
    constructors: HashMap<String, ConstructorInfo>,
    expression_types: HashMap<Span, Type>,
    external_imports: HashMap<String, ExternalImport>,
    imported_external_imports: HashMap<String, ImportedExternalImport>,
    diagnostics: Diagnostics,
    pub lifted_functions: Vec<Function>,
    anonymous_counter: usize,
}

impl Lowerer {
    fn new(module: TypedModule) -> Self {
        let mut function_types = module
            .functions
            .iter()
            .map(|function| (function.name.text.clone(), function.type_.clone()))
            .collect::<HashMap<_, _>>();
        function_types.extend(module.interface.functions.clone());

        let mut function_labels = function_label_map(&module.resolved.ast);
        function_labels.extend(module.function_labels.clone());

        let constructors = module.interface.constructors.clone();
        let expression_types = module
            .expressions
            .iter()
            .map(|expression| (expression.span, expression.type_.clone()))
            .collect();
        let external_imports = external_imports(&module.resolved.ast);

        Self {
            module,
            function_types,
            function_labels,
            imported_functions: HashMap::new(),
            constructors,
            expression_types,
            external_imports,
            imported_external_imports: HashMap::new(),
            diagnostics: Vec::new(),
            lifted_functions: Vec::new(),
            anonymous_counter: 0,
        }
    }

    fn with_project_interfaces(mut self, interfaces: &HashMap<String, InterfaceEntry>) -> Self {
        for (module, entry) in interfaces {
            let interface = &entry.interface;
            for (name, type_) in &interface.functions {
                self.function_types
                    .entry(format!("{module}.{name}"))
                    .or_insert_with(|| type_.clone());
            }
            for (name, labels) in &interface.function_labels {
                self.function_labels
                    .entry(format!("{module}.{name}"))
                    .or_insert_with(|| labels.clone());
            }
        }
        for import in &self.module.resolved.ast.imports {
            let Some(entry) = interfaces.get(&import.module.text) else {
                continue;
            };
            let interface = &entry.interface;
            for imported in &import.unqualified {
                if !matches!(imported.kind, ast::UnqualifiedImportKind::Value) {
                    continue;
                }
                let local = imported.alias.as_ref().unwrap_or(&imported.name).text.clone();
                if let Some(type_) = interface.functions.get(&imported.name.text) {
                    self.function_types
                        .entry(local.clone())
                        .or_insert_with(|| type_.clone());
                    let lowered_name = self
                        .import_package(import)
                        .map(|package| format!("{package}:{}.{}", import.module.text, imported.name.text))
                        .unwrap_or_else(|| format!("{}.{}", import.module.text, imported.name.text));
                    self.imported_functions.entry(local.clone()).or_insert(lowered_name);
                }
                if let Some(external) = interface.externals.get(&imported.name.text) {
                    let Some(type_) = interface.functions.get(&imported.name.text).cloned() else {
                        continue;
                    };
                    let lowered_name = self
                        .import_package(import)
                        .map(|package| format!("{package}:{}.{}", import.module.text, imported.name.text))
                        .unwrap_or_else(|| format!("{}.{}", import.module.text, imported.name.text));
                    self.external_imports
                        .entry(local.clone())
                        .or_insert_with(|| ExternalImport::from(external));
                    self.external_imports
                        .entry(lowered_name.clone())
                        .or_insert_with(|| ExternalImport::from(external));
                    self.function_types
                        .entry(lowered_name.clone())
                        .or_insert_with(|| type_.clone());
                    self.imported_external_imports
                        .entry(lowered_name)
                        .or_insert_with(|| ImportedExternalImport {
                            external: external.clone(),
                            type_,
                            span: imported.span,
                        });
                }
                if let Some(labels) = interface.function_labels.get(&imported.name.text) {
                    self.function_labels.entry(local).or_insert_with(|| labels.clone());
                }
            }
            for (name, external) in &interface.externals {
                let Some(type_) = interface.functions.get(name).cloned() else {
                    continue;
                };
                let qualified = format!("{}.{}", import.module.text, name);
                let lowered_name = self
                    .import_package(import)
                    .map(|package| format!("{package}:{qualified}"))
                    .unwrap_or_else(|| qualified.clone());
                self.external_imports
                    .entry(qualified)
                    .or_insert_with(|| ExternalImport::from(external));
                self.external_imports
                    .entry(lowered_name.clone())
                    .or_insert_with(|| ExternalImport::from(external));
                self.function_types
                    .entry(lowered_name.clone())
                    .or_insert_with(|| type_.clone());
                self.imported_external_imports
                    .entry(lowered_name)
                    .or_insert_with(|| ImportedExternalImport {
                        external: external.clone(),
                        type_,
                        span: external.span,
                    });
            }
        }
        self
    }

    fn lower(mut self) -> Result<Module, Diagnostics> {
        self.validate_concrete_runtime_types();
        self.validate_external_function_abis();
        if !self.diagnostics.is_empty() {
            return Err(self.diagnostics);
        }

        let ast = self.module.resolved.ast.clone();
        let imports = ast
            .imports
            .iter()
            .map(|import| Import {
                package: self.import_package(import),
                module: import.module.text.clone(),
                alias: import.alias.as_ref().map(|alias| alias.text.clone()),
                unqualified: import
                    .unqualified
                    .iter()
                    .map(|item| UnqualifiedImport {
                        name: item.name.text.clone(),
                        alias: item.alias.as_ref().map(|alias| alias.text.clone()),
                        kind: match item.kind {
                            ast::UnqualifiedImportKind::Value => ImportKind::Value,
                            ast::UnqualifiedImportKind::TypeOrConstructor => ImportKind::TypeOrConstructor,
                        },
                        span: item.span,
                    })
                    .collect(),
                span: import.span,
            })
            .collect();

        let declarations = ast.declarations.iter().map(DeclarationMetadata::from).collect();
        let type_declarations = self.lower_type_metadata();
        let references = self.lower_references();
        let mut exports = self.lower_exports();
        let mut constants = Vec::new();
        let mut init = ModuleInit::default();

        if ast
            .declarations
            .iter()
            .any(|declaration| matches!(declaration, ast::Declaration::Constant(_)))
        {
            init.steps.push(InitStep::RuntimeSetup { span: ast.span });
        }

        for declaration in &ast.declarations {
            if let ast::Declaration::Constant(raw) = declaration {
                let id = ConstantId(constants.len() as u32);
                let constant = self.lower_constant(id, raw);
                if constant.public {
                    exports.push(Export::constant(constant.name.clone(), constant.span));
                }
                if matches!(
                    constant.value,
                    ConstantValue::Literal(Literal { kind: LiteralKind::String, .. })
                ) {
                    init.steps
                        .push(InitStep::StaticData { name: constant.name.clone(), span: constant.span });
                }
                init.steps
                    .push(InitStep::Constant { constant: id, span: constant.span });
                constants.push(constant);
            }
        }

        let mut functions = self.lower_external_host_imports(&ast);
        functions.extend(self.lower_stdlib_host_imports(&ast));
        for function in ast.functions {
            if let Some(function) = self.lower_function(&function) {
                functions.push(function);
            }
        }
        functions.extend(std::mem::take(&mut self.lifted_functions));

        if self.diagnostics.is_empty() {
            Ok(Module {
                span: ast.span,
                identity: self
                    .module
                    .package_name
                    .as_ref()
                    .zip(self.module.module_name.as_ref())
                    .map(|(package, module)| ModuleIdentity { package: package.clone(), module: module.clone() }),
                imports,
                declarations,
                type_declarations,
                constants,
                init,
                references,
                exports,
                functions,
                linked_names: Vec::new(),
            })
        } else {
            Err(self.diagnostics)
        }
    }

    fn validate_concrete_runtime_types(&mut self) {
        if self.allows_upstream_stdlib_generics() {
            return;
        }
        for function in self.module.functions.clone() {
            if function.type_.has_generic() {
                self.diagnostics.push(
                    Diagnostic::new(
                        DiagnosticCode::LoweringError,
                        format!(
                            "function `{}` has generic type `{:?}` that cannot be lowered without monomorphization",
                            function.name.text, function.type_
                        ),
                    )
                    .with_label(Label::primary(function.name.span, "generic function type here")),
                );
            }
        }
        for expression in self.module.expressions.clone() {
            if expression.type_.has_generic() {
                self.diagnostics.push(
                    Diagnostic::new(
                        DiagnosticCode::LoweringError,
                        format!(
                            "expression has generic type `{:?}` that cannot be lowered without monomorphization",
                            expression.type_
                        ),
                    )
                    .with_label(Label::primary(expression.span, "generic expression type here")),
                );
            }
        }
    }

    fn allows_upstream_stdlib_generics(&self) -> bool {
        self.module.package_name.as_deref() == Some("gleam_stdlib")
            && matches!(
                self.module.module_name.as_deref(),
                Some(
                    "gleam/bool"
                        | "gleam/float"
                        | "gleam/function"
                        | "gleam/int"
                        | "gleam/list"
                        | "gleam/option"
                        | "gleam/order"
                        | "gleam/pair"
                        | "gleam/result"
                )
            )
    }

    fn validate_external_function_abis(&mut self) {
        for declaration in self.module.resolved.ast.declarations.clone() {
            self.validate_external_function_abi_in_declaration(&declaration);
        }
        for (name, import) in &self.imported_external_imports {
            self.diagnostics
                .extend(validate_external_info_abi(name, &import.external, &import.type_));
        }
    }

    fn validate_external_function_abi_in_declaration(&mut self, declaration: &ast::Declaration) {
        match declaration {
            ast::Declaration::ExternalFunction(function) => {
                if let Some(type_) = self.function_types.get(&function.name.text) {
                    self.diagnostics.extend(validate_extern_function_abi(function, type_));
                }
            }
            ast::Declaration::TargetGroup(group) => {
                for declaration in &group.declarations {
                    self.validate_external_function_abi_in_declaration(declaration);
                }
            }
            _ => {}
        }
    }

    fn lower_type_metadata(&self) -> Vec<TypeMetadata> {
        let mut types = self
            .module
            .interface
            .types
            .values()
            .filter(|type_| !type_.constructors.is_empty())
            .map(|type_| TypeMetadata {
                name: type_.name.clone(),
                parameters: type_.parameters.clone(),
                opaque: type_.opaque,
                constructors: type_
                    .constructors
                    .iter()
                    .map(|constructor| ConstructorMetadata {
                        name: constructor.name.clone(),
                        fields: constructor
                            .fields
                            .iter()
                            .map(|field| FieldMetadata {
                                name: (!field.name.is_empty()).then(|| field.name.clone()),
                                type_: field.type_.clone(),
                            })
                            .collect(),
                    })
                    .collect(),
            })
            .collect::<Vec<_>>();
        types.sort_by(|left, right| left.name.cmp(&right.name));
        types
    }

    fn lower_exports(&self) -> Vec<Export> {
        let mut exports = Vec::new();
        for function in &self.module.resolved.ast.functions {
            if function.public {
                exports.push(Export::function(function.name.text.clone(), function.name.span))
            }
        }
        for declaration in &self.module.resolved.ast.declarations {
            match declaration {
                ast::Declaration::TypeDefinition(type_) if type_.public => {
                    exports.push(Export::type_(type_.name.text.clone(), type_.span))
                }
                ast::Declaration::TypeAlias(alias) if alias.public => {
                    exports.push(Export::type_(alias.name.text.clone(), alias.span))
                }
                _ => {}
            }
        }
        exports
    }

    fn lower_references(&self) -> Vec<Reference> {
        self.module
            .resolved
            .references
            .iter()
            .map(|reference| {
                let target = match &reference.target {
                    ReferenceTarget::Symbol(id) => {
                        let symbol = self.module.resolved.symbols.symbol(*id);
                        let (module, name) = match &symbol.kind {
                            SymbolKind::Imported { module, member, .. } => (Some(module.clone()), member.clone()),
                            _ => (None, symbol.name.clone()),
                        };
                        let package = match &symbol.kind {
                            SymbolKind::Imported { package, .. } => package.clone(),
                            _ => None,
                        };
                        ReferenceTargetName::LocalSymbol {
                            package,
                            module,
                            name,
                            kind: ReferenceKind::from(&symbol.kind),
                        }
                    }
                    ReferenceTarget::QualifiedMember { module, member, symbol } => {
                        let module_symbol = self.module.resolved.symbols.symbol(*module);
                        let resolved = symbol.map(|id| self.module.resolved.symbols.symbol(id).name.clone());
                        let package = match &module_symbol.kind {
                            SymbolKind::Import { package, .. } => package.clone(),
                            _ => None,
                        };
                        ReferenceTargetName::QualifiedMember {
                            package,
                            module: module_symbol.name.clone(),
                            member: member.text.clone(),
                            resolved,
                        }
                    }
                };
                Reference { name: reference.name.text.clone(), target, span: reference.name.span }
            })
            .collect()
    }

    fn lower_function(&mut self, function: &ast::Function) -> Option<Function> {
        let function_type = self.function_types.get(&function.name.text)?.clone();
        let return_type = match &function_type {
            Type::Function { return_type, .. } => *return_type.clone(),
            _ => return None,
        };

        let mut context = FunctionContext::default();
        context.push_scope();

        let mut params = Vec::new();
        for parameter in &function.parameters {
            let Some(name) = &parameter.name else { continue };
            let Some(annotation) = &parameter.type_annotation else { continue };
            let Some(type_) = Type::from_source(&annotation.source) else { continue };
            let local = context.allocate(name, type_);
            context.bind(name.text.clone(), local.id);
            params.push(local);
        }

        let body = self.lower_block(&mut context, &function.body)?;
        context.pop_scope();

        Some(Function {
            name: function.name.text.clone(),
            public: function.public,
            closure_captures: Vec::new(),
            params,
            locals: context.locals,
            return_type,
            abi: call_abi(
                &function_type,
                if function.public { CallBoundary::ModuleExport } else { CallBoundary::Internal },
            ),
            body,
            span: function.span,
        })
    }

    pub fn lower_block(&mut self, context: &mut FunctionContext, block: &ast::Block) -> Option<Block> {
        context.push_scope();
        let mut instructions = Vec::new();
        let mut result = self.nil_expression(block.span);

        for (index, statement) in block.statements.iter().enumerate() {
            let last_statement = index + 1 == block.statements.len();
            match statement {
                Statement::Let(let_) => {
                    let value = self.lower_expression(context, &let_.value)?;
                    match &let_.pattern {
                        Pattern::Name(name) => {
                            let local = context.allocate(name, value.type_.clone());
                            context.bind(name.text.clone(), local.id);
                            instructions.push(Instruction::LocalSet { local: local.id, value, span: let_.span });
                        }
                        Pattern::Discard(_) => {}
                        pattern => {
                            let pattern = self.lower_pattern(context, pattern, &value.type_)?;
                            instructions.push(Instruction::AssertMatch {
                                value: value.clone(),
                                pattern: pattern.clone(),
                                failure: FailurePath { reason: FailureReason::AssertMatch, span: let_.span },
                                span: let_.span,
                            });
                            self.bind_assert_pattern(context, &mut instructions, &value, &pattern, let_.span);
                        }
                    }
                }
                Statement::LetAssert(let_assert) => {
                    let value = self.lower_expression(context, &let_assert.value)?;
                    let pattern = self.lower_pattern(context, &let_assert.pattern, &value.type_)?;
                    instructions.push(Instruction::AssertMatch {
                        value: value.clone(),
                        pattern: pattern.clone(),
                        failure: FailurePath { reason: FailureReason::AssertMatch, span: let_assert.span },
                        span: let_assert.span,
                    });
                    self.bind_assert_pattern(context, &mut instructions, &value, &pattern, let_assert.span);
                }
                Statement::Expression(ast::Expression::Use(use_)) => {
                    result = self.lower_use(context, use_, &block.statements[index + 1..], block.span)?;
                    break;
                }
                Statement::Expression(expression) => {
                    let value = self.lower_expression(context, expression)?;
                    if last_statement {
                        result = value;
                    } else {
                        instructions.push(Instruction::Evaluate { expression: value, span: Span::from(expression) });
                    }
                }
            }
        }

        context.pop_scope();

        Some(Block { instructions, result: Box::new(result), span: block.span })
    }

    pub fn lower_expression(
        &mut self, context: &mut FunctionContext, expression: &ast::Expression,
    ) -> Option<Expression> {
        match expression {
            ast::Expression::Literal(literal) => Some(Expression {
                type_: Type::from(&literal.kind),
                span: literal.span,
                kind: ExpressionKind::Literal(Literal { kind: literal.kind.clone(), source: literal.source.clone() }),
            }),
            ast::Expression::Variable(name) => {
                if let Some(local) = context.lookup(&name.text) {
                    let type_ = context.local(local).type_.clone();
                    return Some(Expression { type_, span: name.span, kind: ExpressionKind::LocalGet(local) });
                }

                let type_ = self.function_types.get(&name.text)?.clone();
                let boundary = self
                    .external_import_boundary(&name.text)
                    .unwrap_or(CallBoundary::Internal);
                Some(Expression {
                    span: name.span,
                    kind: ExpressionKind::FunctionValue(FunctionValue {
                        name: name.text.clone(),
                        abi: call_abi(&type_, boundary),
                    }),
                    type_,
                })
            }
            ast::Expression::Call(call) => self.lower_call(context, call),
            ast::Expression::Block(block) => Some(*self.lower_block(context, block)?.result),
            ast::Expression::Case(case) => {
                let subjects = case
                    .subjects
                    .iter()
                    .map(|subject| self.lower_expression(context, subject))
                    .collect::<Option<Vec<_>>>()?;
                let mut clauses = Vec::new();
                let mut type_ = Type::Nil;
                for clause in &case.clauses {
                    context.push_scope();
                    let patterns = clause
                        .patterns
                        .iter()
                        .zip(subjects.iter())
                        .map(|(pattern, subject)| self.lower_pattern(context, pattern, &subject.type_))
                        .collect::<Option<Vec<_>>>();
                    let guard = match &clause.guard {
                        Some(guard) => Some(self.lower_expression(context, guard)?),
                        None => None,
                    };
                    let patterns = patterns?;
                    let bindings = self.successful_bindings(context, &patterns);
                    let body = self.lower_expression(context, &clause.value)?;
                    context.pop_scope();
                    type_ = body.type_.clone();
                    clauses.push(BranchClause { patterns, guard, bindings, body: Box::new(body), span: clause.span });
                }
                Some(Expression {
                    type_,
                    span: case.span,
                    kind: ExpressionKind::Branch(Branch {
                        subjects,
                        clauses,
                        fallthrough: FailurePath { reason: FailureReason::BranchFallthrough, span: case.span },
                    }),
                })
            }
            ast::Expression::FieldAccess(field_access) => {
                if let Some(stdlib_value) = stdlib_call(&self.module.resolved.ast, expression)
                    && stdlib_value.implementation == Some(StdlibImplementation::RuntimePrimitive)
                    && !matches!(stdlib_value.type_, Type::Function { .. })
                {
                    let type_ = self
                        .typed_expression_type(field_access.span)
                        .unwrap_or_else(|| stdlib_value.type_.clone());
                    return Some(Expression {
                        type_: type_.clone(),
                        span: field_access.span,
                        kind: ExpressionKind::DirectCall(DirectCall {
                            function: stdlib_lowered_name(&stdlib_value.module, &stdlib_value.member),
                            arguments: Vec::new(),
                            abi: call_abi(
                                &Type::Function { params: Vec::new(), return_type: Box::new(type_) },
                                CallBoundary::Internal,
                            ),
                        }),
                    });
                }

                let record = self.lower_expression(context, &field_access.record)?;
                let type_ = self.typed_expression_type(field_access.span).unwrap_or(Type::Nil);
                Some(Expression {
                    type_,
                    span: field_access.span,
                    kind: ExpressionKind::FieldAccess {
                        record: Box::new(record),
                        field: field_access.field.text.clone(),
                    },
                })
            }
            ast::Expression::Tuple(tuple) => Some(Expression {
                type_: self
                    .typed_expression_type(tuple.span)
                    .unwrap_or(Type::Tuple(Vec::new())),
                span: tuple.span,
                kind: ExpressionKind::Tuple(
                    tuple
                        .elements
                        .iter()
                        .map(|item| self.lower_expression(context, item))
                        .collect::<Option<Vec<_>>>()?,
                ),
            }),
            ast::Expression::List(list) => Some(Expression {
                type_: self
                    .typed_expression_type(list.span)
                    .unwrap_or_else(|| Type::List(Box::new(Type::Int))),
                span: list.span,
                kind: ExpressionKind::List(
                    list.elements
                        .iter()
                        .map(|item| self.lower_expression(context, item))
                        .collect::<Option<Vec<_>>>()?,
                ),
            }),
            ast::Expression::Record(record) => {
                let type_ = self.typed_expression_type(record.span).unwrap_or(Type::Nil);
                let name = self.resolved_constructor_name(&constructor_name(&record.constructor));
                let arguments = self.lower_constructor_arguments(context, &name, &record.arguments)?;
                Some(Expression {
                    type_,
                    span: record.span,
                    kind: ExpressionKind::Constructor(ConstructorValue { name, arguments }),
                })
            }
            ast::Expression::BitArray(bit_array) => Some(Expression {
                type_: Type::BitArray,
                span: bit_array.span,
                kind: ExpressionKind::BitArray(ast_bit_array_literal(bit_array)),
            }),
            ast::Expression::Panic(panic) => Some(Expression {
                type_: self.typed_expression_type(panic.span).unwrap_or(Type::Nil),
                span: panic.span,
                kind: ExpressionKind::Failure(FailurePath { reason: FailureReason::Panic, span: panic.span }),
            }),
            ast::Expression::Todo(todo) => Some(Expression {
                type_: self.typed_expression_type(todo.span).unwrap_or(Type::Nil),
                span: todo.span,
                kind: ExpressionKind::Failure(FailurePath { reason: FailureReason::Todo, span: todo.span }),
            }),
            ast::Expression::Assert(assert) => Some(Expression {
                type_: self.typed_expression_type(assert.span).unwrap_or(Type::Nil),
                span: assert.span,
                kind: ExpressionKind::Failure(FailurePath { reason: FailureReason::Assert, span: assert.span }),
            }),
            ast::Expression::Raw(raw) if raw.kind == "bit_string" => Some(Expression {
                type_: Type::BitArray,
                span: raw.span,
                kind: ExpressionKind::BitArray(bit_array_literal(raw)),
            }),
            ast::Expression::Raw(raw) if raw.kind == "tuple" => Some(Expression {
                type_: self.typed_expression_type(raw.span).unwrap_or(Type::Tuple(Vec::new())),
                span: raw.span,
                kind: ExpressionKind::Tuple(raw_literal_arguments(raw)?),
            }),
            ast::Expression::Raw(raw) if raw.kind == "list" => Some(Expression {
                type_: self
                    .typed_expression_type(raw.span)
                    .unwrap_or_else(|| Type::List(Box::new(Type::Int))),
                span: raw.span,
                kind: ExpressionKind::List(raw_literal_arguments(raw)?),
            }),
            ast::Expression::Raw(raw) if raw.kind == "record" => {
                let type_ = self.typed_expression_type(raw.span).unwrap_or(Type::Nil);
                Some(Expression {
                    type_,
                    span: raw.span,
                    kind: ExpressionKind::Constructor(ConstructorValue {
                        name: raw.source.split(['(', ' ']).next().unwrap_or(&raw.source).into(),
                        arguments: raw_record_arguments(raw)?,
                    }),
                })
            }
            ast::Expression::Raw(raw) if matches!(raw.kind.as_str(), "panic" | "todo" | "assert") => Some(Expression {
                type_: self.typed_expression_type(raw.span).unwrap_or(Type::Nil),
                span: raw.span,
                kind: ExpressionKind::Failure(FailurePath {
                    reason: match raw.kind.as_str() {
                        "panic" => FailureReason::Panic,
                        "todo" => FailureReason::Todo,
                        _ => FailureReason::Assert,
                    },
                    span: raw.span,
                }),
            }),
            ast::Expression::BinaryOperation(operation) => self.lower_binary_operation(context, operation),
            ast::Expression::Pipeline(pipeline) => self.lower_pipeline(context, pipeline),
            ast::Expression::UnaryOperation(operation) => self.lower_unary_operation(context, operation),
            ast::Expression::Use(use_) => self.lower_use(context, use_, &[], use_.span),
            ast::Expression::AnonymousFunction(function) => self.lower_anonymous_function(context, function),
            ast::Expression::Capture(capture) => self.lower_capture(context, capture),
            ast::Expression::RecordUpdate(update) => self.lower_record_update(context, update),
            ast::Expression::TupleAccess(access) => self.lower_tuple_access(context, access),
            ast::Expression::Echo(echo) => self.lower_expression(context, &echo.value),
            ast::Expression::Raw(raw) => self.unsupported_ast_expression(&raw.kind, raw.span),
        }
    }

    fn lower_binary_operation(
        &mut self, context: &mut FunctionContext, operation: &ast::BinaryOperation,
    ) -> Option<Expression> {
        let left = self.lower_expression(context, &operation.left)?;
        let right = self.lower_expression(context, &operation.right)?;
        let type_ = self.typed_expression_type(operation.span).unwrap_or(Type::Nil);
        let kind = match comparison_op(&operation.operator) {
            Some(op) => ExpressionKind::Compare { op, left: Box::new(left), right: Box::new(right) },
            None if matches!(
                operation.operator,
                ast::BinaryOperator::Equal | ast::BinaryOperator::NotEqual
            ) =>
            {
                ExpressionKind::RuntimeEquality { left: Box::new(left), right: Box::new(right) }
            }
            None => ExpressionKind::DirectCall(DirectCall {
                function: operator_function_name(&operation.operator).into(),
                arguments: vec![
                    CallArgument { label: None, span: Span::from(operation.left.as_ref()), value: left },
                    CallArgument { label: None, span: Span::from(operation.right.as_ref()), value: right },
                ],
                abi: CallAbi { params: Vec::new(), return_: abi_return(&type_), boundary: CallBoundary::Internal },
            }),
        };
        Some(Expression { type_, span: operation.span, kind })
    }

    fn lower_pipeline(&mut self, context: &mut FunctionContext, pipeline: &ast::Pipeline) -> Option<Expression> {
        let input = self.lower_expression(context, &pipeline.value)?;
        let call = self.lower_expression(context, &pipeline.into)?;
        let type_ = self
            .typed_expression_type(pipeline.span)
            .unwrap_or_else(|| call.type_.clone());
        Some(Expression {
            type_,
            span: pipeline.span,
            kind: ExpressionKind::Pipeline(PipelineLowering {
                input: Box::new(input),
                call: Box::new(call),
                inserted_argument: 0,
            }),
        })
    }

    fn lower_unary_operation(
        &mut self, context: &mut FunctionContext, operation: &ast::UnaryOperation,
    ) -> Option<Expression> {
        let value = self.lower_expression(context, &operation.value)?;
        let type_ = self
            .typed_expression_type(operation.span)
            .unwrap_or_else(|| value.type_.clone());
        Some(Expression {
            type_: type_.clone(),
            span: operation.span,
            kind: ExpressionKind::DirectCall(DirectCall {
                function: match operation.operator {
                    ast::UnaryOperator::BooleanNot => "__op_not".into(),
                    ast::UnaryOperator::IntegerNegate => "__op_negate".into(),
                },
                arguments: vec![CallArgument { label: None, span: Span::from(operation.value.as_ref()), value }],
                abi: CallAbi { params: Vec::new(), return_: abi_return(&type_), boundary: CallBoundary::Internal },
            }),
        })
    }

    fn lower_use(
        &mut self, context: &mut FunctionContext, use_: &ast::Use, continuation: &[Statement], block_span: Span,
    ) -> Option<Expression> {
        let (callback_type, return_type) = self.use_callback_and_return_types(use_)?;
        let callback = self.lower_use_callback(context, use_, continuation, block_span, callback_type)?;
        match use_.value.as_ref() {
            ast::Expression::Call(call) => self.lower_call_with_callback(context, call, callback, return_type),
            value => {
                let callee = self.lower_expression(context, value)?;
                let callee_type = callee.type_.clone();
                Some(Expression {
                    type_: return_type,
                    span: use_.span,
                    kind: ExpressionKind::IndirectCall(IndirectCall {
                        callee: Box::new(callee),
                        arguments: vec![CallArgument { label: None, value: callback, span: use_.span }],
                        abi: call_abi(&callee_type, CallBoundary::Internal),
                    }),
                })
            }
        }
    }

    fn use_callback_and_return_types(&mut self, use_: &ast::Use) -> Option<(Type, Type)> {
        let function_type = match use_.value.as_ref() {
            ast::Expression::Call(call) => self.ast_function_type(&call.function)?,
            value => self.typed_expression_type(Span::from(value))?,
        };
        let Type::Function { params, return_type } = function_type else {
            return None;
        };
        let callback_index = match use_.value.as_ref() {
            ast::Expression::Call(call) => {
                use_callback_placement(self.call_function_labels(call), &call.arguments, params.len())
                    .ok()?
                    .callback_index
            }
            _ => 0,
        };
        let callback_type = params.get(callback_index)?.clone();
        Some((callback_type, *return_type))
    }

    fn ast_function_type(&self, expression: &ast::Expression) -> Option<Type> {
        match expression {
            ast::Expression::Variable(name) => self.function_types.get(&name.text).cloned(),
            expression => self.typed_expression_type(Span::from(expression)),
        }
    }

    fn lower_use_callback(
        &mut self, context: &mut FunctionContext, use_: &ast::Use, continuation: &[Statement], block_span: Span,
        callback_type: Type,
    ) -> Option<Expression> {
        let Type::Function { params, return_type } = callback_type else {
            return None;
        };
        let outer_local_count = context.locals.len();
        context.push_scope();
        let mut original_params = Vec::new();
        let mut instructions = Vec::new();
        for (index, assignment) in use_.assignments.iter().enumerate() {
            let param_type = params.get(index).cloned().unwrap_or(Type::Nil);
            match &assignment.pattern {
                Pattern::Name(name) => {
                    let local = context.allocate(name, param_type);
                    context.bind(name.text.clone(), local.id);
                    original_params.push(local);
                }
                Pattern::Discard(span) => {
                    let name = ast::Name { span: *span, text: format!("_use_{index}") };
                    original_params.push(context.allocate(&name, param_type));
                }
                pattern => {
                    let name = ast::Name { span: assignment.span, text: format!("_use_{index}") };
                    let local = context.allocate(&name, param_type.clone());
                    let value = Expression {
                        type_: param_type.clone(),
                        span: assignment.span,
                        kind: ExpressionKind::LocalGet(local.id),
                    };
                    let pattern = self.lower_pattern(context, pattern, &param_type)?;
                    instructions.push(Instruction::AssertMatch {
                        value: value.clone(),
                        pattern: pattern.clone(),
                        failure: FailurePath { reason: FailureReason::AssertMatch, span: assignment.span },
                        span: assignment.span,
                    });
                    self.bind_assert_pattern(context, &mut instructions, &value, &pattern, assignment.span);
                    original_params.push(local);
                }
            }
        }
        let mut body = self.lower_block(
            context,
            &ast::Block { span: block_span, statements: continuation.to_vec() },
        )?;
        context.pop_scope();
        instructions.append(&mut body.instructions);
        body.instructions = instructions;
        body.span = use_.span;
        body.result.type_ = *return_type.clone();
        Some(closure::lower_synthetic_anonymous_function(
            self,
            context,
            use_.span,
            outer_local_count,
            original_params,
            body,
            &Type::Function { params, return_type },
        ))
    }

    fn lower_use_call_arguments(
        &mut self, context: &mut FunctionContext, call: &ast::Call, callback: Expression,
    ) -> Option<Vec<CallArgument>> {
        let param_count = self
            .call_function_labels(call)
            .map_or(call.arguments.len() + 1, <[_]>::len);
        let placement = use_callback_placement(self.call_function_labels(call), &call.arguments, param_count).ok()?;
        if !placement.has_labels {
            let mut arguments = self.lower_call_arguments(context, &call.arguments)?;
            arguments.push(CallArgument { label: None, value: callback, span: call.span });
            return Some(arguments);
        }
        let labels = self.call_function_labels(call)?.to_vec();
        let mut ordered = vec![None; labels.len()];
        for (argument, index) in call.arguments.iter().zip(placement.argument_indices) {
            ordered[index] = Some(CallArgument {
                label: argument.label.as_ref().map(|label| label.text.clone()),
                value: self.lower_expression(context, &argument.value)?,
                span: argument.span,
            });
        }
        ordered[placement.callback_index] =
            Some(CallArgument { label: labels[placement.callback_index].clone(), value: callback, span: call.span });
        Some(ordered.into_iter().flatten().collect())
    }

    fn call_function_labels(&self, call: &ast::Call) -> Option<&[Option<String>]> {
        match call.function.as_ref() {
            ast::Expression::Variable(name) => self.function_labels.get(&name.text).map(Vec::as_slice),
            ast::Expression::FieldAccess(access) => match access.record.as_ref() {
                ast::Expression::Variable(module) => self
                    .function_labels
                    .get(&format!("{}.{}", module.text, access.field.text))
                    .map(Vec::as_slice),
                _ => None,
            },
            _ => None,
        }
    }

    fn import_local_name(&self, import: &ast::Import) -> String {
        import
            .alias
            .as_ref()
            .map(|alias| alias.text.clone())
            .unwrap_or_else(|| {
                import
                    .module
                    .text
                    .rsplit('/')
                    .next()
                    .unwrap_or(&import.module.text)
                    .to_string()
            })
    }

    fn import_package(&self, import: &ast::Import) -> Option<String> {
        let local = self.import_local_name(import);
        self.module
            .resolved
            .symbols
            .symbols
            .iter()
            .find(|symbol| symbol.namespace == Namespace::Module && symbol.name == local)
            .and_then(|symbol| match &symbol.kind {
                SymbolKind::Import { package, .. } => package.clone(),
                _ => None,
            })
    }

    fn qualified_function(&self, function: &ast::Expression) -> Option<(String, Type)> {
        let ast::Expression::FieldAccess(access) = function else { return None };
        let ast::Expression::Variable(module) = access.record.as_ref() else { return None };
        let import = self
            .module
            .resolved
            .ast
            .imports
            .iter()
            .find(|import| self.import_local_name(import) == module.text);
        let module_name = import.map(|import| import.module.text.as_str()).unwrap_or(&module.text);
        let qualified = format!("{}.{}", module_name, access.field.text);
        let lowered = import
            .and_then(|import| self.import_package(import))
            .map(|package| format!("{package}:{qualified}"))
            .unwrap_or_else(|| qualified.clone());
        self.function_types
            .get(&qualified)
            .cloned()
            .map(|type_| (lowered, type_))
    }

    fn lower_call_with_callback(
        &mut self, context: &mut FunctionContext, call: &ast::Call, callback: Expression, return_type: Type,
    ) -> Option<Expression> {
        if let ast::Expression::Variable(function_name) = call.function.as_ref()
            && let Some(function_type) = self.function_types.get(&function_name.text).cloned()
        {
            let arguments = self.lower_use_call_arguments(context, call, callback)?;
            return Some(Expression {
                type_: return_type,
                span: call.span,
                kind: ExpressionKind::DirectCall(DirectCall {
                    function: self
                        .imported_functions
                        .get(&function_name.text)
                        .cloned()
                        .unwrap_or_else(|| function_name.text.clone()),
                    arguments,
                    abi: call_abi(&function_type, CallBoundary::Internal),
                }),
            });
        }

        let callee = self.lower_expression(context, &call.function)?;
        let callee_type = callee.type_.clone();
        let arguments = self.lower_use_call_arguments(context, call, callback)?;
        Some(Expression {
            type_: return_type,
            span: call.span,
            kind: ExpressionKind::IndirectCall(IndirectCall {
                callee: Box::new(callee),
                arguments,
                abi: call_abi(&callee_type, CallBoundary::Internal),
            }),
        })
    }

    fn lower_anonymous_function(
        &mut self, context: &mut FunctionContext, function: &ast::AnonymousFunction,
    ) -> Option<Expression> {
        closure::lower_anonymous_function(self, context, function)
    }

    fn lower_capture(&mut self, context: &mut FunctionContext, capture: &ast::Capture) -> Option<Expression> {
        closure::lower_capture(self, context, capture)
    }

    pub fn next_anonymous_name(&mut self) -> String {
        let name = format!("__anon_{}", self.anonymous_counter);
        self.anonymous_counter += 1;
        name
    }

    fn resolved_constructor_name(&self, name: &str) -> String {
        if self.constructors.contains_key(name) {
            return name.into();
        }
        let local = name.rsplit('.').next().unwrap_or(name);
        if self.constructors.contains_key(local) {
            return local.into();
        }
        name.into()
    }

    fn lower_constructor_arguments(
        &mut self, context: &mut FunctionContext, constructor: &str, arguments: &[ast::Argument],
    ) -> Option<Vec<Expression>> {
        let Some(info) = self.constructors.get(constructor).cloned() else {
            return arguments
                .iter()
                .map(|argument| self.lower_expression(context, &argument.value))
                .collect();
        };
        let mut ordered = Vec::new();
        for (index, field) in info.fields.iter().enumerate() {
            let argument = arguments
                .iter()
                .find(|argument| argument.label.as_ref().is_some_and(|label| label.text == field.name))
                .or_else(|| arguments.get(index));
            let Some(argument) = argument else { continue };
            ordered.push(self.lower_expression(context, &argument.value)?);
        }
        Some(ordered)
    }

    fn lower_constructor_pattern_arguments(
        &mut self, context: &mut FunctionContext, constructor: &str, arguments: &[ast::RecordPatternArgument],
        subject_type: &Type,
    ) -> Option<Vec<ConstructorPatternArgument>> {
        let Some(info) = self.constructors.get(constructor).cloned() else {
            return arguments
                .iter()
                .map(|argument| self.lower_constructor_pattern_argument(context, argument, subject_type))
                .collect();
        };
        let substitutions = constructor_type_substitutions(&info, subject_type);
        let mut ordered = Vec::new();
        for (index, field) in info.fields.iter().enumerate() {
            let argument = arguments
                .iter()
                .find(|argument| argument.label.as_ref().is_some_and(|label| label.text == field.name))
                .or_else(|| arguments.get(index));
            let Some(argument) = argument else { continue };
            let field_type = substitute_type_generics(&field.type_, &substitutions);
            ordered.push(self.lower_constructor_pattern_argument(context, argument, &field_type)?);
        }
        Some(ordered)
    }

    fn lower_constructor_pattern_argument(
        &mut self, context: &mut FunctionContext, argument: &ast::RecordPatternArgument, type_: &Type,
    ) -> Option<ConstructorPatternArgument> {
        let pattern = match &argument.pattern {
            Some(pattern) => self.lower_pattern(context, pattern, type_)?,
            None => match &argument.label {
                Some(label) => {
                    let local = context.allocate(label, type_.clone());
                    context.bind(label.text.clone(), local.id);
                    IrPattern::Binding(local.id)
                }
                None => IrPattern::Discard,
            },
        };
        Some(ConstructorPatternArgument {
            label: argument.label.as_ref().map(|label| label.text.clone()),
            pattern,
            span: argument.span,
        })
    }

    fn lower_record_update_fields(
        &mut self, context: &mut FunctionContext, constructor: &ConstructorInfo, updates: &[ast::Argument],
    ) -> Option<Vec<RecordFieldUpdate>> {
        constructor
            .fields
            .iter()
            .map(|field| {
                let value = match updates
                    .iter()
                    .find(|argument| argument.label.as_ref().is_some_and(|label| label.text == field.name))
                {
                    Some(argument) => Some(self.lower_expression(context, &argument.value)?),
                    None => None,
                };
                Some(RecordFieldUpdate { name: field.name.clone(), type_: field.type_.clone(), value })
            })
            .collect()
    }

    fn lower_record_update(&mut self, context: &mut FunctionContext, update: &ast::RecordUpdate) -> Option<Expression> {
        let record = self.lower_expression(context, &update.spread)?;
        let constructor = constructor_name(&update.constructor);
        let constructor_info = self.constructors.get(&constructor).cloned();
        let fields = match constructor_info.as_ref() {
            Some(info) => self.lower_record_update_fields(context, info, &update.updates)?,
            None => Vec::new(),
        };
        let type_ = self
            .typed_expression_type(update.span)
            .unwrap_or_else(|| record.type_.clone());
        Some(Expression {
            type_,
            span: update.span,
            kind: ExpressionKind::RecordUpdate { record: Box::new(record), constructor, fields },
        })
    }

    fn lower_tuple_access(&mut self, context: &mut FunctionContext, access: &ast::TupleAccess) -> Option<Expression> {
        let tuple = self.lower_expression(context, &access.tuple)?;
        let type_ = self.typed_expression_type(access.span).unwrap_or(Type::Nil);
        Some(Expression {
            type_,
            span: access.span,
            kind: ExpressionKind::TupleElement { tuple: Box::new(tuple), index: access.index.text.parse().ok()? },
        })
    }

    fn unsupported_ast_expression(&mut self, kind: &str, span: Span) -> Option<Expression> {
        self.diagnostics.push(
            Diagnostic::new(
                DiagnosticCode::LoweringError,
                format!("expression `{kind}` cannot be lowered"),
            )
            .with_label(Label::primary(span, "unsupported expression here")),
        );
        None
    }

    fn lower_call(&mut self, context: &mut FunctionContext, call: &ast::Call) -> Option<Expression> {
        if let Some(stdlib_call) = stdlib_call(&self.module.resolved.ast, &call.function) {
            if let Some(expression) = self.lower_higher_order_stdlib_call(context, call, &stdlib_call) {
                return Some(expression);
            }

            if stdlib_call.implementation.is_none() {
                self.diagnostics.push(
                    Diagnostic::new(
                        DiagnosticCode::LoweringError,
                        format!(
                            "stdlib member `{}` from `{}` cannot be lowered for this target yet",
                            stdlib_call.member, stdlib_call.module
                        ),
                    )
                    .with_label(Label::primary(call.span, "unsupported stdlib call")),
                );
                return None;
            }
            let function_type = stdlib_call.type_.clone();
            let Type::Function { return_type, .. } = function_type.clone() else {
                return None;
            };
            let boundary = match stdlib_call.implementation {
                Some(StdlibImplementation::RuntimePrimitive) => CallBoundary::Internal,
                Some(StdlibImplementation::HostAdapter(adapter)) => {
                    CallBoundary::HostImport { module: adapter.import_module.into(), name: adapter.import_name.into() }
                }
                None => return None,
            };
            let type_ = self.typed_expression_type(call.span).unwrap_or(*return_type);
            return Some(Expression {
                type_,
                span: call.span,
                kind: ExpressionKind::DirectCall(DirectCall {
                    function: stdlib_lowered_name(&stdlib_call.module, &stdlib_call.member),
                    arguments: self.lower_ordered_call_arguments(context, call)?,
                    abi: call_abi(&function_type, boundary),
                }),
            });
        }

        if let ast::Expression::Variable(function_name) = call.function.as_ref()
            && let Some(function_type) = self.function_types.get(&function_name.text).cloned()
        {
            let Type::Function { return_type, .. } = function_type.clone() else {
                return None;
            };
            let boundary = self
                .external_import_boundary(&function_name.text)
                .unwrap_or(CallBoundary::Internal);
            return Some(Expression {
                type_: *return_type,
                span: call.span,
                kind: ExpressionKind::DirectCall(DirectCall {
                    function: self
                        .imported_functions
                        .get(&function_name.text)
                        .cloned()
                        .unwrap_or_else(|| function_name.text.clone()),
                    arguments: self.lower_ordered_call_arguments(context, call)?,
                    abi: call_abi(&function_type, boundary),
                }),
            });
        }

        if let Some((function_name, function_type)) = self.qualified_function(&call.function) {
            let Type::Function { return_type, .. } = function_type.clone() else {
                return None;
            };
            let boundary = self
                .external_import_boundary(&function_name)
                .unwrap_or(CallBoundary::Internal);
            return Some(Expression {
                type_: *return_type,
                span: call.span,
                kind: ExpressionKind::DirectCall(DirectCall {
                    function: function_name,
                    arguments: self.lower_ordered_call_arguments(context, call)?,
                    abi: call_abi(&function_type, boundary),
                }),
            });
        }

        let callee = self.lower_expression(context, &call.function)?;
        let Type::Function { return_type, .. } = callee.type_.clone() else {
            self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::LoweringError, "called value is not a function")
                    .with_label(Label::primary(call.span, "not a function here")),
            );
            return None;
        };
        let callee_type = callee.type_.clone();
        Some(Expression {
            type_: *return_type,
            span: call.span,
            kind: ExpressionKind::IndirectCall(IndirectCall {
                callee: Box::new(callee),
                arguments: self.lower_ordered_call_arguments(context, call)?,
                abi: call_abi(&callee_type, CallBoundary::Internal),
            }),
        })
    }

    fn lower_higher_order_stdlib_call(
        &mut self, context: &mut FunctionContext, call: &ast::Call, stdlib_call: &StdlibCall,
    ) -> Option<Expression> {
        match (stdlib_call.module.as_str(), stdlib_call.member.as_str()) {
            ("gleam/list", "map") => self.lower_list_map(context, call, stdlib_call),
            ("gleam/list", "fold") => self.lower_list_fold(context, call, stdlib_call),
            ("gleam/result", "map") => self.lower_result_map(context, call, stdlib_call),
            ("gleam/option", "map") => self.lower_option_map(context, call, stdlib_call),
            ("gleam/function", "compose") => self.lower_function_compose(context, call, stdlib_call),
            ("gleam/function", "flip") => self.lower_function_flip(context, call, stdlib_call),
            _ => None,
        }
    }

    fn lower_list_map(
        &mut self, context: &mut FunctionContext, call: &ast::Call, stdlib_call: &StdlibCall,
    ) -> Option<Expression> {
        let arguments = self.lower_ordered_call_arguments(context, call)?;
        self.validate_callback_argument(stdlib_call, call.span, arguments.get(1)?)?;
        let list_type = arguments[0].value.type_.clone();
        let callback_type = arguments[1].value.type_.clone();
        let Type::List(input_type) = list_type.clone() else { return None };
        let Type::Function { params: _, return_type } = callback_type.clone() else { return None };
        let output_type = *return_type;
        let adapter = self.list_map_adapter(&input_type, &output_type, &callback_type, call.span);
        let return_type = list(output_type);
        Some(Expression {
            type_: return_type.clone(),
            span: call.span,
            kind: ExpressionKind::DirectCall(DirectCall {
                function: adapter,
                arguments,
                abi: call_abi(
                    &Type::Function { params: vec![list_type, callback_type], return_type: Box::new(return_type) },
                    CallBoundary::Internal,
                ),
            }),
        })
    }

    fn lower_list_fold(
        &mut self, context: &mut FunctionContext, call: &ast::Call, stdlib_call: &StdlibCall,
    ) -> Option<Expression> {
        let arguments = self.lower_ordered_call_arguments(context, call)?;
        self.validate_callback_argument(stdlib_call, call.span, arguments.get(2)?)?;
        let list_type = arguments[0].value.type_.clone();
        let acc_type = arguments[1].value.type_.clone();
        let callback_type = arguments[2].value.type_.clone();
        let Type::List(input_type) = list_type.clone() else { return None };
        let adapter = self.list_fold_adapter(
            (*input_type).clone(),
            acc_type.clone(),
            callback_type.clone(),
            call.span,
        );
        Some(Expression {
            type_: acc_type.clone(),
            span: call.span,
            kind: ExpressionKind::DirectCall(DirectCall {
                function: adapter,
                arguments,
                abi: call_abi(
                    &Type::Function {
                        params: vec![list_type, acc_type.clone(), callback_type],
                        return_type: Box::new(acc_type),
                    },
                    CallBoundary::Internal,
                ),
            }),
        })
    }

    fn lower_option_map(
        &mut self, context: &mut FunctionContext, call: &ast::Call, stdlib_call: &StdlibCall,
    ) -> Option<Expression> {
        let arguments = self.lower_ordered_call_arguments(context, call)?;
        self.validate_callback_argument(stdlib_call, call.span, arguments.get(1)?)?;
        let option_type = arguments[0].value.type_.clone();
        let callback_type = arguments[1].value.type_.clone();
        let Type::Custom { name, args } = option_type.clone() else { return None };
        if name != "Option" || args.len() != 1 {
            return None;
        }
        let Type::Function { return_type, .. } = callback_type.clone() else { return None };
        let return_type = option(*return_type);
        let adapter = self.option_map_adapter(args[0].clone(), return_type.clone(), callback_type.clone(), call.span);
        Some(Expression {
            type_: return_type.clone(),
            span: call.span,
            kind: ExpressionKind::DirectCall(DirectCall {
                function: adapter,
                arguments,
                abi: call_abi(
                    &Type::Function { params: vec![option_type, callback_type], return_type: Box::new(return_type) },
                    CallBoundary::Internal,
                ),
            }),
        })
    }

    fn lower_result_map(
        &mut self, context: &mut FunctionContext, call: &ast::Call, stdlib_call: &StdlibCall,
    ) -> Option<Expression> {
        let arguments = self.lower_ordered_call_arguments(context, call)?;
        self.validate_callback_argument(stdlib_call, call.span, arguments.get(1)?)?;
        let result_type = arguments[0].value.type_.clone();
        let callback_type = arguments[1].value.type_.clone();
        let Type::Custom { name, args } = result_type.clone() else { return None };
        if name != "Result" || args.len() != 2 {
            return None;
        }
        let Type::Function { return_type, .. } = callback_type.clone() else { return None };
        let return_type = result(*return_type, args[1].clone());
        let adapter = self.result_map_adapter(
            args[0].clone(),
            args[1].clone(),
            return_type.clone(),
            callback_type.clone(),
            call.span,
        );
        Some(Expression {
            type_: return_type.clone(),
            span: call.span,
            kind: ExpressionKind::DirectCall(DirectCall {
                function: adapter,
                arguments,
                abi: call_abi(
                    &Type::Function { params: vec![result_type, callback_type], return_type: Box::new(return_type) },
                    CallBoundary::Internal,
                ),
            }),
        })
    }

    fn lower_function_compose(
        &mut self, context: &mut FunctionContext, call: &ast::Call, stdlib_call: &StdlibCall,
    ) -> Option<Expression> {
        let arguments = self.lower_ordered_call_arguments(context, call)?;
        self.validate_callback_argument(stdlib_call, call.span, arguments.first()?)?;
        self.validate_callback_argument(stdlib_call, call.span, arguments.get(1)?)?;
        let composed_type = self.typed_expression_type(call.span)?;
        let Type::Function { params, return_type } = composed_type.clone() else { return None };
        let input_type = params.first()?.clone();
        let outer_local_count = context.locals.len();
        context.push_scope();
        let input = self.synthetic_local(context, "compose_input", input_type, call.span);
        let g_call = Expression {
            type_: match &arguments[1].value.type_ {
                Type::Function { return_type, .. } => *return_type.clone(),
                _ => return None,
            },
            span: call.span,
            kind: ExpressionKind::IndirectCall(IndirectCall {
                callee: Box::new(arguments[1].value.clone()),
                arguments: vec![CallArgument { label: None, value: local_get(&input), span: call.span }],
                abi: call_abi(&arguments[1].value.type_, CallBoundary::Internal),
            }),
        };
        let body = Block {
            instructions: Vec::new(),
            result: Box::new(Expression {
                type_: *return_type,
                span: call.span,
                kind: ExpressionKind::IndirectCall(IndirectCall {
                    callee: Box::new(arguments[0].value.clone()),
                    arguments: vec![CallArgument { label: None, value: g_call, span: call.span }],
                    abi: call_abi(&arguments[0].value.type_, CallBoundary::Internal),
                }),
            }),
            span: call.span,
        };
        context.pop_scope();
        Some(closure::lower_synthetic_anonymous_function(
            self,
            context,
            call.span,
            outer_local_count,
            vec![input],
            body,
            &composed_type,
        ))
    }

    fn lower_function_flip(
        &mut self, context: &mut FunctionContext, call: &ast::Call, stdlib_call: &StdlibCall,
    ) -> Option<Expression> {
        let arguments = self.lower_ordered_call_arguments(context, call)?;
        self.validate_callback_argument(stdlib_call, call.span, arguments.first()?)?;
        let flipped_type = self.typed_expression_type(call.span)?;
        let Type::Function { params, return_type } = flipped_type.clone() else { return None };
        if params.len() != 2 {
            return None;
        }
        let outer_local_count = context.locals.len();
        context.push_scope();
        let first = self.synthetic_local(context, "flip_first", params[0].clone(), call.span);
        let second = self.synthetic_local(context, "flip_second", params[1].clone(), call.span);
        let body = Block {
            instructions: Vec::new(),
            result: Box::new(Expression {
                type_: *return_type,
                span: call.span,
                kind: ExpressionKind::IndirectCall(IndirectCall {
                    callee: Box::new(arguments[0].value.clone()),
                    arguments: vec![
                        CallArgument { label: None, value: local_get(&second), span: call.span },
                        CallArgument { label: None, value: local_get(&first), span: call.span },
                    ],
                    abi: call_abi(&arguments[0].value.type_, CallBoundary::Internal),
                }),
            }),
            span: call.span,
        };
        context.pop_scope();
        Some(closure::lower_synthetic_anonymous_function(
            self,
            context,
            call.span,
            outer_local_count,
            vec![first, second],
            body,
            &flipped_type,
        ))
    }

    fn validate_callback_argument(
        &mut self, stdlib_call: &StdlibCall, call_span: Span, argument: &CallArgument,
    ) -> Option<()> {
        let Type::Function { params, return_type } = &argument.value.type_ else {
            self.unsupported_callback_shape(
                stdlib_call,
                call_span,
                argument.span,
                "callback parameter",
                &argument.value.type_,
            );
            return None;
        };

        for param in params {
            if !callback_value_type_supported(param) {
                self.unsupported_callback_shape(stdlib_call, call_span, argument.span, "callback parameter", param);
                return None;
            }
        }
        if !callback_value_type_supported(return_type) {
            self.unsupported_callback_shape(stdlib_call, call_span, argument.span, "callback return", return_type);
            return None;
        }
        if let Some(type_) = unsupported_callback_capture_type(&argument.value) {
            self.unsupported_callback_shape(stdlib_call, call_span, argument.span, "callback capture", type_);
            return None;
        }
        if callback_crosses_host_boundary(&argument.value) {
            self.unsupported_callback_shape(
                stdlib_call,
                call_span,
                argument.span,
                "callback host boundary",
                &argument.value.type_,
            );
            return None;
        }
        Some(())
    }

    fn unsupported_callback_shape(
        &mut self, stdlib_call: &StdlibCall, call_span: Span, callback_span: Span, shape: &str, type_: &Type,
    ) {
        self.diagnostics.push(
            Diagnostic::new(
                DiagnosticCode::LoweringError,
                format!(
                    "stdlib intrinsic `{}.{}` does not support {shape} ABI shape `{:?}`",
                    stdlib_call.module, stdlib_call.member, type_
                ),
            )
            .with_label(Label::primary(callback_span, "unsupported callback ABI shape here"))
            .with_label(Label::primary(call_span, "callback passed to this intrinsic")),
        );
    }

    fn list_map_adapter(&mut self, input_type: &Type, output_type: &Type, callback_type: &Type, span: Span) -> String {
        let name = self.next_anonymous_name();
        let list_type = list(input_type.clone());
        let return_type = list(output_type.clone());
        let list_param = synthetic_param(0, "list", list_type.clone(), span);
        let callback_param = synthetic_param(1, "callback", callback_type.clone(), span);
        let head = synthetic_param(2, "head", input_type.clone(), span);
        let tail = synthetic_param(3, "tail", list_type.clone(), span);
        let mapped = Expression {
            type_: output_type.clone(),
            span,
            kind: ExpressionKind::IndirectCall(IndirectCall {
                callee: Box::new(local_get(&callback_param)),
                arguments: vec![CallArgument { label: None, value: local_get(&head), span }],
                abi: call_abi(callback_type, CallBoundary::Internal),
            }),
        };
        let recurse = Expression {
            type_: return_type.clone(),
            span,
            kind: ExpressionKind::DirectCall(DirectCall {
                function: name.clone(),
                arguments: vec![
                    CallArgument { label: None, value: local_get(&tail), span },
                    CallArgument { label: None, value: local_get(&callback_param), span },
                ],
                abi: call_abi(
                    &Type::Function {
                        params: vec![list_type.clone(), (*callback_type).clone()],
                        return_type: Box::new(return_type.clone()),
                    },
                    CallBoundary::Internal,
                ),
            }),
        };
        let body = branch_function_body(
            span,
            &return_type,
            local_get(&list_param),
            vec![
                (
                    IrPattern::List { elements: Vec::new(), tail: None },
                    Expression { type_: return_type.clone(), span, kind: ExpressionKind::List(Vec::new()) },
                ),
                (
                    IrPattern::List { elements: vec![IrPattern::Binding(head.id)], tail: Some(tail.id) },
                    Expression {
                        type_: return_type.clone(),
                        span,
                        kind: ExpressionKind::ListCons { head: Box::new(mapped), tail: Box::new(recurse) },
                    },
                ),
            ],
        );
        self.lifted_functions.push(Function {
            name: name.clone(),
            public: false,
            closure_captures: Vec::new(),
            params: vec![list_param.clone(), callback_param.clone()],
            locals: vec![list_param, callback_param, head, tail],
            return_type: return_type.clone(),
            abi: call_abi(
                &Type::Function {
                    params: vec![list_type, (*callback_type).clone()],
                    return_type: Box::new(return_type),
                },
                CallBoundary::Internal,
            ),
            body,
            span,
        });
        name
    }

    fn list_fold_adapter(&mut self, input_type: Type, acc_type: Type, callback_type: Type, span: Span) -> String {
        let name = self.next_anonymous_name();
        let list_type = list(input_type.clone());
        let list_param = synthetic_param(0, "list", list_type.clone(), span);
        let acc_param = synthetic_param(1, "acc", acc_type.clone(), span);
        let callback_param = synthetic_param(2, "callback", callback_type.clone(), span);
        let head = synthetic_param(3, "head", input_type, span);
        let tail = synthetic_param(4, "tail", list_type.clone(), span);
        let next_acc = Expression {
            type_: acc_type.clone(),
            span,
            kind: ExpressionKind::IndirectCall(IndirectCall {
                callee: Box::new(local_get(&callback_param)),
                arguments: vec![
                    CallArgument { label: None, value: local_get(&acc_param), span },
                    CallArgument { label: None, value: local_get(&head), span },
                ],
                abi: call_abi(&callback_type, CallBoundary::Internal),
            }),
        };
        let recurse = Expression {
            type_: acc_type.clone(),
            span,
            kind: ExpressionKind::DirectCall(DirectCall {
                function: name.clone(),
                arguments: vec![
                    CallArgument { label: None, value: local_get(&tail), span },
                    CallArgument { label: None, value: next_acc, span },
                    CallArgument { label: None, value: local_get(&callback_param), span },
                ],
                abi: call_abi(
                    &Type::Function {
                        params: vec![list_type.clone(), acc_type.clone(), callback_type.clone()],
                        return_type: Box::new(acc_type.clone()),
                    },
                    CallBoundary::Internal,
                ),
            }),
        };
        let body = branch_function_body(
            span,
            &acc_type,
            local_get(&list_param),
            vec![
                (
                    IrPattern::List { elements: Vec::new(), tail: None },
                    local_get(&acc_param),
                ),
                (
                    IrPattern::List { elements: vec![IrPattern::Binding(head.id)], tail: Some(tail.id) },
                    recurse,
                ),
            ],
        );
        self.lifted_functions.push(Function {
            name: name.clone(),
            public: false,
            closure_captures: Vec::new(),
            params: vec![list_param.clone(), acc_param.clone(), callback_param.clone()],
            locals: vec![list_param, acc_param, callback_param, head, tail],
            return_type: acc_type.clone(),
            abi: call_abi(
                &Type::Function {
                    params: vec![list_type, acc_type.clone(), callback_type],
                    return_type: Box::new(acc_type),
                },
                CallBoundary::Internal,
            ),
            body,
            span,
        });
        name
    }

    fn option_map_adapter(&mut self, input_type: Type, return_type: Type, callback_type: Type, span: Span) -> String {
        let name = self.next_anonymous_name();
        let option_param = synthetic_param(0, "option", option(input_type.clone()), span);
        let callback_param = synthetic_param(1, "callback", callback_type.clone(), span);
        let value = synthetic_param(2, "value", input_type, span);
        let mapped = Expression {
            type_: match &return_type {
                Type::Custom { args, .. } => args[0].clone(),
                _ => Type::Nil,
            },
            span,
            kind: ExpressionKind::IndirectCall(IndirectCall {
                callee: Box::new(local_get(&callback_param)),
                arguments: vec![CallArgument { label: None, value: local_get(&value), span }],
                abi: call_abi(&callback_type, CallBoundary::Internal),
            }),
        };
        let body = branch_function_body(
            span,
            &return_type,
            local_get(&option_param),
            vec![
                (
                    IrPattern::Constructor {
                        name: "Some".into(),
                        arguments: vec![ConstructorPatternArgument {
                            label: None,
                            pattern: IrPattern::Binding(value.id),
                            span,
                        }],
                    },
                    Expression {
                        type_: return_type.clone(),
                        span,
                        kind: ExpressionKind::Constructor(ConstructorValue {
                            name: "Some".into(),
                            arguments: vec![mapped],
                        }),
                    },
                ),
                (
                    IrPattern::Constructor { name: "None".into(), arguments: Vec::new() },
                    Expression {
                        type_: return_type.clone(),
                        span,
                        kind: ExpressionKind::Constructor(ConstructorValue {
                            name: "None".into(),
                            arguments: Vec::new(),
                        }),
                    },
                ),
            ],
        );
        self.lifted_functions.push(Function {
            name: name.clone(),
            public: false,
            closure_captures: Vec::new(),
            params: vec![option_param.clone(), callback_param.clone()],
            locals: vec![option_param, callback_param, value],
            return_type: return_type.clone(),
            abi: call_abi(
                &Type::Function {
                    params: vec![option(input_type_from_callback(&callback_type)), callback_type],
                    return_type: Box::new(return_type),
                },
                CallBoundary::Internal,
            ),
            body,
            span,
        });
        name
    }

    fn result_map_adapter(
        &mut self, ok_type: Type, error_type: Type, return_type: Type, callback_type: Type, span: Span,
    ) -> String {
        let name = self.next_anonymous_name();
        let result_param = synthetic_param(0, "result", result(ok_type.clone(), error_type.clone()), span);
        let callback_param = synthetic_param(1, "callback", callback_type.clone(), span);
        let ok_value = synthetic_param(2, "ok", ok_type.clone(), span);
        let error_value = synthetic_param(3, "error", error_type.clone(), span);
        let mapped = Expression {
            type_: match &return_type {
                Type::Custom { args, .. } => args[0].clone(),
                _ => Type::Nil,
            },
            span,
            kind: ExpressionKind::IndirectCall(IndirectCall {
                callee: Box::new(local_get(&callback_param)),
                arguments: vec![CallArgument { label: None, value: local_get(&ok_value), span }],
                abi: call_abi(&callback_type, CallBoundary::Internal),
            }),
        };
        let body = branch_function_body(
            span,
            &return_type,
            local_get(&result_param),
            vec![
                (
                    IrPattern::Constructor {
                        name: "Ok".into(),
                        arguments: vec![ConstructorPatternArgument {
                            label: None,
                            pattern: IrPattern::Binding(ok_value.id),
                            span,
                        }],
                    },
                    Expression {
                        type_: return_type.clone(),
                        span,
                        kind: ExpressionKind::Constructor(ConstructorValue {
                            name: "Ok".into(),
                            arguments: vec![mapped],
                        }),
                    },
                ),
                (
                    IrPattern::Constructor {
                        name: "Error".into(),
                        arguments: vec![ConstructorPatternArgument {
                            label: None,
                            pattern: IrPattern::Binding(error_value.id),
                            span,
                        }],
                    },
                    Expression {
                        type_: return_type.clone(),
                        span,
                        kind: ExpressionKind::Constructor(ConstructorValue {
                            name: "Error".into(),
                            arguments: vec![local_get(&error_value)],
                        }),
                    },
                ),
            ],
        );
        self.lifted_functions.push(Function {
            name: name.clone(),
            public: false,
            closure_captures: Vec::new(),
            params: vec![result_param.clone(), callback_param.clone()],
            locals: vec![result_param, callback_param, ok_value, error_value],
            return_type: return_type.clone(),
            abi: call_abi(
                &Type::Function {
                    params: vec![result(ok_type, error_type), callback_type],
                    return_type: Box::new(return_type),
                },
                CallBoundary::Internal,
            ),
            body,
            span,
        });
        name
    }

    fn synthetic_local(&self, context: &mut FunctionContext, name: &str, type_: Type, span: Span) -> Local {
        let name = ast::Name { span, text: name.into() };
        let local = context.allocate(&name, type_);
        context.bind(name.text, local.id);
        local
    }

    fn lower_ordered_call_arguments(
        &mut self, context: &mut FunctionContext, call: &ast::Call,
    ) -> Option<Vec<CallArgument>> {
        let arguments = self.lower_call_arguments(context, &call.arguments)?;
        let param_count = self.call_function_labels(call).map_or(arguments.len(), <[_]>::len);
        let order = call_argument_order(self.call_function_labels(call), &call.arguments, param_count).ok()?;
        self.order_call_arguments(arguments, order.indices, order.has_labels)
    }

    fn lower_call_arguments(
        &mut self, context: &mut FunctionContext, arguments: &[ast::Argument],
    ) -> Option<Vec<CallArgument>> {
        arguments
            .iter()
            .map(|argument| {
                Some(CallArgument {
                    label: argument.label.as_ref().map(|label| label.text.clone()),
                    value: self.lower_expression(context, &argument.value)?,
                    span: argument.span,
                })
            })
            .collect()
    }

    fn order_call_arguments(
        &self, arguments: Vec<CallArgument>, indices: Vec<usize>, has_labels: bool,
    ) -> Option<Vec<CallArgument>> {
        if !has_labels {
            return Some(arguments);
        }
        let mut ordered = vec![None; indices.len()];
        for (argument, index) in arguments.into_iter().zip(indices) {
            if index >= ordered.len() {
                ordered.resize_with(index + 1, || None);
            }
            ordered[index] = Some(argument);
        }
        Some(ordered.into_iter().flatten().collect())
    }

    fn lower_pattern(
        &mut self, context: &mut FunctionContext, pattern: &Pattern, subject_type: &Type,
    ) -> Option<IrPattern> {
        match pattern {
            Pattern::Discard(_) => Some(IrPattern::Discard),
            Pattern::Name(name) => {
                let local = context.allocate(name, subject_type.clone());
                context.bind(name.text.clone(), local.id);
                Some(IrPattern::Binding(local.id))
            }
            Pattern::Integer(literal) => Some(IrPattern::Literal(Literal {
                kind: LiteralKind::Int,
                source: literal.source.clone(),
            })),
            Pattern::Float(literal) => Some(IrPattern::Literal(Literal {
                kind: LiteralKind::Float,
                source: literal.source.clone(),
            })),
            Pattern::String(literal) => Some(IrPattern::Literal(Literal {
                kind: LiteralKind::String,
                source: literal.source.clone(),
            })),
            Pattern::Bool(literal) => Some(IrPattern::Literal(Literal {
                kind: LiteralKind::Bool,
                source: literal.source.clone(),
            })),
            Pattern::Nil(literal) => Some(IrPattern::Literal(Literal {
                kind: LiteralKind::Nil,
                source: literal.source.clone(),
            })),
            Pattern::Tuple(tuple) => {
                let element_types = match subject_type {
                    Type::Tuple(types) => types.clone(),
                    _ => vec![subject_type.clone(); tuple.elements.len()],
                };
                let elements = tuple
                    .elements
                    .iter()
                    .zip(element_types.iter())
                    .map(|(element, type_)| self.lower_pattern(context, element, type_))
                    .collect::<Option<Vec<_>>>()?;
                Some(IrPattern::Tuple(elements))
            }
            Pattern::List(list) => {
                let element_type = match subject_type {
                    Type::List(type_) => type_.as_ref().clone(),
                    _ => subject_type.clone(),
                };
                let elements = list
                    .elements
                    .iter()
                    .map(|element| self.lower_pattern(context, element, &element_type))
                    .collect::<Option<Vec<_>>>()?;
                let tail = match &list.tail {
                    Some(ast::ListPatternTail::Name(name)) => {
                        let local = context.allocate(name, subject_type.clone());
                        context.bind(name.text.clone(), local.id);
                        Some(local.id)
                    }
                    Some(ast::ListPatternTail::Discard(_)) | None => None,
                };
                Some(IrPattern::List { elements, tail })
            }
            Pattern::Constructor(constructor) => {
                let name = self.resolved_constructor_name(&constructor_name(&constructor.constructor));
                let arguments =
                    self.lower_constructor_pattern_arguments(context, &name, &constructor.arguments, subject_type)?;
                Some(IrPattern::Constructor { name, arguments })
            }
            Pattern::Alias(alias) => {
                let inner = self.lower_pattern(context, &alias.pattern, subject_type)?;
                let local = context.allocate(&alias.alias, subject_type.clone());
                context.bind(alias.alias.text.clone(), local.id);
                Some(IrPattern::Alias { pattern: Box::new(inner), local: local.id })
            }
            Pattern::BitString(raw) => Some(IrPattern::BitString(bit_string_pattern_segments(context, raw))),
            Pattern::Raw(raw) => {
                self.diagnostics.push(
                    Diagnostic::new(
                        DiagnosticCode::LoweringError,
                        format!("pattern `{}` cannot be lowered", raw.kind),
                    )
                    .with_label(Label::primary(raw.span, "unsupported pattern here")),
                );
                None
            }
        }
    }

    fn successful_bindings(&self, context: &FunctionContext, patterns: &[IrPattern]) -> Vec<SuccessfulBinding> {
        let mut bindings = Vec::new();
        for (subject, pattern) in patterns.iter().enumerate() {
            collect_successful_bindings(context, pattern, BindingPath::Subject(subject), &mut bindings);
        }
        bindings
    }

    fn bind_assert_pattern(
        &mut self, context: &mut FunctionContext, instructions: &mut Vec<Instruction>, value: &Expression,
        pattern: &IrPattern, span: Span,
    ) {
        match pattern {
            IrPattern::Binding(local) => {
                instructions.push(Instruction::LocalSet { local: *local, value: value.clone(), span })
            }
            IrPattern::Alias { pattern, local } => {
                self.bind_assert_pattern(context, instructions, value, pattern, span);
                instructions.push(Instruction::LocalSet { local: *local, value: value.clone(), span });
            }
            IrPattern::Discard
            | IrPattern::Literal(_)
            | IrPattern::Tuple(_)
            | IrPattern::List { .. }
            | IrPattern::Constructor { .. }
            | IrPattern::BitString(_) => {
                let _ = context;
            }
        }
    }

    pub fn typed_expression_type(&self, span: Span) -> Option<Type> {
        self.expression_types.get(&span).cloned()
    }

    pub fn nil_expression(&self, span: Span) -> Expression {
        Expression {
            type_: Type::Nil,
            span,
            kind: ExpressionKind::Literal(Literal { kind: LiteralKind::Nil, source: "Nil".into() }),
        }
    }

    fn lower_external_host_imports(&self, ast: &ast::Module) -> Vec<Function> {
        let mut imports = Vec::new();
        for declaration in &ast.declarations {
            match declaration {
                ast::Declaration::ExternalFunction(function) => {
                    if let Some(import) = self.lower_external_host_import(function) {
                        imports.push(import);
                    }
                }
                ast::Declaration::TargetGroup(group) => {
                    for declaration in &group.declarations {
                        if let ast::Declaration::ExternalFunction(function) = declaration
                            && let Some(import) = self.lower_external_host_import(function)
                        {
                            imports.push(import);
                        }
                    }
                }
                _ => {}
            }
        }
        imports.extend(self.lower_imported_external_host_imports());
        imports
    }

    fn lower_external_host_import(&self, function: &ast::ExternalFunction) -> Option<Function> {
        let type_ = self.function_types.get(&function.name.text)?.clone();
        let Type::Function { params, return_type } = type_.clone() else {
            return None;
        };
        let locals = params
            .iter()
            .enumerate()
            .map(|(index, type_)| Local {
                id: LocalId(index as u32),
                name: function
                    .parameters
                    .get(index)
                    .and_then(|parameter| parameter.name.as_ref())
                    .map(|name| name.text.clone())
                    .unwrap_or_else(|| format!("arg{index}")),
                type_: type_.clone(),
                span: function
                    .parameters
                    .get(index)
                    .map(|parameter| parameter.span)
                    .unwrap_or(function.span),
            })
            .collect::<Vec<_>>();
        let boundary = CallBoundary::HostImport {
            module: unquote(&function.body.module.source),
            name: unquote(&function.body.function.source),
        };
        Some(Function {
            name: function.name.text.clone(),
            public: false,
            closure_captures: Vec::new(),
            params: locals.clone(),
            locals,
            return_type: *return_type,
            abi: call_abi(&type_, boundary),
            body: Block {
                instructions: Vec::new(),
                result: Box::new(self.nil_expression(function.span)),
                span: function.span,
            },
            span: function.span,
        })
    }

    fn lower_imported_external_host_imports(&self) -> Vec<Function> {
        let mut imports = self
            .imported_external_imports
            .iter()
            .filter_map(|(name, import)| {
                self.lower_external_info_host_import(name, &import.external, &import.type_, import.span)
            })
            .collect::<Vec<_>>();
        imports.sort_by(|left, right| left.name.cmp(&right.name));
        imports
    }

    fn lower_external_info_host_import(
        &self, name: &str, external: &ExternalFunctionInfo, type_: &Type, span: Span,
    ) -> Option<Function> {
        let Type::Function { params, return_type } = type_.clone() else {
            return None;
        };
        let locals = params
            .iter()
            .enumerate()
            .map(|(index, type_)| Local {
                id: LocalId(index as u32),
                name: format!("arg{index}"),
                type_: type_.clone(),
                span,
            })
            .collect::<Vec<_>>();
        let boundary = CallBoundary::HostImport { module: external.module.clone(), name: external.function.clone() };
        Some(Function {
            name: name.to_string(),
            public: false,
            closure_captures: Vec::new(),
            params: locals.clone(),
            locals,
            return_type: *return_type,
            abi: call_abi(type_, boundary),
            body: Block { instructions: Vec::new(), result: Box::new(self.nil_expression(span)), span },
            span,
        })
    }

    fn external_import_boundary(&self, name: &str) -> Option<CallBoundary> {
        let import = self.external_imports.get(name)?;
        Some(CallBoundary::HostImport { module: import.module.clone(), name: import.function.clone() })
    }

    fn lower_stdlib_host_imports(&self, ast: &ast::Module) -> Vec<Function> {
        let used_host_calls = ast.used_stdlib_host_calls();
        let mut imports = Vec::new();
        for import in &ast.imports {
            let Some(module) = StdlibRegistry::new().module(&import.module.text).cloned() else {
                continue;
            };
            for member in module.members {
                let Some(adapter) = stdlib_host_adapter(module.name, member.name) else {
                    continue;
                };
                if !used_host_calls.contains(&(module.name.into(), member.name.into())) {
                    continue;
                }
                let Some(type_) = module.interface.functions.get(member.name).cloned() else {
                    continue;
                };
                let Type::Function { params, return_type } = type_.clone() else {
                    continue;
                };
                let locals = params
                    .iter()
                    .enumerate()
                    .map(|(index, type_)| Local {
                        id: LocalId(index as u32),
                        name: format!("arg{index}"),
                        type_: type_.clone(),
                        span: import.span,
                    })
                    .collect::<Vec<_>>();
                imports.push(Function {
                    name: stdlib_lowered_name(module.name, member.name),
                    public: false,
                    closure_captures: Vec::new(),
                    params: locals.clone(),
                    locals,
                    return_type: *return_type,
                    abi: call_abi(
                        &type_,
                        CallBoundary::HostImport {
                            module: adapter.import_module.into(),
                            name: adapter.import_name.into(),
                        },
                    ),
                    body: Block {
                        instructions: Vec::new(),
                        result: Box::new(self.nil_expression(import.span)),
                        span: import.span,
                    },
                    span: import.span,
                });
            }
        }
        imports
    }

    fn lower_constant(&self, id: ConstantId, constant: &ast::Constant) -> Constant {
        Constant {
            id,
            name: constant.name.text.clone(),
            public: constant.public,
            value: self.ast_constant_value(&constant.value),
            span: constant.span,
        }
    }

    fn ast_constant_value(&self, expression: &ast::Expression) -> ConstantValue {
        match expression {
            ast::Expression::Literal(literal) => {
                ConstantValue::Literal(Literal { kind: literal.kind.clone(), source: literal.source.clone() })
            }
            _ => ConstantValue::Raw(format!("{expression:?}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExternalImport {
    module: String,
    function: String,
}

impl From<&ExternalFunctionInfo> for ExternalImport {
    fn from(info: &ExternalFunctionInfo) -> Self {
        Self { module: info.module.clone(), function: info.function.clone() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ImportedExternalImport {
    external: ExternalFunctionInfo,
    type_: Type,
    span: Span,
}

fn external_imports(module: &ast::Module) -> HashMap<String, ExternalImport> {
    let mut imports = HashMap::new();
    for declaration in &module.declarations {
        collect_external_import(declaration, &mut imports);
    }
    imports
}

fn collect_external_import(declaration: &ast::Declaration, imports: &mut HashMap<String, ExternalImport>) {
    match declaration {
        ast::Declaration::ExternalFunction(function) => {
            imports.insert(
                function.name.text.clone(),
                ExternalImport {
                    module: unquote(&function.body.module.source),
                    function: unquote(&function.body.function.source),
                },
            );
        }
        ast::Declaration::TargetGroup(group) => {
            for declaration in &group.declarations {
                collect_external_import(declaration, imports);
            }
        }
        _ => {}
    }
}

#[derive(Debug, Clone)]
struct StdlibCall {
    module: String,
    member: String,
    implementation: Option<StdlibImplementation>,
    type_: Type,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StdlibImplementation {
    RuntimePrimitive,
    HostAdapter(StdlibHostAdapter),
}

fn stdlib_call(module: &ast::Module, function: &ast::Expression) -> Option<StdlibCall> {
    let registry = StdlibRegistry::new();
    let (module_name, member_name) = match function {
        ast::Expression::FieldAccess(access) => {
            let ast::Expression::Variable(module_alias) = access.record.as_ref() else {
                return None;
            };
            let import = module.imports.iter().find(|import| {
                let local = import
                    .alias
                    .as_ref()
                    .map(|alias| alias.text.as_str())
                    .unwrap_or_else(|| import.module.text.rsplit('/').next().unwrap_or(&import.module.text));
                local == module_alias.text
            })?;
            (import.module.text.clone(), access.field.text.clone())
        }
        ast::Expression::Variable(name) => module.imports.iter().find_map(|import| {
            import.unqualified.iter().find_map(|unqualified| {
                let local = unqualified.alias.as_ref().unwrap_or(&unqualified.name).text.as_str();
                (local == name.text).then(|| (import.module.text.clone(), unqualified.name.text.clone()))
            })
        })?,
        _ => return None,
    };
    let module = registry.module(&module_name)?;
    let type_ = module.interface.functions.get(&member_name)?.clone();
    let implementation = stdlib_member_implementation(&module_name, &member_name);
    Some(StdlibCall { module: module_name, member: member_name, implementation, type_ })
}

fn stdlib_member_implementation(module: &str, member: &str) -> Option<StdlibImplementation> {
    if crate::runtime::stdlib_runtime_primitive(module, member).is_some() {
        return Some(StdlibImplementation::RuntimePrimitive);
    }
    stdlib_host_adapter(module, member).map(StdlibImplementation::HostAdapter)
}

fn stdlib_lowered_name(module: &str, member: &str) -> String {
    format!("__stdlib_{}_{}", module.replace('/', "_"), member)
}

fn synthetic_param(id: u32, name: &str, type_: Type, span: Span) -> Local {
    Local { id: LocalId(id), name: name.into(), type_, span }
}

fn local_get(local: &Local) -> Expression {
    Expression { type_: local.type_.clone(), span: local.span, kind: ExpressionKind::LocalGet(local.id) }
}

fn branch_function_body(span: Span, type_: &Type, subject: Expression, clauses: Vec<(IrPattern, Expression)>) -> Block {
    Block {
        instructions: Vec::new(),
        result: Box::new(Expression {
            type_: type_.clone(),
            span,
            kind: ExpressionKind::Branch(Branch {
                subjects: vec![subject],
                clauses: clauses
                    .into_iter()
                    .map(|(pattern, body)| BranchClause {
                        patterns: vec![pattern],
                        guard: None,
                        bindings: Vec::new(),
                        body: Box::new(body),
                        span,
                    })
                    .collect(),
                fallthrough: FailurePath { reason: FailureReason::BranchFallthrough, span },
            }),
        }),
        span,
    }
}

fn list(item: Type) -> Type {
    Type::List(Box::new(item))
}

fn option(item: Type) -> Type {
    Type::custom("Option", vec![item])
}

fn result(ok: Type, error: Type) -> Type {
    Type::custom("Result", vec![ok, error])
}

fn input_type_from_callback(callback: &Type) -> Type {
    match callback {
        Type::Function { params, .. } => params.first().cloned().unwrap_or(Type::Nil),
        _ => Type::Nil,
    }
}

fn callback_value_type_supported(type_: &Type) -> bool {
    match type_ {
        Type::Nil | Type::Generic(_) | Type::Opaque { .. } => false,
        Type::Tuple(items) => items.iter().all(callback_value_type_supported),
        Type::List(item) => callback_value_type_supported(item),
        Type::Record { fields, .. } => fields.iter().all(|field| callback_value_type_supported(&field.type_)),
        Type::Custom { args, .. } => args.iter().all(callback_value_type_supported),
        Type::Function { params, return_type } => {
            params.iter().all(callback_value_type_supported) && callback_value_type_supported(return_type)
        }
        Type::Int | Type::Float | Type::String | Type::BitArray | Type::Bool => true,
    }
}

fn unsupported_callback_capture_type(expression: &Expression) -> Option<&Type> {
    match &expression.kind {
        ExpressionKind::AnonymousFunction(function) => function
            .captures
            .iter()
            .find_map(|capture| (!callback_value_type_supported(&capture.type_)).then_some(&capture.type_)),
        _ => None,
    }
}

fn callback_crosses_host_boundary(expression: &Expression) -> bool {
    match &expression.kind {
        ExpressionKind::FunctionValue(function) => matches!(
            function.abi.boundary,
            CallBoundary::HostImport { .. } | CallBoundary::ModuleImport { .. }
        ),
        ExpressionKind::AnonymousFunction(function) => {
            function.body.contains_expression(callback_crosses_host_boundary)
        }
        _ => false,
    }
}

trait UsedStdlibHostCalls {
    fn used_stdlib_host_calls(&self) -> HashSet<(String, String)>;
}

impl UsedStdlibHostCalls for ast::Module {
    fn used_stdlib_host_calls(&self) -> HashSet<(String, String)> {
        let mut calls = HashSet::new();
        for declaration in &self.declarations {
            collect_stdlib_host_calls_in_declaration(self, declaration, &mut calls);
        }
        calls
    }
}

fn collect_stdlib_host_calls_in_declaration(
    module: &ast::Module, declaration: &ast::Declaration, calls: &mut HashSet<(String, String)>,
) {
    match declaration {
        ast::Declaration::Function(function) => collect_stdlib_host_calls_in_block(module, &function.body, calls),
        ast::Declaration::Constant(constant) => collect_stdlib_host_calls_in_expression(module, &constant.value, calls),
        ast::Declaration::TargetGroup(group) => {
            for declaration in &group.declarations {
                collect_stdlib_host_calls_in_declaration(module, declaration, calls);
            }
        }
        ast::Declaration::Import(_)
        | ast::Declaration::ExternalFunction(_)
        | ast::Declaration::ExternalType(_)
        | ast::Declaration::TypeAlias(_)
        | ast::Declaration::TypeDefinition(_)
        | ast::Declaration::Attribute(_)
        | ast::Declaration::Comment(_)
        | ast::Declaration::Statement(_) => {}
    }
}

fn collect_stdlib_host_calls_in_block(module: &ast::Module, block: &ast::Block, calls: &mut HashSet<(String, String)>) {
    for statement in &block.statements {
        match statement {
            Statement::Let(let_) => collect_stdlib_host_calls_in_expression(module, &let_.value, calls),
            Statement::LetAssert(let_assert) => {
                collect_stdlib_host_calls_in_expression(module, &let_assert.value, calls);
                if let Some(message) = &let_assert.message {
                    collect_stdlib_host_calls_in_expression(module, message, calls);
                }
            }
            Statement::Expression(expression) => collect_stdlib_host_calls_in_expression(module, expression, calls),
        }
    }
}

fn collect_stdlib_host_calls_in_expression(
    module: &ast::Module, expression: &ast::Expression, calls: &mut HashSet<(String, String)>,
) {
    if let ast::Expression::Call(call) = expression
        && let Some(stdlib_call) = stdlib_call(module, &call.function)
        && matches!(stdlib_call.implementation, Some(StdlibImplementation::HostAdapter(_)))
    {
        calls.insert((stdlib_call.module, stdlib_call.member));
    }

    match expression {
        ast::Expression::Call(call) => {
            collect_stdlib_host_calls_in_expression(module, &call.function, calls);
            for argument in &call.arguments {
                collect_stdlib_host_calls_in_expression(module, &argument.value, calls);
            }
        }
        ast::Expression::FieldAccess(access) => collect_stdlib_host_calls_in_expression(module, &access.record, calls),
        ast::Expression::Block(block) => collect_stdlib_host_calls_in_block(module, block, calls),
        ast::Expression::Case(case) => {
            for subject in &case.subjects {
                collect_stdlib_host_calls_in_expression(module, subject, calls);
            }
            for clause in &case.clauses {
                if let Some(guard) = &clause.guard {
                    collect_stdlib_host_calls_in_expression(module, guard, calls);
                }
                collect_stdlib_host_calls_in_expression(module, &clause.value, calls);
            }
        }
        ast::Expression::BinaryOperation(operation) => {
            collect_stdlib_host_calls_in_expression(module, &operation.left, calls);
            collect_stdlib_host_calls_in_expression(module, &operation.right, calls);
        }
        ast::Expression::Pipeline(pipeline) => {
            collect_stdlib_host_calls_in_expression(module, &pipeline.value, calls);
            collect_stdlib_host_calls_in_expression(module, &pipeline.into, calls);
        }
        ast::Expression::UnaryOperation(operation) => {
            collect_stdlib_host_calls_in_expression(module, &operation.value, calls);
        }
        ast::Expression::Use(use_) => collect_stdlib_host_calls_in_expression(module, &use_.value, calls),
        ast::Expression::AnonymousFunction(function) => {
            collect_stdlib_host_calls_in_block(module, &function.body, calls)
        }
        ast::Expression::Capture(capture) => {
            collect_stdlib_host_calls_in_expression(module, &capture.function, calls);
            for argument in capture.arguments.iter().flatten() {
                collect_stdlib_host_calls_in_expression(module, &argument.value, calls);
            }
        }
        ast::Expression::Record(record) => {
            for argument in &record.arguments {
                collect_stdlib_host_calls_in_expression(module, &argument.value, calls);
            }
        }
        ast::Expression::RecordUpdate(update) => {
            collect_stdlib_host_calls_in_expression(module, &update.spread, calls);
            for argument in &update.updates {
                collect_stdlib_host_calls_in_expression(module, &argument.value, calls);
            }
        }
        ast::Expression::Tuple(tuple) => {
            for element in &tuple.elements {
                collect_stdlib_host_calls_in_expression(module, element, calls);
            }
        }
        ast::Expression::TupleAccess(access) => collect_stdlib_host_calls_in_expression(module, &access.tuple, calls),
        ast::Expression::List(list) => {
            for element in &list.elements {
                collect_stdlib_host_calls_in_expression(module, element, calls);
            }
            if let Some(spread) = &list.spread {
                collect_stdlib_host_calls_in_expression(module, spread, calls);
            }
        }
        ast::Expression::Panic(failure) | ast::Expression::Todo(failure) => {
            if let Some(message) = &failure.message {
                collect_stdlib_host_calls_in_expression(module, message, calls);
            }
        }
        ast::Expression::Assert(assert) => collect_stdlib_host_calls_in_expression(module, &assert.value, calls),
        ast::Expression::Echo(echo) => collect_stdlib_host_calls_in_expression(module, &echo.value, calls),
        ast::Expression::Literal(_)
        | ast::Expression::Variable(_)
        | ast::Expression::BitArray(_)
        | ast::Expression::Raw(_) => {}
    }
}

fn constructor_type_substitutions(info: &ConstructorInfo, subject_type: &Type) -> HashMap<String, Type> {
    let (Type::Custom { name: return_name, args: return_args } | Type::Opaque { name: return_name, args: return_args }) =
        &info.return_type
    else {
        return HashMap::new();
    };
    let (Type::Custom { name: subject_name, args: subject_args }
    | Type::Opaque { name: subject_name, args: subject_args }) = subject_type
    else {
        return HashMap::new();
    };
    if return_name != subject_name || return_args.len() != subject_args.len() {
        return HashMap::new();
    }
    return_args
        .iter()
        .zip(subject_args.iter())
        .filter_map(|(parameter, argument)| match parameter {
            Type::Generic(name) => Some((name.clone(), argument.clone())),
            _ => None,
        })
        .collect()
}

fn substitute_type_generics(type_: &Type, substitutions: &HashMap<String, Type>) -> Type {
    match type_ {
        Type::Generic(name) => substitutions
            .get(name)
            .cloned()
            .unwrap_or_else(|| Type::Generic(name.clone())),
        Type::Tuple(items) => Type::Tuple(
            items
                .iter()
                .map(|item| substitute_type_generics(item, substitutions))
                .collect(),
        ),
        Type::List(item) => Type::List(Box::new(substitute_type_generics(item, substitutions))),
        Type::Record { name, fields } => Type::Record {
            name: name.clone(),
            fields: fields
                .iter()
                .map(|field| {
                    FieldInfo::new(
                        field.name.clone(),
                        substitute_type_generics(&field.type_, substitutions),
                    )
                })
                .collect(),
        },
        Type::Custom { name, args } => Type::Custom {
            name: name.clone(),
            args: args
                .iter()
                .map(|arg| substitute_type_generics(arg, substitutions))
                .collect(),
        },
        Type::Opaque { name, args } => Type::Opaque {
            name: name.clone(),
            args: args
                .iter()
                .map(|arg| substitute_type_generics(arg, substitutions))
                .collect(),
        },
        Type::Function { params, return_type } => Type::Function {
            params: params
                .iter()
                .map(|param| substitute_type_generics(param, substitutions))
                .collect(),
            return_type: Box::new(substitute_type_generics(return_type, substitutions)),
        },
        Type::Int | Type::Float | Type::String | Type::BitArray | Type::Bool | Type::Nil => type_.clone(),
    }
}

#[derive(Default)]
pub struct FunctionContext {
    pub locals: Vec<Local>,
    scopes: Vec<HashMap<String, LocalId>>,
}

impl FunctionContext {
    pub fn allocate(&mut self, name: &ast::Name, type_: Type) -> Local {
        let local = Local { id: LocalId(self.locals.len() as u32), name: name.text.clone(), type_, span: name.span };
        self.locals.push(local.clone());
        local
    }

    pub fn local(&self, id: LocalId) -> &Local {
        &self.locals[id.0 as usize]
    }

    pub fn bind(&mut self, name: String, local: LocalId) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, local);
        }
    }

    fn lookup(&self, name: &str) -> Option<LocalId> {
        for scope in self.scopes.iter().rev() {
            if let Some(local) = scope.get(name) {
                return Some(*local);
            }
        }
        None
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{SourceFile, SourceFileId};
    use crate::{ast, parse, project, resolve, types};
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::tempdir;

    fn lower_source(source: &str) -> Module {
        let source = SourceFile::new(SourceFileId(0), source);
        let cst = parse::parse(source).expect("parse source");
        let ast = ast::build(&cst).expect("build ast");
        let resolved = resolve::resolve(ast).expect("resolve names");
        let typed = types::check(resolved).expect("type check source");
        lower(typed).expect("lower source")
    }

    fn lower_source_err(source: &str) -> Diagnostics {
        let source = SourceFile::new(SourceFileId(0), source);
        let cst = parse::parse(source).expect("parse source");
        let ast = ast::build(&cst).expect("build ast");
        let resolved = resolve::resolve(ast).expect("resolve names");
        let typed = types::check(resolved).expect("type check source");
        lower(typed).expect_err("lowering should fail")
    }

    fn fixture_project(path: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("fixtures/projects")
            .join(path)
    }

    fn lower_project_fixture(path: &str) -> Module {
        let project = project::load_project(fixture_project(path)).expect("load project fixture");
        let typed = types::check_project(&project).expect("type check project fixture");
        lower_project(typed).expect("lower project fixture")
    }

    #[test]
    fn lowers_project_functions_to_structured_backend_names() {
        let dir = tempdir().expect("tempdir");
        fs::write(
            dir.path().join("gleam.toml"),
            "name = \"sample-app\"\nversion = \"1.0.0\"\n",
        )
        .expect("write manifest");
        fs::create_dir_all(dir.path().join("src")).expect("create src");
        fs::write(
            dir.path().join("src/app.gleam"),
            "pub fn id(x: Int) -> Int { x }\npub fn box(f: fn(Int) -> Int) -> fn(Int) -> Int { f }\n",
        )
        .expect("write app");
        fs::write(
            dir.path().join("src/main.gleam"),
            "import app\npub fn id(x: Int) -> Int { x + 1 }\npub fn run() -> Int { app.id(id(1)) }\npub fn value() -> fn(Int) -> Int { app.box(id) }\n",
        )
        .expect("write main");
        let project = project::load_project(dir.path()).expect("load project");
        let typed = types::check_project(&project).expect("type check project");

        let module = lower_project(typed).expect("lower project");

        let names = module
            .functions
            .iter()
            .map(|function| function.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names.len(), 5);
        assert!(names.iter().all(|name| name.starts_with("r$pkg$")));
        assert_eq!(names.iter().collect::<std::collections::HashSet<_>>().len(), 5);
        let run_export = module
            .exports
            .iter()
            .find(|export| export.name == "run")
            .expect("run export");
        assert_ne!(run_export.name, run_export.backend_name());
        assert!(run_export.backend_name().ends_with("$fn$x72756e"));
        let run = module
            .functions
            .iter()
            .find(|function| function.name.ends_with("$fn$x72756e"))
            .expect("run function");
        let ExpressionKind::DirectCall(outer) = &run.body.result.kind else {
            panic!("expected imported direct call");
        };
        assert!(outer.function.contains("$mod$x617070$fn$x6964"));
        let ExpressionKind::DirectCall(inner) = &outer.arguments[0].value.kind else {
            panic!("expected local direct call");
        };
        assert!(
            inner.function.contains("$mod$x6d61696e$fn$x6964"),
            "inner function was {}",
            inner.function
        );
        let value = module
            .functions
            .iter()
            .find(|function| function.name.ends_with("$fn$x76616c7565"))
            .expect("value function");
        let ExpressionKind::DirectCall(box_call) = &value.body.result.kind else {
            panic!("expected boxed function value call");
        };
        let ExpressionKind::FunctionValue(function_value) = &box_call.arguments[0].value.kind else {
            panic!("expected renamed function value");
        };
        assert!(function_value.name.contains("$mod$x6d61696e$fn$x6964"));
    }

    #[test]
    fn fixture_duplicate_function_names_link_without_collision() {
        let module = lower_project_fixture("generated_names/duplicate_function_names");

        let id_names = module
            .linked_names
            .iter()
            .filter(|name| name.source_name.ends_with(".id"))
            .collect::<Vec<_>>();
        assert_eq!(id_names.len(), 2);
        assert!(
            id_names
                .iter()
                .any(|name| name.source_name == "duplicate_function_names:left.id")
        );
        assert!(
            id_names
                .iter()
                .any(|name| name.source_name == "duplicate_function_names:right.id")
        );
        assert_ne!(id_names[0].generated_name, id_names[1].generated_name);
        assert_eq!(module.functions.len(), 3);
    }

    #[test]
    fn fixture_duplicate_module_basenames_link_without_collision() {
        let module = lower_project_fixture("generated_names/duplicate_module_basenames");

        let value_names = module
            .linked_names
            .iter()
            .filter(|name| name.source_name.ends_with(".value"))
            .collect::<Vec<_>>();
        assert_eq!(value_names.len(), 2);
        assert!(
            value_names
                .iter()
                .any(|name| name.source_name == "duplicate_module_basenames:alpha/main.value")
        );
        assert!(
            value_names
                .iter()
                .any(|name| name.source_name == "duplicate_module_basenames:beta/main.value")
        );
        assert_ne!(value_names[0].generated_name, value_names[1].generated_name);
    }

    #[test]
    fn fixture_dependency_module_name_overlap_keeps_root_package_names() {
        let module = lower_project_fixture("generated_names/dependency_module_overlap");

        let value_names = module
            .linked_names
            .iter()
            .filter(|name| name.source_name.ends_with(":shared.value"))
            .collect::<Vec<_>>();
        assert_eq!(value_names.len(), 2);
        assert!(value_names.iter().any(|name| {
            name.generated_name
                .contains("$pkg$x646570656e64656e63795f6d6f64756c655f6f7665726c6170$")
        }));
        assert!(
            value_names
                .iter()
                .any(|name| name.generated_name.contains("$pkg$x6f7665726c61705f646570$"))
        );
        assert_ne!(value_names[0].generated_name, value_names[1].generated_name);
        assert!(
            value_names
                .iter()
                .all(|name| name.generated_name.contains("$mod$x736861726564$"))
        );

        let root_shared = value_names
            .iter()
            .find(|name| name.source_name == "dependency_module_overlap:shared.value")
            .expect("root shared value")
            .generated_name
            .clone();
        let dependency_shared = value_names
            .iter()
            .find(|name| name.source_name == "overlap_dep:shared.value")
            .expect("dependency shared value")
            .generated_name
            .clone();
        let root_caller = module
            .functions
            .iter()
            .find(|function| function.name.ends_with("$fn$x726f6f745f76616c7565"))
            .expect("root caller");
        let dependency_caller = module
            .functions
            .iter()
            .find(|function| function.name.ends_with("$fn$x646570656e64656e63795f76616c7565"))
            .expect("dependency caller");
        assert!(matches!(
            root_caller.body.result.kind,
            ExpressionKind::DirectCall(DirectCall { ref function, .. }) if function == &root_shared
        ));
        assert!(matches!(
            dependency_caller.body.result.kind,
            ExpressionKind::DirectCall(DirectCall { ref function, .. }) if function == &dependency_shared
        ));
    }

    #[test]
    fn fixture_lifted_closures_receive_generated_helper_names() {
        let module = lower_project_fixture("generated_names/lifted_closures");

        let helper = module
            .linked_names
            .iter()
            .find(|name| name.generated_name.contains("$helper$lifted$"))
            .expect("lifted closure helper name");
        assert_eq!(helper.kind, LinkedNameKind::Helper);
        assert_eq!(helper.source_name, "lifted_closures:main.__anon_0");
        assert!(
            module
                .functions
                .iter()
                .any(|function| function.name == helper.generated_name)
        );
    }

    #[test]
    fn fixture_cross_module_features_rewrite_constructors_records_and_patterns() {
        let module = lower_project_fixture("linking/cross_module_features");

        let user = module
            .linked_names
            .iter()
            .find(|name| name.source_name == "cross_module_features:domain.User")
            .expect("User constructor name");
        let ready = module
            .linked_names
            .iter()
            .find(|name| name.source_name == "cross_module_features:domain.Status")
            .expect("Status constructor name");
        assert_eq!(user.kind, LinkedNameKind::Constructor);
        assert_eq!(ready.kind, LinkedNameKind::Constructor);
        assert!(
            module
                .functions
                .iter()
                .any(|function| function.name.ends_with("$fn$x72756e"))
        );
        assert!(
            module
                .functions
                .iter()
                .any(|function| function.name.ends_with("$fn$x6269727468646179"))
        );
        let dump = module.linked_debug_dump();
        assert!(dump.contains("Constructor source=cross_module_features:domain.User generated="));
        assert!(dump.contains("Constructor source=cross_module_features:domain.Status generated="));
        assert!(dump.contains("Function source=cross_module_features:domain.private_base generated="));
        assert!(!dump.contains("host-import wrapper"));
    }

    #[test]
    fn namespaces_project_host_import_wrappers_without_mangling_abi_names() {
        let dir = tempdir().expect("tempdir");
        fs::write(
            dir.path().join("gleam.toml"),
            "name = \"host-app\"\nversion = \"1.0.0\"\n",
        )
        .expect("write manifest");
        fs::create_dir_all(dir.path().join("src")).expect("create src");
        fs::write(
            dir.path().join("src/main.gleam"),
            "external fn inc(value: Int) -> Int = \"env\" \"host_inc\"\npub fn run() -> Int { inc(1) }\n",
        )
        .expect("write main");
        let project = project::load_project(dir.path()).expect("load project");
        let typed = types::check_project(&project).expect("type check project");

        let module = lower_project(typed).expect("lower project");

        let import = module
            .functions
            .iter()
            .find(|function| matches!(function.abi.boundary, CallBoundary::HostImport { .. }))
            .expect("host import wrapper");
        assert!(import.name.starts_with("r$pkg$"));
        assert!(import.name.contains("$helper$import_wrapper$"));
        assert!(matches!(
            &import.abi.boundary,
            CallBoundary::HostImport { module, name } if module == "env" && name == "host_inc"
        ));
        let run = module
            .functions
            .iter()
            .find(|function| function.name.ends_with("$fn$x72756e"))
            .expect("run function");
        let ExpressionKind::DirectCall(call) = &run.body.result.kind else {
            panic!("expected direct call");
        };
        assert_eq!(call.function, import.name);
    }

    #[test]
    fn lowers_function_params_and_let_locals() {
        let module = lower_source("fn id(x: Int) -> Int { let y = x y }");
        let function = &module.functions[0];
        assert_eq!(function.params.len(), 1);
        assert_eq!(function.locals.len(), 2);
        assert_eq!(function.body.instructions.len(), 1);
        assert_eq!(function.body.result.kind, ExpressionKind::LocalGet(LocalId(1)));
    }

    #[test]
    fn lowers_direct_function_calls() {
        let module = lower_source("fn id(x: Int) -> Int { x }\nfn main() { id(1) }");
        let main = module
            .functions
            .iter()
            .find(|function| function.name == "main")
            .expect("main function");

        assert!(
            matches!(main.body.result.kind, ExpressionKind::DirectCall(DirectCall { ref function, .. }) if function == "id")
        );
    }

    #[test]
    fn lowers_case_to_branch() {
        let module = lower_source("fn main(x: Int) { case x { 0 -> 1 _ -> 2 } }");
        let main = &module.functions[0];
        assert!(matches!(main.body.result.kind, ExpressionKind::Branch(_)));
    }

    #[test]
    fn lowers_module_constants_and_initialization_order() {
        let module = lower_source("pub const answer = 42\nconst greeting = \"hi\"\nfn main() { 1 }");

        assert_eq!(module.constants.len(), 2);
        assert_eq!(module.constants[0].name, "answer");
        assert!(module.constants[0].public);
        assert_eq!(module.constants[1].name, "greeting");
        assert_eq!(
            module.init.steps,
            vec![
                InitStep::RuntimeSetup { span: module.span },
                InitStep::Constant { constant: ConstantId(0), span: module.constants[0].span },
                InitStep::StaticData { name: "greeting".into(), span: module.constants[1].span },
                InitStep::Constant { constant: ConstantId(1), span: module.constants[1].span },
            ]
        );
        assert!(module.exports.iter().any(|export| export.name == "answer"));
    }

    #[test]
    fn records_declaration_metadata_exports_and_references() {
        let module = lower_source("pub const answer = 42\npub fn main(x: Int) { x }");
        assert!(matches!(module.declarations[0].kind, DeclarationKind::Constant));
        assert_eq!(module.declarations[0].visibility, Visibility::Public);
        assert!(module.exports.iter().any(|export| export.name == "main"));
        assert!(module.references.iter().any(|reference| reference.name == "x"));
    }

    #[test]
    fn lowers_managed_value_patterns_with_explicit_bindings_and_fallthrough() {
        let module = lower_source(include_str!("../../../../fixtures/ir/core_control_flow.gleam"));
        let choose = module
            .functions
            .iter()
            .find(|function| function.name == "choose")
            .expect("choose");
        let ExpressionKind::Branch(branch) = &choose.body.result.kind else {
            panic!("expected branch");
        };
        assert_eq!(branch.clauses.len(), 3);
        assert_eq!(branch.fallthrough.reason, FailureReason::BranchFallthrough);
        assert!(matches!(branch.clauses[0].patterns[0], IrPattern::Constructor { .. }));
        assert!(!branch.clauses[0].bindings.is_empty());

        let first = module
            .functions
            .iter()
            .find(|function| function.name == "first")
            .expect("first");
        let ExpressionKind::Branch(branch) = &first.body.result.kind else {
            panic!("expected branch");
        };
        assert!(matches!(branch.clauses[0].patterns[0], IrPattern::Tuple(_)));
    }

    #[test]
    fn snapshots_real_language_ir_fixture() {
        let module = lower_source(include_str!("../../../../fixtures/ir/core_control_flow.gleam"));

        insta::assert_debug_snapshot!("core_control_flow_ir", module);
    }

    #[test]
    fn emits_managed_constructor_ir_to_wat() {
        let module = lower_source("type Box { Box }\nfn main() { Box }");
        let wat = module.emit_wat().expect("emit managed constructor");

        assert!(wat.contains("(memory 1 256)"));
        assert!(wat.contains("(data (memory 0)"));
    }

    #[test]
    fn rejects_generic_runtime_types_before_ir_emission() {
        let diagnostics = lower_source_err("pub fn id(x) { x }");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("cannot be lowered without monomorphization")
        }));
    }

    #[test]
    fn rejects_unsupported_callback_abi_shapes_before_wasm_emission() {
        let diagnostics = lower_source_err(
            r#"import gleam/list

pub fn main() {
  list.map([Nil], fn(x) { x })
}
"#,
        );

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("gleam/list.map")
                && diagnostic.message.contains("callback parameter")
                && diagnostic.message.contains("Nil")
        }));
    }

    #[test]
    fn lowers_function_values_and_indirect_calls() {
        let module = lower_source("fn apply(x: Int, f: fn(Int) -> Int) -> Int { f(x) }");
        let apply = &module.functions[0];
        assert!(matches!(apply.body.result.kind, ExpressionKind::IndirectCall(_)));
        assert_eq!(apply.abi.params.len(), 2);
    }

    #[test]
    fn lowers_partial_application_to_lifted_closure_with_captures() {
        let module = lower_source(
            r#"fn add(a: Int, b: Int) -> Int { a + b }
fn main(x: Int) -> fn(Int) -> Int { add(x, _) }
"#,
        );
        let main = module
            .functions
            .iter()
            .find(|function| function.name == "main")
            .expect("main");
        let ExpressionKind::AnonymousFunction(callback) = &main.body.result.kind else {
            panic!("expected partial application closure");
        };
        let lifted = module
            .functions
            .iter()
            .find(|function| function.name == callback.name)
            .expect("lifted closure");

        assert_eq!(callback.captures.len(), 1);
        assert_eq!(callback.params.len(), 1);
        assert_eq!(lifted.closure_captures, vec![Type::Int]);
    }

    #[test]
    fn lowers_named_functions_as_values() {
        let module = lower_source("fn id(x: Int) -> Int { x }\nfn main() { id }");
        let main = module
            .functions
            .iter()
            .find(|function| function.name == "main")
            .expect("main function");

        assert!(matches!(
            main.body.result.kind,
            ExpressionKind::FunctionValue(FunctionValue { ref name, .. }) if name == "id"
        ));
    }

    #[test]
    fn lowers_bit_arrays_to_managed_value_forms() {
        let module = lower_source("fn bits() { <<1, 2:size(4), 3>> }");
        let bits = &module.functions[0];

        assert!(matches!(
            bits.body.result.kind,
            ExpressionKind::BitArray(BitArrayLiteral { bit_len: 20, .. })
        ));
    }

    #[test]
    fn represents_runtime_managed_value_types() {
        assert_eq!(
            RepresentationType::from(&Type::Int),
            RepresentationType::Scalar(ScalarRepresentation::I64)
        );
        assert_eq!(
            RepresentationType::from(&Type::List(Box::new(Type::Int))),
            RepresentationType::HeapManaged(HeapRepresentation::List)
        );
        assert_eq!(
            RepresentationType::from(&Type::String),
            RepresentationType::HeapManaged(HeapRepresentation::String)
        );
        assert_eq!(
            RepresentationType::from(&Type::BitArray),
            RepresentationType::HeapManaged(HeapRepresentation::BitArray)
        );
    }

    #[test]
    fn lowers_custom_type_constructors_to_managed_value_forms() {
        let module = lower_source("type Box { Box }\nfn main() { Box }");
        let main = &module.functions[0];
        assert!(matches!(
            main.body.result.kind,
            ExpressionKind::Constructor(ConstructorValue { ref name, .. }) if name == "Box"
        ));
    }

    #[test]
    fn core_ir_debug_output_is_deterministic() {
        let module = lower_source("fn id(x: Int) -> Int { x }");
        assert_eq!(format!("{module:#?}"), format!("{module:#?}"));
    }
}

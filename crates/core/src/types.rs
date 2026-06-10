// Copyright (c) 2026, Owais Jamil

//! Type checking and type inference for resolved Gleam modules.
//!
//! This module owns the compiler's current type representation, module
//! interface model, expression typing records, pattern type checks,
//! exhaustiveness inputs, and project-level type checking glue. The checker
//! uses the shared inference engine for substitutions and unification so
//! unannotated functions, local values, generic constructors, and imported
//! generic functions can infer stable types before lowering.

use std::collections::HashMap;

use crate::{
    ast::{self, Declaration, Expression, LiteralKind, Pattern, Statement},
    diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, Label},
    inference::{
        ConstraintGenerationError, ConstraintGenerator, Environment, Field, InferenceVariable, Scheme, Substitutions,
        TypeTerm, UnificationError, Unifier,
    },
    labels::{ArgumentLabelError, FunctionLabelMap, call_argument_order, function_label_map, use_callback_placement},
    project::Project,
    resolve::{self, ResolvedModule},
    source::Span,
    stdlib::StdlibRegistry,
};

/// A type known to the compiler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Int,
    Float,
    String,
    BitArray,
    Bool,
    Nil,
    Tuple(Vec<Type>),
    List(Box<Type>),
    Record { name: String, fields: Vec<FieldInfo> },
    Custom { name: String, args: Vec<Type> },
    Generic(String),
    Opaque { name: String, args: Vec<Type> },
    Function { params: Vec<Type>, return_type: Box<Type> },
}

impl From<LiteralKind> for Type {
    fn from(kind: LiteralKind) -> Self {
        match kind {
            LiteralKind::Int => Self::Int,
            LiteralKind::Float => Self::Float,
            LiteralKind::String => Self::String,
            LiteralKind::Bool => Self::Bool,
            LiteralKind::Nil => Self::Nil,
        }
    }
}

impl From<&LiteralKind> for Type {
    fn from(kind: &LiteralKind) -> Self {
        kind.clone().into()
    }
}

impl Type {
    pub fn has_generic(&self) -> bool {
        match self {
            Self::Generic(_) => true,
            Self::Tuple(items) => items.iter().any(Self::has_generic),
            Self::List(item) => item.has_generic(),
            Self::Record { fields, .. } => fields.iter().any(|field| field.type_.has_generic()),
            Self::Custom { args, .. } | Self::Opaque { args, .. } => args.iter().any(Self::has_generic),
            Self::Function { params, return_type } => params.iter().any(Self::has_generic) || return_type.has_generic(),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldInfo {
    pub name: String,
    pub type_: Type,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstructorInfo {
    pub name: String,
    pub fields: Vec<FieldInfo>,
    pub return_type: Type,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeDeclaration {
    pub name: String,
    pub parameters: Vec<String>,
    pub opaque: bool,
    pub constructors: Vec<ConstructorInfo>,
    pub span: Span,
}

impl From<ast::TypeAlias> for TypeDeclaration {
    fn from(alias: ast::TypeAlias) -> Self {
        TypeDeclaration {
            name: alias.name.text,
            parameters: alias.parameters,
            opaque: alias.opaque,
            constructors: Vec::new(),
            span: alias.span,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModuleInterface {
    pub functions: HashMap<String, Type>,
    pub function_labels: FunctionLabelMap,
    pub types: HashMap<String, TypeDeclaration>,
    pub constructors: HashMap<String, ConstructorInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedExpression {
    pub span: Span,
    pub type_: Type,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedFunction {
    pub name: ast::Name,
    pub type_: Type,
}

/// Resolved module annotated with type information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedModule {
    pub resolved: ResolvedModule,
    pub functions: Vec<TypedFunction>,
    pub expressions: Vec<TypedExpression>,
    pub function_labels: FunctionLabelMap,
    pub interface: ModuleInterface,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedProject {
    pub modules: Vec<TypedModule>,
    pub interfaces: HashMap<String, ModuleInterface>,
}

pub fn check(module: ResolvedModule) -> Result<TypedModule, Diagnostics> {
    TypeChecker::new(module).check()
}

fn check_with_externals(
    module: ResolvedModule, external_constructors: HashMap<String, ConstructorInfo>,
    external_values: HashMap<String, Type>, external_function_labels: FunctionLabelMap,
) -> Result<TypedModule, Diagnostics> {
    TypeChecker::new(module)
        .with_external_constructors(external_constructors)
        .with_external_values(external_values)
        .with_external_function_labels(external_function_labels)
        .check()
}

pub fn check_project(project: &Project) -> Result<TypedProject, Diagnostics> {
    let resolved = resolve::resolve_project(project)?;
    let mut modules = Vec::new();
    let mut interfaces = HashMap::new();
    let mut diagnostics = Vec::new();

    let stdlib_interfaces = StdlibRegistry::new()
        .modules()
        .map(|module| (module.name.to_string(), module.interface.clone()))
        .collect::<HashMap<_, _>>();
    interfaces.extend(stdlib_interfaces.clone());

    let external_constructors = resolved
        .modules
        .iter()
        .flat_map(|module| constructors_from_ast(&module.ast))
        .chain(stdlib_interfaces.values().flat_map(interface_constructors))
        .collect::<HashMap<_, _>>();
    let stdlib_values = stdlib_interfaces
        .iter()
        .flat_map(|(module, interface)| qualified_values_from_interface(module, interface))
        .collect::<HashMap<_, _>>();
    let mut external_values = resolved
        .modules
        .iter()
        .flat_map(|module| values_from_ast(&module.ast))
        .chain(stdlib_values)
        .collect::<HashMap<_, _>>();
    let mut external_function_labels = resolved
        .modules
        .iter()
        .flat_map(|module| function_label_map(&module.ast))
        .collect::<HashMap<_, _>>();

    for (module_info, module) in project.graph.modules.iter().zip(resolved.modules) {
        match check_with_externals(
            module,
            external_constructors.clone(),
            external_values.clone(),
            external_function_labels.clone(),
        ) {
            Ok(typed) => {
                for (name, type_) in &typed.interface.functions {
                    external_values.insert(name.clone(), type_.clone());
                    external_values.insert(format!("{}.{}", module_info.name, name), type_.clone());
                }
                for (name, labels) in &typed.interface.function_labels {
                    external_function_labels.insert(name.clone(), labels.clone());
                    external_function_labels.insert(format!("{}.{}", module_info.name, name), labels.clone());
                }
                interfaces.insert(module_info.name.clone(), typed.interface.clone());
                modules.push(typed);
            }
            Err(mut errors) => diagnostics.append(&mut errors),
        }
    }

    if diagnostics.is_empty() { Ok(TypedProject { modules, interfaces }) } else { Err(diagnostics) }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScopedType {
    type_: Type,
    generalized: bool,
}

struct TypeChecker {
    module: ResolvedModule,
    function_types: HashMap<String, Type>,
    external_values: HashMap<String, Type>,
    constructors: HashMap<String, ConstructorInfo>,
    function_labels: HashMap<String, Vec<Option<String>>>,
    interface: ModuleInterface,
    functions: Vec<TypedFunction>,
    expressions: Vec<TypedExpression>,
    scopes: Vec<HashMap<String, ScopedType>>,
    diagnostics: Diagnostics,
    inference_substitutions: Substitutions,
    next_inference_variable: usize,
}

impl TypeChecker {
    fn new(module: ResolvedModule) -> Self {
        let function_labels = function_label_map(&module.ast);
        Self {
            module,
            function_types: HashMap::new(),
            external_values: HashMap::new(),
            constructors: HashMap::new(),
            function_labels,
            interface: ModuleInterface::default(),
            functions: Vec::new(),
            expressions: Vec::new(),
            scopes: Vec::new(),
            diagnostics: Vec::new(),
            inference_substitutions: Substitutions::new(),
            next_inference_variable: 0,
        }
    }

    fn with_external_constructors(mut self, constructors: HashMap<String, ConstructorInfo>) -> Self {
        self.constructors.extend(constructors);
        self
    }

    fn with_external_values(mut self, values: HashMap<String, Type>) -> Self {
        self.external_values.extend(values);
        self
    }

    fn with_external_function_labels(mut self, labels: FunctionLabelMap) -> Self {
        self.function_labels.extend(labels);
        self
    }

    fn check(mut self) -> Result<TypedModule, Diagnostics> {
        self.collect_type_declarations();
        self.collect_imported_stdlib_interfaces();
        self.collect_annotated_function_types();
        self.collect_external_function_types();
        self.collect_constant_types();

        for declaration in self.module.ast.declarations.clone() {
            self.check_declaration(&declaration);
        }

        if self.diagnostics.is_empty() {
            self.finalize_inferred_types();
            self.check_ambiguous_function_types();
        }

        if self.diagnostics.is_empty() {
            Ok(TypedModule {
                resolved: self.module,
                functions: self.functions,
                expressions: self.expressions,
                function_labels: self.function_labels,
                interface: self.interface,
            })
        } else {
            Err(self.diagnostics)
        }
    }

    fn collect_type_declarations(&mut self) {
        for declaration in self.module.ast.declarations.clone() {
            match declaration {
                Declaration::TypeDefinition(raw) => {
                    if let Some(type_declaration) = type_definition_from_ast(&raw) {
                        for constructor in &type_declaration.constructors {
                            self.constructors.insert(constructor.name.clone(), constructor.clone());
                            self.interface
                                .constructors
                                .insert(constructor.name.clone(), constructor.clone());
                        }
                        self.interface
                            .types
                            .insert(type_declaration.name.clone(), type_declaration);
                    }
                }
                Declaration::TypeAlias(raw) => {
                    if let Some(alias) = type_alias_from_ast(&raw) {
                        self.interface.types.insert(alias.name.clone(), alias);
                    }
                }
                Declaration::ExternalType(type_) => {
                    self.interface.types.insert(
                        type_.name.text.clone(),
                        TypeDeclaration {
                            name: type_.name.text.clone(),
                            parameters: Vec::new(),
                            opaque: type_.opaque,
                            constructors: Vec::new(),
                            span: type_.span,
                        },
                    );
                }
                _ => {}
            }
        }
    }

    fn collect_imported_stdlib_interfaces(&mut self) {
        let registry = StdlibRegistry::new();
        for import in &self.module.ast.imports {
            let Some(interface) = registry.interface(&import.module.text) else {
                continue;
            };
            self.interface.types.extend(interface.types.clone());
            self.interface.constructors.extend(interface.constructors.clone());
            self.constructors.extend(interface.constructors.clone());

            let module_name = import
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
                });
            for (name, type_) in &interface.functions {
                self.external_values
                    .insert(format!("{module_name}.{name}"), type_.clone());
            }
            for imported in &import.unqualified {
                let local_name = imported.alias.as_ref().unwrap_or(&imported.name).text.clone();
                match imported.kind {
                    ast::UnqualifiedImportKind::Value => {
                        if let Some(type_) = interface.functions.get(&imported.name.text) {
                            self.external_values.insert(local_name, type_.clone());
                        }
                    }
                    ast::UnqualifiedImportKind::TypeOrConstructor => {
                        if let Some(constructor) = interface.constructors.get(&imported.name.text) {
                            self.constructors.insert(local_name, constructor.clone());
                        }
                    }
                }
            }
        }
    }

    fn collect_annotated_function_types(&mut self) {
        for function in self.module.ast.functions.clone() {
            if let Some(type_) = self.function_type_from_annotations(&function) {
                self.interface
                    .functions
                    .insert(function.name.text.clone(), type_.clone());
                if let Some(labels) = self.function_labels.get(&function.name.text).cloned() {
                    self.interface
                        .function_labels
                        .insert(function.name.text.clone(), labels);
                }
                self.function_types.insert(function.name.text, type_);
            }
        }
    }

    fn collect_external_function_types(&mut self) {
        for declaration in self.module.ast.declarations.clone() {
            match declaration {
                Declaration::ExternalFunction(function) => {
                    if let Some(type_) = self.external_function_type(&function) {
                        self.interface
                            .functions
                            .insert(function.name.text.clone(), type_.clone());
                        if let Some(labels) = self.function_labels.get(&function.name.text).cloned() {
                            self.interface
                                .function_labels
                                .insert(function.name.text.clone(), labels);
                        }
                        self.function_types.insert(function.name.text, type_);
                    }
                }
                Declaration::TargetGroup(group) => {
                    for declaration in group.declarations {
                        if let Declaration::ExternalFunction(function) = declaration
                            && let Some(type_) = self.external_function_type(&function)
                        {
                            self.interface
                                .functions
                                .insert(function.name.text.clone(), type_.clone());
                            if let Some(labels) = self.function_labels.get(&function.name.text).cloned() {
                                self.interface
                                    .function_labels
                                    .insert(function.name.text.clone(), labels);
                            }
                            self.function_types.insert(function.name.text, type_);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn collect_constant_types(&mut self) {
        for declaration in self.module.ast.declarations.clone() {
            if let Declaration::Constant(constant) = declaration
                && let Some(type_) = self.constant_type(&constant)
            {
                self.external_values.insert(constant.name.text.clone(), type_);
            }
        }
    }

    fn check_declaration(&mut self, declaration: &Declaration) {
        match declaration {
            Declaration::Function(function) => self.check_function(function),
            Declaration::Constant(constant) => {
                if let Some(expected) = constant
                    .type_annotation
                    .as_ref()
                    .and_then(|annotation| self.parse_type_annotation(annotation))
                    && let Some(actual) = self.check_expression(&constant.value)
                {
                    self.expect_same(&expected, &actual, constant.span);
                }
            }
            Declaration::TargetGroup(group) => {
                for declaration in &group.declarations {
                    self.check_declaration(declaration);
                }
            }
            _ => {}
        }
    }

    fn check_function(&mut self, function: &ast::Function) {
        let mut generator = ConstraintGenerator::new(self.inference_environment())
            .with_constructors(self.constructors.clone())
            .with_function_labels(self.function_labels.clone());
        let function_type = match generator.infer_function(function) {
            Ok(type_) => type_,
            Err(error) => {
                self.push_constraint_generation_error(error);
                return;
            }
        };
        let generation = generator.finish(function_type);
        let substitutions = match generation.constraints.solve() {
            Ok(substitutions) => substitutions,
            Err(error) => {
                self.push_unification_error(error);
                return;
            }
        };

        let Some(function_type) = type_term_to_type(&substitutions.walk(&generation.type_)) else {
            self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::TypeError, "could not infer function type")
                    .with_label(Label::primary(function.span, "inference failed")),
            );
            return;
        };
        let function_type = self.finalize_type(&function_type, &mut HashMap::new());

        let mut typed_expression_map = HashMap::new();
        for expression in generation.expressions {
            if let Some(type_) = type_term_to_type(&substitutions.walk(&expression.type_)) {
                let type_ = self.finalize_type(&type_, &mut HashMap::new());
                typed_expression_map.insert(expression.span, type_.clone());
                self.expressions.push(TypedExpression { span: expression.span, type_ });
            }
        }
        self.check_function_case_coverage(function, &typed_expression_map);

        self.function_types
            .insert(function.name.text.clone(), function_type.clone());
        self.interface
            .functions
            .insert(function.name.text.clone(), function_type.clone());
        self.functions
            .push(TypedFunction { name: function.name.clone(), type_: function_type });
    }

    fn check_function_case_coverage(&mut self, function: &ast::Function, expression_types: &HashMap<Span, Type>) {
        for statement in &function.body.statements {
            self.check_statement_case_coverage(statement, expression_types);
        }
    }

    fn check_statement_case_coverage(&mut self, statement: &Statement, expression_types: &HashMap<Span, Type>) {
        match statement {
            Statement::Let(let_) => self.check_expression_case_coverage(&let_.value, expression_types),
            Statement::LetAssert(let_) => self.check_expression_case_coverage(&let_.value, expression_types),
            Statement::Expression(expression) => self.check_expression_case_coverage(expression, expression_types),
        }
    }

    fn check_expression_case_coverage(&mut self, expression: &Expression, expression_types: &HashMap<Span, Type>) {
        match expression {
            Expression::Case(case) => {
                let subject_types = case
                    .subjects
                    .iter()
                    .filter_map(|subject| expression_types.get(&subject.span()).cloned())
                    .collect::<Vec<_>>();
                if subject_types.len() == case.subjects.len() {
                    self.check_case_coverage(case, &subject_types);
                }
                for clause in &case.clauses {
                    self.check_expression_case_coverage(&clause.value, expression_types);
                    if let Some(guard) = &clause.guard {
                        self.check_expression_case_coverage(guard, expression_types);
                    }
                }
            }
            Expression::Call(call) => {
                self.check_expression_case_coverage(&call.function, expression_types);
                for argument in &call.arguments {
                    self.check_expression_case_coverage(&argument.value, expression_types);
                }
            }
            Expression::Block(block) => {
                for statement in &block.statements {
                    self.check_statement_case_coverage(statement, expression_types);
                }
            }
            Expression::Tuple(tuple) => tuple
                .elements
                .iter()
                .for_each(|element| self.check_expression_case_coverage(element, expression_types)),
            Expression::List(list) => {
                for element in &list.elements {
                    self.check_expression_case_coverage(element, expression_types);
                }
                if let Some(spread) = &list.spread {
                    self.check_expression_case_coverage(spread, expression_types);
                }
            }
            _ => {}
        }
    }

    fn inference_environment(&self) -> Environment {
        let mut environment = Environment::new();
        for (name, type_) in &self.function_types {
            environment.insert(name.clone(), Scheme::from_type(type_));
        }
        for (name, type_) in &self.external_values {
            environment.insert(name.clone(), Scheme::from_type(type_));
        }
        environment
    }

    fn push_constraint_generation_error(&mut self, error: ConstraintGenerationError) {
        match error {
            ConstraintGenerationError::UnknownValue { name, span } => self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::TypeError, format!("no type known for `{name}`"))
                    .with_label(Label::primary(span, "unknown type")),
            ),
            ConstraintGenerationError::UnknownConstructor { name, span } => self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::TypeError, format!("unknown constructor `{name}`"))
                    .with_label(Label::primary(span, "unknown constructor here")),
            ),
            ConstraintGenerationError::UnsupportedAnnotation { source, span } => self.diagnostics.push(
                Diagnostic::new(
                    DiagnosticCode::TypeError,
                    format!("unsupported type annotation `{source}`"),
                )
                .with_label(Label::primary(span, "unsupported type annotation")),
            ),
            ConstraintGenerationError::TupleIndexOutOfBounds { span, .. } => self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::TypeError, "tuple index is out of bounds")
                    .with_label(Label::primary(span, "out of bounds")),
            ),
            ConstraintGenerationError::ArgumentLabel(error) => self.push_argument_label_error(error),
        }
    }

    fn push_argument_label_error(&mut self, error: ArgumentLabelError) {
        match error {
            ArgumentLabelError::UnknownLabel { label, span } => self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::TypeError, format!("unknown argument label `{label}`"))
                    .with_label(Label::primary(span, "unknown label")),
            ),
            ArgumentLabelError::DuplicateLabel { label, span } => self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::TypeError, format!("duplicate argument label `{label}`"))
                    .with_label(Label::primary(span, "duplicate label")),
            ),
            ArgumentLabelError::TooManyArguments { span } => self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::TypeError, "too many labelled call arguments")
                    .with_label(Label::primary(span, "extra argument")),
            ),
        }
    }

    fn push_unification_error(&mut self, error: UnificationError) {
        match error {
            UnificationError::Mismatch { expected, actual, span } => {
                let expected = type_term_to_type(&expected).unwrap_or(Type::Nil);
                let actual = type_term_to_type(&actual).unwrap_or(Type::Nil);
                self.diagnostics.push(
                    Diagnostic::new(
                        DiagnosticCode::TypeError,
                        format!("expected `{}` but found `{}`", expected.display(), actual.display()),
                    )
                    .with_label(Label::primary(span.unwrap_or(self.module.ast.span), "type mismatch")),
                );
            }
            UnificationError::ArityMismatch { expected, actual, span } => self.diagnostics.push(
                Diagnostic::new(
                    DiagnosticCode::TypeError,
                    format!("function expected {expected} arguments but got {actual}"),
                )
                .with_label(Label::primary(span.unwrap_or(self.module.ast.span), "arity mismatch")),
            ),
            UnificationError::FieldMismatch { field, span } => self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::TypeError, format!("unknown field `{field}`"))
                    .with_label(Label::primary(span.unwrap_or(self.module.ast.span), "unknown field")),
            ),
            UnificationError::OccursCheck { span, .. } => self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::TypeError, "recursive type inferred")
                    .with_label(Label::primary(span.unwrap_or(self.module.ast.span), "recursive type")),
            ),
        }
    }

    fn function_type_from_annotations(&mut self, function: &ast::Function) -> Option<Type> {
        let mut params = Vec::new();
        for parameter in &function.parameters {
            let Some(annotation) = &parameter.type_annotation else {
                return None;
            };
            params.push(self.parse_type_annotation(annotation)?);
        }
        let return_type = function
            .return_type
            .as_ref()
            .and_then(|annotation| self.parse_type_annotation(annotation))?;

        Some(Type::Function { params, return_type: Box::new(return_type) })
    }

    fn external_function_type(&mut self, function: &ast::ExternalFunction) -> Option<Type> {
        let params = function
            .parameters
            .iter()
            .map(|parameter| {
                parameter
                    .type_annotation
                    .as_ref()
                    .and_then(|annotation| self.parse_type_annotation(annotation))
            })
            .collect::<Option<Vec<_>>>()?;
        let return_type = self.parse_type_annotation(&function.return_type)?;
        Some(Type::Function { params, return_type: Box::new(return_type) })
    }

    fn constant_type(&mut self, constant: &ast::Constant) -> Option<Type> {
        constant
            .type_annotation
            .as_ref()
            .and_then(|annotation| self.parse_type_annotation(annotation))
            .or_else(|| self.check_expression(&constant.value))
    }

    fn check_block(&mut self, block: &ast::Block) -> Option<Type> {
        self.check_statements(&block.statements)
    }

    fn check_statements(&mut self, statements: &[Statement]) -> Option<Type> {
        let mut last_type = Type::Nil;
        for (index, statement) in statements.iter().enumerate() {
            last_type = match statement {
                Statement::Let(let_) => {
                    let value_type = self.check_expression(&let_.value)?;
                    let pattern_type = if let Some(annotation) = &let_.type_annotation
                        && let Some(expected) = self.parse_type_annotation(annotation)
                    {
                        self.expect_same(&expected, &value_type, let_.span);
                        expected
                    } else if eligible_for_local_generalization(&let_.value) {
                        self.finalize_type(&value_type, &mut HashMap::new())
                    } else {
                        value_type
                    };
                    if let Pattern::Name(name) = &let_.pattern
                        && let_.type_annotation.is_none()
                        && eligible_for_local_generalization(&let_.value)
                    {
                        self.define_generalized(name.text.clone(), pattern_type);
                    } else {
                        self.bind_pattern(&let_.pattern, &pattern_type);
                    }
                    Type::Nil
                }
                Statement::LetAssert(let_assert) => {
                    let value_type = self.check_expression(&let_assert.value)?;
                    if let Some(annotation) = &let_assert.type_annotation
                        && let Some(expected) = self.parse_type_annotation(annotation)
                    {
                        self.expect_same(&expected, &value_type, let_assert.span);
                    }
                    if let Some(message) = &let_assert.message {
                        self.check_expression(message);
                    }
                    self.bind_pattern(&let_assert.pattern, &value_type);
                    Type::Nil
                }
                Statement::Expression(Expression::Use(use_)) => {
                    return self.check_use(use_, &statements[index + 1..], true);
                }
                Statement::Expression(expression) => self.check_expression(expression)?,
            };
        }
        Some(last_type)
    }

    fn check_expression(&mut self, expression: &Expression) -> Option<Type> {
        let type_ = match expression {
            Expression::Literal(literal) => Type::from(&literal.kind),
            Expression::Variable(name) => self.lookup_name(name)?,
            Expression::Call(call) => self.check_call(call)?,
            Expression::FieldAccess(field_access) => self.check_field_access(field_access)?,
            Expression::Block(block) => {
                self.push_scope();
                let type_ = self.check_block(block)?;
                self.pop_scope();
                type_
            }
            Expression::Tuple(tuple) => Type::Tuple(
                tuple
                    .elements
                    .iter()
                    .map(|element| self.check_expression(element))
                    .collect::<Option<Vec<_>>>()?,
            ),
            Expression::List(list) => self.check_list(list)?,
            Expression::Record(record) => self.check_record(record)?,
            Expression::BitArray(bit_array) => {
                for segment in &bit_array.segments {
                    self.check_expression(&segment.value)?;
                    for option in &segment.options {
                        if let Some(value) = &option.value {
                            self.check_expression(value)?;
                        }
                    }
                }
                Type::BitArray
            }
            Expression::Panic(failure) | Expression::Todo(failure) => {
                if let Some(message) = &failure.message {
                    self.check_expression(message)?;
                }
                Type::Nil
            }
            Expression::Assert(assert) => {
                let value_type = self.check_expression(&assert.value)?;
                if let Some(annotation) = &assert.type_annotation
                    && let Some(expected) = self.parse_type_annotation(annotation)
                {
                    self.expect_same(&expected, &value_type, assert.span);
                }
                self.bind_pattern(&assert.pattern, &value_type);
                Type::Nil
            }
            Expression::BinaryOperation(operation) => self.check_binary_operation(operation)?,
            Expression::Pipeline(pipeline) => self.check_pipeline(pipeline)?,
            Expression::UnaryOperation(operation) => self.check_unary_operation(operation)?,
            Expression::Use(use_) => self.check_use(use_, &[], false)?,
            Expression::AnonymousFunction(function) => self.check_anonymous_function(function)?,
            Expression::Capture(capture) => self.check_capture(capture)?,
            Expression::RecordUpdate(update) => self.check_record_update(update)?,
            Expression::TupleAccess(access) => self.check_tuple_access(access)?,
            Expression::Echo(echo) => self.check_expression(&echo.value)?,
            Expression::Raw(raw) => self.check_raw_expression(raw)?,
            Expression::Case(case) => self.check_case(case)?,
        };

        self.expressions
            .push(TypedExpression { span: expression.span(), type_: type_.clone() });
        Some(type_)
    }

    fn check_case(&mut self, case: &ast::Case) -> Option<Type> {
        let subject_types = case
            .subjects
            .iter()
            .map(|subject| self.check_expression(subject))
            .collect::<Option<Vec<_>>>()?;
        let mut result_type = None;

        for clause in &case.clauses {
            if clause.patterns.len() != subject_types.len() {
                self.diagnostics.push(
                    Diagnostic::new(
                        DiagnosticCode::TypeError,
                        "case pattern count does not match subject count",
                    )
                    .with_label(Label::primary(clause.span, "wrong number of patterns")),
                );
                continue;
            }

            self.push_scope();
            for (pattern, subject_type) in clause.patterns.iter().zip(subject_types.iter()) {
                self.bind_pattern(pattern, subject_type);
            }
            if let Some(guard) = &clause.guard {
                let guard_type = self
                    .check_guard_expression(guard)
                    .or_else(|| self.check_expression(guard));
                if let Some(guard_type) = guard_type {
                    self.expect_same(&Type::Bool, &guard_type, guard.span());
                }
            }
            let clause_type = self.check_expression(&clause.value)?;
            self.pop_scope();

            match &result_type {
                Some(expected) => self.expect_same(expected, &clause_type, clause.value.span()),
                None => result_type = Some(clause_type),
            }
        }

        self.check_case_coverage(case, &subject_types);
        Some(result_type.unwrap_or(Type::Nil))
    }

    fn check_raw_expression(&mut self, raw: &ast::RawSyntax) -> Option<Type> {
        match raw.kind.as_str() {
            "bit_string" => Some(Type::BitArray),
            "tuple" => Some(Type::Tuple(
                raw.source
                    .trim()
                    .trim_start_matches("#(")
                    .trim_end_matches(')')
                    .split(',')
                    .filter(|item| !item.trim().is_empty())
                    .map(|_| Type::Int)
                    .collect(),
            )),
            "list" => Some(Type::List(Box::new(Type::Int))),
            "record" => {
                let name = raw.source.split(['(', ' ']).next()?;
                let Some(constructor) = self.constructors.get(name).cloned() else {
                    self.diagnostics.push(
                        Diagnostic::new(DiagnosticCode::TypeError, format!("unknown constructor `{name}`"))
                            .with_label(Label::primary(raw.span, "unknown constructor here")),
                    );
                    return None;
                };
                Some(constructor.return_type)
            }
            _ => {
                self.diagnostics.push(
                    Diagnostic::new(
                        DiagnosticCode::TypeError,
                        format!("unsupported expression `{}`", raw.kind),
                    )
                    .with_label(Label::primary(raw.span, "unsupported expression here")),
                );
                None
            }
        }
    }

    fn check_call(&mut self, call: &ast::Call) -> Option<Type> {
        let function_type = self.check_expression(&call.function)?;
        let Type::Function { params, return_type } = function_type else {
            self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::TypeError, "called value is not a function")
                    .with_label(Label::primary(call.span, "not a function")),
            );
            return None;
        };

        if params.len() != call.arguments.len() {
            self.diagnostics.push(
                Diagnostic::new(
                    DiagnosticCode::TypeError,
                    format!(
                        "function expected {} arguments but got {}",
                        params.len(),
                        call.arguments.len()
                    ),
                )
                .with_label(Label::primary(call.span, "wrong number of arguments")),
            );
            return None;
        }

        let order = match call_argument_order(self.call_function_labels(call), &call.arguments, params.len()) {
            Ok(order) => order,
            Err(error) => {
                self.push_argument_label_error(error);
                return None;
            }
        };
        for (argument, index) in call.arguments.iter().zip(order.indices) {
            if let Some(expected) = params.get(index)
                && let Some(actual) = self.check_expression(&argument.value)
            {
                self.expect_same(expected, &actual, argument.span);
            }
        }

        Some(self.resolve_inference_type(&return_type))
    }

    fn check_field_access(&mut self, field_access: &ast::FieldAccess) -> Option<Type> {
        if let Expression::Variable(module) = field_access.record.as_ref()
            && let Some(type_) = self
                .external_values
                .get(&format!("{}.{}", module.text, field_access.field.text))
                .or_else(|| self.external_values.get(&field_access.field.text))
        {
            let type_ = type_.clone();
            return Some(self.instantiate_named_generics(&type_));
        }

        let record_type = self.check_expression(&field_access.record)?;
        match record_type {
            Type::Record { fields, .. } => fields
                .iter()
                .find(|field| field.name == field_access.field.text)
                .map(|field| field.type_.clone())
                .or_else(|| {
                    self.diagnostics.push(
                        Diagnostic::new(
                            DiagnosticCode::TypeError,
                            format!("unknown field `{}`", field_access.field.text),
                        )
                        .with_label(Label::primary(field_access.field.span, "unknown field")),
                    );
                    None
                }),
            _ => {
                self.diagnostics.push(
                    Diagnostic::new(DiagnosticCode::TypeError, "field access used with non-record value")
                        .with_label(Label::primary(field_access.span, "not a record")),
                );
                None
            }
        }
    }

    fn check_list(&mut self, list: &ast::List) -> Option<Type> {
        let mut element_type = None;
        for element in &list.elements {
            let actual = self.check_expression(element)?;
            match &element_type {
                Some(expected) => self.expect_same(expected, &actual, element.span()),
                None => element_type = Some(actual),
            }
        }
        if let Some(spread) = &list.spread {
            let spread_type = self.check_expression(spread)?;
            match spread_type {
                Type::List(item) => match &element_type {
                    Some(expected) => self.expect_same(expected, &item, spread.span()),
                    None => element_type = Some(*item),
                },
                _ => self.diagnostics.push(
                    Diagnostic::new(DiagnosticCode::TypeError, "list spread must be a list")
                        .with_label(Label::primary(spread.span(), "not a list")),
                ),
            }
        }
        Some(Type::List(Box::new(
            element_type.unwrap_or_else(|| self.fresh_inference_type()),
        )))
    }

    fn check_record(&mut self, record: &ast::Record) -> Option<Type> {
        let name = constructor_name_text(&record.constructor);
        let Some(constructor) = self.constructors.get(&name).cloned() else {
            self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::TypeError, format!("unknown constructor `{name}`"))
                    .with_label(Label::primary(record.span, "unknown constructor here")),
            );
            return None;
        };
        let constructor = self.instantiate_constructor(&constructor);
        self.check_record_arguments(&record.arguments, &constructor.fields);
        Some(constructor.return_type)
    }

    fn check_record_update(&mut self, update: &ast::RecordUpdate) -> Option<Type> {
        let spread_type = self.check_expression(&update.spread)?;
        let name = constructor_name_text(&update.constructor);
        let Some(constructor) = self.constructors.get(&name).cloned() else {
            self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::TypeError, format!("unknown constructor `{name}`"))
                    .with_label(Label::primary(update.span, "unknown constructor here")),
            );
            return None;
        };
        let constructor = self.instantiate_constructor(&constructor);
        self.expect_same(&constructor.return_type, &spread_type, update.span);
        let fields = constructor.fields.clone();
        self.check_record_arguments(&update.updates, &fields);
        Some(spread_type)
    }

    fn check_record_arguments(&mut self, arguments: &[ast::Argument], fields: &[FieldInfo]) {
        for (index, argument) in arguments.iter().enumerate() {
            let field = match &argument.label {
                Some(label) => fields.iter().find(|field| field.name == label.text),
                None => fields.get(index),
            };
            let Some(field) = field else {
                self.diagnostics.push(
                    Diagnostic::new(DiagnosticCode::TypeError, "unknown constructor field")
                        .with_label(Label::primary(argument.span, "unknown field here")),
                );
                continue;
            };
            if let Some(actual) = self.check_expression(&argument.value) {
                self.expect_same(&field.type_, &actual, argument.span);
            }
        }
    }

    fn check_binary_operation(&mut self, operation: &ast::BinaryOperation) -> Option<Type> {
        let left = self.check_expression(&operation.left)?;
        let right = self.check_expression(&operation.right)?;
        match operation.operator {
            ast::BinaryOperator::Add
            | ast::BinaryOperator::Subtract
            | ast::BinaryOperator::Multiply
            | ast::BinaryOperator::Divide
            | ast::BinaryOperator::Remainder => {
                self.expect_same(&Type::Int, &left, operation.left.span());
                self.expect_same(&Type::Int, &right, operation.right.span());
                Some(Type::Int)
            }
            ast::BinaryOperator::FloatAdd
            | ast::BinaryOperator::FloatSubtract
            | ast::BinaryOperator::FloatMultiply
            | ast::BinaryOperator::FloatDivide => {
                self.expect_same(&Type::Float, &left, operation.left.span());
                self.expect_same(&Type::Float, &right, operation.right.span());
                Some(Type::Float)
            }
            ast::BinaryOperator::And | ast::BinaryOperator::Or => {
                self.expect_same(&Type::Bool, &left, operation.left.span());
                self.expect_same(&Type::Bool, &right, operation.right.span());
                Some(Type::Bool)
            }
            ast::BinaryOperator::StringConcat => {
                self.expect_same(&Type::String, &left, operation.left.span());
                self.expect_same(&Type::String, &right, operation.right.span());
                Some(Type::String)
            }
            ast::BinaryOperator::Equal | ast::BinaryOperator::NotEqual => {
                self.expect_same(&left, &right, operation.right.span());
                Some(Type::Bool)
            }
            ast::BinaryOperator::LessThan
            | ast::BinaryOperator::LessThanEqual
            | ast::BinaryOperator::GreaterThan
            | ast::BinaryOperator::GreaterThanEqual => {
                self.expect_same(&Type::Int, &left, operation.left.span());
                self.expect_same(&Type::Int, &right, operation.right.span());
                Some(Type::Bool)
            }
            ast::BinaryOperator::FloatLessThan
            | ast::BinaryOperator::FloatLessThanEqual
            | ast::BinaryOperator::FloatGreaterThan
            | ast::BinaryOperator::FloatGreaterThanEqual => {
                self.expect_same(&Type::Float, &left, operation.left.span());
                self.expect_same(&Type::Float, &right, operation.right.span());
                Some(Type::Bool)
            }
        }
    }

    fn check_pipeline(&mut self, pipeline: &ast::Pipeline) -> Option<Type> {
        let input = self.check_expression(&pipeline.value)?;
        let into = self.check_expression(&pipeline.into)?;
        match into {
            Type::Function { params, return_type } => {
                if let Some(first) = params.first() {
                    self.expect_same(first, &input, pipeline.value.span());
                }
                Some(*return_type)
            }
            other => Some(other),
        }
    }

    fn check_unary_operation(&mut self, operation: &ast::UnaryOperation) -> Option<Type> {
        let value = self.check_expression(&operation.value)?;
        match operation.operator {
            ast::UnaryOperator::BooleanNot => {
                self.expect_same(&Type::Bool, &value, operation.value.span());
                Some(Type::Bool)
            }
            ast::UnaryOperator::IntegerNegate => {
                self.expect_same(&Type::Int, &value, operation.value.span());
                Some(Type::Int)
            }
        }
    }

    fn call_function_labels(&self, call: &ast::Call) -> Option<&[Option<String>]> {
        match call.function.as_ref() {
            Expression::Variable(name) => self.function_labels(&name.text),
            Expression::FieldAccess(access) => match access.record.as_ref() {
                Expression::Variable(module) => self.function_labels(&format!("{}.{}", module.text, access.field.text)),
                _ => None,
            },
            _ => None,
        }
    }

    fn function_labels(&self, name: &str) -> Option<&[Option<String>]> {
        self.function_labels.get(name).map(Vec::as_slice)
    }

    fn check_use(
        &mut self, use_: &ast::Use, continuation: &[Statement], allow_empty_continuation: bool,
    ) -> Option<Type> {
        if continuation.is_empty() && !allow_empty_continuation {
            self.diagnostics.push(empty_use_continuation_diagnostic(use_.span));
            return None;
        }

        let function_type = match use_.value.as_ref() {
            Expression::Call(call) => self.check_expression(&call.function)?,
            value => self.check_expression(value)?,
        };
        let Type::Function { params, return_type } = function_type else {
            self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::TypeError, "use value is not a function")
                    .with_label(Label::primary(use_.span, "not a function")),
            );
            return None;
        };

        let callback_index = match use_.value.as_ref() {
            Expression::Call(call) => {
                let placement =
                    match use_callback_placement(self.call_function_labels(call), &call.arguments, params.len()) {
                        Ok(placement) => placement,
                        Err(error) => {
                            self.push_argument_label_error(error);
                            return None;
                        }
                    };
                for (argument, index) in call.arguments.iter().zip(placement.argument_indices.iter().copied()) {
                    if let Some(expected) = params.get(index)
                        && let Some(actual) = self.check_expression(&argument.value)
                    {
                        self.expect_same(expected, &actual, argument.span);
                    }
                }
                placement.callback_index
            }
            _ => 0,
        };
        let supplied_count = match use_.value.as_ref() {
            Expression::Call(call) => call.arguments.len(),
            _ => 0,
        };
        if params.len() != supplied_count + 1 {
            self.diagnostics.push(
                Diagnostic::new(
                    DiagnosticCode::TypeError,
                    format!(
                        "use function expected {} arguments but got {}",
                        params.len(),
                        supplied_count + 1
                    ),
                )
                .with_label(Label::primary(use_.span, "wrong number of arguments")),
            );
            return None;
        }
        let Some(Type::Function { params: callback_params, return_type: callback_return }) = params.get(callback_index)
        else {
            self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::TypeError, "use expected a callback argument")
                    .with_label(Label::primary(use_.span, "missing callback parameter")),
            );
            return None;
        };
        if callback_params.len() != use_.assignments.len() {
            self.diagnostics.push(
                Diagnostic::new(
                    DiagnosticCode::TypeError,
                    format!(
                        "callback expected {} arguments but got {}",
                        callback_params.len(),
                        use_.assignments.len()
                    ),
                )
                .with_label(Label::primary(use_.span, "wrong number of callback arguments")),
            );
            return None;
        }

        self.push_scope();
        for (assignment, expected) in use_.assignments.iter().zip(callback_params.iter()) {
            if let Some(annotation) = &assignment.type_annotation
                && let Some(type_) = self.parse_type_annotation(annotation)
            {
                self.expect_same(expected, &type_, assignment.span);
            }
            self.bind_pattern(&assignment.pattern, expected);
        }
        let continuation_type = self.check_statements(continuation)?;
        self.pop_scope();
        self.expect_same(callback_return, &continuation_type, use_.span);
        Some(*return_type)
    }

    fn check_anonymous_function(&mut self, function: &ast::AnonymousFunction) -> Option<Type> {
        self.push_scope();
        let mut params = Vec::new();
        for parameter in &function.parameters {
            let type_ = match &parameter.type_annotation {
                Some(annotation) => self.parse_type_annotation(annotation)?,
                None => self.fresh_inference_type(),
            };
            if let Some(name) = &parameter.name {
                self.define(name.text.clone(), type_.clone());
            }
            params.push(type_);
        }
        let body_type = self.check_block(&function.body)?;
        self.pop_scope();
        let return_type = function
            .return_type
            .as_ref()
            .and_then(|annotation| self.parse_type_annotation(annotation))
            .unwrap_or(body_type);
        Some(Type::Function { params, return_type: Box::new(return_type) })
    }

    fn check_capture(&mut self, capture: &ast::Capture) -> Option<Type> {
        let function_type = self.check_expression(&capture.function)?;
        let Type::Function { params, return_type } = function_type else {
            self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::TypeError, "captured value is not a function")
                    .with_label(Label::primary(capture.span, "not a function")),
            );
            return None;
        };
        let mut remaining = Vec::new();
        for (index, parameter) in params.iter().enumerate() {
            match capture.arguments.get(index).and_then(Option::as_ref) {
                Some(argument) => {
                    if let Some(actual) = self.check_expression(&argument.value) {
                        self.expect_same(parameter, &actual, argument.span);
                    }
                }
                None => remaining.push(parameter.clone()),
            }
        }
        Some(Type::Function { params: remaining, return_type })
    }

    fn check_tuple_access(&mut self, access: &ast::TupleAccess) -> Option<Type> {
        let tuple = self.check_expression(&access.tuple)?;
        let Type::Tuple(elements) = tuple else {
            self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::TypeError, "tuple access used with non-tuple value")
                    .with_label(Label::primary(access.span, "not a tuple")),
            );
            return None;
        };
        let index = access.index.text.parse::<usize>().ok()?;
        elements.get(index).cloned().or_else(|| {
            self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::TypeError, "tuple index is out of bounds")
                    .with_label(Label::primary(access.index.span, "out of bounds")),
            );
            None
        })
    }

    fn bind_pattern(&mut self, pattern: &Pattern, type_: &Type) {
        match pattern {
            Pattern::Name(name) => self.define(name.text.clone(), type_.clone()),
            Pattern::Discard(_) => {}
            Pattern::Integer(literal) => self.expect_same(&Type::Int, type_, literal.span),
            Pattern::Float(literal) => self.expect_same(&Type::Float, type_, literal.span),
            Pattern::String(literal) => self.expect_same(&Type::String, type_, literal.span),
            Pattern::Bool(literal) => self.expect_same(&Type::Bool, type_, literal.span),
            Pattern::Nil(literal) => self.expect_same(&Type::Nil, type_, literal.span),
            Pattern::Tuple(tuple) => self.bind_tuple_pattern(tuple, type_),
            Pattern::List(list) => self.bind_list_pattern(list, type_),
            Pattern::Constructor(constructor) => self.bind_constructor_pattern(constructor, type_),
            Pattern::Alias(alias) => {
                self.bind_pattern(&alias.pattern, type_);
                self.define(alias.alias.text.clone(), type_.clone());
            }
            Pattern::BitString(raw) => {
                self.expect_same(&Type::BitArray, type_, raw.span);
                for name in bit_string_pattern_bindings(raw) {
                    self.define(name.text, Type::Int);
                }
            }
            Pattern::Raw(raw) => self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::TypeError, format!("unsupported pattern `{}`", raw.kind))
                    .with_label(Label::primary(raw.span, "unsupported pattern here")),
            ),
        }
    }

    fn bind_tuple_pattern(&mut self, tuple: &ast::TuplePattern, type_: &Type) {
        match type_ {
            Type::Tuple(elements) => {
                if tuple.elements.len() != elements.len() {
                    self.diagnostics.push(
                        Diagnostic::new(DiagnosticCode::TypeError, "tuple pattern has the wrong arity")
                            .with_label(Label::primary(tuple.span, "wrong number of elements")),
                    );
                }
                for (pattern, element_type) in tuple.elements.iter().zip(elements.iter()) {
                    self.bind_pattern(pattern, element_type);
                }
            }
            _ => self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::TypeError, "tuple pattern used with non-tuple value")
                    .with_label(Label::primary(tuple.span, "tuple pattern here")),
            ),
        }
    }

    fn bind_list_pattern(&mut self, list: &ast::ListPattern, type_: &Type) {
        let Type::List(element_type) = type_ else {
            self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::TypeError, "list pattern used with non-list value")
                    .with_label(Label::primary(list.span, "list pattern here")),
            );
            return;
        };
        for pattern in &list.elements {
            self.bind_pattern(pattern, element_type);
        }
        if let Some(ast::ListPatternTail::Name(name)) = &list.tail {
            self.define(name.text.clone(), Type::List(element_type.clone()));
        }
    }

    fn bind_constructor_pattern(&mut self, pattern: &ast::ConstructorPattern, type_: &Type) {
        let name = constructor_pattern_name(pattern);
        let Some(constructor) = self.constructors.get(name).cloned() else {
            self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::TypeError, format!("unknown constructor `{name}`"))
                    .with_label(Label::primary(pattern.span, "unknown constructor here")),
            );
            return;
        };

        let constructor = self.instantiate_constructor(&constructor);
        self.expect_same(&constructor.return_type, type_, pattern.span);
        let fields = constructor.fields.clone();
        if pattern.arguments.len() > fields.len() {
            self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::TypeError, "constructor pattern has too many arguments")
                    .with_label(Label::primary(pattern.span, "too many arguments")),
            );
        }

        for (index, argument) in pattern.arguments.iter().enumerate() {
            let field = match &argument.label {
                Some(label) => fields.iter().find(|field| field.name == label.text),
                None => fields.get(index),
            };
            let Some(field) = field else {
                self.diagnostics.push(
                    Diagnostic::new(DiagnosticCode::TypeError, "unknown constructor field")
                        .with_label(Label::primary(argument.span, "unknown field here")),
                );
                continue;
            };
            match &argument.pattern {
                Some(nested) => self.bind_pattern(nested, &field.type_),
                None => {
                    if let Some(label) = &argument.label {
                        self.define(label.text.clone(), field.type_.clone());
                    }
                }
            }
        }
    }

    fn check_guard_expression(&mut self, expr: &Expression) -> Option<Type> {
        match expr {
            Expression::Variable(name) => self.lookup_name(name),
            Expression::Literal(literal) if literal.kind == LiteralKind::Bool => Some(Type::Bool),
            Expression::Raw(raw) if raw.kind == "binary_expression" => Some(Type::Bool),
            _ => None,
        }
    }

    fn check_case_coverage(&mut self, case: &ast::Case, subject_types: &[Type]) {
        for (subject_index, subject_type) in subject_types.iter().enumerate() {
            let mut covered = false;
            for clause in &case.clauses {
                let Some(pattern) = clause.patterns.get(subject_index) else { continue };
                if covered {
                    self.diagnostics.push(
                        Diagnostic::new(DiagnosticCode::TypeError, "case branch is unreachable")
                            .with_label(Label::primary(clause.span, "unreachable branch")),
                    );
                    continue;
                }
                if clause.guard.is_none() && self.pattern_covers_type(pattern, subject_type) {
                    covered = true;
                }
            }
            if covered {
                continue;
            }

            match subject_type {
                Type::Bool => self.check_bool_exhaustiveness(case, subject_index),
                Type::List(_) => self.check_list_exhaustiveness(case, subject_index),
                Type::Tuple(_) => self.check_tuple_exhaustiveness(case, subject_index),
                Type::Custom { .. } => self.check_custom_exhaustiveness(case, subject_index, subject_type),
                _ => {}
            }
        }
    }

    fn check_bool_exhaustiveness(&mut self, case: &ast::Case, subject_index: usize) {
        let mut seen_true = false;
        let mut seen_false = false;
        for clause in case.clauses.iter().filter(|clause| clause.guard.is_none()) {
            match clause.patterns.get(subject_index) {
                Some(Pattern::Bool(literal)) if literal.source == "True" => seen_true = true,
                Some(Pattern::Bool(literal)) if literal.source == "False" => seen_false = true,
                _ => {}
            }
        }
        if !seen_true || !seen_false {
            let missing = match (seen_true, seen_false) {
                (false, false) => "True and False",
                (false, true) => "True",
                (true, false) => "False",
                (true, true) => return,
            };
            self.non_exhaustive(
                case.span,
                format!("case expression is not exhaustive; missing {missing}"),
            );
        }
    }

    fn check_list_exhaustiveness(&mut self, case: &ast::Case, subject_index: usize) {
        let mut empty = false;
        let mut non_empty = false;
        for clause in case.clauses.iter().filter(|clause| clause.guard.is_none()) {
            if let Some(Pattern::List(list)) = clause.patterns.get(subject_index) {
                empty |= list.elements.is_empty() && list.tail.is_none();
                non_empty |= !list.elements.is_empty() && list.tail.is_some();
            }
        }
        if !empty || !non_empty {
            self.non_exhaustive(case.span, "case expression is not exhaustive for list values");
        }
    }

    fn check_tuple_exhaustiveness(&mut self, case: &ast::Case, subject_index: usize) {
        if !case
            .clauses
            .iter()
            .filter(|clause| clause.guard.is_none())
            .any(|clause| {
                clause
                    .patterns
                    .get(subject_index)
                    .is_some_and(|pattern| self.pattern_covers_type(pattern, &Type::Tuple(Vec::new())))
            })
        {
            self.non_exhaustive(case.span, "case expression is not exhaustive for tuple values");
        }
    }

    fn check_custom_exhaustiveness(&mut self, case: &ast::Case, subject_index: usize, subject_type: &Type) {
        let constructors = self
            .constructors
            .values()
            .filter(|constructor| constructor.return_type.substitute_from(subject_type) == *subject_type)
            .map(|constructor| constructor.name.clone())
            .collect::<std::collections::HashSet<_>>();
        if constructors.is_empty() {
            return;
        }
        let seen = case
            .clauses
            .iter()
            .filter(|clause| clause.guard.is_none())
            .filter_map(|clause| match clause.patterns.get(subject_index) {
                Some(Pattern::Constructor(pattern)) => Some(constructor_pattern_name(pattern).to_string()),
                _ => None,
            })
            .collect::<std::collections::HashSet<_>>();
        let missing = constructors.difference(&seen).cloned().collect::<Vec<_>>();
        if !missing.is_empty() {
            self.non_exhaustive(
                case.span,
                format!("case expression is not exhaustive; missing {}", missing.join(", ")),
            );
        }
    }

    fn pattern_covers_type(&self, pattern: &Pattern, type_: &Type) -> bool {
        match pattern {
            Pattern::Name(_) | Pattern::Discard(_) => true,
            Pattern::Alias(alias) => self.pattern_covers_type(&alias.pattern, type_),
            Pattern::Tuple(tuple) => match type_ {
                Type::Tuple(elements) => tuple
                    .elements
                    .iter()
                    .zip(elements.iter())
                    .all(|(pattern, type_)| self.pattern_covers_type(pattern, type_)),
                _ => tuple
                    .elements
                    .iter()
                    .all(|pattern| matches!(pattern, Pattern::Name(_) | Pattern::Discard(_))),
            },
            _ => false,
        }
    }

    fn non_exhaustive(&mut self, span: Span, message: impl Into<String>) {
        self.diagnostics.push(
            Diagnostic::new(DiagnosticCode::TypeError, message).with_label(Label::primary(span, "non-exhaustive case")),
        );
    }

    fn lookup_name(&mut self, name: &ast::Name) -> Option<Type> {
        for scope in self.scopes.iter().rev() {
            if let Some(scoped) = scope.get(&name.text).cloned() {
                if scoped.generalized {
                    return Some(self.instantiate_named_generics(&scoped.type_));
                }
                return Some(scoped.type_);
            }
        }

        if let Some(type_) = self.function_types.get(&name.text).cloned() {
            return Some(self.instantiate_named_generics(&type_));
        }

        if let Some(type_) = self.external_values.get(&name.text).cloned() {
            return Some(self.instantiate_named_generics(&type_));
        }

        self.diagnostics.push(
            Diagnostic::new(DiagnosticCode::TypeError, format!("no type known for `{}`", name.text))
                .with_label(Label::primary(name.span, "unknown type")),
        );
        None
    }

    fn parse_type_annotation(&mut self, annotation: &ast::TypeAnnotation) -> Option<Type> {
        match parse_type_source(&annotation.source) {
            Some(type_) => {
                self.validate_type_annotation_arity(&type_, annotation.span);
                Some(type_)
            }
            None => {
                self.diagnostics.push(
                    Diagnostic::new(
                        DiagnosticCode::TypeError,
                        format!("unsupported type annotation `{}`", annotation.source),
                    )
                    .with_label(Label::primary(annotation.span, "unsupported type annotation")),
                );
                None
            }
        }
    }

    fn validate_type_annotation_arity(&mut self, type_: &Type, span: Span) {
        match type_ {
            Type::Tuple(items) => {
                for item in items {
                    self.validate_type_annotation_arity(item, span);
                }
            }
            Type::List(item) => self.validate_type_annotation_arity(item, span),
            Type::Custom { name, args } | Type::Opaque { name, args } => {
                if let Some(declaration) = self.interface.types.get(name)
                    && declaration.parameters.len() != args.len()
                {
                    self.diagnostics.push(
                        Diagnostic::new(
                            DiagnosticCode::TypeError,
                            format!(
                                "type `{}` expected {} type arguments but got {}",
                                name,
                                declaration.parameters.len(),
                                args.len()
                            ),
                        )
                        .with_label(Label::primary(span, "wrong number of type arguments")),
                    );
                }
                for arg in args {
                    self.validate_type_annotation_arity(arg, span);
                }
            }
            Type::Function { params, return_type } => {
                for param in params {
                    self.validate_type_annotation_arity(param, span);
                }
                self.validate_type_annotation_arity(return_type, span);
            }
            _ => {}
        }
    }

    fn expect_same(&mut self, expected: &Type, actual: &Type, span: Span) {
        let expected = self.resolve_inference_type(expected);
        let actual = self.resolve_inference_type(actual);
        if self.unify_types(&expected, &actual, span).is_err() {
            self.diagnostics.push(
                Diagnostic::new(
                    DiagnosticCode::TypeError,
                    format!("expected `{}` but found `{}`", expected.display(), actual.display()),
                )
                .with_label(Label::primary(span, "type mismatch")),
            );
        }
    }

    fn fresh_inference_type(&mut self) -> Type {
        let id = self.next_inference_variable;
        self.next_inference_variable += 1;
        Type::Generic(format!("${id}"))
    }

    fn is_inference_variable_name(name: &str) -> bool {
        name.starts_with('$')
    }

    fn resolve_inference_type(&self, type_: &Type) -> Type {
        match type_ {
            Type::Generic(name) if Self::is_inference_variable_name(name) => {
                let Some(variable) = inference_variable_from_name(name) else {
                    return type_.clone();
                };
                let walked = self.inference_substitutions.walk(&TypeTerm::Variable(variable));
                if walked == TypeTerm::Variable(variable) {
                    type_.clone()
                } else {
                    type_term_to_type(&walked)
                        .map(|type_| self.resolve_inference_type(&type_))
                        .unwrap_or_else(|| type_.clone())
                }
            }
            Type::Tuple(items) => Type::Tuple(items.iter().map(|item| self.resolve_inference_type(item)).collect()),
            Type::List(item) => Type::List(Box::new(self.resolve_inference_type(item))),
            Type::Record { name, fields } => Type::Record {
                name: name.clone(),
                fields: fields
                    .iter()
                    .map(|field| FieldInfo {
                        name: field.name.clone(),
                        type_: self.resolve_inference_type(&field.type_),
                    })
                    .collect(),
            },
            Type::Custom { name, args } => Type::Custom {
                name: name.clone(),
                args: args.iter().map(|arg| self.resolve_inference_type(arg)).collect(),
            },
            Type::Opaque { name, args } => Type::Opaque {
                name: name.clone(),
                args: args.iter().map(|arg| self.resolve_inference_type(arg)).collect(),
            },
            Type::Function { params, return_type } => Type::Function {
                params: params.iter().map(|param| self.resolve_inference_type(param)).collect(),
                return_type: Box::new(self.resolve_inference_type(return_type)),
            },
            _ => type_.clone(),
        }
    }

    fn unify_types(&mut self, expected: &Type, actual: &Type, span: Span) -> Result<(), ()> {
        let mut unifier = Unifier::with_substitutions(self.inference_substitutions.clone());
        let expected = self.type_to_inference_term(expected);
        let actual = self.type_to_inference_term(actual);
        match unifier.unify(&expected, &actual, Some(span)) {
            Ok(_) => {
                self.inference_substitutions = unifier.into_substitutions();
                Ok(())
            }
            Err(crate::inference::UnificationError::OccursCheck { .. }) => {
                self.diagnostics.push(
                    Diagnostic::new(DiagnosticCode::TypeError, "recursive type inferred")
                        .with_label(Label::primary(span, "recursive type")),
                );
                Err(())
            }
            Err(_) => Err(()),
        }
    }

    fn type_to_inference_term(&self, type_: &Type) -> TypeTerm {
        match type_ {
            Type::Int => TypeTerm::Int,
            Type::Float => TypeTerm::Float,
            Type::String => TypeTerm::String,
            Type::BitArray => TypeTerm::BitArray,
            Type::Bool => TypeTerm::Bool,
            Type::Nil => TypeTerm::Nil,
            Type::Tuple(items) => TypeTerm::Tuple(items.iter().map(|item| self.type_to_inference_term(item)).collect()),
            Type::List(item) => TypeTerm::List(Box::new(self.type_to_inference_term(item))),
            Type::Record { name, fields } => TypeTerm::Record {
                name: name.clone(),
                fields: fields
                    .iter()
                    .map(|field| Field { name: field.name.clone(), type_: self.type_to_inference_term(&field.type_) })
                    .collect(),
            },
            Type::Custom { name, args } => TypeTerm::Custom {
                name: name.clone(),
                args: args.iter().map(|arg| self.type_to_inference_term(arg)).collect(),
            },
            Type::Generic(name) => inference_variable_from_name(name)
                .map(TypeTerm::Variable)
                .unwrap_or_else(|| TypeTerm::Generic(name.clone())),
            Type::Opaque { name, args } => TypeTerm::Opaque {
                name: name.clone(),
                args: args.iter().map(|arg| self.type_to_inference_term(arg)).collect(),
            },
            Type::Function { params, return_type } => TypeTerm::Function {
                params: params.iter().map(|param| self.type_to_inference_term(param)).collect(),
                return_type: Box::new(self.type_to_inference_term(return_type)),
            },
        }
    }

    fn instantiate_named_generics(&mut self, type_: &Type) -> Type {
        let mut substitutions = HashMap::new();
        self.instantiate_named_generics_with(type_, &mut substitutions)
    }

    fn instantiate_constructor(&mut self, constructor: &ConstructorInfo) -> ConstructorInfo {
        let mut substitutions = HashMap::new();
        ConstructorInfo {
            name: constructor.name.clone(),
            fields: constructor
                .fields
                .iter()
                .map(|field| FieldInfo {
                    name: field.name.clone(),
                    type_: self.instantiate_named_generics_with(&field.type_, &mut substitutions),
                })
                .collect(),
            return_type: self.instantiate_named_generics_with(&constructor.return_type, &mut substitutions),
            span: constructor.span,
        }
    }

    fn instantiate_named_generics_with(&mut self, type_: &Type, substitutions: &mut HashMap<String, Type>) -> Type {
        match type_ {
            Type::Generic(name) if !Self::is_inference_variable_name(name) => substitutions
                .entry(name.clone())
                .or_insert_with(|| self.fresh_inference_type())
                .clone(),
            Type::Tuple(items) => Type::Tuple(
                items
                    .iter()
                    .map(|item| self.instantiate_named_generics_with(item, substitutions))
                    .collect(),
            ),
            Type::List(item) => Type::List(Box::new(self.instantiate_named_generics_with(item, substitutions))),
            Type::Record { name, fields } => Type::Record {
                name: name.clone(),
                fields: fields
                    .iter()
                    .map(|field| FieldInfo {
                        name: field.name.clone(),
                        type_: self.instantiate_named_generics_with(&field.type_, substitutions),
                    })
                    .collect(),
            },
            Type::Custom { name, args } => Type::Custom {
                name: name.clone(),
                args: args
                    .iter()
                    .map(|arg| self.instantiate_named_generics_with(arg, substitutions))
                    .collect(),
            },
            Type::Opaque { name, args } => Type::Opaque {
                name: name.clone(),
                args: args
                    .iter()
                    .map(|arg| self.instantiate_named_generics_with(arg, substitutions))
                    .collect(),
            },
            Type::Function { params, return_type } => Type::Function {
                params: params
                    .iter()
                    .map(|param| self.instantiate_named_generics_with(param, substitutions))
                    .collect(),
                return_type: Box::new(self.instantiate_named_generics_with(return_type, substitutions)),
            },
            _ => type_.clone(),
        }
    }

    fn finalize_inferred_types(&mut self) {
        let mut names = HashMap::new();
        self.functions = self
            .functions
            .clone()
            .into_iter()
            .map(|function| TypedFunction {
                name: function.name,
                type_: self.finalize_type(&function.type_, &mut names),
            })
            .collect();
        self.expressions = self
            .expressions
            .clone()
            .into_iter()
            .map(|expression| TypedExpression {
                span: expression.span,
                type_: self.finalize_type(&expression.type_, &mut names),
            })
            .collect();
        self.interface.functions = self
            .interface
            .functions
            .clone()
            .into_iter()
            .map(|(name, type_)| (name, self.finalize_type(&type_, &mut HashMap::new())))
            .collect();
    }

    fn finalize_type(&self, type_: &Type, names: &mut HashMap<String, String>) -> Type {
        match self.resolve_inference_type(type_) {
            Type::Generic(name) if Self::is_inference_variable_name(&name) => {
                let next = names.len();
                Type::Generic(names.entry(name).or_insert_with(|| generic_name(next)).clone())
            }
            Type::Tuple(items) => Type::Tuple(items.iter().map(|item| self.finalize_type(item, names)).collect()),
            Type::List(item) => Type::List(Box::new(self.finalize_type(&item, names))),
            Type::Record { name, fields } => Type::Record {
                name,
                fields: fields
                    .iter()
                    .map(|field| FieldInfo { name: field.name.clone(), type_: self.finalize_type(&field.type_, names) })
                    .collect(),
            },
            Type::Custom { name, args } => {
                Type::Custom { name, args: args.iter().map(|arg| self.finalize_type(arg, names)).collect() }
            }
            Type::Opaque { name, args } => {
                Type::Opaque { name, args: args.iter().map(|arg| self.finalize_type(arg, names)).collect() }
            }
            Type::Function { params, return_type } => Type::Function {
                params: params.iter().map(|param| self.finalize_type(param, names)).collect(),
                return_type: Box::new(self.finalize_type(&return_type, names)),
            },
            other => other,
        }
    }

    fn check_ambiguous_function_types(&mut self) {
        for function in &self.functions {
            let Type::Function { params, return_type } = &function.type_ else {
                continue;
            };
            let parameter_generics = params
                .iter()
                .flat_map(generic_names_in_type)
                .collect::<std::collections::BTreeSet<_>>();
            let ambiguous = generic_names_in_type(return_type)
                .into_iter()
                .filter(|name| !parameter_generics.contains(name))
                .collect::<Vec<_>>();
            if ambiguous.is_empty() {
                continue;
            }
            self.diagnostics.push(
                Diagnostic::new(
                    DiagnosticCode::TypeError,
                    format!(
                        "ambiguous inferred type `{}` in return type of `{}`",
                        ambiguous[0], function.name.text
                    ),
                )
                .with_label(Label::primary(function.name.span, "ambiguous inferred type")),
            );
        }
    }

    fn define(&mut self, name: String, type_: Type) {
        self.define_scoped(name, type_, false);
    }

    fn define_generalized(&mut self, name: String, type_: Type) {
        self.define_scoped(name, type_, true);
    }

    fn define_scoped(&mut self, name: String, type_: Type, generalized: bool) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, ScopedType { type_, generalized });
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }
}

impl Type {
    pub fn from_source(source: &str) -> Option<Self> {
        parse_type_source(source)
    }

    pub fn custom(name: impl Into<String>, args: Vec<Type>) -> Self {
        Self::Custom { name: name.into(), args }
    }

    pub fn generic(name: impl Into<String>) -> Self {
        Self::Generic(name.into())
    }

    fn substitute_from(&self, actual: &Type) -> Type {
        let substitutions = substitutions_for(self, actual);
        substitute_type(self, &substitutions)
    }

    fn display(&self) -> String {
        match self {
            Type::Int => "Int".into(),
            Type::Float => "Float".into(),
            Type::String => "String".into(),
            Type::BitArray => "BitArray".into(),
            Type::Bool => "Bool".into(),
            Type::Nil => "Nil".into(),
            Type::Tuple(items) => format!("#({})", items.iter().map(Type::display).collect::<Vec<_>>().join(", ")),
            Type::List(item) => format!("List({})", item.display()),
            Type::Record { name, .. } => name.clone(),
            Type::Custom { name, args } => {
                if args.is_empty() {
                    name.clone()
                } else {
                    format!(
                        "{}({})",
                        name,
                        args.iter().map(Type::display).collect::<Vec<_>>().join(", ")
                    )
                }
            }
            Type::Generic(name) => name.clone(),
            Type::Opaque { name, args } => {
                if args.is_empty() {
                    name.clone()
                } else {
                    format!(
                        "{}({})",
                        name,
                        args.iter().map(Type::display).collect::<Vec<_>>().join(", ")
                    )
                }
            }
            Type::Function { params, return_type } => {
                let params = params.iter().map(Type::display).collect::<Vec<_>>().join(", ");
                format!("fn({params}) -> {}", return_type.display())
            }
        }
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

fn inference_variable_from_name(name: &str) -> Option<InferenceVariable> {
    name.strip_prefix('$')
        .and_then(|id| id.parse::<u64>().ok())
        .map(InferenceVariable)
}

fn type_term_to_type(type_: &TypeTerm) -> Option<Type> {
    match type_ {
        TypeTerm::Int => Some(Type::Int),
        TypeTerm::Float => Some(Type::Float),
        TypeTerm::String => Some(Type::String),
        TypeTerm::BitArray => Some(Type::BitArray),
        TypeTerm::Bool => Some(Type::Bool),
        TypeTerm::Nil => Some(Type::Nil),
        TypeTerm::Tuple(items) => Some(Type::Tuple(
            items.iter().map(type_term_to_type).collect::<Option<Vec<_>>>()?,
        )),
        TypeTerm::List(item) => Some(Type::List(Box::new(type_term_to_type(item)?))),
        TypeTerm::Record { name, fields } => Some(Type::Record {
            name: name.clone(),
            fields: fields
                .iter()
                .map(|field| Some(FieldInfo { name: field.name.clone(), type_: type_term_to_type(&field.type_)? }))
                .collect::<Option<Vec<_>>>()?,
        }),
        TypeTerm::Custom { name, args } => Some(Type::Custom {
            name: name.clone(),
            args: args.iter().map(type_term_to_type).collect::<Option<Vec<_>>>()?,
        }),
        TypeTerm::Generic(name) => Some(Type::Generic(name.clone())),
        TypeTerm::Opaque { name, args } => Some(Type::Opaque {
            name: name.clone(),
            args: args.iter().map(type_term_to_type).collect::<Option<Vec<_>>>()?,
        }),
        TypeTerm::Function { params, return_type } => Some(Type::Function {
            params: params.iter().map(type_term_to_type).collect::<Option<Vec<_>>>()?,
            return_type: Box::new(type_term_to_type(return_type)?),
        }),
        TypeTerm::Variable(variable) => Some(Type::Generic(format!("${}", variable.0))),
    }
}

fn generic_names_in_type(type_: &Type) -> Vec<String> {
    let mut names = Vec::new();
    collect_generic_names(type_, &mut names);
    names.sort();
    names.dedup();
    names
}

fn collect_generic_names(type_: &Type, names: &mut Vec<String>) {
    match type_ {
        Type::Generic(name) if !TypeChecker::is_inference_variable_name(name) => names.push(name.clone()),
        Type::Tuple(items) => items.iter().for_each(|item| collect_generic_names(item, names)),
        Type::List(item) => collect_generic_names(item, names),
        Type::Record { fields, .. } => fields
            .iter()
            .for_each(|field| collect_generic_names(&field.type_, names)),
        Type::Custom { args, .. } | Type::Opaque { args, .. } => {
            args.iter().for_each(|arg| collect_generic_names(arg, names));
        }
        Type::Function { params, return_type } => {
            params.iter().for_each(|param| collect_generic_names(param, names));
            collect_generic_names(return_type, names);
        }
        _ => {}
    }
}

fn eligible_for_local_generalization(expression: &Expression) -> bool {
    matches!(
        expression,
        Expression::Literal(_)
            | Expression::AnonymousFunction(_)
            | Expression::Tuple(_)
            | Expression::List(_)
            | Expression::Record(_)
            | Expression::Block(_)
    )
}

fn generic_name(index: usize) -> String {
    const NAMES: &[&str] = &["a", "b", "c", "d", "e", "f"];
    NAMES
        .get(index)
        .map(|name| (*name).to_string())
        .unwrap_or_else(|| format!("t{index}"))
}

fn constructor_name_text(name: &ast::ConstructorName) -> String {
    match name {
        ast::ConstructorName::Local(name) => name.text.clone(),
        ast::ConstructorName::Remote { name, .. } => name.text.clone(),
    }
}

fn constructor_pattern_name(pattern: &ast::ConstructorPattern) -> &str {
    match &pattern.constructor {
        ast::ConstructorName::Local(name) => &name.text,
        ast::ConstructorName::Remote { name, .. } => &name.text,
    }
}

fn substitutions_for(expected: &Type, actual: &Type) -> HashMap<String, Type> {
    let mut substitutions = HashMap::new();
    collect_substitutions(expected, actual, &mut substitutions);
    substitutions
}

fn collect_substitutions(expected: &Type, actual: &Type, substitutions: &mut HashMap<String, Type>) {
    match (expected, actual) {
        (Type::Generic(name), actual) => {
            substitutions.insert(name.clone(), actual.clone());
        }
        (Type::Tuple(expected), Type::Tuple(actual)) => {
            for (expected, actual) in expected.iter().zip(actual.iter()) {
                collect_substitutions(expected, actual, substitutions);
            }
        }
        (Type::List(expected), Type::List(actual)) => collect_substitutions(expected, actual, substitutions),
        (Type::Custom { name: expected_name, args: expected }, Type::Custom { name: actual_name, args: actual })
            if expected_name == actual_name =>
        {
            for (expected, actual) in expected.iter().zip(actual.iter()) {
                collect_substitutions(expected, actual, substitutions);
            }
        }
        _ => {}
    }
}

fn substitute_type(type_: &Type, substitutions: &HashMap<String, Type>) -> Type {
    match type_ {
        Type::Generic(name) => substitutions.get(name).cloned().unwrap_or_else(|| type_.clone()),
        Type::Tuple(items) => Type::Tuple(items.iter().map(|item| substitute_type(item, substitutions)).collect()),
        Type::List(item) => Type::List(Box::new(substitute_type(item, substitutions))),
        Type::Custom { name, args } => Type::Custom {
            name: name.clone(),
            args: args.iter().map(|arg| substitute_type(arg, substitutions)).collect(),
        },
        Type::Opaque { name, args } => Type::Opaque {
            name: name.clone(),
            args: args.iter().map(|arg| substitute_type(arg, substitutions)).collect(),
        },
        Type::Function { params, return_type } => Type::Function {
            params: params
                .iter()
                .map(|param| substitute_type(param, substitutions))
                .collect(),
            return_type: Box::new(substitute_type(return_type, substitutions)),
        },
        _ => type_.clone(),
    }
}

fn parse_type_source(source: &str) -> Option<Type> {
    let source = source.trim();
    match source {
        "Int" => Some(Type::Int),
        "Float" => Some(Type::Float),
        "String" => Some(Type::String),
        "BitArray" => Some(Type::BitArray),
        "Bool" => Some(Type::Bool),
        "Nil" => Some(Type::Nil),
        _ if source.starts_with("fn(") => parse_function_type(source),
        _ if source.starts_with("#(") && source.ends_with(')') => {
            Some(Type::Tuple(parse_type_list(&source[2..source.len() - 1])?))
        }
        _ if source.starts_with(char::is_lowercase) => Some(Type::Generic(source.into())),
        _ if let Some((name, args)) = source.split_once('(') => {
            let args = args.strip_suffix(')')?;
            let args = parse_type_list(args)?;
            if name == "List" && args.len() == 1 {
                Some(Type::List(Box::new(args.into_iter().next()?)))
            } else {
                Some(Type::Custom { name: name.into(), args })
            }
        }
        _ if source.chars().next().is_some_and(char::is_uppercase) => {
            Some(Type::Custom { name: source.into(), args: Vec::new() })
        }
        _ => None,
    }
}

fn parse_type_list(source: &str) -> Option<Vec<Type>> {
    if source.trim().is_empty() {
        return Some(Vec::new());
    }
    source.split(',').map(|item| parse_type_source(item.trim())).collect()
}

fn parse_function_type(source: &str) -> Option<Type> {
    let (params, return_type) = source.strip_prefix("fn(")?.split_once(") ->")?;
    let params = parse_type_list(params)?;
    let return_type = parse_type_source(return_type.trim())?;
    Some(Type::Function { params, return_type: Box::new(return_type) })
}

fn bit_string_pattern_bindings(raw: &ast::RawSyntax) -> Vec<ast::Name> {
    raw.source
        .trim()
        .strip_prefix("<<")
        .and_then(|source| source.strip_suffix(">>"))
        .into_iter()
        .flat_map(|inner| inner.split(','))
        .filter_map(|segment| segment.split(':').next())
        .map(str::trim)
        .filter(|name| name.chars().next().is_some_and(char::is_lowercase))
        .map(|text| ast::Name { span: raw.span, text: text.into() })
        .collect()
}

fn values_from_ast(module: &ast::Module) -> Vec<(String, Type)> {
    let mut checker = TypeChecker::new(ResolvedModule {
        ast: module.clone(),
        symbols: resolve::SymbolTable { symbols: Vec::new(), scopes: Vec::new() },
        references: Vec::new(),
    });
    let mut values = Vec::new();
    for declaration in &module.declarations {
        match declaration {
            Declaration::Function(function) => {
                if let Some(type_) = checker.function_type_from_annotations(function) {
                    values.push((function.name.text.clone(), type_.clone()));
                    values.push((format!("{}.{}", module_name(module), function.name.text), type_));
                }
            }
            Declaration::ExternalFunction(function) => {
                if let Some(type_) = checker.external_function_type(function) {
                    values.push((function.name.text.clone(), type_.clone()));
                    values.push((format!("{}.{}", module_name(module), function.name.text), type_));
                }
            }
            Declaration::Constant(constant) => {
                if let Some(type_) = checker.constant_type(constant) {
                    values.push((constant.name.text.clone(), type_.clone()));
                    values.push((format!("{}.{}", module_name(module), constant.name.text), type_));
                }
            }
            _ => {}
        }
    }
    values
}

fn module_name(module: &ast::Module) -> String {
    module
        .imports
        .first()
        .map(|import| import.module.text.clone())
        .unwrap_or_else(|| "module".into())
}

fn interface_constructors(interface: &ModuleInterface) -> Vec<(String, ConstructorInfo)> {
    interface
        .constructors
        .iter()
        .map(|(name, constructor)| (name.clone(), constructor.clone()))
        .collect()
}

fn qualified_values_from_interface(module: &str, interface: &ModuleInterface) -> Vec<(String, Type)> {
    let short = module.rsplit('/').next().unwrap_or(module);
    interface
        .functions
        .iter()
        .flat_map(|(name, type_)| {
            [
                (format!("{module}.{name}"), type_.clone()),
                (format!("{short}.{name}"), type_.clone()),
            ]
        })
        .collect()
}

fn constructors_from_ast(module: &ast::Module) -> Vec<(String, ConstructorInfo)> {
    module
        .declarations
        .iter()
        .filter_map(|declaration| match declaration {
            Declaration::TypeDefinition(raw) => type_definition_from_ast(raw),
            _ => None,
        })
        .flat_map(|declaration| {
            declaration
                .constructors
                .into_iter()
                .map(|constructor| (constructor.name.clone(), constructor))
        })
        .collect()
}

fn empty_use_continuation_diagnostic(span: Span) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::TypeError, "use has no continuation")
        .with_label(Label::primary(span, "nothing follows this use expression"))
        .with_note("`use` passes the following block statements as its callback body")
}

fn type_definition_from_ast(type_: &ast::TypeDefinition) -> Option<TypeDeclaration> {
    let parameters = type_.parameters.clone();
    let return_args = parameters.iter().cloned().map(Type::Generic).collect();
    let return_type = if type_.opaque {
        Type::Opaque { name: type_.name.text.clone(), args: return_args }
    } else {
        Type::Custom { name: type_.name.text.clone(), args: return_args }
    };
    let constructors = type_
        .constructors
        .iter()
        .map(|constructor| ConstructorInfo {
            name: constructor.name.text.clone(),
            fields: constructor
                .arguments
                .iter()
                .enumerate()
                .map(|(index, argument)| FieldInfo {
                    name: argument
                        .label
                        .as_ref()
                        .map(|label| label.text.clone())
                        .unwrap_or_else(|| format!("_{index}")),
                    type_: parse_type_source(&argument.type_annotation.source).unwrap_or(Type::Nil),
                })
                .collect(),
            return_type: return_type.clone(),
            span: constructor.span,
        })
        .collect();
    Some(TypeDeclaration {
        name: type_.name.text.clone(),
        parameters,
        opaque: type_.opaque,
        constructors,
        span: type_.span,
    })
}

fn type_alias_from_ast(alias: &ast::TypeAlias) -> Option<TypeDeclaration> {
    Some(alias.clone().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{SourceFile, SourceFileId};
    use crate::{ast, parse, project, resolve};
    use std::{fs, path::Path};
    use tempfile::tempdir;

    fn check_source(source: &str) -> Result<TypedModule, Diagnostics> {
        let source = SourceFile::new(SourceFileId(0), source);
        let cst = parse::parse(source).expect("parse source");
        let ast = ast::build(&cst).expect("build ast");
        let resolved = resolve::resolve(ast).expect("resolve names");
        check(resolved)
    }

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().expect("fixture parent")).expect("create fixture dir");
        fs::write(path, text).expect("write fixture");
    }

    #[test]
    fn types_literals_and_let_bindings() {
        let typed = check_source("fn main() { let x: Int = 1 x }").expect("type check source");

        assert!(typed.expressions.iter().any(|expression| expression.type_ == Type::Int));
    }

    #[test]
    fn checks_initial_stdlib_interfaces() {
        let typed = check_source(
            r#"import gleam/result.{Ok, Error}
import gleam/option.{Some, None}
import gleam/order.{Lt, Eq, Gt}

fn ok() -> Result(Int, String) { Ok(1) }
fn err() -> Result(Int, String) { Error("no") }
fn some() -> Option(Int) { Some(1) }
fn none() -> Option(Int) { None }
fn order(x: Int) -> Order { case x { 0 -> Eq 1 -> Gt _ -> Lt } }
"#,
        )
        .expect("type check stdlib interfaces");

        assert!(typed.interface.types.contains_key("Result"));
        assert!(typed.interface.types.contains_key("Option"));
        assert!(typed.interface.types.contains_key("Order"));
    }

    #[test]
    fn checks_direct_function_calls() {
        let typed = check_source("fn id(x: Int) -> Int { x }\nfn main() { id(1) }").expect("type check source");

        let main = typed
            .functions
            .iter()
            .find(|function| function.name.text == "main")
            .expect("main type");
        assert_eq!(
            main.type_,
            Type::Function { params: Vec::new(), return_type: Box::new(Type::Int) }
        );
    }

    #[test]
    fn reports_invalid_labelled_call_arguments() {
        let unknown = check_source(
            r#"fn id(value x: Int) -> Int { x }
fn main() { id(other: 1) }
"#,
        )
        .expect_err("unknown label should fail");
        assert!(
            unknown
                .iter()
                .any(|diagnostic| diagnostic.message == "unknown argument label `other`")
        );

        let duplicate = check_source(
            r#"fn add(left x: Int, right y: Int) -> Int { x + y }
fn main() { add(left: 1, left: 2) }
"#,
        )
        .expect_err("duplicate label should fail");
        assert!(
            duplicate
                .iter()
                .any(|diagnostic| diagnostic.message == "duplicate argument label `left`")
        );
    }

    #[test]
    fn reports_use_without_continuation() {
        let source = SourceFile::new(
            SourceFileId(0),
            r#"fn with_value(x: Int, f: fn(Int) -> Int) -> Int { f(x) }
fn main() -> Int {
  let value = use x <- with_value(1)
  value
}
"#,
        );
        let cst = parse::parse(source).expect("parse source");
        let ast = ast::build(&cst).expect("build ast");
        let diagnostics = resolve::resolve(ast).expect_err("use without continuation should fail");

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message == "use has no continuation")
        );
    }

    #[test]
    fn checks_partial_application_capture_types() {
        let typed = check_source(
            r#"fn call(f: fn(Int) -> Int, x: Int) -> Int { f(x) }
fn add(a: Int, b: Int) -> Int { a + b }
fn main(x: Int) -> Int {
  let addx = add(x, _)
  call(addx, 2)
}
"#,
        )
        .expect("type check source");

        assert!(typed.expressions.iter().any(|expression| {
            expression.type_ == Type::Function { params: vec![Type::Int], return_type: Box::new(Type::Int) }
        }));
    }

    #[test]
    fn reports_argument_type_mismatches() {
        let diagnostics =
            check_source("fn id(x: Int) -> Int { x }\nfn main() { id(\"no\") }").expect_err("type check should fail");

        assert_eq!(diagnostics[0].code, DiagnosticCode::TypeError);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("expected `Int`"))
        );
    }

    #[test]
    fn reports_wrong_arity() {
        let diagnostics =
            check_source("fn id(x: Int) -> Int { x }\nfn main() { id(1, 2) }").expect_err("type check should fail");

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("expected 1 arguments"))
        );
    }

    #[test]
    fn checks_case_branch_types() {
        let typed = check_source("fn main(x: Int) { case x { 0 -> 1 _ -> 2 } }").expect("type check source");

        let main = typed
            .functions
            .iter()
            .find(|function| function.name.text == "main")
            .expect("main type");
        assert_eq!(
            main.type_,
            Type::Function { params: vec![Type::Int], return_type: Box::new(Type::Int) }
        );
    }

    #[test]
    fn reports_case_branch_mismatches() {
        let diagnostics =
            check_source("fn main(x: Int) { case x { 0 -> 1 _ -> \"two\" } }").expect_err("type check should fail");

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("expected `Int`"))
        );
    }

    #[test]
    fn infers_unannotated_identity() {
        let typed = check_source("fn main(x) { x }").expect("type check source");

        let main = typed
            .functions
            .iter()
            .find(|function| function.name.text == "main")
            .expect("main type");
        assert_eq!(
            main.type_,
            Type::Function {
                params: vec![Type::Generic("a".into())],
                return_type: Box::new(Type::Generic("a".into()))
            }
        );
    }

    #[test]
    fn infers_generic_lists_and_polymorphic_calls() {
        let typed = check_source(
            r#"fn id(x) { x }
fn singleton(x) { [x] }
fn main() { #(id(1), id("one"), singleton(True)) }
"#,
        )
        .expect("type check source");

        let main = typed
            .functions
            .iter()
            .find(|function| function.name.text == "main")
            .expect("main type");
        assert_eq!(
            main.type_,
            Type::Function {
                params: vec![],
                return_type: Box::new(Type::Tuple(vec![
                    Type::Int,
                    Type::String,
                    Type::List(Box::new(Type::Bool))
                ]))
            }
        );
    }

    #[test]
    fn reports_ambiguous_return_types() {
        let diagnostics = check_source("fn main() { [] }").expect_err("ambiguous type should fail");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("ambiguous inferred type `a` in return type of `main`")
        }));
    }

    #[test]
    fn generalizes_eligible_local_bindings() {
        let typed = check_source(
            r#"fn main() {
  let id = fn(x) { x }
  #(id(1), id("one"))
}
"#,
        )
        .expect("type check source");

        let main = typed
            .functions
            .iter()
            .find(|function| function.name.text == "main")
            .expect("main type");
        assert_eq!(
            main.type_,
            Type::Function { params: vec![], return_type: Box::new(Type::Tuple(vec![Type::Int, Type::String])) }
        );
    }

    #[test]
    fn infers_generic_custom_type_constructors_and_patterns() {
        let typed = check_source(
            r#"pub type Box(value) { Box(value) }
fn make(x) { Box(x) }
fn unwrap(box) { case box { Box(value) -> value } }
fn main() { #(unwrap(Box(1)), unwrap(Box("one"))) }
"#,
        )
        .expect("type check source");

        let main = typed
            .functions
            .iter()
            .find(|function| function.name.text == "main")
            .expect("main type");
        assert_eq!(
            main.type_,
            Type::Function { params: vec![], return_type: Box::new(Type::Tuple(vec![Type::Int, Type::String])) }
        );
    }

    #[test]
    fn infers_imported_generic_functions() {
        let dir = tempdir().expect("tempdir");
        write(
            &dir.path().join("gleam.toml"),
            "name = \"sample\"\nversion = \"1.0.0\"\n",
        );
        write(&dir.path().join("src/app.gleam"), "pub fn id(x) { x }\n");
        write(
            &dir.path().join("src/main.gleam"),
            "import app\nfn main() { #(app.id(1), app.id(\"one\")) }\n",
        );
        let project = project::load_project(dir.path()).expect("load project");

        let typed = check_project(&project).expect("type check project");

        let main_module = typed
            .modules
            .iter()
            .find(|module| {
                module
                    .resolved
                    .ast
                    .imports
                    .iter()
                    .any(|import| import.module.text == "app")
            })
            .expect("main module");
        let main = main_module
            .functions
            .iter()
            .find(|function| function.name.text == "main")
            .expect("main type");
        assert_eq!(
            main.type_,
            Type::Function { params: vec![], return_type: Box::new(Type::Tuple(vec![Type::Int, Type::String])) }
        );
    }

    #[test]
    fn preserves_imported_function_labels_for_use_callbacks() {
        let dir = tempdir().expect("tempdir");
        write(
            &dir.path().join("gleam.toml"),
            "name = \"sample\"\nversion = \"1.0.0\"\n",
        );
        write(
            &dir.path().join("src/app.gleam"),
            "pub fn with_value(callback f: fn(Int) -> Int, value x: Int) -> Int { f(x) }\n",
        );
        write(
            &dir.path().join("src/main.gleam"),
            r#"import app.{with_value}
fn main() -> Int {
  use value <- with_value(value: 41)
  value + 1
}
"#,
        );
        let project = project::load_project(dir.path()).expect("load project");

        let typed = check_project(&project).expect("type check project");
        let app = typed.interfaces.get("app").expect("app interface");

        assert_eq!(
            app.function_labels.get("with_value"),
            Some(&vec![Some("callback".into()), Some("value".into())])
        );
    }

    #[test]
    fn parses_generic_tuple_list_and_custom_annotations() {
        assert_eq!(Type::from_source("BitArray"), Some(Type::BitArray));
        assert_eq!(Type::from_source("List(Int)"), Some(Type::List(Box::new(Type::Int))));
        assert_eq!(
            Type::from_source("#(String, Int)"),
            Some(Type::Tuple(vec![Type::String, Type::Int]))
        );
        assert_eq!(
            Type::from_source("Result(Int, String)"),
            Some(Type::Custom { name: "Result".into(), args: vec![Type::Int, Type::String] })
        );
        assert_eq!(Type::from_source("value"), Some(Type::Generic("value".into())));
    }

    #[test]
    fn reports_generic_arity_mismatches() {
        let diagnostics = check_source(
            r#"pub type Box(value) { Box(value) }
fn main(value: Box(Int, String)) { value }
"#,
        )
        .expect_err("arity should fail");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("type `Box` expected 1 type arguments but got 2")
        }));
    }

    #[test]
    fn records_custom_types_constructors_and_fields_in_interface() {
        let typed = check_source(
            r#"pub type User { User(name: String, age: Int) }
fn new_user() { User(name: "Ada", age: 36) }
"#,
        )
        .expect("type check source");

        let user = typed.interface.types.get("User").expect("User type declaration");
        assert_eq!(user.constructors[0].name, "User");
        assert_eq!(user.constructors[0].fields[0].name, "name");
        assert!(typed.interface.constructors.contains_key("User"));
    }

    #[test]
    fn records_opaque_and_generic_type_declarations() {
        let typed = check_source("pub opaque type Box(value) { Box(value) }").expect("type check source");

        let box_ = typed.interface.types.get("Box").expect("Box type declaration");
        assert!(box_.opaque);
        assert_eq!(box_.parameters, ["value"]);
        assert_eq!(
            box_.constructors[0].return_type,
            Type::Opaque { name: "Box".into(), args: vec![Type::Generic("value".into())] }
        );
    }

    #[test]
    fn checks_tuple_list_record_and_constructor_patterns() {
        let typed = check_source(
            r#"pub type Person {
  Person(name: String, age: Int)
}

pub type Outcome(value) {
  Ok(value)
  Error(String)
}

fn tuple(pair: #(Int, String)) { case pair { #(number, _) -> number } }
fn list(items: List(Int)) { case items { [head, ..tail] -> head _ -> 0 } }
fn record(person: Person) { case person { Person(name:, age: _) -> name } }
fn generic(result: Outcome(Int)) { case result { Ok(value) -> value Error(_) -> 0 } }
"#,
        )
        .expect("type check source");

        assert!(
            typed
                .expressions
                .iter()
                .any(|expression| expression.type_ == Type::String)
        );
    }

    #[test]
    fn checks_imported_constructor_patterns_in_projects() {
        let dir = tempdir().expect("tempdir");
        write(
            &dir.path().join("gleam.toml"),
            "name = \"sample\"\nversion = \"1.0.0\"\n",
        );
        write(&dir.path().join("src/app.gleam"), "pub type Boxed { Boxed(Int) }\n");
        write(
            &dir.path().join("src/main.gleam"),
            "import app\nfn main(value: Boxed) { case value { app.Boxed(inner) -> inner } }\n",
        );
        let project = project::load_project(dir.path()).expect("load project");

        let typed = check_project(&project).expect("type check project");

        assert_eq!(typed.modules.len(), 2);
    }

    #[test]
    fn reports_invalid_nested_patterns() {
        let diagnostics = check_source(
            r#"pub type Person {
  Person(name: String, age: Int)
}

fn main(person: Person) { case person { Person(name: 1, age: _) -> 0 } }
"#,
        )
        .expect_err("invalid pattern should fail");

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("expected `Int` but found `String`"))
        );
        insta::assert_snapshot!(
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.render_plain())
                .collect::<Vec<_>>()
                .join("\n\n")
        );
    }

    #[test]
    fn checks_case_guards_are_bool() {
        let diagnostics = check_source(
            r#"pub type Boxed {
  Boxed(Int)
}

fn main(value: Boxed) { case value { Boxed(number) if number -> number } }
"#,
        )
        .expect_err("guard should be bool");

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("expected `Bool` but found `Int`"))
        );
    }

    #[test]
    fn reports_bool_exhaustiveness_and_redundancy() {
        let missing = check_source("fn main(flag: Bool) { case flag { True -> 1 } }")
            .expect_err("non-exhaustive bool case should fail");
        assert!(
            missing
                .iter()
                .any(|diagnostic| diagnostic.message.contains("not exhaustive"))
        );

        let redundant = check_source("fn main(flag: Bool) { case flag { _ -> 1 False -> 0 } }")
            .expect_err("redundant branch should fail");
        assert!(
            redundant
                .iter()
                .any(|diagnostic| diagnostic.message.contains("unreachable"))
        );

        let rendered = missing
            .iter()
            .chain(redundant.iter())
            .map(|diagnostic| diagnostic.render_plain())
            .collect::<Vec<_>>()
            .join("\n\n");
        insta::assert_snapshot!(rendered);
    }
}

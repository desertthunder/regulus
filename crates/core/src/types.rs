use std::collections::HashMap;

use crate::{
    ast::{self, Declaration, Expression, LiteralKind, Pattern, Statement},
    diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, Label},
    project::Project,
    resolve::{self, ResolvedModule},
    source::Span,
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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModuleInterface {
    pub functions: HashMap<String, Type>,
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

fn check_with_constructors(
    module: ResolvedModule, external_constructors: HashMap<String, ConstructorInfo>,
) -> Result<TypedModule, Diagnostics> {
    TypeChecker::new(module)
        .with_external_constructors(external_constructors)
        .check()
}

pub fn check_project(project: &Project) -> Result<TypedProject, Diagnostics> {
    let resolved = resolve::resolve_project(project)?;
    let mut modules = Vec::new();
    let mut interfaces = HashMap::new();
    let mut diagnostics = Vec::new();

    let external_constructors = resolved
        .modules
        .iter()
        .flat_map(|module| constructors_from_ast(&module.ast))
        .collect::<HashMap<_, _>>();

    for (module_info, module) in project.graph.modules.iter().zip(resolved.modules) {
        match check_with_constructors(module, external_constructors.clone()) {
            Ok(typed) => {
                interfaces.insert(module_info.name.clone(), typed.interface.clone());
                modules.push(typed);
            }
            Err(mut errors) => diagnostics.append(&mut errors),
        }
    }

    if diagnostics.is_empty() { Ok(TypedProject { modules, interfaces }) } else { Err(diagnostics) }
}

struct TypeChecker {
    module: ResolvedModule,
    function_types: HashMap<String, Type>,
    constructors: HashMap<String, ConstructorInfo>,
    interface: ModuleInterface,
    functions: Vec<TypedFunction>,
    expressions: Vec<TypedExpression>,
    scopes: Vec<HashMap<String, Type>>,
    diagnostics: Diagnostics,
}

impl TypeChecker {
    fn new(module: ResolvedModule) -> Self {
        Self {
            module,
            function_types: HashMap::new(),
            constructors: HashMap::new(),
            interface: ModuleInterface::default(),
            functions: Vec::new(),
            expressions: Vec::new(),
            scopes: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn with_external_constructors(mut self, constructors: HashMap<String, ConstructorInfo>) -> Self {
        self.constructors.extend(constructors);
        self
    }

    fn check(mut self) -> Result<TypedModule, Diagnostics> {
        self.collect_type_declarations();
        self.collect_annotated_function_types();

        for function in self.module.ast.functions.clone() {
            self.check_function(&function);
        }

        if self.diagnostics.is_empty() {
            Ok(TypedModule {
                resolved: self.module,
                functions: self.functions,
                expressions: self.expressions,
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
                _ => {}
            }
        }
    }

    fn collect_annotated_function_types(&mut self) {
        for function in self.module.ast.functions.clone() {
            if let Some(type_) = self.function_type_from_annotations(&function) {
                self.interface
                    .functions
                    .insert(function.name.text.clone(), type_.clone());
                self.function_types.insert(function.name.text, type_);
            }
        }
    }

    fn check_function(&mut self, function: &ast::Function) {
        self.push_scope();

        let mut params = Vec::new();
        for parameter in &function.parameters {
            let Some(name) = &parameter.name else {
                continue;
            };
            let Some(type_annotation) = &parameter.type_annotation else {
                self.diagnostics.push(
                    Diagnostic::new(
                        DiagnosticCode::TypeError,
                        format!("parameter `{}` needs a type annotation", name.text),
                    )
                    .with_label(Label::primary(name.span, "missing type annotation")),
                );
                continue;
            };
            let Some(type_) = self.parse_type_annotation(type_annotation) else {
                continue;
            };
            self.define(name.text.clone(), type_.clone());
            params.push(type_);
        }

        let body_type = self.check_block(&function.body).unwrap_or(Type::Nil);
        let return_type = match &function.return_type {
            Some(annotation) => match self.parse_type_annotation(annotation) {
                Some(type_) => {
                    self.expect_same(&type_, &body_type, function.body.span);
                    type_
                }
                None => body_type.clone(),
            },
            None => body_type.clone(),
        };

        let function_type = Type::Function { params, return_type: Box::new(return_type) };
        self.function_types
            .insert(function.name.text.clone(), function_type.clone());
        self.functions
            .push(TypedFunction { name: function.name.clone(), type_: function_type });

        self.pop_scope();
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

    fn check_block(&mut self, block: &ast::Block) -> Option<Type> {
        let mut last_type = Type::Nil;
        for statement in &block.statements {
            last_type = match statement {
                Statement::Let(let_) => {
                    let value_type = self.check_expression(&let_.value)?;
                    if let Some(annotation) = &let_.type_annotation
                        && let Some(expected) = self.parse_type_annotation(annotation)
                    {
                        self.expect_same(&expected, &value_type, let_.span);
                    }
                    self.bind_pattern(&let_.pattern, &value_type);
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
                Statement::Expression(expression) => self.check_expression(expression)?,
            };
        }
        Some(last_type)
    }

    fn check_expression(&mut self, expression: &Expression) -> Option<Type> {
        let type_ = match expression {
            Expression::Literal(literal) => Type::from(&literal.kind),
            Expression::Variable(name) => self.lookup_name(name)?,
            Expression::Call(call) => {
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

                for (argument, expected) in call.arguments.iter().zip(params.iter()) {
                    if let Some(actual) = self.check_expression(&argument.value) {
                        self.expect_same(expected, &actual, argument.span);
                    }
                }

                *return_type
            }
            Expression::FieldAccess(field_access) => {
                self.diagnostics.push(
                    Diagnostic::new(DiagnosticCode::TypeError, "qualified values cannot be typed yet")
                        .with_label(Label::primary(field_access.span, "qualified value here")),
                );
                return None;
            }
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
            Expression::List(list) => {
                for element in &list.elements {
                    self.check_expression(element)?;
                }
                if let Some(spread) = &list.spread {
                    self.check_expression(spread)?;
                }
                Type::List(Box::new(Type::Int))
            }
            Expression::Record(record) => {
                let name = constructor_name_text(&record.constructor);
                let Some(constructor) = self.constructors.get(&name).cloned() else {
                    self.diagnostics.push(
                        Diagnostic::new(DiagnosticCode::TypeError, format!("unknown constructor `{name}`"))
                            .with_label(Label::primary(record.span, "unknown constructor here")),
                    );
                    return None;
                };
                constructor.return_type
            }
            Expression::BitArray(_) => Type::BitArray,
            Expression::Panic(_) | Expression::Todo(_) | Expression::Assert(_) => Type::Nil,
            Expression::BinaryOperation(operation) => {
                self.check_expression(&operation.left)?;
                self.check_expression(&operation.right)?;
                match operation.operator {
                    ast::BinaryOperator::Equal
                    | ast::BinaryOperator::NotEqual
                    | ast::BinaryOperator::LessThan
                    | ast::BinaryOperator::LessThanEqual
                    | ast::BinaryOperator::GreaterThan
                    | ast::BinaryOperator::GreaterThanEqual
                    | ast::BinaryOperator::FloatLessThan
                    | ast::BinaryOperator::FloatLessThanEqual
                    | ast::BinaryOperator::FloatGreaterThan
                    | ast::BinaryOperator::FloatGreaterThanEqual
                    | ast::BinaryOperator::And
                    | ast::BinaryOperator::Or => Type::Bool,
                    ast::BinaryOperator::FloatAdd
                    | ast::BinaryOperator::FloatSubtract
                    | ast::BinaryOperator::FloatMultiply
                    | ast::BinaryOperator::FloatDivide => Type::Float,
                    _ => Type::Int,
                }
            }
            Expression::Pipeline(pipeline) => {
                self.check_expression(&pipeline.value)?;
                self.check_expression(&pipeline.into)?
            }
            Expression::UnaryOperation(operation) => self.check_expression(&operation.value)?,
            Expression::Use(use_) => self.check_expression(&use_.value)?,
            Expression::AnonymousFunction(_) | Expression::Capture(_) => Type::Nil,
            Expression::RecordUpdate(update) => self.check_expression(&update.spread)?,
            Expression::TupleAccess(_) => Type::Int,
            Expression::Echo(echo) => self.check_expression(&echo.value)?,
            Expression::Raw(raw) if raw.kind == "bit_string" => Type::BitArray,
            Expression::Raw(raw) if raw.kind == "tuple" => Type::Tuple(
                raw.source
                    .trim()
                    .trim_start_matches("#(")
                    .trim_end_matches(')')
                    .split(',')
                    .filter(|item| !item.trim().is_empty())
                    .map(|_| Type::Int)
                    .collect(),
            ),
            Expression::Raw(raw) if raw.kind == "list" => Type::List(Box::new(Type::Int)),
            Expression::Raw(raw) if raw.kind == "record" => {
                let Some(name) = raw.source.split(['(', ' ']).next() else {
                    return None;
                };
                let Some(constructor) = self.constructors.get(name).cloned() else {
                    self.diagnostics.push(
                        Diagnostic::new(DiagnosticCode::TypeError, format!("unknown constructor `{name}`"))
                            .with_label(Label::primary(raw.span, "unknown constructor here")),
                    );
                    return None;
                };
                constructor.return_type
            }
            Expression::Raw(raw) => {
                self.diagnostics.push(
                    Diagnostic::new(
                        DiagnosticCode::TypeError,
                        format!("unsupported expression `{}`", raw.kind),
                    )
                    .with_label(Label::primary(raw.span, "unsupported expression here")),
                );
                return None;
            }
            Expression::Case(case) => {
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
                result_type.unwrap_or(Type::Nil)
            }
        };

        self.expressions
            .push(TypedExpression { span: expression.span(), type_: type_.clone() });
        Some(type_)
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
            Pattern::BitString(raw) => self.expect_same(&Type::BitArray, type_, raw.span),
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

        self.expect_same(&constructor.return_type.substitute_from(type_), type_, pattern.span);
        let fields = instantiate_fields(&constructor, type_);
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
            if let Some(type_) = scope.get(&name.text) {
                return Some(type_.clone());
            }
        }

        if let Some(type_) = self.function_types.get(&name.text) {
            return Some(type_.clone());
        }

        self.diagnostics.push(
            Diagnostic::new(DiagnosticCode::TypeError, format!("no type known for `{}`", name.text))
                .with_label(Label::primary(name.span, "unknown type")),
        );
        None
    }

    fn parse_type_annotation(&mut self, annotation: &ast::TypeAnnotation) -> Option<Type> {
        match parse_type_source(&annotation.source) {
            Some(type_) => Some(type_),
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

    fn expect_same(&mut self, expected: &Type, actual: &Type, span: Span) {
        if expected != actual {
            self.diagnostics.push(
                Diagnostic::new(
                    DiagnosticCode::TypeError,
                    format!("expected `{}` but found `{}`", expected.display(), actual.display()),
                )
                .with_label(Label::primary(span, "type mismatch")),
            );
        }
    }

    fn define(&mut self, name: String, type_: Type) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, type_);
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

fn instantiate_fields(constructor: &ConstructorInfo, actual_type: &Type) -> Vec<FieldInfo> {
    let substitutions = substitutions_for(&constructor.return_type, actual_type);
    constructor
        .fields
        .iter()
        .map(|field| FieldInfo { name: field.name.clone(), type_: substitute_type(&field.type_, &substitutions) })
        .collect()
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
    Some(TypeDeclaration {
        name: alias.name.text.clone(),
        parameters: alias.parameters.clone(),
        opaque: alias.opaque,
        constructors: Vec::new(),
        span: alias.span,
    })
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
        let ast = ast::build(cst).expect("build ast");
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
    fn requires_parameter_annotations() {
        let diagnostics = check_source("fn main(x) { x }").expect_err("type check should fail");

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("needs a type annotation"))
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

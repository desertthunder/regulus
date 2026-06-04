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

pub fn check_project(project: &Project) -> Result<TypedProject, Diagnostics> {
    let resolved = resolve::resolve_project(project)?;
    let mut modules = Vec::new();
    let mut interfaces = HashMap::new();
    let mut diagnostics = Vec::new();

    for (module_info, module) in project.graph.modules.iter().zip(resolved.modules) {
        match check(module) {
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
                    if let Some(type_declaration) = parse_type_definition(&raw) {
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
                    if let Some(alias) = parse_type_alias(&raw) {
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
                Statement::Expression(expression) => self.check_expression(expression)?,
            };
        }
        Some(last_type)
    }

    fn check_expression(&mut self, expression: &Expression) -> Option<Type> {
        let type_ = match expression {
            Expression::Literal(literal) => match literal.kind {
                LiteralKind::Int => Type::Int,
                LiteralKind::Float => Type::Float,
                LiteralKind::String => Type::String,
                LiteralKind::Bool => Type::Bool,
                LiteralKind::Nil => Type::Nil,
            },
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
                    let clause_type = self.check_expression(&clause.value)?;
                    self.pop_scope();

                    match &result_type {
                        Some(expected) => self.expect_same(expected, &clause_type, clause.value.span()),
                        None => result_type = Some(clause_type),
                    }
                }

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
            Pattern::Raw(raw) => self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::TypeError, format!("unsupported pattern `{}`", raw.kind))
                    .with_label(Label::primary(raw.span, "unsupported pattern here")),
            ),
        }
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

    fn display(&self) -> String {
        match self {
            Type::Int => "Int".into(),
            Type::Float => "Float".into(),
            Type::String => "String".into(),
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
        match self {
            Expression::Literal(literal) => literal.span,
            Expression::Variable(name) => name.span,
            Expression::Call(call) => call.span,
            Expression::FieldAccess(field_access) => field_access.span,
            Expression::Block(block) => block.span,
            Expression::Case(case) => case.span,
            Expression::Raw(raw) => raw.span,
        }
    }
}

fn parse_type_source(source: &str) -> Option<Type> {
    let source = source.trim();
    match source {
        "Int" => Some(Type::Int),
        "Float" => Some(Type::Float),
        "String" => Some(Type::String),
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

fn parse_type_definition(raw: &ast::RawSyntax) -> Option<TypeDeclaration> {
    let header = raw.source.split('{').next()?.trim();
    let opaque = header.split_whitespace().any(|word| word == "opaque");
    let name = type_decl_name(header)?.to_string();
    let parameters = type_parameters(header);
    let return_type = if opaque {
        Type::Opaque { name: name.clone(), args: parameters.iter().cloned().map(Type::Generic).collect() }
    } else {
        Type::Custom { name: name.clone(), args: parameters.iter().cloned().map(Type::Generic).collect() }
    };
    let constructors = raw
        .source
        .split_once('{')?
        .1
        .lines()
        .filter_map(|line| parse_constructor(line.trim(), &return_type, raw.span))
        .collect::<Vec<_>>();

    Some(TypeDeclaration { name, parameters, opaque, constructors, span: raw.span })
}

fn parse_type_alias(raw: &ast::RawSyntax) -> Option<TypeDeclaration> {
    let header = raw.source.split('=').next()?.trim();
    let name = type_decl_name(header)?.to_string();
    Some(TypeDeclaration {
        name,
        parameters: type_parameters(header),
        opaque: false,
        constructors: Vec::new(),
        span: raw.span,
    })
}

fn type_decl_name(header: &str) -> Option<&str> {
    header
        .split_whitespace()
        .filter(|word| *word != "pub" && *word != "opaque" && *word != "type")
        .next()
        .map(|word| word.split(['(', '{', '=']).next().unwrap_or(word))
}

fn type_parameters(header: &str) -> Vec<String> {
    let Some(params) = header
        .split_once('(')
        .and_then(|(_, rest)| rest.split_once(')').map(|(params, _)| params))
    else {
        return Vec::new();
    };
    params
        .split(',')
        .map(str::trim)
        .filter(|param| !param.is_empty())
        .map(String::from)
        .collect()
}

fn parse_constructor(line: &str, return_type: &Type, span: Span) -> Option<ConstructorInfo> {
    let line = line.trim_end_matches('}').trim();
    if line.is_empty() || !line.chars().next().is_some_and(char::is_uppercase) {
        return None;
    }
    let name = line.split(['(', ' ']).next()?.to_string();
    let fields = match line
        .split_once('(')
        .and_then(|(_, rest)| rest.rsplit_once(')').map(|(fields, _)| fields))
    {
        Some(fields) => parse_fields(fields)?,
        None => Vec::new(),
    };
    Some(ConstructorInfo { name, fields, return_type: return_type.clone(), span })
}

fn parse_fields(source: &str) -> Option<Vec<FieldInfo>> {
    if source.trim().is_empty() {
        return Some(Vec::new());
    }
    source
        .split(',')
        .enumerate()
        .map(|(index, field)| {
            let field = field.trim();
            if let Some((name, type_)) = field.split_once(':') {
                Some(FieldInfo { name: name.trim().into(), type_: parse_type_source(type_.trim())? })
            } else {
                Some(FieldInfo { name: format!("_{index}"), type_: parse_type_source(field)? })
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ast, parse, resolve,
        source::{SourceFile, SourceFileId},
    };

    fn check_source(source: &str) -> Result<TypedModule, Diagnostics> {
        let source = SourceFile::new(SourceFileId(0), source);
        let cst = parse::parse(source).expect("parse source");
        let ast = ast::build(cst).expect("build ast");
        let resolved = resolve::resolve(ast).expect("resolve names");
        check(resolved)
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
}

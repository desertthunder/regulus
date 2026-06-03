use tree_sitter::Node;

use crate::{
    diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, Label},
    parse::ConcreteSyntaxTree,
    source::{SourceFile, Span},
};

/// Compiler-owned AST module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    pub span: Span,
    pub declarations: Vec<Declaration>,
    pub imports: Vec<Import>,
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Declaration {
    Import(Import),
    Function(Function),
    Constant(RawSyntax),
    ExternalFunction(RawSyntax),
    ExternalType(RawSyntax),
    TypeAlias(RawSyntax),
    TypeDefinition(RawSyntax),
    Attribute(RawSyntax),
    TargetGroup(RawSyntax),
    Statement(RawSyntax),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    pub span: Span,
    pub module: Name,
    pub alias: Option<Name>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub span: Span,
    pub public: bool,
    pub name: Name,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<TypeAnnotation>,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    pub span: Span,
    pub label: Option<Name>,
    pub name: Option<Name>,
    pub type_annotation: Option<TypeAnnotation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeAnnotation {
    pub span: Span,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub span: Span,
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    Let(Let),
    Expression(Expression),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Let {
    pub span: Span,
    pub pattern: Pattern,
    pub type_annotation: Option<TypeAnnotation>,
    pub value: Expression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pattern {
    Name(Name),
    Discard(Span),
    Integer(Literal),
    Float(Literal),
    String(Literal),
    Raw(RawSyntax),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expression {
    Literal(Literal),
    Variable(Name),
    Call(Call),
    FieldAccess(FieldAccess),
    Block(Block),
    Case(Case),
    Raw(RawSyntax),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Literal {
    pub span: Span,
    pub kind: LiteralKind,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiteralKind {
    Int,
    Float,
    String,
    Bool,
    Nil,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    pub span: Span,
    pub function: Box<Expression>,
    pub arguments: Vec<Argument>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Argument {
    pub span: Span,
    pub label: Option<Name>,
    pub value: Expression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldAccess {
    pub span: Span,
    pub record: Box<Expression>,
    pub field: Name,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Case {
    pub span: Span,
    pub subjects: Vec<Expression>,
    pub clauses: Vec<CaseClause>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseClause {
    pub span: Span,
    pub patterns: Vec<Pattern>,
    pub value: Expression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name {
    pub span: Span,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSyntax {
    pub span: Span,
    pub kind: String,
    pub source: String,
}

pub fn build(cst: ConcreteSyntaxTree) -> Result<Module, Diagnostics> {
    let root = cst.tree.root_node();
    let builder = AstBuilder { source: &cst.source };
    builder.module(root)
}

struct AstBuilder<'a> {
    source: &'a SourceFile,
}

impl AstBuilder<'_> {
    fn module(&self, node: Node<'_>) -> Result<Module, Diagnostics> {
        let mut declarations = Vec::new();
        let mut imports = Vec::new();
        let mut functions = Vec::new();

        for child in self.named_children(node) {
            let declaration = self.declaration(child)?;
            match &declaration {
                Declaration::Import(import) => imports.push(import.clone()),
                Declaration::Function(function) => functions.push(function.clone()),
                _ => {}
            }
            declarations.push(declaration);
        }

        Ok(Module { span: self.span(node), declarations, imports, functions })
    }

    fn declaration(&self, node: Node<'_>) -> Result<Declaration, Diagnostics> {
        match node.kind() {
            "import" => self.import(node).map(Declaration::Import),
            "function" => self.function(node).map(Declaration::Function),
            "constant" => Ok(Declaration::Constant(self.raw(node))),
            "external_function" => Ok(Declaration::ExternalFunction(self.raw(node))),
            "external_type" => Ok(Declaration::ExternalType(self.raw(node))),
            "type_alias" => Ok(Declaration::TypeAlias(self.raw(node))),
            "type_definition" => Ok(Declaration::TypeDefinition(self.raw(node))),
            "attribute" => Ok(Declaration::Attribute(self.raw(node))),
            "target_group" => Ok(Declaration::TargetGroup(self.raw(node))),
            _ => Ok(Declaration::Statement(self.raw(node))),
        }
    }

    fn import(&self, node: Node<'_>) -> Result<Import, Diagnostics> {
        let module = self.required_name_field(node, "module")?;
        let alias = self.name_field(node, "alias")?;

        Ok(Import { span: self.span(node), module, alias })
    }

    fn function(&self, node: Node<'_>) -> Result<Function, Diagnostics> {
        let public = self
            .named_children(node)
            .into_iter()
            .any(|child| child.kind() == "visibility_modifier");
        let name = self.required_name_field(node, "name")?;
        let parameters = node
            .child_by_field_name("parameters")
            .map(|parameters| self.parameters(parameters))
            .transpose()?
            .unwrap_or_default();
        let return_type = self.type_field(node, "return_type")?;
        let body_node = node
            .child_by_field_name("body")
            .ok_or_else(|| vec![self.missing(node, "function body")])?;
        let body = self.block_like(body_node)?;

        Ok(Function { span: self.span(node), public, name, parameters, return_type, body })
    }

    fn parameters(&self, node: Node<'_>) -> Result<Vec<Parameter>, Diagnostics> {
        self.named_children(node)
            .into_iter()
            .filter(|child| child.kind() == "function_parameter")
            .map(|child| self.parameter(child))
            .collect()
    }

    fn parameter(&self, node: Node<'_>) -> Result<Parameter, Diagnostics> {
        Ok(Parameter {
            span: self.span(node),
            label: self.name_field(node, "label")?,
            name: self.name_field(node, "name")?,
            type_annotation: self.type_field(node, "type")?,
        })
    }

    fn block_like(&self, node: Node<'_>) -> Result<Block, Diagnostics> {
        let statements = self
            .named_children(node)
            .into_iter()
            .map(|child| self.statement(child))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Block { span: self.span(node), statements })
    }

    fn statement(&self, node: Node<'_>) -> Result<Statement, Diagnostics> {
        match node.kind() {
            "let" => Ok(Statement::Let(self.let_statement(node)?)),
            _ => Ok(Statement::Expression(self.expression(node)?)),
        }
    }

    fn let_statement(&self, node: Node<'_>) -> Result<Let, Diagnostics> {
        let pattern_node = node
            .child_by_field_name("pattern")
            .ok_or_else(|| vec![self.missing(node, "let pattern")])?;
        let value_node = node
            .child_by_field_name("value")
            .ok_or_else(|| vec![self.missing(node, "let value")])?;

        Ok(Let {
            span: self.span(node),
            pattern: self.pattern(pattern_node)?,
            type_annotation: self.type_field(node, "type")?,
            value: self.expression(value_node)?,
        })
    }

    fn expression(&self, node: Node<'_>) -> Result<Expression, Diagnostics> {
        match node.kind() {
            "integer" => Ok(Expression::Literal(self.literal(node, LiteralKind::Int))),
            "float" => Ok(Expression::Literal(self.literal(node, LiteralKind::Float))),
            "string" => Ok(Expression::Literal(self.literal(node, LiteralKind::String))),
            "identifier" => Ok(Expression::Variable(self.name(node))),
            "record" => self.constructor_literal(node),
            "function_call" => self.call(node).map(Expression::Call),
            "field_access" => self.field_access(node).map(Expression::FieldAccess),
            "block" => self.block_like(node).map(Expression::Block),
            "case" => self.case(node).map(Expression::Case),
            _ => Ok(Expression::Raw(self.raw(node))),
        }
    }

    fn constructor_literal(&self, node: Node<'_>) -> Result<Expression, Diagnostics> {
        let text = self.text(node).to_string();
        let kind = match text.as_str() {
            "True" | "False" => LiteralKind::Bool,
            "Nil" => LiteralKind::Nil,
            _ => return Ok(Expression::Raw(self.raw(node))),
        };

        Ok(Expression::Literal(Literal {
            span: self.span(node),
            kind,
            source: text,
        }))
    }

    fn call(&self, node: Node<'_>) -> Result<Call, Diagnostics> {
        let function_node = node
            .child_by_field_name("function")
            .ok_or_else(|| vec![self.missing(node, "call function")])?;
        let arguments = node
            .child_by_field_name("arguments")
            .map(|arguments| self.arguments(arguments))
            .transpose()?
            .unwrap_or_default();

        Ok(Call { span: self.span(node), function: Box::new(self.expression(function_node)?), arguments })
    }

    fn arguments(&self, node: Node<'_>) -> Result<Vec<Argument>, Diagnostics> {
        self.named_children(node)
            .into_iter()
            .filter(|child| child.kind() == "argument")
            .map(|child| self.argument(child))
            .collect()
    }

    fn argument(&self, node: Node<'_>) -> Result<Argument, Diagnostics> {
        let value_node = node
            .child_by_field_name("value")
            .ok_or_else(|| vec![self.missing(node, "argument value")])?;

        Ok(Argument {
            span: self.span(node),
            label: self.name_field(node, "label")?,
            value: self.expression(value_node)?,
        })
    }

    fn field_access(&self, node: Node<'_>) -> Result<FieldAccess, Diagnostics> {
        let record_node = node
            .child_by_field_name("record")
            .ok_or_else(|| vec![self.missing(node, "field access record")])?;
        let field = self.required_name_field(node, "field")?;

        Ok(FieldAccess { span: self.span(node), record: Box::new(self.expression(record_node)?), field })
    }

    fn case(&self, node: Node<'_>) -> Result<Case, Diagnostics> {
        let subjects_node = node
            .child_by_field_name("subjects")
            .ok_or_else(|| vec![self.missing(node, "case subjects")])?;
        let clauses_node = node
            .child_by_field_name("clauses")
            .ok_or_else(|| vec![self.missing(node, "case clauses")])?;

        Ok(Case {
            span: self.span(node),
            subjects: self
                .named_children(subjects_node)
                .into_iter()
                .map(|child| self.expression(child))
                .collect::<Result<Vec<_>, _>>()?,
            clauses: self
                .named_children(clauses_node)
                .into_iter()
                .filter(|child| child.kind() == "case_clause")
                .map(|child| self.case_clause(child))
                .collect::<Result<Vec<_>, _>>()?,
        })
    }

    fn case_clause(&self, node: Node<'_>) -> Result<CaseClause, Diagnostics> {
        let patterns_node = node
            .child_by_field_name("patterns")
            .ok_or_else(|| vec![self.missing(node, "case clause patterns")])?;
        let value_node = node
            .child_by_field_name("value")
            .ok_or_else(|| vec![self.missing(node, "case clause value")])?;

        Ok(CaseClause {
            span: self.span(node),
            patterns: self
                .named_children(patterns_node)
                .into_iter()
                .map(|child| self.case_clause_pattern(child))
                .collect::<Result<Vec<_>, _>>()?,
            value: self.expression(value_node)?,
        })
    }

    fn case_clause_pattern(&self, node: Node<'_>) -> Result<Pattern, Diagnostics> {
        let child = self
            .named_children(node)
            .into_iter()
            .next()
            .ok_or_else(|| vec![self.missing(node, "case clause pattern")])?;
        self.pattern(child)
    }

    fn pattern(&self, node: Node<'_>) -> Result<Pattern, Diagnostics> {
        match node.kind() {
            "identifier" => Ok(Pattern::Name(self.name(node))),
            "discard" => Ok(Pattern::Discard(self.span(node))),
            "integer" => Ok(Pattern::Integer(self.literal(node, LiteralKind::Int))),
            "float" => Ok(Pattern::Float(self.literal(node, LiteralKind::Float))),
            "string" => Ok(Pattern::String(self.literal(node, LiteralKind::String))),
            _ => Ok(Pattern::Raw(self.raw(node))),
        }
    }

    fn type_field(&self, node: Node<'_>, field: &str) -> Result<Option<TypeAnnotation>, Diagnostics> {
        node.child_by_field_name(field)
            .map(|child| Ok(TypeAnnotation { span: self.span(child), source: self.text(child).to_string() }))
            .transpose()
    }

    fn required_name_field(&self, node: Node<'_>, field: &str) -> Result<Name, Diagnostics> {
        self.name_field(node, field)?
            .ok_or_else(|| vec![self.missing(node, field)])
    }

    fn name_field(&self, node: Node<'_>, field: &str) -> Result<Option<Name>, Diagnostics> {
        node.child_by_field_name(field)
            .map(|child| match child.kind() {
                "identifier" | "label" | "module" | "discard" => Ok(self.name(child)),
                _ => Err(vec![self.unsupported(child)]),
            })
            .transpose()
    }

    fn name(&self, node: Node<'_>) -> Name {
        Name { span: self.span(node), text: self.text(node).to_string() }
    }

    fn literal(&self, node: Node<'_>, kind: LiteralKind) -> Literal {
        Literal { span: self.span(node), kind, source: self.text(node).to_string() }
    }

    fn raw(&self, node: Node<'_>) -> RawSyntax {
        RawSyntax { span: self.span(node), kind: node.kind().into(), source: self.text(node).to_string() }
    }

    fn text(&self, node: Node<'_>) -> &str {
        node.utf8_text(self.source.text.as_bytes())
            .expect("tree-sitter node should point at valid source text")
    }

    fn span(&self, node: Node<'_>) -> Span {
        Span::new(self.source.id, node.start_byte(), node.end_byte())
    }

    fn unsupported(&self, node: Node<'_>) -> Diagnostic {
        Diagnostic::new(
            DiagnosticCode::AstError,
            format!("unsupported Gleam syntax `{}`", node.kind()),
        )
        .with_label(Label::primary(self.span(node), "unsupported syntax here"))
    }

    fn missing(&self, node: Node<'_>, expected: &str) -> Diagnostic {
        Diagnostic::new(DiagnosticCode::AstError, format!("missing {expected}"))
            .with_label(Label::primary(self.span(node), "in this syntax node"))
    }

    fn named_children<'tree>(&self, node: Node<'tree>) -> Vec<Node<'tree>> {
        let mut cursor = node.walk();
        node.children(&mut cursor).filter(|child| child.is_named()).collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::{parse, source::SourceFileId};

    use super::*;

    fn parse_ast(source: &str) -> Module {
        let source = SourceFile::new(SourceFileId(0), source);
        let cst = parse::parse(source).expect("parse source");
        build(cst).expect("build ast")
    }

    #[test]
    fn builds_ast_for_simple_module() {
        let ast = parse_ast(
            r#"import gleam/io

pub fn main() {
  let message = "hello"
  io.println(message)
}
"#,
        );

        assert_eq!(ast.imports[0].module.text, "gleam/io");
        assert_eq!(ast.functions[0].name.text, "main");
        assert!(ast.functions[0].public);
        assert_eq!(ast.functions[0].body.statements.len(), 2);
        assert_eq!(
            format!("{:#?}", ast.functions[0].body.statements[0]).lines().next(),
            Some("Let(")
        );
    }

    #[test]
    fn builds_ast_for_case_expression() {
        let ast = parse_ast("fn choose(x) { case x { 0 -> 1 _ -> 2 } }");
        let Statement::Expression(Expression::Case(case)) = &ast.functions[0].body.statements[0] else {
            panic!("expected case expression");
        };

        assert_eq!(case.subjects.len(), 1);
        assert_eq!(case.clauses.len(), 2);
    }

    #[test]
    fn reports_parse_errors() {
        let source = SourceFile::new(SourceFileId(0), "pub fn main( {");
        let diagnostics = parse::parse(source).expect_err("parse should fail");

        assert_eq!(diagnostics[0].code, DiagnosticCode::ParseError);
        assert_eq!(diagnostics[0].labels.len(), 1);
    }

    #[test]
    fn represents_top_level_gleam_syntax_as_declarations() {
        let ast = parse_ast(include_str!("../../../fixtures/ast/full_syntax.gleam"));

        assert!(matches!(ast.declarations[0], Declaration::Attribute(_)));
        assert!(matches!(ast.declarations[1], Declaration::Constant(_)));
        assert!(matches!(ast.declarations[2], Declaration::TypeDefinition(_)));
        assert!(matches!(ast.declarations[3], Declaration::TypeAlias(_)));
        assert!(matches!(ast.declarations[4], Declaration::ExternalFunction(_)));
    }

    #[test]
    fn represents_expression_and_pattern_forms_as_raw_syntax() {
        let ast = parse_ast(
            r#"fn main(items) {
  let #(first, _) = #(1, 2)
  case items {
    [head, ..tail] -> head
    _ -> first
  }
}
"#,
        );

        assert_eq!(ast.functions[0].body.statements.len(), 2);
    }
}

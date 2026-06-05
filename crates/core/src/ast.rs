pub mod bit_slices;

use tree_sitter::Node;

use crate::{
    diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, Label},
    parse::ConcreteSyntaxTree,
    source::{SourceFile, Span},
};

pub use bit_slices::{BitArray, BitArraySegment, BitArraySegmentOption};

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
    Constant(Constant),
    ExternalFunction(ExternalFunction),
    ExternalType(ExternalType),
    TypeAlias(TypeAlias),
    TypeDefinition(TypeDefinition),
    Attribute(Attribute),
    TargetGroup(TargetGroup),
    Comment(Comment),
    Statement(RawSyntax),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    pub span: Span,
    pub module: Name,
    pub alias: Option<Name>,
    pub unqualified: Vec<UnqualifiedImport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constant {
    pub span: Span,
    pub public: bool,
    pub name: Name,
    pub type_annotation: Option<TypeAnnotation>,
    pub value: Expression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalFunction {
    pub span: Span,
    pub public: bool,
    pub name: Name,
    pub parameters: Vec<Parameter>,
    pub return_type: TypeAnnotation,
    pub body: ExternalFunctionBody,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalFunctionBody {
    pub span: Span,
    pub module: Literal,
    pub function: Literal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalType {
    pub span: Span,
    pub public: bool,
    pub opaque: bool,
    pub name: Name,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeAlias {
    pub span: Span,
    pub public: bool,
    pub opaque: bool,
    pub name: Name,
    pub parameters: Vec<String>,
    pub value: TypeAnnotation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeDefinition {
    pub span: Span,
    pub public: bool,
    pub opaque: bool,
    pub name: Name,
    pub parameters: Vec<String>,
    pub constructors: Vec<DataConstructor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataConstructor {
    pub span: Span,
    pub name: Name,
    pub arguments: Vec<DataConstructorArgument>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataConstructorArgument {
    pub span: Span,
    pub label: Option<Name>,
    pub type_annotation: TypeAnnotation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    pub span: Span,
    pub name: Name,
    pub arguments: Vec<Argument>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetGroup {
    pub span: Span,
    pub target: Name,
    pub declarations: Vec<Declaration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    pub span: Span,
    pub kind: CommentKind,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommentKind {
    Module,
    Statement,
    Regular,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnqualifiedImport {
    pub span: Span,
    pub name: Name,
    pub alias: Option<Name>,
    pub kind: UnqualifiedImportKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnqualifiedImportKind {
    Value,
    TypeOrConstructor,
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
    LetAssert(LetAssert),
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
pub struct LetAssert {
    pub span: Span,
    pub pattern: Pattern,
    pub type_annotation: Option<TypeAnnotation>,
    pub value: Expression,
    pub message: Option<Expression>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pattern {
    Name(Name),
    Discard(Span),
    Integer(Literal),
    Float(Literal),
    String(Literal),
    Bool(Literal),
    Nil(Literal),
    Tuple(TuplePattern),
    List(ListPattern),
    Constructor(ConstructorPattern),
    Alias(AliasPattern),
    BitString(RawSyntax),
    Raw(RawSyntax),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuplePattern {
    pub span: Span,
    pub elements: Vec<Pattern>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListPattern {
    pub span: Span,
    pub elements: Vec<Pattern>,
    pub tail: Option<ListPatternTail>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListPatternTail {
    Name(Name),
    Discard(Span),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstructorPattern {
    pub span: Span,
    pub constructor: ConstructorName,
    pub arguments: Vec<RecordPatternArgument>,
    pub spread: Option<Span>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstructorName {
    Local(Name),
    Remote { span: Span, module: Name, name: Name },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordPatternArgument {
    pub span: Span,
    pub label: Option<Name>,
    pub pattern: Option<Pattern>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliasPattern {
    pub span: Span,
    pub pattern: Box<Pattern>,
    pub alias: Name,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expression {
    Literal(Literal),
    Variable(Name),
    Call(Call),
    FieldAccess(FieldAccess),
    Block(Block),
    Case(Case),
    BinaryOperation(BinaryOperation),
    Pipeline(Pipeline),
    UnaryOperation(UnaryOperation),
    Use(Use),
    AnonymousFunction(AnonymousFunction),
    Capture(Capture),
    Record(Record),
    RecordUpdate(RecordUpdate),
    Tuple(Tuple),
    TupleAccess(TupleAccess),
    List(List),
    BitArray(BitArray),
    Panic(FailureExpression),
    Todo(FailureExpression),
    Assert(Assert),
    Echo(Echo),
    Raw(RawSyntax),
}

impl From<&Expression> for Span {
    fn from(expression: &Expression) -> Self {
        match expression {
            Expression::Literal(literal) => literal.span,
            Expression::Variable(name) => name.span,
            Expression::Call(call) => call.span,
            Expression::FieldAccess(field_access) => field_access.span,
            Expression::Block(block) => block.span,
            Expression::Case(case) => case.span,
            Expression::BinaryOperation(operation) => operation.span,
            Expression::Pipeline(pipeline) => pipeline.span,
            Expression::UnaryOperation(operation) => operation.span,
            Expression::Use(use_) => use_.span,
            Expression::AnonymousFunction(function) => function.span,
            Expression::Capture(capture) => capture.span,
            Expression::Record(record) => record.span,
            Expression::RecordUpdate(update) => update.span,
            Expression::Tuple(tuple) => tuple.span,
            Expression::TupleAccess(access) => access.span,
            Expression::List(list) => list.span,
            Expression::BitArray(bit_array) => bit_array.span,
            Expression::Panic(panic) => panic.span,
            Expression::Todo(todo) => todo.span,
            Expression::Assert(assert) => assert.span,
            Expression::Echo(echo) => echo.span,
            Expression::Raw(raw) => raw.span,
        }
    }
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
pub struct BinaryOperation {
    pub span: Span,
    pub left: Box<Expression>,
    pub operator: BinaryOperator,
    pub right: Box<Expression>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    FloatAdd,
    FloatSubtract,
    FloatMultiply,
    FloatDivide,
    Equal,
    NotEqual,
    LessThan,
    LessThanEqual,
    GreaterThan,
    GreaterThanEqual,
    FloatLessThan,
    FloatLessThanEqual,
    FloatGreaterThan,
    FloatGreaterThanEqual,
    And,
    Or,
    StringConcat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pipeline {
    pub span: Span,
    pub value: Box<Expression>,
    pub into: Box<Expression>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnaryOperation {
    pub span: Span,
    pub operator: UnaryOperator,
    pub value: Box<Expression>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnaryOperator {
    BooleanNot,
    IntegerNegate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Use {
    pub span: Span,
    pub assignments: Vec<UseAssignment>,
    pub value: Box<Expression>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UseAssignment {
    pub span: Span,
    pub pattern: Pattern,
    pub type_annotation: Option<TypeAnnotation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnonymousFunction {
    pub span: Span,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<TypeAnnotation>,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capture {
    pub span: Span,
    pub function: Box<Expression>,
    pub arguments: Vec<Option<Argument>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub span: Span,
    pub constructor: ConstructorName,
    pub arguments: Vec<Argument>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordUpdate {
    pub span: Span,
    pub constructor: ConstructorName,
    pub spread: Box<Expression>,
    pub updates: Vec<Argument>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tuple {
    pub span: Span,
    pub elements: Vec<Expression>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TupleAccess {
    pub span: Span,
    pub tuple: Box<Expression>,
    pub index: Name,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct List {
    pub span: Span,
    pub elements: Vec<Expression>,
    pub spread: Option<Box<Expression>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureExpression {
    pub span: Span,
    pub message: Option<Box<Expression>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assert {
    pub span: Span,
    pub pattern: Pattern,
    pub type_annotation: Option<TypeAnnotation>,
    pub value: Box<Expression>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Echo {
    pub span: Span,
    pub value: Box<Expression>,
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
    pub guard: Option<Expression>,
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

pub fn build(cst: &ConcreteSyntaxTree) -> Result<Module, Diagnostics> {
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
            "constant" => self.constant(node).map(Declaration::Constant),
            "external_function" => self.external_function(node).map(Declaration::ExternalFunction),
            "external_type" => self.external_type(node).map(Declaration::ExternalType),
            "type_alias" => self.type_alias(node).map(Declaration::TypeAlias),
            "type_definition" => self.type_definition(node).map(Declaration::TypeDefinition),
            "attribute" => self.attribute(node).map(Declaration::Attribute),
            "target_group" => self.target_group(node).map(Declaration::TargetGroup),
            "module_comment" | "statement_comment" | "comment" => Ok(Declaration::Comment(self.comment(node))),
            _ => Ok(Declaration::Statement(self.raw(node))),
        }
    }

    fn constant(&self, node: Node<'_>) -> Result<Constant, Diagnostics> {
        let value_node = node
            .child_by_field_name("value")
            .ok_or_else(|| vec![self.missing(node, "constant value")])?;
        Ok(Constant {
            span: self.span(node),
            public: self.has_child_kind(node, "visibility_modifier"),
            name: self.required_name_field(node, "name")?,
            type_annotation: self.type_field(node, "type")?,
            value: self.expression(value_node)?,
        })
    }

    fn external_function(&self, node: Node<'_>) -> Result<ExternalFunction, Diagnostics> {
        let return_type = self
            .type_field(node, "return_type")?
            .ok_or_else(|| vec![self.missing(node, "external function return type")])?;
        let body_node = node
            .child_by_field_name("body")
            .ok_or_else(|| vec![self.missing(node, "external function body")])?;
        Ok(ExternalFunction {
            span: self.span(node),
            public: self.has_child_kind(node, "visibility_modifier"),
            name: self.required_name_field(node, "name")?,
            parameters: node
                .child_by_field_name("parameters")
                .map(|parameters| self.parameters(parameters))
                .transpose()?
                .unwrap_or_default(),
            return_type,
            body: self.external_function_body(body_node)?,
        })
    }

    fn external_function_body(&self, node: Node<'_>) -> Result<ExternalFunctionBody, Diagnostics> {
        let strings = self
            .named_children(node)
            .into_iter()
            .filter(|child| child.kind() == "string")
            .map(|child| self.literal(child, LiteralKind::String))
            .collect::<Vec<_>>();
        let module = strings
            .first()
            .cloned()
            .ok_or_else(|| vec![self.missing(node, "external module")])?;
        let function = strings
            .get(1)
            .cloned()
            .ok_or_else(|| vec![self.missing(node, "external function")])?;
        Ok(ExternalFunctionBody { span: self.span(node), module, function })
    }

    fn external_type(&self, node: Node<'_>) -> Result<ExternalType, Diagnostics> {
        Ok(ExternalType {
            span: self.span(node),
            public: self.has_child_kind(node, "visibility_modifier"),
            opaque: self.has_child_kind(node, "opacity_modifier"),
            name: self.required_named_child_as_name(node, "type_name")?,
        })
    }

    fn type_alias(&self, node: Node<'_>) -> Result<TypeAlias, Diagnostics> {
        let value_node = self
            .named_children(node)
            .into_iter()
            .find(|child| is_type_node(child.kind()) && child.kind() != "type_name")
            .ok_or_else(|| vec![self.missing(node, "type alias value")])?;
        let name_node = self.required_named_child(node, "type_name")?;
        Ok(TypeAlias {
            span: self.span(node),
            public: self.has_child_kind(node, "visibility_modifier"),
            opaque: self.has_child_kind(node, "opacity_modifier"),
            name: self.type_decl_name(name_node),
            parameters: self.type_decl_parameters(name_node),
            value: TypeAnnotation { span: self.span(value_node), source: self.text(value_node).to_string() },
        })
    }

    fn type_definition(&self, node: Node<'_>) -> Result<TypeDefinition, Diagnostics> {
        let constructors = self
            .named_children(node)
            .into_iter()
            .find(|child| child.kind() == "data_constructors")
            .map(|child| self.data_constructors(child))
            .transpose()?
            .unwrap_or_default();
        let name_node = self.required_named_child(node, "type_name")?;
        Ok(TypeDefinition {
            span: self.span(node),
            public: self.has_child_kind(node, "visibility_modifier"),
            opaque: self.has_child_kind(node, "opacity_modifier"),
            name: self.type_decl_name(name_node),
            parameters: self.type_decl_parameters(name_node),
            constructors,
        })
    }

    fn data_constructors(&self, node: Node<'_>) -> Result<Vec<DataConstructor>, Diagnostics> {
        self.named_children(node)
            .into_iter()
            .filter(|child| child.kind() == "data_constructor")
            .map(|child| self.data_constructor(child))
            .collect()
    }

    fn data_constructor(&self, node: Node<'_>) -> Result<DataConstructor, Diagnostics> {
        let arguments = node
            .child_by_field_name("arguments")
            .map(|arguments| self.data_constructor_arguments(arguments))
            .transpose()?
            .unwrap_or_default();
        Ok(DataConstructor { span: self.span(node), name: self.required_name_field(node, "name")?, arguments })
    }

    fn data_constructor_arguments(&self, node: Node<'_>) -> Result<Vec<DataConstructorArgument>, Diagnostics> {
        self.named_children(node)
            .into_iter()
            .filter(|child| child.kind() == "data_constructor_argument")
            .map(|child| {
                let value = child
                    .child_by_field_name("value")
                    .ok_or_else(|| vec![self.missing(child, "constructor argument type")])?;
                Ok(DataConstructorArgument {
                    span: self.span(child),
                    label: self.name_field(child, "label")?,
                    type_annotation: TypeAnnotation { span: self.span(value), source: self.text(value).to_string() },
                })
            })
            .collect()
    }

    fn attribute(&self, node: Node<'_>) -> Result<Attribute, Diagnostics> {
        Ok(Attribute {
            span: self.span(node),
            name: self.required_name_field(node, "name")?,
            arguments: node
                .child_by_field_name("arguments")
                .map(|arguments| self.arguments(arguments))
                .transpose()?
                .unwrap_or_default(),
        })
    }

    fn target_group(&self, node: Node<'_>) -> Result<TargetGroup, Diagnostics> {
        let declarations = self
            .named_children(node)
            .into_iter()
            .filter(|child| child.kind() != "target")
            .map(|child| self.declaration(child))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(TargetGroup { span: self.span(node), target: self.required_name_field(node, "target")?, declarations })
    }

    fn comment(&self, node: Node<'_>) -> Comment {
        let kind = match node.kind() {
            "module_comment" => CommentKind::Module,
            "statement_comment" => CommentKind::Statement,
            _ => CommentKind::Regular,
        };
        Comment { span: self.span(node), kind, text: self.text(node).to_string() }
    }

    fn import(&self, node: Node<'_>) -> Result<Import, Diagnostics> {
        let module = self.required_name_field(node, "module")?;
        let alias = self.name_field(node, "alias")?;
        let unqualified = node
            .child_by_field_name("imports")
            .map(|imports| self.unqualified_imports(imports))
            .transpose()?
            .unwrap_or_default();

        Ok(Import { span: self.span(node), module, alias, unqualified })
    }

    fn unqualified_imports(&self, node: Node<'_>) -> Result<Vec<UnqualifiedImport>, Diagnostics> {
        self.named_children(node)
            .into_iter()
            .filter(|child| child.kind() == "unqualified_import")
            .map(|child| self.unqualified_import(child))
            .collect()
    }

    fn unqualified_import(&self, node: Node<'_>) -> Result<UnqualifiedImport, Diagnostics> {
        let name_node = node
            .child_by_field_name("name")
            .ok_or_else(|| vec![self.missing(node, "unqualified import name")])?;
        let name = self.name(name_node);
        let alias = self.name_field(node, "alias")?;
        let kind = match name_node.kind() {
            "identifier" => UnqualifiedImportKind::Value,
            "type_identifier" => UnqualifiedImportKind::TypeOrConstructor,
            _ => return Err(vec![self.unsupported(name_node)]),
        };

        Ok(UnqualifiedImport { span: self.span(node), name, alias, kind })
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
            "let_assert" => Ok(Statement::LetAssert(self.let_assert_statement(node)?)),
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

    fn let_assert_statement(&self, node: Node<'_>) -> Result<LetAssert, Diagnostics> {
        let pattern_node = node
            .child_by_field_name("pattern")
            .ok_or_else(|| vec![self.missing(node, "let assert pattern")])?;
        let value_node = node
            .child_by_field_name("value")
            .ok_or_else(|| vec![self.missing(node, "let assert value")])?;
        let message = node
            .child_by_field_name("message")
            .map(|message| self.expression(message))
            .transpose()?;

        Ok(LetAssert {
            span: self.span(node),
            pattern: self.pattern(pattern_node)?,
            type_annotation: self.type_field(node, "type")?,
            value: self.expression(value_node)?,
            message,
        })
    }

    fn expression(&self, node: Node<'_>) -> Result<Expression, Diagnostics> {
        match node.kind() {
            "integer" => Ok(Expression::Literal(self.literal(node, LiteralKind::Int))),
            "float" => Ok(Expression::Literal(self.literal(node, LiteralKind::Float))),
            "string" => Ok(Expression::Literal(self.literal(node, LiteralKind::String))),
            "identifier" => Ok(Expression::Variable(self.name(node))),
            "record" => self.record(node),
            "function_call" => self.call_or_capture(node),
            "field_access" => self.field_access(node).map(Expression::FieldAccess),
            "block" => self.block_like(node).map(Expression::Block),
            "case" => self.case(node).map(Expression::Case),
            "binary_expression" => self.binary_expression(node),
            "boolean_negation" => self.unary_expression(node, UnaryOperator::BooleanNot),
            "integer_negation" => self.unary_expression(node, UnaryOperator::IntegerNegate),
            "use" => self.use_expression(node).map(Expression::Use),
            "anonymous_function" => self.anonymous_function(node).map(Expression::AnonymousFunction),
            "record_update" => self.record_update(node).map(Expression::RecordUpdate),
            "tuple" => self.tuple(node).map(Expression::Tuple),
            "tuple_access" => self.tuple_access(node).map(Expression::TupleAccess),
            "list" => self.list(node).map(Expression::List),
            "bit_string" => self.bit_array(node).map(Expression::BitArray),
            "panic" => self.failure_expression(node).map(Expression::Panic),
            "todo" => self.failure_expression(node).map(Expression::Todo),
            "assert" => self.assert_expression(node).map(Expression::Assert),
            "echo" | "pipeline_echo" => self.echo_expression(node).map(Expression::Echo),
            _ => Ok(Expression::Raw(self.raw(node))),
        }
    }

    fn record(&self, node: Node<'_>) -> Result<Expression, Diagnostics> {
        let text = self.text(node).to_string();
        let kind = match text.as_str() {
            "True" | "False" => Some(LiteralKind::Bool),
            "Nil" => Some(LiteralKind::Nil),
            _ => None,
        };
        if let Some(kind) = kind {
            return Ok(Expression::Literal(Literal {
                span: self.span(node),
                kind,
                source: text,
            }));
        }

        let name_node = node
            .child_by_field_name("name")
            .ok_or_else(|| vec![self.missing(node, "record name")])?;
        let arguments = node
            .child_by_field_name("arguments")
            .map(|arguments| self.arguments(arguments))
            .transpose()?
            .unwrap_or_default();
        Ok(Expression::Record(Record {
            span: self.span(node),
            constructor: self.constructor_name(name_node)?,
            arguments,
        }))
    }

    fn call_or_capture(&self, node: Node<'_>) -> Result<Expression, Diagnostics> {
        let call = self.call(node)?;
        if !call.arguments.iter().any(is_capture_hole) {
            return Ok(Expression::Call(call));
        }

        let arguments = call
            .arguments
            .into_iter()
            .map(|argument| if is_capture_hole(&argument) { None } else { Some(argument) })
            .collect();
        Ok(Expression::Capture(Capture {
            span: call.span,
            function: call.function,
            arguments,
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

    fn binary_expression(&self, node: Node<'_>) -> Result<Expression, Diagnostics> {
        let left = node
            .child_by_field_name("left")
            .ok_or_else(|| vec![self.missing(node, "left operand")])?;
        let right = node
            .child_by_field_name("right")
            .ok_or_else(|| vec![self.missing(node, "right operand")])?;
        let operator = node
            .child_by_field_name("operator")
            .ok_or_else(|| vec![self.missing(node, "operator")])?;
        if self.text(operator) == "|>" {
            return Ok(Expression::Pipeline(Pipeline {
                span: self.span(node),
                value: Box::new(self.expression(left)?),
                into: Box::new(self.expression(right)?),
            }));
        }
        Ok(Expression::BinaryOperation(BinaryOperation {
            span: self.span(node),
            left: Box::new(self.expression(left)?),
            operator: self.binary_operator(operator)?,
            right: Box::new(self.expression(right)?),
        }))
    }

    fn binary_operator(&self, node: Node<'_>) -> Result<BinaryOperator, Diagnostics> {
        match self.text(node) {
            "+" => Ok(BinaryOperator::Add),
            "-" => Ok(BinaryOperator::Subtract),
            "*" => Ok(BinaryOperator::Multiply),
            "/" => Ok(BinaryOperator::Divide),
            "%" => Ok(BinaryOperator::Remainder),
            "+." => Ok(BinaryOperator::FloatAdd),
            "-." => Ok(BinaryOperator::FloatSubtract),
            "*." => Ok(BinaryOperator::FloatMultiply),
            "/." => Ok(BinaryOperator::FloatDivide),
            "==" => Ok(BinaryOperator::Equal),
            "!=" => Ok(BinaryOperator::NotEqual),
            "<" => Ok(BinaryOperator::LessThan),
            "<=" => Ok(BinaryOperator::LessThanEqual),
            ">" => Ok(BinaryOperator::GreaterThan),
            ">=" => Ok(BinaryOperator::GreaterThanEqual),
            "<." => Ok(BinaryOperator::FloatLessThan),
            "<=." => Ok(BinaryOperator::FloatLessThanEqual),
            ">." => Ok(BinaryOperator::FloatGreaterThan),
            ">=." => Ok(BinaryOperator::FloatGreaterThanEqual),
            "&&" => Ok(BinaryOperator::And),
            "||" => Ok(BinaryOperator::Or),
            "<>" => Ok(BinaryOperator::StringConcat),
            _ => Err(vec![self.unsupported(node)]),
        }
    }

    fn unary_expression(&self, node: Node<'_>, operator: UnaryOperator) -> Result<Expression, Diagnostics> {
        let value = self
            .named_children(node)
            .into_iter()
            .next()
            .ok_or_else(|| vec![self.missing(node, "unary operand")])?;
        Ok(Expression::UnaryOperation(UnaryOperation {
            span: self.span(node),
            operator,
            value: Box::new(self.expression(value)?),
        }))
    }

    fn use_expression(&self, node: Node<'_>) -> Result<Use, Diagnostics> {
        let assignments = node
            .child_by_field_name("assignments")
            .map(|assignments| self.use_assignments(assignments))
            .transpose()?
            .unwrap_or_default();
        let value = node
            .child_by_field_name("value")
            .ok_or_else(|| vec![self.missing(node, "use value")])?;
        Ok(Use { span: self.span(node), assignments, value: Box::new(self.expression(value)?) })
    }

    fn use_assignments(&self, node: Node<'_>) -> Result<Vec<UseAssignment>, Diagnostics> {
        self.named_children(node)
            .into_iter()
            .filter(|child| child.kind() == "use_assignment")
            .map(|child| {
                let pattern = child
                    .child_by_field_name("pattern")
                    .or_else(|| self.named_children(child).into_iter().next())
                    .ok_or_else(|| vec![self.missing(child, "use pattern")])?;
                Ok(UseAssignment {
                    span: self.span(child),
                    pattern: self.pattern(pattern)?,
                    type_annotation: self.type_field(child, "type")?,
                })
            })
            .collect()
    }

    fn anonymous_function(&self, node: Node<'_>) -> Result<AnonymousFunction, Diagnostics> {
        let body = node
            .child_by_field_name("body")
            .ok_or_else(|| vec![self.missing(node, "anonymous function body")])?;
        Ok(AnonymousFunction {
            span: self.span(node),
            parameters: node
                .child_by_field_name("parameters")
                .map(|parameters| self.parameters(parameters))
                .transpose()?
                .unwrap_or_default(),
            return_type: self.type_field(node, "return_type")?,
            body: self.block_like(body)?,
        })
    }

    fn record_update(&self, node: Node<'_>) -> Result<RecordUpdate, Diagnostics> {
        let constructor = node
            .child_by_field_name("constructor")
            .ok_or_else(|| vec![self.missing(node, "record update constructor")])?;
        let spread = node
            .child_by_field_name("spread")
            .ok_or_else(|| vec![self.missing(node, "record update spread")])?;
        let updates = node
            .child_by_field_name("arguments")
            .map(|arguments| self.record_update_arguments(arguments))
            .transpose()?
            .unwrap_or_default();
        Ok(RecordUpdate {
            span: self.span(node),
            constructor: self.constructor_name(constructor)?,
            spread: Box::new(self.expression(spread)?),
            updates,
        })
    }

    fn record_update_arguments(&self, node: Node<'_>) -> Result<Vec<Argument>, Diagnostics> {
        self.named_children(node)
            .into_iter()
            .filter(|child| child.kind() == "record_update_argument")
            .map(|child| self.argument(child))
            .collect()
    }

    fn tuple(&self, node: Node<'_>) -> Result<Tuple, Diagnostics> {
        Ok(Tuple { span: self.span(node), elements: self.expression_children(node)? })
    }

    fn tuple_access(&self, node: Node<'_>) -> Result<TupleAccess, Diagnostics> {
        let tuple = node
            .child_by_field_name("tuple")
            .or_else(|| self.named_children(node).into_iter().next())
            .ok_or_else(|| vec![self.missing(node, "tuple value")])?;
        let index = node
            .child_by_field_name("index")
            .or_else(|| {
                self.named_children(node)
                    .into_iter()
                    .find(|child| child.kind() == "integer")
            })
            .ok_or_else(|| vec![self.missing(node, "tuple index")])?;
        Ok(TupleAccess { span: self.span(node), tuple: Box::new(self.expression(tuple)?), index: self.name(index) })
    }

    fn list(&self, node: Node<'_>) -> Result<List, Diagnostics> {
        let spread = node
            .child_by_field_name("spread")
            .map(|spread| self.expression(spread).map(Box::new))
            .transpose()?;
        let spread_span = spread.as_ref().map(|spread| Span::from(spread.as_ref()));
        let elements = self
            .named_children(node)
            .into_iter()
            .filter(|child| Some(self.span(*child)) != spread_span)
            .map(|child| self.expression(child))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(List { span: self.span(node), elements, spread })
    }

    fn failure_expression(&self, node: Node<'_>) -> Result<FailureExpression, Diagnostics> {
        Ok(FailureExpression {
            span: self.span(node),
            message: node
                .child_by_field_name("message")
                .map(|message| self.expression(message).map(Box::new))
                .transpose()?,
        })
    }

    fn assert_expression(&self, node: Node<'_>) -> Result<Assert, Diagnostics> {
        let pattern = node
            .child_by_field_name("pattern")
            .ok_or_else(|| vec![self.missing(node, "assert pattern")])?;
        let value = node
            .child_by_field_name("value")
            .ok_or_else(|| vec![self.missing(node, "assert value")])?;
        Ok(Assert {
            span: self.span(node),
            pattern: self.pattern(pattern)?,
            type_annotation: self.type_field(node, "type")?,
            value: Box::new(self.expression(value)?),
        })
    }

    fn echo_expression(&self, node: Node<'_>) -> Result<Echo, Diagnostics> {
        let value = self
            .named_children(node)
            .into_iter()
            .next()
            .ok_or_else(|| vec![self.missing(node, "echo value")])?;
        Ok(Echo { span: self.span(node), value: Box::new(self.expression(value)?) })
    }

    fn expression_children(&self, node: Node<'_>) -> Result<Vec<Expression>, Diagnostics> {
        self.named_children(node)
            .into_iter()
            .map(|child| self.expression(child))
            .collect()
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
        let guard = node
            .child_by_field_name("guard")
            .and_then(|guard| self.named_children(guard).into_iter().find(|child| child.is_named()))
            .map(|guard| self.expression(guard))
            .transpose()?;

        Ok(CaseClause {
            span: self.span(node),
            patterns: self.case_clause_patterns(patterns_node)?,
            guard,
            value: self.expression(value_node)?,
        })
    }

    fn case_clause_patterns(&self, node: Node<'_>) -> Result<Vec<Pattern>, Diagnostics> {
        let mut patterns = Vec::new();
        for child in self.named_children(node) {
            if child.kind() == "case_clause_pattern" {
                patterns.extend(self.case_clause_pattern(child)?);
            }
        }
        Ok(patterns)
    }

    fn case_clause_pattern(&self, node: Node<'_>) -> Result<Vec<Pattern>, Diagnostics> {
        let children = self
            .named_children(node)
            .into_iter()
            .filter(|child| child.kind() != "list_pattern_tail" && child.kind() != "pattern_spread")
            .collect::<Vec<_>>();
        self.pattern_sequence(&children)
    }

    fn pattern(&self, node: Node<'_>) -> Result<Pattern, Diagnostics> {
        let pattern = match node.kind() {
            "identifier" => Pattern::Name(self.name(node)),
            "discard" => Pattern::Discard(self.span(node)),
            "integer" => Pattern::Integer(self.literal(node, LiteralKind::Int)),
            "float" => Pattern::Float(self.literal(node, LiteralKind::Float)),
            "string" => Pattern::String(self.literal(node, LiteralKind::String)),
            "tuple_pattern" => {
                Pattern::Tuple(TuplePattern { span: self.span(node), elements: self.pattern_children(node)? })
            }
            "list_pattern" => self.list_pattern(node)?,
            "record" | "record_pattern" => self.constructor_pattern(node)?,
            "bit_string_pattern" => Pattern::BitString(self.raw(node)),
            _ => Pattern::Raw(self.raw(node)),
        };

        if let Some(assign) = node.child_by_field_name("assign") {
            let alias = self
                .named_children(assign)
                .into_iter()
                .find(|child| child.kind() == "identifier")
                .map(|child| self.name(child))
                .ok_or_else(|| vec![self.missing(assign, "pattern alias")])?;
            Ok(Pattern::Alias(AliasPattern {
                span: self.span(node),
                pattern: Box::new(pattern),
                alias,
            }))
        } else {
            Ok(pattern)
        }
    }

    fn pattern_children(&self, node: Node<'_>) -> Result<Vec<Pattern>, Diagnostics> {
        let children = self
            .named_children(node)
            .into_iter()
            .filter(|child| child.kind() != "list_pattern_tail" && child.kind() != "pattern_spread")
            .collect::<Vec<_>>();
        self.pattern_sequence(&children)
    }

    fn pattern_sequence(&self, children: &[Node<'_>]) -> Result<Vec<Pattern>, Diagnostics> {
        let mut patterns = Vec::new();
        let mut index = 0;
        while index < children.len() {
            let child = children[index];
            let pattern = self.pattern(child)?;
            if let Some(alias_node) = children.get(index + 1).copied()
                && alias_node.kind() == "identifier"
                && self
                    .text_between(child.end_byte(), alias_node.start_byte())
                    .contains("as")
            {
                patterns.push(Pattern::Alias(AliasPattern {
                    span: Span::new(self.source.id, child.start_byte(), alias_node.end_byte()),
                    pattern: Box::new(pattern),
                    alias: self.name(alias_node),
                }));
                index += 2;
            } else {
                patterns.push(pattern);
                index += 1;
            }
        }
        Ok(patterns)
    }

    fn list_pattern(&self, node: Node<'_>) -> Result<Pattern, Diagnostics> {
        let elements = self.pattern_children(node)?;
        let tail = self
            .named_children(node)
            .into_iter()
            .find(|child| child.kind() == "list_pattern_tail")
            .map(|tail| self.list_pattern_tail(tail))
            .transpose()?;

        Ok(Pattern::List(ListPattern { span: self.span(node), elements, tail }))
    }

    fn list_pattern_tail(&self, node: Node<'_>) -> Result<ListPatternTail, Diagnostics> {
        let Some(child) = self.named_children(node).into_iter().next() else {
            return Ok(ListPatternTail::Discard(self.span(node)));
        };
        match child.kind() {
            "identifier" => Ok(ListPatternTail::Name(self.name(child))),
            "discard" => Ok(ListPatternTail::Discard(self.span(child))),
            _ => Err(vec![self.unsupported(child)]),
        }
    }

    fn constructor_pattern(&self, node: Node<'_>) -> Result<Pattern, Diagnostics> {
        match self.text(node) {
            "True" | "False" => {
                return Ok(Pattern::Bool(Literal {
                    span: self.span(node),
                    kind: LiteralKind::Bool,
                    source: self.text(node).to_string(),
                }));
            }
            "Nil" => {
                return Ok(Pattern::Nil(Literal {
                    span: self.span(node),
                    kind: LiteralKind::Nil,
                    source: self.text(node).to_string(),
                }));
            }
            _ => {}
        }

        let name_node = node
            .child_by_field_name("name")
            .ok_or_else(|| vec![self.missing(node, "constructor pattern name")])?;
        let constructor = self.constructor_name(name_node)?;
        let arguments_node = node.child_by_field_name("arguments");
        let arguments = arguments_node
            .map(|arguments| self.record_pattern_arguments(arguments))
            .transpose()?
            .unwrap_or_default();
        let spread = arguments_node
            .and_then(|arguments| {
                self.named_children(arguments)
                    .into_iter()
                    .find(|child| child.kind() == "pattern_spread")
            })
            .map(|spread| self.span(spread));

        Ok(Pattern::Constructor(ConstructorPattern {
            span: self.span(node),
            constructor,
            arguments,
            spread,
        }))
    }

    fn constructor_name(&self, node: Node<'_>) -> Result<ConstructorName, Diagnostics> {
        match node.kind() {
            "constructor_name" => Ok(ConstructorName::Local(self.name(node))),
            "remote_constructor_name" => {
                let module = self.required_name_field(node, "module")?;
                let name = self.required_name_field(node, "name")?;
                Ok(ConstructorName::Remote { span: self.span(node), module, name })
            }
            _ => Err(vec![self.unsupported(node)]),
        }
    }

    fn record_pattern_arguments(&self, node: Node<'_>) -> Result<Vec<RecordPatternArgument>, Diagnostics> {
        self.named_children(node)
            .into_iter()
            .filter(|child| child.kind() == "record_pattern_argument")
            .map(|child| self.record_pattern_argument(child))
            .collect()
    }

    fn record_pattern_argument(&self, node: Node<'_>) -> Result<RecordPatternArgument, Diagnostics> {
        Ok(RecordPatternArgument {
            span: self.span(node),
            label: self.name_field(node, "label")?,
            pattern: node
                .child_by_field_name("pattern")
                .map(|pattern| self.pattern(pattern))
                .transpose()?,
        })
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
                "identifier" | "type_identifier" | "constructor_name" | "label" | "module" | "discard"
                | "type_name" | "target" | "integer" => Ok(self.name(child)),
                _ => Err(vec![self.unsupported(child)]),
            })
            .transpose()
    }

    fn required_named_child<'tree>(&self, node: Node<'tree>, kind: &str) -> Result<Node<'tree>, Diagnostics> {
        self.named_children(node)
            .into_iter()
            .find(|child| child.kind() == kind)
            .ok_or_else(|| vec![self.missing(node, kind)])
    }

    fn required_named_child_as_name(&self, node: Node<'_>, kind: &str) -> Result<Name, Diagnostics> {
        self.required_named_child(node, kind).map(|child| self.name(child))
    }

    fn type_decl_name(&self, node: Node<'_>) -> Name {
        let text = self.text(node);
        let end = text.find('(').unwrap_or(text.len());
        Name {
            span: Span::new(self.source.id, node.start_byte(), node.start_byte() + end),
            text: text[..end].to_string(),
        }
    }

    fn type_decl_parameters(&self, node: Node<'_>) -> Vec<String> {
        self.text(node)
            .split_once('(')
            .and_then(|(_, rest)| rest.split_once(')').map(|(params, _)| params))
            .map(|params| {
                params
                    .split(',')
                    .map(str::trim)
                    .filter(|param| !param.is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default()
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

    fn text_between(&self, start: usize, end: usize) -> &str {
        &self.source.text[start..end]
    }

    fn span(&self, node: Node<'_>) -> Span {
        Span::new(self.source.id, node.start_byte(), node.end_byte())
    }

    fn has_child_kind(&self, node: Node<'_>, kind: &str) -> bool {
        self.named_children(node).into_iter().any(|child| child.kind() == kind)
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

fn is_capture_hole(argument: &Argument) -> bool {
    matches!(&argument.value, Expression::Variable(Name { text, .. }) if text == "_")
        || matches!(&argument.value, Expression::Raw(raw) if raw.kind == "hole" && raw.source == "_")
}

fn is_type_node(kind: &str) -> bool {
    matches!(
        kind,
        "function_type" | "tuple_type" | "type" | "type_hole" | "type_name" | "type_var"
    )
}

#[cfg(test)]
mod tests {
    use crate::{parse, source::SourceFileId};

    use super::*;

    fn parse_ast(source: &str) -> Module {
        let source = SourceFile::new(SourceFileId(0), source);
        let cst = parse::parse(source).expect("parse source");
        build(&cst).expect("build ast")
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
    fn detects_capture_holes_from_argument_nodes_only() {
        let ast = parse_ast("fn main(foo_bar) { add(foo_bar) add(_) }");
        let Statement::Expression(Expression::Call(_)) = &ast.functions[0].body.statements[0] else {
            panic!("identifier underscores should not create captures");
        };
        let Statement::Expression(Expression::Capture(capture)) = &ast.functions[0].body.statements[1] else {
            panic!("discard argument should create a capture");
        };

        assert_eq!(capture.arguments, vec![None]);
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

    #[test]
    fn represents_pattern_matching_syntax() {
        let ast = parse_ast(include_str!("../../../fixtures/ast/pattern_matching.gleam"));

        insta::assert_debug_snapshot!(ast.functions[0].body.statements);
    }
}

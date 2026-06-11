pub mod bit_slices;
mod closure;
mod lowerer;

use std::collections::HashSet;

use crate::{
    ast::{self, Declaration as AstDeclaration, LiteralKind},
    diagnostic::{Diagnostic, DiagnosticCode, Diagnostics},
    resolve::SymbolKind,
    source::Span,
    stdlib,
    types::{Type, TypedModule, TypedProject},
};

pub use bit_slices::{BitArrayLiteral, BitArraySegment, BitSegmentOption, BitSegmentType, BitStringPatternSegment};
pub use lowerer::{FunctionContext, Lowerer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepresentationType {
    Scalar(ScalarRepresentation),
    HeapManaged(HeapRepresentation),
}

impl From<&Type> for RepresentationType {
    fn from(type_: &Type) -> Self {
        match type_ {
            Type::Int => Self::Scalar(ScalarRepresentation::I64),
            Type::Float => Self::Scalar(ScalarRepresentation::F64),
            Type::Bool => Self::Scalar(ScalarRepresentation::I32),
            Type::Nil => Self::Scalar(ScalarRepresentation::Unit),
            Type::String => Self::HeapManaged(HeapRepresentation::String),
            Type::BitArray => Self::HeapManaged(HeapRepresentation::BitArray),
            Type::Tuple(_) => Self::HeapManaged(HeapRepresentation::Tuple),
            Type::List(_) => Self::HeapManaged(HeapRepresentation::List),
            Type::Record { .. } => Self::HeapManaged(HeapRepresentation::Record),
            Type::Custom { .. } => Self::HeapManaged(HeapRepresentation::Custom),
            Type::Function { .. } => Self::HeapManaged(HeapRepresentation::Function),
            Type::Generic(_) | Type::Opaque { .. } => Self::HeapManaged(HeapRepresentation::Opaque),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScalarRepresentation {
    I64,
    F64,
    I32,
    Unit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeapRepresentation {
    String,
    BitArray,
    Tuple,
    List,
    Record,
    Custom,
    Function,
    Opaque,
}

/// Core IR module.
///
/// This is the first compiler-owned representation after parsing, name
/// resolution, and type checking. It keeps module-level structure explicit so
/// later backends do not need to inspect parser-specific declarations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    pub span: Span,
    pub imports: Vec<Import>,
    pub declarations: Vec<DeclarationMetadata>,
    pub constants: Vec<Constant>,
    pub init: ModuleInit,
    pub references: Vec<Reference>,
    pub exports: Vec<Export>,
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConstantId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    pub module: String,
    pub alias: Option<String>,
    pub unqualified: Vec<UnqualifiedImport>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnqualifiedImport {
    pub name: String,
    pub alias: Option<String>,
    pub kind: ImportKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportKind {
    Value,
    TypeOrConstructor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclarationMetadata {
    pub name: Option<String>,
    pub kind: DeclarationKind,
    pub visibility: Visibility,
    pub span: Span,
}

impl From<&AstDeclaration> for DeclarationMetadata {
    fn from(declaration: &AstDeclaration) -> Self {
        match declaration {
            AstDeclaration::Import(import) => Self {
                name: Some(import.module.text.clone()),
                kind: DeclarationKind::Import,
                visibility: Visibility::Private,
                span: import.span,
            },
            AstDeclaration::Function(function) => Self {
                name: Some(function.name.text.clone()),
                kind: DeclarationKind::Function,
                visibility: visibility(function.public),
                span: function.span,
            },
            AstDeclaration::Constant(constant) => Self {
                name: Some(constant.name.text.clone()),
                kind: DeclarationKind::Constant,
                visibility: visibility(constant.public),
                span: constant.span,
            },
            AstDeclaration::ExternalFunction(function) => Self {
                name: Some(function.name.text.clone()),
                kind: DeclarationKind::ExternalFunction,
                visibility: visibility(function.public),
                span: function.span,
            },
            AstDeclaration::ExternalType(type_) => Self {
                name: Some(type_.name.text.clone()),
                kind: DeclarationKind::ExternalType,
                visibility: visibility(type_.public),
                span: type_.span,
            },
            AstDeclaration::TypeAlias(alias) => Self {
                name: Some(alias.name.text.clone()),
                kind: DeclarationKind::TypeAlias,
                visibility: visibility(alias.public),
                span: alias.span,
            },
            AstDeclaration::TypeDefinition(type_) => Self {
                name: Some(type_.name.text.clone()),
                kind: DeclarationKind::TypeDefinition,
                visibility: visibility(type_.public),
                span: type_.span,
            },
            AstDeclaration::Attribute(attribute) => Self {
                name: Some(attribute.name.text.clone()),
                kind: DeclarationKind::Attribute,
                visibility: Visibility::Private,
                span: attribute.span,
            },
            AstDeclaration::TargetGroup(group) => Self {
                name: Some(group.target.text.clone()),
                kind: DeclarationKind::TargetGroup,
                visibility: Visibility::Private,
                span: group.span,
            },
            AstDeclaration::Comment(comment) => Self {
                name: None,
                kind: DeclarationKind::Statement,
                visibility: Visibility::Private,
                span: comment.span,
            },
            AstDeclaration::Statement(raw) => raw_metadata(raw, DeclarationKind::Statement, ""),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeclarationKind {
    Import,
    Function,
    Constant,
    ExternalFunction,
    ExternalType,
    TypeAlias,
    TypeDefinition,
    Attribute,
    TargetGroup,
    Statement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Private,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constant {
    pub id: ConstantId,
    pub name: String,
    pub public: bool,
    pub value: ConstantValue,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstantValue {
    Literal(Literal),
    Raw(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModuleInit {
    pub steps: Vec<InitStep>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InitStep {
    RuntimeSetup { span: Span },
    Constant { constant: ConstantId, span: Span },
    StaticData { name: String, span: Span },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub name: String,
    pub target: ReferenceTargetName,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceTargetName {
    LocalSymbol {
        name: String,
        kind: ReferenceKind,
    },
    QualifiedMember {
        module: String,
        member: String,
        resolved: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceKind {
    Function,
    Constant,
    Import,
    Imported,
    Parameter,
    Local,
    Type,
    Constructor,
    Field,
    Label,
    Prelude,
}

impl From<&SymbolKind> for ReferenceKind {
    fn from(kind: &SymbolKind) -> Self {
        match kind {
            SymbolKind::Function { .. } | SymbolKind::ExternalFunction { .. } => Self::Function,
            SymbolKind::Constant { .. } => Self::Constant,
            SymbolKind::Import { .. } => Self::Import,
            SymbolKind::Imported { .. } => Self::Imported,
            SymbolKind::Parameter => Self::Parameter,
            SymbolKind::Local => Self::Local,
            SymbolKind::Type { .. } => Self::Type,
            SymbolKind::Constructor { .. } => Self::Constructor,
            SymbolKind::Field { .. } => Self::Field,
            SymbolKind::Label => Self::Label,
            SymbolKind::Prelude => Self::Prelude,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Export {
    pub name: String,
    pub kind: ExportKind,
    pub span: Span,
}

impl Export {
    pub fn function(name: String, span: Span) -> Self {
        Self { name, kind: ExportKind::Function, span }
    }

    pub fn constant(name: String, span: Span) -> Self {
        Self { name, kind: ExportKind::Constant, span }
    }

    pub fn type_(name: String, span: Span) -> Self {
        Self { name, kind: ExportKind::Type, span }
    }

    pub fn constructor(name: String, span: Span) -> Self {
        Self { name, kind: ExportKind::Constructor, span }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportKind {
    Function,
    Constant,
    Type,
    Constructor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub name: String,
    pub public: bool,
    pub closure_captures: Vec<Type>,
    pub params: Vec<Local>,
    pub locals: Vec<Local>,
    pub return_type: Type,
    pub abi: CallAbi,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallAbi {
    pub params: Vec<AbiValue>,
    pub return_: Option<AbiValue>,
    pub boundary: CallBoundary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiValue {
    pub type_: Type,
    pub representation: RepresentationType,
}

impl From<&Type> for AbiValue {
    fn from(type_: &Type) -> Self {
        Self { type_: type_.clone(), representation: RepresentationType::from(type_) }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallBoundary {
    Internal,
    ModuleExport,
    ModuleImport { module: String },
    HostImport { module: String, name: String },
}

impl CallBoundary {
    pub fn stdlib(strategy: stdlib::MemberStrategy, member: &str) -> Self {
        match strategy {
            stdlib::MemberStrategy::HostImport => {
                Self::HostImport { module: stdlib::STDLIB_IO_HOST_MODULE.into(), name: member.into() }
            }
            _ => Self::Internal,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Local {
    pub id: LocalId,
    pub name: String,
    pub type_: Type,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub instructions: Vec<Instruction>,
    pub result: Box<Expression>,
    pub span: Span,
}

impl Block {
    pub fn contains_indirect_call(&self) -> bool {
        self.contains_expression(Expression::contains_indirect_call)
    }

    pub fn contains_anonymous_function(&self) -> bool {
        self.contains_expression(Expression::contains_anonymous_function)
    }

    pub fn contains_constructor(&self) -> bool {
        self.contains_expression(Expression::contains_constructor)
    }

    pub fn contains_record_update(&self) -> bool {
        self.contains_expression(Expression::contains_record_update)
    }

    fn contains_expression(&self, predicate: impl Fn(&Expression) -> bool) -> bool {
        self.instructions
            .iter()
            .any(|instruction| predicate(instruction.expression()))
            || predicate(&self.result)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instruction {
    Evaluate {
        expression: Expression,
        span: Span,
    },
    LocalSet {
        local: LocalId,
        value: Expression,
        span: Span,
    },
    AssertMatch {
        value: Expression,
        pattern: IrPattern,
        failure: FailurePath,
        span: Span,
    },
}

impl Instruction {
    pub fn expression(&self) -> &Expression {
        match self {
            Self::Evaluate { expression, .. } => expression,
            Self::LocalSet { value, .. } | Self::AssertMatch { value, .. } => value,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailurePath {
    pub reason: FailureReason,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailureReason {
    AssertMatch,
    BranchFallthrough,
    Panic,
    Todo,
    Assert,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expression {
    pub type_: Type,
    pub span: Span,
    pub kind: ExpressionKind,
}

impl Expression {
    pub fn new(type_: Type, span: Span, kind: ExpressionKind) -> Self {
        Self { type_, span, kind }
    }

    pub fn contains_indirect_call(&self) -> bool {
        matches!(self.kind, ExpressionKind::IndirectCall(_)) || self.children().any(Self::contains_indirect_call)
    }

    pub fn contains_anonymous_function(&self) -> bool {
        matches!(self.kind, ExpressionKind::AnonymousFunction(_))
            || self.children().any(Self::contains_anonymous_function)
    }

    pub fn contains_constructor(&self) -> bool {
        matches!(self.kind, ExpressionKind::Constructor(_)) || self.children().any(Self::contains_constructor)
    }

    pub fn contains_record_update(&self) -> bool {
        matches!(self.kind, ExpressionKind::RecordUpdate { .. }) || self.children().any(Self::contains_record_update)
    }

    pub fn children(&self) -> ExpressionChildren<'_> {
        let mut children = Vec::new();
        match &self.kind {
            ExpressionKind::DirectCall(call) => children.extend(call.arguments.iter().map(|argument| &argument.value)),
            ExpressionKind::IndirectCall(call) => {
                children.push(call.callee.as_ref());
                children.extend(call.arguments.iter().map(|argument| &argument.value));
            }
            ExpressionKind::Pipeline(pipeline) => {
                children.push(pipeline.input.as_ref());
                children.push(pipeline.call.as_ref());
            }
            ExpressionKind::Use(use_) => {
                children.push(use_.callback.as_ref());
                children.push(use_.call.as_ref());
            }
            ExpressionKind::Branch(branch) => {
                children.extend(branch.subjects.iter());
                for clause in &branch.clauses {
                    if let Some(guard) = &clause.guard {
                        children.push(guard);
                    }
                    children.push(clause.body.as_ref());
                }
            }
            ExpressionKind::Tuple(items) | ExpressionKind::List(items) => children.extend(items.iter()),
            ExpressionKind::BitArrayConcat { left, right }
            | ExpressionKind::Compare { left, right, .. }
            | ExpressionKind::RuntimeEquality { left, right } => {
                children.push(left.as_ref());
                children.push(right.as_ref());
            }
            ExpressionKind::BitStringDeconstruct { bit_array, .. }
            | ExpressionKind::FieldAccess { record: bit_array, .. }
            | ExpressionKind::TupleElement { tuple: bit_array, .. }
            | ExpressionKind::ListDeconstruct { list: bit_array, .. } => children.push(bit_array.as_ref()),
            ExpressionKind::Record(record) => children.extend(record.fields.iter().map(|field| &field.value)),
            ExpressionKind::Constructor(constructor) => children.extend(constructor.arguments.iter()),
            ExpressionKind::RecordUpdate { record, fields, .. } => {
                children.push(record.as_ref());
                children.extend(fields.iter().filter_map(|field| field.value.as_ref()));
            }
            ExpressionKind::ListCons { head, tail } => {
                children.push(head.as_ref());
                children.push(tail.as_ref());
            }
            ExpressionKind::Memory(operation) => match operation {
                MemoryOperation::Allocate { bytes } | MemoryOperation::Load { address: bytes, .. } => {
                    children.push(bytes.as_ref())
                }
                MemoryOperation::Store { address, value } => {
                    children.push(address.as_ref());
                    children.push(value.as_ref());
                }
            },
            ExpressionKind::Literal(_)
            | ExpressionKind::LocalGet(_)
            | ExpressionKind::FunctionValue(_)
            | ExpressionKind::AnonymousFunction(_)
            | ExpressionKind::BitArray(_)
            | ExpressionKind::Failure(_) => {}
        }
        ExpressionChildren { children, index: 0 }
    }
}

pub struct ExpressionChildren<'a> {
    children: Vec<&'a Expression>,
    index: usize,
}

impl<'a> Iterator for ExpressionChildren<'a> {
    type Item = &'a Expression;

    fn next(&mut self) -> Option<Self::Item> {
        let child = self.children.get(self.index).copied();
        self.index += usize::from(child.is_some());
        child
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpressionKind {
    Literal(Literal),
    LocalGet(LocalId),
    DirectCall(DirectCall),
    IndirectCall(IndirectCall),
    FunctionValue(FunctionValue),
    AnonymousFunction(AnonymousFunction),
    Pipeline(PipelineLowering),
    Use(UseLowering),
    Branch(Branch),
    Tuple(Vec<Expression>),
    List(Vec<Expression>),
    BitArray(BitArrayLiteral),
    BitArrayConcat {
        left: Box<Expression>,
        right: Box<Expression>,
    },
    BitStringDeconstruct {
        bit_array: Box<Expression>,
        segments: Vec<BitStringPatternSegment>,
    },
    Record(RecordValue),
    Constructor(ConstructorValue),
    FieldAccess {
        record: Box<Expression>,
        field: String,
    },
    RecordUpdate {
        record: Box<Expression>,
        constructor: String,
        fields: Vec<RecordFieldUpdate>,
    },
    ListCons {
        head: Box<Expression>,
        tail: Box<Expression>,
    },
    ListDeconstruct {
        list: Box<Expression>,
        head: LocalId,
        tail: LocalId,
    },
    TupleElement {
        tuple: Box<Expression>,
        index: usize,
    },
    Compare {
        op: ComparisonOp,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    RuntimeEquality {
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Memory(MemoryOperation),
    Failure(FailurePath),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectCall {
    pub function: String,
    pub arguments: Vec<CallArgument>,
    pub abi: CallAbi,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndirectCall {
    pub callee: Box<Expression>,
    pub arguments: Vec<CallArgument>,
    pub abi: CallAbi,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallArgument {
    pub label: Option<String>,
    pub value: Expression,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionValue {
    pub name: String,
    pub abi: CallAbi,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnonymousFunction {
    pub name: String,
    pub params: Vec<Local>,
    pub captures: Vec<Capture>,
    pub body: Block,
    pub abi: CallAbi,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capture {
    pub source: LocalId,
    pub name: String,
    pub type_: Type,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineLowering {
    pub input: Box<Expression>,
    pub call: Box<Expression>,
    pub inserted_argument: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UseLowering {
    pub callback: Box<Expression>,
    pub call: Box<Expression>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordValue {
    pub name: String,
    pub fields: Vec<RecordFieldValue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstructorValue {
    pub name: String,
    pub arguments: Vec<Expression>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordFieldValue {
    pub name: String,
    pub value: Expression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordFieldUpdate {
    pub name: String,
    pub type_: Type,
    pub value: Option<Expression>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonOp {
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryOperation {
    Allocate {
        bytes: Box<Expression>,
    },
    Load {
        address: Box<Expression>,
        type_: RepresentationType,
    },
    Store {
        address: Box<Expression>,
        value: Box<Expression>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Literal {
    pub kind: LiteralKind,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Branch {
    pub subjects: Vec<Expression>,
    pub clauses: Vec<BranchClause>,
    pub fallthrough: FailurePath,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchClause {
    pub patterns: Vec<IrPattern>,
    pub guard: Option<Expression>,
    pub bindings: Vec<SuccessfulBinding>,
    pub body: Box<Expression>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuccessfulBinding {
    pub local: LocalId,
    pub path: BindingPath,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingPath {
    Subject(usize),
    TupleElement {
        subject: usize,
        index: usize,
    },
    ListElement {
        subject: usize,
        index: usize,
    },
    ListTail {
        subject: usize,
    },
    ConstructorField {
        subject: usize,
        field: Option<String>,
        index: usize,
    },
    Alias {
        subject: usize,
    },
}

impl BindingPath {
    fn subject(&self) -> usize {
        match self {
            Self::Subject(subject)
            | Self::TupleElement { subject, .. }
            | Self::ListElement { subject, .. }
            | Self::ListTail { subject }
            | Self::ConstructorField { subject, .. }
            | Self::Alias { subject } => *subject,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrPattern {
    Discard,
    Binding(LocalId),
    Alias {
        pattern: Box<IrPattern>,
        local: LocalId,
    },
    Literal(Literal),
    Tuple(Vec<IrPattern>),
    List {
        elements: Vec<IrPattern>,
        tail: Option<LocalId>,
    },
    Constructor {
        name: String,
        arguments: Vec<ConstructorPatternArgument>,
    },
    BitString(Vec<BitStringPatternSegment>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstructorPatternArgument {
    pub label: Option<String>,
    pub pattern: IrPattern,
    pub span: Span,
}

pub fn lower(module: TypedModule) -> Result<Module, Diagnostics> {
    lowerer::lower(module)
}

pub fn lower_project(project: TypedProject) -> Result<Module, Diagnostics> {
    let mut modules = Vec::new();
    let mut diagnostics = Vec::new();

    for module in project.modules {
        match lowerer::lower_with_project_interfaces(module, &project.interfaces) {
            Ok(module) => modules.push(module),
            Err(mut errors) => diagnostics.append(&mut errors),
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    link_modules(modules)
}

fn link_modules(modules: Vec<Module>) -> Result<Module, Diagnostics> {
    let Some(first) = modules.first() else {
        return Err(vec![Diagnostic::new(
            DiagnosticCode::ProjectError,
            "project has no modules to compile",
        )]);
    };

    let mut linked = Module {
        span: first.span,
        imports: Vec::new(),
        declarations: Vec::new(),
        constants: Vec::new(),
        init: ModuleInit { steps: Vec::new() },
        references: Vec::new(),
        exports: Vec::new(),
        functions: Vec::new(),
    };
    let mut functions = HashSet::new();
    let mut diagnostics = Vec::new();

    for module in modules {
        for function in &module.functions {
            if !functions.insert(function.name.clone()) {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::ProjectError,
                    format!("duplicate lowered function `{}`", function.name),
                ));
            }
        }
        linked.imports.extend(module.imports);
        linked.declarations.extend(module.declarations);
        linked.constants.extend(module.constants);
        linked.init.steps.extend(module.init.steps);
        linked.references.extend(module.references);
        linked.exports.extend(module.exports);
        linked.functions.extend(module.functions);
    }

    if diagnostics.is_empty() { Ok(linked) } else { Err(diagnostics) }
}

fn comparison_op(operator: &ast::BinaryOperator) -> Option<ComparisonOp> {
    match operator {
        ast::BinaryOperator::Equal => Some(ComparisonOp::Equal),
        ast::BinaryOperator::NotEqual => Some(ComparisonOp::NotEqual),
        ast::BinaryOperator::LessThan | ast::BinaryOperator::FloatLessThan => Some(ComparisonOp::Less),
        ast::BinaryOperator::LessThanEqual | ast::BinaryOperator::FloatLessThanEqual => Some(ComparisonOp::LessEqual),
        ast::BinaryOperator::GreaterThan | ast::BinaryOperator::FloatGreaterThan => Some(ComparisonOp::Greater),
        ast::BinaryOperator::GreaterThanEqual | ast::BinaryOperator::FloatGreaterThanEqual => {
            Some(ComparisonOp::GreaterEqual)
        }
        _ => None,
    }
}

fn operator_function_name(operator: &ast::BinaryOperator) -> &'static str {
    match operator {
        ast::BinaryOperator::Add => "__op_add",
        ast::BinaryOperator::Subtract => "__op_subtract",
        ast::BinaryOperator::Multiply => "__op_multiply",
        ast::BinaryOperator::Divide => "__op_divide",
        ast::BinaryOperator::Remainder => "__op_remainder",
        ast::BinaryOperator::FloatAdd => "__op_float_add",
        ast::BinaryOperator::FloatSubtract => "__op_float_subtract",
        ast::BinaryOperator::FloatMultiply => "__op_float_multiply",
        ast::BinaryOperator::FloatDivide => "__op_float_divide",
        ast::BinaryOperator::And => "__op_and",
        ast::BinaryOperator::Or => "__op_or",
        ast::BinaryOperator::StringConcat => "__op_string_concat",
        ast::BinaryOperator::Equal
        | ast::BinaryOperator::NotEqual
        | ast::BinaryOperator::LessThan
        | ast::BinaryOperator::LessThanEqual
        | ast::BinaryOperator::GreaterThan
        | ast::BinaryOperator::GreaterThanEqual
        | ast::BinaryOperator::FloatLessThan
        | ast::BinaryOperator::FloatLessThanEqual
        | ast::BinaryOperator::FloatGreaterThan
        | ast::BinaryOperator::FloatGreaterThanEqual => "__op_compare",
    }
}

fn constructor_name(name: &ast::ConstructorName) -> String {
    match name {
        ast::ConstructorName::Local(name) => name.text.clone(),
        ast::ConstructorName::Remote { module, name, .. } => format!("{}.{}", module.text, name.text),
    }
}

fn collect_successful_bindings(
    context: &FunctionContext, pattern: &IrPattern, path: BindingPath, bindings: &mut Vec<SuccessfulBinding>,
) {
    match pattern {
        IrPattern::Binding(local) => {
            bindings.push(SuccessfulBinding { local: *local, path, span: context.local(*local).span })
        }
        IrPattern::Alias { pattern, local } => {
            collect_successful_bindings(context, pattern, path.clone(), bindings);
            bindings.push(SuccessfulBinding {
                local: *local,
                path: match path {
                    BindingPath::Subject(subject) => BindingPath::Alias { subject },
                    other => other,
                },
                span: context.local(*local).span,
            });
        }
        IrPattern::Tuple(elements) => {
            let subject = path.subject();
            for (index, element) in elements.iter().enumerate() {
                collect_successful_bindings(context, element, BindingPath::TupleElement { subject, index }, bindings);
            }
        }
        IrPattern::List { elements, tail } => {
            let subject = path.subject();
            for (index, element) in elements.iter().enumerate() {
                collect_successful_bindings(context, element, BindingPath::ListElement { subject, index }, bindings);
            }
            if let Some(local) = tail {
                bindings.push(SuccessfulBinding {
                    local: *local,
                    path: BindingPath::ListTail { subject },
                    span: context.local(*local).span,
                });
            }
        }
        IrPattern::Constructor { arguments, .. } => {
            let subject = path.subject();
            for (index, argument) in arguments.iter().enumerate() {
                collect_successful_bindings(
                    context,
                    &argument.pattern,
                    BindingPath::ConstructorField { subject, field: argument.label.clone(), index },
                    bindings,
                );
            }
        }
        IrPattern::BitString(segments) => {
            let subject = path.subject();
            for (index, segment) in segments.iter().enumerate() {
                if let Some(local) = segment.binding {
                    bindings.push(SuccessfulBinding {
                        local,
                        path: BindingPath::ListElement { subject, index },
                        span: context.local(local).span,
                    });
                }
            }
        }
        IrPattern::Discard | IrPattern::Literal(_) => {}
    }
}

fn raw_literal_arguments(raw: &ast::RawSyntax) -> Option<Vec<Expression>> {
    let source = raw.source.trim();
    let inner = source
        .strip_prefix("#(")
        .and_then(|source| source.strip_suffix(')'))
        .or_else(|| source.strip_prefix('[').and_then(|source| source.strip_suffix(']')))?;
    Some(
        inner
            .split(',')
            .filter_map(|item| integer_expression(item.trim(), raw.span))
            .collect(),
    )
}

fn raw_record_arguments(raw: &ast::RawSyntax) -> Option<Vec<Expression>> {
    let Some((_, args)) = raw.source.split_once('(') else {
        return Some(Vec::new());
    };
    let inner = args.strip_suffix(')')?;
    Some(
        inner
            .split(',')
            .filter_map(|item| item.split(':').next_back())
            .filter_map(|item| integer_expression(item.trim(), raw.span))
            .collect(),
    )
}

fn integer_expression(source: &str, span: Span) -> Option<Expression> {
    if source.is_empty() || source.parse::<i64>().is_err() {
        return None;
    }
    Some(Expression {
        type_: Type::Int,
        span,
        kind: ExpressionKind::Literal(Literal { kind: LiteralKind::Int, source: source.into() }),
    })
}

fn call_abi(type_: &Type, boundary: CallBoundary) -> CallAbi {
    match type_ {
        Type::Function { params, return_type } => {
            CallAbi { params: params.iter().map(AbiValue::from).collect(), return_: abi_return(return_type), boundary }
        }
        _ => CallAbi { params: Vec::new(), return_: None, boundary },
    }
}

fn abi_return(type_: &Type) -> Option<AbiValue> {
    if matches!(type_, Type::Nil) { None } else { Some(AbiValue::from(type_)) }
}

fn raw_metadata(raw: &ast::RawSyntax, kind: DeclarationKind, keyword: &str) -> DeclarationMetadata {
    let source = &raw.source;
    let visibility = visibility(source.trim_start().starts_with("pub "));
    DeclarationMetadata { name: declaration_name(source, keyword), kind, visibility, span: raw.span }
}

fn declaration_name(source: &str, keyword: &str) -> Option<String> {
    let source = source.trim_start();
    let source = source.strip_prefix("pub ").unwrap_or(source).trim_start();
    let source = if keyword.is_empty() { source } else { source.strip_prefix(keyword)?.trim_start() };
    source
        .split(|character: char| !matches!(character, '_' | 'a'..='z' | 'A'..='Z' | '0'..='9'))
        .find(|part| !part.is_empty())
        .map(str::to_string)
}

fn visibility(public: bool) -> Visibility {
    if public { Visibility::Public } else { Visibility::Private }
}

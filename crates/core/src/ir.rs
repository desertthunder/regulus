pub mod bit_slices;
mod closure;
mod lowerer;

use std::collections::{BTreeMap, HashMap};

use crate::{
    ast::{self, Declaration as AstDeclaration, LiteralKind},
    diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, Label},
    naming::{
        BackendItem, BackendItemKind, BackendName, CompilerGeneratedIndex, HelperKind, ModuleName, render_backend_name,
    },
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
    pub identity: Option<ModuleIdentity>,
    pub imports: Vec<Import>,
    pub declarations: Vec<DeclarationMetadata>,
    pub constants: Vec<Constant>,
    pub init: ModuleInit,
    pub references: Vec<Reference>,
    pub exports: Vec<Export>,
    pub functions: Vec<Function>,
    /// Source-to-generated names assigned by the project linker.
    ///
    /// This is empty for single-file compilation.
    pub linked_names: Vec<LinkedName>,
}

impl Module {
    pub fn linked_debug_dump(&self) -> String {
        use std::fmt::Write;

        let mut out = String::new();
        if !self.linked_names.is_empty() {
            writeln!(&mut out, "linked names:").expect("write linked IR debug dump");
            let mut names = self.linked_names.iter().collect::<Vec<_>>();
            names.sort_by_key(|name| {
                (
                    name.generated_name.as_str(),
                    name.source_name.as_str(),
                    &name.kind,
                    name.span.file_id.0,
                    name.span.start,
                    name.span.end,
                )
            });
            for name in names {
                writeln!(
                    &mut out,
                    "  {:?} source={} generated={}",
                    name.kind, name.source_name, name.generated_name
                )
                .expect("write linked IR debug dump");
            }
            writeln!(&mut out).expect("write linked IR debug dump");
        }

        let mut boundary_calls = self
            .functions
            .iter()
            .filter_map(|function| import_boundary_debug_line(function))
            .collect::<Vec<_>>();
        if !boundary_calls.is_empty() {
            boundary_calls.sort();
            writeln!(&mut out, "import call boundaries:").expect("write linked IR debug dump");
            for line in boundary_calls {
                writeln!(&mut out, "  {line}").expect("write linked IR debug dump");
            }
            writeln!(&mut out).expect("write linked IR debug dump");
        }

        writeln!(&mut out, "{self:#?}").expect("write linked IR debug dump");
        out
    }
}

fn import_boundary_debug_line(function: &Function) -> Option<String> {
    match &function.abi.boundary {
        CallBoundary::HostImport { module, name } => {
            Some(format!("host-import wrapper={} abi={module}.{name}", function.name))
        }
        CallBoundary::ModuleImport { module, name } => Some(format!(
            "dependency-interface wrapper={} abi={module}.{name}",
            function.name
        )),
        CallBoundary::Internal | CallBoundary::ModuleExport => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConstantId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleIdentity {
    pub package: String,
    pub module: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedName {
    pub source_name: String,
    pub generated_name: String,
    pub kind: LinkedNameKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum LinkedNameKind {
    Function,
    Constant,
    Constructor,
    Helper,
}

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
    /// User-facing ABI export name.
    pub name: String,
    /// Internal backend declaration name. This is assigned by the project
    /// linker after generated names are rendered. Single-file compilation
    /// leaves it unset and uses `name` as the backend name.
    pub backend_name: Option<String>,
    pub kind: ExportKind,
    pub span: Span,
}

impl Export {
    pub fn function(name: String, span: Span) -> Self {
        Self { name, backend_name: None, kind: ExportKind::Function, span }
    }

    pub fn constant(name: String, span: Span) -> Self {
        Self { name, backend_name: None, kind: ExportKind::Constant, span }
    }

    pub fn type_(name: String, span: Span) -> Self {
        Self { name, backend_name: None, kind: ExportKind::Type, span }
    }

    pub fn constructor(name: String, span: Span) -> Self {
        Self { name, backend_name: None, kind: ExportKind::Constructor, span }
    }

    pub fn backend_name(&self) -> &str {
        self.backend_name.as_deref().unwrap_or(&self.name)
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
    ModuleImport { module: String, name: String },
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
// TODO: this (and some other types) collide with ast types need to
// be renamed to avoid confusion. Maybe `IrLiteral`?
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
        identity: None,
        imports: Vec::new(),
        declarations: Vec::new(),
        constants: Vec::new(),
        init: ModuleInit { steps: Vec::new() },
        references: Vec::new(),
        exports: Vec::new(),
        functions: Vec::new(),
        linked_names: Vec::new(),
    };
    let rename_plan = global_backend_renames(&modules);
    let diagnostics = generated_name_collision_diagnostics(&rename_plan.linked_names);
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    linked.linked_names = rename_plan.linked_names;
    let global_renames = rename_plan.renames;

    for module in modules {
        let mut renames = module_backend_renames(&module, &global_renames);
        renames.extend(
            global_renames
                .iter()
                .map(|(source, backend)| (source.clone(), backend.clone())),
        );
        let mut module = module;
        rewrite_module_backend_names(&mut module, &renames);

        linked.imports.extend(module.imports);
        linked.declarations.extend(module.declarations);
        linked.constants.extend(module.constants);
        linked.init.steps.extend(module.init.steps);
        linked.references.extend(module.references);
        linked.exports.extend(module.exports);
        linked.functions.extend(module.functions);
    }

    Ok(linked)
}

struct BackendRenamePlan {
    renames: HashMap<String, String>,
    linked_names: Vec<LinkedName>,
}

fn global_backend_renames(modules: &[Module]) -> BackendRenamePlan {
    let mut renames = HashMap::new();
    let mut linked_names = Vec::new();
    for module in modules {
        let Some(identity) = &module.identity else { continue };
        let module_name = ModuleName::from_path(&identity.module);
        let mut generated = 0;
        for function in &module.functions {
            let backend = if matches!(
                function.abi.boundary,
                CallBoundary::HostImport { .. } | CallBoundary::ModuleImport { .. }
            ) {
                let index = generated;
                generated += 1;
                BackendName::package_item(
                    identity.package.as_str(),
                    module_name.clone(),
                    BackendItem::generated_for_member(
                        BackendItemKind::Helper(HelperKind::ImportWrapper),
                        function.name.as_str(),
                        CompilerGeneratedIndex(index),
                    ),
                )
            } else if let Some(index) = anonymous_function_index(&function.name) {
                BackendName::package_item(
                    identity.package.as_str(),
                    module_name.clone(),
                    BackendItem::generated(
                        BackendItemKind::Helper(HelperKind::LiftedFunction),
                        CompilerGeneratedIndex(index),
                    ),
                )
            } else if function.name.starts_with("__") {
                let index = generated;
                generated += 1;
                BackendName::package_item(
                    identity.package.as_str(),
                    module_name.clone(),
                    BackendItem::generated_for_member(
                        BackendItemKind::Helper(HelperKind::Other("project".into())),
                        function.name.as_str(),
                        CompilerGeneratedIndex(index),
                    ),
                )
            } else {
                BackendName::function(identity.package.as_str(), module_name.clone(), function.name.as_str())
            };
            let generated_name = render_backend_name(&backend);
            let source_name = format!("{}.{}", identity.module, function.name);
            let kind = if function.name.starts_with("__")
                || matches!(
                    function.abi.boundary,
                    CallBoundary::HostImport { .. } | CallBoundary::ModuleImport { .. }
                ) {
                LinkedNameKind::Helper
            } else {
                LinkedNameKind::Function
            };
            renames.insert(source_name.clone(), generated_name.clone());
            linked_names.push(LinkedName { source_name, generated_name, kind, span: function.span });
        }
        for constant in &module.constants {
            let backend = BackendName::constant(identity.package.as_str(), module_name.clone(), constant.name.as_str());
            let generated_name = render_backend_name(&backend);
            let source_name = format!("{}.{}", identity.module, constant.name);
            renames.insert(source_name.clone(), generated_name.clone());
            linked_names.push(LinkedName {
                source_name,
                generated_name,
                kind: LinkedNameKind::Constant,
                span: constant.span,
            });
        }
        for declaration in &module.declarations {
            if declaration.kind == DeclarationKind::TypeDefinition
                && let Some(name) = &declaration.name
            {
                let backend = BackendName::constructor(identity.package.as_str(), module_name.clone(), name.as_str());
                let generated_name = render_backend_name(&backend);
                let source_name = format!("{}.{}", identity.module, name);
                renames.insert(source_name.clone(), generated_name.clone());
                linked_names.push(LinkedName {
                    source_name,
                    generated_name,
                    kind: LinkedNameKind::Constructor,
                    span: declaration.span,
                });
            }
        }
    }
    BackendRenamePlan { renames, linked_names }
}

fn generated_name_collision_diagnostics(linked_names: &[LinkedName]) -> Diagnostics {
    let mut by_generated: BTreeMap<&str, Vec<&LinkedName>> = BTreeMap::new();
    for name in linked_names {
        by_generated.entry(name.generated_name.as_str()).or_default().push(name);
    }

    let mut diagnostics = Vec::new();
    for (generated_name, mut origins) in by_generated {
        if origins.len() < 2 {
            continue;
        }
        origins.sort_by_key(|origin| {
            (
                origin.source_name.as_str(),
                &origin.kind,
                origin.span.file_id.0,
                origin.span.start,
                origin.span.end,
            )
        });

        let mut diagnostic = Diagnostic::new(
            DiagnosticCode::ProjectError,
            format!("duplicate generated backend name `{generated_name}`"),
        )
        .with_note("generated backend names must be unique after project linking");
        for origin in origins {
            diagnostic = diagnostic.with_label(Label::primary(
                origin.span,
                format!("`{}` generated `{}`", origin.source_name, origin.generated_name),
            ));
        }
        diagnostics.push(diagnostic);
    }
    diagnostics
}

fn module_backend_renames(module: &Module, global: &HashMap<String, String>) -> HashMap<String, String> {
    let mut renames = HashMap::new();
    let Some(identity) = &module.identity else { return renames };

    for function in &module.functions {
        if let Some(backend) = global.get(&format!("{}.{}", identity.module, function.name)) {
            renames.insert(function.name.clone(), backend.clone());
        }
    }
    for constant in &module.constants {
        if let Some(backend) = global.get(&format!("{}.{}", identity.module, constant.name)) {
            renames.insert(constant.name.clone(), backend.clone());
        }
    }
    for declaration in &module.declarations {
        if declaration.kind == DeclarationKind::TypeDefinition
            && let Some(name) = &declaration.name
            && let Some(backend) = global.get(&format!("{}.{}", identity.module, name))
        {
            renames.insert(name.clone(), backend.clone());
        }
    }

    for import in &module.imports {
        let local = import.alias.clone().unwrap_or_else(|| {
            import
                .module
                .rsplit('/')
                .next()
                .unwrap_or(import.module.as_str())
                .to_string()
        });
        for (source, backend) in global {
            let Some(member) = source.strip_prefix(&format!("{}.", import.module)) else {
                continue;
            };
            renames.insert(format!("{local}.{member}"), backend.clone());
            if import
                .unqualified
                .iter()
                .any(|item| item.alias.as_deref().unwrap_or(&item.name) == member)
            {
                renames.insert(member.to_string(), backend.clone());
            }
        }
    }

    renames
}

fn anonymous_function_index(name: &str) -> Option<u32> {
    name.strip_prefix("__anon_")?.parse().ok()
}

fn rewrite_module_backend_names(module: &mut Module, renames: &HashMap<String, String>) {
    for constant in &mut module.constants {
        rewrite_name(&mut constant.name, renames);
    }
    for step in &mut module.init.steps {
        if let InitStep::StaticData { name, .. } = step {
            rewrite_name(name, renames);
        }
    }
    for reference in &mut module.references {
        rewrite_reference(reference, renames);
    }
    for export in &mut module.exports {
        if matches!(
            export.kind,
            ExportKind::Function | ExportKind::Constant | ExportKind::Constructor
        ) {
            export.backend_name = Some(
                renames
                    .get(&export.name)
                    .cloned()
                    .unwrap_or_else(|| export.name.clone()),
            );
        }
    }
    for function in &mut module.functions {
        rewrite_function(function, renames);
    }
}

fn rewrite_reference(reference: &mut Reference, renames: &HashMap<String, String>) {
    rewrite_name(&mut reference.name, renames);
    match &mut reference.target {
        ReferenceTargetName::LocalSymbol { name, .. } => rewrite_name(name, renames),
        ReferenceTargetName::QualifiedMember { module, member, resolved } => {
            let qualified = format!("{module}.{member}");
            if let Some(backend) = renames.get(&qualified).cloned() {
                *member = backend.clone();
                *resolved = Some(backend);
            } else if let Some(resolved_name) = resolved {
                rewrite_name(resolved_name, renames);
            }
        }
    }
}

fn rewrite_function(function: &mut Function, renames: &HashMap<String, String>) {
    if let Some(name) = renames.get(&function.name) {
        function.name = name.clone();
    }
    rewrite_block(&mut function.body, renames);
}

fn rewrite_block(block: &mut Block, renames: &HashMap<String, String>) {
    for instruction in &mut block.instructions {
        match instruction {
            Instruction::Evaluate { expression, .. } | Instruction::LocalSet { value: expression, .. } => {
                rewrite_expression(expression, renames)
            }
            Instruction::AssertMatch { value, pattern, .. } => {
                rewrite_expression(value, renames);
                rewrite_pattern(pattern, renames);
            }
        }
    }
    rewrite_expression(&mut block.result, renames);
}

fn rewrite_pattern(pattern: &mut IrPattern, renames: &HashMap<String, String>) {
    match pattern {
        IrPattern::Alias { pattern, .. } => rewrite_pattern(pattern, renames),
        IrPattern::Tuple(items) => {
            for item in items {
                rewrite_pattern(item, renames);
            }
        }
        IrPattern::List { elements, .. } => {
            for item in elements {
                rewrite_pattern(item, renames);
            }
        }
        IrPattern::Constructor { name, arguments } => {
            rewrite_name(name, renames);
            for argument in arguments {
                rewrite_pattern(&mut argument.pattern, renames);
            }
        }
        IrPattern::Discard | IrPattern::Binding(_) | IrPattern::Literal(_) | IrPattern::BitString(_) => {}
    }
}

fn rewrite_expression(expression: &mut Expression, renames: &HashMap<String, String>) {
    match &mut expression.kind {
        ExpressionKind::DirectCall(call) => {
            rewrite_name(&mut call.function, renames);
            for argument in &mut call.arguments {
                rewrite_expression(&mut argument.value, renames);
            }
        }
        ExpressionKind::FunctionValue(function) => rewrite_name(&mut function.name, renames),
        ExpressionKind::AnonymousFunction(function) => {
            rewrite_name(&mut function.name, renames);
            rewrite_block(&mut function.body, renames);
        }
        ExpressionKind::IndirectCall(call) => {
            rewrite_expression(&mut call.callee, renames);
            for argument in &mut call.arguments {
                rewrite_expression(&mut argument.value, renames);
            }
        }
        ExpressionKind::Pipeline(pipeline) => {
            rewrite_expression(&mut pipeline.input, renames);
            rewrite_expression(&mut pipeline.call, renames);
        }
        ExpressionKind::Use(use_) => {
            rewrite_expression(&mut use_.callback, renames);
            rewrite_expression(&mut use_.call, renames);
        }
        ExpressionKind::Branch(branch) => {
            for subject in &mut branch.subjects {
                rewrite_expression(subject, renames);
            }
            for clause in &mut branch.clauses {
                for pattern in &mut clause.patterns {
                    rewrite_pattern(pattern, renames);
                }
                if let Some(guard) = &mut clause.guard {
                    rewrite_expression(guard, renames);
                }
                rewrite_expression(&mut clause.body, renames);
            }
        }
        ExpressionKind::Tuple(items) | ExpressionKind::List(items) => {
            for item in items {
                rewrite_expression(item, renames);
            }
        }
        ExpressionKind::BitArrayConcat { left, right }
        | ExpressionKind::Compare { left, right, .. }
        | ExpressionKind::RuntimeEquality { left, right } => {
            rewrite_expression(left, renames);
            rewrite_expression(right, renames);
        }
        ExpressionKind::BitStringDeconstruct { bit_array, .. }
        | ExpressionKind::FieldAccess { record: bit_array, .. }
        | ExpressionKind::TupleElement { tuple: bit_array, .. }
        | ExpressionKind::ListDeconstruct { list: bit_array, .. } => rewrite_expression(bit_array, renames),
        ExpressionKind::Record(record) => {
            for field in &mut record.fields {
                rewrite_expression(&mut field.value, renames);
            }
        }
        ExpressionKind::Constructor(constructor) => {
            rewrite_name(&mut constructor.name, renames);
            for argument in &mut constructor.arguments {
                rewrite_expression(argument, renames);
            }
        }
        ExpressionKind::RecordUpdate { record, constructor, fields } => {
            rewrite_expression(record, renames);
            rewrite_name(constructor, renames);
            for field in fields {
                if let Some(value) = &mut field.value {
                    rewrite_expression(value, renames);
                }
            }
        }
        ExpressionKind::ListCons { head, tail } => {
            rewrite_expression(head, renames);
            rewrite_expression(tail, renames);
        }
        ExpressionKind::Memory(operation) => match operation {
            MemoryOperation::Allocate { bytes } | MemoryOperation::Load { address: bytes, .. } => {
                rewrite_expression(bytes, renames)
            }
            MemoryOperation::Store { address, value } => {
                rewrite_expression(address, renames);
                rewrite_expression(value, renames);
            }
        },
        ExpressionKind::Literal(_)
        | ExpressionKind::LocalGet(_)
        | ExpressionKind::BitArray(_)
        | ExpressionKind::Failure(_) => {}
    }
}

fn rewrite_name(name: &mut String, renames: &HashMap<String, String>) {
    if let Some(backend) = renames.get(name) {
        *name = backend.clone();
    }
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

#[cfg(test)]
mod tests {
    use crate::source::{SourceFileId, Span};

    use super::*;

    #[test]
    fn reports_generated_name_collisions_with_source_declarations() {
        let first_span = Span::new(SourceFileId(1), 10, 20);
        let second_span = Span::new(SourceFileId(2), 30, 40);
        let names = vec![
            LinkedName {
                source_name: "app/main.run".into(),
                generated_name: "generated/run".into(),
                kind: LinkedNameKind::Function,
                span: first_span,
            },
            LinkedName {
                source_name: "test/main.run".into(),
                generated_name: "generated/run".into(),
                kind: LinkedNameKind::Function,
                span: second_span,
            },
        ];

        let diagnostics = generated_name_collision_diagnostics(&names);

        assert_eq!(diagnostics.len(), 1);
        insta::assert_snapshot!(diagnostics[0].render_plain(), @r#"
ProjectError: duplicate generated backend name `generated/run`
  --> file 1 bytes 10..20
      `app/main.run` generated `generated/run`
  --> file 2 bytes 30..40
      `test/main.run` generated `generated/run`
  note: generated backend names must be unique after project linking
"#);
    }

    #[test]
    fn linked_debug_dump_shows_source_generated_names_and_import_boundaries() {
        let span = Span::new(SourceFileId(1), 0, 3);
        let module = Module {
            span,
            identity: None,
            imports: Vec::new(),
            declarations: Vec::new(),
            constants: Vec::new(),
            init: ModuleInit::default(),
            references: Vec::new(),
            exports: Vec::new(),
            functions: vec![
                Function {
                    name: "dep_parse".into(),
                    public: false,
                    closure_captures: Vec::new(),
                    params: Vec::new(),
                    locals: Vec::new(),
                    return_type: Type::Int,
                    abi: CallAbi {
                        params: Vec::new(),
                        return_: Some(AbiValue::from(&Type::Int)),
                        boundary: CallBoundary::ModuleImport { module: "gleam/int".into(), name: "parse".into() },
                    },
                    body: Block {
                        instructions: Vec::new(),
                        result: Box::new(Expression {
                            type_: Type::Nil,
                            span,
                            kind: ExpressionKind::Literal(Literal { kind: LiteralKind::Nil, source: "Nil".into() }),
                        }),
                        span,
                    },
                    span,
                },
                Function {
                    name: "host_print".into(),
                    public: false,
                    closure_captures: Vec::new(),
                    params: Vec::new(),
                    locals: Vec::new(),
                    return_type: Type::Nil,
                    abi: CallAbi {
                        params: Vec::new(),
                        return_: None,
                        boundary: CallBoundary::HostImport { module: "env".into(), name: "print".into() },
                    },
                    body: Block {
                        instructions: Vec::new(),
                        result: Box::new(Expression {
                            type_: Type::Nil,
                            span,
                            kind: ExpressionKind::Literal(Literal { kind: LiteralKind::Nil, source: "Nil".into() }),
                        }),
                        span,
                    },
                    span,
                },
            ],
            linked_names: vec![LinkedName {
                source_name: "app/main.run".into(),
                generated_name: "generated/run".into(),
                kind: LinkedNameKind::Function,
                span,
            }],
        };

        let dump = module.linked_debug_dump();

        insta::assert_snapshot!(dump.lines().take(7).collect::<Vec<_>>().join("\n"), @r#"
linked names:
  Function source=app/main.run generated=generated/run

import call boundaries:
  dependency-interface wrapper=dep_parse abi=gleam/int.parse
  host-import wrapper=host_print abi=env.print
"#);
    }
}

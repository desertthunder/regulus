pub mod bit_slices;
mod closure;
mod lowerer;
mod specialization;

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::{
    ast::{self, LiteralKind},
    diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, Label},
    naming::{
        BackendItem, BackendItemKind, BackendName, CompilerGeneratedIndex, HelperKind, ModuleName, render_backend_name,
    },
    resolve::{Namespace, ReferenceTarget, SymbolKind},
    source::Span,
    stdlib::StdlibRegistry,
    types::{Type, TypedModule, TypedProject},
};

pub use bit_slices::{BitArrayLiteral, BitArraySegment, BitSegmentOption, BitSegmentType, BitStringPatternSegment};
pub use lowerer::lower;
pub use lowerer::{FunctionContext, Lowerer};
pub use specialization::{DependencySpecialization, DependencySpecializationKey};

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
            Type::Anything | Type::Generic(_) | Type::Opaque { .. } => Self::HeapManaged(HeapRepresentation::Opaque),
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
    pub type_declarations: Vec<TypeMetadata>,
    pub constants: Vec<Constant>,
    pub init: ModuleInit,
    pub references: Vec<Reference>,
    pub js_externals: Vec<JsExternalMetadata>,
    pub exports: Vec<Export>,
    pub functions: Vec<Function>,
    pub dependency_specializations: Vec<DependencySpecialization>,
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
            .filter_map(Function::import_boundary_debug_line)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConstantId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleIdentity {
    pub package: String,
    pub module: String,
}

impl ModuleIdentity {
    fn linked_source_name(&self, member: &str) -> String {
        backend_key(self.package.as_str(), self.module.as_str(), member)
    }
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
    pub package: Option<String>,
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

impl From<&ast::Declaration> for DeclarationMetadata {
    fn from(declaration: &ast::Declaration) -> Self {
        match declaration {
            ast::Declaration::Import(import) => Self {
                name: Some(import.module.text.clone()),
                kind: DeclarationKind::Import,
                visibility: Visibility::Private,
                span: import.span,
            },
            ast::Declaration::Function(function) => Self {
                name: Some(function.name.text.clone()),
                kind: DeclarationKind::Function,
                visibility: Visibility::from_public(function.public),
                span: function.span,
            },
            ast::Declaration::Constant(constant) => Self {
                name: Some(constant.name.text.clone()),
                kind: DeclarationKind::Constant,
                visibility: Visibility::from_public(constant.public),
                span: constant.span,
            },
            ast::Declaration::ExternalFunction(function) => Self {
                name: Some(function.name.text.clone()),
                kind: DeclarationKind::ExternalFunction,
                visibility: Visibility::from_public(function.public),
                span: function.span,
            },
            ast::Declaration::ExternalType(type_) => Self {
                name: Some(type_.name.text.clone()),
                kind: DeclarationKind::ExternalType,
                visibility: Visibility::from_public(type_.public),
                span: type_.span,
            },
            ast::Declaration::TypeAlias(alias) => Self {
                name: Some(alias.name.text.clone()),
                kind: DeclarationKind::TypeAlias,
                visibility: Visibility::from_public(alias.public),
                span: alias.span,
            },
            ast::Declaration::TypeDefinition(type_) => Self {
                name: Some(type_.name.text.clone()),
                kind: DeclarationKind::TypeDefinition,
                visibility: Visibility::from_public(type_.public),
                span: type_.span,
            },
            ast::Declaration::Attribute(attribute) => Self {
                name: Some(attribute.name.text.clone()),
                kind: DeclarationKind::Attribute,
                visibility: Visibility::Private,
                span: attribute.span,
            },
            ast::Declaration::TargetGroup(group) => Self {
                name: Some(group.target.text.clone()),
                kind: DeclarationKind::TargetGroup,
                visibility: Visibility::Private,
                span: group.span,
            },
            ast::Declaration::Comment(comment) => Self {
                name: None,
                kind: DeclarationKind::Statement,
                visibility: Visibility::Private,
                span: comment.span,
            },
            ast::Declaration::Statement(raw) => raw_metadata(raw, DeclarationKind::Statement, ""),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeMetadata {
    pub name: String,
    pub parameters: Vec<String>,
    pub opaque: bool,
    pub constructors: Vec<ConstructorMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstructorMetadata {
    pub name: String,
    pub fields: Vec<FieldMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldMetadata {
    pub name: Option<String>,
    pub type_: Type,
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

impl Visibility {
    fn from_public(public: bool) -> Self {
        if public { Self::Public } else { Self::Private }
    }
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
    Literal(IrLiteral),
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
pub struct JsExternalMetadata {
    pub module: String,
    pub name: String,
    pub params: Vec<Type>,
    pub return_type: Type,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceTargetName {
    LocalSymbol {
        package: Option<String>,
        module: Option<String>,
        name: String,
        kind: ReferenceKind,
    },
    QualifiedMember {
        package: Option<String>,
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

impl Function {
    fn import_boundary_debug_line(&self) -> Option<String> {
        match &self.abi.boundary {
            CallBoundary::HostImport { module, name } => {
                Some(format!("host-import wrapper={} abi={module}.{name}", self.name))
            }
            CallBoundary::ModuleImport { module, name } => Some(format!(
                "dependency-interface wrapper={} abi={module}.{name}",
                self.name
            )),
            CallBoundary::Internal | CallBoundary::ModuleExport => None,
        }
    }

    fn substitute_types(&mut self, substitutions: &HashMap<String, Type>) {
        self.closure_captures = self
            .closure_captures
            .iter()
            .map(|type_| type_.substitute(substitutions))
            .collect();
        for param in &mut self.params {
            param.substitute_type(substitutions);
        }
        for local in &mut self.locals {
            local.substitute_type(substitutions);
        }
        self.return_type = self.return_type.substitute(substitutions);
        self.abi.substitute_types(substitutions);
        self.body.substitute_types(substitutions);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallAbi {
    pub params: Vec<AbiValue>,
    pub return_: Option<AbiValue>,
    pub boundary: CallBoundary,
}

impl CallAbi {
    fn substitute_types(&mut self, substitutions: &HashMap<String, Type>) {
        for param in &mut self.params {
            param.substitute_type(substitutions);
        }
        if let Some(return_) = &mut self.return_ {
            return_.substitute_type(substitutions);
        }
    }
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

impl AbiValue {
    fn substitute_type(&mut self, substitutions: &HashMap<String, Type>) {
        self.type_ = self.type_.substitute(substitutions);
        self.representation = RepresentationType::from(&self.type_);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallBoundary {
    Internal,
    ModuleExport,
    ModuleImport { module: String, name: String },
    HostImport { module: String, name: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Local {
    pub id: LocalId,
    pub name: String,
    pub type_: Type,
    pub span: Span,
}

impl Local {
    fn substitute_type(&mut self, substitutions: &HashMap<String, Type>) {
        self.type_ = self.type_.substitute(substitutions);
    }
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

    fn substitute_types(&mut self, substitutions: &HashMap<String, Type>) {
        substitute_block_types(self, substitutions);
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
    Literal(IrLiteral),
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

impl DirectCall {
    fn abi_rename_key(&self) -> Option<String> {
        let return_type = self.abi.return_.as_ref()?.type_.clone();
        let params = self
            .abi
            .params
            .iter()
            .map(|param| param.type_.clone())
            .collect::<Vec<_>>();
        Some(call_rename_key(&self.function, &params, &return_type))
    }

    fn expression_rename_key(&self, return_type: &Type) -> String {
        let params = self
            .arguments
            .iter()
            .map(|argument| argument.value.type_.clone())
            .collect::<Vec<_>>();
        call_rename_key(&self.function, &params, return_type)
    }

    fn expression_param_rename_key(&self) -> String {
        let params = self
            .arguments
            .iter()
            .map(|argument| argument.value.type_.clone())
            .collect::<Vec<_>>();
        call_param_rename_key(&self.function, &params)
    }

    fn abi_param_rename_key(&self) -> String {
        let params = self
            .abi
            .params
            .iter()
            .map(|param| param.type_.clone())
            .collect::<Vec<_>>();
        call_param_rename_key(&self.function, &params)
    }
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
pub struct IrLiteral {
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
    Literal(IrLiteral),
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

pub fn lower_project(project: TypedProject) -> Result<Module, Diagnostics> {
    let mut modules = Vec::new();
    let mut diagnostics = unsupported_dependency_member_diagnostics(&project);
    let source_backed_stdlib_modules = project
        .modules
        .iter()
        .filter_map(|module| {
            (module.package_name.as_deref() == Some("gleam_stdlib"))
                .then(|| module.module_name.clone())
                .flatten()
        })
        .collect::<std::collections::HashSet<_>>();
    diagnostics.extend(unsupported_source_backed_stdlib_runtime_diagnostics(
        &project,
        &source_backed_stdlib_modules,
    ));

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let dependency_specializations = project.collect_dependency_specializations();
    diagnostics.extend(unsupported_dependency_specialization_diagnostics(
        &project,
        &dependency_specializations,
    ));
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    for module in project.modules {
        let is_dependency_module = module.package_name.as_deref() != Some(project.package_name.as_str());
        match lowerer::lower_with_project_context(module, &project.interfaces, &source_backed_stdlib_modules) {
            Ok(mut module) => {
                if is_dependency_module {
                    keep_dependency_module_internal(&mut module);
                }
                modules.push(module);
            }
            Err(mut errors) => diagnostics.append(&mut errors),
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    link_mods(modules, &dependency_specializations)
}

fn keep_dependency_module_internal(module: &mut Module) {
    module.exports.clear();
    for function in &mut module.functions {
        function.public = false;
        if matches!(function.abi.boundary, CallBoundary::ModuleExport) {
            function.abi.boundary = CallBoundary::Internal;
        }
    }
}

fn unsupported_dependency_member_diagnostics(project: &TypedProject) -> Diagnostics {
    let project_modules = project
        .modules
        .iter()
        .filter_map(|module| module.module_name.as_deref())
        .collect::<HashSet<_>>();
    let stdlib_modules = StdlibRegistry::new()
        .modules()
        .map(|module| module.name)
        .collect::<HashSet<_>>();
    let dependency_modules = project
        .interfaces
        .keys()
        .map(String::as_str)
        .filter(|module| !project_modules.contains(module) && !stdlib_modules.contains(module))
        .collect::<HashSet<_>>();

    if dependency_modules.is_empty() {
        return Vec::new();
    }

    let mut diagnostics = Vec::new();
    let mut reported = HashSet::new();
    for module in &project.modules {
        for reference in &module.resolved.references {
            match &reference.target {
                ReferenceTarget::Symbol(symbol_id) => {
                    let symbol = module.resolved.symbols.symbol(*symbol_id);
                    if !matches!(symbol.namespace, Namespace::Value | Namespace::Constructor) {
                        continue;
                    }
                    let SymbolKind::Imported { module: dependency_module, member, .. } = &symbol.kind else {
                        continue;
                    };
                    if project
                        .interfaces
                        .get(dependency_module)
                        .is_some_and(|entry| entry.interface.externals.contains_key(member))
                    {
                        continue;
                    }
                    if dependency_modules.contains(dependency_module.as_str())
                        && reported.insert((reference.name.span, dependency_module.clone(), member.clone()))
                    {
                        diagnostics.push(unsupported_dependency_member_diagnostic(
                            dependency_module,
                            member,
                            reference.name.span,
                        ));
                    }
                }
                ReferenceTarget::QualifiedMember { module: module_symbol, member, .. } => {
                    let symbol = module.resolved.symbols.symbol(*module_symbol);
                    let SymbolKind::Import { module: dependency_module, .. } = &symbol.kind else {
                        continue;
                    };
                    if !dependency_modules.contains(dependency_module.as_str()) {
                        continue;
                    }
                    let Some(entry) = project.interfaces.get(dependency_module) else {
                        continue;
                    };
                    if entry.interface.externals.contains_key(&member.text) {
                        continue;
                    }
                    if !(entry.interface.functions.contains_key(&member.text)
                        || entry.interface.constructors.contains_key(&member.text))
                    {
                        continue;
                    }
                    if reported.insert((member.span, dependency_module.clone(), member.text.clone())) {
                        diagnostics.push(unsupported_dependency_member_diagnostic(
                            dependency_module,
                            &member.text,
                            member.span,
                        ));
                    }
                }
            }
        }
    }
    diagnostics
}

fn unsupported_dependency_member_diagnostic(module: &str, member: &str, span: Span) -> Diagnostic {
    Diagnostic::new(
        DiagnosticCode::LoweringError,
        format!(
            "dependency member `{module}.{member}` cannot be lowered yet; dependency source compilation is not supported"
        ),
    )
    .with_label(Label::primary(span, "unsupported dependency member used here"))
}

fn unsupported_source_backed_stdlib_runtime_diagnostics(
    project: &TypedProject, source_backed_stdlib_modules: &HashSet<String>,
) -> Diagnostics {
    let registry = StdlibRegistry::new();
    let mut diagnostics = Vec::new();
    let mut reported = HashSet::new();

    for module in &project.modules {
        if module.package_name.as_deref() != Some("gleam_stdlib") {
            continue;
        }

        for reference in &module.resolved.references {
            let Some((dependency_module, member, span)) = stdlib_dependency_reference(module, reference) else {
                continue;
            };
            if source_backed_stdlib_modules.contains(dependency_module) {
                continue;
            }
            let Some(interface) = registry.interface(dependency_module) else {
                continue;
            };
            if !interface.functions.contains_key(member) {
                continue;
            }
            if interface.externals.contains_key(member)
                || crate::runtime::stdlib_runtime_primitive(dependency_module, member).is_some()
                || crate::abi::stdlib_host_adapter(dependency_module, member).is_some()
            {
                continue;
            }
            if reported.insert((span, dependency_module.to_string(), member.to_string())) {
                diagnostics.push(unsupported_source_backed_stdlib_runtime_diagnostic(
                    dependency_module,
                    member,
                    span,
                ));
            }
        }
    }

    diagnostics
}

fn stdlib_dependency_reference<'a>(
    module: &'a TypedModule, reference: &'a crate::resolve::ResolvedReference,
) -> Option<(&'a str, &'a str, Span)> {
    match &reference.target {
        ReferenceTarget::Symbol(symbol_id) => {
            let symbol = module.resolved.symbols.symbol(*symbol_id);
            if !matches!(symbol.namespace, Namespace::Value) {
                return None;
            }
            let SymbolKind::Imported { package: Some(package), module, member } = &symbol.kind else {
                return None;
            };
            (package == "gleam_stdlib").then_some((module.as_str(), member.as_str(), reference.name.span))
        }
        ReferenceTarget::QualifiedMember { module: module_symbol, member, .. } => {
            let symbol = module.resolved.symbols.symbol(*module_symbol);
            let SymbolKind::Import { package: Some(package), module } = &symbol.kind else {
                return None;
            };
            (package == "gleam_stdlib").then_some((module.as_str(), member.text.as_str(), member.span))
        }
    }
}

fn unsupported_source_backed_stdlib_runtime_diagnostic(module: &str, member: &str, span: Span) -> Diagnostic {
    Diagnostic::new(
        DiagnosticCode::LoweringError,
        format!(
            "stdlib member `{module}.{member}` is used by compiled `gleam_stdlib` source but has no source body, runtime primitive, host adapter, or external implementation for this target yet"
        ),
    )
    .with_label(Label::primary(span, "unsupported stdlib dependency member used here"))
    .with_note(
        "add upstream source, a package asset/native external, or a narrow runtime primitive before compiling this stdlib path",
    )
}

fn unsupported_dependency_specialization_diagnostics(
    project: &TypedProject, specializations: &[DependencySpecialization],
) -> Diagnostics {
    specializations
        .iter()
        .filter(|specialization| dependency_specialization_type_is_unsupported(&specialization.instantiated_type))
        .map(|specialization| {
            let span = dependency_function_name_span(project, specialization).unwrap_or(specialization.source_span);
            unsupported_dependency_specialization_diagnostic(specialization, span)
        })
        .collect()
}

fn dependency_specialization_type_is_unsupported(type_: &Type) -> bool {
    type_.has_generic() || type_.contains_anything()
}

fn dependency_function_name_span(project: &TypedProject, specialization: &DependencySpecialization) -> Option<Span> {
    project
        .modules
        .iter()
        .find(|module| {
            module.package_name.as_deref() == Some(specialization.key.package.as_str())
                && module.module_name.as_deref() == Some(specialization.key.module.as_str())
        })
        .and_then(|module| {
            module
                .resolved
                .ast
                .functions
                .iter()
                .find(|function| function.name.text == specialization.key.function)
        })
        .map(|function| function.name.span)
}

fn unsupported_dependency_specialization_diagnostic(
    specialization: &DependencySpecialization, span: Span,
) -> Diagnostic {
    Diagnostic::new(
        DiagnosticCode::LoweringError,
        format!(
            "dependency specialization `{}:{}.{}` uses unsupported type `{}`",
            specialization.key.package,
            specialization.key.module,
            specialization.key.function,
            specialization.instantiated_type.display()
        ),
    )
    .with_label(Label::primary(span, "unsupported dependency specialization shape here"))
    .with_note("dependency specializations must have concrete internal runtime shapes before Wasm emission")
}

fn link_mods(mods: Vec<Module>, dep_specializations: &[DependencySpecialization]) -> Result<Module, Diagnostics> {
    let Some(first) = mods.first() else {
        return Err(vec![Diagnostic::new(
            DiagnosticCode::ProjectError,
            "project has no modules to compile",
        )]);
    };

    let rename_plan = global_backend_renames(&mods, dep_specializations);
    let mut linked = Module {
        span: first.span,
        identity: None,
        imports: Vec::new(),
        declarations: Vec::new(),
        type_declarations: Vec::new(),
        constants: Vec::new(),
        init: ModuleInit { steps: Vec::new() },
        references: Vec::new(),
        js_externals: Vec::new(),
        exports: Vec::new(),
        functions: Vec::new(),
        dependency_specializations: dep_specializations.to_owned(),
        linked_names: Vec::new(),
    };
    let diagnostics = generated_name_collision_diagnostics(&rename_plan.linked_names);
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    linked.linked_names = rename_plan.linked_names.clone();
    for module in mods {
        let renames = mod_backend_renames(&module, &rename_plan);
        let mut module = module;
        add_dep_specialization_funcs(&mut module, dep_specializations, &rename_plan);
        rewrite_mod_backend_names(&mut module, &renames, &rename_plan);
        remove_unspecialized_dep_functions(&mut module, &rename_plan);

        linked.imports.extend(module.imports);
        linked.declarations.extend(module.declarations);
        linked.type_declarations.extend(module.type_declarations);
        linked.constants.extend(module.constants);
        linked.init.steps.extend(module.init.steps);
        linked.references.extend(module.references);
        linked.js_externals.extend(module.js_externals);
        linked.exports.extend(module.exports);
        linked.functions.extend(module.functions);
    }

    Ok(linked)
}

struct BackendRenamePlan {
    renames: HashMap<String, String>,
    dependency_specializations: Vec<(DependencySpecializationKey, String)>,
    dependency_specialization_call_spans: HashMap<Span, String>,
    unspecialized_dependency_functions: HashSet<String>,
    linked_names: Vec<LinkedName>,
}

fn global_backend_renames(modules: &[Module], specializations: &[DependencySpecialization]) -> BackendRenamePlan {
    let mut renames = HashMap::new();
    let mut dependency_specialization_names = Vec::new();
    let mut dependency_specialization_call_spans = HashMap::<Span, Option<String>>::new();
    let mut unspecialized_dependency_functions = HashSet::new();
    let mut linked_names = Vec::new();
    let dependency_packages = specializations
        .iter()
        .map(|specialization| specialization.key.package.as_str())
        .collect::<HashSet<_>>();
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
            let source_name = identity.linked_source_name(function.name.as_str());
            let kind = if function.name.starts_with("__")
                || matches!(
                    function.abi.boundary,
                    CallBoundary::HostImport { .. } | CallBoundary::ModuleImport { .. }
                ) {
                LinkedNameKind::Helper
            } else {
                LinkedNameKind::Function
            };
            renames.insert(
                backend_key(
                    identity.package.as_str(),
                    identity.module.as_str(),
                    function.name.as_str(),
                ),
                generated_name.clone(),
            );
            if dependency_packages.contains(identity.package.as_str())
                && (!function.name.starts_with("__") || anonymous_function_index(&function.name).is_some())
                && !matches!(
                    function.abi.boundary,
                    CallBoundary::HostImport { .. } | CallBoundary::ModuleImport { .. }
                )
            {
                unspecialized_dependency_functions.insert(generated_name.clone());
            }
            linked_names.push(LinkedName { source_name, generated_name, kind, span: function.span });
        }
        for constant in &module.constants {
            let backend = BackendName::constant(identity.package.as_str(), module_name.clone(), constant.name.as_str());
            let generated_name = render_backend_name(&backend);
            let source_name = identity.linked_source_name(constant.name.as_str());
            renames.insert(
                backend_key(
                    identity.package.as_str(),
                    identity.module.as_str(),
                    constant.name.as_str(),
                ),
                generated_name.clone(),
            );
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
                let source_name = identity.linked_source_name(name.as_str());
                renames.insert(
                    backend_key(identity.package.as_str(), identity.module.as_str(), name.as_str()),
                    generated_name.clone(),
                );
                linked_names.push(LinkedName {
                    source_name,
                    generated_name,
                    kind: LinkedNameKind::Constructor,
                    span: declaration.span,
                });
            }
        }
    }
    let mut sorted_specializations = specializations.iter().collect::<Vec<_>>();
    sorted_specializations.sort_by_key(|specialization| specialization.key.source_name());
    let mut generated_by_owner = HashMap::<(&str, &str), u32>::new();
    for specialization in sorted_specializations {
        let module_name = ModuleName::from_path(&specialization.key.module);
        let owner = (specialization.key.package.as_str(), specialization.key.module.as_str());
        let index = generated_by_owner.entry(owner).or_insert(0);
        let backend = BackendName::package_item(
            specialization.key.package.as_str(),
            module_name,
            BackendItem::generated_for_member(
                BackendItemKind::Helper(HelperKind::Other("specialization".into())),
                specialization.key.function.as_str(),
                CompilerGeneratedIndex(*index),
            ),
        );
        *index += 1;
        let generated_name = render_backend_name(&backend);
        dependency_specialization_names.push((specialization.key.clone(), generated_name.clone()));
        dependency_specialization_call_spans
            .entry(specialization.source_span)
            .and_modify(|existing| {
                if existing.as_deref() != Some(generated_name.as_str()) {
                    *existing = None;
                }
            })
            .or_insert_with(|| Some(generated_name.clone()));
        linked_names.push(LinkedName {
            source_name: specialization.key.source_name(),
            generated_name,
            kind: LinkedNameKind::Function,
            span: specialization.source_span,
        });
    }
    BackendRenamePlan {
        renames,
        dependency_specializations: dependency_specialization_names,
        dependency_specialization_call_spans: dependency_specialization_call_spans
            .into_iter()
            .filter_map(|(span, backend)| backend.map(|backend| (span, backend)))
            .collect(),
        unspecialized_dependency_functions,
        linked_names,
    }
}

fn backend_key(package: &str, module: &str, member: &str) -> String {
    format!("{package}:{module}.{member}")
}

impl DependencySpecializationKey {
    fn source_name(&self) -> String {
        format!(
            "{}:{}.{}/{}->{}",
            self.package,
            self.module,
            self.function,
            self.params.iter().map(Type::display).collect::<Vec<_>>().join(","),
            self.return_type.display()
        )
    }
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

struct ModuleBackendRenames {
    names: HashMap<String, String>,
    calls: HashMap<String, String>,
    call_spans: HashMap<Span, String>,
}

fn mod_backend_renames(module: &Module, plan: &BackendRenamePlan) -> ModuleBackendRenames {
    let mut names = HashMap::new();
    let mut calls = HashMap::new();
    let call_spans = plan.dependency_specialization_call_spans.clone();
    let Some(identity) = &module.identity else {
        return ModuleBackendRenames { names, calls, call_spans };
    };
    let global = &plan.renames;

    for function in &module.functions {
        if let Some(backend) = global.get(&backend_key(
            identity.package.as_str(),
            identity.module.as_str(),
            function.name.as_str(),
        )) {
            names.insert(function.name.clone(), backend.clone());
            names.insert(format!("{}.{}", identity.module, function.name), backend.clone());
        }
    }
    for constant in &module.constants {
        if let Some(backend) = global.get(&backend_key(
            identity.package.as_str(),
            identity.module.as_str(),
            constant.name.as_str(),
        )) {
            names.insert(constant.name.clone(), backend.clone());
            names.insert(format!("{}.{}", identity.module, constant.name), backend.clone());
        }
    }
    for declaration in &module.declarations {
        if declaration.kind == DeclarationKind::TypeDefinition
            && let Some(name) = &declaration.name
            && let Some(backend) = global.get(&backend_key(
                identity.package.as_str(),
                identity.module.as_str(),
                name.as_str(),
            ))
        {
            names.insert(name.clone(), backend.clone());
            names.insert(format!("{}.{}", identity.module, name), backend.clone());
        }
    }

    for (key, backend) in &plan.dependency_specializations {
        if key.package == identity.package && key.module == identity.module {
            insert_call_rename(&mut calls, &key.function, key, backend);
            insert_call_rename(
                &mut calls,
                &format!("{}.{}", identity.module, key.function),
                key,
                backend,
            );
            insert_call_rename(
                &mut calls,
                &backend_key(&key.package, &key.module, &key.function),
                key,
                backend,
            );
            if let Some(unspecialized) = global.get(&backend_key(&key.package, &key.module, &key.function)) {
                insert_call_rename(&mut calls, unspecialized, key, backend);
            }
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
        let package = import.package.as_deref().unwrap_or(identity.package.as_str());
        let prefix = backend_key(package, &import.module, "");
        let package_qualified_prefix = backend_key(package, &import.module, "");
        for (source, backend) in global {
            let Some(member) = source.strip_prefix(&prefix) else {
                continue;
            };
            names.insert(source.clone(), backend.clone());
            names.insert(format!("{local}.{member}"), backend.clone());
            names.insert(format!("{}.{}", import.module, member), backend.clone());
            names.insert(format!("{package_qualified_prefix}{member}"), backend.clone());
            if import
                .unqualified
                .iter()
                .any(|item| item.alias.as_deref().unwrap_or(&item.name) == member)
            {
                names.insert(member.to_string(), backend.clone());
            }
        }
        for (key, backend) in &plan.dependency_specializations {
            if key.package != package || key.module != import.module {
                continue;
            }
            insert_call_rename(
                &mut calls,
                &backend_key(&key.package, &key.module, &key.function),
                key,
                backend,
            );
            if let Some(unspecialized) = global.get(&backend_key(&key.package, &key.module, &key.function)) {
                insert_call_rename(&mut calls, unspecialized, key, backend);
            }
            insert_call_rename(&mut calls, &format!("{local}.{}", key.function), key, backend);
            insert_call_rename(&mut calls, &format!("{}.{}", import.module, key.function), key, backend);
            insert_call_rename(
                &mut calls,
                &format!("{package_qualified_prefix}{}", key.function),
                key,
                backend,
            );
            if import
                .unqualified
                .iter()
                .any(|item| item.alias.as_deref().unwrap_or(&item.name) == key.function)
            {
                insert_call_rename(&mut calls, &key.function, key, backend);
            }
        }
    }

    ModuleBackendRenames { names, calls, call_spans }
}

fn insert_call_rename(
    calls: &mut HashMap<String, String>, source_name: &str, key: &DependencySpecializationKey, backend: &str,
) {
    calls.insert(
        call_rename_key(source_name, &key.params, &key.return_type),
        backend.to_string(),
    );
    calls.insert(call_param_rename_key(source_name, &key.params), backend.to_string());
}

fn call_rename_key(source_name: &str, params: &[Type], return_type: &Type) -> String {
    format!(
        "{}({})->{}",
        source_name,
        params.iter().map(Type::display).collect::<Vec<_>>().join(","),
        return_type.display()
    )
}

fn call_param_rename_key(source_name: &str, params: &[Type]) -> String {
    format!(
        "{}({})->?",
        source_name,
        params.iter().map(Type::display).collect::<Vec<_>>().join(",")
    )
}

fn unique_call_rename(source_name: &str, calls: &HashMap<String, String>) -> Option<String> {
    let prefix = format!("{source_name}(");
    let mut matches = calls
        .iter()
        .filter_map(|(key, backend)| key.starts_with(&prefix).then_some(backend.clone()))
        .collect::<Vec<_>>();
    matches.sort();
    matches.dedup();
    (matches.len() == 1).then(|| matches.remove(0))
}

fn add_dep_specialization_funcs(
    module: &mut Module, specializations: &[DependencySpecialization], plan: &BackendRenamePlan,
) {
    let Some(identity) = &module.identity else { return };
    let mut clones = Vec::new();
    for specialization in specializations {
        if specialization.key.package != identity.package || specialization.key.module != identity.module {
            continue;
        }
        let Some(source) = module
            .functions
            .iter()
            .find(|function| function.name == specialization.key.function)
        else {
            continue;
        };
        let Some((_, backend_name)) = plan
            .dependency_specializations
            .iter()
            .find(|(key, _)| key == &specialization.key)
        else {
            continue;
        };
        let mut function = source.clone();
        function.name = backend_name.clone();
        function.public = false;
        function.abi.boundary = CallBoundary::Internal;
        let substitutions = concrete_function_substitutions(source, specialization);
        substitute_function_types(&mut function, &substitutions);
        let helper_renames = dep_specialization_helper_renames(&function, backend_name);
        if !helper_renames.is_empty() {
            let helper_renames =
                ModuleBackendRenames { names: helper_renames, calls: HashMap::new(), call_spans: HashMap::new() };
            rewrite_function(&mut function, &helper_renames);
            for helper_name in helper_renames.names.keys() {
                let Some(helper_source) = module.functions.iter().find(|candidate| candidate.name == *helper_name)
                else {
                    continue;
                };
                let mut helper = helper_source.clone();
                helper.name = helper_renames
                    .names
                    .get(helper_name)
                    .cloned()
                    .unwrap_or_else(|| helper.name.clone());
                helper.public = false;
                helper.abi.boundary = CallBoundary::Internal;
                substitute_function_types(&mut helper, &substitutions);
                rewrite_function(&mut helper, &helper_renames);
                clones.push(helper);
            }
        }
        clones.push(function);
    }
    module.functions.extend(clones);
}

fn concrete_function_substitutions(
    source: &Function, specialization: &DependencySpecialization,
) -> HashMap<String, Type> {
    let mut substitutions = specialization.substitutions.clone();
    for (source_param, concrete_param) in source.params.iter().zip(&specialization.key.params) {
        collect_type_substitutions(&source_param.type_, concrete_param, &mut substitutions);
    }
    collect_type_substitutions(&source.return_type, &specialization.key.return_type, &mut substitutions);
    substitutions
}

fn collect_type_substitutions(source: &Type, concrete: &Type, substitutions: &mut HashMap<String, Type>) {
    match (source, concrete) {
        (Type::Generic(name), concrete) => {
            substitutions.entry(name.clone()).or_insert_with(|| concrete.clone());
        }
        (Type::List(source), Type::List(concrete)) => collect_type_substitutions(source, concrete, substitutions),
        (Type::Tuple(source), Type::Tuple(concrete)) => {
            for (source, concrete) in source.iter().zip(concrete) {
                collect_type_substitutions(source, concrete, substitutions);
            }
        }
        (
            Type::Function { params: source_params, return_type: source_return },
            Type::Function { params: concrete_params, return_type: concrete_return },
        ) => {
            for (source, concrete) in source_params.iter().zip(concrete_params) {
                collect_type_substitutions(source, concrete, substitutions);
            }
            collect_type_substitutions(source_return, concrete_return, substitutions);
        }
        (
            Type::Custom { name: source_name, args: source_args },
            Type::Custom { name: concrete_name, args: concrete_args },
        ) if source_name == concrete_name => {
            for (source, concrete) in source_args.iter().zip(concrete_args) {
                collect_type_substitutions(source, concrete, substitutions);
            }
        }
        (
            Type::Record { name: source_name, fields: source_fields },
            Type::Record { name: concrete_name, fields: concrete_fields },
        ) if source_name == concrete_name => {
            for (source, concrete) in source_fields.iter().zip(concrete_fields) {
                collect_type_substitutions(&source.type_, &concrete.type_, substitutions);
            }
        }
        (
            Type::Opaque { name: source_name, args: source_args },
            Type::Opaque { name: concrete_name, args: concrete_args },
        ) if source_name == concrete_name => {
            for (source, concrete) in source_args.iter().zip(concrete_args) {
                collect_type_substitutions(source, concrete, substitutions);
            }
        }
        _ => {}
    }
}

fn dep_specialization_helper_renames(function: &Function, specialization_backend: &str) -> HashMap<String, String> {
    let mut helper_names = HashSet::new();
    collect_function_value_names(&function.body, &mut helper_names);
    helper_names
        .into_iter()
        .filter(|name| name.starts_with("__anon_"))
        .map(|name| {
            let backend = format!("{specialization_backend}${}", name.trim_start_matches("__"));
            (name, backend)
        })
        .collect()
}

fn collect_function_value_names(block: &Block, names: &mut HashSet<String>) {
    for instruction in &block.instructions {
        collect_function_value_names_in_expr(instruction.expression(), names);
    }
    collect_function_value_names_in_expr(&block.result, names);
}

fn collect_function_value_names_in_expr(expr: &Expression, names: &mut HashSet<String>) {
    match &expr.kind {
        ExpressionKind::FunctionValue(function) => {
            names.insert(function.name.clone());
        }
        ExpressionKind::AnonymousFunction(function) => collect_function_value_names(&function.body, names),
        _ => {}
    }
    for child in expr.children() {
        collect_function_value_names_in_expr(child, names);
    }
}

fn remove_unspecialized_dep_functions(module: &mut Module, plan: &BackendRenamePlan) {
    module
        .functions
        .retain(|function| !plan.unspecialized_dependency_functions.contains(&function.name));
}

fn substitute_function_types(function: &mut Function, substitutions: &HashMap<String, Type>) {
    function.substitute_types(substitutions);
}

fn substitute_local_type(local: &mut Local, substitutions: &HashMap<String, Type>) {
    local.substitute_type(substitutions);
}

fn substitute_call_abi_types(abi: &mut CallAbi, substitutions: &HashMap<String, Type>) {
    abi.substitute_types(substitutions);
}

fn substitute_block_types(block: &mut Block, substitutions: &HashMap<String, Type>) {
    for instruction in &mut block.instructions {
        match instruction {
            Instruction::Evaluate { expression, .. } | Instruction::LocalSet { value: expression, .. } => {
                substitute_expression_types(expression, substitutions)
            }
            Instruction::AssertMatch { value, .. } => substitute_expression_types(value, substitutions),
        }
    }
    substitute_expression_types(&mut block.result, substitutions);
}

fn substitute_expression_types(expression: &mut Expression, substitutions: &HashMap<String, Type>) {
    expression.type_ = expression.type_.substitute(substitutions);
    match &mut expression.kind {
        ExpressionKind::DirectCall(call) => {
            substitute_call_abi_types(&mut call.abi, substitutions);
            for argument in &mut call.arguments {
                substitute_expression_types(&mut argument.value, substitutions);
            }
        }
        ExpressionKind::IndirectCall(call) => {
            substitute_expression_types(&mut call.callee, substitutions);
            substitute_call_abi_types(&mut call.abi, substitutions);
            for argument in &mut call.arguments {
                substitute_expression_types(&mut argument.value, substitutions);
            }
        }
        ExpressionKind::FunctionValue(function) => substitute_call_abi_types(&mut function.abi, substitutions),
        ExpressionKind::AnonymousFunction(function) => {
            for param in &mut function.params {
                substitute_local_type(param, substitutions);
            }
            for capture in &mut function.captures {
                capture.type_ = capture.type_.substitute(substitutions);
            }
            substitute_call_abi_types(&mut function.abi, substitutions);
            substitute_block_types(&mut function.body, substitutions);
        }
        ExpressionKind::Pipeline(pipeline) => {
            substitute_expression_types(&mut pipeline.input, substitutions);
            substitute_expression_types(&mut pipeline.call, substitutions);
        }
        ExpressionKind::Use(use_) => {
            substitute_expression_types(&mut use_.callback, substitutions);
            substitute_expression_types(&mut use_.call, substitutions);
        }
        ExpressionKind::Branch(branch) => {
            for subject in &mut branch.subjects {
                substitute_expression_types(subject, substitutions);
            }
            for clause in &mut branch.clauses {
                if let Some(guard) = &mut clause.guard {
                    substitute_expression_types(guard, substitutions);
                }
                substitute_expression_types(&mut clause.body, substitutions);
            }
        }
        ExpressionKind::Tuple(items) | ExpressionKind::List(items) => {
            for item in items {
                substitute_expression_types(item, substitutions);
            }
        }
        ExpressionKind::BitArrayConcat { left, right }
        | ExpressionKind::Compare { left, right, .. }
        | ExpressionKind::RuntimeEquality { left, right } => {
            substitute_expression_types(left, substitutions);
            substitute_expression_types(right, substitutions);
        }
        ExpressionKind::BitStringDeconstruct { bit_array, .. }
        | ExpressionKind::FieldAccess { record: bit_array, .. }
        | ExpressionKind::TupleElement { tuple: bit_array, .. }
        | ExpressionKind::ListDeconstruct { list: bit_array, .. } => {
            substitute_expression_types(bit_array, substitutions);
        }
        ExpressionKind::Record(record) => {
            for field in &mut record.fields {
                substitute_expression_types(&mut field.value, substitutions);
            }
        }
        ExpressionKind::Constructor(constructor) => {
            for argument in &mut constructor.arguments {
                substitute_expression_types(argument, substitutions);
            }
        }
        ExpressionKind::RecordUpdate { record, fields, .. } => {
            substitute_expression_types(record, substitutions);
            for field in fields {
                field.type_ = field.type_.substitute(substitutions);
                if let Some(value) = &mut field.value {
                    substitute_expression_types(value, substitutions);
                }
            }
        }
        ExpressionKind::ListCons { head, tail } => {
            substitute_expression_types(head, substitutions);
            substitute_expression_types(tail, substitutions);
        }
        ExpressionKind::Memory(operation) => match operation {
            MemoryOperation::Allocate { bytes } => substitute_expression_types(bytes, substitutions),
            MemoryOperation::Load { address, type_: _ } => substitute_expression_types(address, substitutions),
            MemoryOperation::Store { address, value } => {
                substitute_expression_types(address, substitutions);
                substitute_expression_types(value, substitutions);
            }
        },
        ExpressionKind::Literal(_)
        | ExpressionKind::LocalGet(_)
        | ExpressionKind::BitArray(_)
        | ExpressionKind::Failure(_) => {}
    }
}

fn anonymous_function_index(name: &str) -> Option<u32> {
    name.strip_prefix("__anon_")?.parse().ok()
}

fn rewrite_mod_backend_names(module: &mut Module, renames: &ModuleBackendRenames, _plan: &BackendRenamePlan) {
    for constant in &mut module.constants {
        rewrite_name(&mut constant.name, &renames.names);
    }
    for step in &mut module.init.steps {
        if let InitStep::StaticData { name, .. } = step {
            rewrite_name(name, &renames.names);
        }
    }
    for reference in &mut module.references {
        rewrite_reference(reference, &renames.names);
    }
    for export in &mut module.exports {
        if matches!(
            export.kind,
            ExportKind::Function | ExportKind::Constant | ExportKind::Constructor
        ) {
            export.backend_name = Some(
                renames
                    .names
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
        ReferenceTargetName::LocalSymbol { name, .. } => {
            rewrite_name(name, renames);
        }
        ReferenceTargetName::QualifiedMember { package, module, member, resolved } => {
            let qualified = package
                .as_deref()
                .map(|package| backend_key(package, module, member))
                .unwrap_or_else(|| format!("{module}.{member}"));
            if let Some(backend) = renames.get(&qualified).cloned() {
                *member = backend.clone();
                *resolved = Some(backend);
            } else if let Some(resolved_name) = resolved {
                rewrite_name(resolved_name, renames);
            }
        }
    }
}

fn rewrite_function(function: &mut Function, renames: &ModuleBackendRenames) {
    if let Some(name) = renames.names.get(&function.name) {
        function.name = name.clone();
    }
    rewrite_block(&mut function.body, renames);
}

fn rewrite_block(block: &mut Block, renames: &ModuleBackendRenames) {
    for instruction in &mut block.instructions {
        match instruction {
            Instruction::Evaluate { expression, .. } | Instruction::LocalSet { value: expression, .. } => {
                rewrite_expr(expression, renames)
            }
            Instruction::AssertMatch { value, pattern, .. } => {
                rewrite_expr(value, renames);
                rewrite_pattern(pattern, renames);
            }
        }
    }
    rewrite_expr(&mut block.result, renames);
}

fn rewrite_pattern(pattern: &mut IrPattern, renames: &ModuleBackendRenames) {
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
            rewrite_name(name, &renames.names);
            for argument in arguments {
                rewrite_pattern(&mut argument.pattern, renames);
            }
        }
        IrPattern::Discard | IrPattern::Binding(_) | IrPattern::Literal(_) | IrPattern::BitString(_) => {}
    }
}

fn rewrite_expr(expr: &mut Expression, renames: &ModuleBackendRenames) {
    let expression_type = expr.type_.clone();
    match &mut expr.kind {
        ExpressionKind::DirectCall(call) => {
            let expression_key = call.expression_rename_key(&expression_type);
            let expression_param_key = call.expression_param_rename_key();
            if let Some(name) = renames.call_spans.get(&expr.span).cloned().or_else(|| {
                renames
                    .calls
                    .get(&expression_key)
                    .cloned()
                    .or_else(|| renames.calls.get(&expression_param_key).cloned())
                    .or_else(|| call.abi_rename_key().and_then(|key| renames.calls.get(&key).cloned()))
                    .or_else(|| renames.calls.get(&call.abi_param_rename_key()).cloned())
                    .or_else(|| unique_call_rename(&call.function, &renames.calls))
            }) {
                call.function = name;
            } else {
                rewrite_name(&mut call.function, &renames.names);
                let expression_key = call.expression_rename_key(&expression_type);
                let expression_param_key = call.expression_param_rename_key();
                if let Some(name) = renames
                    .calls
                    .get(&expression_key)
                    .cloned()
                    .or_else(|| renames.calls.get(&expression_param_key).cloned())
                    .or_else(|| call.abi_rename_key().and_then(|key| renames.calls.get(&key).cloned()))
                    .or_else(|| renames.calls.get(&call.abi_param_rename_key()).cloned())
                    .or_else(|| unique_call_rename(&call.function, &renames.calls))
                {
                    call.function = name;
                }
            }
            for argument in &mut call.arguments {
                rewrite_expr(&mut argument.value, renames);
            }
        }
        ExpressionKind::FunctionValue(function) => rewrite_name(&mut function.name, &renames.names),
        ExpressionKind::AnonymousFunction(function) => {
            rewrite_name(&mut function.name, &renames.names);
            rewrite_block(&mut function.body, renames);
        }
        ExpressionKind::IndirectCall(call) => {
            rewrite_expr(&mut call.callee, renames);
            for argument in &mut call.arguments {
                rewrite_expr(&mut argument.value, renames);
            }
        }
        ExpressionKind::Pipeline(pipeline) => {
            rewrite_expr(&mut pipeline.input, renames);
            rewrite_expr(&mut pipeline.call, renames);
        }
        ExpressionKind::Use(use_) => {
            rewrite_expr(&mut use_.callback, renames);
            rewrite_expr(&mut use_.call, renames);
        }
        ExpressionKind::Branch(branch) => {
            for subject in &mut branch.subjects {
                rewrite_expr(subject, renames);
            }
            for clause in &mut branch.clauses {
                for pattern in &mut clause.patterns {
                    rewrite_pattern(pattern, renames);
                }
                if let Some(guard) = &mut clause.guard {
                    rewrite_expr(guard, renames);
                }
                rewrite_expr(&mut clause.body, renames);
            }
        }
        ExpressionKind::Tuple(items) | ExpressionKind::List(items) => {
            for item in items {
                rewrite_expr(item, renames);
            }
        }
        ExpressionKind::BitArrayConcat { left, right }
        | ExpressionKind::Compare { left, right, .. }
        | ExpressionKind::RuntimeEquality { left, right } => {
            rewrite_expr(left, renames);
            rewrite_expr(right, renames);
        }
        ExpressionKind::BitStringDeconstruct { bit_array, .. }
        | ExpressionKind::FieldAccess { record: bit_array, .. }
        | ExpressionKind::TupleElement { tuple: bit_array, .. }
        | ExpressionKind::ListDeconstruct { list: bit_array, .. } => rewrite_expr(bit_array, renames),
        ExpressionKind::Record(record) => {
            for field in &mut record.fields {
                rewrite_expr(&mut field.value, renames);
            }
        }
        ExpressionKind::Constructor(constructor) => {
            rewrite_name(&mut constructor.name, &renames.names);
            for argument in &mut constructor.arguments {
                rewrite_expr(argument, renames);
            }
        }
        ExpressionKind::RecordUpdate { record, constructor, fields } => {
            rewrite_expr(record, renames);
            rewrite_name(constructor, &renames.names);
            for field in fields {
                if let Some(value) = &mut field.value {
                    rewrite_expr(value, renames);
                }
            }
        }
        ExpressionKind::ListCons { head, tail } => {
            rewrite_expr(head, renames);
            rewrite_expr(tail, renames);
        }
        ExpressionKind::Memory(operation) => match operation {
            MemoryOperation::Allocate { bytes } | MemoryOperation::Load { address: bytes, .. } => {
                rewrite_expr(bytes, renames)
            }
            MemoryOperation::Store { address, value } => {
                rewrite_expr(address, renames);
                rewrite_expr(value, renames);
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
    ctx: &FunctionContext, pattern: &IrPattern, path: BindingPath, bindings: &mut Vec<SuccessfulBinding>,
) {
    match pattern {
        IrPattern::Binding(local) => {
            bindings.push(SuccessfulBinding { local: *local, path, span: ctx.local(*local).span })
        }
        IrPattern::Alias { pattern, local } => {
            collect_successful_bindings(ctx, pattern, path.clone(), bindings);
            bindings.push(SuccessfulBinding {
                local: *local,
                path: match path {
                    BindingPath::Subject(subject) => BindingPath::Alias { subject },
                    other => other,
                },
                span: ctx.local(*local).span,
            });
        }
        IrPattern::Tuple(elements) => {
            let subject = path.subject();
            for (index, element) in elements.iter().enumerate() {
                collect_successful_bindings(ctx, element, BindingPath::TupleElement { subject, index }, bindings);
            }
        }
        IrPattern::List { elements, tail } => {
            let subject = path.subject();
            for (index, element) in elements.iter().enumerate() {
                collect_successful_bindings(ctx, element, BindingPath::ListElement { subject, index }, bindings);
            }
            if let Some(local) = tail {
                bindings.push(SuccessfulBinding {
                    local: *local,
                    path: BindingPath::ListTail { subject },
                    span: ctx.local(*local).span,
                });
            }
        }
        IrPattern::Constructor { arguments, .. } => {
            let subject = path.subject();
            for (index, argument) in arguments.iter().enumerate() {
                collect_successful_bindings(
                    ctx,
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
                        span: ctx.local(local).span,
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
            .filter_map(|item| integer_expr(item.trim(), raw.span))
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
            .filter_map(|item| integer_expr(item.trim(), raw.span))
            .collect(),
    )
}

fn integer_expr(source: &str, span: Span) -> Option<Expression> {
    if source.is_empty() || source.parse::<i64>().is_err() {
        return None;
    }
    Some(Expression {
        type_: Type::Int,
        span,
        kind: ExpressionKind::Literal(IrLiteral { kind: LiteralKind::Int, source: source.into() }),
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
    let visibility = Visibility::from_public(source.trim_start().starts_with("pub "));
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use crate::loader::dependency::{DependencyPackage, DependencySourcePackage};
    use crate::{
        ast, parse, project,
        project::{DependencySource, GleamToml, ModuleInfo, PackageGraph, PackageNode, Project, SourceRoot},
        source::{SourceFile, SourceFileId, Span},
        target, types,
        types::{InterfaceEntry, ModuleInterface},
    };

    use super::*;

    fn fixture_project(path: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("fixtures/projects")
            .join(path)
    }

    fn stdlib_registry_interfaces() -> HashMap<String, InterfaceEntry> {
        StdlibRegistry::new()
            .modules()
            .map(|module| {
                (
                    module.name.to_string(),
                    InterfaceEntry::new("gleam_stdlib", module.name, module.interface.clone()),
                )
            })
            .collect()
    }

    fn interface_from_source(source: &SourceFile) -> ModuleInterface {
        let cst = parse::parse(source.clone()).expect("parse source interface");
        let module = ast::build(&cst).expect("build source interface");
        ModuleInterface::from(&module)
    }

    fn project_with_source_backed_stdlib_module(module_name: &str, module_source: SourceFile) -> Project {
        let app_source = SourceFile::new(
            SourceFileId(0),
            format!(
                r#"import {module_name}

pub fn main(value: String) {{
  missing.needs_parse(value)
}}
"#
            ),
        );
        let mut dependency_interfaces = stdlib_registry_interfaces();
        dependency_interfaces.insert(
            module_name.to_string(),
            InterfaceEntry::new("gleam_stdlib", module_name, interface_from_source(&module_source)),
        );
        Project {
            root: PathBuf::new(),
            config: GleamToml {
                name: "app".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                licences: Vec::new(),
                repository: None,
                links: Vec::new(),
                gleam: None,
                target: None,
                dependencies: Default::default(),
                dev_dependencies: Default::default(),
            },
            compile_target: target::CompileTarget::Wasmtime,
            graph: PackageGraph {
                root_package: PackageNode {
                    name: "app".to_string(),
                    version: "1.0.0".to_string(),
                    root: PathBuf::new(),
                },
                dependencies: Vec::new(),
                dependency_interfaces,
                dependency_sources: vec![DependencySourcePackage {
                    package: DependencyPackage {
                        name: "gleam_stdlib".to_string(),
                        version: Some("1.0.0".to_string()),
                        root: PathBuf::new(),
                        source: DependencySource::Path,
                    },
                    modules: vec![ModuleInfo {
                        name: module_name.to_string(),
                        path: PathBuf::from("src/gleam/missing.gleam"),
                        source_id: module_source.id,
                        source_root: SourceRoot::Src,
                    }],
                    sources: vec![module_source],
                    assets: Vec::new(),
                }],
                modules: vec![ModuleInfo {
                    name: "app".to_string(),
                    path: PathBuf::from("src/app.gleam"),
                    source_id: app_source.id,
                    source_root: SourceRoot::Src,
                }],
            },
            sources: vec![app_source],
        }
    }

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
    fn fixture_links_compiled_dependency_function_calls() {
        let project = project::load_project(fixture_project("linking/dependency_function")).expect("load project");
        let typed = types::check_project(&project).expect("type check project");
        let module = lower_project(typed).expect("lower linked project");

        let debug = module.linked_debug_dump();
        assert!(debug.contains("dep/foo.answer"));
        assert_eq!(module.dependency_specializations.len(), 1);
        assert_eq!(module.dependency_specializations[0].key.package, "dep_pkg");
        assert_eq!(module.dependency_specializations[0].key.module, "dep/foo");
        assert_eq!(module.dependency_specializations[0].key.function, "answer");
        assert_eq!(module.dependency_specializations[0].key.params, Vec::<Type>::new());
        assert_eq!(module.dependency_specializations[0].key.return_type, Type::Int);
        assert_eq!(module.functions.len(), 2);
        assert!(module.functions.iter().all(|function| {
            !matches!(
                function.abi.boundary,
                CallBoundary::HostImport { .. } | CallBoundary::ModuleImport { .. }
            )
        }));
        assert!(module.functions.iter().any(|function| {
            matches!(
                &function.body.result.kind,
                ExpressionKind::DirectCall(call)
                    if call.function.contains("$pkg$x6465705f706b67$mod$x646570$x666f6f$helper$x7370656369616c697a6174696f6e$x616e73776572$i0")
                        && call.abi.boundary == CallBoundary::Internal
            )
        }));
        assert!(
            !module
                .functions
                .iter()
                .any(|function| { function.name == "r$pkg$x6465705f706b67$mod$x646570$x666f6f$fn$x616e73776572" })
        );
        assert!(module.references.iter().any(|reference| {
            matches!(
                &reference.target,
                ReferenceTargetName::LocalSymbol {
                    package: Some(package),
                    module: Some(module),
                    kind: ReferenceKind::Imported,
                    ..
                } if package == "dep_pkg" && module == "dep/foo"
            )
        }));
    }

    #[test]
    fn collects_reachable_dependency_specializations_with_substitutions() {
        let id_source = SourceFile::new(
            SourceFileId(10),
            r#"pub fn id(value: a) -> a {
  value
}
"#,
        );
        let wrap_source = SourceFile::new(
            SourceFileId(11),
            r#"import dep/id

pub fn wrap(value: a) -> a {
  id.id(value)
}
"#,
        );
        let app_source = SourceFile::new(
            SourceFileId(12),
            r#"import dep/wrap

fn private() -> String {
  wrap.wrap("ok")
}

pub fn main() -> String {
  private()
}
"#,
        );
        let mut dependency_interfaces = HashMap::new();
        dependency_interfaces.insert(
            "dep/id".to_string(),
            InterfaceEntry::new("dep_pkg", "dep/id", interface_from_source(&id_source)),
        );
        dependency_interfaces.insert(
            "dep/wrap".to_string(),
            InterfaceEntry::new("dep_pkg", "dep/wrap", interface_from_source(&wrap_source)),
        );
        let project = Project {
            root: PathBuf::new(),
            config: GleamToml {
                name: "app".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                licences: Vec::new(),
                repository: None,
                links: Vec::new(),
                gleam: None,
                target: None,
                dependencies: Default::default(),
                dev_dependencies: Default::default(),
            },
            compile_target: target::CompileTarget::Wasmtime,
            graph: PackageGraph {
                root_package: PackageNode {
                    name: "app".to_string(),
                    version: "1.0.0".to_string(),
                    root: PathBuf::new(),
                },
                dependencies: Vec::new(),
                dependency_interfaces,
                dependency_sources: vec![DependencySourcePackage {
                    package: DependencyPackage {
                        name: "dep_pkg".to_string(),
                        version: Some("1.0.0".to_string()),
                        root: PathBuf::new(),
                        source: DependencySource::Path,
                    },
                    modules: vec![
                        ModuleInfo {
                            name: "dep/id".to_string(),
                            path: PathBuf::from("src/dep/id.gleam"),
                            source_id: id_source.id,
                            source_root: SourceRoot::Src,
                        },
                        ModuleInfo {
                            name: "dep/wrap".to_string(),
                            path: PathBuf::from("src/dep/wrap.gleam"),
                            source_id: wrap_source.id,
                            source_root: SourceRoot::Src,
                        },
                    ],
                    sources: vec![id_source, wrap_source],
                    assets: Vec::new(),
                }],
                modules: vec![ModuleInfo {
                    name: "app".to_string(),
                    path: PathBuf::from("src/app.gleam"),
                    source_id: app_source.id,
                    source_root: SourceRoot::Src,
                }],
            },
            sources: vec![app_source],
        };

        let typed = types::check_project(&project).expect("type check project");
        let specializations = typed.collect_dependency_specializations();
        let mut keys = specializations
            .iter()
            .map(|specialization| &specialization.key)
            .collect::<Vec<_>>();
        keys.sort_by_key(|key| (key.module.as_str(), key.function.as_str()));

        assert_eq!(keys.len(), 2);
        assert_eq!(keys[0].package, "dep_pkg");
        assert_eq!(keys[0].module, "dep/id");
        assert_eq!(keys[0].function, "id");
        assert_eq!(keys[0].params, vec![Type::String]);
        assert_eq!(keys[0].return_type, Type::String);
        assert_eq!(keys[1].module, "dep/wrap");
        assert_eq!(keys[1].params, vec![Type::String]);
        assert_eq!(keys[1].return_type, Type::String);
        for specialization in &specializations {
            assert_eq!(
                specialization.substitutions.get("a"),
                Some(&Type::String),
                "{specialization:?}"
            );
            let interface_type = project
                .graph
                .dependency_interfaces
                .get(&specialization.key.module)
                .and_then(|entry| entry.interface.functions.get(&specialization.key.function))
                .expect("dependency interface type");
            assert_eq!(
                interface_type.substitute(&specialization.substitutions),
                specialization.instantiated_type
            );
        }
    }

    #[test]
    fn stdlib_io_println_is_lowered_from_abi_host_adapter_table() {
        let source = SourceFile::new(
            SourceFileId(0),
            r#"import gleam/io

pub fn main() {
  io.println("hello")
}
"#,
        );
        let project = Project {
            root: PathBuf::new(),
            config: GleamToml {
                name: "app".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                licences: Vec::new(),
                repository: None,
                links: Vec::new(),
                gleam: None,
                target: None,
                dependencies: Default::default(),
                dev_dependencies: Default::default(),
            },
            compile_target: target::CompileTarget::Wasmtime,
            graph: PackageGraph {
                root_package: PackageNode {
                    name: "app".to_string(),
                    version: "1.0.0".to_string(),
                    root: PathBuf::new(),
                },
                dependencies: Vec::new(),
                dependency_interfaces: stdlib_registry_interfaces(),
                dependency_sources: Vec::new(),
                modules: vec![ModuleInfo {
                    name: "app".to_string(),
                    path: PathBuf::from("src/app.gleam"),
                    source_id: source.id,
                    source_root: SourceRoot::Src,
                }],
            },
            sources: vec![source],
        };
        let typed = types::check_project(&project).expect("type check project");
        let module = lower_project(typed).expect("lower linked project");

        assert_eq!(crate::runtime::stdlib_runtime_primitive("gleam/io", "println"), None);
        assert_eq!(
            crate::abi::stdlib_host_adapter("gleam/io", "println"),
            Some(crate::abi::StdlibHostAdapter {
                import_module: crate::abi::STDLIB_IO_HOST_MODULE,
                import_name: "println",
            })
        );
        assert!(module.functions.iter().any(|function| {
            matches!(
                &function.abi.boundary,
                CallBoundary::HostImport { module, name }
                    if module == crate::abi::STDLIB_IO_HOST_MODULE && name == "println"
            )
        }));
    }

    #[test]
    fn reports_missing_runtime_primitive_used_by_compiled_stdlib_source() {
        let stdlib_source = SourceFile::new(
            SourceFileId(1),
            r#"import gleam/int

pub fn needs_parse(value: String) -> Result(Int, Nil) {
  int.parse(value)
}
"#,
        );
        let project = project_with_source_backed_stdlib_module("gleam/missing", stdlib_source);
        let typed = types::check_project(&project).expect("type check project");
        let errors = lower_project(typed).expect_err("missing primitive should fail lowering");

        insta::assert_snapshot!(errors[0].render_plain(), @r#"
LoweringError: stdlib member `gleam/int.parse` is used by compiled `gleam_stdlib` source but has no source body, runtime primitive, host adapter, or external implementation for this target yet
  --> file 1 bytes 80..85
      unsupported stdlib dependency member used here
  note: add upstream source, a package asset/native external, or a narrow runtime primitive before compiling this stdlib path
"#);
    }

    #[test]
    fn links_dependency_bodyless_externals_as_host_imports() {
        let temp = tempfile::tempdir().expect("create temp project");
        let root = temp.path();
        fs::create_dir_all(root.join("dep_pkg/src/dep")).expect("create dependency src");
        fs::create_dir_all(root.join("src")).expect("create app src");
        fs::write(
            root.join("dep_pkg/gleam.toml"),
            r#"name = "dep_pkg"
version = "0.1.0"
description = "Dependency fixture with a bodyless external."
licences = ["Apache-2.0"]
target = "javascript"
"#,
        )
        .expect("write dependency manifest");
        fs::write(
            root.join("dep_pkg/src/dep/host.gleam"),
            r#"@external(javascript, "regulus/js", "request_text")
pub fn request_text(input: String) -> String
"#,
        )
        .expect("write dependency module");
        fs::write(
            root.join("gleam.toml"),
            r#"name = "bodyless_external_app"
version = "1.0.0"
description = "App fixture for dependency bodyless externals."
licences = ["Apache-2.0"]
target = "javascript"

[dependencies]
dep_pkg = { path = "dep_pkg" }
"#,
        )
        .expect("write app manifest");
        fs::write(
            root.join("src/app.gleam"),
            r#"import dep/host.{request_text}

pub fn main(input: String) -> String {
  request_text(input)
}
"#,
        )
        .expect("write app module");

        let project = project::load_project(root.join("gleam.toml")).expect("load project");
        let typed = types::check_project(&project).expect("type check project");
        let module = lower_project(typed).expect("lower linked project");

        assert!(module.functions.iter().any(|function| {
            matches!(
                &function.abi.boundary,
                CallBoundary::HostImport { module, name }
                    if module == "regulus/js" && name == "request_text"
            )
        }));
        assert!(module.functions.iter().any(|function| {
            matches!(
                &function.body.result.kind,
                ExpressionKind::DirectCall(call)
                    if matches!(
                        &call.abi.boundary,
                        CallBoundary::HostImport { module, name }
                            if module == "regulus/js" && name == "request_text"
                    )
            )
        }));
    }

    #[test]
    fn linked_debug_dump_shows_source_generated_names_and_import_boundaries() {
        let span = Span::new(SourceFileId(1), 0, 3);
        let module = Module {
            span,
            identity: None,
            imports: Vec::new(),
            declarations: Vec::new(),
            type_declarations: Vec::new(),
            constants: Vec::new(),
            init: ModuleInit::default(),
            references: Vec::new(),
            js_externals: Vec::new(),
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
                            kind: ExpressionKind::Literal(IrLiteral { kind: LiteralKind::Nil, source: "Nil".into() }),
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
                            kind: ExpressionKind::Literal(IrLiteral { kind: LiteralKind::Nil, source: "Nil".into() }),
                        }),
                        span,
                    },
                    span,
                },
            ],
            dependency_specializations: Vec::new(),
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

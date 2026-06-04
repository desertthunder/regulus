use std::collections::HashMap;

use crate::{
    ast::{self, Declaration as AstDeclaration, Expression as AstExpression, LiteralKind, Pattern, Statement},
    diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, Label},
    resolve::{ReferenceTarget, SymbolKind},
    source::Span,
    types::{Type, TypedModule},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepresentationType {
    Scalar(ScalarRepresentation),
    HeapManaged(HeapRepresentation),
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
    Import,
    Imported,
    Parameter,
    Local,
    Type,
    Constructor,
    Field,
    Prelude,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Export {
    pub name: String,
    pub kind: ExportKind,
    pub span: Span,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallBoundary {
    Internal,
    ModuleExport,
    ModuleImport { module: String },
    HostImport { module: String, name: String },
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
    Record(RecordValue),
    Constructor(ConstructorValue),
    FieldAccess {
        record: Box<Expression>,
        field: String,
    },
    RecordUpdate {
        record: Box<Expression>,
        updates: Vec<RecordFieldValue>,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstructorPatternArgument {
    pub label: Option<String>,
    pub pattern: IrPattern,
    pub span: Span,
}

pub fn lower(module: TypedModule) -> Result<Module, Diagnostics> {
    Lowerer::new(module).lower()
}

struct Lowerer {
    module: TypedModule,
    function_types: HashMap<String, Type>,
    expression_types: HashMap<Span, Type>,
    diagnostics: Diagnostics,
}

impl Lowerer {
    fn new(module: TypedModule) -> Self {
        let function_types = module
            .functions
            .iter()
            .map(|function| (function.name.text.clone(), function.type_.clone()))
            .collect();
        let expression_types = module
            .expressions
            .iter()
            .map(|expression| (expression.span, expression.type_.clone()))
            .collect();
        Self { module, function_types, expression_types, diagnostics: Vec::new() }
    }

    fn lower(mut self) -> Result<Module, Diagnostics> {
        let ast = self.module.resolved.ast.clone();
        let imports = ast.imports.iter().map(lower_import).collect();
        let declarations = ast.declarations.iter().map(DeclarationMetadata::from).collect();
        let references = self.lower_references();
        let mut exports = self.lower_exports();
        let mut constants = Vec::new();
        let mut init = ModuleInit::default();
        if ast
            .declarations
            .iter()
            .any(|declaration| matches!(declaration, AstDeclaration::Constant(_)))
        {
            init.steps.push(InitStep::RuntimeSetup { span: ast.span });
        }

        for declaration in &ast.declarations {
            if let AstDeclaration::Constant(raw) = declaration {
                let id = ConstantId(constants.len() as u32);
                let constant = lower_constant(id, raw);
                if constant.public {
                    exports.push(Export {
                        name: constant.name.clone(),
                        kind: ExportKind::Constant,
                        span: constant.span,
                    });
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

        let mut functions = Vec::new();
        for function in ast.functions {
            if let Some(function) = self.lower_function(&function) {
                functions.push(function);
            }
        }

        if self.diagnostics.is_empty() {
            Ok(Module { span: ast.span, imports, declarations, constants, init, references, exports, functions })
        } else {
            Err(self.diagnostics)
        }
    }

    fn lower_exports(&self) -> Vec<Export> {
        let mut exports = Vec::new();
        for function in &self.module.resolved.ast.functions {
            if function.public {
                exports.push(Export {
                    name: function.name.text.clone(),
                    kind: ExportKind::Function,
                    span: function.name.span,
                });
            }
        }
        for declaration in &self.module.resolved.ast.declarations {
            match declaration {
                AstDeclaration::TypeDefinition(raw) if is_public_declaration(&raw.source) => {
                    for name in exported_type_names(&raw.source) {
                        exports.push(Export { name, kind: ExportKind::Type, span: raw.span });
                    }
                }
                AstDeclaration::TypeAlias(raw) if is_public_declaration(&raw.source) => {
                    if let Some(name) = declaration_name(&raw.source, "type") {
                        exports.push(Export { name, kind: ExportKind::Type, span: raw.span });
                    }
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
                        ReferenceTargetName::LocalSymbol {
                            name: symbol.name.clone(),
                            kind: ReferenceKind::from(&symbol.kind),
                        }
                    }
                    ReferenceTarget::QualifiedMember { module, member, symbol } => {
                        let module_symbol = self.module.resolved.symbols.symbol(*module);
                        let resolved = symbol.map(|id| self.module.resolved.symbols.symbol(id).name.clone());
                        ReferenceTargetName::QualifiedMember {
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

    fn lower_block(&mut self, context: &mut FunctionContext, block: &ast::Block) -> Option<Block> {
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
                        _ => self.diagnostics.push(
                            Diagnostic::new(DiagnosticCode::LoweringError, "unsupported let pattern")
                                .with_label(Label::primary(let_.span, "unsupported pattern here")),
                        ),
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
                Statement::Expression(expression) => {
                    let value = self.lower_expression(context, expression)?;
                    if last_statement {
                        result = value;
                    } else {
                        instructions
                            .push(Instruction::Evaluate { expression: value, span: ast_expression_span(expression) });
                    }
                }
            }
        }

        context.pop_scope();

        Some(Block { instructions, result: Box::new(result), span: block.span })
    }

    fn lower_expression(&mut self, context: &mut FunctionContext, expression: &AstExpression) -> Option<Expression> {
        match expression {
            AstExpression::Literal(literal) => Some(Expression {
                type_: type_for_literal(literal.kind.clone()),
                span: literal.span,
                kind: ExpressionKind::Literal(Literal { kind: literal.kind.clone(), source: literal.source.clone() }),
            }),
            AstExpression::Variable(name) => {
                if let Some(local) = context.lookup(&name.text) {
                    let type_ = context.local(local).type_.clone();
                    return Some(Expression { type_, span: name.span, kind: ExpressionKind::LocalGet(local) });
                }

                let type_ = self.function_types.get(&name.text)?.clone();
                Some(Expression {
                    span: name.span,
                    kind: ExpressionKind::FunctionValue(FunctionValue {
                        name: name.text.clone(),
                        abi: call_abi(&type_, CallBoundary::Internal),
                    }),
                    type_,
                })
            }
            AstExpression::Call(call) => self.lower_call(context, call),
            AstExpression::Block(block) => Some(*self.lower_block(context, block)?.result),
            AstExpression::Case(case) => {
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
            AstExpression::FieldAccess(field_access) => {
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
            AstExpression::Raw(raw) if raw.kind == "record" => {
                let type_ = self.typed_expression_type(raw.span).unwrap_or(Type::Nil);
                Some(Expression {
                    type_,
                    span: raw.span,
                    kind: ExpressionKind::Constructor(ConstructorValue {
                        name: raw.source.split(['(', ' ']).next().unwrap_or(&raw.source).into(),
                        arguments: Vec::new(),
                    }),
                })
            }
            AstExpression::Raw(raw) if matches!(raw.kind.as_str(), "panic" | "todo" | "assert") => Some(Expression {
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
            AstExpression::Raw(raw) => {
                self.diagnostics.push(
                    Diagnostic::new(
                        DiagnosticCode::LoweringError,
                        format!("expression `{}` cannot be lowered", raw.kind),
                    )
                    .with_label(Label::primary(raw.span, "unsupported expression here")),
                );
                None
            }
        }
    }

    fn lower_call(&mut self, context: &mut FunctionContext, call: &ast::Call) -> Option<Expression> {
        if let AstExpression::Variable(function_name) = call.function.as_ref()
            && let Some(function_type) = self.function_types.get(&function_name.text).cloned()
        {
            let Type::Function { return_type, .. } = function_type.clone() else {
                return None;
            };
            return Some(Expression {
                type_: *return_type,
                span: call.span,
                kind: ExpressionKind::DirectCall(DirectCall {
                    function: function_name.text.clone(),
                    arguments: self.lower_call_arguments(context, &call.arguments)?,
                    abi: call_abi(&function_type, CallBoundary::Internal),
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
                arguments: self.lower_call_arguments(context, &call.arguments)?,
                abi: call_abi(&callee_type, CallBoundary::Internal),
            }),
        })
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
                let arguments = constructor
                    .arguments
                    .iter()
                    .map(|argument| {
                        let pattern = match &argument.pattern {
                            Some(pattern) => self.lower_pattern(context, pattern, subject_type)?,
                            None => match &argument.label {
                                Some(label) => {
                                    let local = context.allocate(label, subject_type.clone());
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
                    })
                    .collect::<Option<Vec<_>>>()?;
                Some(IrPattern::Constructor { name: constructor_name(&constructor.constructor), arguments })
            }
            Pattern::Alias(alias) => {
                let inner = self.lower_pattern(context, &alias.pattern, subject_type)?;
                let local = context.allocate(&alias.alias, subject_type.clone());
                context.bind(alias.alias.text.clone(), local.id);
                Some(IrPattern::Alias { pattern: Box::new(inner), local: local.id })
            }
            Pattern::BitString(raw) => {
                self.unsupported_pattern(raw.span, "bit string pattern");
                None
            }
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
            | IrPattern::Constructor { .. } => {
                let _ = context;
            }
        }
    }

    fn unsupported_pattern(&mut self, span: Span, kind: &str) {
        self.diagnostics.push(
            Diagnostic::new(DiagnosticCode::LoweringError, format!("{kind} cannot be lowered"))
                .with_label(Label::primary(span, "unsupported pattern here")),
        );
    }

    fn typed_expression_type(&self, span: Span) -> Option<Type> {
        self.expression_types.get(&span).cloned()
    }

    fn nil_expression(&self, span: Span) -> Expression {
        Expression {
            type_: Type::Nil,
            span,
            kind: ExpressionKind::Literal(Literal { kind: LiteralKind::Nil, source: "Nil".into() }),
        }
    }
}

#[derive(Default)]
struct FunctionContext {
    locals: Vec<Local>,
    scopes: Vec<HashMap<String, LocalId>>,
}

impl FunctionContext {
    fn allocate(&mut self, name: &ast::Name, type_: Type) -> Local {
        let local = Local { id: LocalId(self.locals.len() as u32), name: name.text.clone(), type_, span: name.span };
        self.locals.push(local.clone());
        local
    }

    fn local(&self, id: LocalId) -> &Local {
        &self.locals[id.0 as usize]
    }

    fn bind(&mut self, name: String, local: LocalId) {
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

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }
}

fn ast_expression_span(expression: &AstExpression) -> Span {
    match expression {
        AstExpression::Literal(literal) => literal.span,
        AstExpression::Variable(name) => name.span,
        AstExpression::Call(call) => call.span,
        AstExpression::FieldAccess(field_access) => field_access.span,
        AstExpression::Block(block) => block.span,
        AstExpression::Case(case) => case.span,
        AstExpression::Raw(raw) => raw.span,
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
            let subject = binding_subject(&path);
            for (index, element) in elements.iter().enumerate() {
                collect_successful_bindings(context, element, BindingPath::TupleElement { subject, index }, bindings);
            }
        }
        IrPattern::List { elements, tail } => {
            let subject = binding_subject(&path);
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
            let subject = binding_subject(&path);
            for (index, argument) in arguments.iter().enumerate() {
                collect_successful_bindings(
                    context,
                    &argument.pattern,
                    BindingPath::ConstructorField { subject, field: argument.label.clone(), index },
                    bindings,
                );
            }
        }
        IrPattern::Discard | IrPattern::Literal(_) => {}
    }
}

fn binding_subject(path: &BindingPath) -> usize {
    match path {
        BindingPath::Subject(subject)
        | BindingPath::TupleElement { subject, .. }
        | BindingPath::ListElement { subject, .. }
        | BindingPath::ListTail { subject }
        | BindingPath::ConstructorField { subject, .. }
        | BindingPath::Alias { subject } => *subject,
    }
}

fn type_for_literal(kind: LiteralKind) -> Type {
    match kind {
        LiteralKind::Int => Type::Int,
        LiteralKind::Float => Type::Float,
        LiteralKind::String => Type::String,
        LiteralKind::Bool => Type::Bool,
        LiteralKind::Nil => Type::Nil,
    }
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

impl From<&Type> for AbiValue {
    fn from(type_: &Type) -> Self {
        Self { type_: type_.clone(), representation: RepresentationType::from(type_) }
    }
}

impl From<&Type> for RepresentationType {
    fn from(type_: &Type) -> Self {
        match type_ {
            Type::Int => Self::Scalar(ScalarRepresentation::I64),
            Type::Float => Self::Scalar(ScalarRepresentation::F64),
            Type::Bool => Self::Scalar(ScalarRepresentation::I32),
            Type::Nil => Self::Scalar(ScalarRepresentation::Unit),
            Type::String => Self::HeapManaged(HeapRepresentation::String),
            Type::Tuple(_) => Self::HeapManaged(HeapRepresentation::Tuple),
            Type::List(_) => Self::HeapManaged(HeapRepresentation::List),
            Type::Record { .. } => Self::HeapManaged(HeapRepresentation::Record),
            Type::Custom { .. } => Self::HeapManaged(HeapRepresentation::Custom),
            Type::Function { .. } => Self::HeapManaged(HeapRepresentation::Function),
            Type::Generic(_) | Type::Opaque { .. } => Self::HeapManaged(HeapRepresentation::Opaque),
        }
    }
}

fn lower_import(import: &ast::Import) -> Import {
    Import {
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
    }
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
            AstDeclaration::Constant(raw) => raw_metadata(raw, DeclarationKind::Constant, "const"),
            AstDeclaration::ExternalFunction(raw) => raw_metadata(raw, DeclarationKind::ExternalFunction, "fn"),
            AstDeclaration::ExternalType(raw) => raw_metadata(raw, DeclarationKind::ExternalType, "type"),
            AstDeclaration::TypeAlias(raw) => raw_metadata(raw, DeclarationKind::TypeAlias, "type"),
            AstDeclaration::TypeDefinition(raw) => raw_metadata(raw, DeclarationKind::TypeDefinition, "type"),
            AstDeclaration::Attribute(raw) => raw_metadata(raw, DeclarationKind::Attribute, "@"),
            AstDeclaration::TargetGroup(raw) => raw_metadata(raw, DeclarationKind::TargetGroup, "target"),
            AstDeclaration::Statement(raw) => raw_metadata(raw, DeclarationKind::Statement, ""),
        }
    }
}

fn raw_metadata(raw: &ast::RawSyntax, kind: DeclarationKind, keyword: &str) -> DeclarationMetadata {
    DeclarationMetadata {
        name: declaration_name(&raw.source, keyword),
        kind,
        visibility: visibility(is_public_declaration(&raw.source)),
        span: raw.span,
    }
}

fn lower_constant(id: ConstantId, raw: &ast::RawSyntax) -> Constant {
    let name = declaration_name(&raw.source, "const").unwrap_or_else(|| format!("__constant_{}", id.0));
    Constant {
        id,
        name,
        public: is_public_declaration(&raw.source),
        value: constant_value(&raw.source),
        span: raw.span,
    }
}

fn constant_value(source: &str) -> ConstantValue {
    let Some(value) = source.split_once('=').map(|(_, value)| value.trim()) else {
        return ConstantValue::Raw(source.trim().into());
    };
    if value == "True" || value == "False" {
        return ConstantValue::Literal(Literal { kind: LiteralKind::Bool, source: value.into() });
    }
    if value == "Nil" {
        return ConstantValue::Literal(Literal { kind: LiteralKind::Nil, source: value.into() });
    }
    if value.starts_with('"') && value.ends_with('"') {
        return ConstantValue::Literal(Literal { kind: LiteralKind::String, source: value.into() });
    }
    if value.parse::<i64>().is_ok() {
        return ConstantValue::Literal(Literal { kind: LiteralKind::Int, source: value.into() });
    }
    if value.parse::<f64>().is_ok() && value.contains('.') {
        return ConstantValue::Literal(Literal { kind: LiteralKind::Float, source: value.into() });
    }
    ConstantValue::Raw(value.into())
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

fn exported_type_names(source: &str) -> Vec<String> {
    declaration_name(source, "type").into_iter().collect()
}

fn is_public_declaration(source: &str) -> bool {
    source.trim_start().starts_with("pub ")
}

fn visibility(public: bool) -> Visibility {
    if public { Visibility::Public } else { Visibility::Private }
}

impl From<&SymbolKind> for ReferenceKind {
    fn from(kind: &SymbolKind) -> Self {
        match kind {
            SymbolKind::Function { .. } => Self::Function,
            SymbolKind::Import { .. } => Self::Import,
            SymbolKind::Imported { .. } => Self::Imported,
            SymbolKind::Parameter => Self::Parameter,
            SymbolKind::Local => Self::Local,
            SymbolKind::Type => Self::Type,
            SymbolKind::Constructor => Self::Constructor,
            SymbolKind::Field => Self::Field,
            SymbolKind::Prelude => Self::Prelude,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{SourceFile, SourceFileId};
    use crate::{ast, parse, resolve, types, wasm};

    fn lower_source(source: &str) -> Module {
        let source = SourceFile::new(SourceFileId(0), source);
        let cst = parse::parse(source).expect("parse source");
        let ast = ast::build(cst).expect("build ast");
        let resolved = resolve::resolve(ast).expect("resolve names");
        let typed = types::check(resolved).expect("type check source");
        lower(typed).expect("lower source")
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
        let module = lower_source(include_str!("../../../fixtures/ir/core_control_flow.gleam"));
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
        let module = lower_source(include_str!("../../../fixtures/ir/core_control_flow.gleam"));

        insta::assert_debug_snapshot!("core_control_flow_ir", module);
    }

    #[test]
    fn reports_spanned_diagnostics_for_typed_ir_the_backend_cannot_emit() {
        let module = lower_source("type Box { Box }\nfn main() { Box }");
        let diagnostics = wasm::emit_wat(&module).expect_err("managed constructor cannot emit yet");

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.labels.iter().any(|label| label.span.start > 0))
        );
    }

    #[test]
    fn lowers_function_values_and_indirect_calls() {
        let module = lower_source("fn apply(x: Int, f: fn(Int) -> Int) -> Int { f(x) }");
        let apply = &module.functions[0];
        assert!(matches!(apply.body.result.kind, ExpressionKind::IndirectCall(_)));
        assert_eq!(apply.abi.params.len(), 2);
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

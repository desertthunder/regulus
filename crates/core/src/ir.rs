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
    pub body: Block,
    pub span: Span,
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
    Call {
        function: String,
        arguments: Vec<Expression>,
    },
    Branch(Branch),
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchClause {
    pub patterns: Vec<IrPattern>,
    pub guard: Option<Expression>,
    pub body: Box<Expression>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrPattern {
    Discard,
    Binding(LocalId),
    Alias { pattern: Box<IrPattern>, local: LocalId },
    Literal(Literal),
}

pub fn lower(module: TypedModule) -> Result<Module, Diagnostics> {
    Lowerer::new(module).lower()
}

struct Lowerer {
    module: TypedModule,
    function_types: HashMap<String, Type>,
    diagnostics: Diagnostics,
}

impl Lowerer {
    fn new(module: TypedModule) -> Self {
        let function_types = module
            .functions
            .iter()
            .map(|function| (function.name.text.clone(), function.type_.clone()))
            .collect();
        Self { module, function_types, diagnostics: Vec::new() }
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
        let return_type = match self.function_types.get(&function.name.text)? {
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
            body,
            span: function.span,
        })
    }

    fn lower_block(&mut self, context: &mut FunctionContext, block: &ast::Block) -> Option<Block> {
        context.push_scope();
        let mut instructions = Vec::new();
        let mut result = self.nil_expression(block.span);

        for statement in &block.statements {
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
                Statement::Expression(expression) => result = self.lower_expression(context, expression)?,
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
                let local = context.lookup(&name.text)?;
                let type_ = context.local(local).type_.clone();
                Some(Expression { type_, span: name.span, kind: ExpressionKind::LocalGet(local) })
            }
            AstExpression::Call(call) => {
                let AstExpression::Variable(function_name) = call.function.as_ref() else {
                    self.diagnostics.push(
                        Diagnostic::new(
                            DiagnosticCode::LoweringError,
                            "only direct function calls can be lowered",
                        )
                        .with_label(Label::primary(call.span, "unsupported call here")),
                    );
                    return None;
                };
                let Type::Function { return_type, .. } = self.function_types.get(&function_name.text)?.clone() else {
                    return None;
                };
                let arguments = call
                    .arguments
                    .iter()
                    .map(|argument| self.lower_expression(context, &argument.value))
                    .collect::<Option<Vec<_>>>()?;
                Some(Expression {
                    type_: *return_type,
                    span: call.span,
                    kind: ExpressionKind::Call { function: function_name.text.clone(), arguments },
                })
            }
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
                    let body = self.lower_expression(context, &clause.value)?;
                    context.pop_scope();
                    type_ = body.type_.clone();
                    clauses.push(BranchClause { patterns: patterns?, guard, body: Box::new(body), span: clause.span });
                }
                Some(Expression { type_, span: case.span, kind: ExpressionKind::Branch(Branch { subjects, clauses }) })
            }
            AstExpression::FieldAccess(field_access) => {
                self.diagnostics.push(
                    Diagnostic::new(DiagnosticCode::LoweringError, "field access cannot be lowered")
                        .with_label(Label::primary(field_access.span, "unsupported field access here")),
                );
                None
            }
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
                self.unsupported_pattern(tuple.span, "tuple pattern");
                None
            }
            Pattern::List(list) => {
                self.unsupported_pattern(list.span, "list pattern");
                None
            }
            Pattern::Constructor(constructor) => {
                self.unsupported_pattern(constructor.span, "constructor pattern");
                None
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
            IrPattern::Discard | IrPattern::Literal(_) => {
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

fn type_for_literal(kind: LiteralKind) -> Type {
    match kind {
        LiteralKind::Int => Type::Int,
        LiteralKind::Float => Type::Float,
        LiteralKind::String => Type::String,
        LiteralKind::Bool => Type::Bool,
        LiteralKind::Nil => Type::Nil,
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
    use crate::{ast, parse, resolve, types};

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

        assert!(matches!(main.body.result.kind, ExpressionKind::Call { ref function, .. } if function == "id"));
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
    fn core_ir_debug_output_is_deterministic() {
        let module = lower_source("fn id(x: Int) -> Int { x }");

        assert_eq!(format!("{module:#?}"), format!("{module:#?}"));
    }
}

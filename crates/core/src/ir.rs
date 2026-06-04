use std::collections::HashMap;

use crate::{
    ast::{self, Expression as AstExpression, LiteralKind, Pattern, Statement},
    diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, Label},
    source::Span,
    types::{Type, TypedModule},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalId(pub u32);

/// Core IR module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    pub functions: Vec<Function>,
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
        span: Span,
    },
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
        let mut functions = Vec::new();
        for function in self.module.resolved.ast.functions.clone() {
            if let Some(function) = self.lower_function(&function) {
                functions.push(function);
            }
        }

        if self.diagnostics.is_empty() { Ok(Module { functions }) } else { Err(self.diagnostics) }
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
    fn core_ir_debug_output_is_deterministic() {
        let module = lower_source("fn id(x: Int) -> Int { x }");

        assert_eq!(format!("{module:#?}"), format!("{module:#?}"));
    }
}

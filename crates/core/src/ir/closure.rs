mod remapping;

use super::{
    AbiValue, AnonymousFunction, Block, CallAbi, CallArgument, CallBoundary, Expression, ExpressionKind, Function,
    FunctionContext, IndirectCall, Local, Lowerer, Type, abi_return, ast, call_abi,
};
use remapping::{LiftedLocalPolicy, captured_locals, lift_closure_body};

pub fn lower_anonymous_function(
    lowerer: &mut Lowerer, context: &mut FunctionContext, function: &ast::AnonymousFunction,
) -> Option<Expression> {
    let name = lowerer.next_anonymous_name();
    let outer_local_count = context.locals.len();
    context.push_scope();
    let mut original_params = Vec::new();
    let inferred_params = match lowerer.typed_expression_type(function.span) {
        Some(Type::Function { params, .. }) => params,
        _ => Vec::new(),
    };
    for (index, parameter) in function.parameters.iter().enumerate() {
        let Some(name) = &parameter.name else { continue };
        let type_ = parameter
            .type_annotation
            .as_ref()
            .and_then(|annotation| Type::from_source(&annotation.source))
            .or_else(|| inferred_params.get(index).cloned())
            .unwrap_or(Type::Nil);
        let local = context.allocate(name, type_);
        context.bind(name.text.clone(), local.id);
        original_params.push(local);
    }
    let mut body = lowerer.lower_block(context, &function.body)?;
    context.pop_scope();

    let captures = captured_locals(context, &body, outer_local_count);
    let type_ = lowerer
        .typed_expression_type(function.span)
        .unwrap_or_else(|| Type::Function {
            params: original_params.iter().map(|param| param.type_.clone()).collect(),
            return_type: Box::new(body.result.type_.clone()),
        });
    let lifted = lift_closure_body(
        context,
        &mut body,
        outer_local_count,
        &original_params,
        &captures,
        LiftedLocalPolicy::IncludeBodyLocals,
    );

    let return_type = match &type_ {
        Type::Function { return_type, .. } => *return_type.clone(),
        _ => body.result.type_.clone(),
    };
    lowerer.lifted_functions.push(Function {
        name: name.clone(),
        public: false,
        closure_captures: captures.iter().map(|capture| capture.type_.clone()).collect(),
        params: lifted.params,
        locals: lifted.locals,
        return_type: return_type.clone(),
        abi: call_abi(
            &Type::Function {
                params: captures
                    .iter()
                    .map(|capture| capture.type_.clone())
                    .chain(lifted.original_params.iter().map(|param| param.type_.clone()))
                    .collect(),
                return_type: Box::new(return_type),
            },
            CallBoundary::Internal,
        ),
        body,
        span: function.span,
    });

    context.locals.truncate(outer_local_count);
    Some(Expression {
        type_: type_.clone(),
        span: function.span,
        kind: ExpressionKind::AnonymousFunction(AnonymousFunction {
            name,
            params: original_params,
            captures,
            abi: call_abi(&type_, CallBoundary::Internal),
            body: empty_body(lowerer, function.span),
        }),
    })
}

pub fn lower_capture(
    lowerer: &mut Lowerer, context: &mut FunctionContext, capture: &ast::Capture,
) -> Option<Expression> {
    let function = lowerer.lower_expression(context, &capture.function)?;
    let Type::Function { params, return_type } = function.type_.clone() else {
        return None;
    };
    let outer_local_count = context.locals.len();
    context.push_scope();
    let mut callback_params = Vec::new();
    let mut call_arguments = Vec::new();
    for (index, param_type) in params.iter().enumerate() {
        match capture.arguments.get(index).and_then(Option::as_ref) {
            Some(argument) => call_arguments.push(CallArgument {
                label: argument.label.as_ref().map(|label| label.text.clone()),
                value: lowerer.lower_expression(context, &argument.value)?,
                span: argument.span,
            }),
            None => {
                let name = ast::Name { span: capture.span, text: format!("_capture_{index}") };
                let local = context.allocate(&name, param_type.clone());
                context.bind(name.text.clone(), local.id);
                call_arguments.push(CallArgument {
                    label: None,
                    value: Expression {
                        type_: param_type.clone(),
                        span: capture.span,
                        kind: ExpressionKind::LocalGet(local.id),
                    },
                    span: capture.span,
                });
                callback_params.push(local);
            }
        }
    }
    let mut body = Block {
        instructions: Vec::new(),
        result: Box::new(Expression {
            type_: *return_type.clone(),
            span: capture.span,
            kind: ExpressionKind::IndirectCall(IndirectCall {
                callee: Box::new(function),
                arguments: call_arguments,
                abi: CallAbi {
                    params: params.iter().map(AbiValue::from).collect(),
                    return_: abi_return(&return_type),
                    boundary: CallBoundary::Internal,
                },
            }),
        }),
        span: capture.span,
    };
    context.pop_scope();
    let type_ =
        Type::Function { params: callback_params.iter().map(|param| param.type_.clone()).collect(), return_type };

    let name = lowerer.next_anonymous_name();
    let captures = captured_locals(context, &body, outer_local_count);
    let lifted = lift_closure_body(
        context,
        &mut body,
        outer_local_count,
        &callback_params,
        &captures,
        LiftedLocalPolicy::ParamsOnly,
    );
    lowerer.lifted_functions.push(Function {
        name: name.clone(),
        public: false,
        closure_captures: captures.iter().map(|capture| capture.type_.clone()).collect(),
        params: lifted.locals.clone(),
        locals: lifted.locals,
        return_type: body.result.type_.clone(),
        abi: call_abi(
            &Type::Function {
                params: captures
                    .iter()
                    .map(|capture| capture.type_.clone())
                    .chain(lifted.original_params.iter().map(|param| param.type_.clone()))
                    .collect(),
                return_type: Box::new(body.result.type_.clone()),
            },
            CallBoundary::Internal,
        ),
        body,
        span: capture.span,
    });

    context.locals.truncate(outer_local_count);
    Some(Expression {
        type_: type_.clone(),
        span: capture.span,
        kind: ExpressionKind::AnonymousFunction(AnonymousFunction {
            name,
            params: callback_params,
            captures,
            abi: call_abi(&type_, CallBoundary::Internal),
            body: empty_body(lowerer, capture.span),
        }),
    })
}

pub fn lower_synthetic_anonymous_function(
    lowerer: &mut Lowerer, context: &mut FunctionContext, span: super::Span, outer_local_count: usize,
    original_params: Vec<Local>, mut body: Block, type_: &Type,
) -> Expression {
    let name = lowerer.next_anonymous_name();
    let captures = captured_locals(context, &body, outer_local_count);
    let lifted = lift_closure_body(
        context,
        &mut body,
        outer_local_count,
        &original_params,
        &captures,
        LiftedLocalPolicy::IncludeBodyLocals,
    );
    let return_type = match &type_ {
        Type::Function { return_type, .. } => *return_type.clone(),
        _ => body.result.type_.clone(),
    };
    lowerer.lifted_functions.push(Function {
        name: name.clone(),
        public: false,
        closure_captures: captures.iter().map(|capture| capture.type_.clone()).collect(),
        params: lifted.params,
        locals: lifted.locals,
        return_type: return_type.clone(),
        abi: call_abi(
            &Type::Function {
                params: captures
                    .iter()
                    .map(|capture| capture.type_.clone())
                    .chain(lifted.original_params.iter().map(|param| param.type_.clone()))
                    .collect(),
                return_type: Box::new(return_type),
            },
            CallBoundary::Internal,
        ),
        body,
        span,
    });
    context.locals.truncate(outer_local_count);
    Expression {
        type_: type_.clone(),
        span,
        kind: ExpressionKind::AnonymousFunction(AnonymousFunction {
            name,
            params: original_params,
            captures,
            abi: call_abi(type_, CallBoundary::Internal),
            body: empty_body(lowerer, span),
        }),
    }
}

fn empty_body(lowerer: &Lowerer, span: super::Span) -> Block {
    Block { instructions: Vec::new(), result: Box::new(lowerer.nil_expression(span)), span }
}

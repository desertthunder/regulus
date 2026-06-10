use std::collections::HashMap;

use crate::ir::{
    Block, Capture, Expression, ExpressionKind, FunctionContext, Instruction, IrPattern, Local, LocalId,
    MemoryOperation,
};

pub struct LiftedLocals {
    pub params: Vec<Local>,
    pub locals: Vec<Local>,
    pub original_params: Vec<Local>,
}

#[derive(Clone, Copy)]
pub enum LiftedLocalPolicy {
    IncludeBodyLocals,
    ParamsOnly,
}

pub fn lift_closure_body(
    context: &FunctionContext, body: &mut Block, outer_local_count: usize, original_params: &[Local],
    captures: &[Capture], policy: LiftedLocalPolicy,
) -> LiftedLocals {
    let mut locals = Vec::new();
    let mut remap = HashMap::new();
    let mut lifted_params = Vec::new();

    for capture in captures {
        let id = LocalId(locals.len() as u32);
        remap.insert(capture.source, id);
        locals.push(Local { id, name: capture.name.clone(), type_: capture.type_.clone(), span: capture.span });
    }
    for param in original_params {
        let id = LocalId(locals.len() as u32);
        remap.insert(param.id, id);
        let local = Local { id, name: param.name.clone(), type_: param.type_.clone(), span: param.span };
        lifted_params.push(local.clone());
        locals.push(local);
    }
    if matches!(policy, LiftedLocalPolicy::IncludeBodyLocals) {
        for local in context.locals.iter().skip(outer_local_count + original_params.len()) {
            let id = LocalId(locals.len() as u32);
            remap.insert(local.id, id);
            locals.push(Local { id, name: local.name.clone(), type_: local.type_.clone(), span: local.span });
        }
    }
    remap_block_locals(body, &remap);
    let params = locals
        .iter()
        .take(captures.len() + lifted_params.len())
        .cloned()
        .collect();
    LiftedLocals { params, locals, original_params: lifted_params }
}

pub fn captured_locals(context: &FunctionContext, body: &Block, outer_local_count: usize) -> Vec<Capture> {
    let mut used = Vec::new();
    collect_block_locals(body, &mut used);
    let mut captures = Vec::new();
    for id in used {
        if (id.0 as usize) < outer_local_count && !captures.iter().any(|capture: &Capture| capture.source == id) {
            let local = context.local(id).clone();
            captures.push(Capture { source: id, name: local.name, type_: local.type_, span: local.span });
        }
    }
    captures
}

fn collect_block_locals(block: &Block, locals: &mut Vec<LocalId>) {
    for instruction in &block.instructions {
        match instruction {
            Instruction::Evaluate { expression, .. } => collect_expression_locals(expression, locals),
            Instruction::LocalSet { local, value, .. } => {
                collect_local(*local, locals);
                collect_expression_locals(value, locals);
            }
            Instruction::AssertMatch { value, pattern, .. } => {
                collect_expression_locals(value, locals);
                collect_pattern_locals(pattern, locals);
            }
        }
    }
    collect_expression_locals(&block.result, locals);
}

fn collect_local(local: LocalId, locals: &mut Vec<LocalId>) {
    if !locals.contains(&local) {
        locals.push(local);
    }
}

fn collect_expression_locals(expression: &Expression, locals: &mut Vec<LocalId>) {
    match &expression.kind {
        ExpressionKind::LocalGet(id) => collect_local(*id, locals),
        ExpressionKind::DirectCall(call) => call
            .arguments
            .iter()
            .for_each(|arg| collect_expression_locals(&arg.value, locals)),
        ExpressionKind::IndirectCall(call) => {
            collect_expression_locals(&call.callee, locals);
            call.arguments
                .iter()
                .for_each(|arg| collect_expression_locals(&arg.value, locals));
        }
        ExpressionKind::AnonymousFunction(function) => {
            for capture in &function.captures {
                if !locals.contains(&capture.source) {
                    locals.push(capture.source);
                }
            }
            collect_block_locals(&function.body, locals);
        }
        ExpressionKind::Pipeline(pipeline) => {
            collect_expression_locals(&pipeline.input, locals);
            collect_expression_locals(&pipeline.call, locals);
        }
        ExpressionKind::Use(use_) => {
            collect_expression_locals(&use_.callback, locals);
            collect_expression_locals(&use_.call, locals);
        }
        ExpressionKind::Branch(branch) => {
            branch
                .subjects
                .iter()
                .for_each(|subject| collect_expression_locals(subject, locals));

            branch.clauses.iter().for_each(|clause| {
                clause
                    .patterns
                    .iter()
                    .for_each(|pattern| collect_pattern_locals(pattern, locals));
                clause
                    .bindings
                    .iter()
                    .for_each(|binding| collect_local(binding.local, locals));

                if let Some(guard) = &clause.guard {
                    collect_expression_locals(guard, locals);
                }
                collect_expression_locals(&clause.body, locals);
            });
        }
        ExpressionKind::Tuple(items) | ExpressionKind::List(items) => {
            items.iter().for_each(|item| collect_expression_locals(item, locals))
        }
        ExpressionKind::BitArrayConcat { left, right }
        | ExpressionKind::Compare { left, right, .. }
        | ExpressionKind::RuntimeEquality { left, right } => {
            collect_expression_locals(left, locals);
            collect_expression_locals(right, locals);
        }
        ExpressionKind::BitStringDeconstruct { bit_array, .. } => collect_expression_locals(bit_array, locals),
        ExpressionKind::Record(record) => record
            .fields
            .iter()
            .for_each(|field| collect_expression_locals(&field.value, locals)),
        ExpressionKind::Constructor(constructor) => constructor
            .arguments
            .iter()
            .for_each(|arg| collect_expression_locals(arg, locals)),
        ExpressionKind::FieldAccess { record, .. } => collect_expression_locals(record, locals),
        ExpressionKind::RecordUpdate { record, fields, .. } => {
            collect_expression_locals(record, locals);
            fields.iter().for_each(|field| {
                if let Some(value) = &field.value {
                    collect_expression_locals(value, locals);
                }
            });
        }
        ExpressionKind::ListCons { head, tail } => {
            collect_expression_locals(head, locals);
            collect_expression_locals(tail, locals);
        }
        ExpressionKind::ListDeconstruct { list, .. } => collect_expression_locals(list, locals),
        ExpressionKind::TupleElement { tuple, .. } => collect_expression_locals(tuple, locals),
        ExpressionKind::Memory(operation) => match operation {
            MemoryOperation::Allocate { bytes } => collect_expression_locals(bytes, locals),
            MemoryOperation::Load { address, .. } => collect_expression_locals(address, locals),
            MemoryOperation::Store { address, value } => {
                collect_expression_locals(address, locals);
                collect_expression_locals(value, locals);
            }
        },
        _ => {}
    }
}

fn collect_pattern_locals(pattern: &IrPattern, locals: &mut Vec<LocalId>) {
    match pattern {
        IrPattern::Binding(local) => collect_local(*local, locals),
        IrPattern::Alias { pattern, local } => {
            collect_pattern_locals(pattern, locals);
            collect_local(*local, locals);
        }
        IrPattern::Tuple(elements) => elements
            .iter()
            .for_each(|element| collect_pattern_locals(element, locals)),
        IrPattern::List { elements, tail } => {
            elements
                .iter()
                .for_each(|element| collect_pattern_locals(element, locals));
            if let Some(tail) = tail {
                collect_local(*tail, locals);
            }
        }
        IrPattern::Constructor { arguments, .. } => arguments
            .iter()
            .for_each(|argument| collect_pattern_locals(&argument.pattern, locals)),
        IrPattern::BitString(segments) => segments.iter().for_each(|segment| {
            if let Some(binding) = segment.binding {
                collect_local(binding, locals);
            }
        }),
        IrPattern::Discard | IrPattern::Literal(_) => {}
    }
}

fn remap_block_locals(block: &mut Block, remap: &HashMap<LocalId, LocalId>) {
    for instruction in &mut block.instructions {
        match instruction {
            Instruction::Evaluate { expression, .. } => remap_expression_locals(expression, remap),
            Instruction::LocalSet { local, value, .. } => {
                if let Some(id) = remap.get(local) {
                    *local = *id;
                }
                remap_expression_locals(value, remap);
            }
            Instruction::AssertMatch { value, pattern, .. } => {
                remap_expression_locals(value, remap);
                remap_pattern_locals(pattern, remap);
            }
        }
    }
    remap_expression_locals(&mut block.result, remap);
}

fn remap_pattern_locals(pattern: &mut IrPattern, remap: &HashMap<LocalId, LocalId>) {
    match pattern {
        IrPattern::Binding(local) => {
            if let Some(id) = remap.get(local) {
                *local = *id;
            }
        }
        IrPattern::Alias { pattern, local } => {
            remap_pattern_locals(pattern, remap);
            if let Some(id) = remap.get(local) {
                *local = *id;
            }
        }
        IrPattern::Tuple(items) => items.iter_mut().for_each(|item| remap_pattern_locals(item, remap)),
        IrPattern::List { elements, tail } => {
            elements.iter_mut().for_each(|item| remap_pattern_locals(item, remap));
            if let Some(local) = tail
                && let Some(id) = remap.get(local)
            {
                *local = *id;
            }
        }
        IrPattern::Constructor { arguments, .. } => arguments
            .iter_mut()
            .for_each(|arg| remap_pattern_locals(&mut arg.pattern, remap)),
        IrPattern::BitString(segments) => segments.iter_mut().for_each(|segment| {
            if let Some(local) = &mut segment.binding
                && let Some(id) = remap.get(local)
            {
                *local = *id;
            }
        }),
        _ => {}
    }
}

fn remap_expression_locals(expression: &mut Expression, remap: &HashMap<LocalId, LocalId>) {
    match &mut expression.kind {
        ExpressionKind::LocalGet(id) => {
            if let Some(new) = remap.get(id) {
                *id = *new;
            }
        }
        ExpressionKind::DirectCall(call) => call
            .arguments
            .iter_mut()
            .for_each(|arg| remap_expression_locals(&mut arg.value, remap)),
        ExpressionKind::IndirectCall(call) => {
            remap_expression_locals(&mut call.callee, remap);
            call.arguments
                .iter_mut()
                .for_each(|arg| remap_expression_locals(&mut arg.value, remap));
        }
        ExpressionKind::AnonymousFunction(function) => {
            for capture in &mut function.captures {
                if let Some(id) = remap.get(&capture.source) {
                    capture.source = *id;
                }
            }
            for param in &mut function.params {
                if let Some(id) = remap.get(&param.id) {
                    param.id = *id;
                }
            }
            remap_block_locals(&mut function.body, remap);
        }
        ExpressionKind::Branch(branch) => {
            branch
                .subjects
                .iter_mut()
                .for_each(|subject| remap_expression_locals(subject, remap));
            branch.clauses.iter_mut().for_each(|clause| {
                clause
                    .patterns
                    .iter_mut()
                    .for_each(|pattern| remap_pattern_locals(pattern, remap));
                clause.bindings.iter_mut().for_each(|binding| {
                    if let Some(id) = remap.get(&binding.local) {
                        binding.local = *id;
                    }
                });
                if let Some(guard) = &mut clause.guard {
                    remap_expression_locals(guard, remap);
                }
                remap_expression_locals(&mut clause.body, remap);
            });
        }
        ExpressionKind::Tuple(items) | ExpressionKind::List(items) => {
            items.iter_mut().for_each(|item| remap_expression_locals(item, remap))
        }
        ExpressionKind::Record(record) => record
            .fields
            .iter_mut()
            .for_each(|field| remap_expression_locals(&mut field.value, remap)),
        ExpressionKind::Constructor(constructor) => constructor
            .arguments
            .iter_mut()
            .for_each(|arg| remap_expression_locals(arg, remap)),
        ExpressionKind::FieldAccess { record, .. } => remap_expression_locals(record, remap),
        ExpressionKind::TupleElement { tuple, .. } => remap_expression_locals(tuple, remap),
        ExpressionKind::Compare { left, right, .. }
        | ExpressionKind::RuntimeEquality { left, right }
        | ExpressionKind::BitArrayConcat { left, right } => {
            remap_expression_locals(left, remap);
            remap_expression_locals(right, remap);
        }
        ExpressionKind::Pipeline(pipeline) => {
            remap_expression_locals(&mut pipeline.input, remap);
            remap_expression_locals(&mut pipeline.call, remap);
        }
        ExpressionKind::Use(use_) => {
            remap_expression_locals(&mut use_.callback, remap);
            remap_expression_locals(&mut use_.call, remap);
        }
        ExpressionKind::RecordUpdate { record, fields, .. } => {
            remap_expression_locals(record, remap);
            fields.iter_mut().for_each(|field| {
                if let Some(value) = &mut field.value {
                    remap_expression_locals(value, remap);
                }
            });
        }
        ExpressionKind::ListCons { head, tail } => {
            remap_expression_locals(head, remap);
            remap_expression_locals(tail, remap);
        }
        ExpressionKind::ListDeconstruct { list, .. } => remap_expression_locals(list, remap),
        ExpressionKind::BitStringDeconstruct { bit_array, .. } => remap_expression_locals(bit_array, remap),
        ExpressionKind::Memory(operation) => match operation {
            MemoryOperation::Allocate { bytes } => remap_expression_locals(bytes, remap),
            MemoryOperation::Load { address, .. } => remap_expression_locals(address, remap),
            MemoryOperation::Store { address, value } => {
                remap_expression_locals(address, remap);
                remap_expression_locals(value, remap);
            }
        },
        _ => {}
    }
}

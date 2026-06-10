//! Validation for the compiler-owned WebAssembly model.
//!
//! The validator checks module references and instruction semantics before
//! binary emission: stack effects, branch labels/results, locals, function and
//! call signatures, memories, tables, exports, imports, and data offsets.

use super::builder::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidationError {
    pub(crate) message: String,
}

impl ValidationError {
    fn new(message: impl Into<String>) -> Self {
        Self { message: message.into() }
    }
}

pub(crate) type ValidationResult<T> = Result<T, Vec<ValidationError>>;

pub(super) struct Validator<'a> {
    module: &'a Module,
    errors: Vec<ValidationError>,
}

impl<'a> Validator<'a> {
    pub(super) fn new(module: &'a Module) -> Self {
        Self { module, errors: Vec::new() }
    }

    pub(super) fn validate(mut self) -> ValidationResult<()> {
        self.validate_module();
        if self.errors.is_empty() { Ok(()) } else { Err(self.errors) }
    }

    fn validate_module(&mut self) {
        for (id, import) in self.module.imports.iter().enumerate() {
            self.validate_import(id, import);
        }
        for export in &self.module.exports {
            self.validate_export(export);
        }
        for segment in &self.module.data_segments {
            if self.memory_type(segment.memory).is_none() {
                self.error(format!("unknown memory index {} in data segment", segment.memory.0));
            }
            let mut context = FunctionContext::constant_expression();
            if self.validate_sequence(&segment.offset, &mut context, vec![ValueType::I32]) != Some(vec![ValueType::I32])
            {
                self.error("data segment offset must leave one i32 on the stack");
            }
        }
        for (defined_index, function) in self.module.functions.iter().enumerate() {
            self.validate_function(defined_index, function);
        }
    }

    fn validate_import(&mut self, id: usize, import: &Import) {
        match &import.desc {
            ImportDesc::Function(type_id) => {
                if self.type_(type_id).is_none() {
                    self.error(format!("import {id} references unknown function type {}", type_id.0));
                }
            }
            ImportDesc::Memory(memory) => self.validate_limits(
                memory.minimum_pages,
                memory.maximum_pages,
                format!("imported memory {id}"),
            ),
            ImportDesc::Table(table) => {
                self.validate_limits(table.minimum, table.maximum, format!("imported table {id}"))
            }
        }
    }

    fn validate_export(&mut self, export: &Export) {
        match export.desc {
            ExportDesc::Function(id) => {
                if self.function_type(id).is_none() {
                    self.error(format!(
                        "export `{}` references unknown function index {}",
                        export.name, id.0
                    ));
                }
            }
            ExportDesc::Memory(id) => {
                if self.memory_type(id).is_none() {
                    self.error(format!(
                        "export `{}` references unknown memory index {}",
                        export.name, id.0
                    ));
                }
            }
            ExportDesc::Table(id) => {
                if self.table_type(id).is_none() {
                    self.error(format!(
                        "export `{}` references unknown table index {}",
                        export.name, id.0
                    ));
                }
            }
        }
    }

    fn validate_function(&mut self, defined_index: usize, function: &Function) {
        let Some(type_) = self.type_(&function.type_id).cloned() else {
            self.error(format!(
                "function {defined_index} references unknown type {}",
                function.type_id.0
            ));
            return;
        };

        let params = function.params.iter().map(|local| local.type_).collect::<Vec<_>>();
        if params != type_.params {
            self.error(format!(
                "function {defined_index} params {:?} do not match signature {:?}",
                params, type_.params
            ));
        }

        let mut context = FunctionContext::new(
            function
                .params
                .iter()
                .chain(&function.locals)
                .map(|local| local.type_)
                .collect(),
            type_.results.clone(),
        );
        let actual = self.validate_sequence(&function.body, &mut context, type_.results.clone());
        if let Some(actual) = actual
            && actual != type_.results
        {
            self.error(format!(
                "function {defined_index} leaves stack {:?}, expected {:?}",
                actual, type_.results
            ));
        }
    }

    fn validate_sequence(
        &mut self, instructions: &[Instruction], context: &mut FunctionContext, expected: Vec<ValueType>,
    ) -> Option<Vec<ValueType>> {
        self.validate_sequence_from(instructions, context, Vec::new(), expected)
    }

    fn validate_sequence_from(
        &mut self, instructions: &[Instruction], context: &mut FunctionContext, mut stack: Vec<ValueType>,
        expected: Vec<ValueType>,
    ) -> Option<Vec<ValueType>> {
        let mut unreachable = false;
        for instruction in instructions {
            if unreachable {
                stack.clear();
            }
            if !self.validate_instruction(instruction, context, &mut stack, unreachable) {
                unreachable = true;
            }
        }

        if unreachable { Some(expected) } else { Some(stack) }
    }

    fn validate_instruction(
        &mut self, instruction: &Instruction, context: &mut FunctionContext, stack: &mut Vec<ValueType>,
        unreachable: bool,
    ) -> bool {
        if !self.validate_instruction_references(instruction, context) {
            return true;
        }

        match instruction {
            Instruction::Block { type_, body } => {
                self.validate_block_type(type_);
                if !self.pop_types(stack, &type_.params, instruction, unreachable) {
                    return true;
                }
                context.labels.push(LabelType { branch_results: type_.results.clone() });
                let actual = self.validate_sequence_from(body, context, type_.params.clone(), type_.results.clone());
                context.labels.pop();
                if let Some(actual) = actual
                    && actual != type_.results
                {
                    self.error(format!("block leaves stack {actual:?}, expected {:?}", type_.results));
                }
                stack.extend(type_.results.clone());
                true
            }
            Instruction::Loop { type_, body } => {
                self.validate_block_type(type_);
                if !self.pop_types(stack, &type_.params, instruction, unreachable) {
                    return true;
                }
                context.labels.push(LabelType { branch_results: type_.params.clone() });
                let actual = self.validate_sequence_from(body, context, type_.params.clone(), type_.results.clone());
                context.labels.pop();
                if let Some(actual) = actual
                    && actual != type_.results
                {
                    self.error(format!("loop leaves stack {actual:?}, expected {:?}", type_.results));
                }
                stack.extend(type_.results.clone());
                true
            }
            Instruction::If { type_, then_body, else_body } => {
                self.validate_block_type(type_);
                let mut consumes = type_.params.clone();
                consumes.push(ValueType::I32);
                if !self.pop_types(stack, &consumes, instruction, unreachable) {
                    return true;
                }

                context.labels.push(LabelType { branch_results: type_.results.clone() });
                let then_actual =
                    self.validate_sequence_from(then_body, context, type_.params.clone(), type_.results.clone());
                let else_actual =
                    self.validate_sequence_from(else_body, context, type_.params.clone(), type_.results.clone());
                context.labels.pop();

                if let Some(actual) = then_actual
                    && actual != type_.results
                {
                    self.error(format!(
                        "if then branch leaves stack {actual:?}, expected {:?}",
                        type_.results
                    ));
                }
                if let Some(actual) = else_actual
                    && actual != type_.results
                {
                    self.error(format!(
                        "if else branch leaves stack {actual:?}, expected {:?}",
                        type_.results
                    ));
                }
                stack.extend(type_.results.clone());
                true
            }
            Instruction::Br { depth, results } => {
                self.validate_branch(*depth, results, context);
                self.pop_types(stack, results, instruction, unreachable);
                false
            }
            Instruction::BrIf { depth, results } => {
                self.validate_branch(*depth, results, context);
                self.apply_stack_effect(stack, instruction, unreachable);
                true
            }
            Instruction::Return { results } => {
                if results != &context.return_types {
                    self.error(format!(
                        "return has results {results:?}, expected {:?}",
                        context.return_types
                    ));
                }
                self.pop_types(stack, results, instruction, unreachable);
                false
            }
            Instruction::Unreachable => false,
            _ => {
                self.apply_stack_effect(stack, instruction, unreachable);
                true
            }
        }
    }

    fn validate_instruction_references(&mut self, instruction: &Instruction, context: &FunctionContext) -> bool {
        let mut valid = true;
        match instruction {
            Instruction::Call { function, type_ } => match self.function_type(*function) {
                Some(actual) if actual == type_ => {}
                Some(actual) => {
                    self.error(format!(
                        "call to function {} has signature {type_:?}, expected {actual:?}",
                        function.0
                    ));
                    valid = false;
                }
                None => {
                    self.error(format!("call references unknown function index {}", function.0));
                    valid = false;
                }
            },
            Instruction::CallIndirect { table, type_id, type_ } => {
                if self.table_type(*table).is_none() {
                    self.error(format!("call_indirect references unknown table index {}", table.0));
                    valid = false;
                }
                match self.type_(type_id) {
                    Some(actual) if actual == type_ => {}
                    Some(actual) => {
                        self.error(format!(
                            "call_indirect type {type_id:?} is {actual:?}, instruction uses {type_:?}"
                        ));
                        valid = false;
                    }
                    None => {
                        self.error(format!("call_indirect references unknown type index {}", type_id.0));
                        valid = false;
                    }
                }
            }
            Instruction::LocalGet { local, type_ }
            | Instruction::LocalSet { local, type_ }
            | Instruction::LocalTee { local, type_ } => match context.locals.get(local.0 as usize) {
                Some(actual) if actual == type_ => {}
                Some(actual) => {
                    self.error(format!(
                        "local {} has type {actual:?}, instruction uses {type_:?}",
                        local.0
                    ));
                    valid = false;
                }
                None => {
                    self.error(format!("instruction references unknown local index {}", local.0));
                    valid = false;
                }
            },
            Instruction::I32Load(arg)
            | Instruction::I32Store(arg)
            | Instruction::I64Load(arg)
            | Instruction::I64Store(arg)
            | Instruction::F64Load(arg)
            | Instruction::F64Store(arg)
                if self.memory_type(arg.memory).is_none() =>
            {
                self.error(format!(
                    "memory instruction references unknown memory index {}",
                    arg.memory.0
                ));
                valid = false;
            }
            _ => {}
        }
        valid
    }

    fn apply_stack_effect(&mut self, stack: &mut Vec<ValueType>, instruction: &Instruction, unreachable: bool) {
        let effect = instruction.stack_effect();
        if self.pop_types(stack, &effect.consumes, instruction, unreachable) {
            stack.extend(effect.produces);
        }
    }

    fn pop_types(
        &mut self, stack: &mut Vec<ValueType>, expected: &[ValueType], instruction: &Instruction, unreachable: bool,
    ) -> bool {
        if unreachable {
            return true;
        }
        if stack.len() < expected.len() {
            self.error(format!(
                "stack underflow for {instruction:?}: expected {expected:?}, stack was {stack:?}"
            ));
            return false;
        }
        let start = stack.len() - expected.len();
        if stack[start..] != *expected {
            self.error(format!(
                "stack mismatch for {instruction:?}: expected top {expected:?}, stack was {stack:?}"
            ));
            return false;
        }
        stack.truncate(start);
        true
    }

    fn validate_block_type(&mut self, type_: &BlockType) {
        if (type_.params.len() > 0 || type_.results.len() > 1) && self.block_type_id(type_).is_none() {
            self.error(format!(
                "block type params {:?} results {:?} must be present in the module type section",
                type_.params, type_.results
            ));
        }
    }

    fn block_type_id(&self, type_: &BlockType) -> Option<TypeId> {
        let function_type = FunctionType::new(type_.params.clone(), type_.results.clone());
        self.module
            .types
            .iter()
            .position(|candidate| candidate == &function_type)
            .map(|index| TypeId(index as u32))
    }

    fn validate_branch(&mut self, depth: u32, results: &[ValueType], context: &FunctionContext) {
        match context.labels.iter().rev().nth(depth as usize) {
            Some(label) if label.branch_results == results => {}
            Some(label) => self.error(format!(
                "branch depth {depth} has results {results:?}, expected {:?}",
                label.branch_results
            )),
            None => self.error(format!("branch references unknown label depth {depth}")),
        }
    }

    fn validate_limits(&mut self, minimum: u32, maximum: Option<u32>, item: impl std::fmt::Display) {
        if let Some(maximum) = maximum
            && maximum < minimum
        {
            self.error(format!("{item} maximum {maximum} is below minimum {minimum}"));
        }
    }

    fn type_(&self, id: &TypeId) -> Option<&FunctionType> {
        self.module.types.get(id.0 as usize)
    }

    fn function_type(&self, id: FunctionId) -> Option<&FunctionType> {
        let imported_count = self.module.imported_function_count();
        let index = id.0 as usize;
        if index < imported_count {
            self.module
                .imports
                .iter()
                .filter_map(|import| match import.desc {
                    ImportDesc::Function(type_id) => self.type_(&type_id),
                    ImportDesc::Memory(_) | ImportDesc::Table(_) => None,
                })
                .nth(index)
        } else {
            let function = self.module.functions.get(index - imported_count)?;
            self.type_(&function.type_id)
        }
    }

    fn memory_type(&self, id: MemoryId) -> Option<&Memory> {
        let imported_count = self.module.imported_memory_count();
        let index = id.0 as usize;
        if index < imported_count {
            self.module
                .imports
                .iter()
                .filter_map(|import| match &import.desc {
                    ImportDesc::Memory(memory) => Some(memory),
                    ImportDesc::Function(_) | ImportDesc::Table(_) => None,
                })
                .nth(index)
        } else {
            self.module.memories.get(index - imported_count)
        }
    }

    fn table_type(&self, id: TableId) -> Option<&Table> {
        let imported_count = self.module.imported_table_count();
        let index = id.0 as usize;
        if index < imported_count {
            self.module
                .imports
                .iter()
                .filter_map(|import| match &import.desc {
                    ImportDesc::Table(table) => Some(table),
                    ImportDesc::Function(_) | ImportDesc::Memory(_) => None,
                })
                .nth(index)
        } else {
            self.module.tables.get(index - imported_count)
        }
    }

    fn error(&mut self, message: impl Into<String>) {
        self.errors.push(ValidationError::new(message));
    }
}

#[derive(Debug, Clone)]
struct FunctionContext {
    locals: Vec<ValueType>,
    return_types: Vec<ValueType>,
    labels: Vec<LabelType>,
}

impl FunctionContext {
    fn new(locals: Vec<ValueType>, return_types: Vec<ValueType>) -> Self {
        Self { locals, return_types: return_types.clone(), labels: vec![LabelType { branch_results: return_types }] }
    }

    fn constant_expression() -> Self {
        Self { locals: Vec::new(), return_types: Vec::new(), labels: Vec::new() }
    }
}

#[derive(Debug, Clone)]
struct LabelType {
    branch_results: Vec<ValueType>,
}

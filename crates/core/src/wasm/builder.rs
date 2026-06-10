//! Compiler-owned WebAssembly module model.
//!
//! This module provides the structured representation that the
//! backend can validate and lower to bytes.

#![allow(dead_code)]

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct Module {
    pub(crate) types: Vec<FunctionType>,
    pub(crate) imports: Vec<Import>,
    pub(crate) functions: Vec<Function>,
    pub(crate) tables: Vec<Table>,
    pub(crate) memories: Vec<Memory>,
    pub(crate) exports: Vec<Export>,
    pub(crate) data_segments: Vec<DataSegment>,
    pub(crate) custom_sections: Vec<CustomSection>,
}

impl Module {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn push_type(&mut self, type_: FunctionType) -> TypeId {
        let id = TypeId(self.types.len() as u32);
        self.types.push(type_);
        id
    }

    pub(crate) fn push_import(&mut self, import: Import) -> ImportId {
        let id = ImportId(self.imports.len() as u32);
        self.imports.push(import);
        id
    }

    pub(crate) fn push_function(&mut self, function: Function) -> FunctionId {
        let id = FunctionId((self.imported_function_count() + self.functions.len()) as u32);
        self.functions.push(function);
        id
    }

    pub(crate) fn push_memory(&mut self, memory: Memory) -> MemoryId {
        let id = MemoryId(self.memories.len() as u32);
        self.memories.push(memory);
        id
    }

    pub(crate) fn push_table(&mut self, table: Table) -> TableId {
        let id = TableId(self.tables.len() as u32);
        self.tables.push(table);
        id
    }

    fn imported_function_count(&self) -> usize {
        self.imports
            .iter()
            .filter(|import| matches!(import.desc, ImportDesc::Function(_)))
            .count()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct TypeId(pub(crate) u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ImportId(pub(crate) u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct FunctionId(pub(crate) u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct LocalId(pub(crate) u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct MemoryId(pub(crate) u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct TableId(pub(crate) u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct DataSegmentId(pub(crate) u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FunctionType {
    pub(crate) params: Vec<ValueType>,
    pub(crate) results: Vec<ValueType>,
}

impl FunctionType {
    pub(crate) fn new(params: impl Into<Vec<ValueType>>, results: impl Into<Vec<ValueType>>) -> Self {
        Self { params: params.into(), results: results.into() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ValueType {
    I32,
    I64,
    F32,
    F64,
    FuncRef,
    ExternRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Import {
    pub(crate) module: String,
    pub(crate) name: String,
    pub(crate) desc: ImportDesc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ImportDesc {
    Function(TypeId),
    Memory(Memory),
    Table(Table),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Function {
    pub(crate) name: Option<String>,
    pub(crate) type_id: TypeId,
    pub(crate) params: Vec<Local>,
    pub(crate) locals: Vec<Local>,
    pub(crate) body: Vec<Instruction>,
}

impl Function {
    pub(crate) fn new(type_id: TypeId) -> Self {
        Self { name: None, type_id, params: Vec::new(), locals: Vec::new(), body: Vec::new() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Local {
    pub(crate) name: Option<String>,
    pub(crate) type_: ValueType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Memory {
    pub(crate) minimum_pages: u32,
    pub(crate) maximum_pages: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Table {
    pub(crate) element_type: ReferenceType,
    pub(crate) minimum: u32,
    pub(crate) maximum: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ReferenceType {
    FuncRef,
    ExternRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Export {
    pub(crate) name: String,
    pub(crate) desc: ExportDesc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExportDesc {
    Function(FunctionId),
    Memory(MemoryId),
    Table(TableId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DataSegment {
    pub(crate) memory: MemoryId,
    pub(crate) offset: Vec<Instruction>,
    pub(crate) bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CustomSection {
    pub(crate) name: String,
    pub(crate) bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Instruction {
    Unreachable,
    Nop,
    Block {
        type_: BlockType,
        body: Vec<Instruction>,
    },
    Loop {
        type_: BlockType,
        body: Vec<Instruction>,
    },
    If {
        type_: BlockType,
        then_body: Vec<Instruction>,
        else_body: Vec<Instruction>,
    },
    Br {
        depth: u32,
        results: Vec<ValueType>,
    },
    BrIf {
        depth: u32,
        results: Vec<ValueType>,
    },
    Return {
        results: Vec<ValueType>,
    },
    Call {
        function: FunctionId,
        type_: FunctionType,
    },
    CallIndirect {
        table: TableId,
        type_id: TypeId,
        type_: FunctionType,
    },
    Drop(ValueType),
    Select(ValueType),
    LocalGet {
        local: LocalId,
        type_: ValueType,
    },
    LocalSet {
        local: LocalId,
        type_: ValueType,
    },
    LocalTee {
        local: LocalId,
        type_: ValueType,
    },
    I32Const(i32),
    I64Const(i64),
    F32Const(u32),
    F64Const(u64),
    I32Eqz,
    I32Eq,
    I32Ne,
    I32LtS,
    I32GtS,
    I32LeS,
    I32GeS,
    I64Eqz,
    I64Eq,
    F64Eq,
    F64Ne,
    F64Lt,
    F64Gt,
    F64Le,
    F64Ge,
    I32Add,
    I32Sub,
    I32Mul,
    I32DivS,
    I64Add,
    I64Sub,
    I64Mul,
    I64DivS,
    F64Add,
    F64Sub,
    F64Mul,
    F64Div,
    I32Load(MemoryArg),
    I32Store(MemoryArg),
    I64Load(MemoryArg),
    I64Store(MemoryArg),
    F64Load(MemoryArg),
    F64Store(MemoryArg),
}

impl Instruction {
    pub(crate) fn stack_effect(&self) -> StackEffect {
        use Instruction as I;
        use ValueType::{F32, F64, I32, I64};

        match self {
            I::Unreachable => StackEffect::unreachable(),
            I::Nop => StackEffect::new([], []),
            I::Block { type_, .. } | I::Loop { type_, .. } => {
                StackEffect::new(type_.params.clone(), type_.results.clone())
            }
            I::If { type_, .. } => {
                let mut consumes = type_.params.clone();
                consumes.push(I32);
                StackEffect::new(consumes, type_.results.clone())
            }
            I::Br { results, .. } | I::Return { results } => StackEffect::terminating(results.clone(), []),
            I::BrIf { results, .. } => {
                let mut consumes = results.clone();
                consumes.push(I32);
                StackEffect::new(consumes, results.clone())
            }
            I::Call { type_, .. } => StackEffect::new(type_.params.clone(), type_.results.clone()),
            I::CallIndirect { type_, .. } => {
                let mut consumes = type_.params.clone();
                consumes.push(I32);
                StackEffect::new(consumes, type_.results.clone())
            }
            I::Drop(type_) => StackEffect::new([*type_], []),
            I::Select(type_) => StackEffect::new([*type_, *type_, I32], [*type_]),
            I::LocalGet { type_, .. } => StackEffect::new([], [*type_]),
            I::LocalSet { type_, .. } => StackEffect::new([*type_], []),
            I::LocalTee { type_, .. } => StackEffect::new([*type_], [*type_]),
            I::I32Const(_) => StackEffect::new([], [I32]),
            I::I64Const(_) => StackEffect::new([], [I64]),
            I::F32Const(_) => StackEffect::new([], [F32]),
            I::F64Const(_) => StackEffect::new([], [F64]),
            I::I32Eqz => StackEffect::new([I32], [I32]),
            I::I32Eq | I::I32Ne | I::I32LtS | I::I32GtS | I::I32LeS | I::I32GeS => StackEffect::new([I32, I32], [I32]),
            I::I64Eqz => StackEffect::new([I64], [I32]),
            I::I64Eq => StackEffect::new([I64, I64], [I32]),
            I::F64Eq | I::F64Ne | I::F64Lt | I::F64Gt | I::F64Le | I::F64Ge => StackEffect::new([F64, F64], [I32]),
            I::I32Add | I::I32Sub | I::I32Mul | I::I32DivS => StackEffect::new([I32, I32], [I32]),
            I::I64Add | I::I64Sub | I::I64Mul | I::I64DivS => StackEffect::new([I64, I64], [I64]),
            I::F64Add | I::F64Sub | I::F64Mul | I::F64Div => StackEffect::new([F64, F64], [F64]),
            I::I32Load(_) => StackEffect::new([I32], [I32]),
            I::I32Store(_) => StackEffect::new([I32, I32], []),
            I::I64Load(_) => StackEffect::new([I32], [I64]),
            I::I64Store(_) => StackEffect::new([I32, I64], []),
            I::F64Load(_) => StackEffect::new([I32], [F64]),
            I::F64Store(_) => StackEffect::new([I32, F64], []),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BlockType {
    pub(crate) params: Vec<ValueType>,
    pub(crate) results: Vec<ValueType>,
}

impl BlockType {
    pub(crate) fn empty() -> Self {
        Self { params: Vec::new(), results: Vec::new() }
    }

    pub(crate) fn new(params: impl Into<Vec<ValueType>>, results: impl Into<Vec<ValueType>>) -> Self {
        Self { params: params.into(), results: results.into() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MemoryArg {
    pub(crate) memory: MemoryId,
    pub(crate) align: u32,
    pub(crate) offset: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StackEffect {
    pub(crate) consumes: Vec<ValueType>,
    pub(crate) produces: Vec<ValueType>,
    pub(crate) control: ControlEffect,
}

impl StackEffect {
    pub(crate) fn new(consumes: impl Into<Vec<ValueType>>, produces: impl Into<Vec<ValueType>>) -> Self {
        Self { consumes: consumes.into(), produces: produces.into(), control: ControlEffect::Continues }
    }

    pub(crate) fn terminating(consumes: impl Into<Vec<ValueType>>, produces: impl Into<Vec<ValueType>>) -> Self {
        Self { consumes: consumes.into(), produces: produces.into(), control: ControlEffect::Terminates }
    }

    pub(crate) fn unreachable() -> Self {
        Self { consumes: Vec::new(), produces: Vec::new(), control: ControlEffect::Unreachable }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ControlEffect {
    Continues,
    Terminates,
    Unreachable,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_assigns_function_ids_after_imported_functions() {
        let mut module = Module::new();
        let type_id = module.push_type(FunctionType::new([ValueType::I32], [ValueType::I32]));

        module.push_import(Import { module: "env".into(), name: "host".into(), desc: ImportDesc::Function(type_id) });

        assert_eq!(module.push_function(Function::new(type_id)), FunctionId(1));
    }

    #[test]
    fn instructions_report_explicit_operand_stack_effects() {
        let effect = Instruction::Call {
            function: FunctionId(0),
            type_: FunctionType::new([ValueType::I32, ValueType::I64], [ValueType::I32]),
        }
        .stack_effect();

        assert_eq!(effect.consumes, vec![ValueType::I32, ValueType::I64]);
        assert_eq!(effect.produces, vec![ValueType::I32]);
        assert_eq!(effect.control, ControlEffect::Continues);

        let branch_effect = Instruction::BrIf { depth: 0, results: vec![ValueType::I32] }.stack_effect();
        assert_eq!(branch_effect.consumes, vec![ValueType::I32, ValueType::I32]);
        assert_eq!(branch_effect.produces, vec![ValueType::I32]);
    }
}

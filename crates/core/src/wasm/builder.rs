//! Compiler-owned WebAssembly module model.
//!
//! This module provides the structured representation that the
//! backend can validate and lower to bytes.

#![allow(dead_code)]

use std::fmt;

use crate::source::Span;

use super::{
    binary::BinaryEmitter,
    validator::{ValidationResult, Validator},
};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct Module {
    pub(crate) source_span: Option<Span>,
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
        let id = MemoryId((self.imported_memory_count() + self.memories.len()) as u32);
        self.memories.push(memory);
        id
    }

    pub(crate) fn push_table(&mut self, table: Table) -> TableId {
        let id = TableId((self.imported_table_count() + self.tables.len()) as u32);
        self.tables.push(table);
        id
    }

    pub(super) fn imported_function_count(&self) -> usize {
        self.imports
            .iter()
            .filter(|import| matches!(import.desc, ImportDesc::Function(_)))
            .count()
    }

    pub(super) fn imported_memory_count(&self) -> usize {
        self.imports
            .iter()
            .filter(|import| matches!(import.desc, ImportDesc::Memory(_)))
            .count()
    }

    pub(super) fn imported_table_count(&self) -> usize {
        self.imports
            .iter()
            .filter(|import| matches!(import.desc, ImportDesc::Table(_)))
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

impl fmt::Display for ValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::FuncRef => "funcref",
            Self::ExternRef => "externref",
        })
    }
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

impl fmt::Display for ReferenceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::FuncRef => "funcref",
            Self::ExternRef => "externref",
        })
    }
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
    I64Ne,
    I64LtS,
    I64GtS,
    I64LeS,
    I64GeS,
    F64Eq,
    F64Ne,
    F64Lt,
    F64Gt,
    F64Le,
    F64Ge,
    I32Add,
    I32Sub,
    I32And,
    I32Mul,
    I32DivS,
    I64Add,
    I64Sub,
    I64Mul,
    I64DivS,
    I64RemS,
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
            I::I64Eq | I::I64Ne | I::I64LtS | I::I64GtS | I::I64LeS | I::I64GeS => StackEffect::new([I64, I64], [I32]),
            I::F64Eq | I::F64Ne | I::F64Lt | I::F64Gt | I::F64Le | I::F64Ge => StackEffect::new([F64, F64], [I32]),
            I::I32Add | I::I32Sub | I::I32And | I::I32Mul | I::I32DivS => StackEffect::new([I32, I32], [I32]),
            I::I64Add | I::I64Sub | I::I64Mul | I::I64DivS | I::I64RemS => StackEffect::new([I64, I64], [I64]),
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

impl Module {
    pub(crate) fn validate(&self) -> ValidationResult<()> {
        Validator::new(self).validate()
    }

    pub(crate) fn to_wasm_bytes(&self) -> ValidationResult<Vec<u8>> {
        self.validate()?;
        Ok(BinaryEmitter::new(self).emit())
    }

    pub(crate) fn to_wat(&self) -> ValidationResult<String> {
        self.validate()?;
        Ok(WatRenderer::new(self).render())
    }
}

struct WatRenderer<'a> {
    module: &'a Module,
    wat: String,
    indent: usize,
}

impl<'a> WatRenderer<'a> {
    fn new(module: &'a Module) -> Self {
        Self { module, wat: String::new(), indent: 0 }
    }

    fn render(mut self) -> String {
        self.line("(module");
        self.indent += 1;
        for type_ in &self.module.types {
            self.line(&format!(
                "(type (func{}{}))",
                params_wat(&type_.params),
                results_wat(&type_.results)
            ));
        }
        for import in &self.module.imports {
            self.render_import(import);
        }
        for table in &self.module.tables {
            self.line(&format!(
                "(table {}{} {})",
                table.minimum,
                max_suffix(table.maximum),
                table.element_type
            ));
        }
        for memory in &self.module.memories {
            self.line(&format!(
                "(memory {}{})",
                memory.minimum_pages,
                max_suffix(memory.maximum_pages)
            ));
        }
        for function in &self.module.functions {
            self.render_function(function);
        }
        for export in &self.module.exports {
            self.render_export(export);
        }
        for segment in &self.module.data_segments {
            self.render_data_segment(segment);
        }
        self.indent -= 1;
        self.line(")");
        self.wat
    }

    fn render_import(&mut self, import: &Import) {
        match &import.desc {
            ImportDesc::Function(type_id) => {
                let type_ = &self.module.types[type_id.0 as usize];
                self.line(&format!(
                    "(import {:?} {:?} (func (type {}){}{}))",
                    import.module,
                    import.name,
                    type_id.0,
                    params_wat(&type_.params),
                    results_wat(&type_.results)
                ));
            }
            ImportDesc::Memory(memory) => self.line(&format!(
                "(import {:?} {:?} (memory {}{}))",
                import.module,
                import.name,
                memory.minimum_pages,
                max_suffix(memory.maximum_pages)
            )),
            ImportDesc::Table(table) => self.line(&format!(
                "(import {:?} {:?} (table {}{} {}))",
                import.module,
                import.name,
                table.minimum,
                max_suffix(table.maximum),
                table.element_type
            )),
        }
    }

    fn render_function(&mut self, function: &Function) {
        let type_ = &self.module.types[function.type_id.0 as usize];
        let name = function
            .name
            .as_deref()
            .map(wat_id)
            .map(|name| format!(" {name}"))
            .unwrap_or_default();
        self.line(&format!(
            "(func{name} (type {}){}{}",
            function.type_id.0,
            params_wat(&type_.params),
            results_wat(&type_.results)
        ));
        self.indent += 1;
        for local in &function.locals {
            self.line(&format!("(local {})", local.type_));
        }
        for instruction in &function.body {
            self.render_instruction(instruction);
        }
        self.indent -= 1;
        self.line(")");
    }

    fn render_export(&mut self, export: &Export) {
        let desc = match export.desc {
            ExportDesc::Function(id) => format!("func {}", id.0),
            ExportDesc::Memory(id) => format!("memory {}", id.0),
            ExportDesc::Table(id) => format!("table {}", id.0),
        };
        self.line(&format!("(export {:?} ({desc}))", export.name));
    }

    fn render_data_segment(&mut self, segment: &DataSegment) {
        let offset = segment
            .offset
            .iter()
            .map(instruction_inline_wat)
            .collect::<Vec<_>>()
            .join(" ");
        self.line(&format!(
            "(data (memory {}) (offset {offset}) {:?})",
            segment.memory.0,
            String::from_utf8_lossy(&segment.bytes)
        ));
    }

    fn render_instruction(&mut self, instruction: &Instruction) {
        match instruction {
            Instruction::Block { type_, body } => {
                self.line(&format!(
                    "block{}{}",
                    params_wat(&type_.params),
                    results_wat(&type_.results)
                ));
                self.indent += 1;
                for instruction in body {
                    self.render_instruction(instruction);
                }
                self.indent -= 1;
                self.line("end");
            }
            Instruction::Loop { type_, body } => {
                self.line(&format!(
                    "loop{}{}",
                    params_wat(&type_.params),
                    results_wat(&type_.results)
                ));
                self.indent += 1;
                for instruction in body {
                    self.render_instruction(instruction);
                }
                self.indent -= 1;
                self.line("end");
            }
            Instruction::If { type_, then_body, else_body } => {
                self.line(&format!(
                    "if{}{}",
                    params_wat(&type_.params),
                    results_wat(&type_.results)
                ));
                self.indent += 1;
                for instruction in then_body {
                    self.render_instruction(instruction);
                }
                if !else_body.is_empty() {
                    self.indent -= 1;
                    self.line("else");
                    self.indent += 1;
                    for instruction in else_body {
                        self.render_instruction(instruction);
                    }
                }
                self.indent -= 1;
                self.line("end");
            }
            _ => self.line(&instruction_inline_wat(instruction)),
        }
    }

    fn line(&mut self, text: &str) {
        for _ in 0..self.indent {
            self.wat.push_str("  ");
        }
        self.wat.push_str(text);
        self.wat.push('\n');
    }
}

fn instruction_inline_wat(instruction: &Instruction) -> String {
    match instruction {
        Instruction::Unreachable => "unreachable".into(),
        Instruction::Nop => "nop".into(),
        Instruction::Br { depth, .. } => format!("br {depth}"),
        Instruction::BrIf { depth, .. } => format!("br_if {depth}"),
        Instruction::Return { .. } => "return".into(),
        Instruction::Call { function, .. } => format!("call {}", function.0),
        Instruction::CallIndirect { table, type_id, .. } => {
            format!("call_indirect (type {}) (table {})", type_id.0, table.0)
        }
        Instruction::Drop(_) => "drop".into(),
        Instruction::Select(_) => "select".into(),
        Instruction::LocalGet { local, .. } => format!("local.get {}", local.0),
        Instruction::LocalSet { local, .. } => format!("local.set {}", local.0),
        Instruction::LocalTee { local, .. } => format!("local.tee {}", local.0),
        Instruction::I32Const(value) => format!("i32.const {value}"),
        Instruction::I64Const(value) => format!("i64.const {value}"),
        Instruction::F32Const(value) => format!("f32.const {}", f32::from_bits(*value)),
        Instruction::F64Const(value) => format!("f64.const {}", f64::from_bits(*value)),
        Instruction::I32Eqz => "i32.eqz".into(),
        Instruction::I32Eq => "i32.eq".into(),
        Instruction::I32Ne => "i32.ne".into(),
        Instruction::I32LtS => "i32.lt_s".into(),
        Instruction::I32GtS => "i32.gt_s".into(),
        Instruction::I32LeS => "i32.le_s".into(),
        Instruction::I32GeS => "i32.ge_s".into(),
        Instruction::I64Eqz => "i64.eqz".into(),
        Instruction::I64Eq => "i64.eq".into(),
        Instruction::I64Ne => "i64.ne".into(),
        Instruction::I64LtS => "i64.lt_s".into(),
        Instruction::I64GtS => "i64.gt_s".into(),
        Instruction::I64LeS => "i64.le_s".into(),
        Instruction::I64GeS => "i64.ge_s".into(),
        Instruction::F64Eq => "f64.eq".into(),
        Instruction::F64Ne => "f64.ne".into(),
        Instruction::F64Lt => "f64.lt".into(),
        Instruction::F64Gt => "f64.gt".into(),
        Instruction::F64Le => "f64.le".into(),
        Instruction::F64Ge => "f64.ge".into(),
        Instruction::I32Add => "i32.add".into(),
        Instruction::I32Sub => "i32.sub".into(),
        Instruction::I32And => "i32.and".into(),
        Instruction::I32Mul => "i32.mul".into(),
        Instruction::I32DivS => "i32.div_s".into(),
        Instruction::I64Add => "i64.add".into(),
        Instruction::I64Sub => "i64.sub".into(),
        Instruction::I64Mul => "i64.mul".into(),
        Instruction::I64DivS => "i64.div_s".into(),
        Instruction::I64RemS => "i64.rem_s".into(),
        Instruction::F64Add => "f64.add".into(),
        Instruction::F64Sub => "f64.sub".into(),
        Instruction::F64Mul => "f64.mul".into(),
        Instruction::F64Div => "f64.div".into(),
        Instruction::I32Load(arg) => memory_wat("i32.load", arg),
        Instruction::I32Store(arg) => memory_wat("i32.store", arg),
        Instruction::I64Load(arg) => memory_wat("i64.load", arg),
        Instruction::I64Store(arg) => memory_wat("i64.store", arg),
        Instruction::F64Load(arg) => memory_wat("f64.load", arg),
        Instruction::F64Store(arg) => memory_wat("f64.store", arg),
        Instruction::Block { .. } | Instruction::Loop { .. } | Instruction::If { .. } => unreachable!(),
    }
}

fn memory_wat(opcode: &str, arg: &MemoryArg) -> String {
    format!("{opcode} offset={} align={}", arg.offset, 1_u32 << arg.align)
}

fn params_wat(types: &[ValueType]) -> String {
    types
        .iter()
        .map(|type_| format!(" (param {type_})"))
        .collect::<String>()
}

fn results_wat(types: &[ValueType]) -> String {
    types
        .iter()
        .map(|type_| format!(" (result {type_})"))
        .collect::<String>()
}

fn max_suffix(maximum: Option<u32>) -> String {
    maximum.map(|maximum| format!(" {maximum}")).unwrap_or_default()
}

fn wat_id(name: &str) -> String {
    let mut out = String::from("$");
    out.extend(
        name.chars()
            .map(|char| if char.is_ascii_alphanumeric() || char == '_' { char } else { '_' }),
    );
    out
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

    #[test]
    fn validates_stack_locals_branches_and_call_signatures() {
        let mut module = Module::new();
        let host_type = module.push_type(FunctionType::new([ValueType::I32], [ValueType::I32]));
        let void_type = module.push_type(FunctionType::new([], []));
        module.push_import(Import { module: "env".into(), name: "host".into(), desc: ImportDesc::Function(host_type) });
        let mut function = Function::new(void_type);
        function.body = vec![
            Instruction::LocalGet { local: LocalId(99), type_: ValueType::I32 },
            Instruction::Call { function: FunctionId(0), type_: FunctionType::new([], []) },
            Instruction::Block {
                type_: BlockType::new([], [ValueType::I32]),
                body: vec![Instruction::Br { depth: 0, results: vec![] }],
            },
        ];
        module.push_function(function);

        let errors = module.validate().expect_err("module should not validate");
        let messages = errors
            .into_iter()
            .map(|error| error.message)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(messages.contains("unknown local index"), "{messages}");
        assert!(messages.contains("call to function 0 has signature"), "{messages}");
        assert!(messages.contains("branch depth 0 has results"), "{messages}");
    }

    #[test]
    fn emits_valid_wasm_bytes_directly_from_structured_module() {
        let module = add_module();
        let bytes = module
            .to_wasm_bytes()
            .expect("structured module should validate and emit");
        assert_eq!(&bytes[..8], b"\0asm\x01\0\0\0");

        let engine = wasmtime::Engine::default();
        let wasm = wasmtime::Module::from_binary(&engine, &bytes).expect("direct bytes should be valid wasm");
        let mut store = wasmtime::Store::new(&engine, ());
        let instance = wasmtime::Instance::new(&mut store, &wasm, &[]).expect("module should instantiate");
        let add = instance
            .get_typed_func::<(i32, i32), i32>(&mut store, "add")
            .expect("add export should exist");
        assert_eq!(add.call(&mut store, (20, 22)).expect("add call should succeed"), 42);
    }

    #[test]
    fn renders_wat_from_structured_module() {
        let module = add_module();
        let wat = module.to_wat().expect("structured module should render to wat");
        assert!(wat.contains("(type (func (param i32) (param i32) (result i32)))"));
        assert!(wat.contains("i32.add"));
        assert!(wat.contains("(export \"add\" (func 0))"));
        wat::parse_str(&wat).expect("rendered wat should assemble");
    }

    #[test]
    fn emits_type_indexed_block_signatures() {
        let mut module = Module::new();
        let type_id = module.push_type(FunctionType::new([ValueType::I32], [ValueType::I32]));
        let mut function = Function::new(type_id);
        function.params = vec![Local { name: None, type_: ValueType::I32 }];
        function.body = vec![
            Instruction::LocalGet { local: LocalId(0), type_: ValueType::I32 },
            Instruction::Block {
                type_: BlockType::new([ValueType::I32], [ValueType::I32]),
                body: vec![Instruction::I32Const(1), Instruction::I32Add],
            },
        ];
        let function_id = module.push_function(function);
        module
            .exports
            .push(Export { name: "inc".into(), desc: ExportDesc::Function(function_id) });
        wat::parse_str(module.to_wat().expect("typed block module should render"))
            .expect("typed block wat should assemble");

        let engine = wasmtime::Engine::default();
        let wasm = wasmtime::Module::from_binary(
            &engine,
            &module.to_wasm_bytes().expect("typed block module should emit"),
        )
        .expect("typed block bytes should be valid wasm");
        let mut store = wasmtime::Store::new(&engine, ());
        let instance = wasmtime::Instance::new(&mut store, &wasm, &[]).expect("module should instantiate");
        let inc = instance
            .get_typed_func::<i32, i32>(&mut store, "inc")
            .expect("inc export should exist");
        assert_eq!(inc.call(&mut store, 41).expect("inc call should succeed"), 42);
    }

    fn add_module() -> Module {
        let mut module = Module::new();
        let type_id = module.push_type(FunctionType::new([ValueType::I32, ValueType::I32], [ValueType::I32]));
        let mut function = Function::new(type_id);
        function.name = Some("add".into());
        function.params = vec![
            Local { name: Some("left".into()), type_: ValueType::I32 },
            Local { name: Some("right".into()), type_: ValueType::I32 },
        ];
        function.body = vec![
            Instruction::LocalGet { local: LocalId(0), type_: ValueType::I32 },
            Instruction::LocalGet { local: LocalId(1), type_: ValueType::I32 },
            Instruction::I32Add,
        ];
        let function_id = module.push_function(function);
        module
            .exports
            .push(Export { name: "add".into(), desc: ExportDesc::Function(function_id) });
        module
    }
}

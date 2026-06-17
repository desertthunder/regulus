//! Compiler-owned WebAssembly module model.
//!
//! This module provides the structured representation that the
//! backend can validate and lower to bytes.

use std::fmt;

use super::binary::BinaryEmitter;
use super::validator::{ValidationResult, Validator};
use crate::source::Span;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Module {
    pub source_span: Option<Span>,
    pub types: Vec<FunctionType>,
    pub imports: Vec<Import>,
    pub functions: Vec<Function>,
    pub tables: Vec<Table>,
    pub memories: Vec<Memory>,
    pub globals: Vec<Global>,
    pub raw_wat_items: Vec<String>,
    pub exports: Vec<Export>,
    pub element_segments: Vec<ElementSegment>,
    pub data_segments: Vec<DataSegment>,
    pub custom_sections: Vec<CustomSection>,
}

impl Module {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_type(&mut self, type_: FunctionType) -> TypeId {
        let id = TypeId(self.types.len() as u32);
        self.types.push(type_);
        id
    }

    pub fn push_import(&mut self, import: Import) -> ImportId {
        let id = ImportId(self.imports.len() as u32);
        self.imports.push(import);
        id
    }

    pub fn push_function(&mut self, function: Function) -> FunctionId {
        let id = FunctionId((self.imported_function_count() + self.functions.len()) as u32);
        self.functions.push(function);
        id
    }

    pub fn push_memory(&mut self, memory: Memory) -> MemoryId {
        let id = MemoryId((self.imported_memory_count() + self.memories.len()) as u32);
        self.memories.push(memory);
        id
    }

    pub fn push_global(&mut self, global: Global) -> GlobalId {
        let id = GlobalId(self.globals.len() as u32);
        self.globals.push(global);
        id
    }

    /// Intern a function type, reusing an existing entry if one matches.
    pub fn intern_type(&mut self, type_: FunctionType) -> TypeId {
        if let Some(pos) = self.types.iter().position(|t| t == &type_) {
            TypeId(pos as u32)
        } else {
            self.push_type(type_)
        }
    }

    pub fn push_table(&mut self, table: Table) -> TableId {
        let id = TableId((self.imported_table_count() + self.tables.len()) as u32);
        self.tables.push(table);
        id
    }

    pub fn imported_function_count(&self) -> usize {
        self.imports
            .iter()
            .filter(|import| matches!(import.desc, ImportDesc::Function(_)))
            .count()
    }

    pub fn imported_memory_count(&self) -> usize {
        self.imports
            .iter()
            .filter(|import| matches!(import.desc, ImportDesc::Memory(_)))
            .count()
    }

    pub fn imported_table_count(&self) -> usize {
        self.imports
            .iter()
            .filter(|import| matches!(import.desc, ImportDesc::Table(_)))
            .count()
    }

    pub fn push_element(&mut self, segment: ElementSegment) {
        self.element_segments.push(segment);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementSegment {
    pub table: TableId,
    pub offset: u32,
    pub functions: Vec<FunctionId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImportId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunctionId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MemoryId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlobalId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TableId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionType {
    pub params: Vec<ValueType>,
    pub results: Vec<ValueType>,
}

impl FunctionType {
    pub fn new(params: impl Into<Vec<ValueType>>, results: impl Into<Vec<ValueType>>) -> Self {
        Self { params: params.into(), results: results.into() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueType {
    I32,
    I64,
    /// TODO: add ABI/backend support for compiler-emitted 32-bit floats.
    #[allow(dead_code)]
    F32,
    F64,
    /// TODO: add ABI/backend support for compiler-emitted reference values.
    #[allow(dead_code)]
    FuncRef,
    /// TODO: add ABI/backend support for compiler-emitted reference values.
    #[allow(dead_code)]
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
pub struct Import {
    pub module: String,
    pub name: String,
    pub desc: ImportDesc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportDesc {
    Function(TypeId),
    /// TODO: add ABI/backend support for imported Wasm memories.
    #[allow(dead_code)]
    Memory(Memory),
    /// TODO: add ABI/backend support for imported Wasm tables.
    #[allow(dead_code)]
    Table(Table),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub name: Option<String>,
    pub type_id: TypeId,
    pub params: Vec<Local>,
    pub locals: Vec<Local>,
    pub body: Vec<Instruction>,
}

impl Function {
    pub fn new(type_id: TypeId) -> Self {
        Self { name: None, type_id, params: Vec::new(), locals: Vec::new(), body: Vec::new() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Local {
    pub name: Option<String>,
    pub type_: ValueType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Memory {
    pub minimum_pages: u32,
    pub maximum_pages: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Global {
    pub name: Option<String>,
    pub type_: ValueType,
    pub mutable: bool,
    pub init: Vec<Instruction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    pub element_type: ReferenceType,
    pub minimum: u32,
    pub maximum: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReferenceType {
    FuncRef,
    /// TODO: add ABI/backend support for externref tables.
    #[allow(dead_code)]
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
pub struct Export {
    pub name: String,
    pub desc: ExportDesc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportDesc {
    Function(FunctionId),
    Memory(MemoryId),
    /// TODO: add ABI/backend support for exported Wasm tables.
    #[allow(dead_code)]
    Table(TableId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataSegment {
    pub memory: MemoryId,
    pub offset: Vec<Instruction>,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomSection {
    pub name: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instruction {
    Unreachable,
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
    /// TODO: add ABI/backend support for explicit Wasm return emission.
    #[allow(dead_code)]
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
    CallName {
        name: String,
        type_: FunctionType,
    },
    Drop(ValueType),
    /// TODO: add backend support for Wasm select emission.
    #[allow(dead_code)]
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
    GlobalGet {
        global: GlobalId,
        type_: ValueType,
    },
    GlobalSet {
        global: GlobalId,
        type_: ValueType,
    },
    I32Const(i32),
    I64Const(i64),
    /// TODO: add ABI/backend support for compiler-emitted 32-bit floats.
    #[allow(dead_code)]
    F32Const(u32),
    F64Const(u64),
    I32Eqz,
    I32Eq,
    I32LtS,
    I32LtU,
    I32GtS,
    I32GtU,
    I32LeS,
    I32GeS,
    I64Eq,
    I64LtS,
    I64GtS,
    I64LeS,
    I64GeS,
    F64Eq,
    F64Lt,
    F64Gt,
    F64Le,
    F64Ge,
    I32Add,
    I32Sub,
    I32And,
    I32Mul,
    I32DivS,
    I32ShrU,
    I64Add,
    I64Sub,
    I64Mul,
    I64DivS,
    I64RemS,
    I32WrapI64,
    I64ExtendI32U,
    I64ReinterpretF64,
    F64ReinterpretI64,
    F64Add,
    F64Sub,
    F64Mul,
    F64Div,
    F64Min,
    F64Max,
    I32Load(MemoryArg),
    I32Load8U(MemoryArg),
    I32Store(MemoryArg),
    I32Store8(MemoryArg),
    I64Load(MemoryArg),
    I64Store(MemoryArg),
    F64Load(MemoryArg),
    F64Store(MemoryArg),
    MemorySize(MemoryId),
    MemoryGrow(MemoryId),
}

impl Instruction {
    pub fn stack_effect(&self) -> StackEffect {
        use Instruction as I;
        use ValueType::{F32, F64, I32, I64};

        match self {
            I::Unreachable => StackEffect::unreachable(),
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
            I::Call { type_, .. } | I::CallName { type_, .. } => {
                StackEffect::new(type_.params.clone(), type_.results.clone())
            }
            I::CallIndirect { type_, .. } => {
                let mut consumes = type_.params.clone();
                consumes.push(I32);
                StackEffect::new(consumes, type_.results.clone())
            }
            I::Drop(type_) => StackEffect::new([*type_], []),
            I::Select(type_) => StackEffect::new([*type_, *type_, I32], [*type_]),
            I::LocalGet { type_, .. } | I::GlobalGet { type_, .. } => StackEffect::new([], [*type_]),
            I::LocalSet { type_, .. } | I::GlobalSet { type_, .. } => StackEffect::new([*type_], []),
            I::LocalTee { type_, .. } => StackEffect::new([*type_], [*type_]),
            I::I32Const(_) => StackEffect::new([], [I32]),
            I::I64Const(_) => StackEffect::new([], [I64]),
            I::F32Const(_) => StackEffect::new([], [F32]),
            I::F64Const(_) => StackEffect::new([], [F64]),
            I::I32Eqz => StackEffect::new([I32], [I32]),
            I::I32Eq | I::I32LtS | I::I32LtU | I::I32GtS | I::I32GtU | I::I32LeS | I::I32GeS => {
                StackEffect::new([I32, I32], [I32])
            }
            I::I64Eq | I::I64LtS | I::I64GtS | I::I64LeS | I::I64GeS => StackEffect::new([I64, I64], [I32]),
            I::F64Eq | I::F64Lt | I::F64Gt | I::F64Le | I::F64Ge => StackEffect::new([F64, F64], [I32]),
            I::I32Add | I::I32Sub | I::I32And | I::I32Mul | I::I32DivS | I::I32ShrU => {
                StackEffect::new([I32, I32], [I32])
            }
            I::I64Add | I::I64Sub | I::I64Mul | I::I64DivS | I::I64RemS => StackEffect::new([I64, I64], [I64]),
            I::I32WrapI64 => StackEffect::new([I64], [I32]),
            I::I64ExtendI32U => StackEffect::new([I32], [I64]),
            I::I64ReinterpretF64 => StackEffect::new([F64], [I64]),
            I::F64ReinterpretI64 => StackEffect::new([I64], [F64]),
            I::F64Add | I::F64Sub | I::F64Mul | I::F64Div | I::F64Min | I::F64Max => {
                StackEffect::new([F64, F64], [F64])
            }
            I::I32Load(_) | I::I32Load8U(_) => StackEffect::new([I32], [I32]),
            I::I32Store(_) | I::I32Store8(_) => StackEffect::new([I32, I32], []),
            I::I64Load(_) => StackEffect::new([I32], [I64]),
            I::I64Store(_) => StackEffect::new([I32, I64], []),
            I::F64Load(_) => StackEffect::new([I32], [F64]),
            I::F64Store(_) => StackEffect::new([I32, F64], []),
            I::MemorySize(_) => StackEffect::new([], [I32]),
            I::MemoryGrow(_) => StackEffect::new([I32], [I32]),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockType {
    pub params: Vec<ValueType>,
    pub results: Vec<ValueType>,
}

impl BlockType {
    pub fn empty() -> Self {
        Self { params: Vec::new(), results: Vec::new() }
    }

    pub fn new(params: impl Into<Vec<ValueType>>, results: impl Into<Vec<ValueType>>) -> Self {
        Self { params: params.into(), results: results.into() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryArg {
    pub memory: MemoryId,
    pub align: u32,
    pub offset: u32,
}

impl MemoryArg {
    pub fn new(memory: MemoryId, offset: u32, align: u32) -> MemoryArg {
        MemoryArg { memory, align, offset }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackEffect {
    pub consumes: Vec<ValueType>,
    pub produces: Vec<ValueType>,
    pub control: ControlEffect,
}

impl StackEffect {
    pub fn new(consumes: impl Into<Vec<ValueType>>, produces: impl Into<Vec<ValueType>>) -> Self {
        Self { consumes: consumes.into(), produces: produces.into(), control: ControlEffect::Continues }
    }

    pub fn terminating(consumes: impl Into<Vec<ValueType>>, produces: impl Into<Vec<ValueType>>) -> Self {
        Self { consumes: consumes.into(), produces: produces.into(), control: ControlEffect::Terminates }
    }

    pub fn unreachable() -> Self {
        Self { consumes: Vec::new(), produces: Vec::new(), control: ControlEffect::Unreachable }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlEffect {
    Continues,
    Terminates,
    Unreachable,
}

impl Module {
    pub fn validate(&self) -> ValidationResult<()> {
        Validator::new(self).validate()
    }

    pub fn to_wasm_bytes(&self) -> ValidationResult<Vec<u8>> {
        self.validate()?;
        Ok(BinaryEmitter::new(self).emit())
    }

    pub fn to_wat(&self) -> ValidationResult<String> {
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
        for global in &self.module.globals {
            self.render_global(global);
        }
        for function in &self.module.functions {
            self.render_function(function);
        }
        for item in &self.module.raw_wat_items {
            for line in item.lines() {
                self.line(line);
            }
        }
        for export in &self.module.exports {
            self.render_export(export);
        }
        for segment in &self.module.element_segments {
            let funcs = segment
                .functions
                .iter()
                .map(|f| f.0.to_string())
                .collect::<Vec<_>>()
                .join(" ");
            self.line(&format!(
                "(elem (table {}) (i32.const {}) func {})",
                segment.table.0, segment.offset, funcs
            ));
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

    fn render_global(&mut self, global: &Global) {
        let name = global
            .name
            .as_deref()
            .map(wat_id)
            .map(|name| format!(" {name}"))
            .unwrap_or_default();
        let mutability = if global.mutable { "mut " } else { "" };
        let init = global
            .init
            .iter()
            .map(instruction_inline_wat)
            .collect::<Vec<_>>()
            .join(" ");
        self.line(&format!("(global{name} ({mutability}{}) {init})", global.type_));
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
            "(data (memory {}) (offset {offset}) \"{}\")",
            segment.memory.0,
            wat_bytes(&segment.bytes)
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
        Instruction::Br { depth, .. } => format!("br {depth}"),
        Instruction::BrIf { depth, .. } => format!("br_if {depth}"),
        Instruction::Return { .. } => "return".into(),
        Instruction::Call { function, .. } => format!("call {}", function.0),
        Instruction::CallIndirect { table, type_id, .. } => {
            if table.0 == 0 {
                format!("call_indirect (type {})", type_id.0)
            } else {
                format!("call_indirect {} (type {})", table.0, type_id.0)
            }
        }
        Instruction::CallName { name, .. } => format!("call ${}", wat_id_part(name)),
        Instruction::Drop(_) => "drop".into(),
        Instruction::Select(_) => "select".into(),
        Instruction::LocalGet { local, .. } => format!("local.get {}", local.0),
        Instruction::LocalSet { local, .. } => format!("local.set {}", local.0),
        Instruction::LocalTee { local, .. } => format!("local.tee {}", local.0),
        Instruction::GlobalGet { global, .. } => format!("global.get {}", global.0),
        Instruction::GlobalSet { global, .. } => format!("global.set {}", global.0),
        Instruction::I32Const(value) => format!("i32.const {value}"),
        Instruction::I64Const(value) => format!("i64.const {value}"),
        Instruction::F32Const(value) => format!("f32.const {}", f32::from_bits(*value)),
        Instruction::F64Const(value) => format!("f64.const {}", f64::from_bits(*value)),
        Instruction::I32Eqz => "i32.eqz".into(),
        Instruction::I32Eq => "i32.eq".into(),
        Instruction::I32LtS => "i32.lt_s".into(),
        Instruction::I32LtU => "i32.lt_u".into(),
        Instruction::I32GtS => "i32.gt_s".into(),
        Instruction::I32GtU => "i32.gt_u".into(),
        Instruction::I32LeS => "i32.le_s".into(),
        Instruction::I32GeS => "i32.ge_s".into(),
        Instruction::I64Eq => "i64.eq".into(),
        Instruction::I64LtS => "i64.lt_s".into(),
        Instruction::I64GtS => "i64.gt_s".into(),
        Instruction::I64LeS => "i64.le_s".into(),
        Instruction::I64GeS => "i64.ge_s".into(),
        Instruction::F64Eq => "f64.eq".into(),
        Instruction::F64Lt => "f64.lt".into(),
        Instruction::F64Gt => "f64.gt".into(),
        Instruction::F64Le => "f64.le".into(),
        Instruction::F64Ge => "f64.ge".into(),
        Instruction::I32Add => "i32.add".into(),
        Instruction::I32Sub => "i32.sub".into(),
        Instruction::I32And => "i32.and".into(),
        Instruction::I32Mul => "i32.mul".into(),
        Instruction::I32DivS => "i32.div_s".into(),
        Instruction::I32ShrU => "i32.shr_u".into(),
        Instruction::I64Add => "i64.add".into(),
        Instruction::I64Sub => "i64.sub".into(),
        Instruction::I64Mul => "i64.mul".into(),
        Instruction::I64DivS => "i64.div_s".into(),
        Instruction::I64RemS => "i64.rem_s".into(),
        Instruction::I32WrapI64 => "i32.wrap_i64".into(),
        Instruction::I64ExtendI32U => "i64.extend_i32_u".into(),
        Instruction::I64ReinterpretF64 => "i64.reinterpret_f64".into(),
        Instruction::F64ReinterpretI64 => "f64.reinterpret_i64".into(),
        Instruction::F64Add => "f64.add".into(),
        Instruction::F64Sub => "f64.sub".into(),
        Instruction::F64Mul => "f64.mul".into(),
        Instruction::F64Div => "f64.div".into(),
        Instruction::F64Min => "f64.min".into(),
        Instruction::F64Max => "f64.max".into(),
        Instruction::I32Load(arg) => memory_wat("i32.load", arg),
        Instruction::I32Load8U(arg) => memory_wat("i32.load8_u", arg),
        Instruction::I32Store(arg) => memory_wat("i32.store", arg),
        Instruction::I32Store8(arg) => memory_wat("i32.store8", arg),
        Instruction::I64Load(arg) => memory_wat("i64.load", arg),
        Instruction::I64Store(arg) => memory_wat("i64.store", arg),
        Instruction::F64Load(arg) => memory_wat("f64.load", arg),
        Instruction::F64Store(arg) => memory_wat("f64.store", arg),
        Instruction::MemorySize(memory) => format!("memory.size {}", memory.0),
        Instruction::MemoryGrow(memory) => format!("memory.grow {}", memory.0),
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

fn wat_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("\\{byte:02x}")).collect()
}

fn wat_id(name: &str) -> String {
    format!("${}", wat_id_part(name))
}

fn wat_id_part(name: &str) -> String {
    name.chars()
        .map(|char| if char.is_ascii_alphanumeric() || char == '_' { char } else { '_' })
        .collect()
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

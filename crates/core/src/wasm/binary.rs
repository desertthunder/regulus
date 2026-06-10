//! Direct binary emission for structured WebAssembly modules.
//!
//! The emitter writes standard Wasm sections from the compiler-owned module
//! model. It assumes validation has already checked indices, signatures, stack
//! effects, and block types before byte emission starts.

use super::{builder::*, encode::*};

pub(super) struct BinaryEmitter<'a> {
    module: &'a Module,
}

impl<'a> BinaryEmitter<'a> {
    pub(super) fn new(module: &'a Module) -> Self {
        Self { module }
    }

    pub(super) fn emit(&self) -> Vec<u8> {
        let mut out = b"\0asm\x01\0\0\0".to_vec();
        for section in self.module.custom_sections.iter().map(Self::custom_section) {
            out.extend(section);
        }
        self.section(&mut out, 1, self.type_section());
        self.section(&mut out, 2, self.import_section());
        self.section(&mut out, 3, self.function_section());
        self.section(&mut out, 4, self.table_section());
        self.section(&mut out, 5, self.memory_section());
        self.section(&mut out, 6, self.global_section());
        self.section(&mut out, 7, self.export_section());
        self.section(&mut out, 10, self.code_section());
        self.section(&mut out, 11, self.data_section());
        out
    }

    fn section(&self, out: &mut Vec<u8>, id: u8, payload: Vec<u8>) {
        if payload.is_empty() {
            return;
        }
        out.push(id);
        encode_u32(payload.len() as u32, out);
        out.extend(payload);
    }

    fn custom_section(section: &CustomSection) -> Vec<u8> {
        let mut payload = Vec::new();
        encode_name(&section.name, &mut payload);
        payload.extend(&section.bytes);
        let mut out = vec![0];
        encode_u32(payload.len() as u32, &mut out);
        out.extend(payload);
        out
    }

    fn type_section(&self) -> Vec<u8> {
        if self.module.types.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        encode_u32(self.module.types.len() as u32, &mut out);
        for type_ in &self.module.types {
            out.push(0x60);
            encode_vec_types(&type_.params, &mut out);
            encode_vec_types(&type_.results, &mut out);
        }
        out
    }

    fn import_section(&self) -> Vec<u8> {
        if self.module.imports.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        encode_u32(self.module.imports.len() as u32, &mut out);
        for import in &self.module.imports {
            encode_name(&import.module, &mut out);
            encode_name(&import.name, &mut out);
            match &import.desc {
                ImportDesc::Function(type_id) => {
                    out.push(0x00);
                    encode_u32(type_id.0, &mut out);
                }
                ImportDesc::Table(table) => {
                    out.push(0x01);
                    encode_table(table, &mut out);
                }
                ImportDesc::Memory(memory) => {
                    out.push(0x02);
                    encode_limits(memory.minimum_pages, memory.maximum_pages, &mut out);
                }
            }
        }
        out
    }

    fn function_section(&self) -> Vec<u8> {
        if self.module.functions.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        encode_u32(self.module.functions.len() as u32, &mut out);
        for function in &self.module.functions {
            encode_u32(function.type_id.0, &mut out);
        }
        out
    }

    fn table_section(&self) -> Vec<u8> {
        if self.module.tables.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        encode_u32(self.module.tables.len() as u32, &mut out);
        for table in &self.module.tables {
            encode_table(table, &mut out);
        }
        out
    }

    fn memory_section(&self) -> Vec<u8> {
        if self.module.memories.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        encode_u32(self.module.memories.len() as u32, &mut out);
        for memory in &self.module.memories {
            encode_limits(memory.minimum_pages, memory.maximum_pages, &mut out);
        }
        out
    }

    fn global_section(&self) -> Vec<u8> {
        if self.module.globals.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        encode_u32(self.module.globals.len() as u32, &mut out);
        for global in &self.module.globals {
            out.push(u8::from(global.type_));
            out.push(u8::from(global.mutable));
            for instruction in &global.init {
                encode_instruction(instruction, &mut out, self.module);
            }
            out.push(0x0b);
        }
        out
    }

    fn export_section(&self) -> Vec<u8> {
        if self.module.exports.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        encode_u32(self.module.exports.len() as u32, &mut out);
        for export in &self.module.exports {
            encode_name(&export.name, &mut out);
            match export.desc {
                ExportDesc::Function(id) => {
                    out.push(0x00);
                    encode_u32(id.0, &mut out);
                }
                ExportDesc::Table(id) => {
                    out.push(0x01);
                    encode_u32(id.0, &mut out);
                }
                ExportDesc::Memory(id) => {
                    out.push(0x02);
                    encode_u32(id.0, &mut out);
                }
            }
        }
        out
    }

    fn code_section(&self) -> Vec<u8> {
        if self.module.functions.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        encode_u32(self.module.functions.len() as u32, &mut out);
        for function in &self.module.functions {
            let mut body = Vec::new();
            encode_locals(&function.locals, &mut body);
            for instruction in &function.body {
                encode_instruction(instruction, &mut body, self.module);
            }
            body.push(0x0b);
            encode_u32(body.len() as u32, &mut out);
            out.extend(body);
        }
        out
    }

    fn data_section(&self) -> Vec<u8> {
        if self.module.data_segments.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        encode_u32(self.module.data_segments.len() as u32, &mut out);
        for segment in &self.module.data_segments {
            if segment.memory.0 == 0 {
                out.push(0x00);
            } else {
                out.push(0x02);
                encode_u32(segment.memory.0, &mut out);
            }
            for instruction in &segment.offset {
                encode_instruction(instruction, &mut out, self.module);
            }
            out.push(0x0b);
            encode_u32(segment.bytes.len() as u32, &mut out);
            out.extend(&segment.bytes);
        }
        out
    }
}

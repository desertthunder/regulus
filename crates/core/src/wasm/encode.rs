//! Low-level WebAssembly binary encoding helpers.
//!
//! This module contains opcode, LEB128, type, local, memory argument, and block
//! type encoding routines shared by the structured binary emitter.

use super::builder::*;

pub(super) fn encode_instruction(instruction: &Instruction, out: &mut Vec<u8>, module: &Module) {
    match instruction {
        Instruction::Unreachable => out.push(0x00),
        Instruction::Nop => out.push(0x01),
        Instruction::Block { type_, body } => {
            out.push(0x02);
            encode_block_type(type_, out, module);
            for instruction in body {
                encode_instruction(instruction, out, module);
            }
            out.push(0x0b);
        }
        Instruction::Loop { type_, body } => {
            out.push(0x03);
            encode_block_type(type_, out, module);
            for instruction in body {
                encode_instruction(instruction, out, module);
            }
            out.push(0x0b);
        }
        Instruction::If { type_, then_body, else_body } => {
            out.push(0x04);
            encode_block_type(type_, out, module);
            for instruction in then_body {
                encode_instruction(instruction, out, module);
            }
            if !else_body.is_empty() {
                out.push(0x05);
                for instruction in else_body {
                    encode_instruction(instruction, out, module);
                }
            }
            out.push(0x0b);
        }
        Instruction::Br { depth, .. } => {
            out.push(0x0c);
            encode_u32(*depth, out);
        }
        Instruction::BrIf { depth, .. } => {
            out.push(0x0d);
            encode_u32(*depth, out);
        }
        Instruction::Return { .. } => out.push(0x0f),
        Instruction::Call { function, .. } => {
            out.push(0x10);
            encode_u32(function.0, out);
        }
        Instruction::CallIndirect { table, type_id, .. } => {
            out.push(0x11);
            encode_u32(type_id.0, out);
            encode_u32(table.0, out);
        }
        Instruction::Drop(_) => out.push(0x1a),
        Instruction::Select(_) => out.push(0x1b),
        Instruction::LocalGet { local, .. } => {
            out.push(0x20);
            encode_u32(local.0, out);
        }
        Instruction::LocalSet { local, .. } => {
            out.push(0x21);
            encode_u32(local.0, out);
        }
        Instruction::LocalTee { local, .. } => {
            out.push(0x22);
            encode_u32(local.0, out);
        }
        Instruction::GlobalGet { global, .. } => {
            out.push(0x23);
            encode_u32(global.0, out);
        }
        Instruction::GlobalSet { global, .. } => {
            out.push(0x24);
            encode_u32(global.0, out);
        }
        Instruction::I32Const(value) => {
            out.push(0x41);
            encode_i32(*value, out);
        }
        Instruction::I64Const(value) => {
            out.push(0x42);
            encode_i64(*value, out);
        }
        Instruction::F32Const(value) => {
            out.push(0x43);
            out.extend(value.to_le_bytes());
        }
        Instruction::F64Const(value) => {
            out.push(0x44);
            out.extend(value.to_le_bytes());
        }
        Instruction::I32Eqz => out.push(0x45),
        Instruction::I32Eq => out.push(0x46),
        Instruction::I32Ne => out.push(0x47),
        Instruction::I32LtS => out.push(0x48),
        Instruction::I32LtU => out.push(0x49),
        Instruction::I32GtS => out.push(0x4a),
        Instruction::I32GtU => out.push(0x4b),
        Instruction::I32LeS => out.push(0x4c),
        Instruction::I32GeS => out.push(0x4e),
        Instruction::I64Eqz => out.push(0x50),
        Instruction::I64Eq => out.push(0x51),
        Instruction::I64Ne => out.push(0x52),
        Instruction::I64LtS => out.push(0x53),
        Instruction::I64GtS => out.push(0x55),
        Instruction::I64LeS => out.push(0x57),
        Instruction::I64GeS => out.push(0x59),
        Instruction::F64Eq => out.push(0x61),
        Instruction::F64Ne => out.push(0x62),
        Instruction::F64Lt => out.push(0x63),
        Instruction::F64Gt => out.push(0x64),
        Instruction::F64Le => out.push(0x65),
        Instruction::F64Ge => out.push(0x66),
        Instruction::I32Add => out.push(0x6a),
        Instruction::I32Sub => out.push(0x6b),
        Instruction::I32And => out.push(0x71),
        Instruction::I32Mul => out.push(0x6c),
        Instruction::I32DivS => out.push(0x6d),
        Instruction::I32ShrU => out.push(0x76),
        Instruction::I64Add => out.push(0x7c),
        Instruction::I64Sub => out.push(0x7d),
        Instruction::I64Mul => out.push(0x7e),
        Instruction::I64DivS => out.push(0x7f),
        Instruction::I64RemS => out.push(0x81),
        Instruction::I64ExtendI32U => out.push(0xad),
        Instruction::I64ReinterpretF64 => out.push(0xbd),
        Instruction::F64Add => out.push(0xa0),
        Instruction::F64Sub => out.push(0xa1),
        Instruction::F64Mul => out.push(0xa2),
        Instruction::F64Div => out.push(0xa3),
        Instruction::I32Load(arg) => encode_memory_instruction(0x28, arg, out),
        Instruction::I32Load8U(arg) => encode_memory_instruction(0x2d, arg, out),
        Instruction::I64Load(arg) => encode_memory_instruction(0x29, arg, out),
        Instruction::F64Load(arg) => encode_memory_instruction(0x2b, arg, out),
        Instruction::I32Store(arg) => encode_memory_instruction(0x36, arg, out),
        Instruction::I32Store8(arg) => encode_memory_instruction(0x3a, arg, out),
        Instruction::I64Store(arg) => encode_memory_instruction(0x37, arg, out),
        Instruction::F64Store(arg) => encode_memory_instruction(0x39, arg, out),
        Instruction::MemorySize(memory) => {
            out.push(0x3f);
            encode_u32(memory.0, out);
        }
        Instruction::MemoryGrow(memory) => {
            out.push(0x40);
            encode_u32(memory.0, out);
        }
    }
}

pub(super) fn encode_memory_instruction(opcode: u8, arg: &MemoryArg, out: &mut Vec<u8>) {
    out.push(opcode);
    encode_u32(arg.align, out);
    encode_u32(arg.offset, out);
}

pub(super) fn encode_locals(locals: &[Local], out: &mut Vec<u8>) {
    let mut groups: Vec<(u32, ValueType)> = Vec::new();
    for local in locals {
        match groups.last_mut() {
            Some((count, type_)) if *type_ == local.type_ => *count += 1,
            _ => groups.push((1, local.type_)),
        }
    }
    encode_u32(groups.len() as u32, out);
    for (count, type_) in groups {
        encode_u32(count, out);
        out.push(u8::from(type_));
    }
}

pub(super) fn encode_block_type(type_: &BlockType, out: &mut Vec<u8>, module: &Module) {
    match (type_.params.as_slice(), type_.results.as_slice()) {
        ([], []) => out.push(0x40),
        ([], [result]) => out.push(u8::from(*result)),
        _ => {
            let type_id = module
                .types
                .iter()
                .position(|candidate| candidate == &FunctionType::new(type_.params.clone(), type_.results.clone()))
                .expect("validated block type should exist in type section");
            encode_i64(type_id as i64, out);
        }
    }
}

pub(super) fn encode_table(table: &Table, out: &mut Vec<u8>) {
    out.push(u8::from(table.element_type));
    encode_limits(table.minimum, table.maximum, out);
}

pub(super) fn encode_limits(minimum: u32, maximum: Option<u32>, out: &mut Vec<u8>) {
    match maximum {
        Some(maximum) => {
            out.push(0x01);
            encode_u32(minimum, out);
            encode_u32(maximum, out);
        }
        None => {
            out.push(0x00);
            encode_u32(minimum, out);
        }
    }
}

pub(super) fn encode_vec_types(types: &[ValueType], out: &mut Vec<u8>) {
    encode_u32(types.len() as u32, out);
    for type_ in types {
        out.push(u8::from(*type_));
    }
}

impl From<ValueType> for u8 {
    fn from(type_: ValueType) -> Self {
        match type_ {
            ValueType::I32 => 0x7f,
            ValueType::I64 => 0x7e,
            ValueType::F32 => 0x7d,
            ValueType::F64 => 0x7c,
            ValueType::FuncRef => 0x70,
            ValueType::ExternRef => 0x6f,
        }
    }
}

pub(super) fn encode_name(name: &str, out: &mut Vec<u8>) {
    encode_u32(name.len() as u32, out);
    out.extend(name.as_bytes());
}

pub(super) fn encode_u32(mut value: u32, out: &mut Vec<u8>) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            break;
        }
    }
}

pub(super) fn encode_i32(value: i32, out: &mut Vec<u8>) {
    encode_i64(value as i64, out);
}

pub(super) fn encode_i64(mut value: i64, out: &mut Vec<u8>) {
    loop {
        let byte = (value as u8) & 0x7f;
        value >>= 7;
        let done = (value == 0 && (byte & 0x40) == 0) || (value == -1 && (byte & 0x40) != 0);
        out.push(if done { byte } else { byte | 0x80 });
        if done {
            break;
        }
    }
}

impl From<ReferenceType> for u8 {
    fn from(type_: ReferenceType) -> Self {
        match type_ {
            ReferenceType::FuncRef => 0x70,
            ReferenceType::ExternRef => 0x6f,
        }
    }
}

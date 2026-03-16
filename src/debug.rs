use crate::chunk::*;

pub struct Disassembler<'c> {
    pub chunk: &'c Chunk,
}

impl<'c> Disassembler<'c> {
    pub fn new(chunk: &'c Chunk) -> Self {
        Self { chunk }
    }

    pub fn dasm_chunk(&self, name: &str) {
        println!("== {} ==", name);
        for (offset, code) in self.chunk.code.iter().enumerate() {
            self.dasm_instruction(offset, code)
        }
    }

    pub fn dasm_instruction(&self, offset: usize, code: &OpCode) {
        print!("{:04}   ", offset);

        if offset > 0 && self.chunk.lines[offset] == self.chunk.lines[offset - 1] {
            print!("   | ");
        } else {
            print!("{:4} ", self.chunk.lines[offset]);
        }

        match code {
            OpCode::Constant(c) => self.const_instruction("OP_CONSTANT", *c),
            OpCode::Nil => println!("OP_NIL"),
            OpCode::True => println!("OP_TRUE"),
            OpCode::False => println!("OP_FALSE"),
            OpCode::Pop => println!("OP_POP"),
            OpCode::SetLocal(c) => self.byte_instruction("OP_SET_LOCAL", *c), 
            OpCode::GetLocal(c) => self.byte_instruction("OP_GET_LOCAL", *c), 
            OpCode::SetGlobal(c) => self.const_instruction("OP_SET_GLOBAL", *c),
            OpCode::GetGlobal(c) => self.const_instruction("OP_GET_GLOBAL", *c),
            OpCode::DefineGlobal(c) => self.const_instruction("OP_DEFINE_GLOBAL", *c),
            OpCode::Equal => println!("OP_EQUAL"),
            OpCode::Greater => println!("OP_GREATER"),
            OpCode::Less => println!("OP_LESS"),
            OpCode::Negate => println!("OP_NEGATE"),
            OpCode::Print => println!("OP_PRINT"),
            OpCode::Return => println!("OP_RETURN"),
            OpCode::Add => println!("OP_ADD"),
            OpCode::Subtract => println!("OP_SUBTRACT"),
            OpCode::Multiply => println!("OP_MULTIPLY"),
            OpCode::Divide => println!("OP_DIVIDE"),
            OpCode::JumpIfFalse(offset) => self.jump_instruction("OP_JUMP", *offset),
            OpCode::Jump(offset) => self.jump_instruction("OP_JUMP", *offset),
            _ => println!("Unknown opcode: {:?}", code),
        }
    }

    pub fn const_instruction(&self, name: &str, offset: u8) {
        println!(
            "{:<16} {:4} {}",
            name, offset, self.chunk.constants[offset as usize]
        );
    }

    pub fn byte_instruction(&self, name: &str, offset: u8) {
        println!(
            "{:<16} {:4} {:?}",
            name, offset, self.chunk.code[offset as usize + 1]
        );
    }

    pub fn jump_instruction(&self, name: &str, offset: u16) {
        println!(
            "{:<16} {:4} -> {:?}",
            name, offset, self.chunk.code[offset as usize + 1]
        );
    }

}

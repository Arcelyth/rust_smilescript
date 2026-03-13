mod chunk;
use chunk::*;

mod debug;
use debug::*;

mod value;
use value::*;

fn main() {
    let mut chunk = Chunk::new();
    
    let constant = chunk.add_constant(Value::Number(1.2));

    chunk.write(OpCode::OpConstant(constant as u8), 123);
    chunk.write(OpCode::OpReturn, 123);

    let disassembler = Disassembler::new(&chunk);
    disassembler.dasm_chunk("test chunk");
}

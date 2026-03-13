mod chunk;
use chunk::*;

mod debug;
use debug::*;

mod value;
use value::*;

mod vm;
use vm::*;

mod error;

fn main() {
    let mut vm = Vm::new();
    
    let constant = vm.chunk.add_constant(Value::Number(1.2));

    vm.chunk.write(OpCode::Constant(constant as u8), 123);
    vm.chunk.write(OpCode::Negate, 123);
    vm.chunk.write(OpCode::Return, 123);

    let disassembler = Disassembler::new(&vm.chunk);
    disassembler.dasm_chunk("test chunk");
    vm.interpret().unwrap();
}

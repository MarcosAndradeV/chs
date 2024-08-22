pub mod instructions;
use instructions::{Bytecode, Instr};

pub fn jump(addr: usize, rel: isize) -> usize {
    (addr as isize + rel) as usize
}
pub fn jump_to(addr: usize, other: usize) -> isize {
    other as isize - addr as isize
}

type Value = i32;

pub fn vm_run(program: Bytecode) {
    let mut stack: Vec<Value> = Vec::with_capacity(1024);
    // let mut tstack: Vec<Value> = Vec::with_capacity(1024);
    // let mut rstack: Vec<usize> = Vec::with_capacity(1024);
    let mut ip = program.entry;
    // for p in &program.program { println!("{:?}", p) }
    loop {
        if ip >= program.program.len() {
            break;
        }
        let mut next_addr = ip + 1;
        match program.program[ip] {
            Instr::Halt => {
                next_addr = program.program.len();
            }
            Instr::PushI32(v) => stack.push(v),
            Instr::Drop => {
                stack.pop();
            }
            Instr::Debug => {
                println!("Debug:\nData Stack: {:?}", stack);
            }
            Instr::Jmp(rel_addr) => {
                next_addr = jump(ip, rel_addr);
            }
            Instr::JmpIf(rel_addr) => {
                let test = stack.pop().unwrap();
                if test != 0 {
                    next_addr = jump(ip, rel_addr);
                }
            }
            Instr::AJmp(abs_addr) => {
                next_addr = abs_addr;
            }
            Instr::AJmpIf(abs_addr) => {
                let test = stack.pop().unwrap();
                if test != 0 {
                    next_addr = abs_addr;
                }
            }
            Instr::PlusI => {
                let a = stack.pop().unwrap();
                let b = stack.pop().unwrap();
                stack.push(a + b);
            }
        }
        ip = next_addr
    }
}

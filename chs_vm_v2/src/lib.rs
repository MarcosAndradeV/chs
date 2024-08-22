use instructions::Bytecode;

pub mod instructions;

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
            instructions::Instr::Halt => {
                next_addr = program.program.len();
            }
            instructions::Instr::PushI32(v) => stack.push(v),
            instructions::Instr::Drop => {
                stack.pop();
            }
            instructions::Instr::Debug => {
                println!("Debug:\nData Stack: {:?}", stack);
            }
        }
        ip = next_addr
    }
}

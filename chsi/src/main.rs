use std::{env, fs::File, io::Read, process::exit};

use chs_parser::{parse_file, Operation};
use chs_vm_v2::{
    instructions::{Bytecode, Instr},
    vm_run,
};

fn main() {
    let mut args = env::args();
    let _program = args.next().expect("Program always provided.");
    if let Some(filepath) = args.next() {
        if let Ok(mut file) = File::open(filepath.clone()) {
            let mut buf = Vec::new();
            let _ = file.read_to_end(&mut buf);
            let program = parse_file(buf, filepath);
            // dbg!(program);
            let b = compile(program);
            vm_run(b);
        } else {
            exit(-1)
        }
    }
}

fn compile(ops: Vec<Operation>) -> Bytecode {
    let mut p = vec![];
    for op in ops {
        foo(op, &mut p);
    }
    Bytecode::new(p)
}

fn foo(op: Operation, p: &mut Vec<Instr>) {
    match op {
        Operation::PushI(i) => p.push(Instr::PushI32(i)),
        Operation::Debug => p.push(Instr::Debug),
        Operation::If(body) => {
            p.push(Instr::Halt); // Placeholder
            let place_horder = p.len();
            for op in body.iter() {
                foo(op, p);
            }
        }
        Operation::Intrinsic(a) if a.as_str() == "+" => p.push(Instr::PlusI),
        _ => todo!(),
    }
}

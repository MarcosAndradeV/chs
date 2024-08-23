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
        Operation::Drop => p.push(Instr::Drop),
        Operation::Dup => p.push(Instr::Dup),
        Operation::If(body) => {
            let place_horder = p.len();
            p.push(Instr::Halt); // Placeholder
            for op in body.to_vec() {
                foo(op, p);
            }
            let curr_len = p.len();
            let elem = unsafe { p.get_unchecked_mut(place_horder) };
            *elem = Instr::JmpIf((curr_len - place_horder) as isize);
        }
        Operation::IfElse(ifbody, elsebody) => {
            let place_horder = p.len();
            p.push(Instr::Halt); // Placeholder
            for op in ifbody.to_vec() {
                foo(op, p);
            }
            let offset2 = p.len();
            p.push(Instr::Jmp(0));
            let elem = unsafe { p.get_unchecked_mut(place_horder) };
            *elem = Instr::JmpIf((offset2 - (place_horder) + 1) as isize);
            for op in elsebody.to_vec() {
                foo(op, p);
            }
            let curr_len = p.len();
            let elem = unsafe { p.get_unchecked_mut(offset2) };
            *elem = Instr::Jmp((curr_len - offset2) as isize);
        }
        Operation::While(cond, body) => {
            let whileaddrs = p.len();
            for e in cond.to_vec() {
                foo(e, p);
            }
            let ifaddrs = p.len();
            p.push(Instr::JmpIf(0));
            for e in body.to_vec() {
                foo(e, p);
            }
            let curr_len = p.len();
            p.push(Instr::Jmp(-((curr_len - whileaddrs) as isize)));
            let curr_len = p.len();
            let elem = unsafe { p.get_unchecked_mut(ifaddrs) };
            *elem = Instr::JmpIf((curr_len - ifaddrs) as isize);
        }
        Operation::Bind(n) => p.push(Instr::Bind(n)),
        Operation::Intrinsic(a) if a.as_str() == "+" => p.push(Instr::PlusI),
        Operation::Intrinsic(a) if a.as_str() == "==" => p.push(Instr::EqI),
        Operation::Intrinsic(a) if a.as_str() == "!=" => p.push(Instr::NEqI),
        e => {
            dbg!(e);
            todo!()
        }
    }
}

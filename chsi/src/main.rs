use std::{env, fs::File, io::Read, process::exit};

use chs_parser::{parse_file, Operation};
use chs_vm::{instructions::Instr, value::Value};

fn main() {
    let mut args = env::args();
    let _program = args.next().expect("Program always provided.");
    if let Some(filepath) = args.next() {
        if let Ok(mut file) = File::open(filepath.clone()) {
            let mut buf = Vec::new();
            let _ = file.read_to_end(&mut buf);
        } else {
            exit(-1)
        }
    }
}

fn foo(data: Vec<u8>) {
    let ops = parse_file(data, "...".to_string());
    for op in ops {
        match op {
            Operation::PushI(a) => Instr::Const(Value::Int64(a as i64)),
            Operation::Drop => Instr::Pop,
            Operation::Plus => Instr::Add,
        };
    }
}

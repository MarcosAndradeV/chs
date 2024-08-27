use std::{collections::HashMap, process::exit};

use chs_parser::{DataType, Operation};

type TypeStack = Vec<DataType>;

#[derive(Debug, Default)]
struct TypeContext {
    stack: TypeStack,
    ip: usize,
    fndefs: HashMap<String, usize>,
    memdefs: HashMap<String, usize>,
}

pub fn check_program(program: &Vec<Operation>) {
    let mut ctx = TypeContext::default(); // Inicializar o contexto
    check_program_ops(&mut ctx, program); // Analize de operações
    if ctx.stack.len() != 0 {
        eprintln!("Unhandled data on stack at the end of program");
        dbg!(ctx.stack);
        exit(-1);
    }
}

fn check_program_ops(ctx: &mut TypeContext, program: &Vec<Operation>) {
    while ctx.ip < program.len() {
        match &program[ctx.ip] {
            Operation::Debug => {
                ctx.ip += 1;
                continue;
            }
            Operation::Alloc(name, _) => {
                ctx.memdefs.insert(name.clone(), 0);
                ctx.ip += 1;
                continue;
            }
            Operation::Word(name) => {
                if let Some(fnn) = ctx.fndefs.get(name) {
                    if let Some(Operation::Fn(_, _, ins, outs, _)) = program.get(*fnn) {
                        if ins.len() > ctx.stack.len() {
                            eprintln!("Unsifsient data on stack for fn {}", name);
                            exit(-1);
                        }
                        for (expect, actual) in ins.iter().rev().zip(ctx.stack.iter().rev()) {
                            if actual != expect {
                                eprintln!(
                                    "Expected Type {:?} got {:?} in {}",
                                    expect, actual, name
                                );
                                exit(-1);
                            }
                        }

                        ctx.stack
                            .truncate(ctx.stack.len().saturating_sub(ins.len()));
                        ctx.stack.extend(outs.iter());
                    }
                } else if ctx.memdefs.contains_key(name) {
                    ctx.stack.push(DataType::Ptr);
                } else {
                    eprintln!("Unkwon word {}", name);
                    exit(-1);
                }
                ctx.ip += 1;
                continue;
            }
            Operation::Intrinsic(s) => {
                check_intrinsic(s, ctx);
                ctx.ip += 1;
                continue;
            }
            Operation::PushI(_) => {
                ctx.stack.push(DataType::Int);
                ctx.ip += 1;
                continue;
            }
            Operation::Write(_) => {
                // (ptr int -> )
                if ctx.stack.len() < 2 {
                    eprintln!("Not enough arguments for `!`.");
                    exit(-1);
                }
                if let Some(frame) = ctx.stack.last() {
                    // b
                    if *frame != DataType::Ptr {
                        eprintln!("Expected Type `ptr` for `!`, found {:?}", frame);
                        eprintln!("Type Stack: ");
                        eprintln!("\tActual: {:?}", ctx.stack);
                        eprintln!("\tExpected: {:?}", [DataType::Int, DataType::Ptr]);
                        exit(-1);
                    }
                    ctx.stack.pop();
                }
                if let Some(frame) = ctx.stack.pop() {
                    // a
                    if frame != DataType::Int {
                        eprintln!("Expected Type `int` for `!`, found {:?}", frame);
                        eprintln!("Type Stack: {:?}", ctx.stack);
                        exit(-1);
                    }
                }
                ctx.ip += 1;
                continue;
            }
            Operation::Read(_) => {
                // (ptr -> int)
                if ctx.stack.len() < 1 {
                    eprintln!("Not enough arguments for `@` TODO");
                    exit(-1);
                }
                if let Some(frame) = ctx.stack.pop() {
                    // a
                    if frame != DataType::Ptr {
                        eprintln!("Typeof b `@` Actual: {:?} TODO", frame);
                        exit(-1);
                    }
                }
                ctx.stack.push(DataType::Int);
                ctx.ip += 1;
                continue;
            }
            Operation::If(then) => {
                // (bool ->)
                if let Some(frame) = ctx.stack.last() {
                    if *frame != DataType::Bool {
                        eprintln!(
                            "Expected type on `if` must be Bool. Actual: {:?} TODO",
                            frame
                        );
                        exit(-1);
                    }
                } else {
                    eprintln!("Empyt stack on `if`",);
                    exit(-1);
                }
                let _ = ctx.stack.pop();
                let next_ip = ctx.ip + 1;
                ctx.ip = 0;
                check_program_ops(ctx, &then.to_vec());
                ctx.ip = next_ip;
                continue;
            }
            Operation::IfElse(then, else_) => {
                // (bool ->)
                if let Some(frame) = ctx.stack.last() {
                    if *frame != DataType::Bool {
                        eprintln!(
                            "Expected type on `if` must be Bool. Actual: {:?} TODO",
                            frame
                        );
                        exit(-1);
                    }
                } else {
                    eprintln!("Empyt stack on `if`",);
                    exit(-1);
                }
                let _ = ctx.stack.pop();
                let next_ip = ctx.ip + 1;
                ctx.ip = 0;
                check_program_ops(ctx, &then.to_vec());
                ctx.ip = 0;
                check_program_ops(ctx, &else_.to_vec());
                ctx.ip = next_ip;
                continue;
            }
            Operation::While(cond, body) => {
                let next_ip = ctx.ip + 1;
                let tmp = ctx.stack.clone();
                ctx.ip = 0;
                check_program_ops(ctx, &cond.to_vec());
                if let Some(frame) = ctx.stack.pop() {
                    if frame != DataType::Bool {
                        eprintln!("While TODO");
                        exit(-1);
                    }
                } else {
                    eprintln!("While TODO");
                    exit(-1);
                }
                ctx.ip = 0;
                check_program_ops(ctx, &body.to_vec());
                if ctx.stack.len() != tmp.len() {
                    eprintln!("Unhandled data on stack after `while`");
                    eprintln!("Type Stack: ");
                    eprintln!("\tActual: {:?}", ctx.stack);
                    eprintln!("\tExpected: {:?}", tmp);
                    exit(-1);
                }
                for (expect, actual) in ctx.stack.iter().rev().zip(tmp.iter().rev()) {
                    if actual != expect {
                        eprintln!("Expected Type {:?} got {:?}", expect, actual);
                        exit(-1);
                    }
                }
                ctx.stack = tmp;
                ctx.ip = next_ip;
                continue;
            }
            Operation::Bind(i) => {
                // (any. . . -> any)
                if ctx.stack.len() < (*i) as usize {
                    eprintln!("Bind TODO");
                    exit(-1);
                }
                let a = ctx.stack[ctx.stack.len().saturating_sub((*i) as usize)];
                ctx.stack.push(a);
                ctx.ip += 1;
                continue;
            }
            Operation::Assing(_, _) => todo!(),
            Operation::Let(_, _, _) => todo!(),
            Operation::Fn(name, _, ins, outs, body) => {
                let redef = ctx.fndefs.insert(name.clone(), ctx.ip);
                if redef.is_some() {
                    eprintln!("Redefinition of fn {}", name);
                    exit(-1);
                }
                let mut fn_ctx = TypeContext::default();
                fn_ctx.stack.extend(&ins.to_vec());
                check_program_ops(&mut fn_ctx, &body.to_vec());
                if fn_ctx.stack.len() != outs.len() {
                    eprintln!("Unhandled data on stack in fn {}", name);
                    eprintln!("Type Stack: ");
                    eprintln!("\tActual: {:?}", fn_ctx.stack);
                    eprintln!("\tExpected: {:?}", ctx.stack);
                    exit(-1);
                }
                for (expect, actual) in outs.iter().rev().zip(fn_ctx.stack.iter().rev()) {
                    if actual != expect {
                        eprintln!("Expected Type {:?} got {:?} in {}", expect, actual, name);
                        exit(-1);
                    }
                }
                ctx.ip += 1;
                continue;
            }
        }
    }
}

fn check_intrinsic(s: &String, ctx: &mut TypeContext) {
    match s.as_str() {
        "drop" => {
            // (a ->)
            if ctx.stack.len() < 1 {
                eprintln!("Drop TODO");
                exit(-1);
            }
            let _ = ctx.stack.pop();
        }
        "dup" => {
            // (a -> a a)
            if ctx.stack.len() < 1 {
                eprintln!("Dup TODO");
                exit(-1);
            }
            let a = ctx.stack.pop().unwrap();
            ctx.stack.push(a);
            ctx.stack.push(a);
        }
        "swap" => {
            // (a b -> b a)
            if ctx.stack.len() < 2 {
                eprintln!("Unsifsient data on stack for `swap`");
                exit(-1);
            }
            let b = ctx.stack.pop().unwrap();
            let a = ctx.stack.pop().unwrap();
            ctx.stack.push(b);
            ctx.stack.push(a);
        }
        "over" => {
            // (a b -> a b a)
            if ctx.stack.len() < 2 {
                dbg!(&ctx.stack);
                eprintln!("Unsifsient data on stack for `over`");
                exit(-1);
            }
            let b = ctx.stack.pop().unwrap();
            let a = ctx.stack.pop().unwrap();
            ctx.stack.push(a);
            ctx.stack.push(b);
            ctx.stack.push(a);
        }
        "rot" => {
            // (a b c -> b c a)
            if ctx.stack.len() < 3 {
                eprintln!("Unsifsient data on stack for `rot`");
                exit(-1);
            }
            let c = ctx.stack.pop().unwrap();
            let b = ctx.stack.pop().unwrap();
            let a = ctx.stack.pop().unwrap();
            ctx.stack.push(b);
            ctx.stack.push(c);
            ctx.stack.push(a);
        }
        "*" => {
            // (int int -> int)
            if ctx.stack.len() < 2 {
                eprintln!("Args + TODO");
                exit(-1);
            }
            if let Some(frame) = ctx.stack.pop() {
                // b
                if frame != DataType::Int {
                    eprintln!("Typeof b * Actual: {:?} TODO", frame);
                    exit(-1);
                }
            }
            if let Some(frame) = ctx.stack.pop() {
                // a
                if frame != DataType::Int {
                    eprintln!("Typeof a * TODO");
                    exit(-1);
                }
            }
            ctx.stack.push(DataType::Int)
        }
        "+" => {
            // (int int -> int)
            if ctx.stack.len() < 2 {
                eprintln!("Args + TODO");
                exit(-1);
            }
            if let Some(frame) = ctx.stack.pop() {
                // b
                if frame != DataType::Int {
                    eprintln!("Typeof b + Actual: {:?} TODO", frame);
                    exit(-1);
                }
            }
            if let Some(frame) = ctx.stack.pop() {
                // a
                if frame != DataType::Int {
                    eprintln!("Typeof a + TODO");
                    exit(-1);
                }
            }
            ctx.stack.push(DataType::Int)
        }
        "<" => {
            // (int int -> bool)
            if ctx.stack.len() < 2 {
                eprintln!("Unsifsient data on stack for `<`");
                exit(-1);
            }
            if let Some(frame) = ctx.stack.pop() {
                // b
                if frame != DataType::Int {
                    eprintln!("Typeof b `<` Actual: {:?} TODO", frame);
                    exit(-1);
                }
            }
            if let Some(frame) = ctx.stack.pop() {
                // a
                if frame != DataType::Int {
                    eprintln!("Typeof a `<` Actual: {:?} TODO", frame);
                    exit(-1);
                }
            }
            ctx.stack.push(DataType::Bool)
        }
        "mod" => {
            // (int int -> int)
            if ctx.stack.len() < 2 {
                eprintln!("Unsifsient data on stack for `mod`");
                exit(-1);
            }
            if let Some(frame) = ctx.stack.pop() {
                // b
                if frame != DataType::Int {
                    eprintln!("Typeof b `mod` Actual: {:?} TODO", frame);
                    exit(-1);
                }
            }
            if let Some(frame) = ctx.stack.pop() {
                // a
                if frame != DataType::Int {
                    eprintln!("Typeof a `mod` Actual: {:?} TODO", frame);
                    exit(-1);
                }
            }
            ctx.stack.push(DataType::Int)
        }
        "!=" => {
            // (int int -> bool)
            if ctx.stack.len() < 2 {
                eprintln!("!= TODO");
                exit(-1);
            }
            if let Some(frame) = ctx.stack.pop() {
                // b
                if frame != DataType::Int {
                    eprintln!("!= TODO");
                    exit(-1);
                }
            } else {
                eprintln!("!= TODO");
                exit(-1);
            }
            if let Some(frame) = ctx.stack.pop() {
                // a
                if frame != DataType::Int {
                    eprintln!("!= TODO");
                    exit(-1);
                }
            } else {
                eprintln!("!= TODO");
                exit(-1);
            }
            ctx.stack.push(DataType::Bool)
        }
        a => {
            eprintln!("Unkwon instrinsic `{}` in type check", a);
            exit(-1);
        }
    }
}

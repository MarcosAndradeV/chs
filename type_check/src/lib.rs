use std::process::exit;

use chs_parser::Operation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DataType {
    Int,
    Ptr,
    Bool,
}

type TypeStack = Vec<DataType>;

#[derive(Debug, Default)]
struct TypeContext {
    stack: TypeStack,
    ip: usize,
}

pub fn check_program(program: &Vec<Operation>) {
    let mut ctx = TypeContext::default(); // Inicializar o contexto
    check_program_ops(&mut ctx, program); // Analize de operações
}

fn check_program_ops(ctx: &mut TypeContext, program: &Vec<Operation>) {
    while ctx.ip < program.len() {
        match &program[ctx.ip] {
            Operation::Drop => {
                // (any ->)
                if ctx.stack.len() < 1 {
                    eprintln!("Drop TODO");
                    exit(-1);
                }
                let _ = ctx.stack.pop();
                ctx.ip += 1;
                continue;
            }
            Operation::Debug => {
                ctx.ip += 1;
                continue;
            }
            Operation::Dup => {
                // (any -> any any)
                if ctx.stack.len() < 1 {
                    eprintln!("Dup TODO");
                    exit(-1);
                }
                let a = ctx.stack.last().unwrap().clone();
                ctx.stack.push(a);
                ctx.ip += 1;
                continue;
            }
            Operation::Word(_) => todo!(),
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
            Operation::If(then) => {
                // (bool ->)
                if let Some(frame) = ctx.stack.last() {
                    if *frame != DataType::Bool {
                        eprintln!("If TODO");
                        exit(-1);
                    }
                } else {
                    eprintln!("If TODO");
                    exit(-1);
                }
                let _ = ctx.stack.pop();
                let next_ip = ctx.ip;
                ctx.ip = 0;
                check_program_ops(ctx, &then.to_vec());
                ctx.ip = next_ip;
                continue;
            }
            Operation::IfElse(then, else_) => {
                // (bool ->)
                if let Some(frame) = ctx.stack.last() {
                    if *frame != DataType::Bool {
                        eprintln!("If TODO");
                        exit(-1);
                    }
                } else {
                    eprintln!("If TODO");
                    exit(-1);
                }
                let _ = ctx.stack.pop();
                let next_ip = ctx.ip;
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
                if ctx.stack != tmp {
                    eprintln!("While TODO");
                    exit(-1);
                }
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
            Operation::Fn(_, _, _, _, _) => todo!(),
        }
    }
}

fn check_intrinsic(s: &String, ctx: &mut TypeContext) {
    match s.as_str() {
        "+" => {
            // (int int -> int)
            if ctx.stack.len() < 2 {
                eprintln!("Args + TODO");
                exit(-1);
            }
            if let Some(frame) = ctx.stack.pop() {
                // a
                if frame != DataType::Int {
                    eprintln!("Typeof a + TODO");
                    exit(-1);
                }
            }
            if let Some(frame) = ctx.stack.pop() {
                // b
                if frame != DataType::Int {
                    eprintln!("Typeof b + Actual: {:?} TODO", frame);
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
                // a
                if frame != DataType::Int {
                    eprintln!("!= TODO");
                    exit(-1);
                }
            } else {
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
            ctx.stack.push(DataType::Bool)
        }
        _ => todo!(),
    }
}

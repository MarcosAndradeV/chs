use std::collections::HashMap;

use chs_parser::Operation;

use crate::instructions::{Bytecode, Instr};

#[derive(Debug, Default)]
struct CompCtx {
    instr: Vec<Instr>,
    fn_def: HashMap<String, usize>,
}

pub fn compile(ops: Vec<Operation>) -> Bytecode {
    let mut ctx = CompCtx::default();
    let mut ops_iter = ops.into_iter();
    while let Some(expr) = ops_iter.next() {
        compile_op(&mut ctx, expr);
    }
    Bytecode {
        program: ctx.instr,
        entry: 0,
    }
}

fn compile_op(ctx: &mut CompCtx, op: Operation) {
    match op {
        Operation::PushI(i) => ctx.instr.push(Instr::PushI32(i)),
        Operation::Debug => ctx.instr.push(Instr::Debug),
        Operation::If(body) => {
            let offset = ctx.instr.len();
            ctx.instr.push(Instr::JmpIf(0));
            for op in body.to_vec() {
                compile_op(ctx, op)
            }
            let curr_len = ctx.instr.len();
            let elem = unsafe { ctx.instr.get_unchecked_mut(offset) };
            *elem = Instr::JmpIf((curr_len - offset) as isize);
        }
        Operation::IfElse(ifbody, elsebody) => {
            let place_horder = ctx.instr.len();
            ctx.instr.push(Instr::Halt); // Placeholder
            for op in ifbody.to_vec() {
                compile_op(ctx, op)
            }
            let offset2 = ctx.instr.len();
            ctx.instr.push(Instr::Jmp(0));
            let elem = unsafe { ctx.instr.get_unchecked_mut(place_horder) };
            *elem = Instr::JmpIf((offset2 - (place_horder) + 1) as isize);
            for op in elsebody.to_vec() {
                compile_op(ctx, op)
            }
            let curr_len = ctx.instr.len();
            let elem = unsafe { ctx.instr.get_unchecked_mut(offset2) };
            *elem = Instr::Jmp((curr_len - offset2) as isize);
        }
        Operation::While(cond, body) => {
            let whileaddrs = ctx.instr.len();
            for op in cond.to_vec() {
                compile_op(ctx, op)
            }
            let ifaddrs = ctx.instr.len();
            ctx.instr.push(Instr::JmpIf(0));
            for op in body.to_vec() {
                compile_op(ctx, op)
            }
            let curr_len = ctx.instr.len();
            ctx.instr
                .push(Instr::Jmp(-((curr_len - whileaddrs) as isize)));
            let curr_len = ctx.instr.len();
            let elem = unsafe { ctx.instr.get_unchecked_mut(ifaddrs) };
            *elem = Instr::JmpIf((curr_len - ifaddrs) as isize);
        }
        Operation::Bind(n) => ctx.instr.push(Instr::Bind(n)),
        Operation::Intrinsic(a) if a.as_str() == "+" => ctx.instr.push(Instr::PlusI),
        Operation::Intrinsic(a) if a.as_str() == "*" => ctx.instr.push(Instr::MultI),
        Operation::Intrinsic(a) if a.as_str() == "==" => ctx.instr.push(Instr::EqI),
        Operation::Intrinsic(a) if a.as_str() == "!=" => ctx.instr.push(Instr::NEqI),
        Operation::Intrinsic(a) if a.as_str() == "drop" => ctx.instr.push(Instr::Drop),
        Operation::Intrinsic(a) if a.as_str() == "dup" => ctx.instr.push(Instr::Dup),
        Operation::Fn(name, _args, _, _, body) => {
            let addrs = ctx.instr.len();
            ctx.instr.push(Instr::Jmp(0));
            let curr_len = ctx.instr.len();
            ctx.fn_def.insert(name, curr_len);
            for op in body.to_vec() {
                compile_op(ctx, op)
            }
            ctx.instr.push(Instr::Ret);
            let curr_len = ctx.instr.len();
            let elem = unsafe { ctx.instr.get_unchecked_mut(addrs) };
            *elem = Instr::Jmp((curr_len - addrs) as isize);
        }
        Operation::Word(name) => {
            if let Some(fnn) = ctx.fn_def.get(&name) {
                ctx.instr.push(Instr::Call(*fnn));
            }
        }
        e => {
            dbg!(e);
            todo!()
        }
    }
}

/*

*/

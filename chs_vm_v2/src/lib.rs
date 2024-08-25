pub mod instructions;
use core::fmt;
use std::marker::PhantomData;

use instructions::{Bytecode, Instr};
use memory::Memory;

pub fn jump(addr: usize, rel: isize) -> usize {
    (addr as isize + rel) as usize
}
pub fn jump_to(addr: usize, other: usize) -> isize {
    other as isize - addr as isize
}

type Value = u64;

#[derive(Debug)]
struct VMStack<T: Sized> {
    marker: PhantomData<T>,
    data: Memory,
    top: usize,
}

impl fmt::Display for VMStack<Value> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut buf = String::from("[");
        let mut i = self.data.size() - size_of::<Value>();
        while i >= self.top {
            buf.push_str(&format!(" {} ", self.data.read::<Value>(i)));
            i -= size_of::<Value>()
        }
        write!(f, "{}]", buf)
    }
}

impl VMStack<Value> {
    pub fn new(size: usize) -> Self {
        let data = Memory::new(size);
        let top = size; // - size_of::<Value>();
        Self {
            data,
            top,
            marker: PhantomData::default(),
        }
    }
    pub fn push(&mut self, value: Value) {
        self.top -= size_of::<Value>();
        self.data.write(self.top, value);
        self.data.set_write_pos(self.top);
    }
    pub fn pop(&mut self) -> Value {
        let index = self.top;
        self.top += size_of::<Value>();
        self.data.read(index)
    }
    pub fn len(&self) -> usize {
        self.data.size() / size_of::<Value>()
    }
}

pub fn vm_run(program: Bytecode) {
    // let mut stack: Vec<Value> = Vec::with_capacity(1024);
    let mut stack = VMStack::<Value>::new(1024);
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
            Instr::PushI32(v) => stack.push(v as u64),
            Instr::Drop => {
                stack.pop();
            }
            Instr::Dup => {
                let a = stack.pop();
                stack.push(a);
                stack.push(a);
            }
            Instr::Debug => {
                println!("Debug:\nData Stack: {}", stack);
            }
            Instr::Jmp(rel_addr) => {
                next_addr = jump(ip, rel_addr);
            }
            Instr::JmpIf(rel_addr) => {
                let test = stack.pop();
                if test == 0 {
                    next_addr = jump(ip, rel_addr);
                }
            }
            Instr::AJmp(abs_addr) => {
                next_addr = abs_addr;
            }
            Instr::AJmpIf(abs_addr) => {
                let test = stack.pop();
                if test == 0 {
                    next_addr = abs_addr;
                }
            }
            Instr::PlusI => {
                let a = stack.pop();
                let b = stack.pop();
                stack.push(a + b);
            }
            Instr::EqI => {
                let a = stack.pop();
                let b = stack.pop();
                stack.push((a == b) as u64);
            }
            Instr::NEqI => {
                let a = stack.pop();
                let b = stack.pop();
                stack.push((a != b) as u64);
            }
            Instr::Bind(rel) => {
                let rel = rel as usize * size_of::<Value>();
                assert!(stack.len() >= rel && rel <= stack.top);
                stack.push(stack.data.read((stack.top + rel) - size_of::<Value>()));
            }
        }
        ip = next_addr
    }
}

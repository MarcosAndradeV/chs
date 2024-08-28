pub mod compiler;
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
    let mut strs_size = 0;
    for e in program.strs.iter() {
        strs_size += e.len();
    }
    let mut stack = VMStack::<Value>::new(1024);
    let mut rstack: Vec<usize> = Vec::with_capacity(1024);
    let mut mem = Memory::new(strs_size + program.program_mem);
    mem.set_write_pos(0);
    for e in program.strs.iter() {
        for v in e.iter() {
            mem.write_push::<u8>(*v)
        }
    }
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
            Instr::PushPtr(v) => stack.push(v as u64),
            Instr::Drop => {
                stack.pop();
            }
            Instr::Dup => {
                let a = stack.pop();
                stack.push(a);
                stack.push(a);
            }
            Instr::Swap => {
                let b = stack.pop();
                let a = stack.pop();
                stack.push(b);
                stack.push(a);
            }
            Instr::Over => {
                let b = stack.pop();
                let a = stack.pop();
                stack.push(a);
                stack.push(b);
                stack.push(a);
            }
            Instr::Rot => {
                let c = stack.pop();
                let b = stack.pop();
                let a = stack.pop();
                stack.push(b);
                stack.push(c);
                stack.push(a);
            }
            Instr::Write(bytes) => {
                let b = stack.pop() as usize; // ptr
                let a = stack.pop(); // value
                match bytes {
                    64 => mem.write(b, a as u64),
                    32 => mem.write(b, a as u32),
                    16 => mem.write(b, a as u16),
                    08 => mem.write(b, a as u8),
                    _ => todo!(),
                }
                // unsafe {
                //     match bytes {
                //         64 => *(a as *mut u64) = b,
                //         32 => *(a as *mut u32) = b as u32,
                //         16 => *(a as *mut u16) = b as u16,
                //         8 => *(a as *mut u8) = b as u8,
                //         _ => todo!(),
                //     }
                // }
            }
            Instr::Read(bytes) => {
                let a = stack.pop() as usize; // ptr
                let value = match bytes {
                    64 => mem.read::<u64>(a) as u64,
                    32 => mem.read::<u32>(a) as u64,
                    16 => mem.read::<u16>(a) as u64,
                    08 => mem.read::<u8>(a) as u64,
                    _ => todo!(),
                };
                stack.push(value);

                // unsafe {
                //     let value = match bytes {
                //         64 => *(a as *mut u64) as u64,
                //         32 => *(a as *mut u32) as u64,
                //         16 => *(a as *mut u16) as u64,
                //         8 => *(a as *mut u8) as u64,
                //         _ => todo!(),
                //     };
                //     stack.push(value);
                // }
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
                let b = stack.pop();
                let a = stack.pop();
                stack.push(a + b);
            }
            Instr::MultI => {
                let b = stack.pop();
                let a = stack.pop();
                stack.push(a * b);
            }
            Instr::Offset => {
                let b = stack.pop(); // offset
                let a = stack.pop(); // ptr
                stack.push(a + b);
            }
            Instr::Mod => {
                let b = stack.pop();
                let a = stack.pop();
                stack.push(a % b);
            }
            Instr::EqI => {
                let b = stack.pop();
                let a = stack.pop();
                stack.push((a == b) as u64);
            }
            Instr::NEqI => {
                let b = stack.pop();
                let a = stack.pop();
                stack.push((a != b) as u64);
            }
            Instr::Lt => {
                let b = stack.pop();
                let a = stack.pop();
                stack.push((a < b) as u64);
            }
            Instr::Bind(rel) => {
                let rel = rel as usize * size_of::<Value>();
                assert!(stack.len() >= rel && rel <= stack.top);
                stack.push(stack.data.read((stack.top + rel) - size_of::<Value>()));
            }
            Instr::Ret => {
                next_addr = match rstack.pop() {
                    Some(o) => o,
                    None => break,
                };
            }
            Instr::Call(addr) => {
                rstack.push(next_addr);
                next_addr = addr;
            }
        }
        ip = next_addr
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Instr {
    Halt,
    Drop,
    Dup,
    Rot,
    Over,
    Swap,
    Debug,
    PlusI,
    MultI,
    Mod,
    Offset,
    Lt,
    EqI,
    NEqI,
    Ret,
    Write(usize),   // Bytes
    Read(usize),    // Bytes
    Call(usize),    // addr
    Bind(u32),      // Relative Position
    PushI32(i32),   // Immediate
    PushPtr(usize), // Ptr
    Jmp(isize),     // Relative Address
    JmpIf(isize),   // Relative Address
    AJmp(usize),    // Abslute Address
    AJmpIf(usize),  // Abslute Address
}

#[derive(Debug, Clone)]
pub struct Bytecode {
    pub program: Vec<Instr>,
    pub program_mem: usize,
    pub entry: usize,
}

impl Bytecode {
    pub fn new(program: Vec<Instr>, program_mem: usize) -> Self {
        Self {
            program,
            program_mem,
            entry: 0,
        }
    }
    pub fn len(&self) -> usize {
        self.program.len()
    }
}

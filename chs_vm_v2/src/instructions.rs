#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Instr {
    Halt,
    Drop,
    Debug,
    Dup,
    PlusI,
    EqI,
    NEqI,
    Bind(u32),     // Relative Position
    PushI32(i32),  // Immediate
    Jmp(isize),    // Relative Address
    JmpIf(isize),  // Relative Address
    AJmp(usize),   // Abslute Address
    AJmpIf(usize), // Abslute Address
}

#[derive(Debug, Clone)]
pub struct Bytecode {
    pub program: Vec<Instr>,
    pub entry: usize,
}

impl Bytecode {
    pub fn new(program: Vec<Instr>) -> Self {
        Self { program, entry: 0 }
    }
    pub fn len(&self) -> usize {
        self.program.len()
    }
}

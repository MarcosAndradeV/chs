#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Instr {
    Halt,
    PushI32(i32),
    Drop,
    Debug,
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
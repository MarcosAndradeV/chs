use super::types::Type;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InstId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub u32);

#[derive(Debug, Clone)]
pub enum Operand {
    Null,
    Reg(InstId),
    Int(u64),
    Bool(bool),
    Float(f64),
    String(std::rc::Rc<str>),
    Param(u32),
    Global(std::rc::Rc<str>),
    ThreadLocalGlobal(std::rc::Rc<str>),
}

#[derive(Debug, Clone)]
pub enum Instruction {
    // Arithmetic & Logic
    Add(Operand, Operand),
    Sub(Operand, Operand),
    Mul(Operand, Operand),
    Div(Operand, Operand),
    Mod(Operand, Operand),

    // Relational
    Eq(Type, Operand, Operand),
    NotEq(Type, Operand, Operand),
    Lt(Type, Operand, Operand),
    LtEq(Type, Operand, Operand),
    Gt(Type, Operand, Operand),
    GtEq(Type, Operand, Operand),

    // Logical & Bitwise
    And(Operand, Operand),
    Or(Operand, Operand),
    BitAnd(Operand, Operand),
    BitOr(Operand, Operand),
    BitXor(Operand, Operand),

    // Unary
    Neg(Operand),
    Not(Operand),
    Cast(Operand),

    // Memory
    Alloca(Type),
    Load(Operand),                       // Load(Pointer)
    Store(Type, Operand, Operand),       // Store(Type, Pointer, Value)
    Index(Operand, Operand),             // Index(Array, IndexOffset)
    OOBCheck(Operand, Operand, Operand), // OOBCheck(Message, Index, Len)

    // Calls
    Call(Operand, Box<[Operand]>),  // Call(Callee, Args)
    DynArrayGrow(Operand, Operand), // DynArrayGrow(DynArrayPtr, ElemSize)
    Dealloc(Operand),               // Dealloc(Pointer)

    // Control Flow (Terminators)
    Br(BlockId),
    CondBr(Operand, BlockId, BlockId),
    Return(Option<Operand>),

    // Pointer Projections
    GetMemberPtr(Operand, u32),    // GetMemberPtr(StructPtr, ByteOffset)
    GetIndexPtr(Operand, Operand), // GetIndexPtr(ArrayOrPtr, IndexOffset)
}

#[derive(Debug, Clone)]
pub struct InstData {
    pub inst: Instruction,
    pub ty: Type,
}

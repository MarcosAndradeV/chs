use lex_just_parse::lexer::*;

use types as t;

#[derive(Debug)]
pub struct FileAst {
    pub module: Token,
    pub items: Vec<FileItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArraySize {
    Literal(usize),
    Ident(Token),
}

#[derive(Debug, Clone)]
pub struct ConstDecl {
    pub loc: Loc,
    pub name: Token,
    pub typ: Type,
    pub expr: Expr,
}

#[derive(Debug, Clone)]
pub enum Type {
    Scalar(Token),
    Pointer(u32, Box<Type>),
    Array(Box<Type>, ArraySize),
    Slice(Box<Type>),
    DynArray(Box<Type>),
    Tuple(Vec<Type>, Loc),
    FnPointer {
        loc: Loc,
        parameters: Vec<FunctionPointerParameter>,
        return_type: Option<Box<Type>>,
    },
}

impl Type {
    pub fn loc(&self) -> Loc {
        match self {
            Type::Scalar(token) => token.loc,
            Type::Pointer(_, inner) => inner.loc(),
            Type::Array(inner, _) => inner.loc(),
            Type::Slice(inner) => inner.loc(),
            Type::DynArray(inner) => inner.loc(),
            Type::Tuple(_, loc) => *loc,
            Type::FnPointer { loc, .. } => *loc,
        }
    }
}

#[derive(Debug, Clone)]
pub enum FileItem {
    MethodImplementation(MethodImplementation),
    Function(Function),
    Import(Import),
    Library(Library),
    Struct(Struct),
    Enum(Enum),
    TypeDecl(TypeDecl),
    VarDecl(VarDeclStmt),
    Operator(OperatorOverload),
    Const(ConstDecl),
}

#[derive(Debug, Clone)]
pub struct TypeDecl {
    pub name: Token,
    pub is_distinct: bool,
    pub base_type: Type,
}

#[derive(Debug, Clone)]
pub enum LibraryKind {
    Static,
    Dynlib,
}

impl LibraryKind {
    /// Returns `true` if the library kind is [`Static`].
    ///
    /// [`Static`]: LibraryKind::Static
    #[must_use]
    pub fn is_static(&self) -> bool {
        matches!(self, Self::Static)
    }
}

#[derive(Debug, Clone)]
pub struct Library {
    pub name: Token,
    pub link_name: TokenSource,
    pub kind: LibraryKind,
}

/// Represents executable statements within a function or block.
#[derive(Debug, Clone)]
pub enum Stmt {
    ExprStmt(Expr),
    Call(CallExpr),
    Block(BlockStmt),
    VarDecl(VarDeclStmt),
    Return(Loc, Option<Expr>),
    ForStmt(ForStmt),
    ForEach(ForEachStmt),
    IfStmt(IfStmt),
    Break(Loc),
    Continue(Loc),
    Defer(Loc, Box<Stmt>),
    Switch(SwitchStmt),
    Const(ConstDecl),
}

#[derive(Debug, Clone)]
pub struct SwitchStmt {
    pub loc: Loc,
    pub cond: Expr,
    pub branches: Vec<SwitchBranch>,
    pub default: Option<Box<Stmt>>,
}

#[derive(Debug, Clone)]
pub struct SwitchBranch {
    pub pattern: Expr,
    pub body: Stmt,
}

impl Stmt {
    pub fn loc(&self) -> Loc {
        match self {
            Stmt::ExprStmt(expr_stmt) => expr_stmt.loc(),
            Stmt::Call(call_expr) => call_expr.loc,
            Stmt::Block(block_stmt) => block_stmt.loc,
            Stmt::VarDecl(var_decl) => var_decl.names[0].loc,
            Stmt::Return(loc, _) => *loc,
            Stmt::ForStmt(for_stmt) => for_stmt.loc(),
            Stmt::ForEach(for_each_stmt) => for_each_stmt.loc(),
            Stmt::IfStmt(if_stmt) => if_stmt.loc(),
            Stmt::Break(loc) => *loc,
            Stmt::Continue(loc) => *loc,
            Stmt::Defer(loc, _) => *loc,
            Stmt::Switch(switch_stmt) => switch_stmt.loc,
            Stmt::Const(const_decl) => const_decl.loc,
        }
    }
}

/// Represents an import declaration `import path/to/module`.
#[derive(Debug, Clone)]
pub struct Import {
    pub mode: ImportMode,
    pub path: Token,
}

#[derive(Debug, Clone)]
pub enum ImportMode {
    Default,
    All,
    Alias(TokenSource),
}

impl ImportMode {
    /// Returns `true` if the import mode is [`All`].
    ///
    /// [`All`]: ImportMode::All
    #[must_use]
    pub fn is_all(&self) -> bool {
        matches!(self, Self::All)
    }
}

#[derive(Debug, Clone)]
pub struct Enum {
    pub name: Token,
    pub directives: Vec<EnumDirective>,
    pub inner_type: Option<Type>,
    pub variants: Vec<EnumVariant>,
}

#[derive(Debug, Clone)]
pub enum EnumVariant {
    Name(Token),               // FOO
    DefaultValue(Token, Expr), // BAR = 1,
}

impl EnumVariant {
    pub fn token(&self) -> &Token {
        match self {
            EnumVariant::Name(tok) => tok,
            EnumVariant::DefaultValue(tok, _) => tok,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Struct {
    pub name: Token,
    pub directives: Vec<StructDirective>,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone)]
pub struct FunctionPointerParameter {
    pub name: Token,
    pub typ: Type,
}

#[derive(Debug, Clone)]
pub struct FunctionParameter {
    pub name: Token,
    pub typ: Type,
    // pub value: Option<Expr>,
    pub is_variadic: bool,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: Token,
    pub typ: Type,
    // pub value: Option<Expr>
}

#[derive(Debug, Clone)]
pub struct FunctionSignature {
    pub name: Token,
    pub parameters: Vec<FunctionParameter>,
    pub return_type: Option<Type>,
    pub va_args: bool,
}

#[derive(Debug, Clone)]
pub struct OperatorOverload {
    pub loc: Loc,
    pub op: Op,
    pub parameters: Vec<FunctionParameter>,
    pub return_type: Type,
    pub body: BlockStmt,
    pub resolved_name: Option<TokenSource>,
}

#[derive(Debug, Clone)]
pub struct MethodImplementation {
    pub loc: Loc,
    pub self_parameter: FunctionPointerParameter,
    pub function: Function,
}

/// Represents a function declaration, including its signature and body.
#[derive(Debug, Clone)]
pub struct Function {
    pub signature: FunctionSignature,
    pub directives: Vec<FunctionDirective>,
    pub body: Option<BlockStmt>,
    pub resolved_name: Option<String>,
}

#[derive(Debug, Clone)]
pub enum FunctionDirective {
    Foreign(Loc, Token),
    LinkName(Token),
    Private,
    OwnedReturn,
    Export,
}

impl FunctionDirective {
    pub fn as_link_name(&self) -> Option<&Token> {
        if let Self::LinkName(v) = self {
            Some(v)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub enum StructDirective {
    Test(Loc),
}

#[derive(Debug, Clone)]
pub enum EnumDirective {
    Test(Loc),
}

#[derive(Debug, Clone)]
pub struct BlockStmt {
    pub loc: Loc,
    pub stmts: Vec<Stmt>,
}
impl BlockStmt {
    pub fn new(loc: Loc) -> Self {
        Self {
            loc,
            stmts: Vec::new(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.stmts.is_empty()
    }
}

#[derive(Debug, Clone)]
pub enum IfStmt {
    If {
        cond: Expr,
        true_body: BlockStmt,
    },
    IfElse {
        cond: Expr,
        true_body: BlockStmt,
        false_body: BlockStmt,
    },
}

impl IfStmt {
    pub fn loc(&self) -> Loc {
        match self {
            IfStmt::If { cond, .. } => cond.loc(),
            IfStmt::IfElse { cond, .. } => cond.loc(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ForStmt {
    ForLoop(BlockStmt),
    ForCond { cond: Expr, body: BlockStmt },
}

impl ForStmt {
    pub fn loc(&self) -> Loc {
        match self {
            ForStmt::ForLoop(block_stmt) => block_stmt.loc,
            ForStmt::ForCond { cond, .. } => cond.loc(),
        }
    }
}

#[allow(unused)]
#[derive(Debug, Clone)]
pub struct ForEachStmt {
    pub var_name: Token,
    pub iter_expr: Expr,
    pub body: BlockStmt,
}

impl ForEachStmt {
    pub fn loc(&self) -> Loc {
        self.var_name.loc
    }
}

#[derive(Debug, Clone)]
pub struct VarDeclStmt {
    pub names: Vec<Token>,
    pub var_type: Option<Type>,
    pub expr: Expr,
    pub is_thread_local: bool,
    pub is_private: bool,
    pub is_export: bool,
    pub is_foreign: bool,
    pub foreign_lib: Option<(Loc, Token)>,
    pub link_name: Option<Token>,
}

#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub resolved_type: Option<t::TypeID>,
}

impl Expr {
    pub fn new(kind: ExprKind) -> Self {
        Self {
            kind,
            resolved_type: None,
        }
    }

    pub fn loc(&self) -> Loc {
        self.kind.loc()
    }

    pub fn name(&self) -> &str {
        self.kind.name()
    }
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    InitList(InitListExpr),
    Identifier(Token),
    StringLiteral(Token),
    Char(CharLiteral),
    Integer(IntegerLiteral),
    Bool(BoolLiteral),
    Float(FloatLiteral),
    Call(CallExpr),
    Index(IndexExpr),
    Member(MemberExpr),
    Binary(BinaryExpr),
    Unary(UnaryExpr),
    Assign(AssignExpr),
    Null(Loc),
    TypeInfo(Box<Type>, Loc),
    Cast(Type, Box<Expr>, Loc),
    AutoCast(Box<Expr>, Loc),
    AnyCast(AnyCastExpr, Loc),
    Tuple(Vec<Expr>, Loc),
    Default(Loc),
    SizeOf(Type, Loc),
    AlignOf(Type, Loc),
    Feature(Loc, Token),
    New(Type, Loc),
    Make(Type, Vec<Expr>, Loc),
    Drop(Box<Expr>, Loc),
}

impl ExprKind {
    pub fn loc(&self) -> Loc {
        match self {
            ExprKind::Null(loc) => *loc,
            ExprKind::InitList(init_list) => init_list.loc,
            ExprKind::Identifier(identifier) => identifier.loc,
            ExprKind::StringLiteral(string_literal) => string_literal.loc,
            ExprKind::Char(char_literal) => char_literal.loc,
            ExprKind::Integer(integer_literal) => integer_literal.loc,
            ExprKind::Bool(bool_literal) => bool_literal.loc,
            ExprKind::Float(float_literal) => float_literal.loc,
            ExprKind::Call(call_expr) => call_expr.loc,
            ExprKind::Index(index_expr) => index_expr.loc,
            ExprKind::Member(member_expr) => member_expr.property.loc,
            ExprKind::Binary(binary_expr) => binary_expr.right.loc(),
            ExprKind::Unary(unary_expr) => unary_expr.right.loc(),
            ExprKind::Assign(assign_expr) => assign_expr.right.loc(),
            ExprKind::TypeInfo(_, loc) => *loc,
            ExprKind::Cast(_, _, loc) => *loc,
            ExprKind::AutoCast(_, loc) => *loc,
            ExprKind::Tuple(_, loc) => *loc,
            ExprKind::AnyCast(_, loc) => *loc,
            ExprKind::Default(loc) => *loc,
            ExprKind::SizeOf(_, loc) => *loc,
            ExprKind::AlignOf(_, loc) => *loc,
            ExprKind::Feature(loc, ..) => *loc,
            ExprKind::New(_, loc) => *loc,
            ExprKind::Make(_, _, loc) => *loc,
            ExprKind::Drop(_, loc) => *loc,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            ExprKind::Identifier(..) => "identifier",
            ExprKind::InitList(_) => "initializer list",
            ExprKind::StringLiteral(_) => "string literal",
            ExprKind::Char(..) => "character literal",
            ExprKind::Integer(..) => "integer literal",
            ExprKind::Bool(..) => "bool literal",
            ExprKind::Float(..) => "float literal",
            ExprKind::Call(_) => "call",
            ExprKind::Index(_) => "index",
            ExprKind::Member(_) => "member",
            ExprKind::Binary(_) => "binary",
            ExprKind::Unary(_) => "unary",
            ExprKind::Assign(_) => "assign",
            ExprKind::Null(_) => "null",
            ExprKind::TypeInfo(..) => "type_info",
            ExprKind::Cast(..) => "cast",
            ExprKind::AutoCast(..) => "autocast",
            ExprKind::Tuple(..) => "tuple",
            ExprKind::AnyCast(..) => "anycast",
            ExprKind::Default(..) => "default",
            ExprKind::SizeOf(..) => "sizeof",
            ExprKind::AlignOf(..) => "alignof",
            ExprKind::Feature(..) => "feature",
            ExprKind::New(..) => "new",
            ExprKind::Make(..) => "make",
            ExprKind::Drop(..) => "drop",
        }
    }
}

#[derive(Debug, Clone)]
pub enum AnyCastExpr {
    Scalar(Box<Expr>),
    Array(Vec<Expr>),
}

#[derive(Debug, Clone)]
pub struct BoolLiteral {
    pub loc: Loc,
    pub value: bool,
}

#[derive(Debug, Clone)]
pub struct CharLiteral {
    pub loc: Loc,
    pub value: u32,
}

#[derive(Debug, Clone)]
pub struct IntegerLiteral {
    pub loc: Loc,
    pub value: u64,
}

#[derive(Debug, Clone)]
pub struct FloatLiteral {
    pub loc: Loc,
    pub value: f64,
}

#[derive(Debug, Clone)]
pub struct InitListExpr {
    pub loc: Loc,
    pub type_hint: Option<Type>,
    pub elements: Vec<InitElement>,
}

#[derive(Debug, Clone)]
pub enum InitElement {
    Named { name: Token, value: Expr },
    Positional(Expr),
}

impl InitElement {
    pub fn expr(&self) -> &Expr {
        match self {
            InitElement::Named { value, .. } => value,
            InitElement::Positional(expr) => expr,
        }
    }

    pub fn expr_mut(&mut self) -> &mut Expr {
        match self {
            InitElement::Named { value, .. } => value,
            InitElement::Positional(expr) => expr,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CallExpr {
    pub loc: Loc,
    pub callee: Box<Expr>,
    pub arguments: Vec<Argument>,
    pub module_name: Option<TokenSource>,
    pub resolved_name: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Argument {
    Named { name: Token, value: Expr },
    Positional(Expr),
}

impl Argument {
    pub fn expr_mut(&mut self) -> &mut Expr {
        match self {
            Argument::Named { value, .. } => value,
            Argument::Positional(expr) => expr,
        }
    }
    pub fn expr(&self) -> &Expr {
        match self {
            Argument::Named { value, .. } => value,
            Argument::Positional(expr) => expr,
        }
    }

    /// Returns `true` if the argument is [`Positional`].
    ///
    /// [`Positional`]: Argument::Positional
    #[must_use]
    pub fn is_positional(&self) -> bool {
        matches!(self, Self::Positional(..))
    }
}

#[derive(Debug, Clone)]
pub struct IndexExpr {
    pub loc: Loc,
    pub array: Box<Expr>,
    pub index: Box<Expr>,
}

// object.property
#[derive(Debug, Clone)]
pub struct MemberExpr {
    pub object: Box<Expr>,
    pub property: Token,
}

#[derive(Debug, Clone)]
pub struct AssignExpr {
    pub left: Box<Expr>,
    pub assign_kind: AssignKind,
    pub right: Box<Expr>,
}

#[derive(Debug, Clone)]
pub struct BinaryExpr {
    pub left: Box<Expr>,
    pub op: Op,
    pub right: Box<Expr>,
    pub use_operator_overload: Option<TokenSource>,
}

#[derive(Debug, Clone)]
pub struct UnaryExpr {
    pub op: Op,
    pub right: Box<Expr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Lt,
    LtEq,
    Gt,
    GtEq,
    Eq,
    Neg,
    Not,
    NotEq,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Refer,
    Deref,
}

impl BinaryExpr {
    pub fn op_loc(&self) -> Loc {
        self.right.loc()
    }
}

impl Op {
    pub fn get_name(&self) -> &'static str {
        match self {
            Self::Add => "operator_add",
            Self::Sub => "operator_sub",
            Self::Mul => "operator_mul",
            Self::Div => "operator_div",
            Self::Mod => "operator_mod",
            Self::Lt => "operator_lt",
            Self::LtEq => "operator_lteq",
            Self::Gt => "operator_gt",
            Self::GtEq => "operator_gteq",
            Self::Eq => "operator_eq",
            Self::Neg => "operator_neg",
            Self::Not => "operator_not",
            Self::NotEq => "operator_noteq",
            Self::And => "operator_and",
            Self::Or => "operator_or",
            Self::BitAnd => "operator_bitand",
            Self::BitOr => "operator_bitor",
            Self::BitXor => "operator_bitxor",
            Self::Refer => "operator_refer",
            Self::Deref => "operator_deref",
        }
    }

    pub fn is_binary(&self) -> bool {
        match self {
            Self::Add
            | Self::Sub
            | Self::Mul
            | Self::Div
            | Self::Mod
            | Self::Lt
            | Self::LtEq
            | Self::Gt
            | Self::GtEq
            | Self::Eq
            | Self::NotEq
            | Self::And
            | Self::BitAnd
            | Self::BitOr
            | Self::BitXor => true,

            Self::Or | Self::Neg | Self::Not | Self::Refer | Self::Deref => false,
        }
    }

    pub fn is_unary(&self) -> bool {
        match self {
            Self::Add
            | Self::Sub
            | Self::Mul
            | Self::Div
            | Self::Mod
            | Self::Lt
            | Self::LtEq
            | Self::Gt
            | Self::GtEq
            | Self::Eq
            | Self::NotEq
            | Self::And
            | Self::BitAnd
            | Self::BitOr
            | Self::BitXor => false,

            Self::Or | Self::Neg | Self::Not | Self::Refer | Self::Deref => true,
        }
    }

    pub fn is_refer(&self) -> bool {
        matches!(self, Self::Refer)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignKind {
    Default,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

#[derive(PartialEq, PartialOrd)]
pub enum Precedence {
    Lowest,
    Assignment,

    LogicalOr,
    LogicalAnd,
    Equality,

    Comparison,
    BitwiseOr,
    BitwiseXor,
    BitwiseAnd,
    // BitShift,

    // BitWiseNotLogicalNot,
    AddSub,
    MulDivMod,
    Neg,

    Call,
    Member,
    Index,
}

pub fn token_to_precedence(token: &Token) -> Precedence {
    match token.kind {
        TokenKind::Dot => Precedence::Member,
        TokenKind::DoubleColon => Precedence::Member,
        TokenKind::OpenParen => Precedence::Call,
        TokenKind::OpenBracket => Precedence::Index,
        TokenKind::Plus | TokenKind::Minus => Precedence::AddSub,
        TokenKind::Asterisk | TokenKind::Slash | TokenKind::Mod => Precedence::MulDivMod,
        TokenKind::DoubleAmpersand => Precedence::LogicalAnd,
        TokenKind::DoublePipe => Precedence::LogicalOr,
        TokenKind::Ampersand => Precedence::BitwiseAnd,
        TokenKind::Pipe => Precedence::BitwiseOr,
        TokenKind::Caret => Precedence::BitwiseXor,
        t if t.is_assign_kind() => Precedence::Assignment,
        TokenKind::Lt | TokenKind::LtEq | TokenKind::Gt | TokenKind::GtEq => Precedence::Comparison,
        TokenKind::EqEq | TokenKind::NotEq => Precedence::Equality,
        _ => Precedence::Lowest,
    }
}

pub fn map_type(ast_ty: &Type, db: &mut t::TypeDatabase, current_module: &str) -> t::TypeID {
    map_type_ext(ast_ty, db, current_module, &|_| None)
}

pub fn map_type_ext<F>(
    ast_ty: &Type,
    db: &mut t::TypeDatabase,
    current_module: &str,
    lookup_const: &F,
) -> t::TypeID
where
    F: Fn(&str) -> Option<usize>,
{
    fn map_type_name(token: &Token, db: &mut t::TypeDatabase, current_module: &str) -> t::TypeID {
        let name = &token.source;

        // Check if namespaced (contains '.')
        if name.as_str().contains('.') {
            let parts: Vec<&str> = name.as_str().split('.').collect();
            if parts.len() == 2 {
                let alias = parts[0];
                let type_name = parts[1];

                let actual_module = if let Some(module) = db.try_modules_get(current_module) {
                    if let Some(info) = module.imported_modules.get(&TokenSource::from(alias)) {
                        info.original_name().map(|n| n.as_str()).unwrap_or(alias)
                    } else {
                        alias
                    }
                } else {
                    alias
                };

                if let Some(module) = db.try_modules_get(actual_module) {
                    if let Some(id) = module.lookup_type(&TokenSource::from(type_name)) {
                        return id;
                    }
                }
            }
        }

        if let Some(id) = db.lookup_type_in_module(current_module, name) {
            id
        } else {
            let id = db.insert_type(t::Type::Struct {
                name: name.clone(),
                fields: None,
                functions: std::collections::HashMap::new(),
                constructor: None,
                destructor: None,
            });
            db.modules_add(current_module)
                .register_type(name.clone(), id);
            id
        }
    }
    match ast_ty {
        Type::Tuple(elements, _) => {
            let mut mapped_elems = Vec::new();
            for elem in elements {
                mapped_elems.push(map_type_ext(elem, db, current_module, lookup_const));
            }
            db.tuple(mapped_elems)
        }
        Type::Pointer(count, inner_ast) => {
            let mut inner = map_type_ext(inner_ast, db, current_module, lookup_const);
            for _ in 0..*count {
                inner = db.pointer(inner);
            }
            inner
        }
        Type::Scalar(token) => map_type_name(token, db, current_module),
        Type::Array(inner_ast, size) => {
            let inner_ty = map_type_ext(inner_ast, db, current_module, lookup_const);
            let size_val = match size {
                ArraySize::Literal(sz) => *sz,
                ArraySize::Ident(ident) => lookup_const(&ident.source).unwrap_or_default(),
            };
            db.array(inner_ty, size_val)
        }
        Type::Slice(inner_ast) => {
            let inner_ty = map_type_ext(inner_ast, db, current_module, lookup_const);
            db.slice(inner_ty)
        }
        Type::DynArray(inner_ast) => {
            let inner_ty = map_type_ext(inner_ast, db, current_module, lookup_const);
            db.dyn_array(inner_ty)
        }
        Type::FnPointer {
            parameters,
            return_type,
            ..
        } => {
            let mut param_ids = Vec::new();
            for param in parameters {
                param_ids.push(map_type_ext(&param.typ, db, current_module, lookup_const));
            }
            let ret_id = match return_type {
                Some(ret) => map_type_ext(ret, db, current_module, lookup_const),
                None => db.void(),
            };
            db.fn_pointer(param_ids, ret_id)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    pub package: Vec<String>,
    pub imports: Vec<Vec<String>>,
    pub structs: Vec<StructDef>,
    pub enums: Vec<EnumDef>,
    pub impls: Vec<ImplBlock>,
    pub consts: Vec<ConstDef>,
    pub functions: Vec<Function>,
    pub script_body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructDef {
    pub public: bool,
    pub package: Vec<String>,
    pub name: String,
    pub type_params: Vec<String>,
    pub fields: Vec<Field>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub public: bool,
    pub name: String,
    pub type_ref: TypeRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumDef {
    pub public: bool,
    pub package: Vec<String>,
    pub name: String,
    pub type_params: Vec<String>,
    pub variants: Vec<EnumVariant>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumVariant {
    pub name: String,
    pub payload: Option<TypeRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImplBlock {
    pub type_name: TypeRef,
    pub methods: Vec<Function>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstDef {
    pub public: bool,
    pub name: String,
    pub type_ref: TypeRef,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub public: bool,
    pub is_test: bool,
    pub package: Vec<String>,
    pub name: String,
    pub type_params: Vec<String>,
    pub params: Vec<Param>,
    pub return_type: TypeRef,
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    pub name: String,
    pub mutable: bool,
    pub type_ref: TypeRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeRef {
    pub path: Vec<String>,
    pub args: Vec<TypeRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    Let {
        name: String,
        mutable: bool,
        type_annotation: Option<TypeRef>,
        value: Expr,
        span: Span,
    },
    LetElse {
        pattern: Vec<String>,
        binding: String,
        value: Expr,
        else_body: Vec<Stmt>,
        span: Span,
    },
    IfLet {
        pattern: Vec<String>,
        binding: Option<String>,
        value: Expr,
        body: Vec<Stmt>,
        else_body: Option<Vec<Stmt>>,
        span: Span,
    },
    Assign {
        target: Vec<String>,
        op: AssignOp,
        value: Expr,
        span: Span,
    },
    Postfix {
        target: Vec<String>,
        op: PostfixOp,
        span: Span,
    },
    Return {
        value: Option<Expr>,
        span: Span,
    },
    Match {
        value: Expr,
        arms: Vec<MatchStmtArm>,
        span: Span,
    },
    Expr {
        expr: Expr,
        span: Span,
    },
    For {
        variant: ForVariant,
        span: Span,
    },
    Break {
        span: Span,
    },
    Continue {
        span: Span,
    },
    Defer {
        stmt: Box<Stmt>,
        span: Span,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    Assign,
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    ShiftLeft,
    ShiftRight,
    BitAnd,
    BitXor,
    BitOr,
    BitAndNot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostfixOp {
    Increment,
    Decrement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForVariant {
    Infinite {
        body: Vec<Stmt>,
    },
    While {
        condition: Expr,
        body: Vec<Stmt>,
    },
    Iterate {
        binding: String,
        iterable: Expr,
        body: Vec<Stmt>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Call {
        callee: Vec<String>,
        type_args: Vec<TypeRef>,
        args: Vec<Expr>,
    },
    StructLiteral {
        type_name: Vec<String>,
        fields: Vec<(String, Expr)>,
    },
    Match {
        value: Box<Expr>,
        arms: Vec<MatchArm>,
    },
    If {
        condition: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Box<Expr>,
    },
    Panic {
        message: Box<Expr>,
    },
    Question {
        expr: Box<Expr>,
    },
    MutArg {
        name: Vec<String>,
    },
    Cast {
        expr: Box<Expr>,
        target: TypeRef,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
    },
    Name(Vec<String>),
    String(String),
    Int(i64),
    Float(String),
    Char(char),
    Bool(bool),
    Void,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchArm {
    pub pattern: Vec<String>,
    pub binding: Option<String>,
    pub value: Expr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchStmtArm {
    pub pattern: Vec<String>,
    pub binding: Option<String>,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinaryOp {
    LogicalOr,
    LogicalAnd,
    Add,
    Subtract,
    BitOr,
    BitXor,
    Multiply,
    Divide,
    Remainder,
    ShiftLeft,
    ShiftRight,
    BitAnd,
    BitAndNot,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub line: usize,
    pub column: usize,
    pub length: usize,
    pub text: String,
}

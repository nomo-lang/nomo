#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    pub package: Vec<String>,
    pub imports: Vec<Vec<String>>,
    pub structs: Vec<StructDef>,
    pub enums: Vec<EnumDef>,
    pub impls: Vec<ImplBlock>,
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructDef {
    pub public: bool,
    pub name: String,
    pub fields: Vec<Field>,
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
    pub name: String,
    pub type_params: Vec<String>,
    pub variants: Vec<EnumVariant>,
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
pub struct Function {
    pub public: bool,
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: TypeRef,
    pub body: Vec<Stmt>,
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
    Assign {
        name: String,
        value: Expr,
        span: Span,
    },
    Return {
        value: Option<Expr>,
        span: Span,
    },
    Expr {
        expr: Expr,
        span: Span,
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
    Try {
        expr: Box<Expr>,
    },
    Cast {
        expr: Box<Expr>,
        target: TypeRef,
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
pub struct MatchArm {
    pub pattern: Vec<String>,
    pub binding: Option<String>,
    pub value: Expr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
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

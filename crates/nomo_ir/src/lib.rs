#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub package: String,
    pub imports: Vec<String>,
    pub extern_functions: Vec<ExternFunction>,
    pub structs: Vec<StructType>,
    pub enums: Vec<EnumType>,
    pub consts: Vec<Const>,
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternFunction {
    pub symbol: String,
    pub params: Vec<ValueType>,
    pub return_type: ValueType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Const {
    pub name: String,
    pub value_type: ValueType,
    pub initializer: ValueExpr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructType {
    pub package: String,
    pub name: String,
    pub type_params: Vec<String>,
    pub fields: Vec<StructField>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructField {
    pub name: String,
    pub value_type: ValueType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumType {
    pub package: String,
    pub name: String,
    pub type_params: Vec<String>,
    pub variants: Vec<EnumVariantType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumVariantType {
    pub name: String,
    pub payload: Option<ValueType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub package: String,
    pub name: String,
    pub is_suspend: bool,
    pub params: Vec<Parameter>,
    pub return_type: ValueType,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    pub name: String,
    pub mutable: bool,
    pub value_type: ValueType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    Let {
        name: String,
        value_type: ValueType,
        initializer: ValueExpr,
    },
    LetIf {
        name: String,
        value_type: ValueType,
        condition: ValueExpr,
        body: Vec<Statement>,
        else_body: Vec<Statement>,
    },
    LetMatch {
        name: String,
        value_type: ValueType,
        value: ValueExpr,
        enum_name: String,
        enum_args: Vec<ValueType>,
        arms: Vec<MatchStatementArm>,
    },
    QuestionLet {
        carrier: QuestionCarrier,
        name: String,
        value_type: ValueType,
        result_type: ValueType,
        return_type: ValueType,
        result_expr: ValueExpr,
        early_exit_actions: Vec<ValueExpr>,
    },
    QuestionReturn {
        carrier: QuestionCarrier,
        ok_type: ValueType,
        result_type: ValueType,
        return_type: ValueType,
        result_expr: ValueExpr,
        early_exit_actions: Vec<ValueExpr>,
    },
    LetElse {
        binding: String,
        value_type: ValueType,
        value: ValueExpr,
        enum_name: String,
        enum_args: Vec<ValueType>,
        variant: String,
        else_body: Vec<Statement>,
    },
    IfLet {
        binding: Option<String>,
        value_type: Option<ValueType>,
        value: ValueExpr,
        enum_name: String,
        enum_args: Vec<ValueType>,
        variant: String,
        body: Vec<Statement>,
        else_body: Option<Vec<Statement>>,
    },
    If {
        condition: ValueExpr,
        body: Vec<Statement>,
        else_body: Vec<Statement>,
    },
    Assign {
        name: String,
        value: ValueExpr,
    },
    AssignField {
        base: String,
        field: String,
        value_type: ValueType,
        value: ValueExpr,
    },
    ArrayIndexAssign {
        root: String,
        indices: Vec<ValueExpr>,
        array_types: Vec<ValueType>,
        value: ValueExpr,
        mutation_mode: ArrayMutationMode,
    },
    Eprintln(ValueExpr),
    Eprint(ValueExpr),
    Println(ValueExpr),
    Print(ValueExpr),
    Panic(ValueExpr),
    Return(Option<ValueExpr>),
    Expr(ValueExpr),
    Match {
        value: ValueExpr,
        enum_name: String,
        enum_args: Vec<ValueType>,
        arms: Vec<MatchStatementArm>,
    },
    TaskSelect {
        arms: Vec<TaskSelectArm>,
    },
    Loop {
        kind: LoopKind,
        body: Vec<Statement>,
    },
    Break,
    Continue,
    Defer {
        call: DeferredCall,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestionCarrier {
    Result,
    Option,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchStatementArm {
    pub variant: String,
    pub binding: Option<String>,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSelectArm {
    pub operation: TaskSelectOperation,
    pub binding: String,
    pub binding_type: ValueType,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskSelectOperation {
    Receive {
        channel: ValueExpr,
        element_type: ValueType,
    },
    Send {
        channel: ValueExpr,
        value: Box<ValueExpr>,
        element_type: ValueType,
    },
    Sleep {
        duration: ValueExpr,
    },
    Join {
        handle: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopKind {
    Infinite,
    While(ValueExpr),
    CStyle {
        binding: String,
        value_type: ValueType,
        initializer: Box<ValueExpr>,
        condition: Box<ValueExpr>,
        update: Box<ValueExpr>,
    },
    Iterate {
        binding: String,
        element_type: ValueType,
        iterable: ValueExpr,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrayMutationMode {
    CheckedCow,
    CheckedUnique,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeferredCall {
    Expr(ValueExpr),
    Eprintln(ValueExpr),
    Eprint(ValueExpr),
    Println(ValueExpr),
    Print(ValueExpr),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathUnaryFunction {
    Abs,
    Floor,
    Ceil,
    Round,
    Sqrt,
    Sin,
    Cos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathBinaryFunction {
    Min,
    Max,
    Pow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumBinaryFunction {
    Checked,
    Wrapping,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonOperation {
    Kind,
    IsNull,
    AsBool,
    NumberText,
    AsString,
    ArrayItems,
    ObjectMembers,
    Get,
    FromNull,
    FromBool,
    FromNumberText,
    FromI64,
    FromU64,
    FromString,
    FromArray,
    FromObject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonRpcOperation {
    Decoder,
    Feed,
    Finish,
    Parse,
    Encode,
    Value,
    Kind,
    Request,
    Notification,
    Success,
    Failure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CronOperation {
    Parse,
    Matches,
    NextAfter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueType {
    String,
    CString,
    Opaque,
    OpaqueHandle(String),
    OwnedHandle(String),
    BorrowedHandle(String),
    Nullable(Box<ValueType>),
    ExternCallback {
        params: Vec<ValueType>,
        return_type: Box<ValueType>,
    },
    TaskCallback {
        params: Vec<ValueType>,
        return_type: Box<ValueType>,
    },
    Int,
    I32,
    U32,
    U64,
    Float,
    Char,
    Bool,
    Array(Box<ValueType>),
    Struct(String, Vec<ValueType>),
    Enum(String, Vec<ValueType>),
    TypeParam(String),
    Void,
    Never,
}

impl ValueType {
    pub fn name(&self) -> &str {
        match self {
            ValueType::String => "string",
            ValueType::CString => "CString",
            ValueType::Opaque => "Opaque",
            ValueType::OpaqueHandle(name) => name,
            ValueType::OwnedHandle(name) => name,
            ValueType::BorrowedHandle(name) => name,
            ValueType::Nullable(_) => "Nullable",
            ValueType::ExternCallback { .. } => "extern C callback",
            ValueType::TaskCallback { .. } => "task worker",
            ValueType::Int => "i64",
            ValueType::I32 => "i32",
            ValueType::U32 => "u32",
            ValueType::U64 => "u64",
            ValueType::Float => "f64",
            ValueType::Char => "char",
            ValueType::Bool => "bool",
            ValueType::Array(_) => "Array",
            ValueType::Struct(name, args) => {
                if args.is_empty() {
                    name
                } else {
                    "struct"
                }
            }
            ValueType::Enum(name, _) => name,
            ValueType::TypeParam(name) => name,
            ValueType::Void => "void",
            ValueType::Never => "never",
        }
    }

    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            ValueType::Int | ValueType::I32 | ValueType::U32 | ValueType::U64
        )
    }

    pub fn is_numeric(&self) -> bool {
        self.is_integer() || matches!(self, ValueType::Float)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueExpr {
    StringLiteral(String),
    IntLiteral(i64),
    FloatLiteral(String),
    CharLiteral(char),
    BoolLiteral(bool),
    VoidLiteral,
    Variable(String),
    FunctionRef(String),
    Binary {
        left: Box<ValueExpr>,
        op: BinaryOp,
        right: Box<ValueExpr>,
        value_type: ValueType,
    },
    Unary {
        op: UnaryOp,
        expr: Box<ValueExpr>,
    },
    StringCompare {
        left: Box<ValueExpr>,
        op: BinaryOp,
        right: Box<ValueExpr>,
    },
    Cast {
        expr: Box<ValueExpr>,
        target_type: ValueType,
    },
    Call {
        name: String,
        args: Vec<ValueExpr>,
    },
    MutBorrow(Vec<String>),
    StringLen {
        value: Box<ValueExpr>,
    },
    StringConcat {
        left: Box<ValueExpr>,
        right: Box<ValueExpr>,
    },
    StringIsEmpty {
        value: Box<ValueExpr>,
    },
    StringContains {
        value: Box<ValueExpr>,
        needle: Box<ValueExpr>,
    },
    StringStartsWith {
        value: Box<ValueExpr>,
        prefix: Box<ValueExpr>,
    },
    StringEndsWith {
        value: Box<ValueExpr>,
        suffix: Box<ValueExpr>,
    },
    StringSplit {
        value: Box<ValueExpr>,
        separator: Box<ValueExpr>,
    },
    StringTrim {
        value: Box<ValueExpr>,
    },
    StringToLower {
        value: Box<ValueExpr>,
    },
    StringToUpper {
        value: Box<ValueExpr>,
    },
    CharIsDigit {
        value: Box<ValueExpr>,
    },
    CharIsAlpha {
        value: Box<ValueExpr>,
    },
    CharIsWhitespace {
        value: Box<ValueExpr>,
    },
    CharToString {
        value: Box<ValueExpr>,
    },
    OsPlatform,
    OsArch,
    OsPathSeparator,
    OsLineEnding,
    TimeNowMillis,
    TimeMonotonicMillis,
    TimeDurationMillis {
        millis: Box<ValueExpr>,
    },
    TimeDurationSeconds {
        seconds: Box<ValueExpr>,
    },
    TimeDurationAsMillis {
        duration: Box<ValueExpr>,
    },
    TimeFormatDuration {
        duration: Box<ValueExpr>,
    },
    TimeSleep {
        duration: Box<ValueExpr>,
    },
    TimeSleepMillis {
        duration: Box<ValueExpr>,
    },
    LogEnabled {
        level: Box<ValueExpr>,
    },
    HashNew,
    HashString {
        value: Box<ValueExpr>,
    },
    HashBytes {
        value: Box<ValueExpr>,
    },
    HashWriteString {
        state: Box<ValueExpr>,
        value: Box<ValueExpr>,
    },
    HashWriteBytes {
        state: Box<ValueExpr>,
        value: Box<ValueExpr>,
    },
    HashFinish {
        state: Box<ValueExpr>,
    },
    CryptoSha256 {
        value: Box<ValueExpr>,
    },
    CryptoSha512 {
        value: Box<ValueExpr>,
    },
    CryptoRandomBytes {
        count: Box<ValueExpr>,
    },
    JsonParse {
        value: Box<ValueExpr>,
    },
    JsonStringify {
        value: Box<ValueExpr>,
    },
    JsonStructured {
        operation: JsonOperation,
        args: Vec<ValueExpr>,
    },
    JsonRpc {
        operation: JsonRpcOperation,
        args: Vec<ValueExpr>,
    },
    Cron {
        operation: CronOperation,
        args: Vec<ValueExpr>,
    },
    RegexCompile {
        pattern: Box<ValueExpr>,
    },
    RegexIsMatch {
        regex: Box<ValueExpr>,
        value: Box<ValueExpr>,
    },
    RegexCaptures {
        regex: Box<ValueExpr>,
        value: Box<ValueExpr>,
    },
    CollectionsStringMapNew,
    CollectionsStringMapLen {
        map: Box<ValueExpr>,
    },
    CollectionsStringMapGet {
        map: Box<ValueExpr>,
        key: Box<ValueExpr>,
    },
    CollectionsStringMapContains {
        map: Box<ValueExpr>,
        key: Box<ValueExpr>,
    },
    CollectionsStringMapSet {
        map: Box<ValueExpr>,
        key: Box<ValueExpr>,
        value: Box<ValueExpr>,
    },
    CollectionsStringMapRemove {
        map: Box<ValueExpr>,
        key: Box<ValueExpr>,
    },
    CollectionsStringSetNew,
    CollectionsStringSetLen {
        set: Box<ValueExpr>,
    },
    CollectionsStringSetContains {
        set: Box<ValueExpr>,
        value: Box<ValueExpr>,
    },
    CollectionsStringSetInsert {
        set: Box<ValueExpr>,
        value: Box<ValueExpr>,
    },
    CollectionsStringSetRemove {
        set: Box<ValueExpr>,
        value: Box<ValueExpr>,
    },
    ProcessExit {
        code: Box<ValueExpr>,
    },
    ProcessSpawn {
        command: Box<ValueExpr>,
    },
    ProcessStatus {
        command: Box<ValueExpr>,
    },
    ProcessExec {
        command: Box<ValueExpr>,
    },
    ProcessOutput {
        command: Box<ValueExpr>,
    },
    NumParseI64 {
        value: Box<ValueExpr>,
    },
    NumParseU64 {
        value: Box<ValueExpr>,
    },
    NumParseF64 {
        value: Box<ValueExpr>,
    },
    NumToString {
        value: Box<ValueExpr>,
        value_type: ValueType,
    },
    NumBinary {
        function: NumBinaryFunction,
        op: BinaryOp,
        left: Box<ValueExpr>,
        right: Box<ValueExpr>,
        value_type: ValueType,
    },
    PathJoin {
        left: Box<ValueExpr>,
        right: Box<ValueExpr>,
    },
    PathBasename {
        path: Box<ValueExpr>,
    },
    PathDirname {
        path: Box<ValueExpr>,
    },
    PathExtension {
        path: Box<ValueExpr>,
    },
    PathNormalize {
        path: Box<ValueExpr>,
    },
    PathIsAbsolute {
        path: Box<ValueExpr>,
    },
    MathUnary {
        function: MathUnaryFunction,
        value: Box<ValueExpr>,
        value_type: ValueType,
    },
    MathBinary {
        function: MathBinaryFunction,
        left: Box<ValueExpr>,
        right: Box<ValueExpr>,
        value_type: ValueType,
    },
    FsReadToString {
        path: Box<ValueExpr>,
    },
    FsWriteString {
        path: Box<ValueExpr>,
        content: Box<ValueExpr>,
    },
    FsReadBytes {
        path: Box<ValueExpr>,
    },
    FsWriteBytes {
        path: Box<ValueExpr>,
        bytes: Box<ValueExpr>,
    },
    FsExists {
        path: Box<ValueExpr>,
    },
    FsMetadata {
        path: Box<ValueExpr>,
    },
    FsCreateDir {
        path: Box<ValueExpr>,
    },
    FsRemoveDir {
        path: Box<ValueExpr>,
    },
    FsReadDir {
        path: Box<ValueExpr>,
    },
    FsOpen {
        path: Box<ValueExpr>,
    },
    IoReadLine,
    FileClose {
        file: Box<ValueExpr>,
    },
    FileReadToString {
        file: Box<ValueExpr>,
    },
    FileWriteString {
        file: Box<ValueExpr>,
        content: Box<ValueExpr>,
    },
    NetConnect {
        host: Box<ValueExpr>,
        port: Box<ValueExpr>,
    },
    NetListen {
        host: Box<ValueExpr>,
        port: Box<ValueExpr>,
    },
    NetUdpBind {
        host: Box<ValueExpr>,
        port: Box<ValueExpr>,
    },
    TcpListenerAccept {
        listener: Box<ValueExpr>,
    },
    TcpListenerClose {
        listener: Box<ValueExpr>,
    },
    TcpStreamClose {
        stream: Box<ValueExpr>,
    },
    TcpStreamReadToString {
        stream: Box<ValueExpr>,
    },
    TcpStreamWriteString {
        stream: Box<ValueExpr>,
        content: Box<ValueExpr>,
    },
    UdpSocketClose {
        socket: Box<ValueExpr>,
    },
    UdpSocketRecvFromString {
        socket: Box<ValueExpr>,
        max_bytes: Box<ValueExpr>,
    },
    UdpSocketSendToString {
        socket: Box<ValueExpr>,
        content: Box<ValueExpr>,
        host: Box<ValueExpr>,
        port: Box<ValueExpr>,
    },
    ResultMapErr {
        result: Box<ValueExpr>,
        ok_type: ValueType,
        source_err_type: ValueType,
        target_err_type: ValueType,
        converter: String,
    },
    ResultIsOk {
        result: Box<ValueExpr>,
        ok_type: ValueType,
        err_type: ValueType,
    },
    ResultIsErr {
        result: Box<ValueExpr>,
        ok_type: ValueType,
        err_type: ValueType,
    },
    ResultUnwrapOr {
        result: Box<ValueExpr>,
        default: Box<ValueExpr>,
        ok_type: ValueType,
        err_type: ValueType,
    },
    ResultMap {
        result: Box<ValueExpr>,
        source_ok_type: ValueType,
        target_ok_type: ValueType,
        err_type: ValueType,
        converter: String,
    },
    ResultAndThen {
        result: Box<ValueExpr>,
        source_ok_type: ValueType,
        target_ok_type: ValueType,
        err_type: ValueType,
        converter: String,
    },
    OptionIsSome {
        option: Box<ValueExpr>,
        payload_type: ValueType,
    },
    OptionIsNone {
        option: Box<ValueExpr>,
        payload_type: ValueType,
    },
    OptionUnwrapOr {
        option: Box<ValueExpr>,
        default: Box<ValueExpr>,
        payload_type: ValueType,
    },
    OptionMap {
        option: Box<ValueExpr>,
        source_type: ValueType,
        target_type: ValueType,
        converter: String,
    },
    OptionAndThen {
        option: Box<ValueExpr>,
        source_type: ValueType,
        target_type: ValueType,
        converter: String,
    },
    EnvGet {
        name: Box<ValueExpr>,
    },
    EnvSet {
        name: Box<ValueExpr>,
        value: Box<ValueExpr>,
    },
    EnvCwd,
    EnvHomeDir,
    EnvTempDir,
    EnvArgs,
    ArrayNew {
        element_type: ValueType,
    },
    ArrayLiteral {
        elements: Vec<ValueExpr>,
        element_type: ValueType,
    },
    ArrayIndex {
        array: Box<ValueExpr>,
        index: Box<ValueExpr>,
        element_type: ValueType,
    },
    ArrayLen {
        array: Box<ValueExpr>,
    },
    ArrayIter {
        array: Box<ValueExpr>,
        element_type: ValueType,
    },
    ArrayGet {
        array: Box<ValueExpr>,
        index: Box<ValueExpr>,
        element_type: ValueType,
    },
    ArrayPop {
        array: String,
        element_type: ValueType,
    },
    ArrayRemove {
        array: String,
        index: Box<ValueExpr>,
        element_type: ValueType,
    },
    ArrayPush {
        array: String,
        value: Box<ValueExpr>,
        element_type: ValueType,
    },
    ArraySet {
        array: String,
        index: Box<ValueExpr>,
        value: Box<ValueExpr>,
        element_type: ValueType,
        mutation_mode: ArrayMutationMode,
    },
    ArrayInsert {
        array: String,
        index: Box<ValueExpr>,
        value: Box<ValueExpr>,
        element_type: ValueType,
    },
    ArrayClear {
        array: String,
        element_type: ValueType,
    },
    StructLiteral {
        type_name: String,
        struct_args: Vec<ValueType>,
        fields: Vec<(String, ValueExpr)>,
    },
    FieldAccess {
        base: String,
        field: String,
    },
    EnumPayloadFieldAccess {
        value: Box<ValueExpr>,
        variant: String,
        field: String,
    },
    EnumVariant {
        enum_name: String,
        enum_args: Vec<ValueType>,
        variant: String,
        payload: Option<Box<ValueExpr>>,
    },
    EnumPayload {
        value: Box<ValueExpr>,
        variant: String,
    },
    Match {
        value: Box<ValueExpr>,
        arms: Vec<MatchValueArm>,
    },
    If {
        condition: Box<ValueExpr>,
        then_branch: Box<ValueExpr>,
        else_branch: Box<ValueExpr>,
    },
    Panic {
        message: Box<ValueExpr>,
        fallback_type: ValueType,
    },
}

impl ValueExpr {
    /// Returns the directly nested expressions in source evaluation order.
    ///
    /// Ownership and effect analyses use this exhaustive accessor so a newly
    /// added expression kind must make an explicit choice about its children.
    pub fn child_expressions(&self) -> Vec<&ValueExpr> {
        match self {
            Self::Binary { left, right, .. }
            | Self::StringCompare { left, right, .. }
            | Self::StringConcat { left, right }
            | Self::StringContains {
                value: left,
                needle: right,
            }
            | Self::StringStartsWith {
                value: left,
                prefix: right,
            }
            | Self::StringEndsWith {
                value: left,
                suffix: right,
            }
            | Self::StringSplit {
                value: left,
                separator: right,
            }
            | Self::PathJoin { left, right }
            | Self::NumBinary { left, right, .. }
            | Self::MathBinary { left, right, .. }
            | Self::CollectionsStringMapGet {
                map: left,
                key: right,
            }
            | Self::CollectionsStringMapContains {
                map: left,
                key: right,
            }
            | Self::CollectionsStringMapRemove {
                map: left,
                key: right,
            }
            | Self::CollectionsStringSetContains {
                set: left,
                value: right,
            }
            | Self::CollectionsStringSetInsert {
                set: left,
                value: right,
            }
            | Self::CollectionsStringSetRemove {
                set: left,
                value: right,
            }
            | Self::RegexIsMatch {
                regex: left,
                value: right,
            }
            | Self::RegexCaptures {
                regex: left,
                value: right,
            }
            | Self::NetConnect {
                host: left,
                port: right,
            }
            | Self::NetListen {
                host: left,
                port: right,
            }
            | Self::NetUdpBind {
                host: left,
                port: right,
            }
            | Self::UdpSocketRecvFromString {
                socket: left,
                max_bytes: right,
            }
            | Self::TcpStreamWriteString {
                stream: left,
                content: right,
            }
            | Self::FsWriteString {
                path: left,
                content: right,
            }
            | Self::FsWriteBytes {
                path: left,
                bytes: right,
            }
            | Self::EnvSet {
                name: left,
                value: right,
            }
            | Self::HashWriteString {
                state: left,
                value: right,
            }
            | Self::HashWriteBytes {
                state: left,
                value: right,
            }
            | Self::FileWriteString {
                file: left,
                content: right,
            }
            | Self::ArrayGet {
                array: left,
                index: right,
                ..
            }
            | Self::ArrayIndex {
                array: left,
                index: right,
                ..
            }
            | Self::ResultUnwrapOr {
                result: left,
                default: right,
                ..
            }
            | Self::OptionUnwrapOr {
                option: left,
                default: right,
                ..
            } => vec![left, right],
            Self::CollectionsStringMapSet { map, key, value } => vec![map, key, value],
            Self::UdpSocketSendToString {
                socket,
                content,
                host,
                port,
            } => vec![socket, content, host, port],
            Self::Call { args, .. }
            | Self::JsonStructured { args, .. }
            | Self::JsonRpc { args, .. }
            | Self::Cron { args, .. } => args.iter().collect(),
            Self::ArrayLiteral { elements, .. } => elements.iter().collect(),
            Self::ArrayRemove { index, .. } => vec![index],
            Self::ArraySet { index, value, .. } => vec![index, value],
            Self::ArrayPush { value, .. } => vec![value],
            Self::ArrayInsert { index, value, .. } => vec![index, value],
            Self::FsReadToString { path }
            | Self::FsReadBytes { path }
            | Self::FsOpen { path }
            | Self::FsExists { path }
            | Self::FsMetadata { path }
            | Self::FsCreateDir { path }
            | Self::FsRemoveDir { path }
            | Self::FsReadDir { path }
            | Self::FileClose { file: path }
            | Self::FileReadToString { file: path }
            | Self::TcpListenerAccept { listener: path }
            | Self::TcpListenerClose { listener: path }
            | Self::TcpStreamClose { stream: path }
            | Self::TcpStreamReadToString { stream: path }
            | Self::UdpSocketClose { socket: path }
            | Self::EnvGet { name: path }
            | Self::StringLen { value: path }
            | Self::StringIsEmpty { value: path }
            | Self::StringTrim { value: path }
            | Self::StringToLower { value: path }
            | Self::StringToUpper { value: path }
            | Self::CharIsDigit { value: path }
            | Self::CharIsAlpha { value: path }
            | Self::CharIsWhitespace { value: path }
            | Self::CharToString { value: path }
            | Self::PathBasename { path }
            | Self::PathDirname { path }
            | Self::PathExtension { path }
            | Self::PathNormalize { path }
            | Self::PathIsAbsolute { path }
            | Self::MathUnary { value: path, .. }
            | Self::TimeDurationMillis { millis: path }
            | Self::TimeDurationSeconds { seconds: path }
            | Self::TimeDurationAsMillis { duration: path }
            | Self::TimeFormatDuration { duration: path }
            | Self::TimeSleep { duration: path }
            | Self::TimeSleepMillis { duration: path }
            | Self::LogEnabled { level: path }
            | Self::HashString { value: path }
            | Self::HashBytes { value: path }
            | Self::HashFinish { state: path }
            | Self::CryptoSha256 { value: path }
            | Self::CryptoSha512 { value: path }
            | Self::CryptoRandomBytes { count: path }
            | Self::JsonParse { value: path }
            | Self::JsonStringify { value: path }
            | Self::RegexCompile { pattern: path }
            | Self::CollectionsStringMapLen { map: path }
            | Self::CollectionsStringSetLen { set: path }
            | Self::ProcessExit { code: path }
            | Self::ProcessSpawn { command: path }
            | Self::ProcessStatus { command: path }
            | Self::ProcessExec { command: path }
            | Self::ProcessOutput { command: path }
            | Self::NumParseI64 { value: path }
            | Self::NumParseU64 { value: path }
            | Self::NumParseF64 { value: path }
            | Self::NumToString { value: path, .. }
            | Self::Unary { expr: path, .. }
            | Self::Cast { expr: path, .. }
            | Self::ResultMapErr { result: path, .. }
            | Self::ResultIsOk { result: path, .. }
            | Self::ResultIsErr { result: path, .. }
            | Self::ResultMap { result: path, .. }
            | Self::ResultAndThen { result: path, .. }
            | Self::OptionIsSome { option: path, .. }
            | Self::OptionIsNone { option: path, .. }
            | Self::OptionMap { option: path, .. }
            | Self::OptionAndThen { option: path, .. }
            | Self::EnumPayload { value: path, .. }
            | Self::EnumPayloadFieldAccess { value: path, .. }
            | Self::ArrayIter { array: path, .. }
            | Self::ArrayLen { array: path }
            | Self::Panic { message: path, .. } => vec![path],
            Self::StructLiteral { fields, .. } => fields.iter().map(|(_, value)| value).collect(),
            Self::EnumVariant { payload, .. } => payload.iter().map(Box::as_ref).collect(),
            Self::If {
                condition,
                then_branch,
                else_branch,
            } => vec![condition, then_branch, else_branch],
            Self::Match { value, arms } => std::iter::once(value.as_ref())
                .chain(arms.iter().map(|arm| &arm.value))
                .collect(),
            Self::StringLiteral(_)
            | Self::IntLiteral(_)
            | Self::FloatLiteral(_)
            | Self::CharLiteral(_)
            | Self::BoolLiteral(_)
            | Self::VoidLiteral
            | Self::HashNew
            | Self::CollectionsStringMapNew
            | Self::CollectionsStringSetNew
            | Self::Variable(_)
            | Self::FunctionRef(_)
            | Self::MutBorrow(_)
            | Self::EnvArgs
            | Self::IoReadLine
            | Self::EnvCwd
            | Self::EnvHomeDir
            | Self::EnvTempDir
            | Self::OsPlatform
            | Self::OsArch
            | Self::OsPathSeparator
            | Self::OsLineEnding
            | Self::TimeNowMillis
            | Self::TimeMonotonicMillis
            | Self::ArrayNew { .. }
            | Self::ArrayPop { .. }
            | Self::ArrayClear { .. }
            | Self::FieldAccess { .. } => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchValueArm {
    pub enum_name: String,
    pub enum_args: Vec<ValueType>,
    pub variant: String,
    pub binding: Option<String>,
    pub value: ValueExpr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
    Negate,
}

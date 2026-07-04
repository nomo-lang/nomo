use crate::ast::{
    AssignOp, BinaryOp as AstBinaryOp, EnumDef as AstEnumDef, Expr as AstExpr, ForVariant,
    Function as AstFunction, MatchArm as AstMatchArm, PostfixOp, SourceFile, Span, Stmt,
    StructDef as AstStructDef, TypeRef as AstTypeRef, UnaryOp as AstUnaryOp,
};
use crate::codegen;
use crate::diagnostic::{Diagnostic, Suggestion};
use crate::lexer;
use crate::parser;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

const BUILTIN_PRINTLN_EXPR: &str = "__nomo_builtin_println";
const BUILTIN_PRINT_EXPR: &str = "__nomo_builtin_print";
const BUILTIN_EPRINTLN_EXPR: &str = "__nomo_builtin_eprintln";
const BUILTIN_EPRINT_EXPR: &str = "__nomo_builtin_eprint";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub package: String,
    pub imports: Vec<String>,
    pub structs: Vec<StructType>,
    pub enums: Vec<EnumType>,
    pub consts: Vec<Const>,
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalModule {
    pub import_root: String,
    pub source_root: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryMode {
    MainFunctionRequired,
    ScriptFile,
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
    },
    QuestionReturnOk {
        ok_type: ValueType,
        result_type: ValueType,
        return_type: ValueType,
        result_expr: ValueExpr,
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
pub enum LoopKind {
    Infinite,
    While(ValueExpr),
    Iterate {
        binding: String,
        element_type: ValueType,
        iterable: ValueExpr,
    },
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueType {
    String,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueExpr {
    StringLiteral(String),
    IntLiteral(i64),
    FloatLiteral(String),
    CharLiteral(char),
    BoolLiteral(bool),
    VoidLiteral,
    Variable(String),
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
    HashWriteString {
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
}

#[derive(Debug, Clone)]
struct FunctionSignature {
    type_params: Vec<String>,
    params: Vec<ParamSignature>,
    return_type: ValueType,
}

#[derive(Debug, Clone)]
struct ParamSignature {
    value_type: ValueType,
    mutable: bool,
}

#[derive(Debug, Clone)]
struct Binding {
    value_type: ValueType,
    mutable: bool,
    source: BindingSource,
}

#[derive(Debug, Clone)]
enum BindingSource {
    Local,
    Param,
    EnumPayload { value: ValueExpr, variant: String },
}

fn binding_value_expr(name: &str, binding: &Binding) -> ValueExpr {
    match &binding.source {
        BindingSource::Local | BindingSource::Param => ValueExpr::Variable(name.to_string()),
        BindingSource::EnumPayload { value, variant } => ValueExpr::EnumPayload {
            value: Box::new(value.clone()),
            variant: variant.clone(),
        },
    }
}

fn binding_source_noun(binding: &Binding) -> &'static str {
    match binding.source {
        BindingSource::Param => "parameter",
        _ => "variable",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FunctionInstance {
    name: String,
    args: Vec<ValueType>,
}

pub fn check_source(path: &Path) -> Result<Program, Diagnostic> {
    check_source_with_external_imports(path, &[])
}

pub fn check_source_with_external_imports(
    path: &Path,
    external_import_roots: &[String],
) -> Result<Program, Diagnostic> {
    check_source_with_external_modules(path, external_import_roots, &[])
}

pub fn check_source_with_external_modules(
    path: &Path,
    external_import_roots: &[String],
    external_modules: &[ExternalModule],
) -> Result<Program, Diagnostic> {
    let source = fs::read_to_string(path).map_err(|err| {
        Diagnostic::new(
            "E0001",
            format!("failed to read source file: {err}"),
            path,
            1,
            1,
            1,
            "",
        )
    })?;
    check_source_text_with_project_modules(
        path,
        &source,
        None,
        external_import_roots,
        external_modules,
    )
}

pub fn check_source_text(path: &Path, source: &str) -> Result<Program, Diagnostic> {
    check_source_text_with_external_imports(path, source, &[])
}

pub fn check_source_text_with_external_imports(
    path: &Path,
    source: &str,
    external_import_roots: &[String],
) -> Result<Program, Diagnostic> {
    check_source_text_with_external_modules(path, source, external_import_roots, &[])
}

pub fn check_source_text_with_external_modules(
    path: &Path,
    source: &str,
    external_import_roots: &[String],
    external_modules: &[ExternalModule],
) -> Result<Program, Diagnostic> {
    check_source_text_with_project_modules(
        path,
        source,
        None,
        external_import_roots,
        external_modules,
    )
}

pub fn check_source_text_with_project_modules(
    path: &Path,
    source: &str,
    local_source_root: Option<&Path>,
    external_import_roots: &[String],
    external_modules: &[ExternalModule],
) -> Result<Program, Diagnostic> {
    check_source_text_with_project_modules_and_overrides(
        path,
        source,
        local_source_root,
        external_import_roots,
        external_modules,
        &[],
    )
}

pub fn check_source_text_with_project_modules_and_overrides(
    path: &Path,
    source: &str,
    local_source_root: Option<&Path>,
    external_import_roots: &[String],
    external_modules: &[ExternalModule],
    module_source_overrides: &[(PathBuf, String)],
) -> Result<Program, Diagnostic> {
    let tokens = lexer::lex(path, source)?;
    let mut ast = parser::parse(path, &tokens)?;
    let local_import_root = local_source_root.and_then(|_| ast.package.first().cloned());
    let mut visited = HashSet::new();
    visited.insert(ast.package.clone());
    merge_imported_public_api(
        path,
        &mut ast,
        local_source_root,
        local_import_root.as_deref(),
        external_modules,
        module_source_overrides,
        &mut visited,
    )?;
    lower_program(
        path,
        ast,
        external_import_roots,
        local_import_root.as_deref(),
        EntryMode::MainFunctionRequired,
    )
}

pub fn check_script_source_text(path: &Path, source: &str) -> Result<Program, Diagnostic> {
    let tokens = lexer::lex(path, source)?;
    let ast = parser::parse(path, &tokens)?;
    lower_program(path, ast, &[], None, EntryMode::ScriptFile)
}

pub fn compile_source_to_c(path: &Path) -> Result<String, Diagnostic> {
    compile_source_to_c_with_external_imports(path, &[])
}

pub fn compile_script_source_to_c(path: &Path) -> Result<String, Diagnostic> {
    let source = fs::read_to_string(path).map_err(|err| {
        Diagnostic::new(
            "E0001",
            format!("failed to read source file: {err}"),
            path,
            1,
            1,
            1,
            "",
        )
    })?;
    let program = check_script_source_text(path, &source)?;
    Ok(codegen::emit_c(&program))
}

pub fn compile_source_to_c_with_external_imports(
    path: &Path,
    external_import_roots: &[String],
) -> Result<String, Diagnostic> {
    let program = check_source_with_external_modules(path, external_import_roots, &[])?;
    Ok(codegen::emit_c(&program))
}

pub fn compile_source_to_c_with_external_modules(
    path: &Path,
    external_import_roots: &[String],
    external_modules: &[ExternalModule],
) -> Result<String, Diagnostic> {
    let program =
        check_source_with_external_modules(path, external_import_roots, external_modules)?;
    Ok(codegen::emit_c(&program))
}

pub fn compile_source_to_c_with_project_modules(
    path: &Path,
    local_source_root: Option<&Path>,
    external_import_roots: &[String],
    external_modules: &[ExternalModule],
) -> Result<String, Diagnostic> {
    let source = fs::read_to_string(path).map_err(|err| {
        Diagnostic::new(
            "E0001",
            format!("failed to read source file: {err}"),
            path,
            1,
            1,
            1,
            "",
        )
    })?;
    let program = check_source_text_with_project_modules(
        path,
        &source,
        local_source_root,
        external_import_roots,
        external_modules,
    )?;
    Ok(codegen::emit_c(&program))
}

pub fn compile_source_text_to_c_with_project_modules(
    path: &Path,
    source: &str,
    local_source_root: Option<&Path>,
    external_import_roots: &[String],
    external_modules: &[ExternalModule],
) -> Result<String, Diagnostic> {
    let program = check_source_text_with_project_modules(
        path,
        source,
        local_source_root,
        external_import_roots,
        external_modules,
    )?;
    Ok(codegen::emit_c(&program))
}

fn merge_imported_public_api(
    importer_path: &Path,
    ast: &mut SourceFile,
    local_source_root: Option<&Path>,
    local_import_root: Option<&str>,
    external_modules: &[ExternalModule],
    module_source_overrides: &[(PathBuf, String)],
    visited: &mut HashSet<Vec<String>>,
) -> Result<(), Diagnostic> {
    let imports = ast.imports.clone();
    for import in imports {
        if import.first().is_some_and(|root| root == "std") {
            continue;
        }
        let Some((source_root, module_path)) = resolve_imported_module(
            importer_path,
            &import,
            local_source_root,
            local_import_root,
            external_modules,
        )?
        else {
            continue;
        };
        let Some(source_path) = module_source_path(source_root, &module_path) else {
            return Err(Diagnostic::new(
                "E0903",
                format!("could not find module `{}`", import.join(".")),
                importer_path,
                1,
                1,
                import.join(".").len().max(1),
                import.join("."),
            ));
        };
        let source_override = module_source_overrides
            .iter()
            .find(|(path, _)| path == &source_path)
            .map(|(_, source)| source.as_str());
        let source = match source_override {
            Some(source) => source.to_string(),
            None => fs::read_to_string(&source_path).map_err(|err| {
                Diagnostic::new(
                    "E0902",
                    format!("failed to read module `{}`: {err}", source_path.display()),
                    importer_path,
                    1,
                    1,
                    1,
                    "",
                )
            })?,
        };
        let tokens = lexer::lex(&source_path, &source)?;
        let mut module_ast = parser::parse(&source_path, &tokens)?;
        reject_script_body(
            &source_path,
            &module_ast,
            "imported modules cannot contain top-level script statements",
        )?;
        if module_ast.package != import {
            return Err(Diagnostic::new(
                "E0904",
                format!(
                    "module `{}` declares package `{}`",
                    import.join("."),
                    module_ast.package.join(".")
                ),
                &source_path,
                1,
                1,
                module_ast.package.join(".").len().max(1),
                module_ast.package.join("."),
            ));
        }
        if !visited.insert(module_ast.package.clone()) {
            continue;
        }
        merge_imported_public_api(
            &source_path,
            &mut module_ast,
            local_source_root,
            local_import_root,
            external_modules,
            module_source_overrides,
            visited,
        )?;
        merge_public_items(ast, module_ast);
    }
    Ok(())
}

fn resolve_imported_module<'a>(
    importer_path: &Path,
    import: &[String],
    local_source_root: Option<&'a Path>,
    local_import_root: Option<&str>,
    external_modules: &'a [ExternalModule],
) -> Result<Option<(&'a Path, Vec<String>)>, Diagnostic> {
    let Some(import_root) = import.first() else {
        return Ok(None);
    };
    if local_import_root.is_some_and(|root| root == import_root) {
        let Some(source_root) = local_source_root else {
            return Ok(None);
        };
        return Ok(Some((source_root, import[1..].to_vec())));
    }
    if let Some(module) = external_modules
        .iter()
        .find(|module| module.import_root == *import_root)
    {
        return Ok(Some((module.source_root.as_path(), import[1..].to_vec())));
    }
    if external_modules
        .iter()
        .any(|module| module.import_root == *import_root)
    {
        return Ok(None);
    }
    let _ = importer_path;
    Ok(None)
}

fn module_source_path(source_root: &Path, module_path: &[String]) -> Option<PathBuf> {
    if module_path.is_empty() || (module_path.len() == 1 && module_path[0] == "main") {
        let main = source_root.join("main.nomo");
        return main.is_file().then_some(main);
    }
    let mut flat = source_root.to_path_buf();
    for segment in module_path {
        flat.push(segment);
    }
    flat.set_extension("nomo");
    if flat.is_file() {
        return Some(flat);
    }
    let mut dir_main = source_root.to_path_buf();
    for segment in module_path {
        dir_main.push(segment);
    }
    dir_main.push("main.nomo");
    dir_main.is_file().then_some(dir_main)
}

fn merge_public_items(ast: &mut SourceFile, module_ast: SourceFile) {
    let public_structs = module_ast
        .structs
        .iter()
        .filter(|item| item.public)
        .map(|item| item.name.clone())
        .collect::<HashSet<_>>();

    ast.imports.extend(module_ast.imports);
    ast.structs
        .extend(module_ast.structs.into_iter().filter(|item| item.public));
    ast.enums
        .extend(module_ast.enums.into_iter().filter(|item| item.public));
    ast.consts
        .extend(module_ast.consts.into_iter().filter(|item| item.public));
    ast.functions.extend(
        module_ast
            .functions
            .into_iter()
            .filter(|item| item.public && item.name != "main"),
    );
    ast.impls
        .extend(module_ast.impls.into_iter().filter_map(|mut item| {
            let target = item.type_name.path.first()?;
            if !public_structs.contains(target) {
                return None;
            }
            item.methods.retain(|method| method.public);
            if item.methods.is_empty() {
                None
            } else {
                Some(item)
            }
        }));
}

fn lower_program(
    path: &Path,
    mut ast: SourceFile,
    external_import_roots: &[String],
    local_import_root: Option<&str>,
    entry_mode: EntryMode,
) -> Result<Program, Diagnostic> {
    let imports = ast
        .imports
        .iter()
        .map(|path| path.join("."))
        .collect::<Vec<_>>();
    validate_imports(path, &imports, external_import_roots, local_import_root)?;
    prepare_entry_point(path, &mut ast, entry_mode)?;
    validate_standard_type_imports(path, &imports, &ast)?;
    let standard_type_needs = standard_type_needs(&imports, &ast);
    validate_standard_type_conflicts(path, standard_type_needs, &ast.structs, &ast.enums)?;
    let mut structs = lower_structs(path, &ast.structs, &ast.enums, standard_type_needs)?;
    let mut enums = lower_enums(path, &structs, &ast.enums, standard_type_needs)?;
    inject_standard_types(standard_type_needs, &mut structs, &mut enums);
    validate_type_namespace(path, &structs, &enums)?;
    validate_no_recursive_value_types(path, &structs, &enums)?;
    let struct_map = structs
        .iter()
        .map(|item| (item.name.clone(), item.clone()))
        .collect::<HashMap<_, _>>();
    let enum_map = enums
        .iter()
        .map(|item| (item.name.clone(), item.clone()))
        .collect::<HashMap<_, _>>();
    let mut signatures = HashMap::new();
    for function in &ast.functions {
        if signatures.contains_key(&function.name) {
            return Err(Diagnostic::new(
                "E0304",
                format!("function `{}` is already defined", function.name),
                path,
                function.span.line,
                function.span.column,
                function.span.length,
                &function.span.text,
            ));
        }
        signatures.insert(
            function.name.clone(),
            function_signature(path, function, &struct_map, &enum_map)?,
        );
    }
    let local_struct_names = ast
        .structs
        .iter()
        .map(|item| item.name.as_str())
        .collect::<HashSet<_>>();
    for impl_block in &ast.impls {
        let impl_target = impl_block.type_name.path.join(".");
        if !impl_block
            .type_name
            .path
            .first()
            .is_some_and(|name| local_struct_names.contains(name.as_str()))
        {
            return Err(Diagnostic::new(
                "E0255",
                format!("v0.1 impl blocks must target a local struct, got `{impl_target}`"),
                path,
                1,
                1,
                1,
                "",
            ));
        }
        let owner =
            parse_value_type(&impl_block.type_name, &struct_map, &enum_map).ok_or_else(|| {
                Diagnostic::new(
                    "E0309",
                    format!("unknown impl target `{impl_target}`"),
                    path,
                    1,
                    1,
                    1,
                    "",
                )
            })?;
        let ValueType::Struct(owner_name, owner_args) = owner else {
            return Err(Diagnostic::new(
                "E0255",
                "v0.1 impl blocks can only target structs",
                path,
                1,
                1,
                1,
                "",
            ));
        };
        if !owner_args.is_empty() {
            return Err(Diagnostic::new(
                "E0255",
                "v0.1 impl blocks can only target non-generic structs",
                path,
                1,
                1,
                1,
                "",
            ));
        }
        for method in &impl_block.methods {
            validate_method_self(path, method, &owner_name, &struct_map, &enum_map)?;
            let lowered_name = method_internal_name(&owner_name, &method.name);
            if signatures.contains_key(&lowered_name) {
                return Err(Diagnostic::new(
                    "E0304",
                    format!("method `{owner_name}.{}` is already defined", method.name),
                    path,
                    method.span.line,
                    method.span.column,
                    method.span.length,
                    &method.span.text,
                ));
            }
            signatures.insert(
                lowered_name,
                function_signature(path, method, &struct_map, &enum_map)?,
            );
        }
    }

    let Some(main_signature) = signatures.get("main") else {
        return Err(Diagnostic::new(
            "E0201",
            "expected `fn main() -> void { ... }`",
            path,
            1,
            1,
            1,
            "",
        ));
    };
    let valid_main_return = main_signature.return_type == ValueType::Void
        || matches!(
            result_parts(&main_signature.return_type),
            Some((ValueType::Void, _))
        );
    if !main_signature.params.is_empty() || !valid_main_return {
        return Err(Diagnostic::new(
            "E0401",
            "v0.1 `main` must return `void` or `Result<void, E>`",
            path,
            1,
            1,
            1,
            "",
        ));
    }
    if !main_signature.type_params.is_empty() {
        return Err(Diagnostic::new(
            "E0401",
            "v0.1 `main` cannot be generic",
            path,
            1,
            1,
            1,
            "",
        ));
    }

    let function_defs = ast
        .functions
        .iter()
        .map(|function| (function.name.clone(), function))
        .collect::<HashMap<_, _>>();
    let generic_instances = collect_generic_function_instances(
        path,
        &ast,
        &imports,
        &signatures,
        &struct_map,
        &enum_map,
    )?;
    for instance in &generic_instances {
        let signature = signatures
            .get(&instance.name)
            .expect("generic function instance must refer to a known function");
        let instance_name = generic_function_instance_name(&instance.name, &instance.args);
        signatures.insert(
            instance_name,
            instantiate_function_signature(signature, &instance.args),
        );
    }

    let mut const_types: Vec<(String, ValueType)> = Vec::new();
    let mut consts = Vec::new();
    for const_def in &ast.consts {
        let struct_names = struct_map
            .values()
            .map(|item| (item.name.clone(), item.type_params.len()))
            .collect::<Vec<_>>();
        let enum_names = enum_map
            .values()
            .map(|item| (item.name.clone(), item.type_params.len()))
            .collect::<Vec<_>>();
        let value_type =
            parse_value_type(&const_def.type_ref, &struct_map, &enum_map).ok_or_else(|| {
                unsupported_type_diagnostic(
                    path,
                    &const_def.span,
                    &const_def.type_ref,
                    format!(
                        "unsupported constant type `{}` in v0.1 current implementation",
                        const_def.type_ref.path.join(".")
                    ),
                    &struct_names,
                    &enum_names,
                )
            })?;
        ensure_supported_value_type(path, &value_type, &const_def.span)?;
        let mut const_scope = HashMap::new();
        let (init_type, initializer) = lower_value_expr_with_expected(
            path,
            &const_def.value,
            &mut const_scope,
            &imports,
            &signatures,
            &struct_map,
            &enum_map,
            Some(&value_type),
            &const_def.span,
        )?;
        if init_type != value_type {
            return Err(type_mismatch_expected_found(
                path,
                &const_def.span,
                format!(
                    "constant `{}` is annotated as `{}` but initializer is `{}`",
                    const_def.name,
                    value_type.name(),
                    init_type.name()
                ),
                &value_type,
                &init_type,
            ));
        }
        if !is_constant_expr(&initializer) {
            return Err(Diagnostic::new(
                "E0430",
                "`const` initializer must be a constant expression (a literal)",
                path,
                const_def.span.line,
                const_def.span.column,
                const_def.span.length,
                &const_def.span.text,
            ));
        }
        if const_types.iter().any(|(name, _)| name == &const_def.name) {
            return Err(Diagnostic::new(
                "E0304",
                format!("constant `{}` is already defined", const_def.name),
                path,
                const_def.span.line,
                const_def.span.column,
                const_def.span.length,
                &const_def.span.text,
            ));
        }
        const_types.push((const_def.name.clone(), value_type.clone()));
        consts.push(Const {
            name: const_def.name.clone(),
            value_type,
            initializer,
        });
    }

    let mut functions = Vec::new();
    for function in &ast.functions {
        if !function.type_params.is_empty() {
            continue;
        }
        functions.push(lower_function_as(
            path,
            function,
            &function.name,
            &imports,
            &signatures,
            &struct_map,
            &enum_map,
            &const_types,
        )?);
    }
    for impl_block in &ast.impls {
        let owner_name = impl_block.type_name.path[0].clone();
        for method in &impl_block.methods {
            let lowered_name = method_internal_name(&owner_name, &method.name);
            functions.push(lower_function_as(
                path,
                method,
                &lowered_name,
                &imports,
                &signatures,
                &struct_map,
                &enum_map,
                &const_types,
            )?);
        }
    }
    for instance in &generic_instances {
        let Some(function) = function_defs.get(&instance.name) else {
            continue;
        };
        let lowered_name = generic_function_instance_name(&instance.name, &instance.args);
        functions.push(lower_function_as(
            path,
            function,
            &lowered_name,
            &imports,
            &signatures,
            &struct_map,
            &enum_map,
            &const_types,
        )?);
    }

    Ok(Program {
        package: ast.package.join("."),
        imports,
        structs,
        enums,
        consts,
        functions,
    })
}

fn prepare_entry_point(
    path: &Path,
    ast: &mut SourceFile,
    entry_mode: EntryMode,
) -> Result<(), Diagnostic> {
    let has_main = ast.functions.iter().any(|function| function.name == "main");
    match entry_mode {
        EntryMode::MainFunctionRequired => {
            reject_script_body(
                path,
                ast,
                "top-level script statements are only supported by `nomo run <source.nomo>`",
            )?;
        }
        EntryMode::ScriptFile if has_main && !ast.script_body.is_empty() => {
            return Err(script_body_diagnostic(
                path,
                &ast.script_body,
                "top-level script statements cannot be combined with an explicit `main` function",
            ));
        }
        EntryMode::ScriptFile if !has_main && !ast.script_body.is_empty() => {
            let span = stmt_span(&ast.script_body[0]).clone();
            ast.functions.push(AstFunction {
                public: false,
                is_test: false,
                package: ast.package.clone(),
                name: "main".to_string(),
                type_params: Vec::new(),
                params: Vec::new(),
                return_type: AstTypeRef {
                    path: vec!["void".to_string()],
                    args: Vec::new(),
                },
                body: std::mem::take(&mut ast.script_body),
                span,
            });
        }
        EntryMode::ScriptFile => {}
    }
    Ok(())
}

fn reject_script_body(
    path: &Path,
    ast: &SourceFile,
    message: &'static str,
) -> Result<(), Diagnostic> {
    if ast.script_body.is_empty() {
        Ok(())
    } else {
        Err(script_body_diagnostic(path, &ast.script_body, message))
    }
}

fn script_body_diagnostic(path: &Path, script_body: &[Stmt], message: &'static str) -> Diagnostic {
    let span = stmt_span(&script_body[0]);
    Diagnostic::new(
        "E0201",
        message,
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    )
}

fn stmt_span(stmt: &Stmt) -> &Span {
    match stmt {
        Stmt::Let { span, .. }
        | Stmt::LetElse { span, .. }
        | Stmt::IfLet { span, .. }
        | Stmt::Assign { span, .. }
        | Stmt::Postfix { span, .. }
        | Stmt::Return { span, .. }
        | Stmt::Match { span, .. }
        | Stmt::Expr { span, .. }
        | Stmt::For { span, .. }
        | Stmt::Break { span, .. }
        | Stmt::Continue { span, .. }
        | Stmt::Defer { span, .. } => span,
    }
}

fn validate_imports(
    path: &Path,
    imports: &[String],
    external_import_roots: &[String],
    local_import_root: Option<&str>,
) -> Result<(), Diagnostic> {
    for import in imports {
        let is_local_import = local_import_root
            .is_some_and(|root| import.split('.').next().is_some_and(|item| item == root));
        if !is_local_import && !is_supported_import(import, external_import_roots) {
            return Err(Diagnostic::new(
                "E0301",
                format!("unsupported import `{import}` in v0.1"),
                path,
                1,
                1,
                import.len().max(1),
                import,
            ));
        }
    }
    Ok(())
}

fn validate_standard_type_imports(
    path: &Path,
    imports: &[String],
    ast: &SourceFile,
) -> Result<(), Diagnostic> {
    for item in &ast.structs {
        for field in &item.fields {
            validate_type_ref_imports(path, imports, &field.type_ref, &synthetic_span())?;
        }
    }
    for item in &ast.enums {
        for variant in &item.variants {
            if let Some(type_ref) = &variant.payload {
                validate_type_ref_imports(path, imports, type_ref, &synthetic_span())?;
            }
        }
    }
    for item in &ast.consts {
        validate_type_ref_imports(path, imports, &item.type_ref, &item.span)?;
        validate_expr_type_imports(path, imports, &item.value, &item.span)?;
    }
    for function in ast_functions(ast) {
        for param in &function.params {
            validate_type_ref_imports(path, imports, &param.type_ref, &function.span)?;
        }
        validate_type_ref_imports(path, imports, &function.return_type, &function.span)?;
        for stmt in &function.body {
            validate_stmt_type_imports(path, imports, stmt)?;
        }
    }
    Ok(())
}

fn validate_stmt_type_imports(
    path: &Path,
    imports: &[String],
    stmt: &Stmt,
) -> Result<(), Diagnostic> {
    match stmt {
        Stmt::Let {
            type_annotation,
            value,
            span,
            ..
        } => {
            if let Some(type_ref) = type_annotation {
                validate_type_ref_imports(path, imports, type_ref, span)?;
            }
            validate_expr_type_imports(path, imports, value, span)
        }
        Stmt::LetElse {
            value,
            else_body,
            span,
            ..
        } => {
            validate_expr_type_imports(path, imports, value, span)?;
            for stmt in else_body {
                validate_stmt_type_imports(path, imports, stmt)?;
            }
            Ok(())
        }
        Stmt::IfLet {
            value,
            body,
            else_body,
            span,
            ..
        } => {
            validate_expr_type_imports(path, imports, value, span)?;
            for stmt in body {
                validate_stmt_type_imports(path, imports, stmt)?;
            }
            if let Some(else_body) = else_body {
                for stmt in else_body {
                    validate_stmt_type_imports(path, imports, stmt)?;
                }
            }
            Ok(())
        }
        Stmt::Assign { value, span, .. }
        | Stmt::Return {
            value: Some(value),
            span,
        }
        | Stmt::Expr { expr: value, span } => {
            validate_expr_type_imports(path, imports, value, span)
        }
        Stmt::Postfix { .. }
        | Stmt::Return { value: None, .. }
        | Stmt::Break { .. }
        | Stmt::Continue { .. } => Ok(()),
        Stmt::Match { value, arms, span } => {
            validate_expr_type_imports(path, imports, value, span)?;
            for arm in arms {
                for stmt in &arm.body {
                    validate_stmt_type_imports(path, imports, stmt)?;
                }
            }
            Ok(())
        }
        Stmt::For { variant, span } => match variant {
            ForVariant::Infinite { body } => {
                for stmt in body {
                    validate_stmt_type_imports(path, imports, stmt)?;
                }
                Ok(())
            }
            ForVariant::While { condition, body } => {
                validate_expr_type_imports(path, imports, condition, span)?;
                for stmt in body {
                    validate_stmt_type_imports(path, imports, stmt)?;
                }
                Ok(())
            }
            ForVariant::Iterate { iterable, body, .. } => {
                validate_expr_type_imports(path, imports, iterable, span)?;
                for stmt in body {
                    validate_stmt_type_imports(path, imports, stmt)?;
                }
                Ok(())
            }
        },
        Stmt::Defer { stmt, .. } => validate_stmt_type_imports(path, imports, stmt),
    }
}

fn validate_expr_type_imports(
    path: &Path,
    imports: &[String],
    expr: &AstExpr,
    span: &Span,
) -> Result<(), Diagnostic> {
    match expr {
        AstExpr::Call {
            type_args, args, ..
        } => {
            for type_ref in type_args {
                validate_type_ref_imports(path, imports, type_ref, span)?;
            }
            for arg in args {
                validate_expr_type_imports(path, imports, arg, span)?;
            }
            Ok(())
        }
        AstExpr::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                validate_expr_type_imports(path, imports, value, span)?;
            }
            Ok(())
        }
        AstExpr::Match { value, arms } => {
            validate_expr_type_imports(path, imports, value, span)?;
            for arm in arms {
                validate_expr_type_imports(path, imports, &arm.value, span)?;
            }
            Ok(())
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            validate_expr_type_imports(path, imports, condition, span)?;
            validate_expr_type_imports(path, imports, then_branch, span)?;
            validate_expr_type_imports(path, imports, else_branch, span)
        }
        AstExpr::Panic { message }
        | AstExpr::Question { expr: message }
        | AstExpr::Unary { expr: message, .. } => {
            validate_expr_type_imports(path, imports, message, span)
        }
        AstExpr::Cast { expr, target } => {
            validate_expr_type_imports(path, imports, expr, span)?;
            validate_type_ref_imports(path, imports, target, span)
        }
        AstExpr::Binary { left, right, .. } => {
            validate_expr_type_imports(path, imports, left, span)?;
            validate_expr_type_imports(path, imports, right, span)
        }
        AstExpr::MutArg { .. }
        | AstExpr::Name(_)
        | AstExpr::String(_)
        | AstExpr::Int(_)
        | AstExpr::Float(_)
        | AstExpr::Char(_)
        | AstExpr::Bool(_)
        | AstExpr::Void => Ok(()),
    }
}

fn validate_type_ref_imports(
    path: &Path,
    imports: &[String],
    type_ref: &crate::ast::TypeRef,
    span: &Span,
) -> Result<(), Diagnostic> {
    if type_ref.path == ["Array"]
        && !imports
            .iter()
            .any(|item| item == "std.array" || item == "std.array.Array")
    {
        return Err(Diagnostic::new(
            "E0301",
            "`Array` requires `import std.array.Array`",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    for arg in &type_ref.args {
        validate_type_ref_imports(path, imports, arg, span)?;
    }
    Ok(())
}

fn is_supported_import(import: &str, external_import_roots: &[String]) -> bool {
    matches!(
        import,
        "std.io"
            | "std.io.print"
            | "std.io.println"
            | "std.io.read_line"
            | "std.io.eprint"
            | "std.io.eprintln"
            | "std.fs"
            | "std.fs.FsError"
            | "std.fs.File"
            | "std.fs.FileMetadata"
            | "std.fs.read_to_string"
            | "std.fs.write_string"
            | "std.fs.read_bytes"
            | "std.fs.write_bytes"
            | "std.fs.exists"
            | "std.fs.metadata"
            | "std.fs.create_dir"
            | "std.fs.remove_dir"
            | "std.fs.read_dir"
            | "std.fs.open"
            | "std.env"
            | "std.env.args"
            | "std.env.cwd"
            | "std.env.get"
            | "std.env.home_dir"
            | "std.env.set"
            | "std.env.temp_dir"
            | "std.result"
            | "std.result.Result"
            | "std.result.is_ok"
            | "std.result.is_err"
            | "std.result.unwrap_or"
            | "std.result.map"
            | "std.result.map_err"
            | "std.result.and_then"
            | "std.option"
            | "std.option.Option"
            | "std.option.is_some"
            | "std.option.is_none"
            | "std.option.unwrap_or"
            | "std.option.map"
            | "std.option.and_then"
            | "std.array"
            | "std.array.Array"
            | "std.array.new"
            | "std.array.len"
            | "std.array.push"
            | "std.array.get"
            | "std.array.set"
            | "std.array.pop"
            | "std.array.insert"
            | "std.array.remove"
            | "std.array.clear"
            | "std.array.iter"
            | "std.string"
            | "std.string.len"
            | "std.string.concat"
            | "std.string.is_empty"
            | "std.string.contains"
            | "std.string.starts_with"
            | "std.string.ends_with"
            | "std.string.split"
            | "std.string.trim"
            | "std.string.to_lower"
            | "std.string.to_upper"
            | "std.char"
            | "std.char.is_digit"
            | "std.char.is_alpha"
            | "std.char.is_whitespace"
            | "std.char.to_string"
            | "std.debug"
            | "std.debug.print"
            | "std.debug.println"
            | "std.debug.panic"
            | "std.debug.backtrace"
            | "std.log"
            | "std.log.debug"
            | "std.log.info"
            | "std.log.warn"
            | "std.log.error"
            | "std.log.enabled"
            | "std.hash"
            | "std.hash.HashState"
            | "std.hash.new"
            | "std.hash.string"
            | "std.hash.write_string"
            | "std.hash.finish"
            | "std.crypto"
            | "std.crypto.sha256"
            | "std.crypto.sha512"
            | "std.crypto.random_bytes"
            | "std.json"
            | "std.json.JsonValue"
            | "std.json.JsonError"
            | "std.json.parse"
            | "std.json.stringify"
            | "std.regex"
            | "std.regex.Regex"
            | "std.regex.RegexError"
            | "std.regex.compile"
            | "std.regex.is_match"
            | "std.regex.captures"
            | "std.collections"
            | "std.collections.StringMap"
            | "std.collections.StringSet"
            | "std.collections.map_new"
            | "std.collections.map_len"
            | "std.collections.map_get"
            | "std.collections.map_contains"
            | "std.collections.map_set"
            | "std.collections.map_remove"
            | "std.collections.set_new"
            | "std.collections.set_len"
            | "std.collections.set_contains"
            | "std.collections.set_insert"
            | "std.collections.set_remove"
            | "std.os"
            | "std.os.platform"
            | "std.os.arch"
            | "std.os.path_separator"
            | "std.os.line_ending"
            | "std.time"
            | "std.time.Duration"
            | "std.time.duration_millis"
            | "std.time.duration_seconds"
            | "std.time.duration_as_millis"
            | "std.time.format_duration"
            | "std.time.sleep"
            | "std.time.now_millis"
            | "std.time.monotonic_millis"
            | "std.time.sleep_millis"
            | "std.testing"
            | "std.testing.assert"
            | "std.testing.assert_equal"
            | "std.testing.assert_error"
            | "std.process"
            | "std.process.ProcessError"
            | "std.process.ProcessOutput"
            | "std.process.exit"
            | "std.process.spawn"
            | "std.process.status"
            | "std.process.exec"
            | "std.process.output"
            | "std.num"
            | "std.num.NumError"
            | "std.num.parse_i64"
            | "std.num.parse_u64"
            | "std.num.parse_f64"
            | "std.num.checked_add"
            | "std.num.checked_sub"
            | "std.num.checked_mul"
            | "std.num.wrapping_add"
            | "std.num.wrapping_sub"
            | "std.num.wrapping_mul"
            | "std.path"
            | "std.path.join"
            | "std.path.basename"
            | "std.path.dirname"
            | "std.path.extension"
            | "std.path.normalize"
            | "std.path.is_absolute"
            | "std.math"
            | "std.math.abs"
            | "std.math.min"
            | "std.math.max"
            | "std.math.floor"
            | "std.math.ceil"
            | "std.math.round"
            | "std.math.sqrt"
            | "std.math.pow"
            | "std.math.sin"
            | "std.math.cos"
    ) || is_supported_external_import(import, external_import_roots)
}

fn is_supported_external_import(import: &str, external_import_roots: &[String]) -> bool {
    let Some((root, _rest)) = import.split_once('.') else {
        return false;
    };
    root != "std" && external_import_roots.iter().any(|alias| alias == root)
}

fn is_constant_expr(expr: &ValueExpr) -> bool {
    matches!(
        expr,
        ValueExpr::IntLiteral(_)
            | ValueExpr::FloatLiteral(_)
            | ValueExpr::StringLiteral(_)
            | ValueExpr::BoolLiteral(_)
            | ValueExpr::CharLiteral(_)
    )
}

fn unsupported_type_diagnostic(
    path: &Path,
    span: &Span,
    type_ref: &crate::ast::TypeRef,
    message: impl Into<String>,
    struct_names: &[(String, usize)],
    enum_names: &[(String, usize)],
) -> Diagnostic {
    if type_ref.path == ["int"] {
        return Diagnostic::new(
            "E0403",
            "`int` is not a v0.1 builtin type; use `i64` or an explicit-width integer type (`i32`, `u32`, `u64`)",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        );
    }

    if let Some(import) = missing_standard_type_import(type_ref, struct_names, enum_names) {
        let type_name = type_ref.path.first().expect("type ref must have a root");
        return Diagnostic::new(
            "E0301",
            format!("`{type_name}` requires `import {import}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        );
    }
    Diagnostic::new(
        "E0403",
        message,
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    )
}

fn unsupported_type_diagnostic_from_maps(
    path: &Path,
    span: &Span,
    type_ref: &crate::ast::TypeRef,
    message: impl Into<String>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Diagnostic {
    let struct_names = structs
        .values()
        .map(|item| (item.name.clone(), item.type_params.len()))
        .collect::<Vec<_>>();
    let enum_names = enums
        .values()
        .map(|item| (item.name.clone(), item.type_params.len()))
        .collect::<Vec<_>>();
    unsupported_type_diagnostic(path, span, type_ref, message, &struct_names, &enum_names)
}

fn missing_standard_type_import(
    type_ref: &crate::ast::TypeRef,
    struct_names: &[(String, usize)],
    enum_names: &[(String, usize)],
) -> Option<&'static str> {
    let root = type_ref.path.first()?;
    if struct_names.iter().any(|(name, _)| name == root)
        || enum_names.iter().any(|(name, _)| name == root)
    {
        return None;
    }
    match root.as_str() {
        "Result" => Some("std.result"),
        "Option" => Some("std.option"),
        "Array" => Some("std.array"),
        "FsError" | "File" | "FileMetadata" => Some("std.fs"),
        "IoError" => Some("std.io"),
        "NumError" => Some("std.num"),
        "HashState" => Some("std.hash"),
        "JsonValue" | "JsonError" => Some("std.json"),
        "Regex" | "RegexError" => Some("std.regex"),
        "StringMap" | "StringSet" => Some("std.collections"),
        "Duration" => Some("std.time"),
        _ => None,
    }
}

fn validate_type_namespace(
    path: &Path,
    structs: &[StructType],
    enums: &[EnumType],
) -> Result<(), Diagnostic> {
    let struct_names = structs
        .iter()
        .map(|item| item.name.as_str())
        .collect::<HashSet<_>>();
    for enum_type in enums {
        if struct_names.contains(enum_type.name.as_str()) {
            return Err(Diagnostic::new(
                "E0312",
                format!("type `{}` is already defined", enum_type.name),
                path,
                1,
                1,
                1,
                "",
            ));
        }
    }
    Ok(())
}

fn validate_no_recursive_value_types(
    path: &Path,
    structs: &[StructType],
    enums: &[EnumType],
) -> Result<(), Diagnostic> {
    let mut graph = HashMap::<String, Vec<String>>::new();
    let nominal_names = structs
        .iter()
        .map(|item| item.name.as_str())
        .chain(enums.iter().map(|item| item.name.as_str()))
        .collect::<HashSet<_>>();

    for struct_type in structs {
        let mut deps = Vec::new();
        for field in &struct_type.fields {
            collect_value_type_dependencies(&field.value_type, &nominal_names, &mut deps);
        }
        graph.insert(struct_type.name.clone(), deps);
    }
    for enum_type in enums {
        let mut deps = Vec::new();
        for variant in &enum_type.variants {
            if let Some(payload) = &variant.payload {
                collect_value_type_dependencies(payload, &nominal_names, &mut deps);
            }
        }
        graph.insert(enum_type.name.clone(), deps);
    }

    for name in graph.keys() {
        let mut visiting = Vec::new();
        let mut visited = HashSet::new();
        if type_dependency_reaches(name, name, &graph, &mut visiting, &mut visited) {
            return Err(Diagnostic::new(
                "E0410",
                format!("type `{name}` is recursively embedded by value"),
                path,
                1,
                1,
                1,
                "",
            ));
        }
    }
    Ok(())
}

fn collect_value_type_dependencies(
    value_type: &ValueType,
    nominal_names: &HashSet<&str>,
    out: &mut Vec<String>,
) {
    match value_type {
        ValueType::Struct(name, args) | ValueType::Enum(name, args) => {
            if nominal_names.contains(name.as_str()) {
                out.push(name.clone());
            }
            for arg in args {
                collect_value_type_dependencies(arg, nominal_names, out);
            }
        }
        ValueType::Array(_) => {}
        ValueType::String
        | ValueType::Int
        | ValueType::I32
        | ValueType::U32
        | ValueType::U64
        | ValueType::Float
        | ValueType::Char
        | ValueType::Bool
        | ValueType::TypeParam(_)
        | ValueType::Void
        | ValueType::Never => {}
    }
}

fn type_dependency_reaches(
    start: &str,
    current: &str,
    graph: &HashMap<String, Vec<String>>,
    visiting: &mut Vec<String>,
    visited: &mut HashSet<String>,
) -> bool {
    if !visited.insert(current.to_string()) {
        return false;
    }
    visiting.push(current.to_string());
    for dep in graph.get(current).into_iter().flatten() {
        if dep == start {
            return true;
        }
        if !visiting.iter().any(|item| item == dep)
            && type_dependency_reaches(start, dep, graph, visiting, visited)
        {
            return true;
        }
    }
    visiting.pop();
    false
}

fn validate_standard_type_conflicts(
    path: &Path,
    needs: StandardTypeNeeds,
    structs: &[AstStructDef],
    enums: &[AstEnumDef],
) -> Result<(), Diagnostic> {
    if needs.io {
        reject_user_std_struct(path, structs, "IoError")?;
    }
    if needs.fs {
        reject_user_std_struct(path, structs, "FsError")?;
        reject_user_std_struct(path, structs, "File")?;
    }
    if needs.num {
        reject_user_std_struct(path, structs, "NumError")?;
    }
    if needs.process {
        reject_user_std_struct(path, structs, "ProcessError")?;
        reject_user_std_struct(path, structs, "ProcessOutput")?;
    }
    if needs.hash {
        reject_user_std_struct(path, structs, "HashState")?;
    }
    if needs.io || needs.fs || needs.num || needs.process || needs.result {
        reject_user_std_enum(path, enums, "Result")?;
    }
    if needs.env || needs.num || needs.option || needs.array {
        reject_user_std_enum(path, enums, "Option")?;
    }
    Ok(())
}

fn reject_user_std_struct(
    path: &Path,
    structs: &[AstStructDef],
    name: &str,
) -> Result<(), Diagnostic> {
    if structs.iter().any(|item| item.name == name) {
        return Err(Diagnostic::new(
            "E0312",
            format!("type `{name}` conflicts with a required standard library type"),
            path,
            1,
            1,
            1,
            "",
        ));
    }
    Ok(())
}

fn reject_user_std_enum(path: &Path, enums: &[AstEnumDef], name: &str) -> Result<(), Diagnostic> {
    if enums.iter().any(|item| item.name == name) {
        return Err(Diagnostic::new(
            "E0312",
            format!("type `{name}` conflicts with a required standard library type"),
            path,
            1,
            1,
            1,
            "",
        ));
    }
    Ok(())
}

fn lower_structs(
    path: &Path,
    structs: &[AstStructDef],
    enums: &[AstEnumDef],
    standard_type_needs: StandardTypeNeeds,
) -> Result<Vec<StructType>, Diagnostic> {
    let mut lowered = Vec::new();
    let mut known = HashMap::new();
    for item in structs {
        if known.contains_key(&item.name) {
            return Err(Diagnostic::new(
                "E0306",
                format!("struct `{}` is already defined", item.name),
                path,
                1,
                1,
                1,
                "",
            ));
        }
        known.insert(item.name.clone(), item.type_params.len());
    }
    let known_structs = known
        .iter()
        .map(|(name, arity)| (name.clone(), *arity))
        .chain(standard_struct_names(standard_type_needs))
        .collect::<Vec<_>>();
    let known_enums = enums
        .iter()
        .map(|item| (item.name.clone(), item.type_params.len()))
        .chain(standard_enum_names(standard_type_needs))
        .collect::<Vec<_>>();

    for item in structs {
        let mut fields = Vec::new();
        let mut field_names = HashMap::new();
        for field in &item.fields {
            if field_names.contains_key(&field.name) {
                return Err(Diagnostic::new(
                    "E0307",
                    format!(
                        "field `{}` is already defined on `{}`",
                        field.name, item.name
                    ),
                    path,
                    1,
                    1,
                    1,
                    "",
                ));
            }
            field_names.insert(field.name.clone(), ());
            let value_type = parse_value_type_with_names(
                &field.type_ref,
                &known_structs,
                &known_enums,
                &item.type_params,
            )
            .ok_or_else(|| {
                unsupported_type_diagnostic(
                    path,
                    &synthetic_span(),
                    &field.type_ref,
                    format!(
                        "unsupported field type `{}` in v0.1 current implementation",
                        field.type_ref.path.join(".")
                    ),
                    &known_structs,
                    &known_enums,
                )
            })?;
            ensure_supported_value_type(path, &value_type, &synthetic_span())?;
            if value_type == ValueType::Void {
                return Err(Diagnostic::new(
                    "E0403",
                    "struct fields cannot have type `void`",
                    path,
                    1,
                    1,
                    1,
                    "",
                ));
            }
            fields.push(StructField {
                name: field.name.clone(),
                value_type,
            });
        }
        lowered.push(StructType {
            package: item.package.join("."),
            name: item.name.clone(),
            type_params: item.type_params.clone(),
            fields,
        });
    }

    Ok(lowered)
}

fn lower_enums(
    path: &Path,
    structs: &[StructType],
    enums: &[AstEnumDef],
    standard_type_needs: StandardTypeNeeds,
) -> Result<Vec<EnumType>, Diagnostic> {
    let mut lowered = Vec::new();
    let mut known = HashMap::new();
    for item in enums {
        if known.contains_key(&item.name) {
            return Err(Diagnostic::new(
                "E0313",
                format!("enum `{}` is already defined", item.name),
                path,
                1,
                1,
                1,
                "",
            ));
        }
        known.insert(item.name.clone(), ());
        let mut variants = Vec::new();
        let mut variant_names = HashMap::new();
        for variant in &item.variants {
            if variant_names.contains_key(&variant.name) {
                return Err(Diagnostic::new(
                    "E0314",
                    format!(
                        "variant `{}` is already defined on `{}`",
                        variant.name, item.name
                    ),
                    path,
                    1,
                    1,
                    1,
                    "",
                ));
            }
            variant_names.insert(variant.name.clone(), ());
            let payload = if let Some(type_ref) = &variant.payload {
                let type_name = type_ref.path.first().cloned().unwrap_or_default();
                let known_structs = structs
                    .iter()
                    .map(|item| (item.name.clone(), item.type_params.len()))
                    .chain(standard_struct_names(standard_type_needs))
                    .collect::<Vec<_>>();
                let known_enums = enums
                    .iter()
                    .map(|item| (item.name.clone(), item.type_params.len()))
                    .chain(standard_enum_names(standard_type_needs))
                    .collect::<Vec<_>>();
                let payload_type = parse_value_type_with_names(
                    type_ref,
                    &known_structs,
                    &known_enums,
                    &item.type_params,
                )
                .ok_or_else(|| {
                    unsupported_type_diagnostic(
                        path,
                        &synthetic_span(),
                        type_ref,
                        format!(
                            "unsupported enum payload type `{}` in v0.1 current implementation",
                            type_ref.path.join(".")
                        ),
                        &known_structs,
                        &known_enums,
                    )
                })?;
                ensure_supported_value_type(path, &payload_type, &synthetic_span())?;
                if payload_type == ValueType::Void {
                    return Err(Diagnostic::new(
                        "E0403",
                        format!("enum variant `{}` cannot carry `void`", type_name),
                        path,
                        1,
                        1,
                        1,
                        "",
                    ));
                }
                Some(payload_type)
            } else {
                None
            };
            variants.push(EnumVariantType {
                name: variant.name.clone(),
                payload,
            });
        }
        lowered.push(EnumType {
            package: item.package.join("."),
            name: item.name.clone(),
            type_params: item.type_params.clone(),
            variants,
        });
    }
    Ok(lowered)
}

#[derive(Debug, Clone, Copy)]
struct StandardTypeNeeds {
    io: bool,
    fs: bool,
    env: bool,
    process: bool,
    hash: bool,
    json: bool,
    regex: bool,
    collections: bool,
    time: bool,
    num: bool,
    result: bool,
    option: bool,
    array: bool,
}

fn standard_type_needs(imports: &[String], ast: &SourceFile) -> StandardTypeNeeds {
    StandardTypeNeeds {
        io: imports.iter().any(|item| item == "std.io.read_line")
            || (imports.iter().any(|item| item == "std.io") && source_uses_io_read_line(ast)),
        fs: imports
            .iter()
            .any(|item| item == "std.fs" || item.starts_with("std.fs."))
            || source_uses_fs_builtin(ast),
        env: imports
            .iter()
            .any(|item| item == "std.env" || item.starts_with("std.env."))
            || source_uses_env_builtin(ast),
        process: imports
            .iter()
            .any(|item| item == "std.process" || item.starts_with("std.process."))
            || source_uses_process_builtin(ast),
        hash: imports
            .iter()
            .any(|item| item == "std.hash" || item.starts_with("std.hash."))
            || source_uses_hash_builtin(ast),
        json: imports
            .iter()
            .any(|item| item == "std.json" || item.starts_with("std.json."))
            || source_uses_json_builtin(ast),
        regex: imports
            .iter()
            .any(|item| item == "std.regex" || item.starts_with("std.regex."))
            || source_uses_regex_builtin(ast),
        collections: imports
            .iter()
            .any(|item| item == "std.collections" || item.starts_with("std.collections.")),
        time: imports
            .iter()
            .any(|item| item == "std.time" || item.starts_with("std.time."))
            || source_uses_time_builtin(ast),
        num: imports
            .iter()
            .any(|item| item == "std.num" || item.starts_with("std.num."))
            || source_uses_num_builtin(ast),
        result: imports
            .iter()
            .any(|item| item == "std.result" || item.starts_with("std.result."))
            || source_uses_result_prelude_variant(ast),
        option: imports
            .iter()
            .any(|item| item == "std.option" || item == "std.option.Option")
            || source_uses_option_prelude_variant(ast),
        // std.collections/std.regex are backed by Array<string> and Option in v0.1.
        array: imports.iter().any(|item| {
            item == "std.array" || item == "std.array.Array" || item.starts_with("std.array.")
        }) || source_uses_array_builtin(ast)
            || imports.iter().any(|item| {
                item == "std.collections"
                    || item.starts_with("std.collections.")
                    || item == "std.regex"
                    || item.starts_with("std.regex.")
            }),
    }
}

fn standard_struct_names(needs: StandardTypeNeeds) -> impl Iterator<Item = (String, usize)> {
    let mut names = Vec::new();
    if needs.io {
        names.push(("IoError".to_string(), 0));
    }
    if needs.fs {
        names.push(("FsError".to_string(), 0));
        names.push(("File".to_string(), 0));
        names.push(("FileMetadata".to_string(), 0));
    }
    if needs.num {
        names.push(("NumError".to_string(), 0));
    }
    if needs.process {
        names.push(("ProcessError".to_string(), 0));
        names.push(("ProcessOutput".to_string(), 0));
    }
    if needs.hash {
        names.push(("HashState".to_string(), 0));
    }
    if needs.json {
        names.push(("JsonValue".to_string(), 0));
        names.push(("JsonError".to_string(), 0));
    }
    if needs.regex {
        names.push(("Regex".to_string(), 0));
        names.push(("RegexError".to_string(), 0));
    }
    if needs.collections {
        names.push(("StringMap".to_string(), 0));
        names.push(("StringSet".to_string(), 0));
    }
    if needs.time {
        names.push(("Duration".to_string(), 0));
    }
    names.into_iter()
}

fn standard_enum_names(needs: StandardTypeNeeds) -> impl Iterator<Item = (String, usize)> {
    let mut names = Vec::new();
    if needs.io
        || needs.fs
        || needs.num
        || needs.process
        || needs.json
        || needs.regex
        || needs.result
    {
        names.push(("Result".to_string(), 2));
    }
    if needs.env || needs.num || needs.option || needs.array || needs.collections || needs.regex {
        names.push(("Option".to_string(), 1));
    }
    names.into_iter()
}

fn inject_standard_types(
    needs: StandardTypeNeeds,
    structs: &mut Vec<StructType>,
    enums: &mut Vec<EnumType>,
) {
    if needs.io && !structs.iter().any(|item| item.name == "IoError") {
        structs.push(StructType {
            package: "std.io".to_string(),
            name: "IoError".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "message".to_string(),
                value_type: ValueType::String,
            }],
        });
    }
    if needs.fs && !structs.iter().any(|item| item.name == "FsError") {
        structs.push(StructType {
            package: "std.fs".to_string(),
            name: "FsError".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "message".to_string(),
                value_type: ValueType::String,
            }],
        });
    }
    if needs.fs && !structs.iter().any(|item| item.name == "File") {
        structs.push(StructType {
            package: "std.fs".to_string(),
            name: "File".to_string(),
            type_params: Vec::new(),
            fields: Vec::new(),
        });
    }
    if needs.fs && !structs.iter().any(|item| item.name == "FileMetadata") {
        structs.push(StructType {
            package: "std.fs".to_string(),
            name: "FileMetadata".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "is_file".to_string(),
                    value_type: ValueType::Bool,
                },
                StructField {
                    name: "is_dir".to_string(),
                    value_type: ValueType::Bool,
                },
                StructField {
                    name: "size".to_string(),
                    value_type: ValueType::U64,
                },
            ],
        });
    }
    if needs.num && !structs.iter().any(|item| item.name == "NumError") {
        structs.push(StructType {
            package: "std.num".to_string(),
            name: "NumError".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "message".to_string(),
                value_type: ValueType::String,
            }],
        });
    }
    if needs.process && !structs.iter().any(|item| item.name == "ProcessError") {
        structs.push(StructType {
            package: "std.process".to_string(),
            name: "ProcessError".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "message".to_string(),
                value_type: ValueType::String,
            }],
        });
    }
    if needs.process && !structs.iter().any(|item| item.name == "ProcessOutput") {
        structs.push(StructType {
            package: "std.process".to_string(),
            name: "ProcessOutput".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "status".to_string(),
                    value_type: ValueType::I32,
                },
                StructField {
                    name: "stdout".to_string(),
                    value_type: ValueType::String,
                },
                StructField {
                    name: "stderr".to_string(),
                    value_type: ValueType::String,
                },
            ],
        });
    }
    if needs.hash && !structs.iter().any(|item| item.name == "HashState") {
        structs.push(StructType {
            package: "std.hash".to_string(),
            name: "HashState".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "value".to_string(),
                value_type: ValueType::U64,
            }],
        });
    }
    if needs.json && !structs.iter().any(|item| item.name == "JsonValue") {
        structs.push(StructType {
            package: "std.json".to_string(),
            name: "JsonValue".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "raw".to_string(),
                value_type: ValueType::String,
            }],
        });
    }
    if needs.json && !structs.iter().any(|item| item.name == "JsonError") {
        structs.push(StructType {
            package: "std.json".to_string(),
            name: "JsonError".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "message".to_string(),
                value_type: ValueType::String,
            }],
        });
    }
    if needs.regex && !structs.iter().any(|item| item.name == "Regex") {
        structs.push(StructType {
            package: "std.regex".to_string(),
            name: "Regex".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "pattern".to_string(),
                value_type: ValueType::String,
            }],
        });
    }
    if needs.regex && !structs.iter().any(|item| item.name == "RegexError") {
        structs.push(StructType {
            package: "std.regex".to_string(),
            name: "RegexError".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "message".to_string(),
                value_type: ValueType::String,
            }],
        });
    }
    if needs.collections && !structs.iter().any(|item| item.name == "StringMap") {
        structs.push(StructType {
            package: "std.collections".to_string(),
            name: "StringMap".to_string(),
            type_params: Vec::new(),
            fields: vec![
                StructField {
                    name: "keys".to_string(),
                    value_type: ValueType::Array(Box::new(ValueType::String)),
                },
                StructField {
                    name: "values".to_string(),
                    value_type: ValueType::Array(Box::new(ValueType::String)),
                },
            ],
        });
    }
    if needs.collections && !structs.iter().any(|item| item.name == "StringSet") {
        structs.push(StructType {
            package: "std.collections".to_string(),
            name: "StringSet".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "values".to_string(),
                value_type: ValueType::Array(Box::new(ValueType::String)),
            }],
        });
    }
    if needs.time && !structs.iter().any(|item| item.name == "Duration") {
        structs.push(StructType {
            package: "std.time".to_string(),
            name: "Duration".to_string(),
            type_params: Vec::new(),
            fields: vec![StructField {
                name: "millis".to_string(),
                value_type: ValueType::Int,
            }],
        });
    }
    if (needs.io || needs.fs || needs.num || needs.json || needs.regex || needs.result)
        && !enums.iter().any(|item| item.name == "Result")
    {
        enums.push(EnumType {
            package: "std.result".to_string(),
            name: "Result".to_string(),
            type_params: vec!["T".to_string(), "E".to_string()],
            variants: vec![
                EnumVariantType {
                    name: "Ok".to_string(),
                    payload: Some(ValueType::TypeParam("T".to_string())),
                },
                EnumVariantType {
                    name: "Err".to_string(),
                    payload: Some(ValueType::TypeParam("E".to_string())),
                },
            ],
        });
    }
    if (needs.env || needs.num || needs.option || needs.array || needs.collections || needs.regex)
        && !enums.iter().any(|item| item.name == "Option")
    {
        enums.push(EnumType {
            package: "std.option".to_string(),
            name: "Option".to_string(),
            type_params: vec!["T".to_string()],
            variants: vec![
                EnumVariantType {
                    name: "Some".to_string(),
                    payload: Some(ValueType::TypeParam("T".to_string())),
                },
                EnumVariantType {
                    name: "None".to_string(),
                    payload: None,
                },
            ],
        });
    }
}

fn source_uses_fs_builtin(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_fs_builtin)
}

fn source_uses_io_read_line(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_io_read_line)
}

fn source_uses_env_builtin(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_env_builtin)
}

fn source_uses_process_builtin(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_process_builtin)
}

fn source_uses_hash_builtin(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_hash_builtin)
}

fn source_uses_json_builtin(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_json_builtin)
}

fn source_uses_regex_builtin(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_regex_builtin)
}

fn source_uses_num_builtin(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_num_builtin)
}

fn source_uses_time_builtin(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_time_builtin)
}

fn source_uses_array_builtin(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_array_builtin)
}

fn source_uses_result_prelude_variant(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_result_prelude_variant)
        || ast
            .consts
            .iter()
            .any(|const_def| expr_uses_result_prelude_variant(&const_def.value))
}

fn source_uses_option_prelude_variant(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_option_prelude_variant)
        || ast
            .consts
            .iter()
            .any(|const_def| expr_uses_option_prelude_variant(&const_def.value))
}

fn ast_functions(ast: &SourceFile) -> impl Iterator<Item = &AstFunction> {
    ast.functions
        .iter()
        .chain(ast.impls.iter().flat_map(|item| item.methods.iter()))
}

fn collect_generic_function_instances(
    path: &Path,
    ast: &SourceFile,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Result<Vec<FunctionInstance>, Diagnostic> {
    let mut out = Vec::new();
    for function in ast_functions(ast) {
        for stmt in &function.body {
            collect_stmt_generic_function_instances(
                path, stmt, imports, signatures, structs, enums, &mut out,
            )?;
        }
    }
    Ok(out)
}

fn collect_stmt_generic_function_instances(
    path: &Path,
    stmt: &Stmt,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    out: &mut Vec<FunctionInstance>,
) -> Result<(), Diagnostic> {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => {
            collect_expr_generic_function_instances(
                path, value, imports, signatures, structs, enums, out,
            )
        }
        Stmt::LetElse {
            value, else_body, ..
        } => {
            collect_expr_generic_function_instances(
                path, value, imports, signatures, structs, enums, out,
            )?;
            for stmt in else_body {
                collect_stmt_generic_function_instances(
                    path, stmt, imports, signatures, structs, enums, out,
                )?;
            }
            Ok(())
        }
        Stmt::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            collect_expr_generic_function_instances(
                path, value, imports, signatures, structs, enums, out,
            )?;
            for stmt in body {
                collect_stmt_generic_function_instances(
                    path, stmt, imports, signatures, structs, enums, out,
                )?;
            }
            if let Some(else_body) = else_body {
                for stmt in else_body {
                    collect_stmt_generic_function_instances(
                        path, stmt, imports, signatures, structs, enums, out,
                    )?;
                }
            }
            Ok(())
        }
        Stmt::Return { value, .. } => {
            if let Some(value) = value {
                collect_expr_generic_function_instances(
                    path, value, imports, signatures, structs, enums, out,
                )?;
            }
            Ok(())
        }
        Stmt::Expr { expr, .. } => collect_expr_generic_function_instances(
            path, expr, imports, signatures, structs, enums, out,
        ),
        Stmt::Match { value, arms, .. } => {
            collect_expr_generic_function_instances(
                path, value, imports, signatures, structs, enums, out,
            )?;
            for arm in arms {
                for stmt in &arm.body {
                    collect_stmt_generic_function_instances(
                        path, stmt, imports, signatures, structs, enums, out,
                    )?;
                }
            }
            Ok(())
        }
        Stmt::For { variant, .. } => match variant {
            ForVariant::Infinite { body } => {
                for stmt in body {
                    collect_stmt_generic_function_instances(
                        path, stmt, imports, signatures, structs, enums, out,
                    )?;
                }
                Ok(())
            }
            ForVariant::While { condition, body } => {
                collect_expr_generic_function_instances(
                    path, condition, imports, signatures, structs, enums, out,
                )?;
                for stmt in body {
                    collect_stmt_generic_function_instances(
                        path, stmt, imports, signatures, structs, enums, out,
                    )?;
                }
                Ok(())
            }
            ForVariant::Iterate { iterable, body, .. } => {
                collect_expr_generic_function_instances(
                    path, iterable, imports, signatures, structs, enums, out,
                )?;
                for stmt in body {
                    collect_stmt_generic_function_instances(
                        path, stmt, imports, signatures, structs, enums, out,
                    )?;
                }
                Ok(())
            }
        },
        Stmt::Defer { stmt, .. } => collect_stmt_generic_function_instances(
            path, stmt, imports, signatures, structs, enums, out,
        ),
        Stmt::Postfix { .. } => Ok(()),
        Stmt::Break { .. } | Stmt::Continue { .. } => Ok(()),
    }
}

fn collect_expr_generic_function_instances(
    path: &Path,
    expr: &AstExpr,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    out: &mut Vec<FunctionInstance>,
) -> Result<(), Diagnostic> {
    match expr {
        AstExpr::Call {
            callee,
            type_args,
            args,
        } => {
            if callee.len() == 1 && !type_args.is_empty() {
                let name = &callee[0];
                if resolve_specific_value_builtin(name, imports).is_some() {
                    for arg in args {
                        collect_expr_generic_function_instances(
                            path, arg, imports, signatures, structs, enums, out,
                        )?;
                    }
                    return Ok(());
                }
                if core_prelude_variant(name).is_some() {
                    for arg in args {
                        collect_expr_generic_function_instances(
                            path, arg, imports, signatures, structs, enums, out,
                        )?;
                    }
                    return Ok(());
                }
                let signature = signatures.get(name).ok_or_else(|| {
                    Diagnostic::new(
                        "E0305",
                        format!("unknown function `{name}`"),
                        path,
                        1,
                        1,
                        1,
                        "",
                    )
                })?;
                if signature.type_params.is_empty() {
                    return Err(Diagnostic::new(
                        "E0404",
                        format!("function `{name}` does not accept type arguments"),
                        path,
                        1,
                        1,
                        1,
                        "",
                    ));
                }
                if type_args.len() != signature.type_params.len() {
                    return Err(Diagnostic::new(
                        "E0407",
                        format!(
                            "function `{name}` expects {} type argument(s), got {}",
                            signature.type_params.len(),
                            type_args.len()
                        ),
                        path,
                        1,
                        1,
                        1,
                        "",
                    ));
                }
                let args = type_args
                    .iter()
                    .map(|arg| parse_non_void_type(arg, structs, enums))
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| {
                        let type_arg = type_args
                            .iter()
                            .find(|arg| parse_non_void_type(arg, structs, enums).is_none())
                            .expect("at least one type argument failed to lower");
                        unsupported_type_diagnostic_from_maps(
                            path,
                            &synthetic_span(),
                            type_arg,
                            format!("unsupported type argument for `{name}`"),
                            structs,
                            enums,
                        )
                    })?;
                let instance = FunctionInstance {
                    name: name.clone(),
                    args,
                };
                if !out.contains(&instance) {
                    out.push(instance);
                }
            }
            for arg in args {
                collect_expr_generic_function_instances(
                    path, arg, imports, signatures, structs, enums, out,
                )?;
            }
            Ok(())
        }
        AstExpr::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                collect_expr_generic_function_instances(
                    path, value, imports, signatures, structs, enums, out,
                )?;
            }
            Ok(())
        }
        AstExpr::Match { value, arms } => {
            collect_expr_generic_function_instances(
                path, value, imports, signatures, structs, enums, out,
            )?;
            for arm in arms {
                collect_expr_generic_function_instances(
                    path, &arm.value, imports, signatures, structs, enums, out,
                )?;
            }
            Ok(())
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_expr_generic_function_instances(
                path, condition, imports, signatures, structs, enums, out,
            )?;
            collect_expr_generic_function_instances(
                path,
                then_branch,
                imports,
                signatures,
                structs,
                enums,
                out,
            )?;
            collect_expr_generic_function_instances(
                path,
                else_branch,
                imports,
                signatures,
                structs,
                enums,
                out,
            )
        }
        AstExpr::Panic { message }
        | AstExpr::Question { expr: message }
        | AstExpr::Unary { expr: message, .. } => collect_expr_generic_function_instances(
            path, message, imports, signatures, structs, enums, out,
        ),
        AstExpr::Cast { expr, .. } => collect_expr_generic_function_instances(
            path, expr, imports, signatures, structs, enums, out,
        ),
        AstExpr::Binary { left, right, .. } => {
            collect_expr_generic_function_instances(
                path, left, imports, signatures, structs, enums, out,
            )?;
            collect_expr_generic_function_instances(
                path, right, imports, signatures, structs, enums, out,
            )
        }
        AstExpr::MutArg { .. }
        | AstExpr::Name(_)
        | AstExpr::String(_)
        | AstExpr::Int(_)
        | AstExpr::Float(_)
        | AstExpr::Char(_)
        | AstExpr::Bool(_)
        | AstExpr::Void => Ok(()),
    }
}

fn stmt_uses_fs_builtin(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => expr_uses_fs_builtin(value),
        Stmt::LetElse {
            value, else_body, ..
        } => expr_uses_fs_builtin(value) || else_body.iter().any(stmt_uses_fs_builtin),
        Stmt::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_fs_builtin(value)
                || body.iter().any(stmt_uses_fs_builtin)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(stmt_uses_fs_builtin))
        }
        Stmt::Return { value, .. } => value.as_ref().is_some_and(expr_uses_fs_builtin),
        Stmt::Expr { expr, .. } => expr_uses_fs_builtin(expr),
        Stmt::Match { value, arms, .. } => {
            expr_uses_fs_builtin(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(stmt_uses_fs_builtin))
        }
        Stmt::For { variant, .. } => match variant {
            ForVariant::Infinite { body } => body.iter().any(stmt_uses_fs_builtin),
            ForVariant::While { condition, body } => {
                expr_uses_fs_builtin(condition) || body.iter().any(stmt_uses_fs_builtin)
            }
            ForVariant::Iterate { iterable, body, .. } => {
                expr_uses_fs_builtin(iterable) || body.iter().any(stmt_uses_fs_builtin)
            }
        },
        Stmt::Defer { stmt, .. } => stmt_uses_fs_builtin(stmt),
        Stmt::Postfix { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => false,
    }
}

fn stmt_uses_io_read_line(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => expr_uses_io_read_line(value),
        Stmt::LetElse {
            value, else_body, ..
        } => expr_uses_io_read_line(value) || else_body.iter().any(stmt_uses_io_read_line),
        Stmt::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_io_read_line(value)
                || body.iter().any(stmt_uses_io_read_line)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(stmt_uses_io_read_line))
        }
        Stmt::Return { value, .. } => value.as_ref().is_some_and(expr_uses_io_read_line),
        Stmt::Expr { expr, .. } => expr_uses_io_read_line(expr),
        Stmt::Match { value, arms, .. } => {
            expr_uses_io_read_line(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(stmt_uses_io_read_line))
        }
        Stmt::For { variant, .. } => match variant {
            ForVariant::Infinite { body } => body.iter().any(stmt_uses_io_read_line),
            ForVariant::While { condition, body } => {
                expr_uses_io_read_line(condition) || body.iter().any(stmt_uses_io_read_line)
            }
            ForVariant::Iterate { iterable, body, .. } => {
                expr_uses_io_read_line(iterable) || body.iter().any(stmt_uses_io_read_line)
            }
        },
        Stmt::Defer { stmt, .. } => stmt_uses_io_read_line(stmt),
        Stmt::Postfix { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => false,
    }
}

fn stmt_uses_env_builtin(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => expr_uses_env_builtin(value),
        Stmt::LetElse {
            value, else_body, ..
        } => expr_uses_env_builtin(value) || else_body.iter().any(stmt_uses_env_builtin),
        Stmt::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_env_builtin(value)
                || body.iter().any(stmt_uses_env_builtin)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(stmt_uses_env_builtin))
        }
        Stmt::Return { value, .. } => value.as_ref().is_some_and(expr_uses_env_builtin),
        Stmt::Expr { expr, .. } => expr_uses_env_builtin(expr),
        Stmt::Match { value, arms, .. } => {
            expr_uses_env_builtin(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(stmt_uses_env_builtin))
        }
        Stmt::For { variant, .. } => match variant {
            ForVariant::Infinite { body } => body.iter().any(stmt_uses_env_builtin),
            ForVariant::While { condition, body } => {
                expr_uses_env_builtin(condition) || body.iter().any(stmt_uses_env_builtin)
            }
            ForVariant::Iterate { iterable, body, .. } => {
                expr_uses_env_builtin(iterable) || body.iter().any(stmt_uses_env_builtin)
            }
        },
        Stmt::Defer { stmt, .. } => stmt_uses_env_builtin(stmt),
        Stmt::Postfix { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => false,
    }
}

fn stmt_uses_process_builtin(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => expr_uses_process_builtin(value),
        Stmt::LetElse {
            value, else_body, ..
        } => expr_uses_process_builtin(value) || else_body.iter().any(stmt_uses_process_builtin),
        Stmt::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_process_builtin(value)
                || body.iter().any(stmt_uses_process_builtin)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(stmt_uses_process_builtin))
        }
        Stmt::Return { value, .. } => value.as_ref().is_some_and(expr_uses_process_builtin),
        Stmt::Expr { expr, .. } => expr_uses_process_builtin(expr),
        Stmt::Match { value, arms, .. } => {
            expr_uses_process_builtin(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(stmt_uses_process_builtin))
        }
        Stmt::For { variant, .. } => match variant {
            ForVariant::Infinite { body } => body.iter().any(stmt_uses_process_builtin),
            ForVariant::While { condition, body } => {
                expr_uses_process_builtin(condition) || body.iter().any(stmt_uses_process_builtin)
            }
            ForVariant::Iterate { iterable, body, .. } => {
                expr_uses_process_builtin(iterable) || body.iter().any(stmt_uses_process_builtin)
            }
        },
        Stmt::Defer { stmt, .. } => stmt_uses_process_builtin(stmt),
        Stmt::Postfix { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => false,
    }
}

fn stmt_uses_hash_builtin(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => expr_uses_hash_builtin(value),
        Stmt::LetElse {
            value, else_body, ..
        } => expr_uses_hash_builtin(value) || else_body.iter().any(stmt_uses_hash_builtin),
        Stmt::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_hash_builtin(value)
                || body.iter().any(stmt_uses_hash_builtin)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(stmt_uses_hash_builtin))
        }
        Stmt::Return { value, .. } => value.as_ref().is_some_and(expr_uses_hash_builtin),
        Stmt::Expr { expr, .. } => expr_uses_hash_builtin(expr),
        Stmt::Match { value, arms, .. } => {
            expr_uses_hash_builtin(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(stmt_uses_hash_builtin))
        }
        Stmt::For { variant, .. } => match variant {
            ForVariant::Infinite { body } => body.iter().any(stmt_uses_hash_builtin),
            ForVariant::While { condition, body } => {
                expr_uses_hash_builtin(condition) || body.iter().any(stmt_uses_hash_builtin)
            }
            ForVariant::Iterate { iterable, body, .. } => {
                expr_uses_hash_builtin(iterable) || body.iter().any(stmt_uses_hash_builtin)
            }
        },
        Stmt::Defer { stmt, .. } => stmt_uses_hash_builtin(stmt),
        Stmt::Postfix { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => false,
    }
}

fn stmt_uses_json_builtin(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => expr_uses_json_builtin(value),
        Stmt::LetElse {
            value, else_body, ..
        } => expr_uses_json_builtin(value) || else_body.iter().any(stmt_uses_json_builtin),
        Stmt::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_json_builtin(value)
                || body.iter().any(stmt_uses_json_builtin)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(stmt_uses_json_builtin))
        }
        Stmt::Return { value, .. } => value.as_ref().is_some_and(expr_uses_json_builtin),
        Stmt::Expr { expr, .. } => expr_uses_json_builtin(expr),
        Stmt::Match { value, arms, .. } => {
            expr_uses_json_builtin(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(stmt_uses_json_builtin))
        }
        Stmt::For { variant, .. } => match variant {
            ForVariant::Infinite { body } => body.iter().any(stmt_uses_json_builtin),
            ForVariant::While { condition, body } => {
                expr_uses_json_builtin(condition) || body.iter().any(stmt_uses_json_builtin)
            }
            ForVariant::Iterate { iterable, body, .. } => {
                expr_uses_json_builtin(iterable) || body.iter().any(stmt_uses_json_builtin)
            }
        },
        Stmt::Defer { stmt, .. } => stmt_uses_json_builtin(stmt),
        Stmt::Postfix { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => false,
    }
}

fn stmt_uses_regex_builtin(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => expr_uses_regex_builtin(value),
        Stmt::LetElse {
            value, else_body, ..
        } => expr_uses_regex_builtin(value) || else_body.iter().any(stmt_uses_regex_builtin),
        Stmt::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_regex_builtin(value)
                || body.iter().any(stmt_uses_regex_builtin)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(stmt_uses_regex_builtin))
        }
        Stmt::Return { value, .. } => value.as_ref().is_some_and(expr_uses_regex_builtin),
        Stmt::Expr { expr, .. } => expr_uses_regex_builtin(expr),
        Stmt::Match { value, arms, .. } => {
            expr_uses_regex_builtin(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(stmt_uses_regex_builtin))
        }
        Stmt::For { variant, .. } => match variant {
            ForVariant::Infinite { body } => body.iter().any(stmt_uses_regex_builtin),
            ForVariant::While { condition, body } => {
                expr_uses_regex_builtin(condition) || body.iter().any(stmt_uses_regex_builtin)
            }
            ForVariant::Iterate { iterable, body, .. } => {
                expr_uses_regex_builtin(iterable) || body.iter().any(stmt_uses_regex_builtin)
            }
        },
        Stmt::Defer { stmt, .. } => stmt_uses_regex_builtin(stmt),
        Stmt::Postfix { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => false,
    }
}

fn stmt_uses_num_builtin(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => expr_uses_num_builtin(value),
        Stmt::LetElse {
            value, else_body, ..
        } => expr_uses_num_builtin(value) || else_body.iter().any(stmt_uses_num_builtin),
        Stmt::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_num_builtin(value)
                || body.iter().any(stmt_uses_num_builtin)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(stmt_uses_num_builtin))
        }
        Stmt::Return { value, .. } => value.as_ref().is_some_and(expr_uses_num_builtin),
        Stmt::Expr { expr, .. } => expr_uses_num_builtin(expr),
        Stmt::Match { value, arms, .. } => {
            expr_uses_num_builtin(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(stmt_uses_num_builtin))
        }
        Stmt::For { variant, .. } => match variant {
            ForVariant::Infinite { body } => body.iter().any(stmt_uses_num_builtin),
            ForVariant::While { condition, body } => {
                expr_uses_num_builtin(condition) || body.iter().any(stmt_uses_num_builtin)
            }
            ForVariant::Iterate { iterable, body, .. } => {
                expr_uses_num_builtin(iterable) || body.iter().any(stmt_uses_num_builtin)
            }
        },
        Stmt::Defer { stmt, .. } => stmt_uses_num_builtin(stmt),
        Stmt::Postfix { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => false,
    }
}

fn stmt_uses_time_builtin(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => expr_uses_time_builtin(value),
        Stmt::LetElse {
            value, else_body, ..
        } => expr_uses_time_builtin(value) || else_body.iter().any(stmt_uses_time_builtin),
        Stmt::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_time_builtin(value)
                || body.iter().any(stmt_uses_time_builtin)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(stmt_uses_time_builtin))
        }
        Stmt::Return { value, .. } => value.as_ref().is_some_and(expr_uses_time_builtin),
        Stmt::Expr { expr, .. } => expr_uses_time_builtin(expr),
        Stmt::Match { value, arms, .. } => {
            expr_uses_time_builtin(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(stmt_uses_time_builtin))
        }
        Stmt::For { variant, .. } => match variant {
            ForVariant::Infinite { body } => body.iter().any(stmt_uses_time_builtin),
            ForVariant::While { condition, body } => {
                expr_uses_time_builtin(condition) || body.iter().any(stmt_uses_time_builtin)
            }
            ForVariant::Iterate { iterable, body, .. } => {
                expr_uses_time_builtin(iterable) || body.iter().any(stmt_uses_time_builtin)
            }
        },
        Stmt::Defer { stmt, .. } => stmt_uses_time_builtin(stmt),
        Stmt::Postfix { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => false,
    }
}

fn stmt_uses_array_builtin(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => expr_uses_array_builtin(value),
        Stmt::LetElse {
            value, else_body, ..
        } => expr_uses_array_builtin(value) || else_body.iter().any(stmt_uses_array_builtin),
        Stmt::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses_array_builtin(value)
                || body.iter().any(stmt_uses_array_builtin)
                || else_body
                    .as_ref()
                    .is_some_and(|else_body| else_body.iter().any(stmt_uses_array_builtin))
        }
        Stmt::Return { value, .. } => value.as_ref().is_some_and(expr_uses_array_builtin),
        Stmt::Expr { expr, .. } => expr_uses_array_builtin(expr),
        Stmt::Match { value, arms, .. } => {
            expr_uses_array_builtin(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(stmt_uses_array_builtin))
        }
        Stmt::For { variant, .. } => match variant {
            ForVariant::Infinite { body } => body.iter().any(stmt_uses_array_builtin),
            ForVariant::While { condition, body } => {
                expr_uses_array_builtin(condition) || body.iter().any(stmt_uses_array_builtin)
            }
            ForVariant::Iterate { iterable, body, .. } => {
                expr_uses_array_builtin(iterable) || body.iter().any(stmt_uses_array_builtin)
            }
        },
        Stmt::Defer { stmt, .. } => stmt_uses_array_builtin(stmt),
        Stmt::Postfix { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => false,
    }
}

fn expr_uses_fs_builtin(expr: &AstExpr) -> bool {
    match expr {
        AstExpr::Call { callee, args, .. } => {
            (callee == &["fs", "read_to_string"]
                || callee == &["fs", "write_string"]
                || callee == &["fs", "read_bytes"]
                || callee == &["fs", "write_bytes"]
                || callee == &["fs", "exists"]
                || callee == &["fs", "metadata"]
                || callee == &["fs", "create_dir"]
                || callee == &["fs", "remove_dir"]
                || callee == &["fs", "read_dir"]
                || callee == &["fs", "open"])
                || args.iter().any(expr_uses_fs_builtin)
        }
        AstExpr::StructLiteral { fields, .. } => {
            fields.iter().any(|(_, value)| expr_uses_fs_builtin(value))
        }
        AstExpr::Match { value, arms } => {
            expr_uses_fs_builtin(value) || arms.iter().any(|arm| expr_uses_fs_builtin(&arm.value))
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_fs_builtin(condition)
                || expr_uses_fs_builtin(then_branch)
                || expr_uses_fs_builtin(else_branch)
        }
        AstExpr::Panic { message }
        | AstExpr::Question { expr: message }
        | AstExpr::Unary { expr: message, .. } => expr_uses_fs_builtin(message),
        AstExpr::MutArg { .. } => false,
        AstExpr::Cast { expr, .. } => expr_uses_fs_builtin(expr),
        AstExpr::Binary { left, right, .. } => {
            expr_uses_fs_builtin(left) || expr_uses_fs_builtin(right)
        }
        AstExpr::Name(_)
        | AstExpr::String(_)
        | AstExpr::Int(_)
        | AstExpr::Float(_)
        | AstExpr::Char(_)
        | AstExpr::Bool(_)
        | AstExpr::Void => false,
    }
}

fn expr_uses_io_read_line(expr: &AstExpr) -> bool {
    match expr {
        AstExpr::Call { callee, args, .. } => {
            callee == &["io", "read_line"] || args.iter().any(expr_uses_io_read_line)
        }
        AstExpr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_uses_io_read_line(value)),
        AstExpr::Match { value, arms } => {
            expr_uses_io_read_line(value)
                || arms.iter().any(|arm| expr_uses_io_read_line(&arm.value))
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_io_read_line(condition)
                || expr_uses_io_read_line(then_branch)
                || expr_uses_io_read_line(else_branch)
        }
        AstExpr::Panic { message }
        | AstExpr::Question { expr: message }
        | AstExpr::Unary { expr: message, .. } => expr_uses_io_read_line(message),
        AstExpr::Cast { expr, .. } => expr_uses_io_read_line(expr),
        AstExpr::Binary { left, right, .. } => {
            expr_uses_io_read_line(left) || expr_uses_io_read_line(right)
        }
        AstExpr::MutArg { .. }
        | AstExpr::Name(_)
        | AstExpr::String(_)
        | AstExpr::Int(_)
        | AstExpr::Float(_)
        | AstExpr::Char(_)
        | AstExpr::Bool(_)
        | AstExpr::Void => false,
    }
}

fn expr_uses_env_builtin(expr: &AstExpr) -> bool {
    match expr {
        AstExpr::Call { callee, args, .. } => {
            is_env_builtin_call(callee) || args.iter().any(expr_uses_env_builtin)
        }
        AstExpr::StructLiteral { fields, .. } => {
            fields.iter().any(|(_, value)| expr_uses_env_builtin(value))
        }
        AstExpr::Match { value, arms } => {
            expr_uses_env_builtin(value) || arms.iter().any(|arm| expr_uses_env_builtin(&arm.value))
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_env_builtin(condition)
                || expr_uses_env_builtin(then_branch)
                || expr_uses_env_builtin(else_branch)
        }
        AstExpr::Panic { message }
        | AstExpr::Question { expr: message }
        | AstExpr::Unary { expr: message, .. } => expr_uses_env_builtin(message),
        AstExpr::MutArg { .. } => false,
        AstExpr::Cast { expr, .. } => expr_uses_env_builtin(expr),
        AstExpr::Binary { left, right, .. } => {
            expr_uses_env_builtin(left) || expr_uses_env_builtin(right)
        }
        AstExpr::Name(_)
        | AstExpr::String(_)
        | AstExpr::Int(_)
        | AstExpr::Float(_)
        | AstExpr::Char(_)
        | AstExpr::Bool(_)
        | AstExpr::Void => false,
    }
}

fn expr_uses_process_builtin(expr: &AstExpr) -> bool {
    match expr {
        AstExpr::Call { callee, args, .. } => {
            is_process_builtin_call(callee) || args.iter().any(expr_uses_process_builtin)
        }
        AstExpr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_uses_process_builtin(value)),
        AstExpr::Match { value, arms } => {
            expr_uses_process_builtin(value)
                || arms.iter().any(|arm| expr_uses_process_builtin(&arm.value))
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_process_builtin(condition)
                || expr_uses_process_builtin(then_branch)
                || expr_uses_process_builtin(else_branch)
        }
        AstExpr::Panic { message }
        | AstExpr::Question { expr: message }
        | AstExpr::Unary { expr: message, .. } => expr_uses_process_builtin(message),
        AstExpr::MutArg { .. } => false,
        AstExpr::Cast { expr, .. } => expr_uses_process_builtin(expr),
        AstExpr::Binary { left, right, .. } => {
            expr_uses_process_builtin(left) || expr_uses_process_builtin(right)
        }
        AstExpr::Name(_)
        | AstExpr::String(_)
        | AstExpr::Int(_)
        | AstExpr::Float(_)
        | AstExpr::Char(_)
        | AstExpr::Bool(_)
        | AstExpr::Void => false,
    }
}

fn expr_uses_hash_builtin(expr: &AstExpr) -> bool {
    match expr {
        AstExpr::Call { callee, args, .. } => {
            is_hash_builtin_call(callee) || args.iter().any(expr_uses_hash_builtin)
        }
        AstExpr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_uses_hash_builtin(value)),
        AstExpr::Match { value, arms } => {
            expr_uses_hash_builtin(value)
                || arms.iter().any(|arm| expr_uses_hash_builtin(&arm.value))
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_hash_builtin(condition)
                || expr_uses_hash_builtin(then_branch)
                || expr_uses_hash_builtin(else_branch)
        }
        AstExpr::Panic { message }
        | AstExpr::Question { expr: message }
        | AstExpr::Unary { expr: message, .. } => expr_uses_hash_builtin(message),
        AstExpr::MutArg { .. } => false,
        AstExpr::Cast { expr, .. } => expr_uses_hash_builtin(expr),
        AstExpr::Binary { left, right, .. } => {
            expr_uses_hash_builtin(left) || expr_uses_hash_builtin(right)
        }
        AstExpr::Name(_)
        | AstExpr::String(_)
        | AstExpr::Int(_)
        | AstExpr::Float(_)
        | AstExpr::Char(_)
        | AstExpr::Bool(_)
        | AstExpr::Void => false,
    }
}

fn expr_uses_json_builtin(expr: &AstExpr) -> bool {
    match expr {
        AstExpr::Call { callee, args, .. } => {
            is_json_builtin_call(callee) || args.iter().any(expr_uses_json_builtin)
        }
        AstExpr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_uses_json_builtin(value)),
        AstExpr::Match { value, arms } => {
            expr_uses_json_builtin(value)
                || arms.iter().any(|arm| expr_uses_json_builtin(&arm.value))
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_json_builtin(condition)
                || expr_uses_json_builtin(then_branch)
                || expr_uses_json_builtin(else_branch)
        }
        AstExpr::Panic { message }
        | AstExpr::Question { expr: message }
        | AstExpr::Unary { expr: message, .. } => expr_uses_json_builtin(message),
        AstExpr::MutArg { .. } => false,
        AstExpr::Cast { expr, .. } => expr_uses_json_builtin(expr),
        AstExpr::Binary { left, right, .. } => {
            expr_uses_json_builtin(left) || expr_uses_json_builtin(right)
        }
        AstExpr::Name(_)
        | AstExpr::String(_)
        | AstExpr::Int(_)
        | AstExpr::Float(_)
        | AstExpr::Char(_)
        | AstExpr::Bool(_)
        | AstExpr::Void => false,
    }
}

fn expr_uses_regex_builtin(expr: &AstExpr) -> bool {
    match expr {
        AstExpr::Call { callee, args, .. } => {
            is_regex_builtin_call(callee) || args.iter().any(expr_uses_regex_builtin)
        }
        AstExpr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_uses_regex_builtin(value)),
        AstExpr::Match { value, arms } => {
            expr_uses_regex_builtin(value)
                || arms.iter().any(|arm| expr_uses_regex_builtin(&arm.value))
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_regex_builtin(condition)
                || expr_uses_regex_builtin(then_branch)
                || expr_uses_regex_builtin(else_branch)
        }
        AstExpr::Panic { message }
        | AstExpr::Question { expr: message }
        | AstExpr::Unary { expr: message, .. } => expr_uses_regex_builtin(message),
        AstExpr::MutArg { .. } => false,
        AstExpr::Cast { expr, .. } => expr_uses_regex_builtin(expr),
        AstExpr::Binary { left, right, .. } => {
            expr_uses_regex_builtin(left) || expr_uses_regex_builtin(right)
        }
        AstExpr::Name(_)
        | AstExpr::String(_)
        | AstExpr::Int(_)
        | AstExpr::Float(_)
        | AstExpr::Char(_)
        | AstExpr::Bool(_)
        | AstExpr::Void => false,
    }
}

fn expr_uses_num_builtin(expr: &AstExpr) -> bool {
    match expr {
        AstExpr::Call { callee, args, .. } => {
            (callee.len() == 2
                && callee[0] == "num"
                && matches!(
                    callee[1].as_str(),
                    "parse_i64"
                        | "parse_u64"
                        | "parse_f64"
                        | "to_string"
                        | "checked_add"
                        | "checked_sub"
                        | "checked_mul"
                        | "wrapping_add"
                        | "wrapping_sub"
                        | "wrapping_mul"
                ))
                || args.iter().any(expr_uses_num_builtin)
        }
        AstExpr::StructLiteral { fields, .. } => {
            fields.iter().any(|(_, value)| expr_uses_num_builtin(value))
        }
        AstExpr::Match { value, arms } => {
            expr_uses_num_builtin(value) || arms.iter().any(|arm| expr_uses_num_builtin(&arm.value))
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_num_builtin(condition)
                || expr_uses_num_builtin(then_branch)
                || expr_uses_num_builtin(else_branch)
        }
        AstExpr::Panic { message }
        | AstExpr::Question { expr: message }
        | AstExpr::Unary { expr: message, .. } => expr_uses_num_builtin(message),
        AstExpr::MutArg { .. } => false,
        AstExpr::Cast { expr, .. } => expr_uses_num_builtin(expr),
        AstExpr::Binary { left, right, .. } => {
            expr_uses_num_builtin(left) || expr_uses_num_builtin(right)
        }
        AstExpr::Name(_)
        | AstExpr::String(_)
        | AstExpr::Int(_)
        | AstExpr::Float(_)
        | AstExpr::Char(_)
        | AstExpr::Bool(_)
        | AstExpr::Void => false,
    }
}

fn expr_uses_time_builtin(expr: &AstExpr) -> bool {
    match expr {
        AstExpr::Call { callee, args, .. } => {
            (callee.len() == 2
                && callee[0] == "time"
                && matches!(
                    callee[1].as_str(),
                    "duration_millis"
                        | "duration_seconds"
                        | "duration_as_millis"
                        | "format_duration"
                        | "sleep"
                ))
                || args.iter().any(expr_uses_time_builtin)
        }
        AstExpr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_uses_time_builtin(value)),
        AstExpr::Match { value, arms } => {
            expr_uses_time_builtin(value)
                || arms.iter().any(|arm| expr_uses_time_builtin(&arm.value))
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_time_builtin(condition)
                || expr_uses_time_builtin(then_branch)
                || expr_uses_time_builtin(else_branch)
        }
        AstExpr::Panic { message }
        | AstExpr::Question { expr: message }
        | AstExpr::Unary { expr: message, .. } => expr_uses_time_builtin(message),
        AstExpr::MutArg { .. } => false,
        AstExpr::Cast { expr, .. } => expr_uses_time_builtin(expr),
        AstExpr::Binary { left, right, .. } => {
            expr_uses_time_builtin(left) || expr_uses_time_builtin(right)
        }
        AstExpr::Name(_)
        | AstExpr::String(_)
        | AstExpr::Int(_)
        | AstExpr::Float(_)
        | AstExpr::Char(_)
        | AstExpr::Bool(_)
        | AstExpr::Void => false,
    }
}

fn expr_uses_array_builtin(expr: &AstExpr) -> bool {
    match expr {
        AstExpr::Call { callee, args, .. } => {
            callee == &["Array", "new"]
                || (callee.len() == 2
                    && !is_known_std_value_module(&callee[0])
                    && matches!(callee[1].as_str(), "len" | "get" | "push" | "set"))
                || args.iter().any(expr_uses_array_builtin)
        }
        AstExpr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_uses_array_builtin(value)),
        AstExpr::Match { value, arms } => {
            expr_uses_array_builtin(value)
                || arms.iter().any(|arm| expr_uses_array_builtin(&arm.value))
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_array_builtin(condition)
                || expr_uses_array_builtin(then_branch)
                || expr_uses_array_builtin(else_branch)
        }
        AstExpr::Panic { message }
        | AstExpr::Question { expr: message }
        | AstExpr::Unary { expr: message, .. } => expr_uses_array_builtin(message),
        AstExpr::MutArg { .. } => false,
        AstExpr::Cast { expr, .. } => expr_uses_array_builtin(expr),
        AstExpr::Binary { left, right, .. } => {
            expr_uses_array_builtin(left) || expr_uses_array_builtin(right)
        }
        AstExpr::Name(_)
        | AstExpr::String(_)
        | AstExpr::Int(_)
        | AstExpr::Float(_)
        | AstExpr::Char(_)
        | AstExpr::Bool(_)
        | AstExpr::Void => false,
    }
}

fn stmt_uses_result_prelude_variant(stmt: &Stmt) -> bool {
    stmt_uses_core_prelude_variant(stmt, "Result")
}

fn stmt_uses_option_prelude_variant(stmt: &Stmt) -> bool {
    stmt_uses_core_prelude_variant(stmt, "Option")
}

fn stmt_uses_core_prelude_variant(stmt: &Stmt, enum_name: &str) -> bool {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => {
            expr_uses_core_prelude_variant(value, enum_name)
        }
        Stmt::LetElse {
            pattern,
            value,
            else_body,
            ..
        } => {
            pattern_uses_core_prelude_variant(pattern, enum_name)
                || expr_uses_core_prelude_variant(value, enum_name)
                || else_body
                    .iter()
                    .any(|stmt| stmt_uses_core_prelude_variant(stmt, enum_name))
        }
        Stmt::IfLet {
            pattern,
            value,
            body,
            else_body,
            ..
        } => {
            pattern_uses_core_prelude_variant(pattern, enum_name)
                || expr_uses_core_prelude_variant(value, enum_name)
                || body
                    .iter()
                    .any(|stmt| stmt_uses_core_prelude_variant(stmt, enum_name))
                || else_body.as_ref().is_some_and(|else_body| {
                    else_body
                        .iter()
                        .any(|stmt| stmt_uses_core_prelude_variant(stmt, enum_name))
                })
        }
        Stmt::Return { value, .. } => value
            .as_ref()
            .is_some_and(|value| expr_uses_core_prelude_variant(value, enum_name)),
        Stmt::Expr { expr, .. } => expr_uses_core_prelude_variant(expr, enum_name),
        Stmt::Match { value, arms, .. } => {
            expr_uses_core_prelude_variant(value, enum_name)
                || arms.iter().any(|arm| {
                    pattern_uses_core_prelude_variant(&arm.pattern, enum_name)
                        || arm
                            .body
                            .iter()
                            .any(|stmt| stmt_uses_core_prelude_variant(stmt, enum_name))
                })
        }
        Stmt::For { variant, .. } => match variant {
            ForVariant::Infinite { body } => body
                .iter()
                .any(|stmt| stmt_uses_core_prelude_variant(stmt, enum_name)),
            ForVariant::While { condition, body } => {
                expr_uses_core_prelude_variant(condition, enum_name)
                    || body
                        .iter()
                        .any(|stmt| stmt_uses_core_prelude_variant(stmt, enum_name))
            }
            ForVariant::Iterate { iterable, body, .. } => {
                expr_uses_core_prelude_variant(iterable, enum_name)
                    || body
                        .iter()
                        .any(|stmt| stmt_uses_core_prelude_variant(stmt, enum_name))
            }
        },
        Stmt::Defer { stmt, .. } => stmt_uses_core_prelude_variant(stmt, enum_name),
        Stmt::Postfix { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => false,
    }
}

fn is_known_std_value_module(name: &str) -> bool {
    matches!(
        name,
        "io" | "fs" | "env" | "process" | "string" | "path" | "math" | "collections" | "Array"
    )
}

fn expr_uses_result_prelude_variant(expr: &AstExpr) -> bool {
    expr_uses_core_prelude_variant(expr, "Result")
}

fn expr_uses_option_prelude_variant(expr: &AstExpr) -> bool {
    expr_uses_core_prelude_variant(expr, "Option")
}

fn expr_uses_core_prelude_variant(expr: &AstExpr, enum_name: &str) -> bool {
    match expr {
        AstExpr::Call { callee, args, .. } => {
            pattern_uses_core_prelude_variant(callee, enum_name)
                || args
                    .iter()
                    .any(|arg| expr_uses_core_prelude_variant(arg, enum_name))
        }
        AstExpr::Name(path) => pattern_uses_core_prelude_variant(path, enum_name),
        AstExpr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_uses_core_prelude_variant(value, enum_name)),
        AstExpr::Match { value, arms } => {
            expr_uses_core_prelude_variant(value, enum_name)
                || arms.iter().any(|arm| {
                    pattern_uses_core_prelude_variant(&arm.pattern, enum_name)
                        || expr_uses_core_prelude_variant(&arm.value, enum_name)
                })
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_core_prelude_variant(condition, enum_name)
                || expr_uses_core_prelude_variant(then_branch, enum_name)
                || expr_uses_core_prelude_variant(else_branch, enum_name)
        }
        AstExpr::Panic { message }
        | AstExpr::Question { expr: message }
        | AstExpr::Unary { expr: message, .. } => {
            expr_uses_core_prelude_variant(message, enum_name)
        }
        AstExpr::Cast { expr, .. } => expr_uses_core_prelude_variant(expr, enum_name),
        AstExpr::Binary { left, right, .. } => {
            expr_uses_core_prelude_variant(left, enum_name)
                || expr_uses_core_prelude_variant(right, enum_name)
        }
        AstExpr::MutArg { .. }
        | AstExpr::String(_)
        | AstExpr::Int(_)
        | AstExpr::Float(_)
        | AstExpr::Char(_)
        | AstExpr::Bool(_)
        | AstExpr::Void => false,
    }
}

fn pattern_uses_core_prelude_variant(path: &[String], enum_name: &str) -> bool {
    matches!(
        path,
        [variant]
            if core_prelude_variant(variant)
                .is_some_and(|(resolved_enum, _)| resolved_enum == enum_name)
    )
}

fn function_signature(
    path: &Path,
    function: &AstFunction,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Result<FunctionSignature, Diagnostic> {
    let struct_names = structs
        .values()
        .map(|item| (item.name.clone(), item.type_params.len()))
        .collect::<Vec<_>>();
    let enum_names = enums
        .values()
        .map(|item| (item.name.clone(), item.type_params.len()))
        .collect::<Vec<_>>();
    let params = function
        .params
        .iter()
        .map(|param| {
            let value_type = parse_value_type_with_names(
                &param.type_ref,
                &struct_names,
                &enum_names,
                &function.type_params,
            )
            .ok_or_else(|| {
                unsupported_type_diagnostic(
                    path,
                    &function.span,
                    &param.type_ref,
                    "unsupported parameter type in v0.1 current implementation",
                    &struct_names,
                    &enum_names,
                )
            })?;
            ensure_supported_value_type(path, &value_type, &synthetic_span())?;
            Ok(ParamSignature {
                value_type,
                mutable: param.mutable,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let return_type = parse_value_type_with_names(
        &function.return_type,
        &struct_names,
        &enum_names,
        &function.type_params,
    )
    .ok_or_else(|| {
        unsupported_type_diagnostic(
            path,
            &function.span,
            &function.return_type,
            format!(
                "unsupported return type `{}` in v0.1 current implementation",
                function.return_type.path.join(".")
            ),
            &struct_names,
            &enum_names,
        )
    })?;
    ensure_supported_value_type(path, &return_type, &synthetic_span())?;
    Ok(FunctionSignature {
        type_params: function.type_params.clone(),
        params,
        return_type,
    })
}

fn lower_function_as(
    path: &Path,
    function: &AstFunction,
    lowered_name: &str,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    consts: &[(String, ValueType)],
) -> Result<Function, Diagnostic> {
    let signature = signatures
        .get(lowered_name)
        .expect("signature table is built before lowering");
    let mut scope = HashMap::new();
    for (name, value_type) in consts {
        scope.insert(
            name.clone(),
            Binding {
                value_type: value_type.clone(),
                mutable: false,
                source: BindingSource::Local,
            },
        );
    }
    let mut params = Vec::new();
    for (param, value_type) in function.params.iter().zip(signature.params.iter()) {
        if scope.contains_key(&param.name) {
            return Err(Diagnostic::new(
                "E0302",
                format!(
                    "parameter `{}` is already defined in this scope",
                    param.name
                ),
                path,
                1,
                1,
                1,
                "",
            ));
        }
        scope.insert(
            param.name.clone(),
            Binding {
                value_type: value_type.value_type.clone(),
                mutable: param.mutable,
                source: BindingSource::Param,
            },
        );
        params.push(Parameter {
            name: param.name.clone(),
            mutable: param.mutable,
            value_type: value_type.value_type.clone(),
        });
    }

    let mut body = Vec::new();
    for (index, stmt) in function.body.iter().enumerate() {
        let is_tail = index + 1 == function.body.len();
        lower_stmt_into(
            path,
            stmt,
            &mut scope,
            imports,
            signatures,
            structs,
            enums,
            &signature.return_type,
            is_tail,
            0,
            &mut body,
        )?;
    }

    if signature.return_type != ValueType::Void && !statements_satisfy_function_return(&body) {
        return Err(Diagnostic::new(
            "E0406",
            format!(
                "function `{}` must return `{}`",
                function.name,
                signature.return_type.name()
            ),
            path,
            1,
            1,
            1,
            "",
        ));
    }

    Ok(Function {
        package: function.package.join("."),
        name: lowered_name.to_string(),
        params,
        return_type: signature.return_type.clone(),
        body,
    })
}

fn validate_method_self(
    path: &Path,
    method: &AstFunction,
    owner_name: &str,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Result<(), Diagnostic> {
    let Some(self_param) = method.params.first() else {
        return Err(Diagnostic::new(
            "E0256",
            format!("method `{owner_name}.{}` must declare `self`", method.name),
            path,
            1,
            1,
            1,
            "",
        ));
    };
    if self_param.name != "self" {
        return Err(Diagnostic::new(
            "E0256",
            format!(
                "method `{owner_name}.{}` first parameter must be `self`",
                method.name
            ),
            path,
            1,
            1,
            1,
            "",
        ));
    }
    let Some(ValueType::Struct(self_type, self_args)) =
        parse_value_type(&self_param.type_ref, structs, enums)
    else {
        return Err(Diagnostic::new(
            "E0257",
            format!(
                "method `{owner_name}.{}` has invalid `self` type",
                method.name
            ),
            path,
            1,
            1,
            1,
            "",
        ));
    };
    if self_type != owner_name || !self_args.is_empty() {
        return Err(Diagnostic::new(
            "E0257",
            format!(
                "method `{owner_name}.{}` declares `self` as `{self_type}`",
                method.name
            ),
            path,
            1,
            1,
            1,
            "",
        ));
    }
    Ok(())
}

fn lower_stmt(
    path: &Path,
    stmt: &Stmt,
    scope: &mut HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    return_type: &ValueType,
    is_tail: bool,
    loop_depth: usize,
) -> Result<Statement, Diagnostic> {
    match stmt {
        Stmt::Let {
            name,
            mutable,
            type_annotation,
            value,
            span,
        } => {
            if scope.contains_key(name) {
                return Err(Diagnostic::new(
                    "E0302",
                    format!("variable `{name}` is already defined in this scope"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }

            if let AstExpr::Question { expr } = value {
                let Some(annotation) = type_annotation.as_ref() else {
                    return Err(Diagnostic::new(
                        "E0403",
                        "`?` let bindings require an explicit non-void type annotation",
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                };
                let annotated_type =
                    parse_non_void_type(annotation, structs, enums).ok_or_else(|| {
                        if annotation.path == ["void"] {
                            Diagnostic::new(
                                "E0403",
                                "`?` let bindings require an explicit non-void type annotation",
                                path,
                                span.line,
                                span.column,
                                span.length,
                                &span.text,
                            )
                        } else {
                            unsupported_type_diagnostic_from_maps(
                                path,
                                span,
                                annotation,
                                format!(
                                    "unsupported variable type `{}` in v0.1 current implementation",
                                    annotation.path.join(".")
                                ),
                                structs,
                                enums,
                            )
                        }
                    })?;
                ensure_supported_value_type(path, &annotated_type, span)?;
                let (result_type, result_expr) =
                    lower_value_expr(path, expr, scope, imports, signatures, structs, enums, span)?;
                let (carrier, ok_type) = question_payload(path, span, &result_type, return_type)?;
                if ok_type != annotated_type {
                    return Err(type_mismatch_expected_found(
                        path,
                        span,
                        format!(
                            "`?` unwraps `{}` but binding `{name}` is annotated as `{}`",
                            ok_type.name(),
                            annotated_type.name()
                        ),
                        &annotated_type,
                        &ok_type,
                    ));
                }
                scope.insert(
                    name.clone(),
                    Binding {
                        value_type: annotated_type.clone(),
                        mutable: *mutable,
                        source: BindingSource::Local,
                    },
                );
                return Ok(Statement::QuestionLet {
                    carrier,
                    name: name.clone(),
                    value_type: annotated_type,
                    result_type,
                    return_type: return_type.clone(),
                    result_expr,
                });
            }

            let annotated_type = if let Some(annotation) = type_annotation {
                let annotated_type =
                    parse_non_void_type(annotation, structs, enums).ok_or_else(|| {
                        unsupported_type_diagnostic_from_maps(
                            path,
                            span,
                            annotation,
                            format!(
                                "unsupported variable type `{}` in v0.1 current implementation",
                                annotation.path.join(".")
                            ),
                            structs,
                            enums,
                        )
                    })?;
                ensure_supported_value_type(path, &annotated_type, span)?;
                Some(annotated_type)
            } else {
                None
            };
            let (inferred_type, initializer) = lower_value_expr_with_expected(
                path,
                value,
                scope,
                imports,
                signatures,
                structs,
                enums,
                annotated_type.as_ref(),
                span,
            )?;
            let value_type = if let Some(annotated_type) = annotated_type {
                if annotated_type != inferred_type {
                    return Err(type_mismatch_expected_found(
                        path,
                        span,
                        format!(
                            "cannot initialize `{name}` as `{}` from `{}`",
                            annotated_type.name(),
                            inferred_type.name()
                        ),
                        &annotated_type,
                        &inferred_type,
                    ));
                }
                annotated_type
            } else {
                inferred_type
            };

            scope.insert(
                name.clone(),
                Binding {
                    value_type: value_type.clone(),
                    mutable: *mutable,
                    source: BindingSource::Local,
                },
            );
            Ok(Statement::Let {
                name: name.clone(),
                value_type,
                initializer,
            })
        }
        Stmt::LetElse {
            pattern,
            binding,
            value,
            else_body,
            span,
        } => lower_let_else_stmt(
            path,
            pattern,
            binding,
            value,
            else_body,
            scope,
            imports,
            signatures,
            structs,
            enums,
            return_type,
            loop_depth,
            span,
        ),
        Stmt::IfLet {
            pattern,
            binding,
            value,
            body,
            else_body,
            span,
        } => lower_if_let_stmt(
            path,
            pattern,
            binding.as_deref(),
            value,
            body,
            else_body.as_deref(),
            scope,
            imports,
            signatures,
            structs,
            enums,
            return_type,
            loop_depth,
            span,
        ),
        Stmt::Assign {
            target,
            op,
            value,
            span,
        } => lower_assign_stmt(
            path, target, *op, value, scope, imports, signatures, structs, enums, span,
        ),
        Stmt::Postfix { target, op, span } => lower_postfix_stmt(
            path, target, *op, scope, imports, signatures, structs, enums, span,
        ),
        Stmt::Return { value, span } => lower_return_stmt(
            path,
            value.as_ref(),
            scope,
            imports,
            signatures,
            structs,
            enums,
            return_type,
            span,
        ),
        Stmt::Expr { expr, span } if is_tail && return_type != &ValueType::Void => {
            let (expr_type, lowered) = lower_value_expr_with_expected(
                path,
                expr,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(return_type),
                span,
            )?;
            if &expr_type != return_type {
                return Err(type_mismatch(
                    path,
                    span,
                    format!(
                        "tail expression returns `{}` but function expects `{}`",
                        expr_type.name(),
                        return_type.name()
                    ),
                ));
            }
            Ok(Statement::Return(Some(lowered)))
        }
        Stmt::Expr {
            expr: AstExpr::Call { callee, args, .. },
            span,
        } if is_io_print_call(callee) => {
            let Some(function_name) = resolve_io_print_function(callee, imports) else {
                return Err(missing_io_import_diagnostic(path, span, callee));
            };
            let [arg] = args.as_slice() else {
                return Err(println_type_error(path, span, function_name));
            };
            let (arg_type, lowered) =
                lower_value_expr(path, arg, scope, imports, signatures, structs, enums, span)?;
            if arg_type != ValueType::String {
                return Err(println_type_error(path, span, function_name));
            }
            Ok(io_print_statement(function_name, lowered))
        }
        Stmt::Expr {
            expr: AstExpr::Panic { message },
            span,
        } => {
            let lowered = lower_panic_message(
                path, message, scope, imports, signatures, structs, enums, span,
            )?;
            Ok(Statement::Panic(lowered))
        }
        Stmt::Expr {
            expr:
                AstExpr::Call {
                    callee,
                    type_args,
                    args,
                },
            span,
        } if callee.len() == 2
            && matches!(callee[1].as_str(), "push" | "set" | "insert" | "clear")
            && !is_env_builtin_call(callee)
            && type_args.is_empty() =>
        {
            let lowered = lower_array_mutation(
                path, callee, args, scope, imports, signatures, structs, enums, span,
            )?;
            Ok(Statement::Assign {
                name: callee[0].clone(),
                value: lowered,
            })
        }
        Stmt::Match { value, arms, span } => lower_match_stmt(
            path,
            value,
            arms,
            scope,
            imports,
            signatures,
            structs,
            enums,
            return_type,
            loop_depth,
            span,
        ),
        Stmt::For { variant, span } => lower_for_stmt(
            path,
            variant,
            scope,
            imports,
            signatures,
            structs,
            enums,
            return_type,
            loop_depth,
            span,
        ),
        Stmt::Break { span } => {
            if loop_depth == 0 {
                return Err(Diagnostic::new(
                    "E0510",
                    "`break` is not allowed outside of a `for` loop",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok(Statement::Break)
        }
        Stmt::Continue { span } => {
            if loop_depth == 0 {
                return Err(Diagnostic::new(
                    "E0511",
                    "`continue` is not allowed outside of a `for` loop",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok(Statement::Continue)
        }
        Stmt::Defer { stmt, span } => {
            let Stmt::Expr { expr, .. } = stmt.as_ref() else {
                return Err(Diagnostic::new(
                    "E0265",
                    "`defer` expects a call expression",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            if let AstExpr::Call { callee, args, .. } = expr
                && is_io_print_call(callee)
            {
                let Some(function_name) = resolve_io_print_function(callee, imports) else {
                    return Err(missing_io_import_diagnostic(path, span, callee));
                };
                let [arg] = args.as_slice() else {
                    return Err(println_type_error(path, span, function_name));
                };
                let (arg_type, lowered) =
                    lower_value_expr(path, arg, scope, imports, signatures, structs, enums, span)?;
                if arg_type != ValueType::String {
                    return Err(println_type_error(path, span, function_name));
                }
                let call = io_print_deferred_call(function_name, lowered);
                return Ok(Statement::Defer { call });
            }
            let (_call_type, call) = lower_value_expr_with_expected(
                path,
                expr,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&ValueType::Void),
                span,
            )?;
            Ok(Statement::Defer {
                call: DeferredCall::Expr(call),
            })
        }
        Stmt::Expr { expr, span } => {
            let (expr_type, lowered) = lower_value_expr_with_expected(
                path,
                expr,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&ValueType::Void),
                span,
            )?;
            if expr_type != ValueType::Void {
                return Err(Diagnostic::new(
                    "E0203",
                    "unsupported non-void expression statement in v0.1 current implementation",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok(Statement::Expr(lowered))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_stmt_into(
    path: &Path,
    stmt: &Stmt,
    scope: &mut HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    return_type: &ValueType,
    is_tail: bool,
    loop_depth: usize,
    out: &mut Vec<Statement>,
) -> Result<(), Diagnostic> {
    if lower_question_exprs_in_stmt_into(
        path,
        stmt,
        scope,
        imports,
        signatures,
        structs,
        enums,
        return_type,
        is_tail,
        loop_depth,
        out,
    )? {
        return Ok(());
    }
    out.push(lower_stmt(
        path,
        stmt,
        scope,
        imports,
        signatures,
        structs,
        enums,
        return_type,
        is_tail,
        loop_depth,
    )?);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn lower_question_exprs_in_stmt_into(
    path: &Path,
    stmt: &Stmt,
    scope: &mut HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    return_type: &ValueType,
    is_tail: bool,
    loop_depth: usize,
    out: &mut Vec<Statement>,
) -> Result<bool, Diagnostic> {
    let rewritten = match stmt {
        Stmt::Let {
            name,
            mutable,
            type_annotation,
            value,
            span,
        } if matches!(value, AstExpr::If { .. }) && ast_expr_contains_question(value) => {
            if scope.contains_key(name) {
                return Err(Diagnostic::new(
                    "E0302",
                    format!("variable `{name}` is already defined in this scope"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            let AstExpr::If {
                condition,
                then_branch,
                else_branch,
            } = value
            else {
                unreachable!("guard matched if expression");
            };
            let annotated_type = if let Some(annotation) = type_annotation {
                let annotated_type =
                    parse_non_void_type(annotation, structs, enums).ok_or_else(|| {
                        unsupported_type_diagnostic_from_maps(
                            path,
                            span,
                            annotation,
                            format!(
                                "unsupported variable type `{}` in v0.1 current implementation",
                                annotation.path.join(".")
                            ),
                            structs,
                            enums,
                        )
                    })?;
                ensure_supported_value_type(path, &annotated_type, span)?;
                Some(annotated_type)
            } else {
                None
            };
            let (condition, _) = extract_question_exprs(
                path,
                condition,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            let (condition_type, condition) = lower_value_expr(
                path, &condition, scope, imports, signatures, structs, enums, span,
            )?;
            if condition_type != ValueType::Bool {
                return Err(type_mismatch(path, span, "`if` condition must be `bool`"));
            }
            let (then_type, body) = lower_expr_as_assignment_block(
                path,
                name,
                then_branch,
                &mut scope.clone(),
                imports,
                signatures,
                structs,
                enums,
                return_type,
                annotated_type.as_ref(),
                span,
            )?;
            let else_expected = annotated_type
                .as_ref()
                .or(if then_type == ValueType::Never {
                    None
                } else {
                    Some(&then_type)
                });
            let (else_type, else_body) = lower_expr_as_assignment_block(
                path,
                name,
                else_branch,
                &mut scope.clone(),
                imports,
                signatures,
                structs,
                enums,
                return_type,
                else_expected,
                span,
            )?;
            let value_type = if let Some(annotated_type) = annotated_type {
                if then_type != ValueType::Never && then_type != annotated_type {
                    return Err(type_mismatch_expected_found(
                        path,
                        span,
                        format!(
                            "`if` branch returns `{}` but `{name}` is annotated as `{}`",
                            then_type.name(),
                            annotated_type.name()
                        ),
                        &annotated_type,
                        &then_type,
                    ));
                }
                if else_type != ValueType::Never && else_type != annotated_type {
                    return Err(type_mismatch_expected_found(
                        path,
                        span,
                        format!(
                            "`if` branch returns `{}` but `{name}` is annotated as `{}`",
                            else_type.name(),
                            annotated_type.name()
                        ),
                        &annotated_type,
                        &else_type,
                    ));
                }
                annotated_type
            } else if then_type == ValueType::Never && else_type == ValueType::Never {
                return Err(Diagnostic::new(
                    "E0403",
                    "`if` initializer must contain at least one value-producing branch",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            } else if then_type == ValueType::Never {
                else_type
            } else if else_type == ValueType::Never || then_type == else_type {
                then_type
            } else {
                return Err(type_mismatch(
                    path,
                    span,
                    format!(
                        "`if` branches return `{}` and `{}`",
                        then_type.name(),
                        else_type.name()
                    ),
                ));
            };
            scope.insert(
                name.clone(),
                Binding {
                    value_type: value_type.clone(),
                    mutable: *mutable,
                    source: BindingSource::Local,
                },
            );
            out.push(Statement::LetIf {
                name: name.clone(),
                value_type,
                condition,
                body,
                else_body,
            });
            return Ok(true);
        }
        Stmt::Let {
            name,
            mutable,
            type_annotation,
            value,
            span,
        } if matches!(value, AstExpr::Match { .. }) && ast_expr_contains_question(value) => {
            if scope.contains_key(name) {
                return Err(Diagnostic::new(
                    "E0302",
                    format!("variable `{name}` is already defined in this scope"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            let AstExpr::Match { value, arms } = value else {
                unreachable!("guard matched match expression");
            };
            let annotated_type = if let Some(annotation) = type_annotation {
                let annotated_type =
                    parse_non_void_type(annotation, structs, enums).ok_or_else(|| {
                        unsupported_type_diagnostic_from_maps(
                            path,
                            span,
                            annotation,
                            format!(
                                "unsupported variable type `{}` in v0.1 current implementation",
                                annotation.path.join(".")
                            ),
                            structs,
                            enums,
                        )
                    })?;
                ensure_supported_value_type(path, &annotated_type, span)?;
                Some(annotated_type)
            } else {
                None
            };
            let (value, _) = extract_question_exprs(
                path,
                value,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            let (value_type, lowered_value) = lower_value_expr(
                path, &value, scope, imports, signatures, structs, enums, span,
            )?;
            let ValueType::Enum(enum_name, enum_args) = value_type else {
                return Err(type_mismatch(path, span, "`match` expects an enum value"));
            };
            let enum_type = enums
                .get(&enum_name)
                .expect("enum value must refer to a known enum");
            let mut seen = HashMap::new();
            let mut result_type = annotated_type.clone();
            let mut lowered_arms = Vec::new();
            for arm in arms {
                let Some(variant) = resolve_match_arm_variant(&arm.pattern, &enum_name, scope)
                else {
                    return Err(Diagnostic::new(
                        "E0316",
                        format!(
                            "match arm must use `{enum_name}.Variant` or a supported prelude variant"
                        ),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                };
                let Some(variant_type) =
                    enum_type.variants.iter().find(|item| item.name == *variant)
                else {
                    return Err(Diagnostic::new(
                        "E0315",
                        format!("enum `{enum_name}` has no variant `{variant}`"),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                };
                if seen.insert(variant.clone(), ()).is_some() {
                    return Err(Diagnostic::new(
                        "E0317",
                        format!("duplicate match arm for `{enum_name}.{variant}`"),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
                let mut arm_scope = scope.clone();
                let payload_type = variant_type.payload.as_ref().map(|payload| {
                    substitute_type_params(payload, &enum_type.type_params, &enum_args)
                });
                match (&payload_type, &arm.binding) {
                    (Some(payload_type), Some(binding)) => {
                        if scope.contains_key(binding) {
                            return Err(Diagnostic::new(
                                "E0302",
                                format!("variable `{binding}` is already defined in this scope"),
                                path,
                                span.line,
                                span.column,
                                span.length,
                                &span.text,
                            ));
                        }
                        arm_scope.insert(
                            binding.clone(),
                            Binding {
                                value_type: payload_type.clone(),
                                mutable: false,
                                source: BindingSource::EnumPayload {
                                    value: lowered_value.clone(),
                                    variant: variant.clone(),
                                },
                            },
                        );
                    }
                    (Some(_), None) => {
                        return Err(Diagnostic::new(
                            "E0321",
                            format!("match arm `{enum_name}.{variant}` must bind its payload"),
                            path,
                            span.line,
                            span.column,
                            span.length,
                            &span.text,
                        ));
                    }
                    (None, Some(_)) => {
                        return Err(Diagnostic::new(
                            "E0322",
                            format!("match arm `{enum_name}.{variant}` has no payload to bind"),
                            path,
                            span.line,
                            span.column,
                            span.length,
                            &span.text,
                        ));
                    }
                    (None, None) => {}
                }
                let (arm_type, body) = lower_expr_as_assignment_block(
                    path,
                    name,
                    &arm.value,
                    &mut arm_scope,
                    imports,
                    signatures,
                    structs,
                    enums,
                    return_type,
                    result_type.as_ref(),
                    span,
                )?;
                if let Some(expected_type) = &result_type {
                    if arm_type != ValueType::Never && expected_type != &arm_type {
                        return Err(type_mismatch(
                            path,
                            span,
                            format!(
                                "match arm returns `{}` but previous arms return `{}`",
                                arm_type.name(),
                                expected_type.name()
                            ),
                        ));
                    }
                } else if arm_type == ValueType::Never {
                    // A diverging arm does not determine the match initializer type.
                } else {
                    result_type = Some(arm_type);
                }
                lowered_arms.push(MatchStatementArm {
                    variant,
                    binding: arm.binding.clone(),
                    body,
                });
            }
            for variant in &enum_type.variants {
                if !seen.contains_key(&variant.name) {
                    return Err(Diagnostic::new(
                        "E0318",
                        format!("match is missing arm `{enum_name}.{}`", variant.name),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
            }
            let Some(value_type) = result_type else {
                return Err(Diagnostic::new(
                    "E0319",
                    "`match` initializer must contain at least one value-producing arm",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            scope.insert(
                name.clone(),
                Binding {
                    value_type: value_type.clone(),
                    mutable: *mutable,
                    source: BindingSource::Local,
                },
            );
            out.push(Statement::LetMatch {
                name: name.clone(),
                value_type,
                value: lowered_value,
                enum_name,
                enum_args,
                arms: lowered_arms,
            });
            return Ok(true);
        }
        Stmt::Let {
            name,
            mutable,
            type_annotation,
            value,
            span,
        } if !matches!(value, AstExpr::Question { .. }) => {
            if scope.contains_key(name) {
                return Err(Diagnostic::new(
                    "E0302",
                    format!("variable `{name}` is already defined in this scope"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            let (value, changed) = extract_question_exprs(
                path,
                value,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            if !changed {
                return Ok(false);
            }
            Stmt::Let {
                name: name.clone(),
                mutable: *mutable,
                type_annotation: type_annotation.clone(),
                value,
                span: span.clone(),
            }
        }
        Stmt::LetElse {
            pattern,
            binding,
            value,
            else_body,
            span,
        } if ast_expr_contains_question(value) => {
            let (value, changed) = extract_question_exprs(
                path,
                value,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            if !changed {
                return Ok(false);
            }
            Stmt::LetElse {
                pattern: pattern.clone(),
                binding: binding.clone(),
                value,
                else_body: else_body.clone(),
                span: span.clone(),
            }
        }
        Stmt::IfLet {
            pattern,
            binding,
            value,
            body,
            else_body,
            span,
        } if ast_expr_contains_question(value) => {
            let (value, changed) = extract_question_exprs(
                path,
                value,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            if !changed {
                return Ok(false);
            }
            Stmt::IfLet {
                pattern: pattern.clone(),
                binding: binding.clone(),
                value,
                body: body.clone(),
                else_body: else_body.clone(),
                span: span.clone(),
            }
        }
        Stmt::Match { value, arms, span } if ast_expr_contains_question(value) => {
            let (value, changed) = extract_question_exprs(
                path,
                value,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            if !changed {
                return Ok(false);
            }
            Stmt::Match {
                value,
                arms: arms.clone(),
                span: span.clone(),
            }
        }
        Stmt::For {
            variant:
                ForVariant::Iterate {
                    binding,
                    iterable,
                    body,
                },
            span,
        } if ast_expr_contains_question(iterable) => {
            let (iterable, changed) = extract_question_exprs(
                path,
                iterable,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            if !changed {
                return Ok(false);
            }
            Stmt::For {
                variant: ForVariant::Iterate {
                    binding: binding.clone(),
                    iterable,
                    body: body.clone(),
                },
                span: span.clone(),
            }
        }
        Stmt::Assign {
            target,
            op,
            value,
            span,
        } if matches!(value, AstExpr::If { .. }) && ast_expr_contains_question(value) => {
            let AstExpr::If {
                condition,
                then_branch,
                else_branch,
            } = value
            else {
                unreachable!("guard matched if expression");
            };
            let (condition, _) = extract_question_exprs(
                path,
                condition,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            let (condition_type, condition) = lower_value_expr(
                path, &condition, scope, imports, signatures, structs, enums, span,
            )?;
            if condition_type != ValueType::Bool {
                return Err(type_mismatch(path, span, "`if` condition must be `bool`"));
            }
            let body = lower_expr_as_target_assignment_block(
                path,
                target,
                *op,
                then_branch,
                &mut scope.clone(),
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
            )?;
            let else_body = lower_expr_as_target_assignment_block(
                path,
                target,
                *op,
                else_branch,
                &mut scope.clone(),
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
            )?;
            out.push(Statement::If {
                condition,
                body,
                else_body,
            });
            return Ok(true);
        }
        Stmt::Assign {
            target,
            op,
            value: AstExpr::Match { value, arms },
            span,
        } if ast_expr_contains_question(value)
            || arms
                .iter()
                .any(|arm| ast_expr_contains_question(&arm.value)) =>
        {
            let (value, _) = extract_question_exprs(
                path,
                value,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            let (value_type, lowered_value) = lower_value_expr(
                path, &value, scope, imports, signatures, structs, enums, span,
            )?;
            let ValueType::Enum(enum_name, enum_args) = value_type else {
                return Err(type_mismatch(path, span, "`match` expects an enum value"));
            };
            let enum_type = enums
                .get(&enum_name)
                .expect("enum value must refer to a known enum");
            let mut seen = HashMap::new();
            let mut lowered_arms = Vec::new();
            for arm in arms {
                let Some(variant) = resolve_match_arm_variant(&arm.pattern, &enum_name, scope)
                else {
                    return Err(Diagnostic::new(
                        "E0316",
                        format!(
                            "match arm must use `{enum_name}.Variant` or a supported prelude variant"
                        ),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                };
                let Some(variant_type) =
                    enum_type.variants.iter().find(|item| item.name == *variant)
                else {
                    return Err(Diagnostic::new(
                        "E0315",
                        format!("enum `{enum_name}` has no variant `{variant}`"),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                };
                if seen.insert(variant.clone(), ()).is_some() {
                    return Err(Diagnostic::new(
                        "E0317",
                        format!("duplicate match arm for `{enum_name}.{variant}`"),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
                let mut arm_scope = scope.clone();
                let payload_type = variant_type.payload.as_ref().map(|payload| {
                    substitute_type_params(payload, &enum_type.type_params, &enum_args)
                });
                match (&payload_type, &arm.binding) {
                    (Some(payload_type), Some(binding)) => {
                        if scope.contains_key(binding) {
                            return Err(Diagnostic::new(
                                "E0302",
                                format!("variable `{binding}` is already defined in this scope"),
                                path,
                                span.line,
                                span.column,
                                span.length,
                                &span.text,
                            ));
                        }
                        arm_scope.insert(
                            binding.clone(),
                            Binding {
                                value_type: payload_type.clone(),
                                mutable: false,
                                source: BindingSource::EnumPayload {
                                    value: lowered_value.clone(),
                                    variant: variant.clone(),
                                },
                            },
                        );
                    }
                    (Some(_), None) => {
                        return Err(Diagnostic::new(
                            "E0321",
                            format!("match arm `{enum_name}.{variant}` must bind its payload"),
                            path,
                            span.line,
                            span.column,
                            span.length,
                            &span.text,
                        ));
                    }
                    (None, Some(_)) => {
                        return Err(Diagnostic::new(
                            "E0322",
                            format!("match arm `{enum_name}.{variant}` has no payload to bind"),
                            path,
                            span.line,
                            span.column,
                            span.length,
                            &span.text,
                        ));
                    }
                    (None, None) => {}
                }
                let body = lower_expr_as_target_assignment_block(
                    path,
                    target,
                    *op,
                    &arm.value,
                    &mut arm_scope,
                    imports,
                    signatures,
                    structs,
                    enums,
                    return_type,
                    span,
                )?;
                lowered_arms.push(MatchStatementArm {
                    variant,
                    binding: arm.binding.clone(),
                    body,
                });
            }
            for variant in &enum_type.variants {
                if !seen.contains_key(&variant.name) {
                    return Err(Diagnostic::new(
                        "E0318",
                        format!("match is missing arm `{enum_name}.{}`", variant.name),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
            }
            out.push(Statement::Match {
                value: lowered_value,
                enum_name,
                enum_args,
                arms: lowered_arms,
            });
            return Ok(true);
        }
        Stmt::Assign {
            target,
            op,
            value,
            span,
        } if ast_expr_contains_question(value) => {
            let (value, changed) = extract_question_exprs(
                path,
                value,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            if !changed {
                return Ok(false);
            }
            Stmt::Assign {
                target: target.clone(),
                op: *op,
                value,
                span: span.clone(),
            }
        }
        Stmt::Defer { stmt, span } if matches!(stmt.as_ref(), Stmt::Expr { .. }) => {
            let Stmt::Expr {
                expr,
                span: expr_span,
            } = stmt.as_ref()
            else {
                unreachable!("guard matched expression defer");
            };
            if !ast_expr_contains_question(expr) {
                return Ok(false);
            }
            let (expr, changed) = extract_question_exprs(
                path,
                expr,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            if !changed {
                return Ok(false);
            }
            Stmt::Defer {
                stmt: Box::new(Stmt::Expr {
                    expr,
                    span: expr_span.clone(),
                }),
                span: span.clone(),
            }
        }
        Stmt::Expr { expr, span }
            if !(is_tail && return_type != &ValueType::Void)
                && ast_expr_contains_question(expr) =>
        {
            let (expr, changed) = extract_question_exprs(
                path,
                expr,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            if !changed {
                return Ok(false);
            }
            Stmt::Expr {
                expr,
                span: span.clone(),
            }
        }
        Stmt::Return {
            value:
                Some(AstExpr::If {
                    condition,
                    then_branch,
                    else_branch,
                }),
            span,
        } if ast_expr_contains_question(condition)
            || ast_expr_contains_question(then_branch)
            || ast_expr_contains_question(else_branch) =>
        {
            let (condition, _) = extract_question_exprs(
                path,
                condition,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            let (condition_type, condition) = lower_value_expr(
                path, &condition, scope, imports, signatures, structs, enums, span,
            )?;
            if condition_type != ValueType::Bool {
                return Err(type_mismatch(path, span, "`if` condition must be `bool`"));
            }
            let body = lower_tail_expr_as_return_block(
                path,
                then_branch,
                &mut scope.clone(),
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
            )?;
            let else_body = lower_tail_expr_as_return_block(
                path,
                else_branch,
                &mut scope.clone(),
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
            )?;
            out.push(Statement::If {
                condition,
                body,
                else_body,
            });
            return Ok(true);
        }
        Stmt::Return {
            value:
                Some(AstExpr::Call {
                    callee,
                    type_args,
                    args,
                }),
            span,
        } if type_args.is_empty()
            && is_result_ok_callee(callee, signatures)
            && matches!(args.as_slice(), [AstExpr::If { .. }])
            && args.iter().any(ast_expr_contains_question) =>
        {
            let [
                AstExpr::If {
                    condition,
                    then_branch,
                    else_branch,
                },
            ] = args.as_slice()
            else {
                unreachable!("guard matched single if argument");
            };
            let (condition, _) = extract_question_exprs(
                path,
                condition,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            let (condition_type, condition) = lower_value_expr(
                path, &condition, scope, imports, signatures, structs, enums, span,
            )?;
            if condition_type != ValueType::Bool {
                return Err(type_mismatch(path, span, "`if` condition must be `bool`"));
            }
            let then_ok = AstExpr::Call {
                callee: callee.clone(),
                type_args: Vec::new(),
                args: vec![then_branch.as_ref().clone()],
            };
            let else_ok = AstExpr::Call {
                callee: callee.clone(),
                type_args: Vec::new(),
                args: vec![else_branch.as_ref().clone()],
            };
            let body = lower_tail_expr_as_return_block(
                path,
                &then_ok,
                &mut scope.clone(),
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
            )?;
            let else_body = lower_tail_expr_as_return_block(
                path,
                &else_ok,
                &mut scope.clone(),
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
            )?;
            out.push(Statement::If {
                condition,
                body,
                else_body,
            });
            return Ok(true);
        }
        Stmt::Return {
            value:
                Some(AstExpr::Call {
                    callee,
                    type_args,
                    args,
                }),
            span,
        } if type_args.is_empty()
            && is_result_ok_callee(callee, signatures)
            && matches!(args.as_slice(), [AstExpr::Match { .. }])
            && args.iter().any(ast_expr_contains_question) =>
        {
            let [AstExpr::Match { value, arms }] = args.as_slice() else {
                unreachable!("guard matched single match argument");
            };
            let (value, _) = extract_question_exprs(
                path,
                value,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            let (value_type, lowered_value) = lower_value_expr(
                path, &value, scope, imports, signatures, structs, enums, span,
            )?;
            let ValueType::Enum(enum_name, enum_args) = value_type else {
                return Err(type_mismatch(path, span, "`match` expects an enum value"));
            };
            let enum_type = enums
                .get(&enum_name)
                .expect("enum value must refer to a known enum");
            let mut seen = HashMap::new();
            let mut lowered_arms = Vec::new();
            for arm in arms {
                let Some(variant) = resolve_match_arm_variant(&arm.pattern, &enum_name, scope)
                else {
                    return Err(Diagnostic::new(
                        "E0316",
                        format!(
                            "match arm must use `{enum_name}.Variant` or a supported prelude variant"
                        ),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                };
                let Some(variant_type) =
                    enum_type.variants.iter().find(|item| item.name == *variant)
                else {
                    return Err(Diagnostic::new(
                        "E0315",
                        format!("enum `{enum_name}` has no variant `{variant}`"),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                };
                if seen.insert(variant.clone(), ()).is_some() {
                    return Err(Diagnostic::new(
                        "E0317",
                        format!("duplicate match arm for `{enum_name}.{variant}`"),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
                let mut arm_scope = scope.clone();
                let payload_type = variant_type.payload.as_ref().map(|payload| {
                    substitute_type_params(payload, &enum_type.type_params, &enum_args)
                });
                match (&payload_type, &arm.binding) {
                    (Some(payload_type), Some(binding)) => {
                        if scope.contains_key(binding) {
                            return Err(Diagnostic::new(
                                "E0302",
                                format!("variable `{binding}` is already defined in this scope"),
                                path,
                                span.line,
                                span.column,
                                span.length,
                                &span.text,
                            ));
                        }
                        arm_scope.insert(
                            binding.clone(),
                            Binding {
                                value_type: payload_type.clone(),
                                mutable: false,
                                source: BindingSource::EnumPayload {
                                    value: lowered_value.clone(),
                                    variant: variant.clone(),
                                },
                            },
                        );
                    }
                    (Some(_), None) => {
                        return Err(Diagnostic::new(
                            "E0321",
                            format!("match arm `{enum_name}.{variant}` must bind its payload"),
                            path,
                            span.line,
                            span.column,
                            span.length,
                            &span.text,
                        ));
                    }
                    (None, Some(_)) => {
                        return Err(Diagnostic::new(
                            "E0322",
                            format!("match arm `{enum_name}.{variant}` has no payload to bind"),
                            path,
                            span.line,
                            span.column,
                            span.length,
                            &span.text,
                        ));
                    }
                    (None, None) => {}
                }
                let ok_arm = AstExpr::Call {
                    callee: callee.clone(),
                    type_args: Vec::new(),
                    args: vec![arm.value.clone()],
                };
                let body = lower_tail_expr_as_return_block(
                    path,
                    &ok_arm,
                    &mut arm_scope,
                    imports,
                    signatures,
                    structs,
                    enums,
                    return_type,
                    span,
                )?;
                lowered_arms.push(MatchStatementArm {
                    variant,
                    binding: arm.binding.clone(),
                    body,
                });
            }
            for variant in &enum_type.variants {
                if !seen.contains_key(&variant.name) {
                    return Err(Diagnostic::new(
                        "E0318",
                        format!("match is missing arm `{enum_name}.{}`", variant.name),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
            }
            out.push(Statement::Match {
                value: lowered_value,
                enum_name,
                enum_args,
                arms: lowered_arms,
            });
            return Ok(true);
        }
        Stmt::Return {
            value: Some(value),
            span,
        } if matches!(value, AstExpr::Match { .. }) && ast_expr_contains_question(value) => {
            let AstExpr::Match { value, arms } = value else {
                unreachable!("guard matched match expression");
            };
            let (value, _) = extract_question_exprs(
                path,
                value,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            out.push(lower_tail_match_expr_as_statement(
                path,
                &value,
                arms,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
            )?);
            return Ok(true);
        }
        Stmt::Return {
            value: Some(value),
            span,
        } if !matches!(value, AstExpr::Question { .. })
            && question_expr_from_result_ok_return(value, signatures).is_none() =>
        {
            let (value, changed) = extract_question_exprs(
                path,
                value,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            if !changed {
                return Ok(false);
            }
            Stmt::Return {
                value: Some(value),
                span: span.clone(),
            }
        }
        Stmt::Expr {
            expr:
                AstExpr::If {
                    condition,
                    then_branch,
                    else_branch,
                },
            span,
        } if is_tail
            && return_type != &ValueType::Void
            && (ast_expr_contains_question(condition)
                || ast_expr_contains_question(then_branch)
                || ast_expr_contains_question(else_branch)) =>
        {
            let (condition, _) = extract_question_exprs(
                path,
                condition,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            let (condition_type, condition) = lower_value_expr(
                path, &condition, scope, imports, signatures, structs, enums, span,
            )?;
            if condition_type != ValueType::Bool {
                return Err(type_mismatch(path, span, "`if` condition must be `bool`"));
            }
            let body = lower_tail_expr_as_return_block(
                path,
                then_branch,
                &mut scope.clone(),
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
            )?;
            let else_body = lower_tail_expr_as_return_block(
                path,
                else_branch,
                &mut scope.clone(),
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
            )?;
            out.push(Statement::If {
                condition,
                body,
                else_body,
            });
            return Ok(true);
        }
        Stmt::Expr {
            expr: AstExpr::Match { value, arms },
            span,
        } if is_tail
            && return_type != &ValueType::Void
            && arms
                .iter()
                .any(|arm| ast_expr_contains_question(&arm.value)) =>
        {
            out.push(lower_tail_match_expr_as_statement(
                path,
                value,
                arms,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
            )?);
            return Ok(true);
        }
        Stmt::Expr { expr, span } if is_tail && return_type != &ValueType::Void => {
            let (expr, changed) = extract_question_exprs(
                path,
                expr,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            if !changed {
                return Ok(false);
            }
            Stmt::Expr {
                expr,
                span: span.clone(),
            }
        }
        _ => return Ok(false),
    };
    out.push(lower_stmt(
        path,
        &rewritten,
        scope,
        imports,
        signatures,
        structs,
        enums,
        return_type,
        false,
        loop_depth,
    )?);
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
fn lower_expr_as_assignment_block(
    path: &Path,
    target: &str,
    expr: &AstExpr,
    scope: &mut HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    return_type: &ValueType,
    expected: Option<&ValueType>,
    span: &Span,
) -> Result<(ValueType, Vec<Statement>), Diagnostic> {
    let mut out = Vec::new();
    let (expr, _) = extract_question_exprs(
        path,
        expr,
        scope,
        imports,
        signatures,
        structs,
        enums,
        return_type,
        span,
        &mut out,
    )?;
    let (value_type, value) = lower_value_expr_with_expected(
        path, &expr, scope, imports, signatures, structs, enums, expected, span,
    )?;
    if value_type != ValueType::Never {
        out.push(Statement::Assign {
            name: target.to_string(),
            value,
        });
    }
    Ok((value_type, out))
}

#[allow(clippy::too_many_arguments)]
fn lower_expr_as_target_assignment_block(
    path: &Path,
    target: &[String],
    op: AssignOp,
    expr: &AstExpr,
    scope: &mut HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    return_type: &ValueType,
    span: &Span,
) -> Result<Vec<Statement>, Diagnostic> {
    let mut out = Vec::new();
    let (expr, _) = extract_question_exprs(
        path,
        expr,
        scope,
        imports,
        signatures,
        structs,
        enums,
        return_type,
        span,
        &mut out,
    )?;
    out.push(lower_assign_stmt(
        path, target, op, &expr, scope, imports, signatures, structs, enums, span,
    )?);
    Ok(out)
}

#[allow(clippy::too_many_arguments)]
fn lower_tail_expr_as_return_block(
    path: &Path,
    expr: &AstExpr,
    scope: &mut HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    return_type: &ValueType,
    span: &Span,
) -> Result<Vec<Statement>, Diagnostic> {
    let mut out = Vec::new();
    let stmt = Stmt::Return {
        value: Some(expr.clone()),
        span: span.clone(),
    };
    lower_stmt_into(
        path,
        &stmt,
        scope,
        imports,
        signatures,
        structs,
        enums,
        return_type,
        false,
        0,
        &mut out,
    )?;
    Ok(out)
}

#[allow(clippy::too_many_arguments)]
fn lower_tail_match_expr_as_statement(
    path: &Path,
    value: &AstExpr,
    arms: &[AstMatchArm],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    return_type: &ValueType,
    span: &Span,
) -> Result<Statement, Diagnostic> {
    let (value_type, lowered_value) = lower_value_expr(
        path, value, scope, imports, signatures, structs, enums, span,
    )?;
    let ValueType::Enum(enum_name, enum_args) = value_type else {
        return Err(type_mismatch(path, span, "`match` expects an enum value"));
    };
    let enum_type = enums
        .get(&enum_name)
        .expect("enum value must refer to a known enum");
    let mut seen = HashMap::new();
    let mut lowered_arms = Vec::new();
    for arm in arms {
        let Some(variant) = resolve_match_arm_variant(&arm.pattern, &enum_name, scope) else {
            return Err(Diagnostic::new(
                "E0316",
                format!("match arm must use `{enum_name}.Variant` or a supported prelude variant"),
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        };
        let Some(variant_type) = enum_type.variants.iter().find(|item| item.name == *variant)
        else {
            return Err(Diagnostic::new(
                "E0315",
                format!("enum `{enum_name}` has no variant `{variant}`"),
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        };
        if seen.insert(variant.clone(), ()).is_some() {
            return Err(Diagnostic::new(
                "E0317",
                format!("duplicate match arm for `{enum_name}.{variant}`"),
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        }
        let mut arm_scope = scope.clone();
        let payload_type = variant_type
            .payload
            .as_ref()
            .map(|payload| substitute_type_params(payload, &enum_type.type_params, &enum_args));
        match (&payload_type, &arm.binding) {
            (Some(payload_type), Some(binding)) => {
                if scope.contains_key(binding) {
                    return Err(Diagnostic::new(
                        "E0302",
                        format!("variable `{binding}` is already defined in this scope"),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
                arm_scope.insert(
                    binding.clone(),
                    Binding {
                        value_type: payload_type.clone(),
                        mutable: false,
                        source: BindingSource::EnumPayload {
                            value: lowered_value.clone(),
                            variant: variant.clone(),
                        },
                    },
                );
            }
            (Some(_), None) => {
                return Err(Diagnostic::new(
                    "E0321",
                    format!("match arm `{enum_name}.{variant}` must bind its payload"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            (None, Some(_)) => {
                return Err(Diagnostic::new(
                    "E0322",
                    format!("match arm `{enum_name}.{variant}` has no payload to bind"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            (None, None) => {}
        }
        let body = lower_tail_expr_as_return_block(
            path,
            &arm.value,
            &mut arm_scope,
            imports,
            signatures,
            structs,
            enums,
            return_type,
            span,
        )?;
        lowered_arms.push(MatchStatementArm {
            variant,
            binding: arm.binding.clone(),
            body,
        });
    }
    for variant in &enum_type.variants {
        if !seen.contains_key(&variant.name) {
            return Err(Diagnostic::new(
                "E0318",
                format!("match is missing arm `{enum_name}.{}`", variant.name),
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        }
    }
    Ok(Statement::Match {
        value: lowered_value,
        enum_name,
        enum_args,
        arms: lowered_arms,
    })
}

fn ast_expr_contains_question(expr: &AstExpr) -> bool {
    match expr {
        AstExpr::Question { .. } => true,
        AstExpr::Call { args, .. } => args.iter().any(ast_expr_contains_question),
        AstExpr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| ast_expr_contains_question(value)),
        AstExpr::Match { value, arms } => {
            ast_expr_contains_question(value)
                || arms
                    .iter()
                    .any(|arm| ast_expr_contains_question(&arm.value))
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            ast_expr_contains_question(condition)
                || ast_expr_contains_question(then_branch)
                || ast_expr_contains_question(else_branch)
        }
        AstExpr::Panic { message } | AstExpr::Unary { expr: message, .. } => {
            ast_expr_contains_question(message)
        }
        AstExpr::Cast { expr, .. } => ast_expr_contains_question(expr),
        AstExpr::Binary { left, right, .. } => {
            ast_expr_contains_question(left) || ast_expr_contains_question(right)
        }
        AstExpr::MutArg { .. }
        | AstExpr::Name(_)
        | AstExpr::String(_)
        | AstExpr::Int(_)
        | AstExpr::Float(_)
        | AstExpr::Char(_)
        | AstExpr::Bool(_)
        | AstExpr::Void => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn extract_question_exprs(
    path: &Path,
    expr: &AstExpr,
    scope: &mut HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    return_type: &ValueType,
    span: &Span,
    out: &mut Vec<Statement>,
) -> Result<(AstExpr, bool), Diagnostic> {
    match expr {
        AstExpr::Question { expr } => {
            let (rewritten_result, _) = extract_question_exprs(
                path,
                expr,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            let (result_type, result_expr) = lower_value_expr(
                path,
                &rewritten_result,
                scope,
                imports,
                signatures,
                structs,
                enums,
                span,
            )?;
            let (carrier, ok_type) = question_payload(path, span, &result_type, return_type)?;
            let temp = fresh_internal_binding(scope, "question_value");
            scope.insert(
                temp.clone(),
                Binding {
                    value_type: ok_type.clone(),
                    mutable: false,
                    source: BindingSource::Local,
                },
            );
            out.push(Statement::QuestionLet {
                carrier,
                name: temp.clone(),
                value_type: ok_type,
                result_type,
                return_type: return_type.clone(),
                result_expr,
            });
            Ok((AstExpr::Name(vec![temp]), true))
        }
        AstExpr::Call {
            callee,
            type_args,
            args,
        } => {
            let (args, changed) = extract_question_exprs_from_vec(
                path,
                args,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            Ok((
                AstExpr::Call {
                    callee: callee.clone(),
                    type_args: type_args.clone(),
                    args,
                },
                changed,
            ))
        }
        AstExpr::StructLiteral { type_name, fields } => {
            let mut changed = false;
            let mut rewritten = Vec::new();
            for (field, value) in fields {
                let (value, value_changed) = extract_question_exprs(
                    path,
                    value,
                    scope,
                    imports,
                    signatures,
                    structs,
                    enums,
                    return_type,
                    span,
                    out,
                )?;
                changed |= value_changed;
                rewritten.push((field.clone(), value));
            }
            Ok((
                AstExpr::StructLiteral {
                    type_name: type_name.clone(),
                    fields: rewritten,
                },
                changed,
            ))
        }
        AstExpr::Binary { left, op, right } => {
            let (left, left_changed) = extract_question_exprs(
                path,
                left,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            let (right, right_changed) = extract_question_exprs(
                path,
                right,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            Ok((
                AstExpr::Binary {
                    left: Box::new(left),
                    op: op.clone(),
                    right: Box::new(right),
                },
                left_changed || right_changed,
            ))
        }
        AstExpr::Cast { expr, target } => {
            let (expr, changed) = extract_question_exprs(
                path,
                expr,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            Ok((
                AstExpr::Cast {
                    expr: Box::new(expr),
                    target: target.clone(),
                },
                changed,
            ))
        }
        AstExpr::Unary { op, expr } => {
            let (expr, changed) = extract_question_exprs(
                path,
                expr,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            Ok((
                AstExpr::Unary {
                    op: op.clone(),
                    expr: Box::new(expr),
                },
                changed,
            ))
        }
        AstExpr::Match { value, arms } => {
            let (value, changed) = extract_question_exprs(
                path,
                value,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            Ok((
                AstExpr::Match {
                    value: Box::new(value),
                    arms: arms.clone(),
                },
                changed,
            ))
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let (condition, changed) = extract_question_exprs(
                path,
                condition,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            Ok((
                AstExpr::If {
                    condition: Box::new(condition),
                    then_branch: then_branch.clone(),
                    else_branch: else_branch.clone(),
                },
                changed,
            ))
        }
        AstExpr::Panic { message } => {
            let (message, changed) = extract_question_exprs(
                path,
                message,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                span,
                out,
            )?;
            Ok((
                AstExpr::Panic {
                    message: Box::new(message),
                },
                changed,
            ))
        }
        AstExpr::MutArg { .. }
        | AstExpr::Name(_)
        | AstExpr::String(_)
        | AstExpr::Int(_)
        | AstExpr::Float(_)
        | AstExpr::Char(_)
        | AstExpr::Bool(_)
        | AstExpr::Void => Ok((expr.clone(), false)),
    }
}

#[allow(clippy::too_many_arguments)]
fn extract_question_exprs_from_vec(
    path: &Path,
    exprs: &[AstExpr],
    scope: &mut HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    return_type: &ValueType,
    span: &Span,
    out: &mut Vec<Statement>,
) -> Result<(Vec<AstExpr>, bool), Diagnostic> {
    let mut changed = false;
    let mut rewritten = Vec::new();
    for expr in exprs {
        let (expr, expr_changed) = extract_question_exprs(
            path,
            expr,
            scope,
            imports,
            signatures,
            structs,
            enums,
            return_type,
            span,
            out,
        )?;
        changed |= expr_changed;
        rewritten.push(expr);
    }
    Ok((rewritten, changed))
}

fn fresh_internal_binding(scope: &HashMap<String, Binding>, prefix: &str) -> String {
    let mut index = 0;
    loop {
        let candidate = format!("__{prefix}_{index}");
        if !scope.contains_key(&candidate) {
            return candidate;
        }
        index += 1;
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_let_else_stmt(
    path: &Path,
    pattern: &[String],
    binding: &str,
    value: &AstExpr,
    else_body: &[Stmt],
    scope: &mut HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    return_type: &ValueType,
    loop_depth: usize,
    span: &Span,
) -> Result<Statement, Diagnostic> {
    if scope.contains_key(binding) {
        return Err(Diagnostic::new(
            "E0302",
            format!("variable `{binding}` is already defined in this scope"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    let (value_type, lowered_value) = lower_value_expr(
        path, value, scope, imports, signatures, structs, enums, span,
    )?;
    let ValueType::Enum(enum_name, enum_args) = value_type else {
        return Err(type_mismatch(
            path,
            span,
            "`let else` expects an enum value",
        ));
    };
    let enum_type = enums
        .get(&enum_name)
        .expect("enum value must refer to a known enum");
    let Some(variant) = resolve_match_arm_variant(pattern, &enum_name, scope) else {
        return Err(Diagnostic::new(
            "E0316",
            format!(
                "let-else pattern must use `{enum_name}.Variant` or a supported prelude variant"
            ),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let Some(variant_type) = enum_type.variants.iter().find(|item| item.name == variant) else {
        return Err(Diagnostic::new(
            "E0315",
            format!("enum `{enum_name}` has no variant `{variant}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let Some(raw_payload_type) = &variant_type.payload else {
        return Err(Diagnostic::new(
            "E0322",
            format!("let-else pattern `{enum_name}.{variant}` has no payload to bind"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let payload_type = substitute_type_params(raw_payload_type, &enum_type.type_params, &enum_args);
    let lowered_else = lower_block(
        path,
        else_body,
        &mut scope.clone(),
        imports,
        signatures,
        structs,
        enums,
        return_type,
        loop_depth,
    )?;
    if !statements_diverge(&lowered_else) {
        return Err(Diagnostic::new(
            "E0521",
            "`let else` else body must diverge with `panic`, `return`, `break`, or `continue`",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    scope.insert(
        binding.to_string(),
        Binding {
            value_type: payload_type.clone(),
            mutable: false,
            source: BindingSource::Local,
        },
    );
    Ok(Statement::LetElse {
        binding: binding.to_string(),
        value_type: payload_type,
        value: lowered_value,
        enum_name,
        enum_args,
        variant,
        else_body: lowered_else,
    })
}

fn statements_diverge(statements: &[Statement]) -> bool {
    statements.last().is_some_and(statement_diverges)
}

fn statement_diverges(statement: &Statement) -> bool {
    match statement {
        Statement::Return(_)
        | Statement::QuestionReturnOk { .. }
        | Statement::Panic(_)
        | Statement::Break
        | Statement::Continue => true,
        Statement::Match { arms, .. } => arms.iter().all(|arm| statements_diverge(&arm.body)),
        Statement::If {
            body, else_body, ..
        } => statements_diverge(body) && statements_diverge(else_body),
        Statement::IfLet {
            body,
            else_body: Some(else_body),
            ..
        } => statements_diverge(body) && statements_diverge(else_body),
        Statement::Loop { kind, .. } => matches!(kind, LoopKind::Infinite),
        _ => false,
    }
}

fn statements_satisfy_function_return(statements: &[Statement]) -> bool {
    statements
        .last()
        .is_some_and(statement_satisfies_function_return)
}

fn statement_satisfies_function_return(statement: &Statement) -> bool {
    match statement {
        Statement::Return(Some(_)) | Statement::QuestionReturnOk { .. } | Statement::Panic(_) => {
            true
        }
        Statement::Match { arms, .. } => arms
            .iter()
            .all(|arm| statements_satisfy_function_return(&arm.body)),
        Statement::If {
            body, else_body, ..
        } => {
            statements_satisfy_function_return(body)
                && statements_satisfy_function_return(else_body)
        }
        Statement::IfLet {
            body,
            else_body: Some(else_body),
            ..
        } => {
            statements_satisfy_function_return(body)
                && statements_satisfy_function_return(else_body)
        }
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_if_let_stmt(
    path: &Path,
    pattern: &[String],
    binding: Option<&str>,
    value: &AstExpr,
    body: &[Stmt],
    else_body: Option<&[Stmt]>,
    scope: &mut HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    return_type: &ValueType,
    loop_depth: usize,
    span: &Span,
) -> Result<Statement, Diagnostic> {
    let (value_type, lowered_value) = lower_value_expr(
        path, value, scope, imports, signatures, structs, enums, span,
    )?;
    let ValueType::Enum(enum_name, enum_args) = value_type else {
        return Err(type_mismatch(path, span, "`if let` expects an enum value"));
    };
    let enum_type = enums
        .get(&enum_name)
        .expect("enum value must refer to a known enum");
    let Some(variant) = resolve_match_arm_variant(pattern, &enum_name, scope) else {
        return Err(Diagnostic::new(
            "E0316",
            format!("if-let pattern must use `{enum_name}.Variant` or a supported prelude variant"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let Some(variant_type) = enum_type.variants.iter().find(|item| item.name == variant) else {
        return Err(Diagnostic::new(
            "E0315",
            format!("enum `{enum_name}` has no variant `{variant}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let payload_type = variant_type
        .payload
        .as_ref()
        .map(|payload| substitute_type_params(payload, &enum_type.type_params, &enum_args));

    let mut body_scope = scope.clone();
    match (&payload_type, binding) {
        (Some(payload_type), Some(binding)) => {
            if scope.contains_key(binding) {
                return Err(Diagnostic::new(
                    "E0302",
                    format!("variable `{binding}` is already defined in this scope"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            body_scope.insert(
                binding.to_string(),
                Binding {
                    value_type: payload_type.clone(),
                    mutable: false,
                    source: BindingSource::Local,
                },
            );
        }
        (Some(_), None) => {
            return Err(Diagnostic::new(
                "E0234",
                "expected binding name in if-let pattern with payload",
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        }
        (None, Some(binding)) => {
            return Err(Diagnostic::new(
                "E0322",
                format!(
                    "if-let pattern `{enum_name}.{variant}` has no payload to bind as `{binding}`"
                ),
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        }
        (None, None) => {}
    }

    let lowered_body = lower_block(
        path,
        body,
        &mut body_scope,
        imports,
        signatures,
        structs,
        enums,
        return_type,
        loop_depth,
    )?;
    let lowered_else = if let Some(else_body) = else_body {
        Some(lower_block(
            path,
            else_body,
            &mut scope.clone(),
            imports,
            signatures,
            structs,
            enums,
            return_type,
            loop_depth,
        )?)
    } else {
        None
    };

    Ok(Statement::IfLet {
        binding: binding.map(str::to_string),
        value_type: payload_type,
        value: lowered_value,
        enum_name,
        enum_args,
        variant,
        body: lowered_body,
        else_body: lowered_else,
    })
}

#[allow(clippy::too_many_arguments)]
fn lower_match_stmt(
    path: &Path,
    value: &AstExpr,
    arms: &[crate::ast::MatchStmtArm],
    scope: &mut HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    return_type: &ValueType,
    loop_depth: usize,
    span: &Span,
) -> Result<Statement, Diagnostic> {
    let (value_type, lowered_value) = lower_value_expr(
        path, value, scope, imports, signatures, structs, enums, span,
    )?;
    let ValueType::Enum(enum_name, enum_args) = value_type else {
        return Err(type_mismatch(path, span, "`match` expects an enum value"));
    };
    let enum_type = enums
        .get(&enum_name)
        .expect("enum value must refer to a known enum");
    let mut seen = HashMap::new();
    let mut lowered_arms = Vec::new();
    for arm in arms {
        let Some(variant) = resolve_match_arm_variant(&arm.pattern, &enum_name, scope) else {
            return Err(Diagnostic::new(
                "E0316",
                format!("match arm must use `{enum_name}.Variant` or a supported prelude variant"),
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        };
        let Some(variant_type) = enum_type.variants.iter().find(|item| item.name == *variant)
        else {
            return Err(Diagnostic::new(
                "E0315",
                format!("enum `{enum_name}` has no variant `{variant}`"),
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        };
        let mut arm_scope = scope.clone();
        let payload_type = variant_type
            .payload
            .as_ref()
            .map(|payload| substitute_type_params(payload, &enum_type.type_params, &enum_args));
        match (&payload_type, &arm.binding) {
            (Some(payload_type), Some(binding)) => {
                if scope.contains_key(binding) {
                    return Err(Diagnostic::new(
                        "E0302",
                        format!("variable `{binding}` is already defined in this scope"),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
                arm_scope.insert(
                    binding.clone(),
                    Binding {
                        value_type: payload_type.clone(),
                        mutable: false,
                        source: BindingSource::EnumPayload {
                            value: lowered_value.clone(),
                            variant: variant.clone(),
                        },
                    },
                );
            }
            (Some(_), None) => {
                return Err(Diagnostic::new(
                    "E0321",
                    format!("match arm `{enum_name}.{variant}` must bind its payload"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            (None, Some(_)) => {
                return Err(Diagnostic::new(
                    "E0322",
                    format!("match arm `{enum_name}.{variant}` has no payload to bind"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            (None, None) => {}
        }
        if seen.insert(variant.clone(), ()).is_some() {
            return Err(Diagnostic::new(
                "E0317",
                format!("duplicate match arm for `{enum_name}.{variant}`"),
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        }
        let body = lower_block(
            path,
            &arm.body,
            &mut arm_scope,
            imports,
            signatures,
            structs,
            enums,
            return_type,
            loop_depth,
        )?;
        lowered_arms.push(MatchStatementArm {
            variant,
            binding: arm.binding.clone(),
            body,
        });
    }
    for variant in &enum_type.variants {
        if !seen.contains_key(&variant.name) {
            return Err(Diagnostic::new(
                "E0318",
                format!(
                    "match on `{enum_name}` is missing arm `{enum_name}.{}`",
                    variant.name
                ),
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        }
    }
    Ok(Statement::Match {
        value: lowered_value,
        enum_name,
        enum_args,
        arms: lowered_arms,
    })
}

#[allow(clippy::too_many_arguments)]
fn lower_for_stmt(
    path: &Path,
    variant: &ForVariant,
    scope: &mut HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    return_type: &ValueType,
    loop_depth: usize,
    span: &Span,
) -> Result<Statement, Diagnostic> {
    let (kind, body) = match variant {
        ForVariant::Infinite { body } => {
            let lowered = lower_block(
                path,
                body,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                loop_depth + 1,
            )?;
            (LoopKind::Infinite, lowered)
        }
        ForVariant::While { condition, body } => {
            if ast_expr_contains_question(condition) {
                let mut condition_scope = scope.clone();
                let mut lowered = Vec::new();
                let (condition, _) = extract_question_exprs(
                    path,
                    condition,
                    &mut condition_scope,
                    imports,
                    signatures,
                    structs,
                    enums,
                    return_type,
                    span,
                    &mut lowered,
                )?;
                let (cond_type, cond) = lower_value_expr(
                    path,
                    &condition,
                    &condition_scope,
                    imports,
                    signatures,
                    structs,
                    enums,
                    span,
                )?;
                if cond_type != ValueType::Bool {
                    return Err(type_mismatch(
                        path,
                        span,
                        format!(
                            "`for` condition must be `bool`, found `{}`",
                            cond_type.name()
                        ),
                    ));
                }
                let body = lower_block(
                    path,
                    body,
                    scope,
                    imports,
                    signatures,
                    structs,
                    enums,
                    return_type,
                    loop_depth + 1,
                )?;
                lowered.push(Statement::If {
                    condition: cond,
                    body,
                    else_body: vec![Statement::Break],
                });
                return Ok(Statement::Loop {
                    kind: LoopKind::Infinite,
                    body: lowered,
                });
            }
            let (cond_type, cond) = lower_value_expr(
                path, condition, scope, imports, signatures, structs, enums, span,
            )?;
            if cond_type != ValueType::Bool {
                return Err(type_mismatch(
                    path,
                    span,
                    format!(
                        "`for` condition must be `bool`, found `{}`",
                        cond_type.name()
                    ),
                ));
            }
            let lowered = lower_block(
                path,
                body,
                scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                loop_depth + 1,
            )?;
            (LoopKind::While(cond), lowered)
        }
        ForVariant::Iterate {
            binding,
            iterable,
            body,
        } => {
            let (iter_type, iterable) = lower_value_expr(
                path, iterable, scope, imports, signatures, structs, enums, span,
            )?;
            let ValueType::Array(element_type) = &iter_type else {
                return Err(type_mismatch(
                    path,
                    span,
                    format!(
                        "`for ... in` requires `Array<T>`, found `{}`",
                        iter_type.name()
                    ),
                ));
            };
            if scope.contains_key(binding) {
                return Err(Diagnostic::new(
                    "E0302",
                    format!("variable `{binding}` is already defined in this scope"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            let element_type = element_type.as_ref().clone();
            let mut loop_scope = scope.clone();
            loop_scope.insert(
                binding.clone(),
                Binding {
                    value_type: element_type.clone(),
                    mutable: false,
                    source: BindingSource::Local,
                },
            );
            let lowered = lower_block(
                path,
                body,
                &mut loop_scope,
                imports,
                signatures,
                structs,
                enums,
                return_type,
                loop_depth + 1,
            )?;
            (
                LoopKind::Iterate {
                    binding: binding.clone(),
                    element_type,
                    iterable,
                },
                lowered,
            )
        }
    };
    Ok(Statement::Loop { kind, body })
}

#[allow(clippy::too_many_arguments)]
fn lower_block(
    path: &Path,
    statements: &[Stmt],
    scope: &mut HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    return_type: &ValueType,
    loop_depth: usize,
) -> Result<Vec<Statement>, Diagnostic> {
    let mut block_scope = scope.clone();
    let mut out = Vec::new();
    for stmt in statements {
        lower_stmt_into(
            path,
            stmt,
            &mut block_scope,
            imports,
            signatures,
            structs,
            enums,
            return_type,
            false,
            loop_depth,
            &mut out,
        )?;
    }
    Ok(out)
}

fn lower_assign_stmt(
    path: &Path,
    target: &[String],
    op: AssignOp,
    value: &AstExpr,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<Statement, Diagnostic> {
    let compound_value = compound_assign_value(target, op, value);
    let value = compound_value.as_ref().unwrap_or(value);
    match target {
        [name] => {
            let Some(binding) = scope.get(name) else {
                return Err(Diagnostic::new(
                    "E0303",
                    format!("unknown variable `{name}`"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            if !binding.mutable {
                let message = format!(
                    "cannot assign to immutable {} `{name}`",
                    binding_source_noun(binding)
                );
                return Err(Diagnostic::new(
                    "E0501",
                    message,
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            let expected_type = binding.value_type.clone();
            let (actual_type, value) = lower_value_expr_with_expected(
                path,
                value,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&expected_type),
                span,
            )?;
            if actual_type != expected_type {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!(
                        "cannot assign `{}` to variable `{name}` of type `{}`",
                        actual_type.name(),
                        expected_type.name()
                    ),
                    &expected_type,
                    &actual_type,
                ));
            }
            Ok(Statement::Assign {
                name: name.clone(),
                value,
            })
        }
        [base, field] => {
            let Some(binding) = scope.get(base) else {
                return Err(Diagnostic::new(
                    "E0303",
                    format!("unknown variable `{base}`"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            if !binding.mutable {
                let message = format!(
                    "cannot assign to field of immutable {} `{base}`",
                    binding_source_noun(binding)
                );
                return Err(Diagnostic::new(
                    "E0501",
                    message,
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            let ValueType::Struct(struct_name, struct_args) = &binding.value_type else {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("`{base}` is not a struct"),
                ));
            };
            let struct_type = structs
                .get(struct_name)
                .expect("struct binding must refer to a known struct");
            let Some(field_type) = struct_type
                .fields
                .iter()
                .find(|item| item.name == *field)
                .map(|item| {
                    substitute_type_params(&item.value_type, &struct_type.type_params, struct_args)
                })
            else {
                return Err(Diagnostic::new(
                    "E0316",
                    format!("struct `{struct_name}` has no field `{field}`"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (actual_type, value) = lower_value_expr_with_expected(
                path,
                value,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&field_type),
                span,
            )?;
            if actual_type != field_type {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!(
                        "cannot assign `{}` to field `{field}` of type `{}`",
                        actual_type.name(),
                        field_type.name()
                    ),
                    &field_type,
                    &actual_type,
                ));
            }
            Ok(Statement::AssignField {
                base: base.clone(),
                field: field.clone(),
                value_type: field_type,
                value,
            })
        }
        _ => Err(Diagnostic::new(
            "E0217",
            "assignment target must be a variable or field",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        )),
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_postfix_stmt(
    path: &Path,
    target: &[String],
    op: PostfixOp,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<Statement, Diagnostic> {
    let assign_op = match op {
        PostfixOp::Increment => AssignOp::Add,
        PostfixOp::Decrement => AssignOp::Subtract,
    };
    lower_assign_stmt(
        path,
        target,
        assign_op,
        &AstExpr::Int(1),
        scope,
        imports,
        signatures,
        structs,
        enums,
        span,
    )
}

fn compound_assign_value(target: &[String], op: AssignOp, value: &AstExpr) -> Option<AstExpr> {
    let op = assign_op_to_binary_op(op)?;
    Some(AstExpr::Binary {
        left: Box::new(AstExpr::Name(target.to_vec())),
        op,
        right: Box::new(value.clone()),
    })
}

fn assign_op_to_binary_op(op: AssignOp) -> Option<AstBinaryOp> {
    match op {
        AssignOp::Assign => None,
        AssignOp::Add => Some(AstBinaryOp::Add),
        AssignOp::Subtract => Some(AstBinaryOp::Subtract),
        AssignOp::Multiply => Some(AstBinaryOp::Multiply),
        AssignOp::Divide => Some(AstBinaryOp::Divide),
        AssignOp::Remainder => Some(AstBinaryOp::Remainder),
        AssignOp::ShiftLeft => Some(AstBinaryOp::ShiftLeft),
        AssignOp::ShiftRight => Some(AstBinaryOp::ShiftRight),
        AssignOp::BitAnd => Some(AstBinaryOp::BitAnd),
        AssignOp::BitXor => Some(AstBinaryOp::BitXor),
        AssignOp::BitOr => Some(AstBinaryOp::BitOr),
        AssignOp::BitAndNot => Some(AstBinaryOp::BitAndNot),
    }
}

fn lower_return_stmt(
    path: &Path,
    value: Option<&AstExpr>,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    return_type: &ValueType,
    span: &Span,
) -> Result<Statement, Diagnostic> {
    match (return_type, value) {
        (ValueType::Void, None) => Ok(Statement::Return(None)),
        (ValueType::Void, Some(_)) => Err(type_mismatch(
            path,
            span,
            "`void` function cannot return a value",
        )),
        (_, None) => Err(type_mismatch(
            path,
            span,
            format!("function must return `{}`", return_type.name()),
        )),
        (expected, Some(value)) => {
            if let AstExpr::Question {
                expr: question_expr,
            } = value
            {
                let (return_ok_type, return_err_type) =
                    result_parts(expected).ok_or_else(|| {
                        Diagnostic::new(
                            "E0421",
                            "`?` requires the current function to return `Result<T, E>`",
                            path,
                            span.line,
                            span.column,
                            span.length,
                            &span.text,
                        )
                    })?;
                let (result_type, result_expr) = lower_value_expr(
                    path,
                    question_expr,
                    scope,
                    imports,
                    signatures,
                    structs,
                    enums,
                    span,
                )?;
                let (ok_type, err_type) = result_parts(&result_type).ok_or_else(|| {
                    Diagnostic::new(
                        "E0420",
                        "`?` can only be used with `Result<T, E>`",
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    )
                })?;
                if ok_type != return_ok_type {
                    return Err(type_mismatch_expected_found(
                        path,
                        span,
                        format!(
                            "`?` unwraps `{}` but function returns `Result<{}, E>`",
                            ok_type.name(),
                            return_ok_type.name()
                        ),
                        &return_ok_type,
                        &ok_type,
                    ));
                }
                if err_type != return_err_type {
                    return Err(type_mismatch_expected_found(
                        path,
                        span,
                        format!(
                            "`?` error type is `{}` but function returns `{}`",
                            err_type.name(),
                            return_err_type.name()
                        ),
                        &return_err_type,
                        &err_type,
                    ));
                }
                return Ok(Statement::QuestionReturnOk {
                    ok_type,
                    result_type,
                    return_type: expected.clone(),
                    result_expr,
                });
            }
            if let Some(question_expr) = question_expr_from_result_ok_return(value, signatures) {
                let (return_ok_type, return_err_type) =
                    result_parts(expected).ok_or_else(|| {
                        Diagnostic::new(
                            "E0421",
                            "`?` requires the current function to return `Result<T, E>`",
                            path,
                            span.line,
                            span.column,
                            span.length,
                            &span.text,
                        )
                    })?;
                let (result_type, result_expr) = lower_value_expr(
                    path,
                    question_expr,
                    scope,
                    imports,
                    signatures,
                    structs,
                    enums,
                    span,
                )?;
                let (ok_type, err_type) = result_parts(&result_type).ok_or_else(|| {
                    Diagnostic::new(
                        "E0420",
                        "`?` can only be used with `Result<T, E>`",
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    )
                })?;
                if ok_type != return_ok_type {
                    return Err(type_mismatch_expected_found(
                        path,
                        span,
                        format!(
                            "`?` unwraps `{}` but returned `Ok` expects `{}`",
                            ok_type.name(),
                            return_ok_type.name()
                        ),
                        &return_ok_type,
                        &ok_type,
                    ));
                }
                if err_type != return_err_type {
                    return Err(type_mismatch_expected_found(
                        path,
                        span,
                        format!(
                            "`?` error type is `{}` but function returns `{}`",
                            err_type.name(),
                            return_err_type.name()
                        ),
                        &return_err_type,
                        &err_type,
                    ));
                }
                return Ok(Statement::QuestionReturnOk {
                    ok_type,
                    result_type,
                    return_type: expected.clone(),
                    result_expr,
                });
            }
            let (actual, lowered) = lower_value_expr_with_expected(
                path,
                value,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(expected),
                span,
            )?;
            if &actual != expected {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!(
                        "return value is `{}` but function expects `{}`",
                        actual.name(),
                        expected.name()
                    ),
                    expected,
                    &actual,
                ));
            }
            Ok(Statement::Return(Some(lowered)))
        }
    }
}

fn question_expr_from_result_ok_return<'a>(
    value: &'a AstExpr,
    signatures: &HashMap<String, FunctionSignature>,
) -> Option<&'a AstExpr> {
    let AstExpr::Call { callee, args, .. } = value else {
        return None;
    };
    if !is_result_ok_callee(callee, signatures) {
        return None;
    }
    let [AstExpr::Question { expr }] = args.as_slice() else {
        return None;
    };
    Some(expr)
}

fn is_result_ok_callee(callee: &[String], signatures: &HashMap<String, FunctionSignature>) -> bool {
    match callee {
        [name] => name == "Ok" && !signatures.contains_key("Ok"),
        [enum_name, variant] => enum_name == "Result" && variant == "Ok",
        _ => false,
    }
}

fn lower_value_expr(
    path: &Path,
    expr: &AstExpr,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    lower_value_expr_with_expected(
        path, expr, scope, imports, signatures, structs, enums, None, span,
    )
}

fn lower_enum_variant_without_payload(
    path: &Path,
    enum_name: &str,
    variant: &str,
    enums: &HashMap<String, EnumType>,
    expected: Option<&ValueType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let Some(enum_type) = enums.get(enum_name) else {
        return Err(Diagnostic::new(
            "E0315",
            format!("unknown prelude enum `{enum_name}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let Some(variant_type) = enum_type.variants.iter().find(|item| item.name == variant) else {
        return Err(Diagnostic::new(
            "E0315",
            format!("enum `{enum_name}` has no variant `{variant}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    if variant_type.payload.is_some() {
        return Err(Diagnostic::new(
            "E0320",
            format!("enum variant `{enum_name}.{variant}` requires a payload"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    let enum_args = match expected {
        Some(ValueType::Enum(expected_name, expected_args)) if expected_name == enum_name => {
            expected_args.clone()
        }
        _ if enum_type.type_params.is_empty() => Vec::new(),
        _ => {
            return Err(Diagnostic::new(
                "E0324",
                format!("generic enum constructor `{enum_name}.{variant}` needs a type annotation"),
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        }
    };
    Ok((
        ValueType::Enum(enum_name.to_string(), enum_args.clone()),
        ValueExpr::EnumVariant {
            enum_name: enum_name.to_string(),
            enum_args,
            variant: variant.to_string(),
            payload: None,
        },
    ))
}

fn lower_enum_variant_with_payload(
    path: &Path,
    enum_name: &str,
    variant: &str,
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    expected: Option<&ValueType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let Some(enum_type) = enums.get(enum_name) else {
        return Err(Diagnostic::new(
            "E0315",
            format!("unknown prelude enum `{enum_name}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let Some(variant_type) = enum_type.variants.iter().find(|item| item.name == variant) else {
        return Err(Diagnostic::new(
            "E0315",
            format!("enum `{enum_name}` has no variant `{variant}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let Some(raw_payload_type) = &variant_type.payload else {
        return Err(Diagnostic::new(
            "E0323",
            format!("enum variant `{enum_name}.{variant}` does not accept a payload"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let [arg] = args else {
        return Err(Diagnostic::new(
            "E0407",
            format!("enum variant `{enum_name}.{variant}` expects exactly one payload"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let enum_args = match expected {
        Some(ValueType::Enum(expected_name, expected_args)) if expected_name == enum_name => {
            expected_args.clone()
        }
        _ if enum_type.type_params.is_empty() => Vec::new(),
        _ => {
            return Err(Diagnostic::new(
                "E0324",
                format!("generic enum constructor `{enum_name}.{variant}` needs a type annotation"),
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        }
    };
    let payload_type = substitute_type_params(raw_payload_type, &enum_type.type_params, &enum_args);
    let (actual_type, payload) = lower_value_expr_with_expected(
        path,
        arg,
        scope,
        imports,
        signatures,
        structs,
        enums,
        Some(&payload_type),
        span,
    )?;
    if actual_type != payload_type {
        return Err(type_mismatch(
            path,
            span,
            format!(
                "payload for `{enum_name}.{variant}` is `{}` but expected `{}`",
                actual_type.name(),
                payload_type.name()
            ),
        ));
    }
    Ok((
        ValueType::Enum(enum_name.to_string(), enum_args.clone()),
        ValueExpr::EnumVariant {
            enum_name: enum_name.to_string(),
            enum_args,
            variant: variant.to_string(),
            payload: Some(Box::new(payload)),
        },
    ))
}

fn lower_value_expr_with_expected(
    path: &Path,
    expr: &AstExpr,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    expected: Option<&ValueType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    match expr {
        AstExpr::String(value) => Ok((ValueType::String, ValueExpr::StringLiteral(value.clone()))),
        AstExpr::Int(value) => lower_int_literal(path, *value, expected, span),
        AstExpr::Float(value) => Ok((ValueType::Float, ValueExpr::FloatLiteral(value.clone()))),
        AstExpr::Char(value) => Ok((ValueType::Char, ValueExpr::CharLiteral(*value))),
        AstExpr::Bool(value) => Ok((ValueType::Bool, ValueExpr::BoolLiteral(*value))),
        AstExpr::Void => Ok((ValueType::Void, ValueExpr::VoidLiteral)),
        AstExpr::MutArg { .. } => Err(Diagnostic::new(
            "E0505",
            "`mut` is only valid in function call arguments",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        )),
        AstExpr::Name(name) if name.len() == 1 => {
            let name = &name[0];
            let Some(binding) = scope.get(name) else {
                if let Some((enum_name, variant)) = core_prelude_variant(name) {
                    return lower_enum_variant_without_payload(
                        path, enum_name, variant, enums, expected, span,
                    );
                }
                return Err(Diagnostic::new(
                    "E0303",
                    format!("unknown variable `{name}`"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let _ = binding.mutable;
            if let BindingSource::EnumPayload { value, variant } = &binding.source {
                return Ok((
                    binding.value_type.clone(),
                    ValueExpr::EnumPayload {
                        value: Box::new(value.clone()),
                        variant: variant.clone(),
                    },
                ));
            }
            Ok((
                binding.value_type.clone(),
                ValueExpr::Variable(name.clone()),
            ))
        }
        AstExpr::Name(name) if name.len() == 2 => {
            let base = &name[0];
            let field = &name[1];
            if let Some(enum_type) = enums.get(base) {
                let Some(variant_type) = enum_type
                    .variants
                    .iter()
                    .find(|variant| variant.name == *field)
                else {
                    return Err(Diagnostic::new(
                        "E0315",
                        format!("enum `{base}` has no variant `{field}`"),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                };
                if variant_type.payload.is_some() {
                    return Err(Diagnostic::new(
                        "E0320",
                        format!("enum variant `{base}.{field}` requires a payload"),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
                let enum_args = match expected {
                    Some(ValueType::Enum(expected_name, expected_args))
                        if expected_name == base =>
                    {
                        expected_args.clone()
                    }
                    _ if enum_type.type_params.is_empty() => Vec::new(),
                    _ => {
                        return Err(Diagnostic::new(
                            "E0324",
                            format!(
                                "generic enum constructor `{base}.{field}` needs a type annotation"
                            ),
                            path,
                            span.line,
                            span.column,
                            span.length,
                            &span.text,
                        ));
                    }
                };
                return Ok((
                    ValueType::Enum(base.clone(), enum_args.clone()),
                    ValueExpr::EnumVariant {
                        enum_name: base.clone(),
                        enum_args,
                        variant: field.clone(),
                        payload: None,
                    },
                ));
            }
            let Some(binding) = scope.get(base) else {
                return Err(Diagnostic::new(
                    "E0303",
                    format!("unknown variable `{base}`"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let ValueType::Struct(type_name, struct_args) = &binding.value_type else {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("`{base}` is not a struct value"),
                ));
            };
            let struct_type = structs
                .get(type_name)
                .expect("struct binding must refer to a known struct");
            let Some(field_type) = struct_type
                .fields
                .iter()
                .find(|item| item.name == *field)
                .map(|item| {
                    substitute_type_params(&item.value_type, &struct_type.type_params, struct_args)
                })
            else {
                return Err(Diagnostic::new(
                    "E0308",
                    format!("struct `{type_name}` has no field `{field}`"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let value = match &binding.source {
                BindingSource::EnumPayload { value, variant } => {
                    ValueExpr::EnumPayloadFieldAccess {
                        value: Box::new(value.clone()),
                        variant: variant.clone(),
                        field: field.clone(),
                    }
                }
                BindingSource::Local | BindingSource::Param => ValueExpr::FieldAccess {
                    base: base.clone(),
                    field: field.clone(),
                },
            };
            Ok((field_type, value))
        }
        AstExpr::Match { value, arms } => {
            let (value_type, lowered_value) = lower_value_expr(
                path, value, scope, imports, signatures, structs, enums, span,
            )?;
            let ValueType::Enum(enum_name, enum_args) = value_type else {
                return Err(type_mismatch(path, span, "`match` expects an enum value"));
            };
            let enum_type = enums
                .get(&enum_name)
                .expect("enum value must refer to a known enum");
            let mut seen = HashMap::new();
            let mut lowered_arms: Vec<MatchValueArm> = Vec::new();
            let mut result_type: Option<ValueType> = expected.cloned();
            for arm in arms {
                let Some(variant) = resolve_match_arm_variant(&arm.pattern, &enum_name, scope)
                else {
                    return Err(Diagnostic::new(
                        "E0316",
                        format!(
                            "match arm must use `{enum_name}.Variant` or a supported prelude variant"
                        ),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                };
                let Some(variant_type) =
                    enum_type.variants.iter().find(|item| item.name == *variant)
                else {
                    return Err(Diagnostic::new(
                        "E0315",
                        format!("enum `{enum_name}` has no variant `{variant}`"),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                };
                let mut arm_scope = scope.clone();
                let payload_type = variant_type.payload.as_ref().map(|payload| {
                    substitute_type_params(payload, &enum_type.type_params, &enum_args)
                });
                match (&payload_type, &arm.binding) {
                    (Some(payload_type), Some(binding)) => {
                        if scope.contains_key(binding) {
                            return Err(Diagnostic::new(
                                "E0302",
                                format!("variable `{binding}` is already defined in this scope"),
                                path,
                                span.line,
                                span.column,
                                span.length,
                                &span.text,
                            ));
                        }
                        arm_scope.insert(
                            binding.clone(),
                            Binding {
                                value_type: payload_type.clone(),
                                mutable: false,
                                source: BindingSource::EnumPayload {
                                    value: lowered_value.clone(),
                                    variant: variant.clone(),
                                },
                            },
                        );
                    }
                    (Some(_), None) => {
                        return Err(Diagnostic::new(
                            "E0321",
                            format!("match arm `{enum_name}.{variant}` must bind its payload"),
                            path,
                            span.line,
                            span.column,
                            span.length,
                            &span.text,
                        ));
                    }
                    (None, Some(_)) => {
                        return Err(Diagnostic::new(
                            "E0322",
                            format!("match arm `{enum_name}.{variant}` has no payload to bind"),
                            path,
                            span.line,
                            span.column,
                            span.length,
                            &span.text,
                        ));
                    }
                    (None, None) => {}
                }
                if seen.insert(variant.clone(), ()).is_some() {
                    return Err(Diagnostic::new(
                        "E0317",
                        format!("duplicate match arm for `{enum_name}.{variant}`"),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
                let (arm_type, mut arm_value) = lower_value_expr_with_expected(
                    path,
                    &arm.value,
                    &arm_scope,
                    imports,
                    signatures,
                    structs,
                    enums,
                    result_type.as_ref(),
                    span,
                )?;
                if let Some(expected_type) = &result_type {
                    if arm_type == ValueType::Never {
                        arm_value = coerce_never_expr(arm_value, expected_type);
                    } else if expected_type != &arm_type {
                        return Err(type_mismatch(
                            path,
                            span,
                            format!(
                                "match arm returns `{}` but previous arms return `{}`",
                                arm_type.name(),
                                expected_type.name()
                            ),
                        ));
                    }
                } else if arm_type == ValueType::Never {
                    // A diverging arm does not determine the match expression type.
                } else {
                    result_type = Some(arm_type.clone());
                    for previous in &mut lowered_arms {
                        previous.value = coerce_never_expr(previous.value.clone(), &arm_type);
                    }
                }
                lowered_arms.push(MatchValueArm {
                    enum_name: enum_name.clone(),
                    enum_args: enum_args.clone(),
                    variant,
                    binding: arm.binding.clone(),
                    value: arm_value,
                });
            }
            for variant in &enum_type.variants {
                if !seen.contains_key(&variant.name) {
                    return Err(Diagnostic::new(
                        "E0318",
                        format!("match is missing arm `{enum_name}.{}`", variant.name),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
            }
            let Some(result_type) = result_type else {
                return Err(Diagnostic::new(
                    "E0319",
                    "`match` must contain at least one non-diverging arm",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            Ok((
                result_type,
                ValueExpr::Match {
                    value: Box::new(lowered_value),
                    arms: lowered_arms,
                },
            ))
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let (condition_type, lowered_condition) = lower_value_expr(
                path, condition, scope, imports, signatures, structs, enums, span,
            )?;
            if condition_type != ValueType::Bool {
                return Err(type_mismatch(path, span, "`if` condition must be `bool`"));
            }

            let (then_type, mut lowered_then) = lower_value_expr_with_expected(
                path,
                then_branch,
                scope,
                imports,
                signatures,
                structs,
                enums,
                expected,
                span,
            )?;
            let else_expected = if then_type == ValueType::Never {
                expected
            } else {
                Some(&then_type)
            };
            let (else_type, mut lowered_else) = lower_value_expr_with_expected(
                path,
                else_branch,
                scope,
                imports,
                signatures,
                structs,
                enums,
                else_expected,
                span,
            )?;
            let result_type = if then_type == ValueType::Never && else_type == ValueType::Never {
                ValueType::Never
            } else if then_type == ValueType::Never {
                lowered_then = coerce_never_expr(lowered_then, &else_type);
                else_type
            } else if else_type == ValueType::Never {
                lowered_else = coerce_never_expr(lowered_else, &then_type);
                then_type
            } else if else_type == then_type {
                then_type
            } else {
                return Err(type_mismatch(
                    path,
                    span,
                    format!(
                        "`if` branches return `{}` and `{}`",
                        then_type.name(),
                        else_type.name()
                    ),
                ));
            };

            Ok((
                result_type,
                ValueExpr::If {
                    condition: Box::new(lowered_condition),
                    then_branch: Box::new(lowered_then),
                    else_branch: Box::new(lowered_else),
                },
            ))
        }
        AstExpr::Panic { message } => {
            let message = lower_panic_message(
                path, message, scope, imports, signatures, structs, enums, span,
            )?;
            let fallback_type = expected.cloned().unwrap_or(ValueType::Never);
            Ok((
                fallback_type.clone(),
                ValueExpr::Panic {
                    message: Box::new(message),
                    fallback_type,
                },
            ))
        }
        AstExpr::Question { .. } => Err(Diagnostic::new(
            "E0422",
            "`?` is currently supported only in statement-level expressions with unconditional evaluation",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        )),
        AstExpr::Cast { expr, target } => {
            let Some(target_type) = parse_value_type(target, structs, enums) else {
                return Err(unsupported_type_diagnostic_from_maps(
                    path,
                    span,
                    target,
                    "unknown cast target type",
                    structs,
                    enums,
                ));
            };
            let (source_type, lowered) =
                lower_value_expr(path, expr, scope, imports, signatures, structs, enums, span)?;
            match (&source_type, &target_type) {
                (source, ValueType::Float) if source.is_integer() => Ok((
                    target_type.clone(),
                    ValueExpr::Cast {
                        expr: Box::new(lowered),
                        target_type,
                    },
                )),
                (ValueType::Float, ValueType::Float) => Ok((
                    target_type.clone(),
                    ValueExpr::Cast {
                        expr: Box::new(lowered),
                        target_type,
                    },
                )),
                (source, target) if source.is_integer() && target.is_integer() => Ok((
                    target_type.clone(),
                    ValueExpr::Cast {
                        expr: Box::new(lowered),
                        target_type,
                    },
                )),
                _ => Err(type_mismatch(
                    path,
                    span,
                    format!(
                        "cannot cast `{}` to `{}`",
                        source_type.name(),
                        target_type.name()
                    ),
                )),
            }
        }
        AstExpr::Unary { op, expr } => {
            let (expr_type, expr) =
                lower_value_expr(path, expr, scope, imports, signatures, structs, enums, span)?;
            let lowered_op = match op {
                AstUnaryOp::Not => UnaryOp::Not,
            };
            match (lowered_op, &expr_type) {
                (UnaryOp::Not, ValueType::Bool) => Ok((
                    ValueType::Bool,
                    ValueExpr::Unary {
                        op: lowered_op,
                        expr: Box::new(expr),
                    },
                )),
                (UnaryOp::Not, _) => Err(type_mismatch(
                    path,
                    span,
                    "`!` expects a bool operand".to_string(),
                )),
            }
        }
        AstExpr::Binary { left, op, right } => {
            let ((left_type, left), (right_type, right)) = lower_binary_operands(
                path, left, right, scope, imports, signatures, structs, enums, span,
            )?;
            let lowered_op = match op {
                AstBinaryOp::LogicalOr => BinaryOp::LogicalOr,
                AstBinaryOp::LogicalAnd => BinaryOp::LogicalAnd,
                AstBinaryOp::Add => BinaryOp::Add,
                AstBinaryOp::Subtract => BinaryOp::Subtract,
                AstBinaryOp::BitOr => BinaryOp::BitOr,
                AstBinaryOp::BitXor => BinaryOp::BitXor,
                AstBinaryOp::Multiply => BinaryOp::Multiply,
                AstBinaryOp::Divide => BinaryOp::Divide,
                AstBinaryOp::Remainder => BinaryOp::Remainder,
                AstBinaryOp::ShiftLeft => BinaryOp::ShiftLeft,
                AstBinaryOp::ShiftRight => BinaryOp::ShiftRight,
                AstBinaryOp::BitAnd => BinaryOp::BitAnd,
                AstBinaryOp::BitAndNot => BinaryOp::BitAndNot,
                AstBinaryOp::Equal => BinaryOp::Equal,
                AstBinaryOp::NotEqual => BinaryOp::NotEqual,
                AstBinaryOp::Less => BinaryOp::Less,
                AstBinaryOp::LessEqual => BinaryOp::LessEqual,
                AstBinaryOp::Greater => BinaryOp::Greater,
                AstBinaryOp::GreaterEqual => BinaryOp::GreaterEqual,
            };
            let value_type = match (lowered_op, &left_type, &right_type) {
                (BinaryOp::LogicalOr | BinaryOp::LogicalAnd, ValueType::Bool, ValueType::Bool) => {
                    ValueType::Bool
                }
                (
                    BinaryOp::Add | BinaryOp::Subtract | BinaryOp::Multiply | BinaryOp::Divide,
                    left_type,
                    right_type,
                ) if numeric_pair_matches(left_type, right_type) => left_type.clone(),
                (BinaryOp::Remainder, left_type, right_type)
                    if left_type == right_type && left_type.is_integer() =>
                {
                    left_type.clone()
                }
                (
                    BinaryOp::BitOr | BinaryOp::BitXor | BinaryOp::BitAnd | BinaryOp::BitAndNot,
                    left_type,
                    right_type,
                ) if left_type == right_type && left_type.is_integer() => left_type.clone(),
                (BinaryOp::ShiftLeft | BinaryOp::ShiftRight, left_type, right_type)
                    if left_type.is_integer() && right_type.is_integer() =>
                {
                    left_type.clone()
                }
                (BinaryOp::Equal | BinaryOp::NotEqual, ValueType::String, ValueType::String)
                | (BinaryOp::Equal | BinaryOp::NotEqual, ValueType::Char, ValueType::Char)
                | (BinaryOp::Equal | BinaryOp::NotEqual, ValueType::Bool, ValueType::Bool) => {
                    ValueType::Bool
                }
                (
                    BinaryOp::Equal
                    | BinaryOp::NotEqual
                    | BinaryOp::Less
                    | BinaryOp::LessEqual
                    | BinaryOp::Greater
                    | BinaryOp::GreaterEqual,
                    ValueType::Int,
                    ValueType::Int,
                )
                | (
                    BinaryOp::Equal
                    | BinaryOp::NotEqual
                    | BinaryOp::Less
                    | BinaryOp::LessEqual
                    | BinaryOp::Greater
                    | BinaryOp::GreaterEqual,
                    ValueType::I32,
                    ValueType::I32,
                )
                | (
                    BinaryOp::Equal
                    | BinaryOp::NotEqual
                    | BinaryOp::Less
                    | BinaryOp::LessEqual
                    | BinaryOp::Greater
                    | BinaryOp::GreaterEqual,
                    ValueType::U32,
                    ValueType::U32,
                )
                | (
                    BinaryOp::Equal
                    | BinaryOp::NotEqual
                    | BinaryOp::Less
                    | BinaryOp::LessEqual
                    | BinaryOp::Greater
                    | BinaryOp::GreaterEqual,
                    ValueType::U64,
                    ValueType::U64,
                )
                | (
                    BinaryOp::Equal
                    | BinaryOp::NotEqual
                    | BinaryOp::Less
                    | BinaryOp::LessEqual
                    | BinaryOp::Greater
                    | BinaryOp::GreaterEqual,
                    ValueType::Float,
                    ValueType::Float,
                ) => ValueType::Bool,
                _ => {
                    let operand_kind = if matches!(
                        lowered_op,
                        BinaryOp::Add
                            | BinaryOp::Subtract
                            | BinaryOp::Multiply
                            | BinaryOp::Divide
                            | BinaryOp::Remainder
                    ) {
                        "numeric"
                    } else if matches!(lowered_op, BinaryOp::LogicalOr | BinaryOp::LogicalAnd) {
                        "bool"
                    } else if matches!(
                        lowered_op,
                        BinaryOp::BitOr
                            | BinaryOp::BitXor
                            | BinaryOp::BitAnd
                            | BinaryOp::BitAndNot
                            | BinaryOp::ShiftLeft
                            | BinaryOp::ShiftRight
                    ) {
                        "integer"
                    } else {
                        "comparable"
                    };
                    return Err(type_mismatch(
                        path,
                        span,
                        format!(
                            "`{}` expects two matching {operand_kind} operands",
                            ast_binary_symbol(op),
                        ),
                    ));
                }
            };
            let value = if left_type == ValueType::String
                && right_type == ValueType::String
                && matches!(lowered_op, BinaryOp::Equal | BinaryOp::NotEqual)
            {
                ValueExpr::StringCompare {
                    left: Box::new(left),
                    op: lowered_op,
                    right: Box::new(right),
                }
            } else {
                ValueExpr::Binary {
                    left: Box::new(left),
                    op: lowered_op,
                    right: Box::new(right),
                    value_type: value_type.clone(),
                }
            };
            Ok((value_type, value))
        }
        AstExpr::Call {
            callee,
            args,
            type_args,
        } if callee.len() == 1 => {
            let name = &callee[0];
            if let Some(qualified) = resolve_specific_value_builtin(name, imports) {
                if qualified == ["Array", "new"] {
                    return lower_array_new(path, type_args, args, structs, enums, span);
                }
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        format!(
                            "standard library function `{name}` does not accept type arguments"
                        ),
                    ));
                }
                if qualified[0] == "string" {
                    return lower_string_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "fs" {
                    return lower_fs_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "io" {
                    return lower_io_builtin(path, &qualified, args, span);
                }
                if qualified[0] == "debug" {
                    return lower_debug_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "log" {
                    return lower_log_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "hash" {
                    return lower_hash_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "crypto" {
                    return lower_crypto_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "json" {
                    return lower_json_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "regex" {
                    return lower_regex_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "collections" {
                    return lower_collections_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "env" {
                    return lower_env_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "process" {
                    return lower_process_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "path" {
                    return lower_path_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "math" {
                    return lower_math_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "char" {
                    return lower_char_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "os" {
                    return lower_os_builtin(path, &qualified, args, span);
                }
                if qualified[0] == "time" {
                    return lower_time_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "testing" {
                    return lower_testing_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "num" {
                    return lower_num_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "option" {
                    return lower_option_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "result" {
                    return lower_result_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
            }
            let Some(template_signature) = signatures.get(name) else {
                if scope.contains_key(name) {
                    return Err(Diagnostic::new(
                        "E0305",
                        format!("local variable `{name}` is not callable"),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
                if let Some((enum_name, variant)) = core_prelude_variant(name) {
                    if !type_args.is_empty() {
                        return Err(type_mismatch(
                            path,
                            span,
                            format!(
                                "enum variant `{enum_name}.{variant}` does not accept type arguments"
                            ),
                        ));
                    }
                    return lower_enum_variant_with_payload(
                        path, enum_name, variant, args, scope, imports, signatures, structs, enums,
                        expected, span,
                    );
                }
                return Err(Diagnostic::new(
                    "E0305",
                    format!("unknown function `{name}`"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (call_name, signature) = if type_args.is_empty() {
                if !template_signature.type_params.is_empty() {
                    return Err(Diagnostic::new(
                        "E0407",
                        format!("generic function `{name}` requires explicit type arguments"),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
                (name.clone(), template_signature.clone())
            } else {
                if template_signature.type_params.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        format!("function `{name}` does not accept type arguments"),
                    ));
                }
                if type_args.len() != template_signature.type_params.len() {
                    return Err(Diagnostic::new(
                        "E0407",
                        format!(
                            "function `{name}` expects {} type argument(s), got {}",
                            template_signature.type_params.len(),
                            type_args.len()
                        ),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
                let instance_args = type_args
                    .iter()
                    .map(|arg| parse_non_void_type(arg, structs, enums))
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| {
                        let type_arg = type_args
                            .iter()
                            .find(|arg| parse_non_void_type(arg, structs, enums).is_none())
                            .expect("at least one type argument failed to lower");
                        unsupported_type_diagnostic_from_maps(
                            path,
                            span,
                            type_arg,
                            format!("unsupported type argument for `{name}`"),
                            structs,
                            enums,
                        )
                    })?;
                (
                    generic_function_instance_name(name, &instance_args),
                    instantiate_function_signature(template_signature, &instance_args),
                )
            };
            if signature.return_type == ValueType::Void
                && !matches!(expected, Some(ValueType::Void))
            {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("function `{call_name}` returns `void` and cannot be used as a value"),
                ));
            }
            if args.len() != signature.params.len() {
                return Err(Diagnostic::new(
                    "E0407",
                    format!(
                        "function `{call_name}` expects {} argument(s), got {}",
                        signature.params.len(),
                        args.len()
                    ),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }

            let mut lowered_args = Vec::new();
            let mut mutable_borrows = Vec::new();
            for (index, (arg, expected)) in args.iter().zip(signature.params.iter()).enumerate() {
                lowered_args.push(lower_call_arg_for_param(
                    path,
                    arg,
                    expected,
                    scope,
                    imports,
                    signatures,
                    structs,
                    enums,
                    span,
                    &call_name,
                    index + 1,
                    &mut mutable_borrows,
                )?);
            }

            Ok((
                signature.return_type.clone(),
                ValueExpr::Call {
                    name: call_name,
                    args: lowered_args,
                },
            ))
        }
        AstExpr::Call {
            callee,
            args,
            type_args,
        } if callee.len() == 2 => {
            if is_io_print_call(callee) {
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        format!(
                            "standard library function `{}` does not accept type arguments",
                            callee.join(".")
                        ),
                    ));
                }
                let Some(function_name) = resolve_io_print_function(callee, imports) else {
                    return Err(missing_io_import_diagnostic(path, span, callee));
                };
                if !matches!(expected, Some(ValueType::Void)) {
                    return Err(type_mismatch(
                        path,
                        span,
                        format!(
                            "function `{}` returns `void` and cannot be used as a value",
                            callee.join(".")
                        ),
                    ));
                }
                let [arg] = args.as_slice() else {
                    return Err(println_type_error(path, span, function_name));
                };
                let (arg_type, lowered) =
                    lower_value_expr(path, arg, scope, imports, signatures, structs, enums, span)?;
                if arg_type != ValueType::String {
                    return Err(println_type_error(path, span, function_name));
                }
                let name = io_print_builtin_expr_name(function_name);
                return Ok((
                    ValueType::Void,
                    ValueExpr::Call {
                        name,
                        args: vec![lowered],
                    },
                ));
            }
            if callee == &["Array", "new"] {
                require_import(path, imports, span, "std.array", "Array.new")?;
                return lower_array_new(path, type_args, args, structs, enums, span);
            }
            if is_string_builtin_call(callee) {
                require_import(path, imports, span, "std.string", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "string builtins do not accept type arguments",
                    ));
                }
                return lower_string_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if callee == &["fs", "read_to_string"]
                || callee == &["fs", "write_string"]
                || callee == &["fs", "read_bytes"]
                || callee == &["fs", "write_bytes"]
                || callee == &["fs", "exists"]
                || callee == &["fs", "metadata"]
                || callee == &["fs", "create_dir"]
                || callee == &["fs", "remove_dir"]
                || callee == &["fs", "read_dir"]
                || callee == &["fs", "open"]
            {
                require_import(path, imports, span, "std.fs", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "fs builtins do not accept type arguments",
                    ));
                }
                return lower_fs_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_io_value_builtin_call(callee) {
                require_import(path, imports, span, "std.io", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "io builtins do not accept type arguments",
                    ));
                }
                return lower_io_builtin(path, callee, args, span);
            }
            if is_debug_builtin_call(callee) {
                require_import(path, imports, span, "std.debug", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "debug builtins do not accept type arguments",
                    ));
                }
                return lower_debug_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_log_builtin_call(callee) {
                require_import(path, imports, span, "std.log", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "log builtins do not accept type arguments",
                    ));
                }
                return lower_log_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_hash_builtin_call(callee) {
                require_import(path, imports, span, "std.hash", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "hash builtins do not accept type arguments",
                    ));
                }
                return lower_hash_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_crypto_builtin_call(callee) {
                require_import(path, imports, span, "std.crypto", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "crypto builtins do not accept type arguments",
                    ));
                }
                return lower_crypto_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_json_builtin_call(callee) {
                require_import(path, imports, span, "std.json", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "json builtins do not accept type arguments",
                    ));
                }
                return lower_json_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_regex_builtin_call(callee) {
                require_import(path, imports, span, "std.regex", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "regex builtins do not accept type arguments",
                    ));
                }
                return lower_regex_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_collections_builtin_call(callee) {
                require_import(path, imports, span, "std.collections", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "collections builtins do not accept type arguments",
                    ));
                }
                return lower_collections_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_env_builtin_call(callee) {
                require_import(path, imports, span, "std.env", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "env builtins do not accept type arguments",
                    ));
                }
                return lower_env_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_process_builtin_call(callee) {
                require_import(path, imports, span, "std.process", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "process builtins do not accept type arguments",
                    ));
                }
                return lower_process_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_path_builtin_call(callee) {
                require_import(path, imports, span, "std.path", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "path builtins do not accept type arguments",
                    ));
                }
                return lower_path_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_math_builtin_call(callee) {
                require_import(path, imports, span, "std.math", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "math builtins do not accept type arguments",
                    ));
                }
                return lower_math_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_char_builtin_call(callee) {
                require_import(path, imports, span, "std.char", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "char builtins do not accept type arguments",
                    ));
                }
                return lower_char_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_os_builtin_call(callee) {
                require_import(path, imports, span, "std.os", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "os builtins do not accept type arguments",
                    ));
                }
                return lower_os_builtin(path, callee, args, span);
            }
            if is_time_builtin_call(callee) {
                require_import(path, imports, span, "std.time", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "time builtins do not accept type arguments",
                    ));
                }
                return lower_time_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_testing_builtin_call(callee) {
                require_import(path, imports, span, "std.testing", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "testing builtins do not accept type arguments",
                    ));
                }
                return lower_testing_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_num_builtin_call(callee) {
                require_import(path, imports, span, "std.num", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "num builtins do not accept type arguments",
                    ));
                }
                return lower_num_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_option_builtin_call(callee) {
                require_option_method_import(path, imports, span, &callee[1])?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "option builtins do not accept type arguments",
                    ));
                }
                return lower_option_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_result_builtin_call(callee) {
                require_result_method_import(path, imports, span, &callee[1])?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "result builtins do not accept type arguments",
                    ));
                }
                return lower_result_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if type_args.is_empty() && is_array_value_method(callee, scope) {
                return lower_array_value_method(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if type_args.is_empty() && is_string_value_method(callee, scope) {
                return lower_string_value_method(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if type_args.is_empty() && is_file_value_method(callee, scope) {
                return lower_file_value_method(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if type_args.is_empty() {
                if let Some(lowered) = lower_option_value_method(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                )? {
                    return Ok(lowered);
                }
                if let Some(lowered) = lower_result_value_method(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                )? {
                    return Ok(lowered);
                }
            }
            if type_args.is_empty() {
                if let Some(lowered) = lower_struct_value_method(
                    path,
                    callee,
                    args,
                    scope,
                    imports,
                    signatures,
                    structs,
                    enums,
                    span,
                    matches!(expected, Some(ValueType::Void)),
                )? {
                    return Ok(lowered);
                }
            }
            if !type_args.is_empty() {
                return Err(type_mismatch(
                    path,
                    span,
                    format!(
                        "function `{}` does not accept type arguments",
                        callee.join(".")
                    ),
                ));
            }
            let enum_name = &callee[0];
            let variant = &callee[1];
            let Some(enum_type) = enums.get(enum_name) else {
                return Err(Diagnostic::new(
                    "E0305",
                    format!("unknown function `{}`", callee.join(".")),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let Some(variant_type) = enum_type.variants.iter().find(|item| item.name == *variant)
            else {
                return Err(Diagnostic::new(
                    "E0315",
                    format!("enum `{enum_name}` has no variant `{variant}`"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let Some(raw_payload_type) = &variant_type.payload else {
                return Err(Diagnostic::new(
                    "E0323",
                    format!("enum variant `{enum_name}.{variant}` does not accept a payload"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let [arg] = args.as_slice() else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("enum variant `{enum_name}.{variant}` expects exactly one payload"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let enum_args = match expected {
                Some(ValueType::Enum(expected_name, expected_args))
                    if expected_name == enum_name =>
                {
                    expected_args.clone()
                }
                _ if enum_type.type_params.is_empty() => Vec::new(),
                _ => {
                    return Err(Diagnostic::new(
                        "E0324",
                        format!(
                            "generic enum constructor `{enum_name}.{variant}` needs a type annotation"
                        ),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
            };
            let payload_type =
                substitute_type_params(raw_payload_type, &enum_type.type_params, &enum_args);
            let (actual_type, payload) = lower_value_expr_with_expected(
                path,
                arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&payload_type),
                span,
            )?;
            if actual_type != payload_type {
                return Err(type_mismatch(
                    path,
                    span,
                    format!(
                        "payload for `{enum_name}.{variant}` is `{}` but expected `{}`",
                        actual_type.name(),
                        payload_type.name()
                    ),
                ));
            }
            Ok((
                ValueType::Enum(enum_name.clone(), enum_args.clone()),
                ValueExpr::EnumVariant {
                    enum_name: enum_name.clone(),
                    enum_args,
                    variant: variant.clone(),
                    payload: Some(Box::new(payload)),
                },
            ))
        }
        AstExpr::StructLiteral { type_name, fields } if type_name.len() == 1 => {
            let type_name = &type_name[0];
            let Some(struct_type) = structs.get(type_name) else {
                return Err(Diagnostic::new(
                    "E0309",
                    format!("unknown struct `{type_name}`"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let struct_args = match expected {
                Some(ValueType::Struct(expected_name, expected_args))
                    if expected_name == type_name =>
                {
                    expected_args.clone()
                }
                _ if struct_type.type_params.is_empty() => Vec::new(),
                _ => {
                    return Err(Diagnostic::new(
                        "E0317",
                        format!("generic struct literal `{type_name}` needs a type annotation"),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
            };
            let mut seen = HashMap::new();
            for (field_name, _) in fields {
                if seen.insert(field_name.clone(), ()).is_some() {
                    return Err(Diagnostic::new(
                        "E0311",
                        format!("field `{field_name}` is specified more than once"),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
            }
            let mut lowered_fields = Vec::new();
            for expected_field in &struct_type.fields {
                let expected_field_type = substitute_type_params(
                    &expected_field.value_type,
                    &struct_type.type_params,
                    &struct_args,
                );
                let Some((_, value)) = fields
                    .iter()
                    .find(|(field_name, _)| field_name == &expected_field.name)
                else {
                    return Err(Diagnostic::new(
                        "E0310",
                        format!(
                            "missing field `{}` for struct `{type_name}`",
                            expected_field.name
                        ),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                };
                let (actual_type, lowered) = lower_value_expr_with_expected(
                    path,
                    value,
                    scope,
                    imports,
                    signatures,
                    structs,
                    enums,
                    Some(&expected_field_type),
                    span,
                )?;
                if actual_type != expected_field_type {
                    return Err(type_mismatch(
                        path,
                        span,
                        format!(
                            "field `{}` is `{}` but expected `{}`",
                            expected_field.name,
                            actual_type.name(),
                            expected_field_type.name()
                        ),
                    ));
                }
                lowered_fields.push((expected_field.name.clone(), lowered));
            }
            for (field_name, _) in fields {
                if !struct_type
                    .fields
                    .iter()
                    .any(|field| field.name == *field_name)
                {
                    return Err(Diagnostic::new(
                        "E0312",
                        format!("struct `{type_name}` has no field `{field_name}`"),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
            }
            Ok((
                ValueType::Struct(type_name.clone(), struct_args.clone()),
                ValueExpr::StructLiteral {
                    type_name: type_name.clone(),
                    struct_args,
                    fields: lowered_fields,
                },
            ))
        }
        _ => Err(Diagnostic::new(
            "E0405",
            "expression is not supported as a value in v0.1 current implementation",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        )),
    }
}

fn lower_panic_message(
    path: &Path,
    message: &AstExpr,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<ValueExpr, Diagnostic> {
    let (message_type, lowered) = lower_value_expr(
        path, message, scope, imports, signatures, structs, enums, span,
    )?;
    if message_type != ValueType::String {
        return Err(type_mismatch(
            path,
            span,
            "`panic` expects a string message",
        ));
    }
    Ok(lowered)
}

fn is_string_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "string"
                && matches!(
                    name.as_str(),
                    "len"
                        | "concat"
                        | "is_empty"
                        | "contains"
                        | "starts_with"
                        | "ends_with"
                        | "split"
                        | "trim"
                        | "to_lower"
                        | "to_upper"
                )
    )
}

fn lower_string_builtin(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    match callee {
        [module, name] if module == "string" && name == "len" => {
            let lowered = lower_string_unary_builtin_arg(
                path, name, args, scope, imports, signatures, structs, enums, span,
            )?;
            Ok((
                ValueType::U64,
                ValueExpr::StringLen {
                    value: Box::new(lowered),
                },
            ))
        }
        [module, name] if module == "string" && name == "concat" => {
            let (lowered_left, lowered_right) = lower_string_binary_builtin_args(
                path, name, args, scope, imports, signatures, structs, enums, span,
            )?;
            Ok((
                ValueType::String,
                ValueExpr::StringConcat {
                    left: Box::new(lowered_left),
                    right: Box::new(lowered_right),
                },
            ))
        }
        [module, name] if module == "string" && name == "is_empty" => {
            let lowered = lower_string_unary_builtin_arg(
                path, name, args, scope, imports, signatures, structs, enums, span,
            )?;
            Ok((
                ValueType::Bool,
                ValueExpr::StringIsEmpty {
                    value: Box::new(lowered),
                },
            ))
        }
        [module, name] if module == "string" && name == "contains" => {
            let (lowered_value, lowered_needle) = lower_string_binary_builtin_args(
                path, name, args, scope, imports, signatures, structs, enums, span,
            )?;
            Ok((
                ValueType::Bool,
                ValueExpr::StringContains {
                    value: Box::new(lowered_value),
                    needle: Box::new(lowered_needle),
                },
            ))
        }
        [module, name] if module == "string" && name == "starts_with" => {
            let (lowered_value, lowered_prefix) = lower_string_binary_builtin_args(
                path, name, args, scope, imports, signatures, structs, enums, span,
            )?;
            Ok((
                ValueType::Bool,
                ValueExpr::StringStartsWith {
                    value: Box::new(lowered_value),
                    prefix: Box::new(lowered_prefix),
                },
            ))
        }
        [module, name] if module == "string" && name == "ends_with" => {
            let (lowered_value, lowered_suffix) = lower_string_binary_builtin_args(
                path, name, args, scope, imports, signatures, structs, enums, span,
            )?;
            Ok((
                ValueType::Bool,
                ValueExpr::StringEndsWith {
                    value: Box::new(lowered_value),
                    suffix: Box::new(lowered_suffix),
                },
            ))
        }
        [module, name] if module == "string" && name == "split" => {
            let (lowered_value, lowered_separator) = lower_string_binary_builtin_args(
                path, name, args, scope, imports, signatures, structs, enums, span,
            )?;
            Ok((
                ValueType::Array(Box::new(ValueType::String)),
                ValueExpr::StringSplit {
                    value: Box::new(lowered_value),
                    separator: Box::new(lowered_separator),
                },
            ))
        }
        [module, name] if module == "string" && name == "trim" => {
            let lowered = lower_string_unary_builtin_arg(
                path, name, args, scope, imports, signatures, structs, enums, span,
            )?;
            Ok((
                ValueType::String,
                ValueExpr::StringTrim {
                    value: Box::new(lowered),
                },
            ))
        }
        [module, name] if module == "string" && name == "to_lower" => {
            let lowered = lower_string_unary_builtin_arg(
                path, name, args, scope, imports, signatures, structs, enums, span,
            )?;
            Ok((
                ValueType::String,
                ValueExpr::StringToLower {
                    value: Box::new(lowered),
                },
            ))
        }
        [module, name] if module == "string" && name == "to_upper" => {
            let lowered = lower_string_unary_builtin_arg(
                path, name, args, scope, imports, signatures, structs, enums, span,
            )?;
            Ok((
                ValueType::String,
                ValueExpr::StringToUpper {
                    value: Box::new(lowered),
                },
            ))
        }
        _ => unreachable!("string builtin dispatcher only passes known calls"),
    }
}

fn lower_string_unary_builtin_arg(
    path: &Path,
    name: &str,
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<ValueExpr, Diagnostic> {
    let [arg] = args else {
        return Err(Diagnostic::new(
            "E0407",
            format!("`string.{name}` expects exactly one string argument"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let (arg_type, lowered) =
        lower_value_expr(path, arg, scope, imports, signatures, structs, enums, span)?;
    if arg_type != ValueType::String {
        return Err(type_mismatch(
            path,
            span,
            format!("`string.{name}` expects a string"),
        ));
    }
    Ok(lowered)
}

fn lower_string_binary_builtin_args(
    path: &Path,
    name: &str,
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueExpr, ValueExpr), Diagnostic> {
    let [left, right] = args else {
        return Err(Diagnostic::new(
            "E0407",
            format!("`string.{name}` expects exactly two string arguments"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let (left_type, lowered_left) =
        lower_value_expr(path, left, scope, imports, signatures, structs, enums, span)?;
    let (right_type, lowered_right) = lower_value_expr(
        path, right, scope, imports, signatures, structs, enums, span,
    )?;
    if left_type != ValueType::String || right_type != ValueType::String {
        return Err(type_mismatch(
            path,
            span,
            format!("`string.{name}` expects two strings"),
        ));
    }
    Ok((lowered_left, lowered_right))
}

fn is_string_value_method(callee: &[String], scope: &HashMap<String, Binding>) -> bool {
    if callee.len() != 2 {
        return false;
    }
    scope
        .get(&callee[0])
        .is_some_and(|binding| binding.value_type == ValueType::String)
}

fn lower_string_value_method(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let receiver = &callee[0];
    let method = &callee[1];
    let binding = scope
        .get(receiver)
        .expect("string method receiver is in scope");
    let receiver_expr = binding_value_expr(receiver, binding);
    require_string_method_import(path, imports, span, method)?;
    match method.as_str() {
        "len" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`string.len` does not accept arguments when called as a method",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::U64,
                ValueExpr::StringLen {
                    value: Box::new(receiver_expr),
                },
            ))
        }
        "concat" => {
            let lowered_other = lower_string_method_arg(
                path, method, args, scope, imports, signatures, structs, enums, span,
            )?;
            Ok((
                ValueType::String,
                ValueExpr::StringConcat {
                    left: Box::new(receiver_expr),
                    right: Box::new(lowered_other),
                },
            ))
        }
        "is_empty" => {
            require_string_method_arity(path, span, method, args, 0)?;
            Ok((
                ValueType::Bool,
                ValueExpr::StringIsEmpty {
                    value: Box::new(receiver_expr),
                },
            ))
        }
        "contains" => {
            let lowered_needle = lower_string_method_arg(
                path, method, args, scope, imports, signatures, structs, enums, span,
            )?;
            Ok((
                ValueType::Bool,
                ValueExpr::StringContains {
                    value: Box::new(receiver_expr),
                    needle: Box::new(lowered_needle),
                },
            ))
        }
        "starts_with" => {
            let lowered_prefix = lower_string_method_arg(
                path, method, args, scope, imports, signatures, structs, enums, span,
            )?;
            Ok((
                ValueType::Bool,
                ValueExpr::StringStartsWith {
                    value: Box::new(receiver_expr),
                    prefix: Box::new(lowered_prefix),
                },
            ))
        }
        "ends_with" => {
            let lowered_suffix = lower_string_method_arg(
                path, method, args, scope, imports, signatures, structs, enums, span,
            )?;
            Ok((
                ValueType::Bool,
                ValueExpr::StringEndsWith {
                    value: Box::new(receiver_expr),
                    suffix: Box::new(lowered_suffix),
                },
            ))
        }
        "split" => {
            let lowered_separator = lower_string_method_arg(
                path, method, args, scope, imports, signatures, structs, enums, span,
            )?;
            Ok((
                ValueType::Array(Box::new(ValueType::String)),
                ValueExpr::StringSplit {
                    value: Box::new(receiver_expr),
                    separator: Box::new(lowered_separator),
                },
            ))
        }
        "trim" => {
            require_string_method_arity(path, span, method, args, 0)?;
            Ok((
                ValueType::String,
                ValueExpr::StringTrim {
                    value: Box::new(receiver_expr),
                },
            ))
        }
        "to_lower" => {
            require_string_method_arity(path, span, method, args, 0)?;
            Ok((
                ValueType::String,
                ValueExpr::StringToLower {
                    value: Box::new(receiver_expr),
                },
            ))
        }
        "to_upper" => {
            require_string_method_arity(path, span, method, args, 0)?;
            Ok((
                ValueType::String,
                ValueExpr::StringToUpper {
                    value: Box::new(receiver_expr),
                },
            ))
        }
        _ => Err(Diagnostic::new(
            "E0305",
            format!("unknown string method `{method}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        )),
    }
}

fn require_string_method_arity(
    path: &Path,
    span: &Span,
    method: &str,
    args: &[AstExpr],
    expected: usize,
) -> Result<(), Diagnostic> {
    if args.len() == expected {
        return Ok(());
    }
    let message = if expected == 0 {
        format!("`string.{method}` does not accept arguments when called as a method")
    } else {
        format!(
            "`string.{method}` expects exactly {expected} string argument(s) when called as a method"
        )
    };
    Err(Diagnostic::new(
        "E0407",
        message,
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    ))
}

fn lower_string_method_arg(
    path: &Path,
    method: &str,
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<ValueExpr, Diagnostic> {
    require_string_method_arity(path, span, method, args, 1)?;
    let (arg_type, lowered_arg) = lower_value_expr(
        path, &args[0], scope, imports, signatures, structs, enums, span,
    )?;
    if arg_type != ValueType::String {
        return Err(type_mismatch_expected_found(
            path,
            span,
            format!("`string.{method}` expects a string argument"),
            &ValueType::String,
            &arg_type,
        ));
    }
    Ok(lowered_arg)
}

fn lower_fs_builtin(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let fs_error = ValueType::Struct("FsError".to_string(), Vec::new());
    match callee {
        [module, name] if module == "fs" && name == "read_to_string" => {
            let [path_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`fs.read_to_string` expects exactly one path string",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (path_type, lowered_path) = lower_value_expr(
                path, path_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if path_type != ValueType::String {
                return Err(type_mismatch(
                    path,
                    span,
                    "`fs.read_to_string` expects a string path",
                ));
            }
            Ok((
                ValueType::Enum("Result".to_string(), vec![ValueType::String, fs_error]),
                ValueExpr::FsReadToString {
                    path: Box::new(lowered_path),
                },
            ))
        }
        [module, name] if module == "fs" && name == "write_string" => {
            let [path_arg, content_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`fs.write_string` expects path and content strings",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (path_type, lowered_path) = lower_value_expr(
                path, path_arg, scope, imports, signatures, structs, enums, span,
            )?;
            let (content_type, lowered_content) = lower_value_expr(
                path,
                content_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                span,
            )?;
            if path_type != ValueType::String || content_type != ValueType::String {
                return Err(type_mismatch(
                    path,
                    span,
                    "`fs.write_string` expects string path and content",
                ));
            }
            Ok((
                ValueType::Enum("Result".to_string(), vec![ValueType::Void, fs_error]),
                ValueExpr::FsWriteString {
                    path: Box::new(lowered_path),
                    content: Box::new(lowered_content),
                },
            ))
        }
        [module, name] if module == "fs" && name == "read_bytes" => {
            let [path_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`fs.read_bytes` expects exactly one path string",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (path_type, lowered_path) = lower_value_expr(
                path, path_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if path_type != ValueType::String {
                return Err(type_mismatch(
                    path,
                    span,
                    "`fs.read_bytes` expects a string path",
                ));
            }
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![ValueType::Array(Box::new(ValueType::U32)), fs_error],
                ),
                ValueExpr::FsReadBytes {
                    path: Box::new(lowered_path),
                },
            ))
        }
        [module, name] if module == "fs" && name == "write_bytes" => {
            let [path_arg, bytes_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`fs.write_bytes` expects a path string and Array<u32> bytes",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (path_type, lowered_path) = lower_value_expr(
                path, path_arg, scope, imports, signatures, structs, enums, span,
            )?;
            let (bytes_type, lowered_bytes) = lower_value_expr(
                path, bytes_arg, scope, imports, signatures, structs, enums, span,
            )?;
            let expected_bytes = ValueType::Array(Box::new(ValueType::U32));
            if path_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`fs.write_bytes` expects a string path",
                    &ValueType::String,
                    &path_type,
                ));
            }
            if bytes_type != expected_bytes {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`fs.write_bytes` expects Array<u32> bytes",
                    &expected_bytes,
                    &bytes_type,
                ));
            }
            Ok((
                ValueType::Enum("Result".to_string(), vec![ValueType::Void, fs_error]),
                ValueExpr::FsWriteBytes {
                    path: Box::new(lowered_path),
                    bytes: Box::new(lowered_bytes),
                },
            ))
        }
        [module, name] if module == "fs" && name == "exists" => {
            let [path_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`fs.exists` expects exactly one path string",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (path_type, lowered_path) = lower_value_expr(
                path, path_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if path_type != ValueType::String {
                return Err(type_mismatch(
                    path,
                    span,
                    "`fs.exists` expects a string path",
                ));
            }
            Ok((
                ValueType::Bool,
                ValueExpr::FsExists {
                    path: Box::new(lowered_path),
                },
            ))
        }
        [module, name] if module == "fs" && name == "metadata" => {
            let [path_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`fs.metadata` expects exactly one path string",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (path_type, lowered_path) = lower_value_expr(
                path, path_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if path_type != ValueType::String {
                return Err(type_mismatch(
                    path,
                    span,
                    "`fs.metadata` expects a string path",
                ));
            }
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![
                        ValueType::Struct("FileMetadata".to_string(), Vec::new()),
                        fs_error,
                    ],
                ),
                ValueExpr::FsMetadata {
                    path: Box::new(lowered_path),
                },
            ))
        }
        [module, name] if module == "fs" && (name == "create_dir" || name == "remove_dir") => {
            let [path_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`fs.{name}` expects exactly one path string"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (path_type, lowered_path) = lower_value_expr(
                path, path_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if path_type != ValueType::String {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("`fs.{name}` expects a string path"),
                ));
            }
            let expr = if name == "create_dir" {
                ValueExpr::FsCreateDir {
                    path: Box::new(lowered_path),
                }
            } else {
                ValueExpr::FsRemoveDir {
                    path: Box::new(lowered_path),
                }
            };
            Ok((
                ValueType::Enum("Result".to_string(), vec![ValueType::Void, fs_error]),
                expr,
            ))
        }
        [module, name] if module == "fs" && name == "read_dir" => {
            let [path_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`fs.read_dir` expects exactly one path string",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (path_type, lowered_path) = lower_value_expr(
                path, path_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if path_type != ValueType::String {
                return Err(type_mismatch(
                    path,
                    span,
                    "`fs.read_dir` expects a string path",
                ));
            }
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![
                        ValueType::Array(Box::new(ValueType::String)),
                        ValueType::Struct("FsError".to_string(), Vec::new()),
                    ],
                ),
                ValueExpr::FsReadDir {
                    path: Box::new(lowered_path),
                },
            ))
        }
        [module, name] if module == "fs" && name == "open" => {
            let [path_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`fs.open` expects exactly one path string",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (path_type, lowered_path) = lower_value_expr(
                path, path_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if path_type != ValueType::String {
                return Err(type_mismatch(path, span, "`fs.open` expects a string path"));
            }
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![ValueType::Struct("File".to_string(), Vec::new()), fs_error],
                ),
                ValueExpr::FsOpen {
                    path: Box::new(lowered_path),
                },
            ))
        }
        _ => unreachable!("fs builtin dispatcher only passes known calls"),
    }
}

fn lower_io_builtin(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    match callee {
        [module, name] if module == "io" && name == "read_line" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`io.read_line` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![
                        ValueType::String,
                        ValueType::Struct("IoError".to_string(), Vec::new()),
                    ],
                ),
                ValueExpr::IoReadLine,
            ))
        }
        _ => unreachable!("io builtin dispatcher only passes known calls"),
    }
}

fn lower_env_builtin(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    match callee {
        [module, name] if module == "env" && name == "get" => {
            let [name_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`env.get` expects exactly one environment variable name",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (name_type, lowered_name) = lower_value_expr(
                path, name_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if name_type != ValueType::String {
                return Err(type_mismatch(
                    path,
                    span,
                    "`env.get` expects a string variable name",
                ));
            }
            Ok((
                ValueType::Enum("Option".to_string(), vec![ValueType::String]),
                ValueExpr::EnvGet {
                    name: Box::new(lowered_name),
                },
            ))
        }
        [module, name] if module == "env" && name == "set" => {
            let [name_arg, value_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`env.set` expects exactly a name and value string",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (name_type, lowered_name) = lower_value_expr(
                path, name_arg, scope, imports, signatures, structs, enums, span,
            )?;
            let (value_type, lowered_value) = lower_value_expr(
                path, value_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if name_type != ValueType::String || value_type != ValueType::String {
                return Err(type_mismatch(
                    path,
                    span,
                    "`env.set` expects two string arguments",
                ));
            }
            Ok((
                ValueType::Void,
                ValueExpr::EnvSet {
                    name: Box::new(lowered_name),
                    value: Box::new(lowered_value),
                },
            ))
        }
        [module, name] if module == "env" && name == "cwd" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`env.cwd` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((ValueType::String, ValueExpr::EnvCwd))
        }
        [module, name] if module == "env" && name == "home_dir" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`env.home_dir` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::Enum("Option".to_string(), vec![ValueType::String]),
                ValueExpr::EnvHomeDir,
            ))
        }
        [module, name] if module == "env" && name == "temp_dir" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`env.temp_dir` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((ValueType::String, ValueExpr::EnvTempDir))
        }
        [module, name] if module == "env" && name == "args" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`env.args` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::Array(Box::new(ValueType::String)),
                ValueExpr::EnvArgs,
            ))
        }
        _ => unreachable!("env builtin dispatcher only passes known calls"),
    }
}

fn lower_process_builtin(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let process_error = ValueType::Struct("ProcessError".to_string(), Vec::new());
    match callee {
        [module, name] if module == "process" && name == "exit" => {
            let [code_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`process.exit` expects exactly one i64 exit code",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (code_type, lowered_code) = lower_value_expr(
                path, code_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if code_type != ValueType::Int {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`process.exit` expects an i64 exit code",
                    &ValueType::Int,
                    &code_type,
                ));
            }
            Ok((
                ValueType::Void,
                ValueExpr::ProcessExit {
                    code: Box::new(lowered_code),
                },
            ))
        }
        [module, name]
            if module == "process"
                && (name == "spawn" || name == "status" || name == "exec" || name == "output") =>
        {
            let [command_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`process.{name}` expects exactly one command string"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (command_type, lowered_command) = lower_value_expr(
                path,
                command_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                span,
            )?;
            if command_type != ValueType::String {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("`process.{name}` expects a string command"),
                ));
            }
            if name == "spawn" {
                Ok((
                    ValueType::Enum("Result".to_string(), vec![ValueType::I32, process_error]),
                    ValueExpr::ProcessSpawn {
                        command: Box::new(lowered_command),
                    },
                ))
            } else if name == "status" {
                Ok((
                    ValueType::Enum("Result".to_string(), vec![ValueType::I32, process_error]),
                    ValueExpr::ProcessStatus {
                        command: Box::new(lowered_command),
                    },
                ))
            } else if name == "exec" {
                Ok((
                    ValueType::Enum("Result".to_string(), vec![ValueType::String, process_error]),
                    ValueExpr::ProcessExec {
                        command: Box::new(lowered_command),
                    },
                ))
            } else {
                Ok((
                    ValueType::Enum(
                        "Result".to_string(),
                        vec![
                            ValueType::Struct("ProcessOutput".to_string(), Vec::new()),
                            process_error,
                        ],
                    ),
                    ValueExpr::ProcessOutput {
                        command: Box::new(lowered_command),
                    },
                ))
            }
        }
        _ => unreachable!("process builtin dispatcher only passes known calls"),
    }
}

fn is_env_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "env"
                && matches!(
                    name.as_str(),
                    "args" | "get" | "set" | "cwd" | "home_dir" | "temp_dir"
                )
    )
}

fn is_io_value_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name] if module == "io" && matches!(name.as_str(), "read_line")
    )
}

fn is_debug_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "debug"
                && matches!(name.as_str(), "print" | "println" | "panic" | "backtrace")
    )
}

fn is_log_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "log"
                && matches!(name.as_str(), "debug" | "info" | "warn" | "error" | "enabled")
    )
}

fn lower_log_builtin(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let [module, name] = callee else {
        unreachable!("log builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "log");
    match name.as_str() {
        "enabled" => {
            let [level_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`log.enabled` expects exactly one string level",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (level_type, level) = lower_value_expr(
                path, level_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if level_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`log.enabled` expects a string level",
                    &ValueType::String,
                    &level_type,
                ));
            }
            Ok((
                ValueType::Bool,
                ValueExpr::LogEnabled {
                    level: Box::new(level),
                },
            ))
        }
        "debug" | "info" | "warn" | "error" => {
            let [message_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`log.{name}` expects exactly one string message"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (message_type, message) = lower_value_expr(
                path,
                message_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                span,
            )?;
            if message_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!("`log.{name}` expects a string message"),
                    &ValueType::String,
                    &message_type,
                ));
            }
            Ok((ValueType::Void, log_statement_expr(name, message)))
        }
        _ => unreachable!("log builtin dispatcher only passes known calls"),
    }
}

fn log_statement_expr(level: &str, message: ValueExpr) -> ValueExpr {
    let prefix = ValueExpr::StringLiteral(format!("[{level}] "));
    ValueExpr::If {
        condition: Box::new(ValueExpr::LogEnabled {
            level: Box::new(ValueExpr::StringLiteral(level.to_string())),
        }),
        then_branch: Box::new(ValueExpr::Call {
            name: BUILTIN_EPRINTLN_EXPR.to_string(),
            args: vec![ValueExpr::StringConcat {
                left: Box::new(prefix),
                right: Box::new(message),
            }],
        }),
        else_branch: Box::new(ValueExpr::VoidLiteral),
    }
}

fn is_hash_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "hash"
                && matches!(name.as_str(), "new" | "string" | "write_string" | "finish")
    )
}

fn is_crypto_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "crypto" && matches!(name.as_str(), "sha256" | "sha512" | "random_bytes")
    )
}

fn is_json_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name] if module == "json" && matches!(name.as_str(), "parse" | "stringify")
    )
}

fn is_regex_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "regex" && matches!(name.as_str(), "compile" | "is_match" | "captures")
    )
}

fn is_collections_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "collections"
                && matches!(
                    name.as_str(),
                    "map_new"
                        | "map_len"
                        | "map_get"
                        | "map_contains"
                        | "map_set"
                        | "map_remove"
                        | "set_new"
                        | "set_len"
                        | "set_contains"
                        | "set_insert"
                        | "set_remove"
                )
    )
}

fn lower_hash_builtin(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let [module, name] = callee else {
        unreachable!("hash builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "hash");
    match name.as_str() {
        "new" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`hash.new` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::Struct("HashState".to_string(), Vec::new()),
                ValueExpr::HashNew,
            ))
        }
        "string" => {
            let [value_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`hash.string` expects exactly one string value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (value_type, value) = lower_value_expr(
                path, value_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if value_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`hash.string` expects a string value",
                    &ValueType::String,
                    &value_type,
                ));
            }
            Ok((
                ValueType::U64,
                ValueExpr::HashString {
                    value: Box::new(value),
                },
            ))
        }
        "write_string" => {
            let [state_arg, value_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`hash.write_string` expects a HashState and string value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (state_type, state) = lower_value_expr(
                path, state_arg, scope, imports, signatures, structs, enums, span,
            )?;
            let expected_state = ValueType::Struct("HashState".to_string(), Vec::new());
            if state_type != expected_state {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`hash.write_string` expects a HashState value",
                    &expected_state,
                    &state_type,
                ));
            }
            let (value_type, value) = lower_value_expr(
                path, value_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if value_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`hash.write_string` expects a string value",
                    &ValueType::String,
                    &value_type,
                ));
            }
            Ok((
                ValueType::Struct("HashState".to_string(), Vec::new()),
                ValueExpr::HashWriteString {
                    state: Box::new(state),
                    value: Box::new(value),
                },
            ))
        }
        "finish" => {
            let [state_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`hash.finish` expects exactly one HashState value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (state_type, state) = lower_value_expr(
                path, state_arg, scope, imports, signatures, structs, enums, span,
            )?;
            let expected_state = ValueType::Struct("HashState".to_string(), Vec::new());
            if state_type != expected_state {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`hash.finish` expects a HashState value",
                    &expected_state,
                    &state_type,
                ));
            }
            Ok((
                ValueType::U64,
                ValueExpr::HashFinish {
                    state: Box::new(state),
                },
            ))
        }
        _ => unreachable!("hash builtin dispatcher only passes known calls"),
    }
}

fn lower_crypto_builtin(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let [module, name] = callee else {
        unreachable!("crypto builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "crypto");
    if name == "random_bytes" {
        let [count_arg] = args else {
            return Err(Diagnostic::new(
                "E0407",
                "`crypto.random_bytes` expects exactly one u64 count",
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        };
        let (count_type, count) = lower_value_expr(
            path, count_arg, scope, imports, signatures, structs, enums, span,
        )?;
        if count_type != ValueType::U64 {
            return Err(type_mismatch_expected_found(
                path,
                span,
                "`crypto.random_bytes` expects a u64 count",
                &ValueType::U64,
                &count_type,
            ));
        }
        return Ok((
            ValueType::Array(Box::new(ValueType::U32)),
            ValueExpr::CryptoRandomBytes {
                count: Box::new(count),
            },
        ));
    }
    let [value_arg] = args else {
        return Err(Diagnostic::new(
            "E0407",
            format!("`crypto.{name}` expects exactly one string value"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let (value_type, value) = lower_value_expr(
        path, value_arg, scope, imports, signatures, structs, enums, span,
    )?;
    if value_type != ValueType::String {
        return Err(type_mismatch_expected_found(
            path,
            span,
            format!("`crypto.{name}` expects a string value"),
            &ValueType::String,
            &value_type,
        ));
    }
    let expr = match name.as_str() {
        "sha256" => ValueExpr::CryptoSha256 {
            value: Box::new(value),
        },
        "sha512" => ValueExpr::CryptoSha512 {
            value: Box::new(value),
        },
        _ => unreachable!("crypto builtin dispatcher only passes known calls"),
    };
    Ok((ValueType::String, expr))
}

fn lower_json_builtin(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let [module, name] = callee else {
        unreachable!("json builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "json");
    let [value_arg] = args else {
        return Err(Diagnostic::new(
            "E0407",
            format!("`json.{name}` expects exactly one argument"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let (value_type, value) = lower_value_expr(
        path, value_arg, scope, imports, signatures, structs, enums, span,
    )?;
    match name.as_str() {
        "parse" => {
            if value_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`json.parse` expects a string value",
                    &ValueType::String,
                    &value_type,
                ));
            }
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![
                        ValueType::Struct("JsonValue".to_string(), Vec::new()),
                        ValueType::Struct("JsonError".to_string(), Vec::new()),
                    ],
                ),
                ValueExpr::JsonParse {
                    value: Box::new(value),
                },
            ))
        }
        "stringify" => {
            let json_value = ValueType::Struct("JsonValue".to_string(), Vec::new());
            if value_type != json_value {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`json.stringify` expects a JsonValue value",
                    &json_value,
                    &value_type,
                ));
            }
            Ok((
                ValueType::String,
                ValueExpr::JsonStringify {
                    value: Box::new(value),
                },
            ))
        }
        _ => unreachable!("json builtin dispatcher only passes known calls"),
    }
}

fn lower_regex_builtin(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let [module, name] = callee else {
        unreachable!("regex builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "regex");
    let regex_type = ValueType::Struct("Regex".to_string(), Vec::new());
    let regex_error = ValueType::Struct("RegexError".to_string(), Vec::new());
    match name.as_str() {
        "compile" => {
            let [pattern_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`regex.compile` expects exactly one string pattern",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (pattern_type, pattern) = lower_value_expr(
                path,
                pattern_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                span,
            )?;
            if pattern_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`regex.compile` expects a string pattern",
                    &ValueType::String,
                    &pattern_type,
                ));
            }
            Ok((
                ValueType::Enum("Result".to_string(), vec![regex_type.clone(), regex_error]),
                ValueExpr::RegexCompile {
                    pattern: Box::new(pattern),
                },
            ))
        }
        "is_match" | "captures" => {
            let [regex_arg, value_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`regex.{name}` expects a Regex and string value"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (actual_regex_type, regex) = lower_value_expr(
                path, regex_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if actual_regex_type != regex_type {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!("`regex.{name}` expects a Regex value"),
                    &regex_type,
                    &actual_regex_type,
                ));
            }
            let (value_type, value) = lower_value_expr(
                path, value_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if value_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!("`regex.{name}` expects a string value"),
                    &ValueType::String,
                    &value_type,
                ));
            }
            if name == "is_match" {
                Ok((
                    ValueType::Bool,
                    ValueExpr::RegexIsMatch {
                        regex: Box::new(regex),
                        value: Box::new(value),
                    },
                ))
            } else {
                Ok((
                    ValueType::Enum(
                        "Option".to_string(),
                        vec![ValueType::Array(Box::new(ValueType::String))],
                    ),
                    ValueExpr::RegexCaptures {
                        regex: Box::new(regex),
                        value: Box::new(value),
                    },
                ))
            }
        }
        _ => unreachable!("regex builtin dispatcher only passes known calls"),
    }
}

fn lower_collections_builtin(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let [module, name] = callee else {
        unreachable!("collections builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "collections");
    let map_type = ValueType::Struct("StringMap".to_string(), Vec::new());
    let set_type = ValueType::Struct("StringSet".to_string(), Vec::new());
    match name.as_str() {
        "map_new" => {
            expect_no_args(path, span, "collections.map_new", args)?;
            Ok((map_type, ValueExpr::CollectionsStringMapNew))
        }
        "map_len" => {
            let map = lower_collections_unary_arg(
                path,
                span,
                "collections.map_len",
                args,
                &map_type,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )?;
            Ok((
                ValueType::U64,
                ValueExpr::CollectionsStringMapLen { map: Box::new(map) },
            ))
        }
        "map_get" => {
            let (map, key) = lower_collections_map_key_args(
                path,
                span,
                "collections.map_get",
                args,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )?;
            Ok((
                ValueType::Enum("Option".to_string(), vec![ValueType::String]),
                ValueExpr::CollectionsStringMapGet {
                    map: Box::new(map),
                    key: Box::new(key),
                },
            ))
        }
        "map_contains" => {
            let (map, key) = lower_collections_map_key_args(
                path,
                span,
                "collections.map_contains",
                args,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )?;
            Ok((
                ValueType::Bool,
                ValueExpr::CollectionsStringMapContains {
                    map: Box::new(map),
                    key: Box::new(key),
                },
            ))
        }
        "map_set" => {
            let [map_arg, key_arg, value_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`collections.map_set` expects a StringMap, string key, and string value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let map = lower_collections_arg(
                path,
                span,
                "collections.map_set",
                map_arg,
                &map_type,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )?;
            let key = lower_collections_arg(
                path,
                span,
                "collections.map_set",
                key_arg,
                &ValueType::String,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )?;
            let value = lower_collections_arg(
                path,
                span,
                "collections.map_set",
                value_arg,
                &ValueType::String,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )?;
            Ok((
                map_type,
                ValueExpr::CollectionsStringMapSet {
                    map: Box::new(map),
                    key: Box::new(key),
                    value: Box::new(value),
                },
            ))
        }
        "map_remove" => {
            let (map, key) = lower_collections_map_key_args(
                path,
                span,
                "collections.map_remove",
                args,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )?;
            Ok((
                map_type,
                ValueExpr::CollectionsStringMapRemove {
                    map: Box::new(map),
                    key: Box::new(key),
                },
            ))
        }
        "set_new" => {
            expect_no_args(path, span, "collections.set_new", args)?;
            Ok((set_type, ValueExpr::CollectionsStringSetNew))
        }
        "set_len" => {
            let set = lower_collections_unary_arg(
                path,
                span,
                "collections.set_len",
                args,
                &set_type,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )?;
            Ok((
                ValueType::U64,
                ValueExpr::CollectionsStringSetLen { set: Box::new(set) },
            ))
        }
        "set_contains" => {
            let (set, value) = lower_collections_set_value_args(
                path,
                span,
                "collections.set_contains",
                args,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )?;
            Ok((
                ValueType::Bool,
                ValueExpr::CollectionsStringSetContains {
                    set: Box::new(set),
                    value: Box::new(value),
                },
            ))
        }
        "set_insert" => {
            let (set, value) = lower_collections_set_value_args(
                path,
                span,
                "collections.set_insert",
                args,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )?;
            Ok((
                set_type,
                ValueExpr::CollectionsStringSetInsert {
                    set: Box::new(set),
                    value: Box::new(value),
                },
            ))
        }
        "set_remove" => {
            let (set, value) = lower_collections_set_value_args(
                path,
                span,
                "collections.set_remove",
                args,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )?;
            Ok((
                set_type,
                ValueExpr::CollectionsStringSetRemove {
                    set: Box::new(set),
                    value: Box::new(value),
                },
            ))
        }
        _ => unreachable!("collections builtin dispatcher only passes known calls"),
    }
}

fn expect_no_args(
    path: &Path,
    span: &Span,
    callable: &str,
    args: &[AstExpr],
) -> Result<(), Diagnostic> {
    if args.is_empty() {
        return Ok(());
    }
    Err(Diagnostic::new(
        "E0407",
        format!("`{callable}` does not accept arguments"),
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    ))
}

#[allow(clippy::too_many_arguments)]
fn lower_collections_unary_arg(
    path: &Path,
    span: &Span,
    callable: &str,
    args: &[AstExpr],
    expected: &ValueType,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Result<ValueExpr, Diagnostic> {
    let [arg] = args else {
        return Err(Diagnostic::new(
            "E0407",
            format!("`{callable}` expects exactly one argument"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    lower_collections_arg(
        path, span, callable, arg, expected, scope, imports, signatures, structs, enums,
    )
}

#[allow(clippy::too_many_arguments)]
fn lower_collections_map_key_args(
    path: &Path,
    span: &Span,
    callable: &str,
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Result<(ValueExpr, ValueExpr), Diagnostic> {
    let [map_arg, key_arg] = args else {
        return Err(Diagnostic::new(
            "E0407",
            format!("`{callable}` expects a StringMap and string key"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let map_type = ValueType::Struct("StringMap".to_string(), Vec::new());
    let map = lower_collections_arg(
        path, span, callable, map_arg, &map_type, scope, imports, signatures, structs, enums,
    )?;
    let key = lower_collections_arg(
        path,
        span,
        callable,
        key_arg,
        &ValueType::String,
        scope,
        imports,
        signatures,
        structs,
        enums,
    )?;
    Ok((map, key))
}

#[allow(clippy::too_many_arguments)]
fn lower_collections_set_value_args(
    path: &Path,
    span: &Span,
    callable: &str,
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Result<(ValueExpr, ValueExpr), Diagnostic> {
    let [set_arg, value_arg] = args else {
        return Err(Diagnostic::new(
            "E0407",
            format!("`{callable}` expects a StringSet and string value"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let set_type = ValueType::Struct("StringSet".to_string(), Vec::new());
    let set = lower_collections_arg(
        path, span, callable, set_arg, &set_type, scope, imports, signatures, structs, enums,
    )?;
    let value = lower_collections_arg(
        path,
        span,
        callable,
        value_arg,
        &ValueType::String,
        scope,
        imports,
        signatures,
        structs,
        enums,
    )?;
    Ok((set, value))
}

#[allow(clippy::too_many_arguments)]
fn lower_collections_arg(
    path: &Path,
    span: &Span,
    callable: &str,
    arg: &AstExpr,
    expected: &ValueType,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Result<ValueExpr, Diagnostic> {
    let (actual, lowered) = lower_value_expr_with_expected(
        path,
        arg,
        scope,
        imports,
        signatures,
        structs,
        enums,
        Some(expected),
        span,
    )?;
    if &actual != expected {
        return Err(type_mismatch_expected_found(
            path,
            span,
            format!(
                "`{callable}` argument is `{}` but expected `{}`",
                actual.name(),
                expected.name()
            ),
            expected,
            &actual,
        ));
    }
    Ok(lowered)
}

fn lower_debug_builtin(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let [module, name] = callee else {
        unreachable!("debug builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "debug");
    match name.as_str() {
        "print" | "println" | "panic" => {
            let [message_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`debug.{name}` expects exactly one string message"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (message_type, message) = lower_value_expr(
                path,
                message_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                span,
            )?;
            if message_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!("`debug.{name}` expects a string message"),
                    &ValueType::String,
                    &message_type,
                ));
            }
            let value = match name.as_str() {
                "print" => ValueExpr::Call {
                    name: BUILTIN_EPRINT_EXPR.to_string(),
                    args: vec![message],
                },
                "println" => ValueExpr::Call {
                    name: BUILTIN_EPRINTLN_EXPR.to_string(),
                    args: vec![message],
                },
                "panic" => ValueExpr::Panic {
                    message: Box::new(message),
                    fallback_type: ValueType::Void,
                },
                _ => unreachable!("debug string helper matched above"),
            };
            Ok((ValueType::Void, value))
        }
        "backtrace" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`debug.backtrace` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::String,
                ValueExpr::StringLiteral("backtrace unavailable".to_string()),
            ))
        }
        _ => unreachable!("debug builtin dispatcher only passes known calls"),
    }
}

fn is_process_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "process"
                && matches!(name.as_str(), "exit" | "spawn" | "status" | "exec" | "output")
    )
}

fn is_testing_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "testing"
                && matches!(name.as_str(), "assert" | "assert_equal" | "assert_error")
    )
}

fn lower_testing_builtin(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let [module, name] = callee else {
        unreachable!("testing builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "testing");
    match name.as_str() {
        "assert" => {
            let [condition_arg, message_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`testing.assert` expects a bool condition and string message",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (condition_type, condition) = lower_value_expr(
                path,
                condition_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                span,
            )?;
            if condition_type != ValueType::Bool {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`testing.assert` expects a bool condition",
                    &ValueType::Bool,
                    &condition_type,
                ));
            }
            let (message_type, message) = lower_value_expr(
                path,
                message_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                span,
            )?;
            if message_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`testing.assert` expects a string message",
                    &ValueType::String,
                    &message_type,
                ));
            }
            Ok((ValueType::Void, assert_expr(condition, message)))
        }
        "assert_equal" => {
            let [left_arg, right_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`testing.assert_equal` expects two comparable values",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (left_type, left) = lower_value_expr(
                path, left_arg, scope, imports, signatures, structs, enums, span,
            )?;
            let (right_type, right) = lower_value_expr(
                path, right_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if left_type != right_type {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`testing.assert_equal` expects both values to have the same type",
                    &left_type,
                    &right_type,
                ));
            }
            let condition = equality_expr(left, right, &left_type).ok_or_else(|| {
                type_mismatch(
                    path,
                    span,
                    format!(
                        "`testing.assert_equal` does not support values of type `{}`",
                        left_type.name()
                    ),
                )
            })?;
            Ok((
                ValueType::Void,
                assert_expr(
                    condition,
                    ValueExpr::StringLiteral("assert_equal failed".to_string()),
                ),
            ))
        }
        "assert_error" => {
            let [result_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`testing.assert_error` expects one Result<T, E> value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (result_type, result) = lower_value_expr(
                path, result_arg, scope, imports, signatures, structs, enums, span,
            )?;
            let ValueType::Enum(enum_name, enum_args) = result_type.clone() else {
                return Err(type_mismatch(
                    path,
                    span,
                    "`testing.assert_error` expects a Result<T, E> value",
                ));
            };
            if enum_name != "Result" || enum_args.len() != 2 {
                return Err(type_mismatch(
                    path,
                    span,
                    "`testing.assert_error` expects a Result<T, E> value",
                ));
            }
            let condition = ValueExpr::ResultIsErr {
                result: Box::new(result),
                ok_type: enum_args[0].clone(),
                err_type: enum_args[1].clone(),
            };
            Ok((
                ValueType::Void,
                assert_expr(
                    condition,
                    ValueExpr::StringLiteral("expected Result.Err".to_string()),
                ),
            ))
        }
        _ => unreachable!("testing builtin dispatcher only passes known calls"),
    }
}

fn assert_expr(condition: ValueExpr, message: ValueExpr) -> ValueExpr {
    ValueExpr::If {
        condition: Box::new(condition),
        then_branch: Box::new(ValueExpr::VoidLiteral),
        else_branch: Box::new(ValueExpr::Panic {
            message: Box::new(message),
            fallback_type: ValueType::Void,
        }),
    }
}

fn equality_expr(left: ValueExpr, right: ValueExpr, value_type: &ValueType) -> Option<ValueExpr> {
    match value_type {
        ValueType::String => Some(ValueExpr::StringCompare {
            left: Box::new(left),
            op: BinaryOp::Equal,
            right: Box::new(right),
        }),
        ValueType::Char
        | ValueType::Bool
        | ValueType::Int
        | ValueType::I32
        | ValueType::U64
        | ValueType::Float => Some(ValueExpr::Binary {
            left: Box::new(left),
            op: BinaryOp::Equal,
            right: Box::new(right),
            value_type: value_type.clone(),
        }),
        _ => None,
    }
}

fn is_path_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "path"
                && matches!(
                    name.as_str(),
                    "join" | "basename" | "dirname" | "extension" | "normalize" | "is_absolute"
                )
    )
}

fn lower_path_builtin(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let [module, name] = callee else {
        unreachable!("path builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "path");
    match name.as_str() {
        "join" => {
            let [left, right] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`path.join` expects exactly two string arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (left_type, lowered_left) =
                lower_value_expr(path, left, scope, imports, signatures, structs, enums, span)?;
            let (right_type, lowered_right) = lower_value_expr(
                path, right, scope, imports, signatures, structs, enums, span,
            )?;
            if left_type != ValueType::String || right_type != ValueType::String {
                return Err(type_mismatch(path, span, "`path.join` expects two strings"));
            }
            Ok((
                ValueType::String,
                ValueExpr::PathJoin {
                    left: Box::new(lowered_left),
                    right: Box::new(lowered_right),
                },
            ))
        }
        "basename" | "dirname" | "extension" | "normalize" | "is_absolute" => {
            let [path_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`path.{name}` expects exactly one string argument"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (path_type, lowered_path) = lower_value_expr(
                path, path_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if path_type != ValueType::String {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("`path.{name}` expects a string"),
                ));
            }
            let return_type = if name == "is_absolute" {
                ValueType::Bool
            } else {
                ValueType::String
            };
            let lowered = match name.as_str() {
                "basename" => ValueExpr::PathBasename {
                    path: Box::new(lowered_path),
                },
                "dirname" => ValueExpr::PathDirname {
                    path: Box::new(lowered_path),
                },
                "extension" => ValueExpr::PathExtension {
                    path: Box::new(lowered_path),
                },
                "normalize" => ValueExpr::PathNormalize {
                    path: Box::new(lowered_path),
                },
                "is_absolute" => ValueExpr::PathIsAbsolute {
                    path: Box::new(lowered_path),
                },
                _ => unreachable!("path builtin dispatcher only passes known calls"),
            };
            Ok((return_type, lowered))
        }
        _ => unreachable!("path builtin dispatcher only passes known calls"),
    }
}

fn is_math_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "math"
                && matches!(
                    name.as_str(),
                    "abs"
                        | "min"
                        | "max"
                        | "floor"
                        | "ceil"
                        | "round"
                        | "sqrt"
                        | "pow"
                        | "sin"
                        | "cos"
                )
    )
}

fn is_char_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "char"
                && matches!(
                    name.as_str(),
                    "is_digit" | "is_alpha" | "is_whitespace" | "to_string"
                )
    )
}

fn is_os_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "os"
                && matches!(
                    name.as_str(),
                    "platform" | "arch" | "path_separator" | "line_ending"
                )
    )
}

fn is_time_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "time"
                && matches!(
                    name.as_str(),
                    "now_millis"
                        | "monotonic_millis"
                        | "duration_millis"
                        | "duration_seconds"
                        | "duration_as_millis"
                        | "format_duration"
                        | "sleep"
                        | "sleep_millis"
                )
    )
}

fn is_num_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "num"
                && matches!(
                    name.as_str(),
                    "parse_i64"
                        | "parse_u64"
                        | "parse_f64"
                        | "to_string"
                        | "checked_add"
                        | "checked_sub"
                        | "checked_mul"
                        | "wrapping_add"
                        | "wrapping_sub"
                        | "wrapping_mul"
                )
    )
}

fn lower_os_builtin(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let [module, name] = callee else {
        unreachable!("os builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "os");
    if !args.is_empty() {
        return Err(Diagnostic::new(
            "E0407",
            format!("`os.{name}` does not accept arguments"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    let expr = match name.as_str() {
        "platform" => ValueExpr::OsPlatform,
        "arch" => ValueExpr::OsArch,
        "path_separator" => ValueExpr::OsPathSeparator,
        "line_ending" => ValueExpr::OsLineEnding,
        _ => unreachable!("os builtin dispatcher only passes known calls"),
    };
    Ok((ValueType::String, expr))
}

fn lower_time_builtin(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let [module, name] = callee else {
        unreachable!("time builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "time");
    match name.as_str() {
        "now_millis" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`time.now_millis` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((ValueType::Int, ValueExpr::TimeNowMillis))
        }
        "monotonic_millis" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`time.monotonic_millis` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((ValueType::Int, ValueExpr::TimeMonotonicMillis))
        }
        "duration_millis" => {
            let [millis] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`time.duration_millis` expects exactly one i64 millisecond value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (millis_type, lowered_millis) = lower_value_expr(
                path, millis, scope, imports, signatures, structs, enums, span,
            )?;
            if millis_type != ValueType::Int {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`time.duration_millis` expects an i64 millisecond value",
                    &ValueType::Int,
                    &millis_type,
                ));
            }
            Ok((
                ValueType::Struct("Duration".to_string(), Vec::new()),
                ValueExpr::TimeDurationMillis {
                    millis: Box::new(lowered_millis),
                },
            ))
        }
        "duration_seconds" => {
            let [seconds] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`time.duration_seconds` expects exactly one i64 second value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (seconds_type, lowered_seconds) = lower_value_expr(
                path, seconds, scope, imports, signatures, structs, enums, span,
            )?;
            if seconds_type != ValueType::Int {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`time.duration_seconds` expects an i64 second value",
                    &ValueType::Int,
                    &seconds_type,
                ));
            }
            Ok((
                ValueType::Struct("Duration".to_string(), Vec::new()),
                ValueExpr::TimeDurationSeconds {
                    seconds: Box::new(lowered_seconds),
                },
            ))
        }
        "duration_as_millis" => {
            let [duration] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`time.duration_as_millis` expects exactly one Duration value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (duration_type, lowered_duration) = lower_value_expr(
                path, duration, scope, imports, signatures, structs, enums, span,
            )?;
            let expected = ValueType::Struct("Duration".to_string(), Vec::new());
            if duration_type != expected {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`time.duration_as_millis` expects a Duration value",
                    &expected,
                    &duration_type,
                ));
            }
            Ok((
                ValueType::Int,
                ValueExpr::TimeDurationAsMillis {
                    duration: Box::new(lowered_duration),
                },
            ))
        }
        "format_duration" => {
            let [duration] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`time.format_duration` expects exactly one Duration value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (duration_type, lowered_duration) = lower_value_expr(
                path, duration, scope, imports, signatures, structs, enums, span,
            )?;
            let expected = ValueType::Struct("Duration".to_string(), Vec::new());
            if duration_type != expected {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`time.format_duration` expects a Duration value",
                    &expected,
                    &duration_type,
                ));
            }
            Ok((
                ValueType::String,
                ValueExpr::TimeFormatDuration {
                    duration: Box::new(lowered_duration),
                },
            ))
        }
        "sleep" => {
            let [duration] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`time.sleep` expects exactly one Duration value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (duration_type, lowered_duration) = lower_value_expr(
                path, duration, scope, imports, signatures, structs, enums, span,
            )?;
            let expected = ValueType::Struct("Duration".to_string(), Vec::new());
            if duration_type != expected {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`time.sleep` expects a Duration value",
                    &expected,
                    &duration_type,
                ));
            }
            Ok((
                ValueType::Void,
                ValueExpr::TimeSleep {
                    duration: Box::new(lowered_duration),
                },
            ))
        }
        "sleep_millis" => {
            let [duration] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`time.sleep_millis` expects exactly one i64 duration in milliseconds",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (duration_type, lowered_duration) = lower_value_expr(
                path, duration, scope, imports, signatures, structs, enums, span,
            )?;
            if duration_type != ValueType::Int {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`time.sleep_millis` expects an i64 duration in milliseconds",
                    &ValueType::Int,
                    &duration_type,
                ));
            }
            Ok((
                ValueType::Void,
                ValueExpr::TimeSleepMillis {
                    duration: Box::new(lowered_duration),
                },
            ))
        }
        _ => unreachable!("time builtin dispatcher only passes known calls"),
    }
}

fn lower_num_builtin(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let [module, name] = callee else {
        unreachable!("num builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "num");
    let num_error = ValueType::Struct("NumError".to_string(), Vec::new());
    match name.as_str() {
        "parse_i64" | "parse_u64" | "parse_f64" => {
            let [value] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`num.{name}` expects exactly one argument"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (value_type, lowered_value) = lower_value_expr(
                path, value, scope, imports, signatures, structs, enums, span,
            )?;
            if value_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!("`num.{name}` expects a string argument"),
                    &ValueType::String,
                    &value_type,
                ));
            }
            let (ok_type, expr) = match name.as_str() {
                "parse_i64" => (
                    ValueType::Int,
                    ValueExpr::NumParseI64 {
                        value: Box::new(lowered_value),
                    },
                ),
                "parse_u64" => (
                    ValueType::U64,
                    ValueExpr::NumParseU64 {
                        value: Box::new(lowered_value),
                    },
                ),
                "parse_f64" => (
                    ValueType::Float,
                    ValueExpr::NumParseF64 {
                        value: Box::new(lowered_value),
                    },
                ),
                _ => unreachable!("num parse dispatcher only passes known calls"),
            };
            Ok((
                ValueType::Enum("Result".to_string(), vec![ok_type, num_error]),
                expr,
            ))
        }
        "to_string" => {
            let [value] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`num.to_string` expects exactly one argument",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (value_type, lowered_value) = lower_value_expr(
                path, value, scope, imports, signatures, structs, enums, span,
            )?;
            if !matches!(
                value_type,
                ValueType::Int
                    | ValueType::I32
                    | ValueType::U32
                    | ValueType::U64
                    | ValueType::Float
            ) {
                return Err(type_mismatch(
                    path,
                    span,
                    "`num.to_string` expects an i64, i32, u32, u64, or f64 value",
                ));
            }
            Ok((
                ValueType::String,
                ValueExpr::NumToString {
                    value: Box::new(lowered_value),
                    value_type,
                },
            ))
        }
        "checked_add" | "checked_sub" | "checked_mul" | "wrapping_add" | "wrapping_sub"
        | "wrapping_mul" => {
            let [left, right] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`num.{name}` expects exactly two integer arguments"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let ((left_type, lowered_left), (right_type, lowered_right)) = lower_binary_operands(
                path, left, right, scope, imports, signatures, structs, enums, span,
            )?;
            if left_type != right_type || !left_type.is_integer() {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("`num.{name}` expects two matching integer operands"),
                ));
            }
            let op = match name.as_str() {
                "checked_add" | "wrapping_add" => BinaryOp::Add,
                "checked_sub" | "wrapping_sub" => BinaryOp::Subtract,
                "checked_mul" | "wrapping_mul" => BinaryOp::Multiply,
                _ => unreachable!("num binary dispatcher only passes known calls"),
            };
            let function = if name.starts_with("checked_") {
                NumBinaryFunction::Checked
            } else {
                NumBinaryFunction::Wrapping
            };
            let result_type = if function == NumBinaryFunction::Checked {
                ValueType::Enum("Option".to_string(), vec![left_type.clone()])
            } else {
                left_type.clone()
            };
            Ok((
                result_type,
                ValueExpr::NumBinary {
                    function,
                    op,
                    left: Box::new(lowered_left),
                    right: Box::new(lowered_right),
                    value_type: left_type,
                },
            ))
        }
        _ => unreachable!("num builtin dispatcher only passes known calls"),
    }
}

fn lower_char_builtin(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let [module, name] = callee else {
        unreachable!("char builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "char");
    let [value] = args else {
        return Err(Diagnostic::new(
            "E0407",
            format!("`char.{name}` expects exactly one char argument"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let (value_type, lowered) = lower_value_expr(
        path, value, scope, imports, signatures, structs, enums, span,
    )?;
    if value_type != ValueType::Char {
        return Err(type_mismatch_expected_found(
            path,
            span,
            format!("`char.{name}` expects a char value"),
            &ValueType::Char,
            &value_type,
        ));
    }
    let expr = match name.as_str() {
        "is_digit" => ValueExpr::CharIsDigit {
            value: Box::new(lowered),
        },
        "is_alpha" => ValueExpr::CharIsAlpha {
            value: Box::new(lowered),
        },
        "is_whitespace" => ValueExpr::CharIsWhitespace {
            value: Box::new(lowered),
        },
        "to_string" => ValueExpr::CharToString {
            value: Box::new(lowered),
        },
        _ => unreachable!("char builtin dispatcher only passes known calls"),
    };
    let return_type = if name == "to_string" {
        ValueType::String
    } else {
        ValueType::Bool
    };
    Ok((return_type, expr))
}

fn lower_math_builtin(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let [module, name] = callee else {
        unreachable!("math builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "math");
    match name.as_str() {
        "abs" => {
            let [value] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`math.abs` expects exactly one numeric argument",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (value_type, lowered) = lower_value_expr(
                path, value, scope, imports, signatures, structs, enums, span,
            )?;
            if !value_type.is_numeric() {
                return Err(type_mismatch(
                    path,
                    span,
                    "`math.abs` expects a numeric value",
                ));
            }
            Ok((
                value_type.clone(),
                ValueExpr::MathUnary {
                    function: MathUnaryFunction::Abs,
                    value: Box::new(lowered),
                    value_type,
                },
            ))
        }
        "floor" | "ceil" | "round" | "sqrt" | "sin" | "cos" => {
            let [value] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`math.{name}` expects exactly one f64 argument"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (value_type, lowered) = lower_value_expr(
                path, value, scope, imports, signatures, structs, enums, span,
            )?;
            if value_type != ValueType::Float {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!("`math.{name}` expects an f64 value"),
                    &ValueType::Float,
                    &value_type,
                ));
            }
            let function = match name.as_str() {
                "floor" => MathUnaryFunction::Floor,
                "ceil" => MathUnaryFunction::Ceil,
                "round" => MathUnaryFunction::Round,
                "sqrt" => MathUnaryFunction::Sqrt,
                "sin" => MathUnaryFunction::Sin,
                "cos" => MathUnaryFunction::Cos,
                _ => unreachable!("math builtin dispatcher only passes known calls"),
            };
            Ok((
                ValueType::Float,
                ValueExpr::MathUnary {
                    function,
                    value: Box::new(lowered),
                    value_type: ValueType::Float,
                },
            ))
        }
        "min" | "max" => {
            let [left, right] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`math.{name}` expects exactly two matching numeric arguments"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (left_type, lowered_left) =
                lower_value_expr(path, left, scope, imports, signatures, structs, enums, span)?;
            let (right_type, lowered_right) = lower_value_expr(
                path, right, scope, imports, signatures, structs, enums, span,
            )?;
            if left_type != right_type || !left_type.is_numeric() {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("`math.{name}` expects two matching numeric values"),
                ));
            }
            let function = if name == "min" {
                MathBinaryFunction::Min
            } else {
                MathBinaryFunction::Max
            };
            Ok((
                left_type.clone(),
                ValueExpr::MathBinary {
                    function,
                    left: Box::new(lowered_left),
                    right: Box::new(lowered_right),
                    value_type: left_type,
                },
            ))
        }
        "pow" => {
            let [left, right] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`math.pow` expects exactly two f64 arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (left_type, lowered_left) =
                lower_value_expr(path, left, scope, imports, signatures, structs, enums, span)?;
            let (right_type, lowered_right) = lower_value_expr(
                path, right, scope, imports, signatures, structs, enums, span,
            )?;
            if left_type != ValueType::Float || right_type != ValueType::Float {
                return Err(type_mismatch(
                    path,
                    span,
                    "`math.pow` expects two f64 values",
                ));
            }
            Ok((
                ValueType::Float,
                ValueExpr::MathBinary {
                    function: MathBinaryFunction::Pow,
                    left: Box::new(lowered_left),
                    right: Box::new(lowered_right),
                    value_type: ValueType::Float,
                },
            ))
        }
        _ => unreachable!("math builtin dispatcher only passes known calls"),
    }
}

fn lower_array_new(
    path: &Path,
    type_args: &[crate::ast::TypeRef],
    args: &[AstExpr],
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let [type_arg] = type_args else {
        return Err(Diagnostic::new(
            "E0407",
            "`Array.new` expects exactly one type argument",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    if !args.is_empty() {
        return Err(Diagnostic::new(
            "E0407",
            "`Array.new<T>()` does not accept value arguments",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    let element_type = parse_value_type(type_arg, structs, enums).ok_or_else(|| {
        unsupported_type_diagnostic_from_maps(
            path,
            span,
            type_arg,
            "unsupported Array element type",
            structs,
            enums,
        )
    })?;
    ensure_supported_array_element(path, &element_type, span)?;
    Ok((
        ValueType::Array(Box::new(element_type.clone())),
        ValueExpr::ArrayNew { element_type },
    ))
}

fn is_array_value_method(callee: &[String], scope: &HashMap<String, Binding>) -> bool {
    if callee.len() != 2 {
        return false;
    }
    scope
        .get(&callee[0])
        .is_some_and(|binding| matches!(binding.value_type, ValueType::Array(_)))
}

fn lower_array_value_method(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let name = &callee[0];
    let method = &callee[1];
    require_array_method_import(path, imports, span, method)?;
    let binding = scope.get(name).expect("array method receiver is in scope");
    let receiver_expr = binding_value_expr(name, binding);
    let ValueType::Array(element_type) = &binding.value_type else {
        unreachable!("array method dispatcher only passes arrays");
    };
    ensure_supported_array_element(path, element_type, span)?;
    match method.as_str() {
        "len" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`Array.len` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::U64,
                ValueExpr::ArrayLen {
                    array: Box::new(receiver_expr),
                },
            ))
        }
        "iter" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`Array.iter` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::Array(Box::new(element_type.as_ref().clone())),
                ValueExpr::ArrayIter {
                    array: Box::new(receiver_expr),
                    element_type: element_type.as_ref().clone(),
                },
            ))
        }
        "get" => {
            let [index] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`Array.get` expects exactly one index",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (index_type, lowered_index) = lower_value_expr_with_expected(
                path,
                index,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&ValueType::U64),
                span,
            )?;
            if index_type != ValueType::U64 {
                return Err(type_mismatch(path, span, "`Array.get` index must be `u64`"));
            }
            Ok((
                ValueType::Enum("Option".to_string(), vec![element_type.as_ref().clone()]),
                ValueExpr::ArrayGet {
                    array: Box::new(receiver_expr),
                    index: Box::new(lowered_index),
                    element_type: element_type.as_ref().clone(),
                },
            ))
        }
        "pop" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`Array.pop` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            if !binding.mutable {
                return Err(Diagnostic::new(
                    "E0501",
                    format!(
                        "cannot call mutating Array method on immutable {} `{name}`",
                        binding_source_noun(binding)
                    ),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::Enum("Option".to_string(), vec![element_type.as_ref().clone()]),
                ValueExpr::ArrayPop {
                    array: name.clone(),
                    element_type: element_type.as_ref().clone(),
                },
            ))
        }
        "remove" => {
            let [index] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`Array.remove` expects exactly one index",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            if !binding.mutable {
                return Err(Diagnostic::new(
                    "E0501",
                    format!(
                        "cannot call mutating Array method on immutable {} `{name}`",
                        binding_source_noun(binding)
                    ),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            let (index_type, lowered_index) = lower_value_expr_with_expected(
                path,
                index,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&ValueType::U64),
                span,
            )?;
            if index_type != ValueType::U64 {
                return Err(type_mismatch(
                    path,
                    span,
                    "`Array.remove` index must be `u64`",
                ));
            }
            Ok((
                ValueType::Enum("Option".to_string(), vec![element_type.as_ref().clone()]),
                ValueExpr::ArrayRemove {
                    array: name.clone(),
                    index: Box::new(lowered_index),
                    element_type: element_type.as_ref().clone(),
                },
            ))
        }
        _ => Err(Diagnostic::new(
            "E0305",
            format!("unknown Array method `{method}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        )),
    }
}

fn is_file_value_method(callee: &[String], scope: &HashMap<String, Binding>) -> bool {
    if callee.len() != 2 {
        return false;
    }
    scope.get(&callee[0]).is_some_and(|binding| {
        binding.value_type == ValueType::Struct("File".to_string(), Vec::new())
    })
}

fn lower_file_value_method(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let name = &callee[0];
    let method = &callee[1];
    let binding = scope.get(name).expect("file method receiver is in scope");
    let receiver_expr = binding_value_expr(name, binding);
    let fs_error = ValueType::Struct("FsError".to_string(), Vec::new());
    match method.as_str() {
        "close" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`File.close` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::Void,
                ValueExpr::FileClose {
                    file: Box::new(receiver_expr),
                },
            ))
        }
        "read_to_string" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`File.read_to_string` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::Enum("Result".to_string(), vec![ValueType::String, fs_error]),
                ValueExpr::FileReadToString {
                    file: Box::new(receiver_expr),
                },
            ))
        }
        "write_string" => {
            let [content_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`File.write_string` expects exactly one content string",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (content_type, lowered_content) = lower_value_expr_with_expected(
                path,
                content_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&ValueType::String),
                span,
            )?;
            if content_type != ValueType::String {
                return Err(type_mismatch(
                    path,
                    span,
                    "`File.write_string` expects string content",
                ));
            }
            Ok((
                ValueType::Enum("Result".to_string(), vec![ValueType::Void, fs_error]),
                ValueExpr::FileWriteString {
                    file: Box::new(receiver_expr),
                    content: Box::new(lowered_content),
                },
            ))
        }
        _ => Err(Diagnostic::new(
            "E0305",
            format!("unknown File method `{method}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        )),
    }
}

fn is_option_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "option"
                && matches!(
                    name.as_str(),
                    "is_some" | "is_none" | "unwrap_or" | "map" | "and_then"
                )
    )
}

fn is_result_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "result"
                && matches!(
                    name.as_str(),
                    "is_ok" | "is_err" | "unwrap_or" | "map" | "map_err" | "and_then"
                )
    )
}

fn lower_option_builtin(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let [module, method] = callee else {
        unreachable!("option builtin dispatcher only passes qualified calls");
    };
    debug_assert_eq!(module, "option");
    match method.as_str() {
        "is_some" | "is_none" => {
            let [option] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`option.{method}` expects exactly one Option argument"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (option_type, lowered_option) = lower_value_expr(
                path, option, scope, imports, signatures, structs, enums, span,
            )?;
            let payload_type = option_payload(&option_type).ok_or_else(|| {
                type_mismatch(
                    path,
                    span,
                    format!("`option.{method}` expects an Option value"),
                )
            })?;
            let value = if method == "is_some" {
                ValueExpr::OptionIsSome {
                    option: Box::new(lowered_option),
                    payload_type,
                }
            } else {
                ValueExpr::OptionIsNone {
                    option: Box::new(lowered_option),
                    payload_type,
                }
            };
            Ok((ValueType::Bool, value))
        }
        "unwrap_or" => {
            let [option, default] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`option.unwrap_or` expects an Option value and a default value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (option_type, lowered_option) = lower_value_expr(
                path, option, scope, imports, signatures, structs, enums, span,
            )?;
            let payload_type = option_payload(&option_type).ok_or_else(|| {
                type_mismatch(path, span, "`option.unwrap_or` expects an Option value")
            })?;
            if payload_type == ValueType::Void {
                return Err(type_mismatch(
                    path,
                    span,
                    "`option.unwrap_or` does not support Option<void>",
                ));
            }
            let (default_type, lowered_default) = lower_value_expr_with_expected(
                path,
                default,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&payload_type),
                span,
            )?;
            if default_type != payload_type {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!(
                        "`option.unwrap_or` default is `{}` but payload is `{}`",
                        default_type.name(),
                        payload_type.name()
                    ),
                    &payload_type,
                    &default_type,
                ));
            }
            Ok((
                payload_type.clone(),
                ValueExpr::OptionUnwrapOr {
                    option: Box::new(lowered_option),
                    default: Box::new(lowered_default),
                    payload_type,
                },
            ))
        }
        "map" | "and_then" => {
            let [option, converter] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`option.{method}` expects an Option value and a converter function"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (option_type, lowered_option) = lower_value_expr(
                path, option, scope, imports, signatures, structs, enums, span,
            )?;
            let source_type = option_payload(&option_type).ok_or_else(|| {
                type_mismatch(
                    path,
                    span,
                    format!("`option.{method}` expects an Option value"),
                )
            })?;
            lower_option_converter_call(
                path,
                span,
                method,
                lowered_option,
                source_type,
                converter,
                signatures,
            )
        }
        _ => unreachable!("option builtin dispatcher only passes known calls"),
    }
}

fn lower_option_value_method(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<Option<(ValueType, ValueExpr)>, Diagnostic> {
    if callee.len() != 2 {
        return Ok(None);
    }
    let receiver_name = &callee[0];
    let method = &callee[1];
    if !matches!(
        method.as_str(),
        "is_some" | "is_none" | "unwrap_or" | "map" | "and_then"
    ) {
        return Ok(None);
    }
    let Some(binding) = scope.get(receiver_name) else {
        return Ok(None);
    };
    let Some(payload_type) = option_payload(&binding.value_type) else {
        if matches!(method.as_str(), "unwrap_or" | "map" | "and_then")
            && result_parts(&binding.value_type).is_some()
        {
            return Ok(None);
        }
        return Err(type_mismatch(
            path,
            span,
            format!("`{receiver_name}.{method}` expects an Option value"),
        ));
    };
    require_option_method_import(path, imports, span, method)?;
    let option = binding_value_expr(receiver_name, binding);
    match method.as_str() {
        "is_some" => Ok(Some((
            ValueType::Bool,
            ValueExpr::OptionIsSome {
                option: Box::new(option),
                payload_type,
            },
        ))),
        "is_none" => Ok(Some((
            ValueType::Bool,
            ValueExpr::OptionIsNone {
                option: Box::new(option),
                payload_type,
            },
        ))),
        "unwrap_or" => {
            let [default] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`Option.unwrap_or` expects exactly one default value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            if payload_type == ValueType::Void {
                return Err(type_mismatch(
                    path,
                    span,
                    "`Option.unwrap_or` does not support Option<void>",
                ));
            }
            let (default_type, lowered_default) = lower_value_expr_with_expected(
                path,
                default,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&payload_type),
                span,
            )?;
            if default_type != payload_type {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!(
                        "`Option.unwrap_or` default is `{}` but payload is `{}`",
                        default_type.name(),
                        payload_type.name()
                    ),
                    &payload_type,
                    &default_type,
                ));
            }
            Ok(Some((
                payload_type.clone(),
                ValueExpr::OptionUnwrapOr {
                    option: Box::new(option),
                    default: Box::new(lowered_default),
                    payload_type,
                },
            )))
        }
        "map" | "and_then" => {
            let [converter] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`Option.{method}` expects exactly one converter function"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            lower_option_converter_call(
                path,
                span,
                method,
                option,
                payload_type,
                converter,
                signatures,
            )
            .map(Some)
        }
        _ => unreachable!("option method dispatcher only passes known calls"),
    }
}

fn lower_option_converter_call(
    path: &Path,
    span: &Span,
    method: &str,
    option: ValueExpr,
    source_type: ValueType,
    converter: &AstExpr,
    signatures: &HashMap<String, FunctionSignature>,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let converter_name = option_converter_name(path, span, method, converter)?;
    let converter_signature =
        option_converter_signature(path, span, method, &converter_name, signatures)?;
    let [converter_param] = converter_signature.params.as_slice() else {
        return Err(Diagnostic::new(
            "E0407",
            format!("converter function `{converter_name}` must take exactly one argument"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    if converter_param.value_type != source_type {
        return Err(type_mismatch_expected_found(
            path,
            span,
            format!(
                "`Option.{method}` converter `{converter_name}` takes `{}` but payload is `{}`",
                converter_param.value_type.name(),
                source_type.name()
            ),
            &source_type,
            &converter_param.value_type,
        ));
    }
    match method {
        "map" => {
            if converter_signature.return_type == ValueType::Void {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("converter function `{converter_name}` must return a mapped value"),
                ));
            }
            let target_type = converter_signature.return_type.clone();
            Ok((
                ValueType::Enum("Option".to_string(), vec![target_type.clone()]),
                ValueExpr::OptionMap {
                    option: Box::new(option),
                    source_type,
                    target_type,
                    converter: converter_name,
                },
            ))
        }
        "and_then" => {
            let Some(target_type) = option_payload(&converter_signature.return_type) else {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("converter function `{converter_name}` must return an Option value"),
                ));
            };
            Ok((
                ValueType::Enum("Option".to_string(), vec![target_type.clone()]),
                ValueExpr::OptionAndThen {
                    option: Box::new(option),
                    source_type,
                    target_type,
                    converter: converter_name,
                },
            ))
        }
        _ => unreachable!("option converter helper only supports map/and_then"),
    }
}

fn option_converter_name(
    path: &Path,
    span: &Span,
    method: &str,
    converter: &AstExpr,
) -> Result<String, Diagnostic> {
    let AstExpr::Name(converter_path) = converter else {
        return Err(Diagnostic::new(
            "E0407",
            format!("`Option.{method}` expects a converter function name"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let [converter_name] = converter_path.as_slice() else {
        return Err(Diagnostic::new(
            "E0407",
            format!("`Option.{method}` expects an unqualified converter function name"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    Ok(converter_name.clone())
}

fn option_converter_signature<'a>(
    path: &Path,
    span: &Span,
    method: &str,
    converter_name: &str,
    signatures: &'a HashMap<String, FunctionSignature>,
) -> Result<&'a FunctionSignature, Diagnostic> {
    let Some(converter_signature) = signatures.get(converter_name) else {
        return Err(Diagnostic::new(
            "E0305",
            format!("unknown converter function `{converter_name}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    if !converter_signature.type_params.is_empty() {
        return Err(Diagnostic::new(
            "E0407",
            format!("`Option.{method}` converter `{converter_name}` must not be generic"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    Ok(converter_signature)
}

fn lower_result_builtin(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let [module, method] = callee else {
        unreachable!("result builtin dispatcher only passes qualified calls");
    };
    debug_assert_eq!(module, "result");
    match method.as_str() {
        "is_ok" | "is_err" => {
            let [result] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`result.{method}` expects exactly one Result argument"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (result_type, lowered_result) = lower_value_expr(
                path, result, scope, imports, signatures, structs, enums, span,
            )?;
            lower_result_predicate(path, span, method, lowered_result, &result_type)
        }
        "unwrap_or" => {
            let [result, default] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`result.unwrap_or` expects a Result value and a default value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (result_type, lowered_result) = lower_value_expr(
                path, result, scope, imports, signatures, structs, enums, span,
            )?;
            lower_result_unwrap_or(
                path,
                span,
                "result.unwrap_or",
                lowered_result,
                &result_type,
                default,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )
        }
        "map" | "map_err" | "and_then" => {
            let [result, converter] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`result.{method}` expects a Result value and a converter function"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (result_type, lowered_result) = lower_value_expr(
                path, result, scope, imports, signatures, structs, enums, span,
            )?;
            let (ok_type, err_type) = result_parts(&result_type).ok_or_else(|| {
                type_mismatch(
                    path,
                    span,
                    format!("`result.{method}` expects a Result value"),
                )
            })?;
            lower_result_converter_call(
                path,
                span,
                method,
                lowered_result,
                ok_type,
                err_type,
                converter,
                signatures,
            )
        }
        _ => unreachable!("result builtin dispatcher only passes known calls"),
    }
}

fn lower_result_value_method(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<Option<(ValueType, ValueExpr)>, Diagnostic> {
    if callee.len() != 2 {
        return Ok(None);
    }
    let receiver_name = &callee[0];
    let method = &callee[1];
    if !matches!(
        method.as_str(),
        "is_ok" | "is_err" | "unwrap_or" | "map" | "map_err" | "and_then"
    ) {
        return Ok(None);
    }
    let Some(binding) = scope.get(receiver_name) else {
        return Ok(None);
    };
    require_result_method_import(path, imports, span, method)?;
    let result = binding_value_expr(receiver_name, binding);
    let (ok_type, err_type) = result_parts(&binding.value_type).ok_or_else(|| {
        type_mismatch(
            path,
            span,
            format!("`{receiver_name}.{method}` expects a Result value"),
        )
    })?;
    match method.as_str() {
        "is_ok" | "is_err" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`Result.{method}` expects no arguments"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            lower_result_predicate(path, span, method, result, &binding.value_type).map(Some)
        }
        "unwrap_or" => {
            let [default] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`Result.unwrap_or` expects exactly one default value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            lower_result_unwrap_or(
                path,
                span,
                "Result.unwrap_or",
                result,
                &binding.value_type,
                default,
                scope,
                imports,
                signatures,
                structs,
                enums,
            )
            .map(Some)
        }
        "map" | "map_err" | "and_then" => {
            let [converter] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`Result.{method}` expects exactly one converter function"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            lower_result_converter_call(
                path, span, method, result, ok_type, err_type, converter, signatures,
            )
            .map(Some)
        }
        _ => unreachable!("result method dispatcher only passes known calls"),
    }
}

fn lower_result_predicate(
    path: &Path,
    span: &Span,
    method: &str,
    result: ValueExpr,
    result_type: &ValueType,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let (ok_type, err_type) = result_parts(result_type).ok_or_else(|| {
        type_mismatch(
            path,
            span,
            format!("`Result.{method}` expects a Result value"),
        )
    })?;
    let value = if method == "is_ok" {
        ValueExpr::ResultIsOk {
            result: Box::new(result),
            ok_type,
            err_type,
        }
    } else {
        ValueExpr::ResultIsErr {
            result: Box::new(result),
            ok_type,
            err_type,
        }
    };
    Ok((ValueType::Bool, value))
}

#[allow(clippy::too_many_arguments)]
fn lower_result_unwrap_or(
    path: &Path,
    span: &Span,
    label: &str,
    result: ValueExpr,
    result_type: &ValueType,
    default: &AstExpr,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let (ok_type, err_type) = result_parts(result_type)
        .ok_or_else(|| type_mismatch(path, span, format!("`{label}` expects a Result value")))?;
    if ok_type == ValueType::Void {
        return Err(type_mismatch(
            path,
            span,
            format!("`{label}` does not support Result<void, E>"),
        ));
    }
    let (default_type, lowered_default) = lower_value_expr_with_expected(
        path,
        default,
        scope,
        imports,
        signatures,
        structs,
        enums,
        Some(&ok_type),
        span,
    )?;
    if default_type != ok_type {
        return Err(type_mismatch_expected_found(
            path,
            span,
            format!(
                "`{label}` default is `{}` but ok type is `{}`",
                default_type.name(),
                ok_type.name()
            ),
            &ok_type,
            &default_type,
        ));
    }
    Ok((
        ok_type.clone(),
        ValueExpr::ResultUnwrapOr {
            result: Box::new(result),
            default: Box::new(lowered_default),
            ok_type,
            err_type,
        },
    ))
}

fn lower_result_converter_call(
    path: &Path,
    span: &Span,
    method: &str,
    result: ValueExpr,
    ok_type: ValueType,
    err_type: ValueType,
    converter: &AstExpr,
    signatures: &HashMap<String, FunctionSignature>,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let converter_name = result_converter_name(path, span, method, converter)?;
    let converter_signature =
        result_converter_signature(path, span, method, &converter_name, signatures)?;
    let [converter_param] = converter_signature.params.as_slice() else {
        return Err(Diagnostic::new(
            "E0407",
            format!("converter function `{converter_name}` must take exactly one argument"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    match method {
        "map" => {
            if ok_type == ValueType::Void {
                return Err(type_mismatch(
                    path,
                    span,
                    "`Result.map` does not support Result<void, E>",
                ));
            }
            if converter_param.value_type != ok_type {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!(
                        "`Result.map` converter `{converter_name}` takes `{}` but ok type is `{}`",
                        converter_param.value_type.name(),
                        ok_type.name()
                    ),
                    &ok_type,
                    &converter_param.value_type,
                ));
            }
            if converter_signature.return_type == ValueType::Void {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("converter function `{converter_name}` must return a mapped value"),
                ));
            }
            let target_ok_type = converter_signature.return_type.clone();
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![target_ok_type.clone(), err_type.clone()],
                ),
                ValueExpr::ResultMap {
                    result: Box::new(result),
                    source_ok_type: ok_type,
                    target_ok_type,
                    err_type,
                    converter: converter_name,
                },
            ))
        }
        "map_err" => {
            if converter_param.value_type != err_type {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!(
                        "`Result.map_err` converter `{converter_name}` takes `{}` but error type is `{}`",
                        converter_param.value_type.name(),
                        err_type.name()
                    ),
                    &err_type,
                    &converter_param.value_type,
                ));
            }
            if converter_signature.return_type == ValueType::Void {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("converter function `{converter_name}` must return an error value"),
                ));
            }
            let target_err_type = converter_signature.return_type.clone();
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![ok_type.clone(), target_err_type.clone()],
                ),
                ValueExpr::ResultMapErr {
                    result: Box::new(result),
                    ok_type,
                    source_err_type: err_type,
                    target_err_type,
                    converter: converter_name,
                },
            ))
        }
        "and_then" => {
            if ok_type == ValueType::Void {
                return Err(type_mismatch(
                    path,
                    span,
                    "`Result.and_then` does not support Result<void, E>",
                ));
            }
            if converter_param.value_type != ok_type {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!(
                        "`Result.and_then` converter `{converter_name}` takes `{}` but ok type is `{}`",
                        converter_param.value_type.name(),
                        ok_type.name()
                    ),
                    &ok_type,
                    &converter_param.value_type,
                ));
            }
            let Some((target_ok_type, target_err_type)) =
                result_parts(&converter_signature.return_type)
            else {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("converter function `{converter_name}` must return a Result value"),
                ));
            };
            if target_err_type != err_type {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!(
                        "`Result.and_then` converter `{converter_name}` returns error `{}` but source error is `{}`",
                        target_err_type.name(),
                        err_type.name()
                    ),
                    &err_type,
                    &target_err_type,
                ));
            }
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![target_ok_type.clone(), err_type.clone()],
                ),
                ValueExpr::ResultAndThen {
                    result: Box::new(result),
                    source_ok_type: ok_type,
                    target_ok_type,
                    err_type,
                    converter: converter_name,
                },
            ))
        }
        _ => unreachable!("result converter helper only supports map/map_err/and_then"),
    }
}

fn result_converter_name(
    path: &Path,
    span: &Span,
    method: &str,
    converter: &AstExpr,
) -> Result<String, Diagnostic> {
    let AstExpr::Name(converter_path) = converter else {
        return Err(Diagnostic::new(
            "E0407",
            format!("`Result.{method}` expects a converter function name"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    let [converter_name] = converter_path.as_slice() else {
        return Err(Diagnostic::new(
            "E0407",
            format!("`Result.{method}` expects an unqualified converter function name"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    Ok(converter_name.clone())
}

fn result_converter_signature<'a>(
    path: &Path,
    span: &Span,
    method: &str,
    converter_name: &str,
    signatures: &'a HashMap<String, FunctionSignature>,
) -> Result<&'a FunctionSignature, Diagnostic> {
    let Some(converter_signature) = signatures.get(converter_name) else {
        return Err(Diagnostic::new(
            "E0305",
            format!("unknown converter function `{converter_name}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    if !converter_signature.type_params.is_empty() {
        return Err(Diagnostic::new(
            "E0407",
            format!("`Result.{method}` converter `{converter_name}` must not be generic"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    Ok(converter_signature)
}

fn lower_struct_value_method(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
    allow_void: bool,
) -> Result<Option<(ValueType, ValueExpr)>, Diagnostic> {
    if callee.len() != 2 {
        return Ok(None);
    }
    let receiver_name = &callee[0];
    let method_name = &callee[1];
    let Some(binding) = scope.get(receiver_name) else {
        return Ok(None);
    };
    let ValueType::Struct(owner_name, owner_args) = &binding.value_type else {
        return Ok(None);
    };
    if !owner_args.is_empty() {
        return Ok(None);
    }
    let lowered_name = method_internal_name(owner_name, method_name);
    let Some(signature) = signatures.get(&lowered_name) else {
        return Err(Diagnostic::new(
            "E0314",
            format!("struct `{owner_name}` has no method `{method_name}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    if signature.return_type == ValueType::Void && !allow_void {
        return Err(type_mismatch(
            path,
            span,
            format!(
                "method `{owner_name}.{method_name}` returns `void` and cannot be used as a value"
            ),
        ));
    }
    if args.len() + 1 != signature.params.len() {
        return Err(Diagnostic::new(
            "E0407",
            format!(
                "method `{owner_name}.{method_name}` expects {} argument(s), got {}",
                signature.params.len() - 1,
                args.len()
            ),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    let Some(receiver_param) = signature.params.first() else {
        return Ok(None);
    };
    if receiver_param.value_type != binding.value_type {
        return Err(Diagnostic::new(
            "E0257",
            format!("method `{owner_name}.{method_name}` has invalid receiver type"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    if receiver_param.mutable && !binding.mutable {
        return Err(Diagnostic::new(
            "E0501",
            format!(
                "cannot call mutating method `{owner_name}.{method_name}` on immutable {} `{receiver_name}`",
                binding_source_noun(binding)
            ),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }

    let mut mutable_borrows = Vec::new();
    let mut lowered_args = if receiver_param.mutable {
        let receiver_path = vec![receiver_name.clone()];
        mutable_borrows.push(receiver_path.clone());
        vec![ValueExpr::MutBorrow(receiver_path)]
    } else {
        vec![binding_value_expr(receiver_name, binding)]
    };
    for (index, (arg, expected)) in args.iter().zip(signature.params.iter().skip(1)).enumerate() {
        lowered_args.push(lower_call_arg_for_param(
            path,
            arg,
            expected,
            scope,
            imports,
            signatures,
            structs,
            enums,
            span,
            &format!("{owner_name}.{method_name}"),
            index + 1,
            &mut mutable_borrows,
        )?);
    }

    Ok(Some((
        signature.return_type.clone(),
        ValueExpr::Call {
            name: lowered_name,
            args: lowered_args,
        },
    )))
}

fn lower_call_arg_for_param(
    path: &Path,
    arg: &AstExpr,
    expected: &ParamSignature,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
    callable: &str,
    position: usize,
    mutable_borrows: &mut Vec<Vec<String>>,
) -> Result<ValueExpr, Diagnostic> {
    match (expected.mutable, arg) {
        (true, AstExpr::MutArg { name }) => lower_mut_call_arg(
            path,
            name,
            expected,
            scope,
            structs,
            span,
            callable,
            position,
            mutable_borrows,
        ),
        (true, _) => Err(Diagnostic::new(
            "E0500",
            format!("argument {position} to `{callable}` must be passed as `mut`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        )),
        (false, AstExpr::MutArg { .. }) => Err(Diagnostic::new(
            "E0504",
            format!("argument {position} to `{callable}` is not declared `mut`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        )),
        (false, _) => {
            let (actual, lowered) = lower_value_expr_with_expected(
                path,
                arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&expected.value_type),
                span,
            )?;
            if actual != expected.value_type {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!(
                        "argument {position} to `{callable}` is `{}` but expected `{}`",
                        actual.name(),
                        expected.value_type.name()
                    ),
                    &expected.value_type,
                    &actual,
                ));
            }
            Ok(lowered)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_mut_call_arg(
    path: &Path,
    name: &[String],
    expected: &ParamSignature,
    scope: &HashMap<String, Binding>,
    structs: &HashMap<String, StructType>,
    span: &Span,
    callable: &str,
    position: usize,
    mutable_borrows: &mut Vec<Vec<String>>,
) -> Result<ValueExpr, Diagnostic> {
    if name.is_empty() {
        return Err(Diagnostic::new(
            "E0503",
            "`mut` call arguments must be local variables or fields",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }

    let root = &name[0];
    let Some(binding) = scope.get(root) else {
        return Err(Diagnostic::new(
            "E0303",
            format!("unknown variable `{root}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    if !matches!(binding.source, BindingSource::Local | BindingSource::Param) {
        return Err(Diagnostic::new(
            "E0503",
            "`mut` call arguments must be local variables or fields",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    if !binding.mutable {
        let message = match binding.source {
            BindingSource::Param => format!("cannot pass immutable parameter `{root}` as `mut`"),
            _ => format!("cannot pass immutable variable `{root}` as `mut`"),
        };
        return Err(Diagnostic::new(
            "E0501",
            message,
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }

    let actual_type = mut_borrow_path_type(path, name, &binding.value_type, structs, span)?;
    if actual_type != expected.value_type {
        return Err(type_mismatch_expected_found(
            path,
            span,
            format!(
                "argument {position} to `{callable}` is `{}` but expected `{}`",
                actual_type.name(),
                expected.value_type.name()
            ),
            &expected.value_type,
            &actual_type,
        ));
    }

    if let Some(conflict) = mutable_borrows
        .iter()
        .find(|borrowed| mut_borrow_paths_conflict(borrowed, name))
    {
        return Err(Diagnostic::new(
            "E0502",
            format!(
                "mutable borrow `{}` conflicts with active mutable borrow `{}` in this call",
                mut_borrow_path_name(name),
                mut_borrow_path_name(conflict)
            ),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }

    mutable_borrows.push(name.to_vec());
    Ok(ValueExpr::MutBorrow(name.to_vec()))
}

fn mut_borrow_path_type(
    path: &Path,
    name: &[String],
    root_type: &ValueType,
    structs: &HashMap<String, StructType>,
    span: &Span,
) -> Result<ValueType, Diagnostic> {
    let mut current = root_type.clone();
    for field in name.iter().skip(1) {
        let ValueType::Struct(struct_name, struct_args) = &current else {
            return Err(type_mismatch(
                path,
                span,
                format!("`{}` is not a struct value", mut_borrow_path_name(name)),
            ));
        };
        let struct_type = structs
            .get(struct_name)
            .expect("struct binding must refer to a known struct");
        let Some(field_type) = struct_type
            .fields
            .iter()
            .find(|item| item.name == *field)
            .map(|item| {
                substitute_type_params(&item.value_type, &struct_type.type_params, struct_args)
            })
        else {
            return Err(Diagnostic::new(
                "E0316",
                format!("struct `{struct_name}` has no field `{field}`"),
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        };
        current = field_type;
    }
    Ok(current)
}

fn mut_borrow_paths_conflict(left: &[String], right: &[String]) -> bool {
    left.first() == right.first() && (left.starts_with(right) || right.starts_with(left))
}

fn mut_borrow_path_name(path: &[String]) -> String {
    path.join(".")
}

fn lower_array_mutation(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<ValueExpr, Diagnostic> {
    let name = &callee[0];
    let method = &callee[1];
    require_array_method_import(path, imports, span, method)?;
    let Some(binding) = scope.get(name) else {
        return Err(Diagnostic::new(
            "E0303",
            format!("unknown variable `{name}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    if !binding.mutable {
        return Err(Diagnostic::new(
            "E0501",
            format!(
                "cannot call mutating Array method on immutable {} `{name}`",
                binding_source_noun(binding)
            ),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    let ValueType::Array(element_type) = &binding.value_type else {
        return Err(type_mismatch(
            path,
            span,
            format!("`{name}` is not an Array"),
        ));
    };
    ensure_supported_array_element(path, element_type, span)?;
    match method.as_str() {
        "push" => {
            let [value] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`Array.push` expects exactly one value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (value_type, lowered_value) = lower_value_expr_with_expected(
                path,
                value,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(element_type),
                span,
            )?;
            if &value_type != element_type.as_ref() {
                return Err(type_mismatch(
                    path,
                    span,
                    format!(
                        "`Array.push` value is `{}` but expected `{}`",
                        value_type.name(),
                        element_type.name()
                    ),
                ));
            }
            Ok(ValueExpr::ArrayPush {
                array: name.clone(),
                value: Box::new(lowered_value),
                element_type: element_type.as_ref().clone(),
            })
        }
        "set" => {
            let [index, value] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`Array.set` expects index and value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (index_type, lowered_index) = lower_value_expr_with_expected(
                path,
                index,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&ValueType::U64),
                span,
            )?;
            if index_type != ValueType::U64 {
                return Err(type_mismatch(path, span, "`Array.set` index must be `u64`"));
            }
            let (value_type, lowered_value) = lower_value_expr_with_expected(
                path,
                value,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(element_type),
                span,
            )?;
            if &value_type != element_type.as_ref() {
                return Err(type_mismatch(
                    path,
                    span,
                    format!(
                        "`Array.set` value is `{}` but expected `{}`",
                        value_type.name(),
                        element_type.name()
                    ),
                ));
            }
            Ok(ValueExpr::ArraySet {
                array: name.clone(),
                index: Box::new(lowered_index),
                value: Box::new(lowered_value),
                element_type: element_type.as_ref().clone(),
            })
        }
        "insert" => {
            let [index, value] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`Array.insert` expects index and value",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (index_type, lowered_index) = lower_value_expr_with_expected(
                path,
                index,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&ValueType::U64),
                span,
            )?;
            if index_type != ValueType::U64 {
                return Err(type_mismatch(
                    path,
                    span,
                    "`Array.insert` index must be `u64`",
                ));
            }
            let (value_type, lowered_value) = lower_value_expr_with_expected(
                path,
                value,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(element_type),
                span,
            )?;
            if &value_type != element_type.as_ref() {
                return Err(type_mismatch(
                    path,
                    span,
                    format!(
                        "`Array.insert` value is `{}` but expected `{}`",
                        value_type.name(),
                        element_type.name()
                    ),
                ));
            }
            Ok(ValueExpr::ArrayInsert {
                array: name.clone(),
                index: Box::new(lowered_index),
                value: Box::new(lowered_value),
                element_type: element_type.as_ref().clone(),
            })
        }
        "clear" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`Array.clear` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok(ValueExpr::ArrayClear {
                array: name.clone(),
                element_type: element_type.as_ref().clone(),
            })
        }
        _ => Err(Diagnostic::new(
            "E0305",
            format!("unknown mutating Array method `{method}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        )),
    }
}

fn ensure_supported_array_element(
    path: &Path,
    element_type: &ValueType,
    span: &Span,
) -> Result<(), Diagnostic> {
    if is_supported_array_element(element_type) {
        Ok(())
    } else {
        Err(type_mismatch(
            path,
            span,
            format!(
                "Array elements must be concrete non-void values, got `{}`",
                element_type.name()
            ),
        ))
    }
}

fn ensure_supported_value_type(
    path: &Path,
    value_type: &ValueType,
    span: &Span,
) -> Result<(), Diagnostic> {
    match value_type {
        ValueType::Array(element_type) => {
            if matches!(element_type.as_ref(), ValueType::Void | ValueType::Never) {
                return Err(type_mismatch(
                    path,
                    span,
                    format!(
                        "Array elements must be non-void values, got `{}`",
                        element_type.name()
                    ),
                ));
            }
            ensure_supported_value_type(path, element_type, span)
        }
        ValueType::Struct(_, args) | ValueType::Enum(_, args) => {
            for arg in args {
                ensure_supported_value_type(path, arg, span)?;
            }
            Ok(())
        }
        ValueType::String
        | ValueType::Int
        | ValueType::I32
        | ValueType::U32
        | ValueType::U64
        | ValueType::Float
        | ValueType::Char
        | ValueType::Bool
        | ValueType::Void
        | ValueType::Never
        | ValueType::TypeParam(_) => Ok(()),
    }
}

fn synthetic_span() -> Span {
    Span {
        line: 1,
        column: 1,
        length: 1,
        text: String::new(),
    }
}

fn is_supported_array_element(element_type: &ValueType) -> bool {
    !matches!(
        element_type,
        ValueType::Void | ValueType::Never | ValueType::TypeParam(_)
    )
}

type LoweredValue = (ValueType, ValueExpr);

fn lower_binary_operands(
    path: &Path,
    left: &AstExpr,
    right: &AstExpr,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(LoweredValue, LoweredValue), Diagnostic> {
    let left_default =
        lower_value_expr(path, left, scope, imports, signatures, structs, enums, span)?;
    let right_with_left = lower_value_expr_with_expected(
        path,
        right,
        scope,
        imports,
        signatures,
        structs,
        enums,
        Some(&left_default.0),
        span,
    )?;
    if numeric_pair_matches(&left_default.0, &right_with_left.0) {
        return Ok((left_default, right_with_left));
    }

    if matches!(left, AstExpr::Int(_)) {
        let right_default = lower_value_expr(
            path, right, scope, imports, signatures, structs, enums, span,
        )?;
        if right_default.0.is_integer() {
            let left_with_right = lower_value_expr_with_expected(
                path,
                left,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&right_default.0),
                span,
            )?;
            return Ok((left_with_right, right_default));
        }
    }

    Ok((left_default, right_with_left))
}

fn numeric_pair_matches(left: &ValueType, right: &ValueType) -> bool {
    (left == right && left.is_integer())
        || (left == &ValueType::Float && right == &ValueType::Float)
}

fn lower_int_literal(
    path: &Path,
    value: i64,
    expected: Option<&ValueType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let value_type = expected
        .filter(|value_type| value_type.is_integer())
        .cloned()
        .unwrap_or(ValueType::Int);
    if !int_literal_fits(value, &value_type) {
        return Err(type_mismatch(
            path,
            span,
            format!(
                "integer literal `{value}` does not fit in `{}`",
                value_type.name()
            ),
        ));
    }
    Ok((value_type, ValueExpr::IntLiteral(value)))
}

fn int_literal_fits(value: i64, value_type: &ValueType) -> bool {
    match value_type {
        ValueType::Int => true,
        ValueType::I32 => i32::try_from(value).is_ok(),
        ValueType::U32 => u32::try_from(value).is_ok(),
        ValueType::U64 => value >= 0,
        _ => false,
    }
}

fn coerce_never_expr(expr: ValueExpr, target_type: &ValueType) -> ValueExpr {
    match expr {
        ValueExpr::Panic { message, .. } => ValueExpr::Panic {
            message,
            fallback_type: target_type.clone(),
        },
        other => other,
    }
}

fn parse_non_void_type(
    type_ref: &crate::ast::TypeRef,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Option<ValueType> {
    parse_value_type(type_ref, structs, enums).filter(|value_type| value_type != &ValueType::Void)
}

fn substitute_type_params(
    value_type: &ValueType,
    type_params: &[String],
    args: &[ValueType],
) -> ValueType {
    match value_type {
        ValueType::TypeParam(name) => type_params
            .iter()
            .position(|param| param == name)
            .and_then(|index| args.get(index).cloned())
            .unwrap_or_else(|| value_type.clone()),
        ValueType::Enum(name, nested_args) => ValueType::Enum(
            name.clone(),
            nested_args
                .iter()
                .map(|arg| substitute_type_params(arg, type_params, args))
                .collect(),
        ),
        ValueType::Struct(name, nested_args) => ValueType::Struct(
            name.clone(),
            nested_args
                .iter()
                .map(|arg| substitute_type_params(arg, type_params, args))
                .collect(),
        ),
        ValueType::Array(element) => {
            ValueType::Array(Box::new(substitute_type_params(element, type_params, args)))
        }
        _ => value_type.clone(),
    }
}

fn instantiate_function_signature(
    signature: &FunctionSignature,
    args: &[ValueType],
) -> FunctionSignature {
    FunctionSignature {
        type_params: Vec::new(),
        params: signature
            .params
            .iter()
            .map(|param| ParamSignature {
                value_type: substitute_type_params(&param.value_type, &signature.type_params, args),
                mutable: param.mutable,
            })
            .collect(),
        return_type: substitute_type_params(&signature.return_type, &signature.type_params, args),
    }
}

fn result_parts(value_type: &ValueType) -> Option<(ValueType, ValueType)> {
    let ValueType::Enum(name, args) = value_type else {
        return None;
    };
    if name != "Result" || args.len() != 2 {
        return None;
    }
    Some((args[0].clone(), args[1].clone()))
}

fn option_payload(value_type: &ValueType) -> Option<ValueType> {
    let ValueType::Enum(name, args) = value_type else {
        return None;
    };
    if name != "Option" || args.len() != 1 {
        return None;
    }
    Some(args[0].clone())
}

fn question_payload(
    path: &Path,
    span: &Span,
    question_type: &ValueType,
    return_type: &ValueType,
) -> Result<(QuestionCarrier, ValueType), Diagnostic> {
    if let Some((ok_type, err_type)) = result_parts(question_type) {
        let (_, return_err_type) = result_parts(return_type).ok_or_else(|| {
            Diagnostic::new(
                "E0421",
                "`?` on Result<T, E> requires the current function to return Result<U, E>",
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            )
        })?;
        if err_type != return_err_type {
            return Err(type_mismatch_expected_found(
                path,
                span,
                format!(
                    "`?` error type is `{}` but function returns `{}`",
                    err_type.name(),
                    return_err_type.name()
                ),
                &return_err_type,
                &err_type,
            ));
        }
        return Ok((QuestionCarrier::Result, ok_type));
    }

    if let Some(payload_type) = option_payload(question_type) {
        option_payload(return_type).ok_or_else(|| {
            Diagnostic::new(
                "E0421",
                "`?` on Option<T> requires the current function to return Option<U>",
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            )
        })?;
        return Ok((QuestionCarrier::Option, payload_type));
    }

    Err(Diagnostic::new(
        "E0420",
        "`?` can only be used with `Result<T, E>` or `Option<T>`",
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    ))
}

fn core_prelude_variant(name: &str) -> Option<(&'static str, &'static str)> {
    match name {
        "Some" => Some(("Option", "Some")),
        "None" => Some(("Option", "None")),
        "Ok" => Some(("Result", "Ok")),
        "Err" => Some(("Result", "Err")),
        _ => None,
    }
}

fn resolve_match_arm_variant(
    pattern: &[String],
    enum_name: &str,
    scope: &HashMap<String, Binding>,
) -> Option<String> {
    match pattern {
        [base, variant] if base == enum_name => Some(variant.clone()),
        [variant]
            if !scope.contains_key(variant)
                && core_prelude_variant(variant)
                    .is_some_and(|(resolved_enum, _)| resolved_enum == enum_name) =>
        {
            Some(variant.clone())
        }
        _ => None,
    }
}

fn ast_binary_symbol(op: &AstBinaryOp) -> &'static str {
    match op {
        AstBinaryOp::LogicalOr => "||",
        AstBinaryOp::LogicalAnd => "&&",
        AstBinaryOp::Add => "+",
        AstBinaryOp::Subtract => "-",
        AstBinaryOp::BitOr => "|",
        AstBinaryOp::BitXor => "^",
        AstBinaryOp::Multiply => "*",
        AstBinaryOp::Divide => "/",
        AstBinaryOp::Remainder => "%",
        AstBinaryOp::ShiftLeft => "<<",
        AstBinaryOp::ShiftRight => ">>",
        AstBinaryOp::BitAnd => "&",
        AstBinaryOp::BitAndNot => "&^",
        AstBinaryOp::Equal => "==",
        AstBinaryOp::NotEqual => "!=",
        AstBinaryOp::Less => "<",
        AstBinaryOp::LessEqual => "<=",
        AstBinaryOp::Greater => ">",
        AstBinaryOp::GreaterEqual => ">=",
    }
}

fn parse_value_type(
    type_ref: &crate::ast::TypeRef,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Option<ValueType> {
    let struct_names = structs
        .values()
        .map(|item| (item.name.clone(), item.type_params.len()))
        .collect::<Vec<_>>();
    let enum_names = enums
        .values()
        .map(|item| (item.name.clone(), item.type_params.len()))
        .collect::<Vec<_>>();
    parse_value_type_with_names(type_ref, &struct_names, &enum_names, &[])
}

fn parse_value_type_with_names(
    type_ref: &crate::ast::TypeRef,
    struct_names: &[(String, usize)],
    enum_names: &[(String, usize)],
    type_params: &[String],
) -> Option<ValueType> {
    match type_ref.path.as_slice() {
        [name] if name == "string" && type_ref.args.is_empty() => Some(ValueType::String),
        [name] if name == "i64" && type_ref.args.is_empty() => Some(ValueType::Int),
        [name] if name == "i32" && type_ref.args.is_empty() => Some(ValueType::I32),
        [name] if name == "u32" && type_ref.args.is_empty() => Some(ValueType::U32),
        [name] if name == "u64" && type_ref.args.is_empty() => Some(ValueType::U64),
        [name] if name == "f64" && type_ref.args.is_empty() => Some(ValueType::Float),
        [name] if name == "char" && type_ref.args.is_empty() => Some(ValueType::Char),
        [name] if name == "bool" && type_ref.args.is_empty() => Some(ValueType::Bool),
        [name] if name == "void" && type_ref.args.is_empty() => Some(ValueType::Void),
        [name] if name == "Array" => {
            let [element] = type_ref.args.as_slice() else {
                return None;
            };
            let element_type =
                parse_value_type_with_names(element, struct_names, enum_names, type_params)?;
            Some(ValueType::Array(Box::new(element_type)))
        }
        [name] if struct_names.iter().any(|(item, _)| item == name) => {
            let arity = struct_names
                .iter()
                .find(|(item, _)| item == name)
                .map(|(_, arity)| *arity)?;
            if type_ref.args.len() != arity {
                return None;
            }
            let args = type_ref
                .args
                .iter()
                .map(|arg| parse_value_type_with_names(arg, struct_names, enum_names, type_params))
                .collect::<Option<Vec<_>>>()?;
            Some(ValueType::Struct(name.to_string(), args))
        }
        [name] if enum_names.iter().any(|(item, _)| item == name) => {
            let arity = enum_names
                .iter()
                .find(|(item, _)| item == name)
                .map(|(_, arity)| *arity)?;
            if type_ref.args.len() != arity {
                return None;
            }
            let args = type_ref
                .args
                .iter()
                .map(|arg| parse_value_type_with_names(arg, struct_names, enum_names, type_params))
                .collect::<Option<Vec<_>>>()?;
            Some(ValueType::Enum(name.to_string(), args))
        }
        [name] if type_params.iter().any(|item| item == name) => {
            if !type_ref.args.is_empty() {
                return None;
            }
            Some(ValueType::TypeParam(name.to_string()))
        }
        _ => None,
    }
}

fn method_internal_name(owner_name: &str, method_name: &str) -> String {
    format!("{owner_name}_{method_name}")
}

fn generic_function_instance_name(name: &str, args: &[ValueType]) -> String {
    let suffix = args
        .iter()
        .map(value_type_key_part)
        .collect::<Vec<_>>()
        .join("_");
    format!("{name}_{suffix}")
}

fn value_type_key_part(value_type: &ValueType) -> String {
    match value_type {
        ValueType::String => "string".to_string(),
        ValueType::Int => "i64".to_string(),
        ValueType::I32 => "i32".to_string(),
        ValueType::U32 => "u32".to_string(),
        ValueType::U64 => "u64".to_string(),
        ValueType::Float => "f64".to_string(),
        ValueType::Char => "char".to_string(),
        ValueType::Bool => "bool".to_string(),
        ValueType::Array(element) => format!("array_{}", value_type_key_part(element)),
        ValueType::Struct(name, args) => format!("struct_{}{}", name, generic_type_suffix(args)),
        ValueType::Enum(name, args) => format!("enum_{}{}", name, generic_type_suffix(args)),
        ValueType::TypeParam(name) => format!("param_{name}"),
        ValueType::Void => "void".to_string(),
        ValueType::Never => "never".to_string(),
    }
}

fn generic_type_suffix(args: &[ValueType]) -> String {
    if args.is_empty() {
        String::new()
    } else {
        format!(
            "_{}",
            args.iter()
                .map(value_type_key_part)
                .collect::<Vec<_>>()
                .join("_")
        )
    }
}

fn is_io_print_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "io"
                && matches!(name.as_str(), "print" | "println" | "eprint" | "eprintln")
    ) || matches!(
        callee,
        [name] if matches!(name.as_str(), "print" | "println" | "eprint" | "eprintln")
    )
}

fn io_print_statement(function_name: &str, arg: ValueExpr) -> Statement {
    match function_name {
        "print" => Statement::Print(arg),
        "eprint" => Statement::Eprint(arg),
        "eprintln" => Statement::Eprintln(arg),
        "println" => Statement::Println(arg),
        _ => unreachable!("io print dispatcher only passes known functions"),
    }
}

fn io_print_deferred_call(function_name: &str, arg: ValueExpr) -> DeferredCall {
    match function_name {
        "print" => DeferredCall::Print(arg),
        "eprint" => DeferredCall::Eprint(arg),
        "eprintln" => DeferredCall::Eprintln(arg),
        "println" => DeferredCall::Println(arg),
        _ => unreachable!("io print dispatcher only passes known functions"),
    }
}

fn io_print_builtin_expr_name(function_name: &str) -> String {
    match function_name {
        "print" => BUILTIN_PRINT_EXPR.to_string(),
        "eprint" => BUILTIN_EPRINT_EXPR.to_string(),
        "eprintln" => BUILTIN_EPRINTLN_EXPR.to_string(),
        "println" => BUILTIN_PRINTLN_EXPR.to_string(),
        _ => unreachable!("io print dispatcher only passes known functions"),
    }
}

fn resolve_specific_value_builtin(name: &str, imports: &[String]) -> Option<Vec<String>> {
    let qualified = match name {
        "len" if imports.iter().any(|item| item == "std.string.len") => {
            vec!["string".to_string(), "len".to_string()]
        }
        "concat" if imports.iter().any(|item| item == "std.string.concat") => {
            vec!["string".to_string(), "concat".to_string()]
        }
        "is_empty" if imports.iter().any(|item| item == "std.string.is_empty") => {
            vec!["string".to_string(), "is_empty".to_string()]
        }
        "contains" if imports.iter().any(|item| item == "std.string.contains") => {
            vec!["string".to_string(), "contains".to_string()]
        }
        "starts_with" if imports.iter().any(|item| item == "std.string.starts_with") => {
            vec!["string".to_string(), "starts_with".to_string()]
        }
        "ends_with" if imports.iter().any(|item| item == "std.string.ends_with") => {
            vec!["string".to_string(), "ends_with".to_string()]
        }
        "split" if imports.iter().any(|item| item == "std.string.split") => {
            vec!["string".to_string(), "split".to_string()]
        }
        "trim" if imports.iter().any(|item| item == "std.string.trim") => {
            vec!["string".to_string(), "trim".to_string()]
        }
        "to_lower" if imports.iter().any(|item| item == "std.string.to_lower") => {
            vec!["string".to_string(), "to_lower".to_string()]
        }
        "to_upper" if imports.iter().any(|item| item == "std.string.to_upper") => {
            vec!["string".to_string(), "to_upper".to_string()]
        }
        "read_to_string" if imports.iter().any(|item| item == "std.fs.read_to_string") => {
            vec!["fs".to_string(), "read_to_string".to_string()]
        }
        "write_string" if imports.iter().any(|item| item == "std.fs.write_string") => {
            vec!["fs".to_string(), "write_string".to_string()]
        }
        "read_bytes" if imports.iter().any(|item| item == "std.fs.read_bytes") => {
            vec!["fs".to_string(), "read_bytes".to_string()]
        }
        "write_bytes" if imports.iter().any(|item| item == "std.fs.write_bytes") => {
            vec!["fs".to_string(), "write_bytes".to_string()]
        }
        "exists" if imports.iter().any(|item| item == "std.fs.exists") => {
            vec!["fs".to_string(), "exists".to_string()]
        }
        "metadata" if imports.iter().any(|item| item == "std.fs.metadata") => {
            vec!["fs".to_string(), "metadata".to_string()]
        }
        "create_dir" if imports.iter().any(|item| item == "std.fs.create_dir") => {
            vec!["fs".to_string(), "create_dir".to_string()]
        }
        "remove_dir" if imports.iter().any(|item| item == "std.fs.remove_dir") => {
            vec!["fs".to_string(), "remove_dir".to_string()]
        }
        "read_dir" if imports.iter().any(|item| item == "std.fs.read_dir") => {
            vec!["fs".to_string(), "read_dir".to_string()]
        }
        "open" if imports.iter().any(|item| item == "std.fs.open") => {
            vec!["fs".to_string(), "open".to_string()]
        }
        "read_line" if imports.iter().any(|item| item == "std.io.read_line") => {
            vec!["io".to_string(), "read_line".to_string()]
        }
        "print" if imports.iter().any(|item| item == "std.debug.print") => {
            vec!["debug".to_string(), "print".to_string()]
        }
        "println" if imports.iter().any(|item| item == "std.debug.println") => {
            vec!["debug".to_string(), "println".to_string()]
        }
        "panic" if imports.iter().any(|item| item == "std.debug.panic") => {
            vec!["debug".to_string(), "panic".to_string()]
        }
        "backtrace" if imports.iter().any(|item| item == "std.debug.backtrace") => {
            vec!["debug".to_string(), "backtrace".to_string()]
        }
        "debug" if imports.iter().any(|item| item == "std.log.debug") => {
            vec!["log".to_string(), "debug".to_string()]
        }
        "info" if imports.iter().any(|item| item == "std.log.info") => {
            vec!["log".to_string(), "info".to_string()]
        }
        "warn" if imports.iter().any(|item| item == "std.log.warn") => {
            vec!["log".to_string(), "warn".to_string()]
        }
        "error" if imports.iter().any(|item| item == "std.log.error") => {
            vec!["log".to_string(), "error".to_string()]
        }
        "enabled" if imports.iter().any(|item| item == "std.log.enabled") => {
            vec!["log".to_string(), "enabled".to_string()]
        }
        "new" if imports.iter().any(|item| item == "std.hash.new") => {
            vec!["hash".to_string(), "new".to_string()]
        }
        "string" if imports.iter().any(|item| item == "std.hash.string") => {
            vec!["hash".to_string(), "string".to_string()]
        }
        "write_string" if imports.iter().any(|item| item == "std.hash.write_string") => {
            vec!["hash".to_string(), "write_string".to_string()]
        }
        "finish" if imports.iter().any(|item| item == "std.hash.finish") => {
            vec!["hash".to_string(), "finish".to_string()]
        }
        "sha256" if imports.iter().any(|item| item == "std.crypto.sha256") => {
            vec!["crypto".to_string(), "sha256".to_string()]
        }
        "sha512" if imports.iter().any(|item| item == "std.crypto.sha512") => {
            vec!["crypto".to_string(), "sha512".to_string()]
        }
        "random_bytes" if imports.iter().any(|item| item == "std.crypto.random_bytes") => {
            vec!["crypto".to_string(), "random_bytes".to_string()]
        }
        "parse" if imports.iter().any(|item| item == "std.json.parse") => {
            vec!["json".to_string(), "parse".to_string()]
        }
        "stringify" if imports.iter().any(|item| item == "std.json.stringify") => {
            vec!["json".to_string(), "stringify".to_string()]
        }
        "compile" if imports.iter().any(|item| item == "std.regex.compile") => {
            vec!["regex".to_string(), "compile".to_string()]
        }
        "is_match" if imports.iter().any(|item| item == "std.regex.is_match") => {
            vec!["regex".to_string(), "is_match".to_string()]
        }
        "captures" if imports.iter().any(|item| item == "std.regex.captures") => {
            vec!["regex".to_string(), "captures".to_string()]
        }
        "map_new" if imports.iter().any(|item| item == "std.collections.map_new") => {
            vec!["collections".to_string(), "map_new".to_string()]
        }
        "map_len" if imports.iter().any(|item| item == "std.collections.map_len") => {
            vec!["collections".to_string(), "map_len".to_string()]
        }
        "map_get" if imports.iter().any(|item| item == "std.collections.map_get") => {
            vec!["collections".to_string(), "map_get".to_string()]
        }
        "map_contains"
            if imports
                .iter()
                .any(|item| item == "std.collections.map_contains") =>
        {
            vec!["collections".to_string(), "map_contains".to_string()]
        }
        "map_set" if imports.iter().any(|item| item == "std.collections.map_set") => {
            vec!["collections".to_string(), "map_set".to_string()]
        }
        "map_remove"
            if imports
                .iter()
                .any(|item| item == "std.collections.map_remove") =>
        {
            vec!["collections".to_string(), "map_remove".to_string()]
        }
        "set_new" if imports.iter().any(|item| item == "std.collections.set_new") => {
            vec!["collections".to_string(), "set_new".to_string()]
        }
        "set_len" if imports.iter().any(|item| item == "std.collections.set_len") => {
            vec!["collections".to_string(), "set_len".to_string()]
        }
        "set_contains"
            if imports
                .iter()
                .any(|item| item == "std.collections.set_contains") =>
        {
            vec!["collections".to_string(), "set_contains".to_string()]
        }
        "set_insert"
            if imports
                .iter()
                .any(|item| item == "std.collections.set_insert") =>
        {
            vec!["collections".to_string(), "set_insert".to_string()]
        }
        "set_remove"
            if imports
                .iter()
                .any(|item| item == "std.collections.set_remove") =>
        {
            vec!["collections".to_string(), "set_remove".to_string()]
        }
        "get" if imports.iter().any(|item| item == "std.env.get") => {
            vec!["env".to_string(), "get".to_string()]
        }
        "set" if imports.iter().any(|item| item == "std.env.set") => {
            vec!["env".to_string(), "set".to_string()]
        }
        "cwd" if imports.iter().any(|item| item == "std.env.cwd") => {
            vec!["env".to_string(), "cwd".to_string()]
        }
        "home_dir" if imports.iter().any(|item| item == "std.env.home_dir") => {
            vec!["env".to_string(), "home_dir".to_string()]
        }
        "temp_dir" if imports.iter().any(|item| item == "std.env.temp_dir") => {
            vec!["env".to_string(), "temp_dir".to_string()]
        }
        "args" if imports.iter().any(|item| item == "std.env.args") => {
            vec!["env".to_string(), "args".to_string()]
        }
        "exit" if imports.iter().any(|item| item == "std.process.exit") => {
            vec!["process".to_string(), "exit".to_string()]
        }
        "spawn" if imports.iter().any(|item| item == "std.process.spawn") => {
            vec!["process".to_string(), "spawn".to_string()]
        }
        "status" if imports.iter().any(|item| item == "std.process.status") => {
            vec!["process".to_string(), "status".to_string()]
        }
        "exec" if imports.iter().any(|item| item == "std.process.exec") => {
            vec!["process".to_string(), "exec".to_string()]
        }
        "output" if imports.iter().any(|item| item == "std.process.output") => {
            vec!["process".to_string(), "output".to_string()]
        }
        "assert" if imports.iter().any(|item| item == "std.testing.assert") => {
            vec!["testing".to_string(), "assert".to_string()]
        }
        "assert_equal"
            if imports
                .iter()
                .any(|item| item == "std.testing.assert_equal") =>
        {
            vec!["testing".to_string(), "assert_equal".to_string()]
        }
        "assert_error"
            if imports
                .iter()
                .any(|item| item == "std.testing.assert_error") =>
        {
            vec!["testing".to_string(), "assert_error".to_string()]
        }
        "join" if imports.iter().any(|item| item == "std.path.join") => {
            vec!["path".to_string(), "join".to_string()]
        }
        "basename" if imports.iter().any(|item| item == "std.path.basename") => {
            vec!["path".to_string(), "basename".to_string()]
        }
        "dirname" if imports.iter().any(|item| item == "std.path.dirname") => {
            vec!["path".to_string(), "dirname".to_string()]
        }
        "extension" if imports.iter().any(|item| item == "std.path.extension") => {
            vec!["path".to_string(), "extension".to_string()]
        }
        "normalize" if imports.iter().any(|item| item == "std.path.normalize") => {
            vec!["path".to_string(), "normalize".to_string()]
        }
        "is_absolute" if imports.iter().any(|item| item == "std.path.is_absolute") => {
            vec!["path".to_string(), "is_absolute".to_string()]
        }
        "abs" if imports.iter().any(|item| item == "std.math.abs") => {
            vec!["math".to_string(), "abs".to_string()]
        }
        "min" if imports.iter().any(|item| item == "std.math.min") => {
            vec!["math".to_string(), "min".to_string()]
        }
        "max" if imports.iter().any(|item| item == "std.math.max") => {
            vec!["math".to_string(), "max".to_string()]
        }
        "floor" if imports.iter().any(|item| item == "std.math.floor") => {
            vec!["math".to_string(), "floor".to_string()]
        }
        "ceil" if imports.iter().any(|item| item == "std.math.ceil") => {
            vec!["math".to_string(), "ceil".to_string()]
        }
        "round" if imports.iter().any(|item| item == "std.math.round") => {
            vec!["math".to_string(), "round".to_string()]
        }
        "sqrt" if imports.iter().any(|item| item == "std.math.sqrt") => {
            vec!["math".to_string(), "sqrt".to_string()]
        }
        "pow" if imports.iter().any(|item| item == "std.math.pow") => {
            vec!["math".to_string(), "pow".to_string()]
        }
        "sin" if imports.iter().any(|item| item == "std.math.sin") => {
            vec!["math".to_string(), "sin".to_string()]
        }
        "cos" if imports.iter().any(|item| item == "std.math.cos") => {
            vec!["math".to_string(), "cos".to_string()]
        }
        "is_digit" if imports.iter().any(|item| item == "std.char.is_digit") => {
            vec!["char".to_string(), "is_digit".to_string()]
        }
        "is_alpha" if imports.iter().any(|item| item == "std.char.is_alpha") => {
            vec!["char".to_string(), "is_alpha".to_string()]
        }
        "is_whitespace" if imports.iter().any(|item| item == "std.char.is_whitespace") => {
            vec!["char".to_string(), "is_whitespace".to_string()]
        }
        "to_string" if imports.iter().any(|item| item == "std.char.to_string") => {
            vec!["char".to_string(), "to_string".to_string()]
        }
        "platform" if imports.iter().any(|item| item == "std.os.platform") => {
            vec!["os".to_string(), "platform".to_string()]
        }
        "arch" if imports.iter().any(|item| item == "std.os.arch") => {
            vec!["os".to_string(), "arch".to_string()]
        }
        "path_separator" if imports.iter().any(|item| item == "std.os.path_separator") => {
            vec!["os".to_string(), "path_separator".to_string()]
        }
        "line_ending" if imports.iter().any(|item| item == "std.os.line_ending") => {
            vec!["os".to_string(), "line_ending".to_string()]
        }
        "now_millis" if imports.iter().any(|item| item == "std.time.now_millis") => {
            vec!["time".to_string(), "now_millis".to_string()]
        }
        "monotonic_millis"
            if imports
                .iter()
                .any(|item| item == "std.time.monotonic_millis") =>
        {
            vec!["time".to_string(), "monotonic_millis".to_string()]
        }
        "duration_millis"
            if imports
                .iter()
                .any(|item| item == "std.time.duration_millis") =>
        {
            vec!["time".to_string(), "duration_millis".to_string()]
        }
        "duration_seconds"
            if imports
                .iter()
                .any(|item| item == "std.time.duration_seconds") =>
        {
            vec!["time".to_string(), "duration_seconds".to_string()]
        }
        "duration_as_millis"
            if imports
                .iter()
                .any(|item| item == "std.time.duration_as_millis") =>
        {
            vec!["time".to_string(), "duration_as_millis".to_string()]
        }
        "format_duration"
            if imports
                .iter()
                .any(|item| item == "std.time.format_duration") =>
        {
            vec!["time".to_string(), "format_duration".to_string()]
        }
        "sleep" if imports.iter().any(|item| item == "std.time.sleep") => {
            vec!["time".to_string(), "sleep".to_string()]
        }
        "sleep_millis" if imports.iter().any(|item| item == "std.time.sleep_millis") => {
            vec!["time".to_string(), "sleep_millis".to_string()]
        }
        "parse_i64" if imports.iter().any(|item| item == "std.num.parse_i64") => {
            vec!["num".to_string(), "parse_i64".to_string()]
        }
        "parse_u64" if imports.iter().any(|item| item == "std.num.parse_u64") => {
            vec!["num".to_string(), "parse_u64".to_string()]
        }
        "parse_f64" if imports.iter().any(|item| item == "std.num.parse_f64") => {
            vec!["num".to_string(), "parse_f64".to_string()]
        }
        "checked_add" if imports.iter().any(|item| item == "std.num.checked_add") => {
            vec!["num".to_string(), "checked_add".to_string()]
        }
        "checked_sub" if imports.iter().any(|item| item == "std.num.checked_sub") => {
            vec!["num".to_string(), "checked_sub".to_string()]
        }
        "checked_mul" if imports.iter().any(|item| item == "std.num.checked_mul") => {
            vec!["num".to_string(), "checked_mul".to_string()]
        }
        "wrapping_add" if imports.iter().any(|item| item == "std.num.wrapping_add") => {
            vec!["num".to_string(), "wrapping_add".to_string()]
        }
        "wrapping_sub" if imports.iter().any(|item| item == "std.num.wrapping_sub") => {
            vec!["num".to_string(), "wrapping_sub".to_string()]
        }
        "wrapping_mul" if imports.iter().any(|item| item == "std.num.wrapping_mul") => {
            vec!["num".to_string(), "wrapping_mul".to_string()]
        }
        "is_ok" if imports.iter().any(|item| item == "std.result.is_ok") => {
            vec!["result".to_string(), "is_ok".to_string()]
        }
        "is_err" if imports.iter().any(|item| item == "std.result.is_err") => {
            vec!["result".to_string(), "is_err".to_string()]
        }
        "map_err" if imports.iter().any(|item| item == "std.result.map_err") => {
            vec!["result".to_string(), "map_err".to_string()]
        }
        "is_some" if imports.iter().any(|item| item == "std.option.is_some") => {
            vec!["option".to_string(), "is_some".to_string()]
        }
        "is_none" if imports.iter().any(|item| item == "std.option.is_none") => {
            vec!["option".to_string(), "is_none".to_string()]
        }
        "unwrap_or" if imports.iter().any(|item| item == "std.option.unwrap_or") => {
            vec!["option".to_string(), "unwrap_or".to_string()]
        }
        "map" if imports.iter().any(|item| item == "std.option.map") => {
            vec!["option".to_string(), "map".to_string()]
        }
        "and_then" if imports.iter().any(|item| item == "std.option.and_then") => {
            vec!["option".to_string(), "and_then".to_string()]
        }
        "unwrap_or" if imports.iter().any(|item| item == "std.result.unwrap_or") => {
            vec!["result".to_string(), "unwrap_or".to_string()]
        }
        "map" if imports.iter().any(|item| item == "std.result.map") => {
            vec!["result".to_string(), "map".to_string()]
        }
        "and_then" if imports.iter().any(|item| item == "std.result.and_then") => {
            vec!["result".to_string(), "and_then".to_string()]
        }
        "new" if imports.iter().any(|item| item == "std.array.new") => {
            vec!["Array".to_string(), "new".to_string()]
        }
        _ => return None,
    };
    Some(qualified)
}

fn require_import(
    path: &Path,
    imports: &[String],
    span: &Span,
    module_import: &str,
    symbol: &str,
) -> Result<(), Diagnostic> {
    let imported = if symbol == "Array.new" {
        imports.iter().any(|item| {
            matches!(
                item.as_str(),
                "std.array" | "std.array.Array" | "std.array.new"
            )
        })
    } else {
        imports.iter().any(|item| item == module_import)
    };
    if imported {
        return Ok(());
    }
    Err(Diagnostic::new(
        "E0301",
        format!("`{symbol}` requires `import {module_import}`"),
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    ))
}

fn require_string_method_import(
    path: &Path,
    imports: &[String],
    span: &Span,
    method: &str,
) -> Result<(), Diagnostic> {
    if imports
        .iter()
        .any(|item| item == "std.string" || item == &format!("std.string.{method}"))
    {
        return Ok(());
    }
    Err(Diagnostic::new(
        "E0301",
        format!("`string.{method}` requires `import std.string`"),
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    ))
}

fn require_array_method_import(
    path: &Path,
    imports: &[String],
    span: &Span,
    method: &str,
) -> Result<(), Diagnostic> {
    if imports.iter().any(|item| {
        item == "std.array" || item == "std.array.Array" || item == &format!("std.array.{method}")
    }) {
        return Ok(());
    }
    Err(Diagnostic::new(
        "E0301",
        format!("`Array.{method}` requires `import std.array`"),
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    ))
}

fn require_result_method_import(
    path: &Path,
    imports: &[String],
    span: &Span,
    method: &str,
) -> Result<(), Diagnostic> {
    if imports
        .iter()
        .any(|item| item == "std.result" || item == &format!("std.result.{method}"))
    {
        return Ok(());
    }
    Err(Diagnostic::new(
        "E0301",
        format!("`Result.{method}` requires `import std.result`"),
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    ))
}

fn require_option_method_import(
    path: &Path,
    imports: &[String],
    span: &Span,
    method: &str,
) -> Result<(), Diagnostic> {
    if imports
        .iter()
        .any(|item| item == "std.option" || item == &format!("std.option.{method}"))
    {
        return Ok(());
    }
    Err(Diagnostic::new(
        "E0301",
        format!("`Option.{method}` requires `import std.option`"),
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    ))
}

fn resolve_io_print_function<'a>(callee: &'a [String], imports: &[String]) -> Option<&'a str> {
    match callee {
        [module, name]
            if module == "io"
                && matches!(name.as_str(), "print" | "println" | "eprint" | "eprintln")
                && imports.iter().any(|item| item == "std.io") =>
        {
            Some(name.as_str())
        }
        [name]
            if matches!(name.as_str(), "print" | "println" | "eprint" | "eprintln")
                && imports.iter().any(|item| item == &format!("std.io.{name}")) =>
        {
            Some(name.as_str())
        }
        _ => None,
    }
}

fn io_print_import_error(callee: &[String]) -> String {
    match callee {
        [module, name] if module == "io" => {
            format!("v0.1 current implementation requires `import std.io` for `io.{name}`")
        }
        [name] => {
            format!("v0.1 current implementation requires `import std.io.{name}` for `{name}`")
        }
        _ => "v0.1 current implementation requires an io import".to_string(),
    }
}

fn missing_io_import_diagnostic(path: &Path, span: &Span, callee: &[String]) -> Diagnostic {
    let import = match callee {
        [module, _] if module == "io" => "import std.io\n".to_string(),
        [name] => format!("import std.io.{name}\n"),
        _ => "import std.io\n".to_string(),
    };
    let description = match callee {
        [module, name] if module == "io" => {
            format!("add `import std.io` to use `io.{name}`")
        }
        [name] => format!("add `import std.io.{name}` to use `{name}`"),
        _ => "add `import std.io` to use io functions".to_string(),
    };
    let mut diagnostic = Diagnostic::new(
        "E0301",
        io_print_import_error(callee),
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    );
    diagnostic.suggestions.push(Suggestion {
        line: 2,
        column: 1,
        length: 0,
        text: import,
        description,
    });
    diagnostic
}

fn println_type_error(path: &Path, span: &Span, function_name: &str) -> Diagnostic {
    Diagnostic::new(
        "E0402",
        format!("`io.{function_name}` expects exactly one string argument"),
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    )
}

fn type_mismatch(path: &Path, span: &Span, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(
        "E0404",
        message,
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    )
}

fn type_mismatch_expected_found(
    path: &Path,
    span: &Span,
    message: impl Into<String>,
    expected: &ValueType,
    found: &ValueType,
) -> Diagnostic {
    type_mismatch(path, span, message).with_expected_found(expected.name(), found.name())
}

impl ValueType {
    pub fn name(&self) -> &str {
        match self {
            ValueType::String => "string",
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

    fn is_integer(&self) -> bool {
        matches!(
            self,
            ValueType::Int | ValueType::I32 | ValueType::U32 | ValueType::U64
        )
    }

    fn is_numeric(&self) -> bool {
        self.is_integer() || matches!(self, ValueType::Float)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_inline(source: &str) -> Result<Program, Diagnostic> {
        let path = Path::new("main.nomo");
        let tokens = lexer::lex(path, source)?;
        let ast = parser::parse(path, &tokens)?;
        lower_program(path, ast, &[], None, EntryMode::MainFunctionRequired)
    }

    #[test]
    fn parses_v0_1_hello() {
        let source = r#"package app.main

import std.io

fn main() -> void {
    io.println("Hello, Nomo")
}
"#;

        let program = parse_inline(source).unwrap();
        assert_eq!(program.package, "app.main");
        assert_eq!(program.imports, vec!["std.io"]);
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert_eq!(
            main.body,
            vec![Statement::Println(ValueExpr::StringLiteral(
                "Hello, Nomo".to_string()
            ))]
        );
    }

    #[test]
    fn rejects_unknown_std_import() {
        let source = r#"package app.main

import std.typo

fn main() -> void {
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0301");
        assert!(err.message.contains("std.typo"));
    }

    #[test]
    fn rejects_unknown_specific_std_import() {
        let source = r#"package app.main

import std.io.flush

fn main() -> void {
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0301");
        assert!(err.message.contains("std.io.flush"));
    }

    #[test]
    fn rejects_non_std_import_in_v0_1() {
        let source = r#"package app.main

import app.other

fn main() -> void {
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0301");
        assert!(err.message.contains("app.other"));
    }

    #[test]
    fn rejects_std_module_calls_without_imports() {
        for (source, symbol, import) in [
            (
                "package app.main\nfn main() -> void {\n    let count: u64 = string.len(\"hi\")\n}\n",
                "string.len",
                "std.string",
            ),
            (
                "package app.main\nfn main() -> void {\n    let result: Result<string, FsError> = fs.read_to_string(\"missing.txt\")\n}\n",
                "fs.read_to_string",
                "std.fs",
            ),
            (
                "package app.main\nfn main() -> void {\n    let value: Option<string> = env.get(\"HOME\")\n}\n",
                "env.get",
                "std.env",
            ),
            (
                "package app.main\nfn main() -> void {\n    let name: string = path.basename(\"/tmp/nomo.txt\")\n}\n",
                "path.basename",
                "std.path",
            ),
            (
                "package app.main\nfn main() -> void {\n    let value: i64 = math.abs(0 - 1)\n}\n",
                "math.abs",
                "std.math",
            ),
            (
                "package app.main\nfn main() -> void {\n    let items = Array.new<i32>()\n}\n",
                "Array.new",
                "std.array",
            ),
        ] {
            let err = parse_inline(source).unwrap_err();
            assert_eq!(err.code, "E0301");
            assert!(err.message.contains(symbol), "{:?}", err.message);
            assert!(err.message.contains(import), "{:?}", err.message);
        }
    }

    #[test]
    fn rejects_standard_library_types_without_imports() {
        for (source, type_name, import) in [
            (
                "package app.main\nfn parse() -> Result<i32, string> {\n    return 1\n}\nfn main() -> void {\n}\n",
                "Result",
                "std.result",
            ),
            (
                "package app.main\nfn label(value: Option<i32>) -> void {\n}\nfn main() -> void {\n}\n",
                "Option",
                "std.option",
            ),
            (
                "package app.main\nstruct Bag {\n    items: Array<i32>\n}\nfn main() -> void {\n}\n",
                "Array",
                "std.array",
            ),
            (
                "package app.main\nfn report(error: FsError) -> void {\n}\nfn main() -> void {\n}\n",
                "FsError",
                "std.fs",
            ),
        ] {
            let err = parse_inline(source).unwrap_err();
            assert_eq!(err.code, "E0301", "{:?}", err);
            assert!(err.message.contains(type_name), "{:?}", err.message);
            assert!(err.message.contains(import), "{:?}", err.message);
        }
    }

    #[test]
    fn accepts_string_variable_println() {
        let source = r#"package app.main

import std.io

fn main() -> void {
    let message: string = "Hello, Nomo"
    io.println(message)
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert_eq!(
            main.body,
            vec![
                Statement::Let {
                    name: "message".to_string(),
                    value_type: ValueType::String,
                    initializer: ValueExpr::StringLiteral("Hello, Nomo".to_string()),
                },
                Statement::Println(ValueExpr::Variable("message".to_string())),
            ]
        );
    }

    #[test]
    fn accepts_omitted_void_return_type() {
        let source = r#"package app.main

import std.io

fn log() {
    io.println("hello")
}

fn main() {
    log()
}
"#;

        let program = parse_inline(source).unwrap();
        let log = program
            .functions
            .iter()
            .find(|function| function.name == "log")
            .unwrap();
        let main = program
            .functions
            .iter()
            .find(|function| function.name == "main")
            .unwrap();
        assert_eq!(log.return_type, ValueType::Void);
        assert_eq!(main.return_type, ValueType::Void);
    }

    #[test]
    fn accepts_specific_println_import() {
        let source = r#"package app.main

import std.io.println

fn main() -> void {
    println("Hello, Nomo")
}
"#;

        let program = parse_inline(source).unwrap();
        assert_eq!(program.imports, vec!["std.io.println"]);
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert_eq!(
            main.body,
            vec![Statement::Println(ValueExpr::StringLiteral(
                "Hello, Nomo".to_string()
            ))]
        );
    }

    #[test]
    fn accepts_eprintln() {
        let source = r#"package app.main

import std.io

fn main() -> void {
    io.eprintln("error")
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert_eq!(
            main.body,
            vec![Statement::Eprintln(ValueExpr::StringLiteral(
                "error".to_string()
            ))]
        );
    }

    #[test]
    fn accepts_print_and_eprint() {
        let source = r#"package app.main

import std.io

fn main() -> void {
    io.print("out")
    io.eprint("err")
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert_eq!(
            main.body,
            vec![
                Statement::Print(ValueExpr::StringLiteral("out".to_string())),
                Statement::Eprint(ValueExpr::StringLiteral("err".to_string())),
            ]
        );
    }

    #[test]
    fn accepts_specific_print_and_eprint_imports() {
        let source = r#"package app.main

import std.io.print
import std.io.eprint

fn main() -> void {
    print("out")
    eprint("err")
}
"#;

        let program = parse_inline(source).unwrap();
        assert_eq!(program.imports, vec!["std.io.print", "std.io.eprint"]);
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert_eq!(
            main.body,
            vec![
                Statement::Print(ValueExpr::StringLiteral("out".to_string())),
                Statement::Eprint(ValueExpr::StringLiteral("err".to_string())),
            ]
        );
    }

    #[test]
    fn accepts_io_read_line_builtin() {
        let source = r#"package app.main

import std.io

fn main() -> void {
    let result: Result<string, IoError> = io.read_line()
}
"#;

        let program = parse_inline(source).unwrap();
        assert!(program.structs.iter().any(|item| item.name == "IoError"));
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body.as_slice(),
            [Statement::Let {
                name,
                value_type: ValueType::Enum(result_name, args),
                initializer: ValueExpr::IoReadLine,
            }] if name == "result"
                && result_name == "Result"
                && matches!(
                    args.as_slice(),
                    [
                        ValueType::String,
                        ValueType::Struct(error_name, error_args),
                    ] if error_name == "IoError" && error_args.is_empty()
                )
        ));
    }

    #[test]
    fn accepts_specific_io_read_line_import() {
        let source = r#"package app.main

import std.io.read_line

fn main() -> void {
    let result: Result<string, IoError> = read_line()
}
"#;

        let program = parse_inline(source).unwrap();
        assert_eq!(program.imports, vec!["std.io.read_line"]);
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body.as_slice(),
            [Statement::Let {
                initializer: ValueExpr::IoReadLine,
                ..
            }]
        ));
    }

    #[test]
    fn accepts_string_len_and_concat_builtins() {
        let source = r#"package app.main

import std.io
import std.string

fn main() -> void {
    let message: string = string.concat("No", "mo")
    let count: u64 = string.len(message)
    io.println(message)
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                value_type: ValueType::String,
                initializer: ValueExpr::StringConcat { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[1],
            Statement::Let {
                value_type: ValueType::U64,
                initializer: ValueExpr::StringLen { .. },
                ..
            }
        ));
    }

    #[test]
    fn accepts_specific_string_builtin_imports() {
        let source = r#"package app.main

import std.io
import std.string.concat
import std.string.len

fn main() -> void {
    let message: string = concat("No", "mo")
    let count: u64 = len(message)
    io.println(message)
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                value_type: ValueType::String,
                initializer: ValueExpr::StringConcat { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[1],
            Statement::Let {
                value_type: ValueType::U64,
                initializer: ValueExpr::StringLen { .. },
                ..
            }
        ));
    }

    #[test]
    fn accepts_extended_string_builtins() {
        let source = r#"package app.main

import std.array
import std.string

fn main() -> void {
    let empty: bool = string.is_empty("")
    let contains: bool = string.contains("nomo", "om")
    let starts: bool = string.starts_with("nomo", "no")
    let ends: bool = string.ends_with("nomo", "mo")
    let parts: Array<string> = string.split("no,mo", ",")
    let trimmed: string = string.trim(" nomo ")
    let lower: string = string.to_lower("NOMO")
    let upper: string = string.to_upper("nomo")
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                value_type: ValueType::Bool,
                initializer: ValueExpr::StringIsEmpty { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[1],
            Statement::Let {
                value_type: ValueType::Bool,
                initializer: ValueExpr::StringContains { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[2],
            Statement::Let {
                value_type: ValueType::Bool,
                initializer: ValueExpr::StringStartsWith { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[3],
            Statement::Let {
                value_type: ValueType::Bool,
                initializer: ValueExpr::StringEndsWith { .. },
                ..
            }
        ));
        assert!(matches!(
            &main.body[4],
            Statement::Let {
                value_type: ValueType::Array(element),
                initializer: ValueExpr::StringSplit { .. },
                ..
            } if **element == ValueType::String
        ));
        assert!(matches!(
            main.body[5],
            Statement::Let {
                value_type: ValueType::String,
                initializer: ValueExpr::StringTrim { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[6],
            Statement::Let {
                value_type: ValueType::String,
                initializer: ValueExpr::StringToLower { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[7],
            Statement::Let {
                value_type: ValueType::String,
                initializer: ValueExpr::StringToUpper { .. },
                ..
            }
        ));
    }

    #[test]
    fn accepts_specific_extended_string_builtin_imports() {
        let source = r#"package app.main

import std.array
import std.string.contains
import std.string.split
import std.string.to_upper

fn main() -> void {
    let found: bool = contains("nomo", "no")
    let parts: Array<string> = split("no/mo", "/")
    let loud: string = to_upper("nomo")
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                value_type: ValueType::Bool,
                initializer: ValueExpr::StringContains { .. },
                ..
            }
        ));
        assert!(matches!(
            &main.body[1],
            Statement::Let {
                value_type: ValueType::Array(element),
                initializer: ValueExpr::StringSplit { .. },
                ..
            } if **element == ValueType::String
        ));
        assert!(matches!(
            main.body[2],
            Statement::Let {
                value_type: ValueType::String,
                initializer: ValueExpr::StringToUpper { .. },
                ..
            }
        ));
    }

    #[test]
    fn accepts_extended_string_value_methods() {
        let source = r#"package app.main

import std.array
import std.string

fn main() -> void {
    let text: string = " NoMo "
    let empty: bool = text.is_empty()
    let contains: bool = text.contains("No")
    let starts: bool = text.starts_with(" ")
    let ends: bool = text.ends_with(" ")
    let parts: Array<string> = text.split("o")
    let trimmed: string = text.trim()
    let lower: string = text.to_lower()
    let upper: string = text.to_upper()
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[1],
            Statement::Let {
                value_type: ValueType::Bool,
                initializer: ValueExpr::StringIsEmpty { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[2],
            Statement::Let {
                value_type: ValueType::Bool,
                initializer: ValueExpr::StringContains { .. },
                ..
            }
        ));
        assert!(matches!(
            &main.body[5],
            Statement::Let {
                value_type: ValueType::Array(element),
                initializer: ValueExpr::StringSplit { .. },
                ..
            } if **element == ValueType::String
        ));
        assert!(matches!(
            main.body[8],
            Statement::Let {
                value_type: ValueType::String,
                initializer: ValueExpr::StringToUpper { .. },
                ..
            }
        ));
    }

    #[test]
    fn accepts_path_builtins() {
        let source = r#"package app.main

import std.path

fn main() -> void {
    let joined: string = path.join("/tmp", "nomo.txt")
    let base: string = path.basename(joined)
    let dir: string = path.dirname(joined)
    let ext: string = path.extension("archive.tar.gz")
    let clean: string = path.normalize("/tmp//a/../b/./")
    let absolute: bool = path.is_absolute(clean)
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                value_type: ValueType::String,
                initializer: ValueExpr::PathJoin { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[1],
            Statement::Let {
                value_type: ValueType::String,
                initializer: ValueExpr::PathBasename { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[2],
            Statement::Let {
                value_type: ValueType::String,
                initializer: ValueExpr::PathDirname { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[3],
            Statement::Let {
                value_type: ValueType::String,
                initializer: ValueExpr::PathExtension { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[4],
            Statement::Let {
                value_type: ValueType::String,
                initializer: ValueExpr::PathNormalize { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[5],
            Statement::Let {
                value_type: ValueType::Bool,
                initializer: ValueExpr::PathIsAbsolute { .. },
                ..
            }
        ));
    }

    #[test]
    fn accepts_specific_path_builtin_imports() {
        let source = r#"package app.main

import std.path.basename
import std.path.is_absolute

fn main() -> void {
    let name: string = basename("/tmp/nomo.txt")
    let absolute: bool = is_absolute("/tmp")
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                initializer: ValueExpr::PathBasename { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[1],
            Statement::Let {
                value_type: ValueType::Bool,
                initializer: ValueExpr::PathIsAbsolute { .. },
                ..
            }
        ));
    }

    #[test]
    fn rejects_path_builtin_non_string_argument() {
        let source = r#"package app.main

import std.path

fn main() -> void {
    let name: string = path.basename(1)
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0404");
        assert!(err.message.contains("path.basename"));
        assert!(err.message.contains("string"));
    }

    #[test]
    fn accepts_process_builtins() {
        let source = r#"package app.main

import std.process
import std.result

fn capture() -> Result<ProcessOutput, ProcessError> {
    return process.output("printf captured-ok")?
}

fn run() -> Result<string, ProcessError> {
    let spawned: i32 = process.spawn("printf spawn-ok >/dev/null")?
    let status: i32 = process.status("printf status-ok >/dev/null")?
    return process.exec("printf process-ok")
}

fn quit() -> void {
    process.exit(0)
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        assert!(
            program
                .structs
                .iter()
                .any(|item| item.name == "ProcessError")
        );
        assert!(
            program
                .structs
                .iter()
                .any(|item| item.name == "ProcessOutput")
        );
        let capture = program
            .functions
            .iter()
            .find(|f| f.name == "capture")
            .unwrap();
        assert!(matches!(
            capture.body[0],
            Statement::QuestionReturnOk {
                result_expr: ValueExpr::ProcessOutput { .. },
                ..
            }
        ));
        let run = program.functions.iter().find(|f| f.name == "run").unwrap();
        assert!(matches!(
            run.body[0],
            Statement::QuestionLet {
                value_type: ValueType::I32,
                result_expr: ValueExpr::ProcessSpawn { .. },
                ..
            }
        ));
        assert!(matches!(
            run.body[1],
            Statement::QuestionLet {
                value_type: ValueType::I32,
                result_expr: ValueExpr::ProcessStatus { .. },
                ..
            }
        ));
        assert!(matches!(
            run.body[2],
            Statement::Return(Some(ValueExpr::ProcessExec { .. }))
        ));
        let quit = program.functions.iter().find(|f| f.name == "quit").unwrap();
        assert!(matches!(
            quit.body[0],
            Statement::Expr(ValueExpr::ProcessExit { .. })
        ));
    }

    #[test]
    fn accepts_specific_process_builtin_imports() {
        let source = r#"package app.main

import std.process.exec
import std.process.output
import std.process.spawn
import std.process.status
import std.result

fn capture() -> Result<ProcessOutput, ProcessError> {
    return output("printf captured-ok")?
}

fn run() -> Result<string, ProcessError> {
    let spawned: i32 = spawn("printf spawn-ok >/dev/null")?
    let status: i32 = status("printf status-ok >/dev/null")?
    return exec("printf process-ok")
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let capture = program
            .functions
            .iter()
            .find(|f| f.name == "capture")
            .unwrap();
        assert!(matches!(
            capture.body[0],
            Statement::QuestionReturnOk {
                result_expr: ValueExpr::ProcessOutput { .. },
                ..
            }
        ));
        let run = program.functions.iter().find(|f| f.name == "run").unwrap();
        assert!(matches!(
            run.body[0],
            Statement::QuestionLet {
                result_expr: ValueExpr::ProcessSpawn { .. },
                ..
            }
        ));
        assert!(matches!(
            run.body[1],
            Statement::QuestionLet {
                result_expr: ValueExpr::ProcessStatus { .. },
                ..
            }
        ));
        assert!(matches!(
            run.body[2],
            Statement::Return(Some(ValueExpr::ProcessExec { .. }))
        ));
    }

    #[test]
    fn accepts_testing_builtins() {
        let source = r#"package app.main

import std.result
import std.testing

fn fail() -> Result<i64, string> {
    return Err("boom")
}

fn main() -> void {
    testing.assert(true, "expected true")
    testing.assert_equal(1, 1)
    testing.assert_equal("same", "same")
    testing.assert_error(fail())
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert_eq!(main.body.len(), 4);
        assert!(matches!(
            main.body[0],
            Statement::Expr(ValueExpr::If { .. })
        ));
        assert!(matches!(
            main.body[1],
            Statement::Expr(ValueExpr::If { .. })
        ));
        assert!(matches!(
            main.body[2],
            Statement::Expr(ValueExpr::If { .. })
        ));
        assert!(matches!(
            main.body[3],
            Statement::Expr(ValueExpr::If { .. })
        ));
    }

    #[test]
    fn accepts_specific_testing_builtin_imports() {
        let source = r#"package app.main

import std.result
import std.testing.assert
import std.testing.assert_equal
import std.testing.assert_error

fn fail() -> Result<i64, string> {
    return Err("boom")
}

fn main() -> void {
    assert(true, "expected true")
    assert_equal('n', 'n')
    assert_error(fail())
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert_eq!(main.body.len(), 3);
        assert!(
            main.body
                .iter()
                .all(|stmt| matches!(stmt, Statement::Expr(ValueExpr::If { .. })))
        );
    }

    #[test]
    fn rejects_testing_assert_non_bool_condition() {
        let source = r#"package app.main

import std.testing

fn main() -> void {
    testing.assert("nope", "expected bool")
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0404");
        assert!(err.message.contains("bool condition"));
    }

    #[test]
    fn rejects_testing_assert_error_non_result() {
        let source = r#"package app.main

import std.testing

fn main() -> void {
    testing.assert_error(1)
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0404");
        assert!(err.message.contains("Result<T, E>"));
    }

    #[test]
    fn accepts_debug_builtins() {
        let source = r#"package app.main

import std.debug

fn crash() -> void {
    debug.panic("boom")
}

fn main() -> void {
    debug.print("debug-")
    debug.println("ok")
    let trace: string = debug.backtrace()
}
"#;

        let program = parse_inline(source).unwrap();
        let crash = program
            .functions
            .iter()
            .find(|f| f.name == "crash")
            .unwrap();
        assert!(matches!(
            crash.body[0],
            Statement::Expr(ValueExpr::Panic { .. })
        ));
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Expr(ValueExpr::Call { ref name, .. }) if name == BUILTIN_EPRINT_EXPR
        ));
        assert!(matches!(
            main.body[1],
            Statement::Expr(ValueExpr::Call { ref name, .. }) if name == BUILTIN_EPRINTLN_EXPR
        ));
        assert!(matches!(
            main.body[2],
            Statement::Let {
                value_type: ValueType::String,
                initializer: ValueExpr::StringLiteral(ref value),
                ..
            } if value == "backtrace unavailable"
        ));
    }

    #[test]
    fn accepts_specific_debug_backtrace_import() {
        let source = r#"package app.main

import std.debug.backtrace

fn main() -> void {
    let trace: string = backtrace()
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                initializer: ValueExpr::StringLiteral(ref value),
                ..
            } if value == "backtrace unavailable"
        ));
    }

    #[test]
    fn rejects_debug_print_non_string_message() {
        let source = r#"package app.main

import std.debug

fn main() -> void {
    debug.println(1)
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0404");
        assert!(err.message.contains("string message"));
    }

    #[test]
    fn accepts_log_builtins() {
        let source = r#"package app.main

import std.log

fn main() -> void {
    log.debug("hidden")
    log.info("hello")
    log.warn("careful")
    log.error("bad")
    let enabled: bool = log.enabled("debug")
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert_eq!(main.body.len(), 5);
        assert!(
            main.body[..4]
                .iter()
                .all(|stmt| matches!(stmt, Statement::Expr(ValueExpr::If { .. })))
        );
        assert!(matches!(
            main.body[4],
            Statement::Let {
                value_type: ValueType::Bool,
                initializer: ValueExpr::LogEnabled { .. },
                ..
            }
        ));
    }

    #[test]
    fn accepts_specific_log_builtin_imports() {
        let source = r#"package app.main

import std.log.enabled
import std.log.info

fn main() -> void {
    info("hello")
    let enabled: bool = enabled("info")
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Expr(ValueExpr::If { .. })
        ));
        assert!(matches!(
            main.body[1],
            Statement::Let {
                value_type: ValueType::Bool,
                initializer: ValueExpr::LogEnabled { .. },
                ..
            }
        ));
    }

    #[test]
    fn rejects_log_non_string_message() {
        let source = r#"package app.main

import std.log

fn main() -> void {
    log.info(1)
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0404");
        assert!(err.message.contains("string message"));
    }

    #[test]
    fn accepts_hash_builtins() {
        let source = r#"package app.main

import std.hash

fn main() -> void {
    let direct: u64 = hash.string("nomo")
    let state: HashState = hash.new()
    let written: HashState = hash.write_string(state, "nomo")
    let finished: u64 = hash.finish(written)
}
"#;

        let program = parse_inline(source).unwrap();
        assert!(program.structs.iter().any(|item| item.name == "HashState"));
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                value_type: ValueType::U64,
                initializer: ValueExpr::HashString { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[1],
            Statement::Let {
                value_type: ValueType::Struct(ref name, ref args),
                initializer: ValueExpr::HashNew,
                ..
            } if name == "HashState" && args.is_empty()
        ));
        assert!(matches!(
            main.body[2],
            Statement::Let {
                value_type: ValueType::Struct(ref name, ref args),
                initializer: ValueExpr::HashWriteString { .. },
                ..
            } if name == "HashState" && args.is_empty()
        ));
        assert!(matches!(
            main.body[3],
            Statement::Let {
                value_type: ValueType::U64,
                initializer: ValueExpr::HashFinish { .. },
                ..
            }
        ));
    }

    #[test]
    fn accepts_specific_hash_builtin_imports() {
        let source = r#"package app.main

import std.hash.HashState
import std.hash.finish
import std.hash.new
import std.hash.string
import std.hash.write_string

fn main() -> void {
    let direct: u64 = string("nomo")
    let state: HashState = new()
    let written: HashState = write_string(state, "nomo")
    let finished: u64 = finish(written)
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                initializer: ValueExpr::HashString { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[1],
            Statement::Let {
                initializer: ValueExpr::HashNew,
                ..
            }
        ));
        assert!(matches!(
            main.body[2],
            Statement::Let {
                initializer: ValueExpr::HashWriteString { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[3],
            Statement::Let {
                initializer: ValueExpr::HashFinish { .. },
                ..
            }
        ));
    }

    #[test]
    fn rejects_hash_non_string_value() {
        let source = r#"package app.main

import std.hash

fn main() -> void {
    let value: u64 = hash.string(1)
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0404");
        assert!(err.message.contains("string value"));
    }

    #[test]
    fn accepts_crypto_builtins() {
        let source = r#"package app.main

import std.crypto
import std.array.Array

fn main() -> void {
    let sha256: string = crypto.sha256("nomo")
    let sha512: string = crypto.sha512("nomo")
    let bytes: Array<u32> = crypto.random_bytes(4 as u64)
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                value_type: ValueType::String,
                initializer: ValueExpr::CryptoSha256 { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[1],
            Statement::Let {
                value_type: ValueType::String,
                initializer: ValueExpr::CryptoSha512 { .. },
                ..
            }
        ));
        assert!(matches!(
            &main.body[2],
            Statement::Let {
                value_type: ValueType::Array(element_type),
                initializer: ValueExpr::CryptoRandomBytes { .. },
                ..
            } if **element_type == ValueType::U32
        ));
    }

    #[test]
    fn accepts_specific_crypto_builtin_imports() {
        let source = r#"package app.main

import std.crypto.sha256
import std.crypto.sha512
import std.crypto.random_bytes
import std.array.Array

fn main() -> void {
    let left: string = sha256("nomo")
    let right: string = sha512("nomo")
    let bytes: Array<u32> = random_bytes(4 as u64)
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                initializer: ValueExpr::CryptoSha256 { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[1],
            Statement::Let {
                initializer: ValueExpr::CryptoSha512 { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[2],
            Statement::Let {
                initializer: ValueExpr::CryptoRandomBytes { .. },
                ..
            }
        ));
    }

    #[test]
    fn accepts_json_builtins() {
        let source = r#"package app.main

import std.json

fn main() -> Result<void, JsonError> {
    let parsed: Result<JsonValue, JsonError> = json.parse("{\"lang\":\"nomo\"}")
    let value: JsonValue = parsed?
    let text: string = json.stringify(value)
    return Ok(void)
}
"#;

        let program = parse_inline(source).unwrap();
        assert!(program.structs.iter().any(|item| item.name == "JsonValue"));
        assert!(program.structs.iter().any(|item| item.name == "JsonError"));
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                initializer: ValueExpr::JsonParse { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[2],
            Statement::Let {
                value_type: ValueType::String,
                initializer: ValueExpr::JsonStringify { .. },
                ..
            }
        ));
    }

    #[test]
    fn accepts_specific_json_builtin_imports() {
        let source = r#"package app.main

import std.json.JsonError
import std.json.JsonValue
import std.json.parse
import std.json.stringify

fn main() -> Result<void, JsonError> {
    let parsed: Result<JsonValue, JsonError> = parse("true")
    let value: JsonValue = parsed?
    let text: string = stringify(value)
    return Ok(void)
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                initializer: ValueExpr::JsonParse { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[2],
            Statement::Let {
                initializer: ValueExpr::JsonStringify { .. },
                ..
            }
        ));
    }

    #[test]
    fn accepts_regex_builtins_with_question() {
        let source = r#"package app.main

import std.regex
import std.array

fn main() -> Result<void, RegexError> {
    let compiled: Result<Regex, RegexError> = regex.compile("(nomo)-([0-9]+)")
    let rx: Regex = compiled?
    let matched: bool = regex.is_match(rx, "hello nomo-42")
    let groups: Option<Array<string>> = regex.captures(rx, "hello nomo-42")
    return Ok(void)
}
"#;

        let program = parse_inline(source).unwrap();
        assert!(program.structs.iter().any(|item| item.name == "Regex"));
        assert!(program.structs.iter().any(|item| item.name == "RegexError"));
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                initializer: ValueExpr::RegexCompile { .. },
                ..
            }
        ));
        assert!(main.body.iter().any(|stmt| matches!(
            stmt,
            Statement::QuestionLet {
                carrier: QuestionCarrier::Result,
                ..
            }
        )));
        assert!(main.body.iter().any(|stmt| {
            matches!(
                stmt,
                Statement::Let {
                    value_type: ValueType::Bool,
                    initializer: ValueExpr::RegexIsMatch { .. },
                    ..
                }
            )
        }));
        assert!(main.body.iter().any(|stmt| {
            matches!(
                stmt,
                Statement::Let {
                    value_type: ValueType::Enum(name, args),
                    initializer: ValueExpr::RegexCaptures { .. },
                    ..
                } if name == "Option" && args == &[ValueType::Array(Box::new(ValueType::String))]
            )
        }));
    }

    #[test]
    fn accepts_specific_regex_builtin_imports() {
        let source = r#"package app.main

import std.regex.Regex
import std.regex.RegexError
import std.regex.captures
import std.regex.compile
import std.regex.is_match
import std.array.Array

fn main() -> Result<void, RegexError> {
    let rx: Regex = compile("nomo")?
    let matched: bool = is_match(rx, "nomo")
    let groups: Option<Array<string>> = captures(rx, "nomo")
    return Ok(void)
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(main.body.iter().any(|stmt| matches!(
            stmt,
            Statement::QuestionLet {
                result_expr: ValueExpr::RegexCompile { .. },
                ..
            }
        )));
        assert!(main.body.iter().any(|stmt| {
            matches!(
                stmt,
                Statement::Let {
                    initializer: ValueExpr::RegexIsMatch { .. },
                    ..
                }
            )
        }));
        assert!(main.body.iter().any(|stmt| {
            matches!(
                stmt,
                Statement::Let {
                    initializer: ValueExpr::RegexCaptures { .. },
                    ..
                }
            )
        }));
    }

    #[test]
    fn rejects_regex_compile_non_string_pattern() {
        let source = r#"package app.main

import std.regex

fn main() -> void {
    let parsed: Result<Regex, RegexError> = regex.compile(42)
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0404");
        assert!(err.message.contains("regex.compile"));
        assert_eq!(err.expected.as_deref(), Some("string"));
        assert_eq!(err.found.as_deref(), Some("i64"));
    }

    #[test]
    fn rejects_json_parse_non_string_argument() {
        let source = r#"package app.main

import std.json

fn main() -> void {
    let parsed: Result<JsonValue, JsonError> = json.parse(42)
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0404");
        assert!(err.message.contains("json.parse"));
        assert_eq!(err.expected.as_deref(), Some("string"));
        assert_eq!(err.found.as_deref(), Some("i64"));
    }

    #[test]
    fn rejects_crypto_non_string_value() {
        let source = r#"package app.main

import std.crypto

fn main() -> void {
    let value: string = crypto.sha256(1)
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0404");
        assert!(err.message.contains("string value"));
    }

    #[test]
    fn rejects_crypto_random_bytes_non_u64_count() {
        let source = r#"package app.main

import std.crypto
import std.array.Array

fn main() -> void {
    let value: Array<u32> = crypto.random_bytes("four")
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0404");
        assert!(err.message.contains("crypto.random_bytes"));
        assert_eq!(err.expected.as_deref(), Some("u64"));
        assert_eq!(err.found.as_deref(), Some("string"));
    }

    #[test]
    fn accepts_collections_builtins() {
        let source = r#"package app.main

import std.collections

fn main() -> void {
    let map: StringMap = collections.map_new()
    let updated: StringMap = collections.map_set(map, "lang", "nomo")
    let found: Option<string> = collections.map_get(updated, "lang")
    let has_lang: bool = collections.map_contains(updated, "lang")
    let smaller: StringMap = collections.map_remove(updated, "lang")
    let set: StringSet = collections.set_new()
    let inserted: StringSet = collections.set_insert(set, "nomo")
    let has_nomo: bool = collections.set_contains(inserted, "nomo")
    let removed: StringSet = collections.set_remove(inserted, "nomo")
}
"#;

        let program = parse_inline(source).unwrap();
        assert!(program.structs.iter().any(|item| item.name == "StringMap"));
        assert!(program.structs.iter().any(|item| item.name == "StringSet"));
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                initializer: ValueExpr::CollectionsStringMapNew,
                ..
            }
        ));
        assert!(matches!(
            main.body[1],
            Statement::Let {
                initializer: ValueExpr::CollectionsStringMapSet { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[2],
            Statement::Let {
                value_type: ValueType::Enum(ref name, ref args),
                initializer: ValueExpr::CollectionsStringMapGet { .. },
                ..
            } if name == "Option" && args == &[ValueType::String]
        ));
        assert!(matches!(
            main.body[3],
            Statement::Let {
                value_type: ValueType::Bool,
                initializer: ValueExpr::CollectionsStringMapContains { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[4],
            Statement::Let {
                initializer: ValueExpr::CollectionsStringMapRemove { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[5],
            Statement::Let {
                initializer: ValueExpr::CollectionsStringSetNew,
                ..
            }
        ));
        assert!(matches!(
            main.body[6],
            Statement::Let {
                initializer: ValueExpr::CollectionsStringSetInsert { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[7],
            Statement::Let {
                value_type: ValueType::Bool,
                initializer: ValueExpr::CollectionsStringSetContains { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[8],
            Statement::Let {
                initializer: ValueExpr::CollectionsStringSetRemove { .. },
                ..
            }
        ));
    }

    #[test]
    fn accepts_specific_collections_builtin_imports() {
        let source = r#"package app.main

import std.collections.StringMap
import std.collections.StringSet
import std.collections.map_get
import std.collections.map_new
import std.collections.map_set
import std.collections.set_insert
import std.collections.set_new

fn main() -> void {
    let map: StringMap = map_new()
    let updated: StringMap = map_set(map, "lang", "nomo")
    let found: Option<string> = map_get(updated, "lang")
    let set: StringSet = set_new()
    let inserted: StringSet = set_insert(set, "nomo")
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                initializer: ValueExpr::CollectionsStringMapNew,
                ..
            }
        ));
        assert!(matches!(
            main.body[1],
            Statement::Let {
                initializer: ValueExpr::CollectionsStringMapSet { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[2],
            Statement::Let {
                initializer: ValueExpr::CollectionsStringMapGet { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[3],
            Statement::Let {
                initializer: ValueExpr::CollectionsStringSetNew,
                ..
            }
        ));
        assert!(matches!(
            main.body[4],
            Statement::Let {
                initializer: ValueExpr::CollectionsStringSetInsert { .. },
                ..
            }
        ));
    }

    #[test]
    fn accepts_char_builtins() {
        let source = r#"package app.main

import std.char

fn main() -> void {
    let digit: bool = char.is_digit('7')
    let alpha: bool = char.is_alpha('N')
    let space: bool = char.is_whitespace(' ')
    let text: string = char.to_string('語')
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                value_type: ValueType::Bool,
                initializer: ValueExpr::CharIsDigit { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[1],
            Statement::Let {
                value_type: ValueType::Bool,
                initializer: ValueExpr::CharIsAlpha { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[2],
            Statement::Let {
                value_type: ValueType::Bool,
                initializer: ValueExpr::CharIsWhitespace { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[3],
            Statement::Let {
                value_type: ValueType::String,
                initializer: ValueExpr::CharToString { .. },
                ..
            }
        ));
    }

    #[test]
    fn accepts_specific_char_builtin_imports() {
        let source = r#"package app.main

import std.char.is_digit
import std.char.to_string

fn main() -> void {
    let digit: bool = is_digit('7')
    let text: string = to_string('N')
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                initializer: ValueExpr::CharIsDigit { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[1],
            Statement::Let {
                initializer: ValueExpr::CharToString { .. },
                ..
            }
        ));
    }

    #[test]
    fn rejects_char_builtin_non_char_argument() {
        let source = r#"package app.main

import std.char

fn main() -> void {
    let value: bool = char.is_digit("7")
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0404");
        assert_eq!(err.expected.as_deref(), Some("char"));
        assert_eq!(err.found.as_deref(), Some("string"));
    }

    #[test]
    fn accepts_os_builtins() {
        let source = r#"package app.main

import std.os

fn main() -> void {
    let platform: string = os.platform()
    let arch: string = os.arch()
    let separator: string = os.path_separator()
    let ending: string = os.line_ending()
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                value_type: ValueType::String,
                initializer: ValueExpr::OsPlatform,
                ..
            }
        ));
        assert!(matches!(
            main.body[1],
            Statement::Let {
                value_type: ValueType::String,
                initializer: ValueExpr::OsArch,
                ..
            }
        ));
        assert!(matches!(
            main.body[2],
            Statement::Let {
                value_type: ValueType::String,
                initializer: ValueExpr::OsPathSeparator,
                ..
            }
        ));
        assert!(matches!(
            main.body[3],
            Statement::Let {
                value_type: ValueType::String,
                initializer: ValueExpr::OsLineEnding,
                ..
            }
        ));
    }

    #[test]
    fn accepts_specific_os_builtin_imports() {
        let source = r#"package app.main

import std.os.platform
import std.os.path_separator

fn main() -> void {
    let platform: string = platform()
    let separator: string = path_separator()
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                initializer: ValueExpr::OsPlatform,
                ..
            }
        ));
        assert!(matches!(
            main.body[1],
            Statement::Let {
                initializer: ValueExpr::OsPathSeparator,
                ..
            }
        ));
    }

    #[test]
    fn rejects_os_builtin_arguments() {
        let source = r#"package app.main

import std.os

fn main() -> void {
    let platform: string = os.platform("extra")
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0407");
        assert!(err.message.contains("os.platform"));
    }

    #[test]
    fn accepts_time_builtins() {
        let source = r#"package app.main

import std.time

fn main() -> void {
    let now: i64 = time.now_millis()
    let monotonic: i64 = time.monotonic_millis()
    let short: Duration = time.duration_millis(1500)
    let long: Duration = time.duration_seconds(2)
    let short_millis: i64 = time.duration_as_millis(short)
    let label: string = time.format_duration(short)
    time.sleep(time.duration_millis(0))
    time.sleep_millis(0)
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                value_type: ValueType::Int,
                initializer: ValueExpr::TimeNowMillis,
                ..
            }
        ));
        assert!(matches!(
            main.body[1],
            Statement::Let {
                value_type: ValueType::Int,
                initializer: ValueExpr::TimeMonotonicMillis,
                ..
            }
        ));
        assert!(matches!(
            main.body[2],
            Statement::Let {
                value_type: ValueType::Struct(ref name, ref args),
                initializer: ValueExpr::TimeDurationMillis { .. },
                ..
            } if name == "Duration" && args.is_empty()
        ));
        assert!(matches!(
            main.body[3],
            Statement::Let {
                value_type: ValueType::Struct(ref name, ref args),
                initializer: ValueExpr::TimeDurationSeconds { .. },
                ..
            } if name == "Duration" && args.is_empty()
        ));
        assert!(matches!(
            main.body[4],
            Statement::Let {
                value_type: ValueType::Int,
                initializer: ValueExpr::TimeDurationAsMillis { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[5],
            Statement::Let {
                value_type: ValueType::String,
                initializer: ValueExpr::TimeFormatDuration { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[6],
            Statement::Expr(ValueExpr::TimeSleep { .. })
        ));
        assert!(matches!(
            main.body[7],
            Statement::Expr(ValueExpr::TimeSleepMillis { .. })
        ));
    }

    #[test]
    fn accepts_specific_time_builtin_imports() {
        let source = r#"package app.main

import std.time.now_millis
import std.time.duration_millis
import std.time.duration_as_millis
import std.time.sleep
import std.time.sleep_millis

fn main() -> void {
    let now: i64 = now_millis()
    let duration: Duration = duration_millis(5)
    let millis: i64 = duration_as_millis(duration)
    sleep(duration_millis(0))
    sleep_millis(0)
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                initializer: ValueExpr::TimeNowMillis,
                ..
            }
        ));
        assert!(matches!(
            main.body[1],
            Statement::Let {
                initializer: ValueExpr::TimeDurationMillis { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[2],
            Statement::Let {
                initializer: ValueExpr::TimeDurationAsMillis { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[3],
            Statement::Expr(ValueExpr::TimeSleep { .. })
        ));
        assert!(matches!(
            main.body[4],
            Statement::Expr(ValueExpr::TimeSleepMillis { .. })
        ));
    }

    #[test]
    fn rejects_time_sleep_non_i64_argument() {
        let source = r#"package app.main

import std.time

fn main() -> void {
    time.sleep_millis("soon")
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0404");
        assert!(err.message.contains("time.sleep_millis"));
        assert_eq!(err.expected.as_deref(), Some("i64"));
        assert_eq!(err.found.as_deref(), Some("string"));
    }

    #[test]
    fn rejects_time_sleep_non_duration_argument() {
        let source = r#"package app.main

import std.time

fn main() -> void {
    time.sleep(1)
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0404");
        assert!(err.message.contains("time.sleep"));
        assert_eq!(err.expected.as_deref(), Some("Duration"));
        assert_eq!(err.found.as_deref(), Some("i64"));
    }

    #[test]
    fn accepts_num_builtins() {
        let source = r#"package app.main

import std.num

fn main() -> void {
    let integer: Result<i64, NumError> = num.parse_i64("42")
    let unsigned: Result<u64, NumError> = num.parse_u64("7")
    let decimal: Result<f64, NumError> = num.parse_f64("3.5")
    let text: string = num.to_string(42)
}
"#;

        let program = parse_inline(source).unwrap();
        assert!(
            program
                .structs
                .iter()
                .any(|struct_type| struct_type.name == "NumError")
        );
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                initializer: ValueExpr::NumParseI64 { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[1],
            Statement::Let {
                initializer: ValueExpr::NumParseU64 { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[2],
            Statement::Let {
                initializer: ValueExpr::NumParseF64 { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[3],
            Statement::Let {
                initializer: ValueExpr::NumToString { .. },
                ..
            }
        ));
    }

    #[test]
    fn accepts_specific_num_parse_imports() {
        let source = r#"package app.main

import std.num.parse_i64
import std.num.parse_u64
import std.num.parse_f64

fn main() -> void {
    let integer: Result<i64, NumError> = parse_i64("42")
    let unsigned: Result<u64, NumError> = parse_u64("7")
    let decimal: Result<f64, NumError> = parse_f64("3.5")
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                initializer: ValueExpr::NumParseI64 { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[1],
            Statement::Let {
                initializer: ValueExpr::NumParseU64 { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[2],
            Statement::Let {
                initializer: ValueExpr::NumParseF64 { .. },
                ..
            }
        ));
    }

    #[test]
    fn accepts_num_checked_and_wrapping_builtins() {
        let source = r#"package app.main

import std.num

fn main() -> void {
    let checked: Option<i64> = num.checked_add(9223372036854775807, 1)
    let wrapped: i64 = num.wrapping_add(9223372036854775807, 1)
    let checked32: Option<i32> = num.checked_mul(100000 as i32, 100000 as i32)
    let wrapped64: u64 = num.wrapping_sub(0 as u64, 1 as u64)
}
"#;

        let program = parse_inline(source).unwrap();
        assert!(program.enums.iter().any(|item| item.name == "Option"));
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            &main.body[0],
            Statement::Let {
                value_type: ValueType::Enum(name, args),
                initializer:
                    ValueExpr::NumBinary {
                        function: NumBinaryFunction::Checked,
                        op: BinaryOp::Add,
                        ..
                    },
                ..
            } if name == "Option" && args == &vec![ValueType::Int]
        ));
        assert!(matches!(
            main.body[1],
            Statement::Let {
                value_type: ValueType::Int,
                initializer: ValueExpr::NumBinary {
                    function: NumBinaryFunction::Wrapping,
                    op: BinaryOp::Add,
                    ..
                },
                ..
            }
        ));
        assert!(matches!(
            &main.body[2],
            Statement::Let {
                value_type: ValueType::Enum(name, args),
                initializer:
                    ValueExpr::NumBinary {
                        function: NumBinaryFunction::Checked,
                        op: BinaryOp::Multiply,
                        ..
                    },
                ..
            } if name == "Option" && args == &vec![ValueType::I32]
        ));
        assert!(matches!(
            main.body[3],
            Statement::Let {
                value_type: ValueType::U64,
                initializer: ValueExpr::NumBinary {
                    function: NumBinaryFunction::Wrapping,
                    op: BinaryOp::Subtract,
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn accepts_specific_num_checked_and_wrapping_imports() {
        let source = r#"package app.main

import std.num.checked_add
import std.num.wrapping_mul

fn main() -> void {
    let checked: Option<u32> = checked_add(1 as u32, 2 as u32)
    let wrapped: u32 = wrapping_mul(3 as u32, 4 as u32)
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                initializer: ValueExpr::NumBinary {
                    function: NumBinaryFunction::Checked,
                    op: BinaryOp::Add,
                    ..
                },
                ..
            }
        ));
        assert!(matches!(
            main.body[1],
            Statement::Let {
                initializer: ValueExpr::NumBinary {
                    function: NumBinaryFunction::Wrapping,
                    op: BinaryOp::Multiply,
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn rejects_num_parse_non_string_argument() {
        let source = r#"package app.main

import std.num

fn main() -> void {
    let parsed: Result<i64, NumError> = num.parse_i64(42)
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0404");
        assert!(err.message.contains("num.parse_i64"));
        assert_eq!(err.expected.as_deref(), Some("string"));
        assert_eq!(err.found.as_deref(), Some("i64"));
    }

    #[test]
    fn rejects_num_to_string_non_numeric_argument() {
        let source = r#"package app.main

import std.num

fn main() -> void {
    let text: string = num.to_string(true)
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0404");
        assert!(err.message.contains("num.to_string"));
    }

    #[test]
    fn rejects_num_checked_mismatched_operands() {
        let source = r#"package app.main

import std.num

fn main() -> void {
    let value: Option<i64> = num.checked_add(1, true)
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0404");
        assert!(err.message.contains("num.checked_add"));
    }

    #[test]
    fn rejects_user_num_error_when_std_num_is_needed() {
        let source = r#"package app.main

import std.num

struct NumError {
    message: string
}

fn main() -> void {
    let parsed: Result<i64, NumError> = num.parse_i64("42")
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0312");
        assert!(err.message.contains("NumError"));
        assert!(err.message.contains("standard library"));
    }

    #[test]
    fn accepts_math_builtins() {
        let source = r#"package app.main

import std.math

fn main() -> void {
    let absolute: i64 = math.abs(0 - 7)
    let lower: i32 = math.min(3 as i32, 9 as i32)
    let upper: u64 = math.max(5 as u64, 2 as u64)
    let floored: f64 = math.floor(3.8)
    let ceiled: f64 = math.ceil(3.1)
    let rounded: f64 = math.round(3.5)
    let root: f64 = math.sqrt(9.0)
    let powered: f64 = math.pow(2.0, 3.0)
    let sine: f64 = math.sin(0.0)
    let cosine: f64 = math.cos(0.0)
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                value_type: ValueType::Int,
                initializer: ValueExpr::MathUnary {
                    function: MathUnaryFunction::Abs,
                    ..
                },
                ..
            }
        ));
        assert!(matches!(
            main.body[1],
            Statement::Let {
                value_type: ValueType::I32,
                initializer: ValueExpr::MathBinary {
                    function: MathBinaryFunction::Min,
                    ..
                },
                ..
            }
        ));
        assert!(matches!(
            main.body[7],
            Statement::Let {
                value_type: ValueType::Float,
                initializer: ValueExpr::MathBinary {
                    function: MathBinaryFunction::Pow,
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn accepts_specific_math_builtin_imports() {
        let source = r#"package app.main

import std.math.abs
import std.math.sqrt

fn main() -> void {
    let value: i64 = abs(0 - 2)
    let root: f64 = sqrt(16.0)
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                initializer: ValueExpr::MathUnary {
                    function: MathUnaryFunction::Abs,
                    ..
                },
                ..
            }
        ));
        assert!(matches!(
            main.body[1],
            Statement::Let {
                initializer: ValueExpr::MathUnary {
                    function: MathUnaryFunction::Sqrt,
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn rejects_math_builtin_type_mismatch() {
        let source = r#"package app.main

import std.math

fn main() -> void {
    let value: f64 = math.sqrt(9)
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0404");
        assert_eq!(err.expected.as_deref(), Some("f64"));
        assert_eq!(err.found.as_deref(), Some("i64"));
    }

    #[test]
    fn rejects_math_min_mixed_numeric_types() {
        let source = r#"package app.main

import std.math

fn main() -> void {
    let value: i64 = math.min(1, 2 as i32)
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0404");
        assert!(err.message.contains("math.min"));
    }

    #[test]
    fn accepts_string_value_methods() {
        let source = r#"package app.main

import std.io
import std.string

fn main() -> void {
    let prefix: string = "string "
    let message: string = prefix.concat("methods ok")
    let count: u64 = message.len()
    io.println(message)
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[1],
            Statement::Let {
                value_type: ValueType::String,
                initializer: ValueExpr::StringConcat { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[2],
            Statement::Let {
                value_type: ValueType::U64,
                initializer: ValueExpr::StringLen { .. },
                ..
            }
        ));
    }

    #[test]
    fn rejects_string_value_method_without_import() {
        let source = r#"package app.main

fn main() -> void {
    let message: string = "hello"
    let count: u64 = message.len()
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0301");
        assert!(err.message.contains("std.string"));
    }

    #[test]
    fn rejects_string_concat_method_non_string_argument() {
        let source = r#"package app.main

import std.string

fn main() -> void {
    let prefix: string = "nomo"
    let message: string = prefix.concat(1)
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0404");
        assert_eq!(err.expected.as_deref(), Some("string"));
        assert_eq!(err.found.as_deref(), Some("i64"));
    }

    #[test]
    fn accepts_fs_read_and_write_builtins() {
        let source = r#"package app.main

import std.fs
import std.io
import std.array.Array

fn load(path: string) -> Result<string, FsError> {
    let text: string = fs.read_to_string(path)?
    return Result.Ok(text)
}

fn load_bytes(path: string) -> Result<Array<u32>, FsError> {
    let bytes: Array<u32> = fs.read_bytes(path)?
    return Result.Ok(bytes)
}

fn save(path: string, content: string) -> Result<void, FsError> {
    return fs.write_string(path, content)
}

fn save_bytes(path: string, bytes: Array<u32>) -> Result<void, FsError> {
    return fs.write_bytes(path, bytes)
}

fn main() -> void {
    let write_result: Result<void, FsError> = save("/tmp/nomo-fs-test.txt", "hello")
    let read_result: Result<string, FsError> = load("/tmp/nomo-fs-test.txt")
    let byte_read_result: Result<Array<u32>, FsError> = load_bytes("/tmp/nomo-fs-test.txt")
    io.println("done")
}
"#;

        let program = parse_inline(source).unwrap();
        assert!(program.structs.iter().any(|item| item.name == "FsError"));
        assert!(program.enums.iter().any(|item| item.name == "Result"));
        let load = program.functions.iter().find(|f| f.name == "load").unwrap();
        assert_eq!(
            load.return_type,
            ValueType::Enum(
                "Result".to_string(),
                vec![
                    ValueType::String,
                    ValueType::Struct("FsError".to_string(), Vec::new()),
                ],
            )
        );
        assert!(matches!(
            load.body[0],
            Statement::QuestionLet {
                result_expr: ValueExpr::FsReadToString { .. },
                ..
            }
        ));
        let load_bytes = program
            .functions
            .iter()
            .find(|f| f.name == "load_bytes")
            .unwrap();
        assert!(matches!(
            load_bytes.body[0],
            Statement::QuestionLet {
                result_expr: ValueExpr::FsReadBytes { .. },
                ..
            }
        ));
        let save = program.functions.iter().find(|f| f.name == "save").unwrap();
        assert!(matches!(
            save.body[0],
            Statement::Return(Some(ValueExpr::FsWriteString { .. }))
        ));
        let save_bytes = program
            .functions
            .iter()
            .find(|f| f.name == "save_bytes")
            .unwrap();
        assert!(matches!(
            save_bytes.body[0],
            Statement::Return(Some(ValueExpr::FsWriteBytes { .. }))
        ));
    }

    #[test]
    fn accepts_fs_open_and_file_close_defer() {
        let source = r#"package app.main

import std.fs
import std.io

fn close_and_label(file: File) -> string {
    defer file.close()
    return "ok"
}

fn main() -> void {
    let result: Result<File, FsError> = fs.open("/tmp/nomo-file.txt")
    let message: string = match result {
        Result.Ok(file) => close_and_label(file)
        Result.Err(err) => err.message
    }
    io.println(message)
}
"#;

        let program = parse_inline(source).unwrap();
        assert!(program.structs.iter().any(|item| item.name == "File"));
        let close_and_label = program
            .functions
            .iter()
            .find(|f| f.name == "close_and_label")
            .unwrap();
        assert_eq!(
            close_and_label.params[0].value_type,
            ValueType::Struct("File".to_string(), Vec::new())
        );
        assert!(matches!(
            close_and_label.body[0],
            Statement::Defer {
                call: DeferredCall::Expr(ValueExpr::FileClose { .. })
            }
        ));
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                value_type: ValueType::Enum(ref name, ref args),
                initializer: ValueExpr::FsOpen { .. },
                ..
            } if name == "Result"
                && args == &vec![
                    ValueType::Struct("File".to_string(), Vec::new()),
                    ValueType::Struct("FsError".to_string(), Vec::new()),
                ]
        ));
    }

    #[test]
    fn accepts_specific_fs_builtin_imports() {
        let source = r#"package app.main

import std.fs.read_to_string
import std.fs.write_string
import std.fs.read_bytes
import std.fs.write_bytes
import std.io
import std.array.Array

fn load(path: string) -> Result<string, FsError> {
    let text: string = read_to_string(path)?
    return Result.Ok(text)
}

fn load_bytes(path: string) -> Result<Array<u32>, FsError> {
    let bytes: Array<u32> = read_bytes(path)?
    return Result.Ok(bytes)
}

fn save(path: string, content: string) -> Result<void, FsError> {
    return write_string(path, content)
}

fn save_bytes(path: string, bytes: Array<u32>) -> Result<void, FsError> {
    return write_bytes(path, bytes)
}

fn main() -> void {
    let write_result: Result<void, FsError> = save("/tmp/nomo-fs-test.txt", "hello")
    let read_result: Result<string, FsError> = load("/tmp/nomo-fs-test.txt")
    let byte_read_result: Result<Array<u32>, FsError> = load_bytes("/tmp/nomo-fs-test.txt")
    io.println("done")
}
"#;

        let program = parse_inline(source).unwrap();
        assert!(program.structs.iter().any(|item| item.name == "FsError"));
        assert!(program.enums.iter().any(|item| item.name == "Result"));
        let load = program.functions.iter().find(|f| f.name == "load").unwrap();
        assert!(matches!(
            load.body[0],
            Statement::QuestionLet {
                result_expr: ValueExpr::FsReadToString { .. },
                ..
            }
        ));
        let load_bytes = program
            .functions
            .iter()
            .find(|f| f.name == "load_bytes")
            .unwrap();
        assert!(matches!(
            load_bytes.body[0],
            Statement::QuestionLet {
                result_expr: ValueExpr::FsReadBytes { .. },
                ..
            }
        ));
        let save = program.functions.iter().find(|f| f.name == "save").unwrap();
        assert!(matches!(
            save.body[0],
            Statement::Return(Some(ValueExpr::FsWriteString { .. }))
        ));
        let save_bytes = program
            .functions
            .iter()
            .find(|f| f.name == "save_bytes")
            .unwrap();
        assert!(matches!(
            save_bytes.body[0],
            Statement::Return(Some(ValueExpr::FsWriteBytes { .. }))
        ));
    }

    #[test]
    fn accepts_file_read_and_write_string_methods() {
        let source = r#"package app.main

import std.fs

fn rewrite(file: File) -> Result<string, FsError> {
    file.write_string("file ok")?
    let text: string = file.read_to_string()?
    file.close()
    return Ok(text)
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let rewrite = program
            .functions
            .iter()
            .find(|f| f.name == "rewrite")
            .unwrap();
        assert!(matches!(
            rewrite.body[0],
            Statement::QuestionLet {
                result_expr: ValueExpr::FileWriteString { .. },
                ..
            }
        ));
        assert!(rewrite.body.iter().any(|stmt| matches!(
            stmt,
            Statement::QuestionLet {
                result_expr: ValueExpr::FileReadToString { .. },
                ..
            }
        )));
        assert!(
            rewrite
                .body
                .iter()
                .any(|stmt| matches!(stmt, Statement::Expr(ValueExpr::FileClose { .. })))
        );
    }

    #[test]
    fn accepts_fs_directory_builtins() {
        let source = r#"package app.main

import std.fs
import std.array
import std.io

fn prepare(path: string) -> Result<Array<string>, FsError> {
    let present: bool = fs.exists(path)
    let metadata: FileMetadata = fs.metadata(path)?
    fs.create_dir(path)?
    let entries: Array<string> = fs.read_dir(path)?
    fs.remove_dir(path)?
    return Ok(entries)
}

fn main() -> void {
    let entries: Result<Array<string>, FsError> = prepare("/tmp/nomo-dir")
    io.println("done")
}
"#;

        let program = parse_inline(source).unwrap();
        assert!(program.structs.iter().any(|item| item.name == "FsError"));
        assert!(
            program
                .structs
                .iter()
                .any(|item| item.name == "FileMetadata")
        );
        assert!(program.enums.iter().any(|item| item.name == "Result"));
        let prepare = program
            .functions
            .iter()
            .find(|f| f.name == "prepare")
            .unwrap();
        assert_eq!(
            prepare.return_type,
            ValueType::Enum(
                "Result".to_string(),
                vec![
                    ValueType::Array(Box::new(ValueType::String)),
                    ValueType::Struct("FsError".to_string(), Vec::new()),
                ],
            )
        );
        assert!(matches!(
            prepare.body[1],
            Statement::QuestionLet {
                result_expr: ValueExpr::FsMetadata { .. },
                ..
            }
        ));
        assert!(matches!(
            prepare.body[0],
            Statement::Let {
                initializer: ValueExpr::FsExists { .. },
                ..
            }
        ));
    }

    #[test]
    fn accepts_specific_fs_directory_imports() {
        let source = r#"package app.main

import std.fs.exists
import std.fs.metadata
import std.fs.create_dir
import std.fs.remove_dir
import std.fs.read_dir
import std.array

fn prepare(path: string) -> Result<Array<string>, FsError> {
    let present: bool = exists(path)
    let metadata: FileMetadata = metadata(path)?
    create_dir(path)?
    let entries: Array<string> = read_dir(path)?
    remove_dir(path)?
    return Ok(entries)
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let prepare = program
            .functions
            .iter()
            .find(|f| f.name == "prepare")
            .unwrap();
        assert!(matches!(
            prepare.body[0],
            Statement::Let {
                initializer: ValueExpr::FsExists { .. },
                ..
            }
        ));
        assert!(matches!(
            prepare.body[1],
            Statement::QuestionLet {
                result_expr: ValueExpr::FsMetadata { .. },
                ..
            }
        ));
    }

    #[test]
    fn accepts_env_get_builtin() {
        let source = r#"package app.main

import std.env
import std.io

fn main() -> void {
    let value: Option<string> = env.get("NOMO_TEST_ENV")
    let message: string = match value {
        Option.Some(text) => text
        Option.None => "missing"
    }
    io.println(message)
}
"#;

        let program = parse_inline(source).unwrap();
        assert!(program.enums.iter().any(|item| item.name == "Option"));
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                value_type: ValueType::Enum(ref name, ref args),
                initializer: ValueExpr::EnvGet { .. },
                ..
            } if name == "Option" && args == &vec![ValueType::String]
        ));
    }

    #[test]
    fn accepts_env_args_builtin() {
        let source = r#"package app.main

import std.env
import std.io
import std.array

fn main() -> void {
    let args: Array<string> = env.args()
    let first: Option<string> = args.get(1)
    let message: string = match first {
        Option.Some(text) => text
        Option.None => "missing"
    }
    io.println(message)
}
"#;

        let program = parse_inline(source).unwrap();
        assert!(program.enums.iter().any(|item| item.name == "Option"));
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                value_type: ValueType::Array(ref element),
                initializer: ValueExpr::EnvArgs,
                ..
            } if element.as_ref() == &ValueType::String
        ));
        assert!(matches!(
            main.body[1],
            Statement::Let {
                value_type: ValueType::Enum(ref name, ref args),
                initializer: ValueExpr::ArrayGet {
                    element_type: ValueType::String,
                    ..
                },
                ..
            } if name == "Option" && args == &vec![ValueType::String]
        ));
    }

    #[test]
    fn accepts_extended_env_builtins() {
        let source = r#"package app.main

import std.env
import std.io

fn main() -> void {
    env.set("NOMO_TEST_ENV", "value")
    let cwd: string = env.cwd()
    let home: Option<string> = env.home_dir()
    let temp: string = env.temp_dir()
    io.println(cwd)
}
"#;

        let program = parse_inline(source).unwrap();
        assert!(program.enums.iter().any(|item| item.name == "Option"));
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Expr(ValueExpr::EnvSet { .. })
        ));
        assert!(matches!(
            main.body[1],
            Statement::Let {
                value_type: ValueType::String,
                initializer: ValueExpr::EnvCwd,
                ..
            }
        ));
        assert!(matches!(
            main.body[2],
            Statement::Let {
                value_type: ValueType::Enum(ref name, ref args),
                initializer: ValueExpr::EnvHomeDir,
                ..
            } if name == "Option" && args == &vec![ValueType::String]
        ));
        assert!(matches!(
            main.body[3],
            Statement::Let {
                value_type: ValueType::String,
                initializer: ValueExpr::EnvTempDir,
                ..
            }
        ));
    }

    #[test]
    fn accepts_specific_env_builtin_imports() {
        let source = r#"package app.main

import std.env.args
import std.env.cwd
import std.env.get
import std.env.home_dir
import std.env.set
import std.env.temp_dir
import std.io
import std.array

fn main() -> void {
    set("NOMO_TEST_ENV", "value")
    let values: Array<string> = args()
    let home: Option<string> = get("HOME")
    let cwd_path: string = cwd()
    let maybe_home: Option<string> = home_dir()
    let temp_path: string = temp_dir()
    let message: string = match home {
        Option.Some(text) => text
        Option.None => "missing"
    }
    io.println(message)
}
"#;

        let program = parse_inline(source).unwrap();
        assert!(program.enums.iter().any(|item| item.name == "Option"));
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Expr(ValueExpr::EnvSet { .. })
        ));
        assert!(matches!(
            main.body[1],
            Statement::Let {
                value_type: ValueType::Array(ref element),
                initializer: ValueExpr::EnvArgs,
                ..
            } if element.as_ref() == &ValueType::String
        ));
        assert!(matches!(
            main.body[2],
            Statement::Let {
                value_type: ValueType::Enum(ref name, ref args),
                initializer: ValueExpr::EnvGet { .. },
                ..
            } if name == "Option" && args == &vec![ValueType::String]
        ));
        assert!(matches!(
            main.body[3],
            Statement::Let {
                value_type: ValueType::String,
                initializer: ValueExpr::EnvCwd,
                ..
            }
        ));
        assert!(matches!(
            main.body[4],
            Statement::Let {
                value_type: ValueType::Enum(ref name, ref args),
                initializer: ValueExpr::EnvHomeDir,
                ..
            } if name == "Option" && args == &vec![ValueType::String]
        ));
        assert!(matches!(
            main.body[5],
            Statement::Let {
                value_type: ValueType::String,
                initializer: ValueExpr::EnvTempDir,
                ..
            }
        ));
    }

    #[test]
    fn accepts_imported_result_lang_item() {
        let source = r#"package app.main

import std.result

fn parse() -> Result<i64, string> {
    return Result.Ok(41)
}

fn main() -> void {
    let value: Result<i64, string> = parse()
}
"#;

        let program = parse_inline(source).unwrap();
        assert!(program.enums.iter().any(|item| item.name == "Result"));
        let parse = program
            .functions
            .iter()
            .find(|f| f.name == "parse")
            .unwrap();
        assert_eq!(
            parse.return_type,
            ValueType::Enum(
                "Result".to_string(),
                vec![ValueType::Int, ValueType::String],
            )
        );
        assert!(matches!(
            parse.body[0],
            Statement::Return(Some(ValueExpr::EnumVariant {
                ref enum_name,
                ref variant,
                ..
            })) if enum_name == "Result" && variant == "Ok"
        ));
    }

    #[test]
    fn accepts_imported_option_lang_item() {
        let source = r#"package app.main

import std.option
import std.io

fn label(value: Option<string>) -> string {
    return match value {
        Option.Some(text) => text
        Option.None => "missing"
    }
}

fn main() -> void {
    let value: Option<string> = Option.None
    let text: string = label(value)
    io.println(text)
}
"#;

        let program = parse_inline(source).unwrap();
        assert!(program.enums.iter().any(|item| item.name == "Option"));
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                value_type: ValueType::Enum(ref name, ref args),
                initializer: ValueExpr::EnumVariant {
                    ref enum_name,
                    ref variant,
                    ..
                },
                ..
            } if name == "Option"
                && args == &vec![ValueType::String]
                && enum_name == "Option"
                && variant == "None"
        ));
    }

    #[test]
    fn accepts_string_array_builtins() {
        let source = r#"package app.main

import std.array
import std.io

fn main() -> void {
    let mut items: Array<string> = Array.new<string>()
    items.push("first")
    items.push("second")
    items.set(0, "updated")
    let size: u64 = items.len()
    let first: Option<string> = items.get(0)
    let message: string = match first {
        Option.Some(text) => text
        Option.None => "missing"
    }
    io.println(message)
}
"#;

        let program = parse_inline(source).unwrap();
        assert!(program.enums.iter().any(|item| item.name == "Option"));
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                value_type: ValueType::Array(ref element),
                initializer: ValueExpr::ArrayNew {
                    element_type: ValueType::String,
                },
                ..
            } if element.as_ref() == &ValueType::String
        ));
        assert!(matches!(
            main.body[1],
            Statement::Assign {
                ref name,
                value: ValueExpr::ArrayPush { .. },
            } if name == "items"
        ));
        assert!(matches!(
            main.body[3],
            Statement::Assign {
                ref name,
                value: ValueExpr::ArraySet { .. },
            } if name == "items"
        ));
        assert!(matches!(
            main.body[4],
            Statement::Let {
                value_type: ValueType::U64,
                initializer: ValueExpr::ArrayLen { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[5],
            Statement::Let {
                value_type: ValueType::Enum(ref name, ref args),
                initializer: ValueExpr::ArrayGet { .. },
                ..
            } if name == "Option" && args == &vec![ValueType::String]
        ));
    }

    #[test]
    fn accepts_i32_array_builtins() {
        let source = r#"package app.main

import std.array
import std.io

fn main() -> void {
    let mut items: Array<i32> = Array.new<i32>()
    items.push(1)
    items.push(2)
    items.set(0, 7)
    let first: Option<i32> = items.get(0)
    let message: string = match first {
        Option.Some(value) => if value == 7 {
            "array ok"
        } else {
            "wrong"
        }
        Option.None => "missing"
    }
    io.println(message)
}
"#;

        let program = parse_inline(source).unwrap();
        assert!(program.enums.iter().any(|item| item.name == "Option"));
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                value_type: ValueType::Array(ref element),
                initializer: ValueExpr::ArrayNew {
                    element_type: ValueType::I32,
                },
                ..
            } if element.as_ref() == &ValueType::I32
        ));
        assert!(matches!(
            main.body[1],
            Statement::Assign {
                ref name,
                value: ValueExpr::ArrayPush {
                    element_type: ValueType::I32,
                    ..
                },
            } if name == "items"
        ));
        assert!(matches!(
            main.body[4],
            Statement::Let {
                value_type: ValueType::Enum(ref name, ref args),
                initializer: ValueExpr::ArrayGet {
                    element_type: ValueType::I32,
                    ..
                },
                ..
            } if name == "Option" && args == &vec![ValueType::I32]
        ));
    }

    #[test]
    fn accepts_extended_array_methods() {
        let source = r#"package app.main

import std.array

fn main() -> void {
    let mut items: Array<i32> = Array.new<i32>()
    items.push(1)
    items.insert(1, 2)
    let removed: Option<i32> = items.remove(0)
    let popped: Option<i32> = items.pop()
    items.clear()
    let size: u64 = items.len()
}
"#;

        let program = parse_inline(source).unwrap();
        assert!(program.enums.iter().any(|item| item.name == "Option"));
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[2],
            Statement::Assign {
                ref name,
                value: ValueExpr::ArrayInsert {
                    element_type: ValueType::I32,
                    ..
                },
            } if name == "items"
        ));
        assert!(matches!(
            main.body[3],
            Statement::Let {
                value_type: ValueType::Enum(ref name, ref args),
                initializer: ValueExpr::ArrayRemove {
                    element_type: ValueType::I32,
                    ..
                },
                ..
            } if name == "Option" && args == &vec![ValueType::I32]
        ));
        assert!(matches!(
            main.body[4],
            Statement::Let {
                value_type: ValueType::Enum(ref name, ref args),
                initializer: ValueExpr::ArrayPop {
                    element_type: ValueType::I32,
                    ..
                },
                ..
            } if name == "Option" && args == &vec![ValueType::I32]
        ));
        assert!(matches!(
            main.body[5],
            Statement::Assign {
                ref name,
                value: ValueExpr::ArrayClear {
                    element_type: ValueType::I32,
                    ..
                },
            } if name == "items"
        ));
        assert!(matches!(
            main.body[6],
            Statement::Let {
                value_type: ValueType::U64,
                initializer: ValueExpr::ArrayLen { .. },
                ..
            }
        ));
    }

    #[test]
    fn accepts_array_iter_method() {
        let source = r#"package app.main

import std.array
import std.io

fn main() -> void {
    let mut items: Array<i32> = Array.new<i32>()
    items.push(1)
    let snapshot: Array<i32> = items.iter()
    for item in items.iter() {
        io.println("item")
    }
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[2],
            Statement::Let {
                value_type: ValueType::Array(ref element),
                initializer: ValueExpr::ArrayIter {
                    element_type: ValueType::I32,
                    ..
                },
                ..
            } if element.as_ref() == &ValueType::I32
        ));
        assert!(matches!(
            main.body[3],
            Statement::Loop {
                kind: LoopKind::Iterate {
                    element_type: ValueType::I32,
                    iterable: ValueExpr::ArrayIter {
                        element_type: ValueType::I32,
                        ..
                    },
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn rejects_mutating_array_method_on_immutable_variable() {
        let source = r#"package app.main

import std.array

fn main() -> void {
    let items: Array<i32> = Array.new<i32>()
    items.push(1)
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0501");
        assert!(err.message.contains("immutable variable"));
    }

    #[test]
    fn accepts_struct_array_builtins() {
        let source = r#"package app.main

import std.array
import std.io

struct Point {
    x: i32
    y: i32
}

fn main() -> void {
    let mut points: Array<Point> = Array.new<Point>()
    points.push(Point { x: 3, y: 4 })
    let first: Option<Point> = points.get(0)
    let message: string = match first {
        Option.Some(point) => if point.x == 3 {
            "struct array ok"
        } else {
            "wrong"
        }
        Option.None => "missing"
    }
    io.println(message)
}
"#;

        let program = parse_inline(source).unwrap();
        let point_type = ValueType::Struct("Point".to_string(), Vec::new());
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                value_type: ValueType::Array(ref element),
                initializer: ValueExpr::ArrayNew {
                    element_type: ValueType::Struct(ref name, ref args),
                },
                ..
            } if element.as_ref() == &point_type && name == "Point" && args.is_empty()
        ));
        assert!(matches!(
            main.body[1],
            Statement::Assign {
                ref name,
                value: ValueExpr::ArrayPush {
                    element_type: ValueType::Struct(ref struct_name, ref args),
                    ..
                },
            } if name == "points" && struct_name == "Point" && args.is_empty()
        ));
        assert!(matches!(
            main.body[2],
            Statement::Let {
                value_type: ValueType::Enum(ref name, ref args),
                initializer: ValueExpr::ArrayGet {
                    element_type: ValueType::Struct(ref struct_name, ref struct_args),
                    ..
                },
                ..
            } if name == "Option"
                && args == &vec![point_type]
                && struct_name == "Point"
                && struct_args.is_empty()
        ));
    }

    #[test]
    fn accepts_enum_array_builtins() {
        let source = r#"package app.main

import std.array
import std.io
import std.option

fn main() -> void {
    let mut values: Array<Option<i32>> = Array.new<Option<i32>>()
    values.push(Option.Some(7))
    values.push(Option.None)
    let first: Option<Option<i32>> = values.get(0)
    let message: string = match first {
        Option.Some(value) => match value {
            Option.Some(number) => if number == 7 {
                "enum array ok"
            } else {
                "wrong"
            }
            Option.None => "inner missing"
        }
        Option.None => "outer missing"
    }
    io.println(message)
}
"#;

        let program = parse_inline(source).unwrap();
        let option_i32 = ValueType::Enum("Option".to_string(), vec![ValueType::I32]);
        let option_option_i32 = ValueType::Enum("Option".to_string(), vec![option_i32.clone()]);
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                value_type: ValueType::Array(ref element),
                initializer: ValueExpr::ArrayNew {
                    element_type: ValueType::Enum(ref name, ref args),
                },
                ..
            } if element.as_ref() == &option_i32 && name == "Option" && args == &vec![ValueType::I32]
        ));
        assert!(matches!(
            main.body[1],
            Statement::Assign {
                ref name,
                value: ValueExpr::ArrayPush {
                    element_type: ValueType::Enum(ref enum_name, ref enum_args),
                    ..
                },
            } if name == "values" && enum_name == "Option" && enum_args == &vec![ValueType::I32]
        ));
        assert!(matches!(
            main.body[3],
            Statement::Let {
                ref value_type,
                initializer: ValueExpr::ArrayGet {
                    element_type: ValueType::Enum(ref enum_name, ref enum_args),
                    ..
                },
                ..
            } if value_type == &option_option_i32
                && enum_name == "Option"
                && enum_args == &vec![ValueType::I32]
        ));
    }

    #[test]
    fn accepts_arrays_for_all_v0_1_primitive_elements() {
        let source = r#"package app.main

import std.array
import std.io

fn main() -> void {
    let mut strings: Array<string> = Array.new<string>()
    strings.push("nomo")
    let mut ints: Array<i64> = Array.new<i64>()
    ints.push(1)
    let mut i32s: Array<i32> = Array.new<i32>()
    i32s.push(2)
    let mut u32s: Array<u32> = Array.new<u32>()
    u32s.push(3 as u32)
    let mut u64s: Array<u64> = Array.new<u64>()
    u64s.push(4 as u64)
    let mut floats: Array<f64> = Array.new<f64>()
    floats.push(1.5)
    let mut chars: Array<char> = Array.new<char>()
    chars.push('n')
    let mut bools: Array<bool> = Array.new<bool>()
    bools.push(true)
    io.println("arrays ok")
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        let array_elements = main
            .body
            .iter()
            .filter_map(|statement| match statement {
                Statement::Let {
                    value_type: ValueType::Array(element),
                    initializer: ValueExpr::ArrayNew { element_type },
                    ..
                } if element.as_ref() == element_type => Some(element_type.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            array_elements,
            vec![
                ValueType::String,
                ValueType::Int,
                ValueType::I32,
                ValueType::U32,
                ValueType::U64,
                ValueType::Float,
                ValueType::Char,
                ValueType::Bool,
            ]
        );
    }

    #[test]
    fn rejects_array_void_in_type_positions_before_codegen() {
        for source in [
            r#"package app.main

import std.array

fn main() -> void {
    let values: Array<void> = Array.new<void>()
}
"#,
            r#"package app.main

import std.array

fn bad(values: Array<void>) -> void {
}

fn main() -> void {
}
"#,
            r#"package app.main

import std.array

fn bad() -> Array<void> {
    return Array.new<void>()
}

fn main() -> void {
}
"#,
            r#"package app.main

import std.array

struct Bad {
    values: Array<void>
}

fn main() -> void {
}
"#,
            r#"package app.main

import std.array

enum Bad {
    Values(Array<void>)
}

fn main() -> void {
}
"#,
        ] {
            let err = parse_inline(source).unwrap_err();
            assert!(err.code == "E0403" || err.code == "E0404");
            assert!(err.message.contains("Array elements"));
        }
    }

    #[test]
    fn accepts_generic_array_type_positions_before_instantiation() {
        let source = r#"package app.main

import std.array

struct Bag<T> {
    values: Array<T>
}

fn id<T>(values: Array<T>) -> Array<T> {
    return values
}

fn main() -> void {
    let values: Array<i32> = Array.new<i32>()
    let copy: Array<i32> = id<i32>(values)
}
"#;

        let program = parse_inline(source).unwrap();
        assert_eq!(program.structs[0].type_params, ["T"]);
        let id = program
            .functions
            .iter()
            .find(|f| f.name == "id_i32")
            .unwrap();
        assert_eq!(id.return_type, ValueType::Array(Box::new(ValueType::I32)));
    }

    #[test]
    fn accepts_specific_array_new_import() {
        let source = r#"package app.main

import std.array.new
import std.array.Array
import std.io

fn main() -> void {
    let mut items: Array<i32> = new<i32>()
    items.push(7)
    let first: Option<i32> = items.get(0)
    let message: string = match first {
        Option.Some(value) => if value == 7 {
            "array new import ok"
        } else {
            "wrong"
        }
        Option.None => "missing"
    }
    io.println(message)
}
"#;

        let program = parse_inline(source).unwrap();
        assert!(program.enums.iter().any(|item| item.name == "Option"));
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                value_type: ValueType::Array(ref element),
                initializer: ValueExpr::ArrayNew {
                    element_type: ValueType::I32,
                },
                ..
            } if element.as_ref() == &ValueType::I32
        ));
    }

    #[test]
    fn accepts_specific_array_method_imports() {
        let source = r#"package app.main

import std.env.args
import std.array.Array
import std.array.get
import std.array.clear
import std.array.insert
import std.array.iter
import std.array.len
import std.array.pop
import std.array.push
import std.array.remove
import std.array.set

fn main() -> void {
    let mut values = args()
    values.push("extra")
    values.insert(1, "middle")
    values.set(0, "program")
    let removed: Option<string> = values.remove(1)
    let popped: Option<string> = values.pop()
    values.clear()
    let snapshot: Array<string> = values.iter()
    let size: u64 = values.len()
    let first: Option<string> = values.get(0)
}
"#;

        let program = parse_inline(source).unwrap();
        assert!(program.enums.iter().any(|item| item.name == "Option"));
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                value_type: ValueType::Array(ref element),
                initializer: ValueExpr::EnvArgs,
                ..
            } if element.as_ref() == &ValueType::String
        ));
        assert!(matches!(
            main.body[1],
            Statement::Assign {
                ref name,
                value: ValueExpr::ArrayPush { .. },
            } if name == "values"
        ));
        assert!(matches!(
            main.body[2],
            Statement::Assign {
                ref name,
                value: ValueExpr::ArrayInsert { .. },
            } if name == "values"
        ));
        assert!(matches!(
            main.body[3],
            Statement::Assign {
                ref name,
                value: ValueExpr::ArraySet { .. },
            } if name == "values"
        ));
        assert!(matches!(
            main.body[4],
            Statement::Let {
                value_type: ValueType::Enum(ref name, ref args),
                initializer: ValueExpr::ArrayRemove {
                    element_type: ValueType::String,
                    ..
                },
                ..
            } if name == "Option" && args == &vec![ValueType::String]
        ));
        assert!(matches!(
            main.body[5],
            Statement::Let {
                value_type: ValueType::Enum(ref name, ref args),
                initializer: ValueExpr::ArrayPop {
                    element_type: ValueType::String,
                    ..
                },
                ..
            } if name == "Option" && args == &vec![ValueType::String]
        ));
        assert!(matches!(
            main.body[6],
            Statement::Assign {
                ref name,
                value: ValueExpr::ArrayClear { .. },
            } if name == "values"
        ));
        assert!(matches!(
            main.body[7],
            Statement::Let {
                value_type: ValueType::Array(ref element),
                initializer: ValueExpr::ArrayIter {
                    element_type: ValueType::String,
                    ..
                },
                ..
            } if element.as_ref() == &ValueType::String
        ));
        assert!(matches!(
            main.body[8],
            Statement::Let {
                value_type: ValueType::U64,
                initializer: ValueExpr::ArrayLen { .. },
                ..
            }
        ));
        assert!(matches!(
            main.body[9],
            Statement::Let {
                value_type: ValueType::Enum(ref name, ref args),
                initializer: ValueExpr::ArrayGet {
                    element_type: ValueType::String,
                    ..
                },
                ..
            } if name == "Option" && args == &vec![ValueType::String]
        ));
    }

    #[test]
    fn rejects_unqualified_array_new_without_specific_import() {
        let source = r#"package app.main

import std.array
import std.io

fn main() -> void {
    let mut items: Array<i32> = new<i32>()
    io.println("done")
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0305");
        assert!(err.message.contains("new"));
    }

    #[test]
    fn rejects_array_method_without_array_import() {
        let source = r#"package app.main

import std.array.new

fn main() -> void {
    let mut items: Array<i32> = new<i32>()
    items.push(1)
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0301");
        assert!(err.message.contains("std.array"));
    }

    #[test]
    fn rejects_string_len_as_i64() {
        let source = r#"package app.main

import std.string

fn main() -> void {
    let count: i64 = string.len("hello")
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0404");
    }

    #[test]
    fn accepts_basic_equality_for_string_char_and_bool() {
        let source = r#"package app.main

fn main() -> void {
    let same: bool = "nomo" == "nomo"
    let different: bool = "nomo" != "rust"
    let same_char: bool = '語' == '語'
    let same_bool: bool = true == true
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program
            .functions
            .iter()
            .find(|function| function.name == "main")
            .unwrap();
        assert!(matches!(
            main.body.as_slice(),
            [
                Statement::Let {
                    initializer: ValueExpr::StringCompare {
                        op: BinaryOp::Equal,
                        ..
                    },
                    ..
                },
                Statement::Let {
                    initializer: ValueExpr::StringCompare {
                        op: BinaryOp::NotEqual,
                        ..
                    },
                    ..
                },
                Statement::Let {
                    initializer: ValueExpr::Binary {
                        op: BinaryOp::Equal,
                        ..
                    },
                    ..
                },
                Statement::Let {
                    initializer: ValueExpr::Binary {
                        op: BinaryOp::Equal,
                        ..
                    },
                    ..
                },
            ]
        ));
    }

    #[test]
    fn rejects_ordering_comparison_for_strings() {
        let source = r#"package app.main

fn main() -> void {
    let ordered: bool = "a" < "b"
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0404");
        assert!(err.message.contains("comparable operands"));
    }

    #[test]
    fn accepts_function_call_and_integer_return() {
        let source = r#"package app.main

import std.io

fn add(a: i64, b: i64) -> i64 {
    return a + b
}

fn main() -> void {
    let answer: i64 = add(40, 2)
    io.println("done")
}
"#;

        let program = parse_inline(source).unwrap();
        let add = program.functions.iter().find(|f| f.name == "add").unwrap();
        assert_eq!(add.params.len(), 2);
        assert_eq!(add.return_type, ValueType::Int);
        assert!(matches!(
            add.body[0],
            Statement::Return(Some(ValueExpr::Binary {
                op: BinaryOp::Add,
                ..
            }))
        ));
    }

    #[test]
    fn accepts_binary_arithmetic_operators() {
        let source = r#"package app.main

fn calc(a: i64, b: i64, c: i64, d: i64, e: i64) -> i64 {
    return a - b * c / d % e
}

fn ratio(total: f64, count: f64) -> f64 {
    return total / count
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let calc = program.functions.iter().find(|f| f.name == "calc").unwrap();
        let ratio = program
            .functions
            .iter()
            .find(|f| f.name == "ratio")
            .unwrap();

        assert!(matches!(
            calc.body[0],
            Statement::Return(Some(ValueExpr::Binary {
                op: BinaryOp::Subtract,
                ..
            }))
        ));
        assert!(matches!(
            ratio.body[0],
            Statement::Return(Some(ValueExpr::Binary {
                op: BinaryOp::Divide,
                ..
            }))
        ));
        assert_eq!(calc.return_type, ValueType::Int);
        assert_eq!(ratio.return_type, ValueType::Float);
    }

    #[test]
    fn rejects_float_remainder() {
        let source = r#"package app.main

fn bad(left: f64, right: f64) -> f64 {
    return left % right
}

fn main() -> void {
}
"#;

        let err = parse_inline(source).unwrap_err();

        assert_eq!(err.code, "E0404");
        assert!(err.message.contains("numeric operands"));
    }

    #[test]
    fn accepts_logical_operators() {
        let source = r#"package app.main

fn check(a: bool, b: bool, c: bool) -> bool {
    return !a && b || c
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let check = program
            .functions
            .iter()
            .find(|f| f.name == "check")
            .unwrap();

        assert_eq!(check.return_type, ValueType::Bool);
        assert!(matches!(
            check.body[0],
            Statement::Return(Some(ValueExpr::Binary {
                op: BinaryOp::LogicalOr,
                ..
            }))
        ));
    }

    #[test]
    fn rejects_non_bool_logical_operands() {
        let source = r#"package app.main

fn bad(value: i64) -> bool {
    return value && true
}

fn main() -> void {
}
"#;

        let err = parse_inline(source).unwrap_err();

        assert_eq!(err.code, "E0404");
        assert!(err.message.contains("bool operands"));
    }

    #[test]
    fn rejects_non_bool_not_operand() {
        let source = r#"package app.main

fn bad(value: i64) -> bool {
    return !value
}

fn main() -> void {
}
"#;

        let err = parse_inline(source).unwrap_err();

        assert_eq!(err.code, "E0404");
        assert!(err.message.contains("bool operand"));
    }

    #[test]
    fn accepts_bitwise_operators() {
        let source = r#"package app.main

fn mask(a: i64, b: i64, c: i64, shift: u32) -> i64 {
    return a & b | c ^ a &^ b << shift >> 1
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let mask = program.functions.iter().find(|f| f.name == "mask").unwrap();

        assert_eq!(mask.return_type, ValueType::Int);
        assert!(matches!(
            mask.body[0],
            Statement::Return(Some(ValueExpr::Binary {
                op: BinaryOp::BitXor,
                ..
            }))
        ));
    }

    #[test]
    fn rejects_non_integer_bitwise_operands() {
        let source = r#"package app.main

fn bad(left: bool, right: bool) -> bool {
    return left & right
}

fn main() -> void {
}
"#;

        let err = parse_inline(source).unwrap_err();

        assert_eq!(err.code, "E0404");
        assert!(err.message.contains("integer operands"));
    }

    #[test]
    fn rejects_non_integer_shift_rhs() {
        let source = r#"package app.main

fn bad(left: i64, right: bool) -> i64 {
    return left << right
}

fn main() -> void {
}
"#;

        let err = parse_inline(source).unwrap_err();

        assert_eq!(err.code, "E0404");
        assert!(err.message.contains("integer operands"));
    }

    #[test]
    fn accepts_generic_function_instances() {
        let source = r#"package app.main

import std.io

fn identity<T>(value: T) -> T {
    return value
}

fn main() -> void {
    let number: i32 = identity<i32>(7)
    let text: string = identity<string>("generic")
    io.println(text)
}
"#;

        let program = parse_inline(source).unwrap();
        assert!(
            program
                .functions
                .iter()
                .all(|function| function.name != "identity")
        );
        let identity_i32 = program
            .functions
            .iter()
            .find(|function| function.name == "identity_i32")
            .unwrap();
        assert_eq!(identity_i32.params[0].value_type, ValueType::I32);
        assert_eq!(identity_i32.return_type, ValueType::I32);
        let identity_string = program
            .functions
            .iter()
            .find(|function| function.name == "identity_string")
            .unwrap();
        assert_eq!(identity_string.params[0].value_type, ValueType::String);
        assert_eq!(identity_string.return_type, ValueType::String);
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                initializer: ValueExpr::Call { ref name, .. },
                ..
            } if name == "identity_i32"
        ));
        assert!(matches!(
            main.body[1],
            Statement::Let {
                initializer: ValueExpr::Call { ref name, .. },
                ..
            } if name == "identity_string"
        ));
    }

    #[test]
    fn rejects_generic_function_call_without_type_arguments() {
        let source = r#"package app.main

import std.io

fn identity<T>(value: T) -> T {
    return value
}

fn main() -> void {
    let number: i32 = identity(7)
    io.println("done")
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0407");
    }

    #[test]
    fn accepts_mut_call_argument_for_mut_parameter() {
        let source = r#"package app.main

import std.io

fn inspect(mut value: i64) -> i64 {
    return value
}

fn main() -> void {
    let mut count: i64 = 41
    let answer: i64 = inspect(mut count) + 1
    io.println("done")
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            &main.body[1],
            Statement::Let {
                initializer: ValueExpr::Binary {
                    left,
                    ..
                },
                ..
            } if matches!(
                left.as_ref(),
                ValueExpr::Call {
                    name,
                    args,
                } if name == "inspect"
                    && args == &vec![ValueExpr::MutBorrow(vec!["count".to_string()])]
            )
        ));
    }

    #[test]
    fn accepts_mut_field_path_call_argument_for_mut_parameter() {
        let source = r#"package app.main

struct Point {
    x: i32
    y: i32
}

fn bump(mut value: i32) -> void {
    value = value + 1
}

fn main() -> void {
    let mut point: Point = Point { x: 1, y: 2 }
    bump(mut point.x)
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            &main.body[1],
            Statement::Expr(ValueExpr::Call { name, args })
                if name == "bump"
                    && args == &vec![ValueExpr::MutBorrow(vec![
                        "point".to_string(),
                        "x".to_string()
                    ])]
        ));
    }

    #[test]
    fn accepts_forwarding_mut_parameter_as_mut_argument() {
        let source = r#"package app.main

fn bump(mut value: i32) -> void {
    value = value + 1
}

fn bump_twice(mut value: i32) -> void {
    bump(mut value)
    bump(mut value)
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let bump_twice = program
            .functions
            .iter()
            .find(|function| function.name == "bump_twice")
            .unwrap();
        assert!(matches!(
            bump_twice.body.as_slice(),
            [
                Statement::Expr(ValueExpr::Call {
                    name: first_name,
                    args: first_args,
                }),
                Statement::Expr(ValueExpr::Call {
                    name: second_name,
                    args: second_args,
                }),
            ] if first_name == "bump"
                && second_name == "bump"
                && first_args == &vec![ValueExpr::MutBorrow(vec!["value".to_string()])]
                && second_args == &vec![ValueExpr::MutBorrow(vec!["value".to_string()])]
        ));
    }

    #[test]
    fn rejects_missing_mut_call_argument_for_mut_parameter() {
        let source = r#"package app.main

import std.io

fn inspect(mut value: i64) -> i64 {
    return value
}

fn main() -> void {
    let mut count: i64 = 41
    let answer: i64 = inspect(count)
    io.println("done")
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0500");
    }

    #[test]
    fn rejects_immutable_variable_as_mut_call_argument() {
        let source = r#"package app.main

import std.io

fn inspect(mut value: i64) -> i64 {
    return value
}

fn main() -> void {
    let count: i64 = 41
    let answer: i64 = inspect(mut count)
    io.println("done")
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0501");
    }

    #[test]
    fn rejects_duplicate_mut_call_argument() {
        let source = r#"package app.main

import std.io

fn combine(mut left: i64, mut right: i64) -> i64 {
    return left + right
}

fn main() -> void {
    let mut count: i64 = 41
    let answer: i64 = combine(mut count, mut count)
    io.println("done")
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0502");
    }

    #[test]
    fn rejects_prefix_conflicting_mut_field_borrow_in_same_call() {
        let source = r#"package app.main

struct Point {
    x: i32
    y: i32
}

fn overwrite(mut point: Point, mut value: i32) -> void {
}

fn main() -> void {
    let mut point: Point = Point { x: 1, y: 2 }
    overwrite(mut point, mut point.x)
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0502");
        assert!(err.message.contains("point.x"));
        assert!(err.message.contains("point"));
    }

    #[test]
    fn accepts_non_overlapping_mut_field_borrows_in_same_call() {
        let source = r#"package app.main

struct Point {
    x: i32
    y: i32
}

fn swap_values(mut left: i32, mut right: i32) -> void {
    let temp: i32 = left
    left = right
    right = temp
}

fn main() -> void {
    let mut point: Point = Point { x: 1, y: 2 }
    swap_values(mut point.x, mut point.y)
}
"#;

        parse_inline(source).unwrap();
    }

    #[test]
    fn accepts_f64_literal_cast_addition_and_comparison() {
        let source = r#"package app.main

import std.io

fn ratio(age: i64) -> f64 {
    return age as f64
}

fn add(a: f64, b: f64) -> f64 {
    return a + b
}

fn check(value: f64) -> bool {
    return value >= 1.5
}

fn main() -> void {
    let pi: f64 = 3.14
    let value: f64 = ratio(42)
    let total: f64 = add(pi, value)
    let ok: bool = check(total)
    io.println("done")
}
"#;

        let program = parse_inline(source).unwrap();
        let ratio = program
            .functions
            .iter()
            .find(|f| f.name == "ratio")
            .unwrap();
        assert_eq!(ratio.return_type, ValueType::Float);
        assert!(matches!(
            ratio.body[0],
            Statement::Return(Some(ValueExpr::Cast {
                target_type: ValueType::Float,
                ..
            }))
        ));
        let add = program.functions.iter().find(|f| f.name == "add").unwrap();
        assert!(matches!(
            add.body[0],
            Statement::Return(Some(ValueExpr::Binary {
                op: BinaryOp::Add,
                ..
            }))
        ));
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                value_type: ValueType::Float,
                initializer: ValueExpr::FloatLiteral(ref value),
                ..
            } if value == "3.14"
        ));
    }

    #[test]
    fn rejects_primitive_type_arguments() {
        for (source, type_name) in [
            (
                r#"package app.main

fn main() -> void {
    let value: i32<string> = 1
}
"#,
                "i32",
            ),
            (
                r#"package app.main

fn main() -> void {
    let value: string<i32> = "x"
}
"#,
                "string",
            ),
            (
                r#"package app.main

fn main() -> void {
    let value: bool<i32> = true
}
"#,
                "bool",
            ),
        ] {
            let err = parse_inline(source).unwrap_err();

            assert_eq!(err.code, "E0403");
            assert!(err.message.contains(type_name));
        }
    }

    #[test]
    fn accepts_distinct_integer_types() {
        let source = r#"package app.main

import std.io

fn add32(a: i32, b: i32) -> i32 {
    return a + b
}

fn check64(value: u64) -> bool {
    return value >= 1
}

fn main() -> void {
    let signed: i32 = 1
    let word: u32 = 2
    let wide: u64 = 3
    let total: i32 = add32(signed, 4)
    let ok: bool = check64(wide)
    io.println("done")
}
"#;

        let program = parse_inline(source).unwrap();
        let add32 = program
            .functions
            .iter()
            .find(|f| f.name == "add32")
            .unwrap();
        assert_eq!(add32.params[0].value_type, ValueType::I32);
        assert_eq!(add32.return_type, ValueType::I32);
        let check64 = program
            .functions
            .iter()
            .find(|f| f.name == "check64")
            .unwrap();
        assert_eq!(check64.params[0].value_type, ValueType::U64);
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                value_type: ValueType::I32,
                initializer: ValueExpr::IntLiteral(1),
                ..
            }
        ));
        assert!(matches!(
            main.body[1],
            Statement::Let {
                value_type: ValueType::U32,
                initializer: ValueExpr::IntLiteral(2),
                ..
            }
        ));
        assert!(matches!(
            main.body[2],
            Statement::Let {
                value_type: ValueType::U64,
                initializer: ValueExpr::IntLiteral(3),
                ..
            }
        ));
    }

    #[test]
    fn rejects_int_alias_in_v0_1() {
        for source in [
            r#"package app.main

fn main() -> void {
    let value: int = 1
}
"#,
            r#"package app.main

fn inspect(value: int) -> void {
}

fn main() -> void {
}
"#,
            r#"package app.main

fn inspect() -> int {
    return 1
}

fn main() -> void {
}
"#,
        ] {
            let err = parse_inline(source).unwrap_err();

            assert_eq!(err.code, "E0403");
            assert!(err.message.contains("`int` is not a v0.1 builtin type"));
            assert!(err.message.contains("i64"));
            assert!(err.message.contains("i32"));
            assert!(err.message.contains("u32"));
            assert!(err.message.contains("u64"));
        }
    }

    #[test]
    fn rejects_i32_literal_overflow() {
        let source = r#"package app.main

fn main() -> void {
    let value: i32 = 2147483648
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0404");
    }

    #[test]
    fn rejects_mixed_integer_binary_without_cast() {
        let source = r#"package app.main

fn main() -> void {
    let left: i32 = 1
    let right: i64 = 2
    let value: i64 = left + right
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0404");
    }

    #[test]
    fn accepts_char_literal_and_return() {
        let source = r#"package app.main

import std.io

fn initial() -> char {
    return 'N'
}

fn main() -> void {
    let letter: char = initial()
    io.println("done")
}
"#;

        let program = parse_inline(source).unwrap();
        let initial = program
            .functions
            .iter()
            .find(|f| f.name == "initial")
            .unwrap();
        assert_eq!(initial.return_type, ValueType::Char);
        assert!(matches!(
            initial.body[0],
            Statement::Return(Some(ValueExpr::CharLiteral('N')))
        ));
    }

    #[test]
    fn rejects_implicit_int_to_f64_initializer() {
        let source = r#"package app.main

fn main() -> void {
    let ratio: f64 = 42
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0404");
    }

    #[test]
    fn rejects_char_string_mismatch() {
        let source = r#"package app.main

fn main() -> void {
    let text: string = 'N'
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0404");
    }

    #[test]
    fn accepts_tail_expression_return() {
        let source = r#"package app.main

import std.io

fn add(a: i64, b: i64) -> i64 {
    a + b
}

fn main() -> void {
    io.println("done")
}
"#;

        let program = parse_inline(source).unwrap();
        let add = program.functions.iter().find(|f| f.name == "add").unwrap();
        assert!(matches!(add.body[0], Statement::Return(Some(_))));
    }

    #[test]
    fn accepts_if_expression_and_integer_comparison() {
        let source = r#"package app.main

import std.io

fn label(score: i64) -> string {
    return if score >= 60 {
        "pass"
    } else {
        "fail"
    }
}

fn main() -> void {
    let text: string = label(75)
    io.println(text)
}
"#;

        let program = parse_inline(source).unwrap();
        let label = program
            .functions
            .iter()
            .find(|f| f.name == "label")
            .unwrap();
        assert!(matches!(
            label.body[0],
            Statement::Return(Some(ValueExpr::If {
                ref condition,
                ref then_branch,
                ref else_branch,
            })) if matches!(
                condition.as_ref(),
                ValueExpr::Binary {
                    op: BinaryOp::GreaterEqual,
                    ..
                }
            ) && then_branch.as_ref() == &ValueExpr::StringLiteral("pass".to_string())
                && else_branch.as_ref() == &ValueExpr::StringLiteral("fail".to_string())
        ));
    }

    #[test]
    fn accepts_panic_statement() {
        let source = r#"package app.main

import std.io

fn main() -> void {
    panic("boom")
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Panic(ValueExpr::StringLiteral(ref message)) if message == "boom"
        ));
    }

    #[test]
    fn accepts_panic_as_diverging_match_arm() {
        let source = r#"package app.main

import std.io

enum Option<T> {
    Some(T)
    None
}

fn unwrap_text(value: Option<string>) -> string {
    return match value {
        Option.Some(text) => text
        Option.None => panic("missing text")
    }
}

fn main() -> void {
    let value: Option<string> = Option.Some("hello")
    let text: string = unwrap_text(value)
    io.println(text)
}
"#;

        let program = parse_inline(source).unwrap();
        let unwrap = program
            .functions
            .iter()
            .find(|function| function.name == "unwrap_text")
            .unwrap();
        assert!(matches!(
            unwrap.body[0],
            Statement::Return(Some(ValueExpr::Match { .. }))
        ));
    }

    #[test]
    fn rejects_if_condition_that_is_not_bool() {
        let source = r#"package app.main

import std.io

fn label(score: i64) -> string {
    return if score {
        "pass"
    } else {
        "fail"
    }
}

fn main() -> void {
    io.println("done")
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0404");
    }

    #[test]
    fn rejects_if_branch_type_mismatch() {
        let source = r#"package app.main

import std.io

fn value(flag: bool) -> i64 {
    return if flag {
        1
    } else {
        "nope"
    }
}

fn main() -> void {
    io.println("done")
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0404");
    }

    #[test]
    fn rejects_unknown_variable() {
        let source = r#"package app.main

import std.io

fn main() -> void {
    io.println(message)
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0303");
    }

    #[test]
    fn rejects_let_type_mismatch() {
        let source = r#"package app.main

import std.io

fn main() -> void {
    let message: string = 42
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0404");
    }

    #[test]
    fn rejects_wrong_function_argument_type() {
        let source = r#"package app.main

import std.io

fn id(value: i64) -> i64 {
    return value
}

fn main() -> void {
    let answer: i64 = id("nope")
    io.println("done")
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0404");
    }

    #[test]
    fn accepts_assignment_to_mut_variable() {
        let source = r#"package app.main

import std.io

fn main() -> void {
    let mut count: i64 = 1
    count = count + 1
    io.println("done")
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[1],
            Statement::Assign {
                ref name,
                value: ValueExpr::Binary { .. },
            } if name == "count"
        ));
    }

    #[test]
    fn accepts_compound_assignment_to_mut_variable() {
        let source = r#"package app.main

fn main() -> void {
    let mut count: i64 = 1
    count += 2
    count -= 1
    count *= 3
    count /= 2
    count %= 2
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        for stmt in &main.body[1..] {
            assert!(matches!(
                stmt,
                Statement::Assign {
                    name,
                    value: ValueExpr::Binary { .. },
                } if name == "count"
            ));
        }
    }

    #[test]
    fn accepts_postfix_update_to_mut_variable() {
        let source = r#"package app.main

fn main() -> void {
    let mut count: i64 = 1
    count++
    count--
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        for stmt in &main.body[1..] {
            assert!(matches!(
                stmt,
                Statement::Assign {
                    name,
                    value: ValueExpr::Binary { .. },
                } if name == "count"
            ));
        }
    }

    #[test]
    fn rejects_assignment_to_immutable_variable() {
        let source = r#"package app.main

import std.io

fn main() -> void {
    let count: i64 = 1
    count = count + 1
    io.println("done")
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0501");
    }

    #[test]
    fn rejects_postfix_update_to_immutable_variable() {
        let source = r#"package app.main

fn main() -> void {
    let count: i64 = 1
    count++
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0501");
    }

    #[test]
    fn rejects_postfix_update_to_non_numeric_variable() {
        let source = r#"package app.main

fn main() -> void {
    let mut message: string = "hi"
    message++
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0404");
    }

    #[test]
    fn accepts_assignment_to_mut_parameter() {
        let source = r#"package app.main

fn bump(mut value: i64) -> i64 {
    value = value + 1
    return value
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let bump = program.functions.iter().find(|f| f.name == "bump").unwrap();

        assert!(matches!(
            bump.body[0],
            Statement::Assign {
                ref name,
                value: ValueExpr::Binary { .. },
            } if name == "value"
        ));
    }

    #[test]
    fn rejects_assignment_to_immutable_parameter() {
        let source = r#"package app.main

fn bump(value: i64) -> i64 {
    value = value + 1
    return value
}

fn main() -> void {
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0501");
        assert!(err.message.contains("value"));
    }

    #[test]
    fn rejects_assignment_to_field_of_immutable_parameter() {
        let source = r#"package app.main

struct Counter {
    value: i64
}

fn bump(counter: Counter) -> void {
    counter.value = counter.value + 1
}

fn main() -> void {
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0501");
        assert!(
            err.message
                .contains("cannot assign to field of immutable parameter `counter`")
        );
    }

    #[test]
    fn rejects_duplicate_local_binding() {
        let source = r#"package app.main

fn main() -> void {
    let count: i64 = 1
    let count: i64 = 2
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0302");
    }

    #[test]
    fn duplicate_function_diagnostic_uses_second_declaration_span() {
        let source = r#"package app.main

fn helper() -> void {
}

fn helper() -> void {
}

fn main() -> void {
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0304");
        assert_eq!(err.line, 6);
        assert_eq!(err.column, 1);
        assert_eq!(err.length, 1);
        assert_eq!(err.text, "fn helper() -> void {");
    }

    #[test]
    fn rejects_parameter_shadowing_by_local_binding() {
        let source = r#"package app.main

fn echo(value: i64) -> i64 {
    let value: i64 = 2
    return value
}

fn main() -> void {
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0302");
    }

    #[test]
    fn accepts_assignment_to_mut_struct_field() {
        let source = r#"package app.main

import std.io

struct Counter {
    value: i64
}

fn main() -> void {
    let mut counter: Counter = Counter { value: 1 }
    counter.value = counter.value + 1
    io.println("done")
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[1],
            Statement::AssignField {
                ref base,
                ref field,
                value_type: ValueType::Int,
                value: ValueExpr::Binary { .. },
                ..
            } if base == "counter" && field == "value"
        ));
    }

    #[test]
    fn accepts_compound_assignment_to_mut_struct_field() {
        let source = r#"package app.main

struct Counter {
    value: i64
}

fn main() -> void {
    let mut counter: Counter = Counter { value: 7 }
    counter.value <<= 1
    counter.value >>= 1
    counter.value &= 6
    counter.value |= 8
    counter.value ^= 3
    counter.value &^= 1
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        for stmt in &main.body[1..] {
            assert!(matches!(
                stmt,
                Statement::AssignField {
                    base,
                    field,
                    value_type: ValueType::Int,
                    value: ValueExpr::Binary { .. },
                } if base == "counter" && field == "value"
            ));
        }
    }

    #[test]
    fn accepts_postfix_update_to_mut_struct_field() {
        let source = r#"package app.main

struct Counter {
    value: i64
}

fn main() -> void {
    let mut counter: Counter = Counter { value: 7 }
    counter.value++
    counter.value--
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        for stmt in &main.body[1..] {
            assert!(matches!(
                stmt,
                Statement::AssignField {
                    base,
                    field,
                    value_type: ValueType::Int,
                    value: ValueExpr::Binary { .. },
                } if base == "counter" && field == "value"
            ));
        }
    }

    #[test]
    fn rejects_assignment_to_immutable_struct_field() {
        let source = r#"package app.main

import std.io

struct Counter {
    value: i64
}

fn main() -> void {
    let counter: Counter = Counter { value: 1 }
    counter.value = counter.value + 1
    io.println("done")
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0501");
    }

    #[test]
    fn rejects_assignment_type_mismatch() {
        let source = r#"package app.main

import std.io

fn main() -> void {
    let mut count: i64 = 1
    count = "nope"
    io.println("done")
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0404");
    }

    #[test]
    fn accepts_struct_literal_and_field_access() {
        let source = r#"package app.main

import std.io

struct Point {
    x: i64
    y: i64
}

fn sum(point: Point) -> i64 {
    return point.x + point.y
}

fn main() -> void {
    let point: Point = Point { x: 40, y: 2 }
    let answer: i64 = sum(point)
    io.println("done")
}
"#;

        let program = parse_inline(source).unwrap();
        assert_eq!(program.structs.len(), 1);
        assert_eq!(program.structs[0].name, "Point");
        let sum = program.functions.iter().find(|f| f.name == "sum").unwrap();
        assert_eq!(
            sum.params[0].value_type,
            ValueType::Struct("Point".to_string(), Vec::new())
        );
        assert!(matches!(
            sum.body[0],
            Statement::Return(Some(ValueExpr::Binary { .. }))
        ));
    }

    #[test]
    fn accepts_generic_struct_literal_and_field_access() {
        let source = r#"package app.main

import std.io

struct Box<T> {
    value: T
}

fn main() -> void {
    let item: Box<i32> = Box { value: 7 }
    let value: i32 = item.value
    io.println("done")
}
"#;

        let program = parse_inline(source).unwrap();
        assert_eq!(program.structs[0].type_params, ["T"]);
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                value_type: ValueType::Struct(ref name, ref args),
                initializer: ValueExpr::StructLiteral {
                    struct_args: ref literal_args,
                    ..
                },
                ..
            } if name == "Box"
                && args == &vec![ValueType::I32]
                && literal_args == &vec![ValueType::I32]
        ));
        assert!(matches!(
            main.body[1],
            Statement::Let {
                value_type: ValueType::I32,
                initializer: ValueExpr::FieldAccess { .. },
                ..
            }
        ));
    }

    #[test]
    fn rejects_direct_recursive_struct_value_field() {
        let source = r#"package app.main

struct Node {
    next: Node
}

fn main() -> void {
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0410");
        assert!(err.message.contains("Node"));
        assert!(err.message.contains("recursively embedded"));
    }

    #[test]
    fn rejects_recursive_struct_through_option_payload() {
        let source = r#"package app.main

import std.option

struct Node {
    next: Option<Node>
}

fn main() -> void {
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0410");
        assert!(err.message.contains("Node"));
        assert!(err.message.contains("recursively embedded"));
    }

    #[test]
    fn accepts_recursive_struct_behind_array_boundary() {
        let source = r#"package app.main

import std.array

struct Node {
    children: Array<Node>
}

fn main() -> void {
    let children: Array<Node> = Array.new<Node>()
    let node: Node = Node { children: children }
}
"#;

        let program = parse_inline(source).unwrap();
        assert_eq!(program.structs[0].name, "Node");
        assert_eq!(
            program.structs[0].fields[0].value_type,
            ValueType::Array(Box::new(ValueType::Struct("Node".to_string(), Vec::new())))
        );
    }

    #[test]
    fn rejects_generic_struct_literal_without_type_annotation() {
        let source = r#"package app.main

import std.io

struct Box<T> {
    value: T
}

fn main() -> void {
    let item = Box { value: 7 }
    io.println("done")
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0317");
    }

    #[test]
    fn accepts_impl_method_call() {
        let source = r#"package app.main

import std.io

struct User {
    email: string
}

impl User {
    pub fn get_email(self) -> string {
        return self.email
    }
}

fn main() -> void {
    let user: User = User { email: "a@nomo.dev" }
    let email: string = user.get_email()
    io.println(email)
}
"#;

        let program = parse_inline(source).unwrap();
        let method = program
            .functions
            .iter()
            .find(|function| function.name == "User_get_email")
            .unwrap();
        assert_eq!(
            method.params[0].value_type,
            ValueType::Struct("User".to_string(), Vec::new())
        );
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[1],
            Statement::Let {
                value_type: ValueType::String,
                initializer: ValueExpr::Call {
                    ref name,
                    ref args,
                },
                ..
            } if name == "User_get_email"
                && args == &vec![ValueExpr::Variable("user".to_string())]
        ));
    }

    #[test]
    fn accepts_mut_impl_method_receiver_call() {
        let source = r#"package app.main

import std.io

struct User {
    email: string
}

impl User {
    pub fn set_email(mut self, email: string) -> void {
        self.email = email
    }
}

fn main() -> void {
    let mut user: User = User { email: "old@nomo.dev" }
    user.set_email("new@nomo.dev")
    io.println(user.email)
}
"#;

        let program = parse_inline(source).unwrap();
        let method = program
            .functions
            .iter()
            .find(|function| function.name == "User_set_email")
            .unwrap();
        assert!(method.params[0].mutable);
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[1],
            Statement::Expr(ValueExpr::Call {
                ref name,
                ref args,
            }) if name == "User_set_email"
                && args == &vec![
                    ValueExpr::MutBorrow(vec!["user".to_string()]),
                    ValueExpr::StringLiteral("new@nomo.dev".to_string())
                ]
        ));
    }

    #[test]
    fn rejects_mut_impl_method_receiver_on_immutable_parameter() {
        let source = r#"package app.main

struct Counter {
    value: i32
}

impl Counter {
    pub fn bump(mut self) -> void {
        self.value = self.value + 1
    }
}

fn touch(counter: Counter) -> void {
    counter.bump()
}

fn main() -> void {
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0501");
        assert!(err.message.contains(
            "cannot call mutating method `Counter.bump` on immutable parameter `counter`"
        ));
    }

    #[test]
    fn rejects_duplicate_mut_borrow_between_receiver_and_argument() {
        let source = r#"package app.main

struct Counter {
    value: i32
}

impl Counter {
    pub fn absorb(mut self, mut other: Counter) -> void {
    }
}

fn main() -> void {
    let mut counter: Counter = Counter { value: 1 }
    counter.absorb(mut counter)
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0502");
        assert!(err.message.contains("counter"));
    }

    #[test]
    fn rejects_impl_for_non_local_std_struct() {
        let source = r#"package app.main

import std.fs
import std.io

impl File {
    pub fn label(self) -> string {
        return "file"
    }
}

fn main() -> void {
    io.println("done")
}
"#;

        let err = parse_inline(source).unwrap_err();

        assert_eq!(err.code, "E0255");
        assert!(err.message.contains("local struct"));
        assert!(err.message.contains("File"));
    }

    #[test]
    fn rejects_struct_and_enum_with_same_name() {
        let source = r#"package app.main

struct Status {
    code: i32
}

enum Status {
    Ok
}

fn main() -> void {
}
"#;

        let err = parse_inline(source).unwrap_err();

        assert_eq!(err.code, "E0312");
        assert!(err.message.contains("Status"));
    }

    #[test]
    fn rejects_user_type_conflicting_with_imported_std_type() {
        let source = r#"package app.main

import std.result

struct Result {
    value: i32
}

fn main() -> void {
}
"#;

        let err = parse_inline(source).unwrap_err();

        assert_eq!(err.code, "E0312");
        assert!(err.message.contains("Result"));
    }

    #[test]
    fn rejects_user_enum_conflicting_with_required_std_result() {
        let source = r#"package app.main

import std.result

enum Result {
    Local
}

fn main() -> void {
}
"#;

        let err = parse_inline(source).unwrap_err();

        assert_eq!(err.code, "E0312");
        assert!(err.message.contains("Result"));
        assert!(err.message.contains("standard library"));
    }

    #[test]
    fn rejects_user_enum_conflicting_with_required_std_option() {
        let source = r#"package app.main

import std.array

enum Option {
    Local
}

fn main() -> void {
    let mut items: Array<i32> = Array.new<i32>()
    items.push(1)
}
"#;

        let err = parse_inline(source).unwrap_err();

        assert_eq!(err.code, "E0312");
        assert!(err.message.contains("Option"));
        assert!(err.message.contains("standard library"));
    }

    #[test]
    fn rejects_user_struct_conflicting_with_required_std_fs_error() {
        let source = r#"package app.main

import std.fs

struct FsError {
    code: i32
}

fn main() -> void {
}
"#;

        let err = parse_inline(source).unwrap_err();

        assert_eq!(err.code, "E0312");
        assert!(err.message.contains("FsError"));
        assert!(err.message.contains("standard library"));
    }

    #[test]
    fn accepts_pub_declarations_as_visibility_metadata() {
        let source = r#"package app.main

import std.io

pub struct User {
    pub id: string
    email: string
}

pub enum Color {
    Red
    Blue
}

pub fn label(color: Color) -> string {
    return match color {
        Color.Red => "red"
        Color.Blue => "blue"
    }
}

pub fn main() -> void {
    let user: User = User { id: "42", email: "a@nomo.dev" }
    let color: Color = Color.Red
    let text: string = label(color)
    io.println(text)
}
"#;

        let program = parse_inline(source).unwrap();
        assert_eq!(program.structs.len(), 1);
        assert_eq!(program.enums.len(), 1);
        assert!(
            program
                .functions
                .iter()
                .any(|function| function.name == "main")
        );
    }

    #[test]
    fn rejects_struct_literal_field_type_mismatch() {
        let source = r#"package app.main

import std.io

struct Point {
    x: i64
    y: i64
}

fn main() -> void {
    let point: Point = Point { x: "bad", y: 2 }
    io.println("done")
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0404");
    }

    #[test]
    fn rejects_unknown_struct_field_access() {
        let source = r#"package app.main

import std.io

struct Point {
    x: i64
    y: i64
}

fn main() -> void {
    let point: Point = Point { x: 1, y: 2 }
    let z: i64 = point.z
    io.println("done")
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0308");
    }

    #[test]
    fn accepts_enum_variant_and_exhaustive_match() {
        let source = r#"package app.main

import std.io

enum Color {
    Red
    Blue
}

fn label(color: Color) -> string {
    return match color {
        Color.Red => "red"
        Color.Blue => "blue"
    }
}

fn main() -> void {
    let color: Color = Color.Red
    let text: string = label(color)
    io.println(text)
}
"#;

        let program = parse_inline(source).unwrap();
        assert_eq!(program.enums.len(), 1);
        assert_eq!(
            program.enums[0]
                .variants
                .iter()
                .map(|variant| variant.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Red", "Blue"]
        );
        let label = program
            .functions
            .iter()
            .find(|f| f.name == "label")
            .unwrap();
        assert_eq!(
            label.params[0].value_type,
            ValueType::Enum("Color".to_string(), Vec::new())
        );
        assert!(matches!(
            label.body[0],
            Statement::Return(Some(ValueExpr::Match { .. }))
        ));
    }

    #[test]
    fn rejects_generic_enum_type_with_missing_type_argument() {
        let source = r#"package app.main

enum Option<T> {
    Some(T)
    None
}

fn main() -> void {
    let value: Option = Option.None
}
"#;

        let err = parse_inline(source).unwrap_err();

        assert_eq!(err.code, "E0403");
        assert!(err.message.contains("Option"));
    }

    #[test]
    fn rejects_non_generic_enum_type_with_extra_type_argument() {
        let source = r#"package app.main

enum Color {
    Red
}

fn main() -> void {
    let value: Color<i32> = Color.Red
}
"#;

        let err = parse_inline(source).unwrap_err();

        assert_eq!(err.code, "E0403");
        assert!(err.message.contains("Color"));
    }

    #[test]
    fn rejects_std_result_type_with_missing_type_argument() {
        let source = r#"package app.main

import std.result

fn main() -> void {
    let value: Result<i32> = Result.Ok(1)
}
"#;

        let err = parse_inline(source).unwrap_err();

        assert_eq!(err.code, "E0403");
        assert!(err.message.contains("Result"));
    }

    #[test]
    fn rejects_non_exhaustive_match() {
        let source = r#"package app.main

import std.io

enum Color {
    Red
    Blue
}

fn label(color: Color) -> string {
    return match color {
        Color.Red => "red"
    }
}

fn main() -> void {
    io.println("done")
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0318");
    }

    #[test]
    fn accepts_payload_enum_and_match_binding() {
        let source = r#"package app.main

import std.io

enum MaybeInt {
    Some(i64)
    None
}

fn unwrap_or_zero(value: MaybeInt) -> i64 {
    return match value {
        MaybeInt.Some(n) => n
        MaybeInt.None => 0
    }
}

fn main() -> void {
    let value: MaybeInt = MaybeInt.Some(41)
    let answer: i64 = unwrap_or_zero(value) + 1
    io.println("done")
}
"#;

        let program = parse_inline(source).unwrap();
        assert_eq!(program.enums[0].variants[0].payload, Some(ValueType::Int));
        let unwrap = program
            .functions
            .iter()
            .find(|function| function.name == "unwrap_or_zero")
            .unwrap();
        assert!(matches!(
            unwrap.body[0],
            Statement::Return(Some(ValueExpr::Match { .. }))
        ));
    }

    #[test]
    fn accepts_struct_payload_enum_and_match_field_access() {
        let source = r#"package app.main

import std.io

struct User {
    email: string
}

enum MaybeUser {
    Some(User)
    None
}

fn label(value: MaybeUser) -> string {
    return match value {
        MaybeUser.Some(user) => user.email
        MaybeUser.None => "missing"
    }
}

fn main() -> void {
    let value: MaybeUser = MaybeUser.Some(User { email: "a@nomo.dev" })
    io.println(label(value))
}
"#;

        let program = parse_inline(source).unwrap();
        assert_eq!(
            program.enums[0].variants[0].payload,
            Some(ValueType::Struct("User".to_string(), Vec::new()))
        );
        let label = program
            .functions
            .iter()
            .find(|function| function.name == "label")
            .unwrap();
        assert!(matches!(
            label.body[0],
            Statement::Return(Some(ValueExpr::Match { ref arms, .. }))
                if matches!(
                    arms[0].value,
                    ValueExpr::EnumPayloadFieldAccess {
                        ref variant,
                        ref field,
                        ..
                    } if variant == "Some" && field == "email"
                )
        ));
    }

    #[test]
    fn accepts_struct_payload_enum_and_match_method_call() {
        let source = r#"package app.main

import std.io

struct User {
    email: string
}

impl User {
    pub fn label(self) -> string {
        return self.email
    }
}

enum MaybeUser {
    Some(User)
    None
}

fn label(value: MaybeUser) -> string {
    return match value {
        MaybeUser.Some(user) => user.label()
        MaybeUser.None => "missing"
    }
}

fn main() -> void {
    let value: MaybeUser = MaybeUser.Some(User { email: "a@nomo.dev" })
    io.println(label(value))
}
"#;

        let program = parse_inline(source).unwrap();
        let label = program
            .functions
            .iter()
            .find(|function| function.name == "label")
            .unwrap();
        assert!(matches!(
            label.body[0],
            Statement::Return(Some(ValueExpr::Match { ref arms, .. }))
                if matches!(
                    arms[0].value,
                    ValueExpr::Call {
                        ref name,
                        ref args,
                    } if name == "User_label"
                        && matches!(
                            args.as_slice(),
                            [ValueExpr::EnumPayload { variant, .. }] if variant == "Some"
                        )
                )
        ));
    }

    #[test]
    fn rejects_match_payload_binding_shadowing_outer_variable() {
        let source = r#"package app.main

import std.io

enum Option<T> {
    Some(T)
    None
}

fn main() -> void {
    let text: string = "outer"
    let value: Option<string> = Option.Some("inner")
    let result: string = match value {
        Option.Some(text) => text
        Option.None => text
    }
    io.println(result)
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0302");
        assert!(err.message.contains("text"));
    }

    #[test]
    fn rejects_let_else_binding_shadowing_outer_variable() {
        let source = r#"package app.main

fn label(value: Option<string>) -> string {
    let text: string = "outer"
    let Some(text) = value else {
        return "missing"
    }
    return text
}

fn main() -> void {
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0302");
        assert!(err.message.contains("text"));
    }

    #[test]
    fn rejects_if_let_binding_shadowing_outer_variable() {
        let source = r#"package app.main

fn label(value: Option<string>) -> string {
    let text: string = "outer"
    if let Some(text) = value {
        return text
    } else {
        return "missing"
    }
}

fn main() -> void {
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0302");
        assert!(err.message.contains("text"));
    }

    #[test]
    fn rejects_for_iter_binding_shadowing_outer_variable() {
        let source = r#"package app.main

import std.array

fn main() -> void {
    let item: i32 = 0
    let mut items: Array<i32> = Array.new<i32>()
    items.push(1)
    for item in items {
    }
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0302");
        assert!(err.message.contains("item"));
    }

    #[test]
    fn accepts_generic_enum_instantiation_and_match_binding() {
        let source = r#"package app.main

import std.io

enum Option<T> {
    Some(T)
    None
}

fn unwrap_or_zero(value: Option<i64>) -> i64 {
    return match value {
        Option.Some(n) => n
        Option.None => 0
    }
}

fn main() -> void {
    let value: Option<i64> = Option.Some(41)
    let answer: i64 = unwrap_or_zero(value) + 1
    io.println("done")
}
"#;

        let program = parse_inline(source).unwrap();
        assert_eq!(program.enums[0].type_params, vec!["T"]);
        let unwrap = program
            .functions
            .iter()
            .find(|function| function.name == "unwrap_or_zero")
            .unwrap();
        assert_eq!(
            unwrap.params[0].value_type,
            ValueType::Enum("Option".to_string(), vec![ValueType::Int])
        );
    }

    #[test]
    fn accepts_unqualified_option_and_result_prelude_variants() {
        let source = r#"package app.main

fn parse() -> Result<i32, string> {
    return Ok(41)
}

fn label(value: Option<i32>) -> string {
    return match value {
        Some(number) => if number == 41 {
            "some"
        } else {
            "other"
        }
        None => "none"
    }
}

fn main() -> Result<void, string> {
    let value: i32 = parse()?
    let maybe: Option<i32> = Some(value)
    let text: string = label(maybe)
    return Ok(void)
}
"#;

        let program = parse_inline(source).unwrap();
        let parse = program
            .functions
            .iter()
            .find(|function| function.name == "parse")
            .unwrap();
        assert!(matches!(
            parse.body[0],
            Statement::Return(Some(ValueExpr::EnumVariant {
                ref enum_name,
                ref variant,
                ..
            })) if enum_name == "Result" && variant == "Ok"
        ));
        let label = program
            .functions
            .iter()
            .find(|function| function.name == "label")
            .unwrap();
        assert!(matches!(
            label.body[0],
            Statement::Return(Some(ValueExpr::Match { ref arms, .. }))
                if arms[0].enum_name == "Option"
                    && arms[0].variant == "Some"
                    && arms[1].variant == "None"
        ));
    }

    #[test]
    fn accepts_let_else_with_option_payload_binding() {
        let source = r#"package app.main

fn unwrap_or_fallback(value: Option<string>) -> string {
    let Some(text) = value else {
        return "fallback"
    }
    return text
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let unwrap = program
            .functions
            .iter()
            .find(|function| function.name == "unwrap_or_fallback")
            .unwrap();
        assert!(matches!(
            unwrap.body[0],
            Statement::LetElse {
                ref binding,
                ref value_type,
                ref enum_name,
                ref variant,
                ..
            } if binding == "text"
                && value_type == &ValueType::String
                && enum_name == "Option"
                && variant == "Some"
        ));
        assert!(matches!(
            unwrap.body[1],
            Statement::Return(Some(ValueExpr::Variable(ref name))) if name == "text"
        ));
    }

    #[test]
    fn rejects_let_else_with_non_diverging_else_body() {
        let source = r#"package app.main

fn main() -> void {
    let value: Option<i32> = None
    let Some(number) = value else {
        let fallback: i32 = 0
    }
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0521");
        assert!(err.message.contains("must diverge"));
    }

    #[test]
    fn accepts_if_let_with_option_payload_binding() {
        let source = r#"package app.main

fn label(value: Option<string>) -> string {
    if let Some(text) = value {
        return text
    } else {
        return "missing"
    }
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let label = program
            .functions
            .iter()
            .find(|function| function.name == "label")
            .unwrap();
        assert!(matches!(
            label.body[0],
            Statement::IfLet {
                ref binding,
                ref value_type,
                ref enum_name,
                ref variant,
                ref else_body,
                ..
            } if binding.as_deref() == Some("text")
                && value_type.as_ref() == Some(&ValueType::String)
                && enum_name == "Option"
                && variant == "Some"
                && matches!(else_body.as_deref(), Some([Statement::Return(Some(ValueExpr::StringLiteral(_)))]))
        ));
        let Statement::IfLet { body, .. } = &label.body[0] else {
            panic!("expected if-let statement");
        };
        assert!(matches!(
            body.as_slice(),
            [Statement::Return(Some(ValueExpr::Variable(name)))] if name == "text"
        ));
    }

    #[test]
    fn accepts_if_let_with_unit_variant() {
        let source = r#"package app.main

fn is_missing(value: Option<string>) -> bool {
    if let None = value {
        return true
    }
    return false
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let is_missing = program
            .functions
            .iter()
            .find(|function| function.name == "is_missing")
            .unwrap();
        assert!(matches!(
            is_missing.body[0],
            Statement::IfLet {
                ref binding,
                ref value_type,
                ref variant,
                ..
            } if binding.is_none() && value_type.is_none() && variant == "None"
        ));
    }

    #[test]
    fn accepts_question_in_pattern_scrutinees() {
        let source = r#"package app.main

fn load() -> Result<Option<string>, string> {
    return Ok(Some("value"))
}

fn unwrap_with_let_else() -> Result<string, string> {
    let Some(text) = load()? else {
        return Err("missing")
    }
    return Ok(text)
}

fn unwrap_with_if_let() -> Result<string, string> {
    if let Some(text) = load()? {
        return Ok(text)
    } else {
        return Err("missing")
    }
}

fn unwrap_with_match() -> Result<string, string> {
    match load()? {
        Some(text) => {
            return Ok(text)
        }
        None => {
            return Err("missing")
        }
    }
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let unwrap_with_let_else = program
            .functions
            .iter()
            .find(|function| function.name == "unwrap_with_let_else")
            .unwrap();
        assert!(matches!(
            unwrap_with_let_else.body.as_slice(),
            [
                Statement::QuestionLet {
                    name: temp,
                    result_expr: ValueExpr::Call { name: call_name, .. },
                    ..
                },
                Statement::LetElse {
                    value: ValueExpr::Variable(value),
                    binding,
                    ..
                },
                Statement::Return(Some(_)),
            ] if temp.starts_with("__question_value_")
                && call_name == "load"
                && value == temp
                && binding == "text"
        ));

        let unwrap_with_if_let = program
            .functions
            .iter()
            .find(|function| function.name == "unwrap_with_if_let")
            .unwrap();
        assert!(matches!(
            unwrap_with_if_let.body.as_slice(),
            [
                Statement::QuestionLet {
                    name: temp,
                    result_expr: ValueExpr::Call { name: call_name, .. },
                    ..
                },
                Statement::IfLet {
                    value: ValueExpr::Variable(value),
                    binding: Some(binding),
                    ..
                },
            ] if temp.starts_with("__question_value_")
                && call_name == "load"
                && value == temp
                && binding == "text"
        ));

        let unwrap_with_match = program
            .functions
            .iter()
            .find(|function| function.name == "unwrap_with_match")
            .unwrap();
        assert!(matches!(
            unwrap_with_match.body.as_slice(),
            [
                Statement::QuestionLet {
                    name: temp,
                    result_expr: ValueExpr::Call { name: call_name, .. },
                    ..
                },
                Statement::Match {
                    value: ValueExpr::Variable(value),
                    enum_name,
                    arms,
                    ..
                },
            ] if temp.starts_with("__question_value_")
                && call_name == "load"
                && value == temp
                && enum_name == "Option"
                && arms.len() == 2
        ));
    }

    #[test]
    fn rejects_if_let_binding_outside_body() {
        let source = r#"package app.main

fn main() -> void {
    let value: Option<string> = Some("inner")
    if let Some(text) = value {
    }
    let copy: string = text
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0303");
        assert!(err.message.contains("text"));
    }

    #[test]
    fn unqualified_variant_does_not_target_user_enum() {
        let source = r#"package app.main

enum MaybeInt {
    Some(i32)
    None
}

fn main() -> void {
    let value: MaybeInt = Some(1)
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0324");
        assert!(err.message.contains("Option.Some"));
    }

    #[test]
    fn function_name_shadows_unqualified_prelude_variant() {
        let source = r#"package app.main

fn Ok(value: i32) -> i32 {
    return value
}

fn main() -> void {
    let value: i32 = Ok(1)
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Let {
                initializer: ValueExpr::Call { ref name, .. },
                ..
            } if name == "Ok"
        ));
    }

    #[test]
    fn local_binding_shadows_unqualified_prelude_variant_call() {
        let source = r#"package app.main

fn main() -> void {
    let Ok: i32 = 1
    let value: Result<i32, string> = Ok(2)
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0305");
        assert!(err.message.contains("local variable `Ok` is not callable"));
    }

    #[test]
    fn local_binding_shadows_unqualified_prelude_variant_pattern() {
        let source = r#"package app.main

fn main() -> void {
    let Some: string = "shadow"
    let value: Option<i32> = Option.Some(1)
    let label: string = match value {
        Some(number) => "some"
        None => "none"
    }
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0316");
        assert!(err.message.contains("Option.Variant"));
    }

    #[test]
    fn qualified_core_variant_still_works_when_local_name_shadows_prelude() {
        let source = r#"package app.main

import std.option

fn main() -> void {
    let Some: string = "shadow"
    let value: Option<i32> = Option.Some(1)
    let label: string = match value {
        Option.Some(number) => "some"
        Option.None => "none"
    }
}
"#;

        parse_inline(source).unwrap();
    }

    #[test]
    fn accepts_result_map_err_with_question_propagation() {
        let source = r#"package app.main

import std.result

struct AppError {
    message: string
}

fn parse_label() -> Result<string, string> {
    return Err("bad")
}

fn app_error_from_string(message: string) -> AppError {
    return AppError { message: message }
}

fn decorate_label() -> Result<string, AppError> {
    let raw: Result<string, string> = parse_label()
    let label: string = raw.map_err(app_error_from_string)?
    return Ok(label)
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let decorate = program
            .functions
            .iter()
            .find(|function| function.name == "decorate_label")
            .unwrap();
        assert!(matches!(
            decorate.body[1],
            Statement::QuestionLet {
                ref result_type,
                result_expr: ValueExpr::ResultMapErr {
                    ref ok_type,
                    ref source_err_type,
                    ref target_err_type,
                    ref converter,
                    ..
                },
                ..
            } if result_type == &ValueType::Enum(
                    "Result".to_string(),
                    vec![
                        ValueType::String,
                        ValueType::Struct("AppError".to_string(), Vec::new())
                    ]
                )
                && ok_type == &ValueType::String
                && source_err_type == &ValueType::String
                && target_err_type == &ValueType::Struct("AppError".to_string(), Vec::new())
                && converter == "app_error_from_string"
        ));
    }

    #[test]
    fn accepts_specific_result_map_err_import() {
        let source = r#"package app.main

import std.result.Result
import std.result.map_err

struct AppError {
    message: string
}

fn parse_label() -> Result<string, string> {
    return Err("bad")
}

fn app_error_from_string(message: string) -> AppError {
    return AppError { message: message }
}

fn decorate_label() -> Result<string, AppError> {
    let raw: Result<string, string> = parse_label()
    let label: string = raw.map_err(app_error_from_string)?
    return Ok(label)
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let decorate = program
            .functions
            .iter()
            .find(|function| function.name == "decorate_label")
            .unwrap();
        assert!(matches!(
            decorate.body[1],
            Statement::QuestionLet {
                result_expr: ValueExpr::ResultMapErr {
                    ref converter,
                    ..
                },
                ..
            } if converter == "app_error_from_string"
        ));
    }

    #[test]
    fn accepts_result_value_methods() {
        let source = r#"package app.main

import std.result
import std.string

fn exclaim(text: string) -> string {
    return text.concat("!")
}

fn decorate(text: string) -> Result<string, string> {
    return Ok(text.concat(" ok"))
}

fn main() -> void {
    let ok: Result<string, string> = Ok("seed")
    let err: Result<string, string> = Err("bad")
    let present: bool = ok.is_ok()
    let absent: bool = err.is_err()
    let fallback: string = err.unwrap_or("fallback")
    let mapped: Result<string, string> = ok.map(exclaim)
    let chained: Result<string, string> = ok.and_then(decorate)
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program
            .functions
            .iter()
            .find(|function| function.name == "main")
            .unwrap();
        assert!(matches!(
            main.body[2],
            Statement::Let {
                ref value_type,
                initializer: ValueExpr::ResultIsOk { .. },
                ..
            } if value_type == &ValueType::Bool
        ));
        assert!(matches!(
            main.body[4],
            Statement::Let {
                ref value_type,
                initializer: ValueExpr::ResultUnwrapOr { .. },
                ..
            } if value_type == &ValueType::String
        ));
        assert!(matches!(
            main.body[5],
            Statement::Let {
                ref value_type,
                initializer: ValueExpr::ResultMap { .. },
                ..
            } if value_type == &ValueType::Enum(
                "Result".to_string(),
                vec![ValueType::String, ValueType::String]
            )
        ));
        assert!(matches!(
            main.body[6],
            Statement::Let {
                ref value_type,
                initializer: ValueExpr::ResultAndThen { .. },
                ..
            } if value_type == &ValueType::Enum(
                "Result".to_string(),
                vec![ValueType::String, ValueType::String]
            )
        ));
    }

    #[test]
    fn accepts_specific_result_helper_imports() {
        let source = r#"package app.main

import std.result.Result
import std.result.is_ok
import std.result.is_err
import std.result.unwrap_or
import std.result.map
import std.result.map_err
import std.result.and_then
import std.string

struct AppError {
    message: string
}

fn exclaim(text: string) -> string {
    return text.concat("!")
}

fn decorate(text: string) -> Result<string, string> {
    return Ok(text.concat(" ok"))
}

fn app_error_from_string(message: string) -> AppError {
    return AppError { message: message }
}

fn main() -> void {
    let ok: Result<string, string> = Ok("seed")
    let err: Result<string, string> = Err("bad")
    let present: bool = is_ok(ok)
    let absent: bool = is_err(err)
    let fallback: string = unwrap_or(err, "fallback")
    let mapped: Result<string, string> = map(ok, exclaim)
    let converted: Result<string, AppError> = map_err(err, app_error_from_string)
    let chained: Result<string, string> = and_then(ok, decorate)
}
"#;

        parse_inline(source).unwrap();
    }

    #[test]
    fn accepts_result_module_helpers() {
        let source = r#"package app.main

import std.result
import std.string

struct AppError {
    message: string
}

fn exclaim(text: string) -> string {
    return text.concat("!")
}

fn decorate(text: string) -> Result<string, string> {
    return Ok(text.concat(" ok"))
}

fn app_error_from_string(message: string) -> AppError {
    return AppError { message: message }
}

fn main() -> void {
    let ok: Result<string, string> = Ok("seed")
    let err: Result<string, string> = Err("bad")
    let present: bool = result.is_ok(ok)
    let absent: bool = result.is_err(err)
    let fallback: string = result.unwrap_or(err, "fallback")
    let mapped: Result<string, string> = result.map(ok, exclaim)
    let converted: Result<string, AppError> = result.map_err(err, app_error_from_string)
    let chained: Result<string, string> = result.and_then(ok, decorate)
}
"#;

        parse_inline(source).unwrap();
    }

    #[test]
    fn accepts_option_value_methods() {
        let source = r#"package app.main

import std.option
import std.string

fn exclaim(text: string) -> string {
    return text.concat("!")
}

fn decorate(text: string) -> Option<string> {
    return Some(text.concat(" ok"))
}

fn main() -> void {
    let some: Option<string> = Some("seed")
    let none: Option<string> = None
    let present: bool = some.is_some()
    let absent: bool = none.is_none()
    let fallback: string = none.unwrap_or("fallback")
    let mapped: Option<string> = some.map(exclaim)
    let chained: Option<string> = some.and_then(decorate)
}
"#;

        let program = parse_inline(source).unwrap();
        let main = program
            .functions
            .iter()
            .find(|function| function.name == "main")
            .unwrap();
        assert!(matches!(
            main.body[2],
            Statement::Let {
                ref value_type,
                initializer: ValueExpr::OptionIsSome { .. },
                ..
            } if value_type == &ValueType::Bool
        ));
        assert!(matches!(
            main.body[4],
            Statement::Let {
                ref value_type,
                initializer: ValueExpr::OptionUnwrapOr { .. },
                ..
            } if value_type == &ValueType::String
        ));
        assert!(matches!(
            main.body[5],
            Statement::Let {
                ref value_type,
                initializer: ValueExpr::OptionMap { .. },
                ..
            } if value_type == &ValueType::Enum("Option".to_string(), vec![ValueType::String])
        ));
        assert!(matches!(
            main.body[6],
            Statement::Let {
                ref value_type,
                initializer: ValueExpr::OptionAndThen { .. },
                ..
            } if value_type == &ValueType::Enum("Option".to_string(), vec![ValueType::String])
        ));
    }

    #[test]
    fn accepts_specific_option_helper_imports() {
        let source = r#"package app.main

import std.option.Option
import std.option.is_some
import std.option.is_none
import std.option.unwrap_or
import std.option.map
import std.option.and_then
import std.string

fn exclaim(text: string) -> string {
    return text.concat("!")
}

fn decorate(text: string) -> Option<string> {
    return Some(text.concat(" ok"))
}

fn main() -> void {
    let some: Option<string> = Some("seed")
    let none: Option<string> = None
    let present: bool = is_some(some)
    let absent: bool = is_none(none)
    let fallback: string = unwrap_or(none, "fallback")
    let mapped: Option<string> = map(some, exclaim)
    let chained: Option<string> = and_then(some, decorate)
}
"#;

        parse_inline(source).unwrap();
    }

    #[test]
    fn accepts_option_module_helpers() {
        let source = r#"package app.main

import std.option
import std.string

fn exclaim(text: string) -> string {
    return text.concat("!")
}

fn decorate(text: string) -> Option<string> {
    return Some(text.concat(" ok"))
}

fn main() -> void {
    let some: Option<string> = Some("seed")
    let none: Option<string> = None
    let present: bool = option.is_some(some)
    let absent: bool = option.is_none(none)
    let fallback: string = option.unwrap_or(none, "fallback")
    let mapped: Option<string> = option.map(some, exclaim)
    let chained: Option<string> = option.and_then(some, decorate)
}
"#;

        parse_inline(source).unwrap();
    }

    #[test]
    fn rejects_option_method_without_option_import() {
        let source = r#"package app.main

import std.option.Option

fn main() -> void {
    let some: Option<string> = Some("seed")
    let present: bool = some.is_some()
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0301");
        assert!(err.message.contains("std.option"));
    }

    #[test]
    fn accepts_question_in_let_initializer_call_argument() {
        let source = r#"package app.main

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn decorate(value: string) -> string {
    return value
}

fn compute() -> Result<string, string> {
    let label: string = decorate(parse_label()?)
    return Ok(label)
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let compute = program
            .functions
            .iter()
            .find(|function| function.name == "compute")
            .unwrap();
        assert!(matches!(
            compute.body.as_slice(),
            [
                Statement::QuestionLet {
                    name,
                    value_type,
                    result_type,
                    result_expr: ValueExpr::Call { name: call_name, .. },
                    ..
                },
                Statement::Let {
                    name: label_name,
                    value_type: label_type,
                    initializer: ValueExpr::Call { args, .. },
                },
                Statement::Return(Some(_)),
            ] if name.starts_with("__question_value_")
                && value_type == &ValueType::String
                && result_type == &ValueType::Enum(
                    "Result".to_string(),
                    vec![ValueType::String, ValueType::String]
                )
                && call_name == "parse_label"
                && label_name == "label"
                && label_type == &ValueType::String
                && matches!(args.as_slice(), [ValueExpr::Variable(arg)] if arg == name)
        ));
    }

    #[test]
    fn accepts_question_in_struct_literal_field_and_enum_payload() {
        let source = r#"package app.main

struct Label {
    value: string
}

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn compute() -> Result<Label, string> {
    let label: Label = Label { value: parse_label()? }
    return Ok(Label { value: parse_label()? })
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let compute = program
            .functions
            .iter()
            .find(|function| function.name == "compute")
            .unwrap();
        assert_eq!(
            compute
                .body
                .iter()
                .filter(|stmt| matches!(stmt, Statement::QuestionLet { .. }))
                .count(),
            2
        );
        assert!(matches!(
            &compute.body[1],
            Statement::Let {
                initializer: ValueExpr::StructLiteral { fields, .. },
                ..
            } if matches!(fields.as_slice(), [(field, ValueExpr::Variable(_))] if field == "value")
        ));
        assert!(matches!(
            &compute.body[3],
            Statement::Return(Some(ValueExpr::EnumVariant {
                payload: Some(payload),
                ..
            })) if matches!(payload.as_ref(), ValueExpr::StructLiteral { fields, .. }
                if matches!(fields.as_slice(), [(field, ValueExpr::Variable(_))] if field == "value"))
        ));
    }

    #[test]
    fn accepts_question_in_binary_cast_and_return_ok_call_argument() {
        let source = r#"package app.main

fn parse_number() -> Result<i32, string> {
    return Ok(1)
}

fn wrap(value: i32) -> i32 {
    return value
}

fn compute() -> Result<i32, string> {
    let total: i32 = parse_number()? + parse_number()? as i32
    return Ok(wrap(parse_number()?))
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let compute = program
            .functions
            .iter()
            .find(|function| function.name == "compute")
            .unwrap();
        assert_eq!(
            compute
                .body
                .iter()
                .filter(|stmt| matches!(stmt, Statement::QuestionLet { .. }))
                .count(),
            3
        );
        assert!(matches!(
            &compute.body[2],
            Statement::Let {
                initializer: ValueExpr::Binary { left, right, .. },
                ..
            } if matches!(left.as_ref(), ValueExpr::Variable(_))
                && matches!(right.as_ref(), ValueExpr::Cast { expr, .. }
                    if matches!(expr.as_ref(), ValueExpr::Variable(_)))
        ));
        assert!(matches!(
            &compute.body[4],
            Statement::Return(Some(ValueExpr::EnumVariant {
                payload: Some(payload),
                ..
            })) if matches!(payload.as_ref(), ValueExpr::Call { args, .. }
                if matches!(args.as_slice(), [ValueExpr::Variable(_)]))
        ));
    }

    #[test]
    fn accepts_question_in_if_initializer_branch() {
        let source = r#"package app.main

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn compute(flag: bool) -> Result<string, string> {
    let label: string = if flag {
        parse_label()?
    } else {
        "fallback"
    }
    return Ok(label)
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let compute = program
            .functions
            .iter()
            .find(|function| function.name == "compute")
            .unwrap();
        assert!(matches!(
            compute.body.as_slice(),
            [
                Statement::LetIf {
                    name,
                    value_type,
                    condition: ValueExpr::Variable(condition),
                    body,
                    else_body,
                },
                Statement::Return(Some(_)),
            ] if name == "label"
                && value_type == &ValueType::String
                && condition == "flag"
                && matches!(
                    body.as_slice(),
                    [
                        Statement::QuestionLet {
                            name: temp,
                            result_expr: ValueExpr::Call { name: call_name, .. },
                            ..
                        },
                        Statement::Assign {
                            name: assign_name,
                            value: ValueExpr::Variable(assign_value),
                        },
                    ] if temp.starts_with("__question_value_")
                        && call_name == "parse_label"
                        && assign_name == "label"
                        && assign_value == temp
                )
                && matches!(
                    else_body.as_slice(),
                    [Statement::Assign {
                        name: assign_name,
                        value: ValueExpr::StringLiteral(value),
                    }] if assign_name == "label" && value == "fallback"
                )
        ));
    }

    #[test]
    fn accepts_question_in_if_initializer_condition() {
        let source = r#"package app.main

fn parse_flag() -> Result<bool, string> {
    return Ok(true)
}

fn compute() -> Result<string, string> {
    let label: string = if parse_flag()? {
        "value"
    } else {
        "fallback"
    }
    return Ok(label)
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let compute = program
            .functions
            .iter()
            .find(|function| function.name == "compute")
            .unwrap();
        assert!(matches!(
            compute.body.as_slice(),
            [
                Statement::QuestionLet {
                    name: temp,
                    result_expr: ValueExpr::Call { name: call_name, .. },
                    ..
                },
                Statement::LetIf {
                    name,
                    condition: ValueExpr::Variable(condition),
                    ..
                },
                Statement::Return(Some(_)),
            ] if temp.starts_with("__question_value_")
                && call_name == "parse_flag"
                && name == "label"
                && condition == temp
        ));
    }

    #[test]
    fn accepts_question_in_tail_if_expression_branch() {
        let source = r#"package app.main

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn compute(flag: bool) -> Result<string, string> {
    if flag {
        Ok(parse_label()?)
    } else {
        Ok("fallback")
    }
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let compute = program
            .functions
            .iter()
            .find(|function| function.name == "compute")
            .unwrap();
        assert!(matches!(
            compute.body.as_slice(),
            [Statement::If {
                condition: ValueExpr::Variable(condition),
                body,
                else_body,
            }] if condition == "flag"
                && matches!(
                    body.as_slice(),
                    [Statement::QuestionReturnOk {
                        result_expr: ValueExpr::Call { name: call_name, .. },
                        ..
                    }] if call_name == "parse_label"
                )
                && matches!(
                    else_body.as_slice(),
                    [Statement::Return(Some(ValueExpr::EnumVariant {
                        payload: Some(payload),
                        ..
                    }))] if matches!(payload.as_ref(), ValueExpr::StringLiteral(value) if value == "fallback")
                )
        ));
    }

    #[test]
    fn accepts_question_in_tail_if_expression_condition() {
        let source = r#"package app.main

fn parse_flag() -> Result<bool, string> {
    return Ok(true)
}

fn compute() -> Result<string, string> {
    if parse_flag()? {
        Ok("value")
    } else {
        Ok("fallback")
    }
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let compute = program
            .functions
            .iter()
            .find(|function| function.name == "compute")
            .unwrap();
        assert!(matches!(
            compute.body.as_slice(),
            [
                Statement::QuestionLet {
                    name: temp,
                    result_expr: ValueExpr::Call { name: call_name, .. },
                    ..
                },
                Statement::If {
                    condition: ValueExpr::Variable(condition),
                    ..
                },
            ] if temp.starts_with("__question_value_")
                && call_name == "parse_flag"
                && condition == temp
        ));
    }

    #[test]
    fn accepts_question_in_explicit_return_if_expression() {
        let source = r#"package app.main

fn parse_flag() -> Result<bool, string> {
    return Ok(true)
}

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn compute() -> Result<string, string> {
    return if parse_flag()? {
        Ok(parse_label()?)
    } else {
        Ok("fallback")
    }
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let compute = program
            .functions
            .iter()
            .find(|function| function.name == "compute")
            .unwrap();
        assert!(matches!(
            compute.body.as_slice(),
            [
                Statement::QuestionLet {
                    name: condition_temp,
                    result_expr: ValueExpr::Call { name: condition_call, .. },
                    ..
                },
                Statement::If {
                    condition: ValueExpr::Variable(condition_name),
                    body,
                    else_body,
                },
            ] if condition_temp.starts_with("__question_value_")
                && condition_call == "parse_flag"
                && condition_name == condition_temp
                && matches!(
                    body.as_slice(),
                    [Statement::QuestionReturnOk {
                        result_expr: ValueExpr::Call { name: branch_call, .. },
                        ..
                    }] if branch_call == "parse_label"
                )
                && matches!(
                    else_body.as_slice(),
                    [Statement::Return(Some(ValueExpr::EnumVariant {
                        variant,
                        ..
                    }))] if variant == "Ok"
                )
        ));
    }

    #[test]
    fn accepts_question_in_return_ok_if_expression() {
        let source = r#"package app.main

fn parse_flag() -> Result<bool, string> {
    return Ok(true)
}

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn compute() -> Result<string, string> {
    return Ok(if parse_flag()? {
        parse_label()?
    } else {
        "fallback"
    })
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let compute = program
            .functions
            .iter()
            .find(|function| function.name == "compute")
            .unwrap();
        assert!(matches!(
            compute.body.as_slice(),
            [
                Statement::QuestionLet {
                    name: condition_temp,
                    result_expr: ValueExpr::Call { name: condition_call, .. },
                    ..
                },
                Statement::If {
                    condition: ValueExpr::Variable(condition_name),
                    body,
                    else_body,
                },
            ] if condition_temp.starts_with("__question_value_")
                && condition_call == "parse_flag"
                && condition_name == condition_temp
                && matches!(
                    body.as_slice(),
                    [Statement::QuestionReturnOk {
                        result_expr: ValueExpr::Call { name: branch_call, .. },
                        ..
                    }] if branch_call == "parse_label"
                )
                && matches!(
                    else_body.as_slice(),
                    [Statement::Return(Some(ValueExpr::EnumVariant {
                        variant,
                        ..
                    }))] if variant == "Ok"
                )
        ));
    }

    #[test]
    fn accepts_question_in_return_ok_match_expression() {
        let source = r#"package app.main

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn maybe_label() -> Result<Option<string>, string> {
    return Ok(None)
}

fn compute() -> Result<string, string> {
    return Ok(match maybe_label()? {
        Some(text) => text
        None => parse_label()?
    })
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let compute = program
            .functions
            .iter()
            .find(|function| function.name == "compute")
            .unwrap();
        assert!(matches!(
            compute.body.as_slice(),
            [
                Statement::QuestionLet {
                    name: scrutinee_temp,
                    result_expr: ValueExpr::Call { name: scrutinee_call, .. },
                    ..
                },
                Statement::Match {
                    value: ValueExpr::Variable(scrutinee_name),
                    enum_name,
                    arms,
                    ..
                },
            ] if scrutinee_temp.starts_with("__question_value_")
                && scrutinee_call == "maybe_label"
                && scrutinee_name == scrutinee_temp
                && enum_name == "Option"
                && matches!(
                    arms.as_slice(),
                    [
                        MatchStatementArm {
                            variant: some_variant,
                            binding: Some(binding),
                            body: some_body,
                        },
                        MatchStatementArm {
                            variant: none_variant,
                            binding: None,
                            body: none_body,
                        },
                    ] if some_variant == "Some"
                        && binding == "text"
                        && matches!(
                            some_body.as_slice(),
                            [Statement::Return(Some(ValueExpr::EnumVariant {
                                variant,
                                payload: Some(payload),
                                ..
                            }))] if variant == "Ok"
                                && matches!(payload.as_ref(), ValueExpr::EnumPayload { variant, .. } if variant == "Some")
                        )
                        && none_variant == "None"
                        && matches!(
                            none_body.as_slice(),
                            [Statement::QuestionReturnOk {
                                result_expr: ValueExpr::Call { name: branch_call, .. },
                                ..
                            }] if branch_call == "parse_label"
                        )
                )
        ));
    }

    #[test]
    fn accepts_question_in_tail_match_expression_arm() {
        let source = r#"package app.main

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn compute(value: Option<string>) -> Result<string, string> {
    return match value {
        Some(text) => Ok(text)
        None => Ok(parse_label()?)
    }
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let compute = program
            .functions
            .iter()
            .find(|function| function.name == "compute")
            .unwrap();
        assert!(matches!(
            compute.body.as_slice(),
            [Statement::Match {
                value: ValueExpr::Variable(value),
                enum_name,
                arms,
                ..
            }] if value == "value"
                && enum_name == "Option"
                && matches!(
                    arms.as_slice(),
                    [
                        MatchStatementArm {
                            variant: some_variant,
                            binding: Some(binding),
                            body: some_body,
                        },
                        MatchStatementArm {
                            variant: none_variant,
                            binding: None,
                            body: none_body,
                        },
                    ] if some_variant == "Some"
                        && binding == "text"
                        && matches!(
                            some_body.as_slice(),
                            [Statement::Return(Some(ValueExpr::EnumVariant {
                                payload: Some(payload),
                                ..
                            }))] if matches!(
                                payload.as_ref(),
                                ValueExpr::EnumPayload {
                                    variant,
                                    ..
                                } if variant == "Some"
                            )
                        )
                        && none_variant == "None"
                        && matches!(
                            none_body.as_slice(),
                            [Statement::QuestionReturnOk {
                                result_expr: ValueExpr::Call { name: call_name, .. },
                                ..
                            }] if call_name == "parse_label"
                        )
                )
        ));
    }

    #[test]
    fn accepts_question_in_tail_match_scrutinee() {
        let source = r#"package app.main

fn maybe_label() -> Result<Option<string>, string> {
    return Ok(Some("value"))
}

fn compute() -> Result<string, string> {
    return match maybe_label()? {
        Some(text) => Ok(text)
        None => Ok("fallback")
    }
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let compute = program
            .functions
            .iter()
            .find(|function| function.name == "compute")
            .unwrap();
        assert!(matches!(
            compute.body.as_slice(),
            [
                Statement::QuestionLet {
                    name: temp,
                    result_expr: ValueExpr::Call { name: call_name, .. },
                    ..
                },
                Statement::Match {
                    value: ValueExpr::Variable(value),
                    enum_name,
                    ..
                },
            ] if temp.starts_with("__question_value_")
                && call_name == "maybe_label"
                && value == temp
                && enum_name == "Option"
        ));
    }

    #[test]
    fn accepts_question_in_match_initializer_arm() {
        let source = r#"package app.main

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn compute(value: Option<string>) -> Result<string, string> {
    let label: string = match value {
        Some(text) => text
        None => parse_label()?
    }
    return Ok(label)
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let compute = program
            .functions
            .iter()
            .find(|function| function.name == "compute")
            .unwrap();
        assert!(matches!(
            compute.body.as_slice(),
            [
                Statement::LetMatch {
                    name,
                    value_type,
                    value: ValueExpr::Variable(value),
                    enum_name,
                    arms,
                    ..
                },
                Statement::Return(Some(_)),
            ] if name == "label"
                && value_type == &ValueType::String
                && value == "value"
                && enum_name == "Option"
                && matches!(
                    arms.as_slice(),
                    [
                        MatchStatementArm {
                            variant: some_variant,
                            binding: Some(binding),
                            body: some_body,
                        },
                        MatchStatementArm {
                            variant: none_variant,
                            binding: None,
                            body: none_body,
                        },
                    ] if some_variant == "Some"
                        && binding == "text"
                        && matches!(
                            some_body.as_slice(),
                            [Statement::Assign {
                                name: assign_name,
                                value: ValueExpr::EnumPayload {
                                    variant,
                                    ..
                                },
                            }] if assign_name == "label" && variant == "Some"
                        )
                        && none_variant == "None"
                        && matches!(
                            none_body.as_slice(),
                            [
                                Statement::QuestionLet {
                                    name: temp,
                                    result_expr: ValueExpr::Call { name: call_name, .. },
                                    ..
                                },
                                Statement::Assign {
                                    name: assign_name,
                                    value: ValueExpr::Variable(assign_value),
                                },
                            ] if temp.starts_with("__question_value_")
                                && call_name == "parse_label"
                                && assign_name == "label"
                                && assign_value == temp
                        )
                )
        ));
    }

    #[test]
    fn accepts_question_in_match_initializer_scrutinee() {
        let source = r#"package app.main

fn maybe_label() -> Result<Option<string>, string> {
    return Ok(Some("value"))
}

fn compute() -> Result<string, string> {
    let label: string = match maybe_label()? {
        Some(text) => text
        None => "fallback"
    }
    return Ok(label)
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let compute = program
            .functions
            .iter()
            .find(|function| function.name == "compute")
            .unwrap();
        assert!(matches!(
            compute.body.as_slice(),
            [
                Statement::QuestionLet {
                    name: temp,
                    result_expr: ValueExpr::Call { name: call_name, .. },
                    ..
                },
                Statement::LetMatch {
                    name,
                    value: ValueExpr::Variable(value),
                    enum_name,
                    ..
                },
                Statement::Return(Some(_)),
            ] if temp.starts_with("__question_value_")
                && call_name == "maybe_label"
                && name == "label"
                && value == temp
                && enum_name == "Option"
        ));
    }

    #[test]
    fn rejects_result_map_err_without_result_import() {
        let source = r#"package app.main

struct AppError {
    message: string
}

fn parse_label() -> Result<string, string> {
    return Err("bad")
}

fn app_error_from_string(message: string) -> AppError {
    return AppError { message: message }
}

fn main() -> void {
    let raw: Result<string, string> = parse_label()
    let mapped: Result<string, AppError> = raw.map_err(app_error_from_string)
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0301");
        assert!(err.message.contains("std.result"));
    }

    #[test]
    fn accepts_result_question_let_binding() {
        let source = r#"package app.main

import std.io

enum Result<T, E> {
    Ok(T)
    Err(E)
}

fn parse() -> Result<i64, string> {
    return Result.Ok(41)
}

fn compute() -> Result<i64, string> {
    let value: i64 = parse()?
    return Result.Ok(value + 1)
}

fn main() -> void {
    io.println("done")
}
"#;

        let program = parse_inline(source).unwrap();
        let compute = program
            .functions
            .iter()
            .find(|function| function.name == "compute")
            .unwrap();
        assert!(matches!(
            compute.body[0],
            Statement::QuestionLet {
                ref name,
                value_type: ValueType::Int,
                result_type: ValueType::Enum(ref enum_name, ref enum_args),
                return_type: ValueType::Enum(ref return_name, ref return_args),
                ..
            } if name == "value"
                && enum_name == "Result"
                && enum_args == &vec![ValueType::Int, ValueType::String]
                && return_name == "Result"
                && return_args == &vec![ValueType::Int, ValueType::String]
        ));
    }

    #[test]
    fn accepts_option_question_let_binding() {
        let source = r#"package app.main

fn load() -> Option<string> {
    return Some("value")
}

fn compute() -> Option<string> {
    let text: string = load()?
    return Some(text)
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let compute = program
            .functions
            .iter()
            .find(|function| function.name == "compute")
            .unwrap();
        assert!(matches!(
            compute.body[0],
            Statement::QuestionLet {
                carrier: QuestionCarrier::Option,
                ref name,
                value_type: ValueType::String,
                result_type: ValueType::Enum(ref enum_name, ref enum_args),
                return_type: ValueType::Enum(ref return_name, ref return_args),
                ..
            } if name == "text"
                && enum_name == "Option"
                && enum_args == &vec![ValueType::String]
                && return_name == "Option"
                && return_args == &vec![ValueType::String]
        ));
    }

    #[test]
    fn accepts_question_in_result_ok_return_payload() {
        let source = r#"package app.main

fn parse() -> Result<i64, string> {
    return Ok(41)
}

fn compute() -> Result<i64, string> {
    return Ok(parse()?)
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let compute = program
            .functions
            .iter()
            .find(|function| function.name == "compute")
            .unwrap();
        assert!(matches!(
            compute.body[0],
            Statement::QuestionReturnOk {
                ok_type: ValueType::Int,
                result_type: ValueType::Enum(ref result_name, ref result_args),
                return_type: ValueType::Enum(ref return_name, ref return_args),
                result_expr: ValueExpr::Call { ref name, .. },
            } if result_name == "Result"
                && result_args == &vec![ValueType::Int, ValueType::String]
                && return_name == "Result"
                && return_args == &vec![ValueType::Int, ValueType::String]
                && name == "parse"
        ));
    }

    #[test]
    fn question_in_shadowed_ok_call_is_not_treated_as_result_variant() {
        let source = r#"package app.main

fn Ok(value: i64) -> Result<i64, string> {
    return Result.Ok(value)
}

fn parse() -> Result<i64, string> {
    return Result.Ok(41)
}

fn compute() -> Result<i64, string> {
    return Ok(parse()?)
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let compute = program
            .functions
            .iter()
            .find(|function| function.name == "compute")
            .unwrap();
        assert!(matches!(
            compute.body.as_slice(),
            [
                Statement::QuestionLet {
                    name,
                    result_expr: ValueExpr::Call { name: parse_name, .. },
                    ..
                },
                Statement::Return(Some(ValueExpr::Call { name: ok_name, args })),
            ] if name.starts_with("__question_value_")
                && parse_name == "parse"
                && ok_name == "Ok"
                && matches!(args.as_slice(), [ValueExpr::Variable(arg)] if arg == name)
        ));
    }

    #[test]
    fn accepts_result_void_ok() {
        let source = r#"package app.main

import std.io

enum Result<T, E> {
    Ok(T)
    Err(E)
}

fn write() -> Result<void, string> {
    return Result.Ok(void)
}

fn main() -> void {
    io.println("done")
}
"#;

        let program = parse_inline(source).unwrap();
        let write = program
            .functions
            .iter()
            .find(|function| function.name == "write")
            .unwrap();
        assert_eq!(
            write.return_type,
            ValueType::Enum(
                "Result".to_string(),
                vec![ValueType::Void, ValueType::String]
            )
        );
        assert!(matches!(
            write.body[0],
            Statement::Return(Some(ValueExpr::EnumVariant {
                payload: Some(ref payload),
                ..
            })) if payload.as_ref() == &ValueExpr::VoidLiteral
        ));
    }

    #[test]
    fn rejects_question_in_non_result_function() {
        let source = r#"package app.main

import std.io

enum Result<T, E> {
    Ok(T)
    Err(E)
}

fn parse() -> Result<i64, string> {
    return Result.Ok(41)
}

fn main() -> void {
    let value: i64 = parse()?
    io.println("done")
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0421");
    }

    #[test]
    fn rejects_question_let_without_type_annotation() {
        let source = r#"package app.main

import std.io

enum Result<T, E> {
    Ok(T)
    Err(E)
}

fn parse() -> Result<i64, string> {
    return Result.Ok(41)
}

fn compute() -> Result<i64, string> {
    let value = parse()?
    return Result.Ok(value + 1)
}

fn main() -> void {
    io.println("done")
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0403");
    }

    #[test]
    fn rejects_missing_payload_binding_in_match() {
        let source = r#"package app.main

import std.io

enum MaybeInt {
    Some(i64)
    None
}

fn unwrap_or_zero(value: MaybeInt) -> i64 {
    return match value {
        MaybeInt.Some => 1
        MaybeInt.None => 0
    }
}

fn main() -> void {
    io.println("done")
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0321");
    }

    #[test]
    fn rejects_missing_main() {
        let source = "package app.main\nimport std.io\n";
        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0201");
    }

    #[test]
    fn accepts_script_body_as_synthesized_main_in_script_mode() {
        let source = "package app.main\n\nlet value: i32 = 1\n";
        let program = check_script_source_text(Path::new("script.nomo"), source).unwrap();
        let main = program
            .functions
            .iter()
            .find(|function| function.name == "main")
            .unwrap();

        assert!(main.params.is_empty());
        assert_eq!(main.return_type, ValueType::Void);
        assert!(matches!(
            main.body.as_slice(),
            [Statement::Let { name, value_type: ValueType::I32, .. }] if name == "value"
        ));
    }

    #[test]
    fn rejects_top_level_script_body_outside_script_mode() {
        let source = "package app.main\n\nlet value: i32 = 1\n";
        let err = parse_inline(source).unwrap_err();

        assert_eq!(err.code, "E0201");
        assert!(err.message.contains("top-level script statements"));
    }

    #[test]
    fn rejects_script_body_with_explicit_main_in_script_mode() {
        let source = "package app.main\n\nfn main() -> void {\n}\n\nlet value: i32 = 1\n";
        let err = check_script_source_text(Path::new("script.nomo"), source).unwrap_err();

        assert_eq!(err.code, "E0201");
        assert!(err.message.contains("explicit `main`"));
    }

    #[test]
    fn rejects_missing_io_import() {
        let source = r#"package app.main

fn main() -> void {
    io.println("Hello")
}
"#;
        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0301");
        assert_eq!(err.suggestions.len(), 1);
        assert_eq!(err.suggestions[0].text, "import std.io\n");
        assert!(err.suggestions[0].description.contains("io.println"));
    }

    #[test]
    fn rejects_unqualified_println_without_specific_import() {
        let source = r#"package app.main

import std.io

fn main() -> void {
    println("Hello")
}
"#;
        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0301");
        assert!(err.message.contains("std.io.println"));
        assert_eq!(err.suggestions.len(), 1);
        assert_eq!(err.suggestions[0].text, "import std.io.println\n");
        assert!(err.suggestions[0].description.contains("println"));
    }

    #[test]
    fn rejects_unqualified_print_without_specific_import() {
        let source = r#"package app.main

import std.io

fn main() -> void {
    print("Hello")
}
"#;
        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0301");
        assert!(err.message.contains("std.io.print"));
        assert_eq!(err.suggestions.len(), 1);
        assert_eq!(err.suggestions[0].text, "import std.io.print\n");
        assert!(err.suggestions[0].description.contains("print"));
    }

    #[test]
    fn rejects_unqualified_string_len_without_specific_import() {
        let source = r#"package app.main

import std.io
import std.string

fn main() -> void {
    let size: u64 = len("Nomo")
    io.println("done")
}
"#;
        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0305");
        assert!(err.message.contains("len"));
    }

    #[test]
    fn accepts_for_while_iterate_and_infinite() {
        let source = r#"package app.main

import std.array
import std.io

fn main() -> void {
    let mut i: i32 = 0
    for i < 2 {
        i = i + 1
    }
    let mut nums: Array<i32> = Array.new<i32>()
    nums.push(1)
    for n in nums {
        io.println("item")
    }
    for {
        break
    }
}
"#;
        parse_inline(source).unwrap();
    }

    #[test]
    fn accepts_question_in_for_in_iterable() {
        let source = r#"package app.main

import std.array

fn make_items() -> Result<Array<i32>, string> {
    let mut items: Array<i32> = Array.new<i32>()
    items.push(1)
    return Ok(items)
}

fn sum_items() -> Result<i32, string> {
    let mut total: i32 = 0
    for item in make_items()? {
        total = total + item
    }
    return Ok(total)
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let sum_items = program
            .functions
            .iter()
            .find(|function| function.name == "sum_items")
            .unwrap();
        assert!(matches!(
            sum_items.body.as_slice(),
            [
                Statement::Let { name: total_name, .. },
                Statement::QuestionLet {
                    name: temp,
                    result_expr: ValueExpr::Call { name: call_name, .. },
                    ..
                },
                Statement::Loop {
                    kind: LoopKind::Iterate {
                        binding,
                        iterable: ValueExpr::Variable(iterable),
                        ..
                    },
                    ..
                },
                Statement::Return(Some(_)),
            ] if total_name == "total"
                && temp.starts_with("__question_value_")
                && call_name == "make_items"
                && binding == "item"
                && iterable == temp
        ));
    }

    #[test]
    fn accepts_question_in_for_while_condition() {
        let source = r#"package app.main

fn should_continue() -> Result<bool, string> {
    return Ok(true)
}

fn compute() -> Result<void, string> {
    for should_continue()? {
        break
    }
    return Ok(void)
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let compute = program
            .functions
            .iter()
            .find(|function| function.name == "compute")
            .unwrap();
        assert!(matches!(
            compute.body.as_slice(),
            [
                Statement::Loop {
                    kind: LoopKind::Infinite,
                    body,
                },
                Statement::Return(Some(_)),
            ] if matches!(
                body.as_slice(),
                [
                    Statement::QuestionLet {
                        name: temp,
                        result_expr: ValueExpr::Call { name: call_name, .. },
                        ..
                    },
                    Statement::If {
                        condition: ValueExpr::Variable(condition),
                        body: then_body,
                        else_body,
                    },
                ] if temp.starts_with("__question_value_")
                    && call_name == "should_continue"
                    && condition == temp
                    && matches!(then_body.as_slice(), [Statement::Break])
                    && matches!(else_body.as_slice(), [Statement::Break])
            )
        ));
    }

    #[test]
    fn accepts_question_in_assignment_value() {
        let source = r#"package app.main

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn compute() -> Result<string, string> {
    let mut label: string = "initial"
    label = parse_label()?
    return Ok(label)
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let compute = program
            .functions
            .iter()
            .find(|function| function.name == "compute")
            .unwrap();
        assert!(matches!(
            compute.body.as_slice(),
            [
                Statement::Let { name: label_name, .. },
                Statement::QuestionLet {
                    name: temp,
                    result_expr: ValueExpr::Call { name: call_name, .. },
                    ..
                },
                Statement::Assign {
                    name: assign_name,
                    value: ValueExpr::Variable(value_name),
                },
                Statement::Return(Some(_)),
            ] if label_name == "label"
                && temp.starts_with("__question_value_")
                && call_name == "parse_label"
                && assign_name == "label"
                && value_name == temp
        ));
    }

    #[test]
    fn accepts_question_in_field_assignment_value() {
        let source = r#"package app.main

struct Label {
    value: string
}

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn compute() -> Result<string, string> {
    let mut label: Label = Label { value: "initial" }
    label.value = parse_label()?
    return Ok(label.value)
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let compute = program
            .functions
            .iter()
            .find(|function| function.name == "compute")
            .unwrap();
        assert!(matches!(
            compute.body.as_slice(),
            [
                Statement::Let { name: label_name, .. },
                Statement::QuestionLet {
                    name: temp,
                    result_expr: ValueExpr::Call { name: call_name, .. },
                    ..
                },
                Statement::AssignField {
                    base,
                    field,
                    value: ValueExpr::Variable(value_name),
                    ..
                },
                Statement::Return(Some(_)),
            ] if label_name == "label"
                && temp.starts_with("__question_value_")
                && call_name == "parse_label"
                && base == "label"
                && field == "value"
                && value_name == temp
        ));
    }

    #[test]
    fn accepts_question_in_if_assignment_branch() {
        let source = r#"package app.main

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn should_use_label() -> Result<bool, string> {
    return Ok(true)
}

fn compute() -> Result<string, string> {
    let mut label: string = "initial"
    label = if should_use_label()? {
        parse_label()?
    } else {
        "fallback"
    }
    return Ok(label)
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let compute = program
            .functions
            .iter()
            .find(|function| function.name == "compute")
            .unwrap();
        assert!(matches!(
            compute.body.as_slice(),
            [
                Statement::Let { name: label_name, .. },
                Statement::QuestionLet {
                    name: condition_temp,
                    result_expr: ValueExpr::Call { name: condition_call, .. },
                    ..
                },
                Statement::If {
                    condition: ValueExpr::Variable(condition_name),
                    body,
                    else_body,
                },
                Statement::Return(Some(_)),
            ] if label_name == "label"
                && condition_temp.starts_with("__question_value_")
                && condition_call == "should_use_label"
                && condition_name == condition_temp
                && matches!(
                    body.as_slice(),
                    [
                        Statement::QuestionLet {
                            name: branch_temp,
                            result_expr: ValueExpr::Call { name: branch_call, .. },
                            ..
                        },
                        Statement::Assign {
                            name: assign_name,
                            value: ValueExpr::Variable(assign_value),
                        },
                    ] if branch_temp.starts_with("__question_value_")
                        && branch_call == "parse_label"
                        && assign_name == "label"
                        && assign_value == branch_temp
                )
                && matches!(
                    else_body.as_slice(),
                    [Statement::Assign {
                        name: assign_name,
                        value: ValueExpr::StringLiteral(value),
                    }] if assign_name == "label" && value == "fallback"
                )
        ));
    }

    #[test]
    fn accepts_question_in_match_assignment_arm() {
        let source = r#"package app.main

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn maybe_label() -> Result<Option<string>, string> {
    return Ok(None)
}

fn compute() -> Result<string, string> {
    let mut label: string = "initial"
    label = match maybe_label()? {
        Some(text) => text
        None => parse_label()?
    }
    return Ok(label)
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let compute = program
            .functions
            .iter()
            .find(|function| function.name == "compute")
            .unwrap();
        assert!(matches!(
            compute.body.as_slice(),
            [
                Statement::Let { name: label_name, .. },
                Statement::QuestionLet {
                    name: scrutinee_temp,
                    result_expr: ValueExpr::Call { name: scrutinee_call, .. },
                    ..
                },
                Statement::Match {
                    value: ValueExpr::Variable(scrutinee_name),
                    enum_name,
                    arms,
                    ..
                },
                Statement::Return(Some(_)),
            ] if label_name == "label"
                && scrutinee_temp.starts_with("__question_value_")
                && scrutinee_call == "maybe_label"
                && scrutinee_name == scrutinee_temp
                && enum_name == "Option"
                && matches!(
                    arms.as_slice(),
                    [
                        MatchStatementArm {
                            variant: some_variant,
                            binding: Some(binding),
                            body: some_body,
                        },
                        MatchStatementArm {
                            variant: none_variant,
                            binding: None,
                            body: none_body,
                        },
                    ] if some_variant == "Some"
                        && binding == "text"
                        && matches!(
                            some_body.as_slice(),
                            [Statement::Assign {
                                name: assign_name,
                                value: ValueExpr::EnumPayload { variant, .. },
                            }] if assign_name == "label" && variant == "Some"
                        )
                        && none_variant == "None"
                        && matches!(
                            none_body.as_slice(),
                            [
                                Statement::QuestionLet {
                                    name: branch_temp,
                                    result_expr: ValueExpr::Call { name: branch_call, .. },
                                    ..
                                },
                                Statement::Assign {
                                    name: assign_name,
                                    value: ValueExpr::Variable(assign_value),
                                },
                            ] if branch_temp.starts_with("__question_value_")
                                && branch_call == "parse_label"
                                && assign_name == "label"
                                && assign_value == branch_temp
                        )
                )
        ));
    }

    #[test]
    fn accepts_question_in_void_expression_statement_argument() {
        let source = r#"package app.main

import std.array

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn collect() -> Result<void, string> {
    let mut values: Array<string> = Array.new<string>()
    values.push(parse_label()?)
    return Ok(void)
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let collect = program
            .functions
            .iter()
            .find(|function| function.name == "collect")
            .unwrap();
        assert!(matches!(
            collect.body.as_slice(),
            [
                Statement::Let { name: values_name, .. },
                Statement::QuestionLet {
                    name: temp,
                    result_expr: ValueExpr::Call { name: call_name, .. },
                    ..
                },
                Statement::Assign {
                    name: assign_name,
                    value: ValueExpr::ArrayPush { value, .. },
                },
                Statement::Return(Some(_)),
            ] if values_name == "values"
                && temp.starts_with("__question_value_")
                && call_name == "parse_label"
                && assign_name == "values"
                && matches!(value.as_ref(), ValueExpr::Variable(name) if name == temp)
        ));
    }

    #[test]
    fn accepts_question_in_defer_call_argument() {
        let source = r#"package app.main

fn parse_label() -> Result<string, string> {
    return Ok("value")
}

fn consume(value: string) -> void {
}

fn compute() -> Result<void, string> {
    defer consume(parse_label()?)
    return Ok(void)
}

fn main() -> void {
}
"#;

        let program = parse_inline(source).unwrap();
        let compute = program
            .functions
            .iter()
            .find(|function| function.name == "compute")
            .unwrap();
        assert!(matches!(
            compute.body.as_slice(),
            [
                Statement::QuestionLet {
                    name: temp,
                    result_expr: ValueExpr::Call { name: call_name, .. },
                    ..
                },
                Statement::Defer {
                    call: DeferredCall::Expr(ValueExpr::Call { name: consume_name, args }),
                },
                Statement::Return(Some(_)),
            ] if temp.starts_with("__question_value_")
                && call_name == "parse_label"
                && consume_name == "consume"
                && matches!(args.as_slice(), [ValueExpr::Variable(name)] if name == temp)
        ));
    }

    #[test]
    fn accepts_break_and_continue_in_loop() {
        let source = r#"package app.main

fn main() -> void {
    for {
        break
    }
    for {
        continue
    }
}
"#;
        parse_inline(source).unwrap();
    }

    #[test]
    fn accepts_nested_loop_break() {
        let source = r#"package app.main

fn main() -> void {
    for {
        for {
            break
        }
        break
    }
}
"#;
        parse_inline(source).unwrap();
    }

    #[test]
    fn rejects_break_outside_loop() {
        let source = "package app.main\nfn main() -> void {\n    break\n}\n";
        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0510");
    }

    #[test]
    fn rejects_continue_outside_loop() {
        let source = "package app.main\nfn main() -> void {\n    continue\n}\n";
        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0511");
    }

    #[test]
    fn accepts_defer_inside_loop() {
        let source = "package app.main\nimport std.io\nfn main() -> void {\n    for {\n        defer io.println(\"cleanup\")\n        break\n    }\n}\n";
        let program = parse_inline(source).unwrap();
        let Statement::Loop { body, .. } = &program.functions[0].body[0] else {
            panic!("expected loop");
        };
        assert!(matches!(body[0], Statement::Defer { .. }));
        assert!(matches!(body[1], Statement::Break));
    }

    #[test]
    fn rejects_defer_non_expression() {
        let source = "package app.main\nfn main() -> void {\n    defer return\n}\n";
        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0265");
    }

    #[test]
    fn accepts_defer_method_call() {
        let source = r#"package app.main

import std.io

struct R {
    pub id: i32
}

impl R {
    pub fn close(self) -> void {
        io.println("closed")
    }
}

fn main() -> void {
    let r: R = R { id: 1 }
    defer r.close()
    io.println("working")
}
"#;
        parse_inline(source).unwrap();
    }

    #[test]
    fn accepts_defer_io_print_calls() {
        let source = r#"package app.main

import std.io

fn main() -> void {
    defer io.println("cleanup")
    defer io.eprintln("error cleanup")
    io.println("working")
}
"#;
        let program = parse_inline(source).unwrap();
        let main = program
            .functions
            .iter()
            .find(|function| function.name == "main")
            .unwrap();
        assert!(matches!(
            main.body[0],
            Statement::Defer {
                call: DeferredCall::Println(_)
            }
        ));
        assert!(matches!(
            main.body[1],
            Statement::Defer {
                call: DeferredCall::Eprintln(_)
            }
        ));
    }

    #[test]
    fn accepts_defer_specific_print_import() {
        let source = r#"package app.main

import std.io.println

fn main() -> void {
    defer println("cleanup")
    println("working")
}
"#;
        parse_inline(source).unwrap();
    }

    #[test]
    fn rejects_defer_io_print_without_import() {
        let source = r#"package app.main

fn main() -> void {
    defer io.println("cleanup")
}
"#;
        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0301");
    }

    #[test]
    fn accepts_package_const_reference() {
        let source = r#"package app.main

import std.io

const LIMIT: i32 = 5
const NAME: string = "nomo"

fn main() -> void {
    let mut i: i32 = 0
    for i < LIMIT {
        i = i + 1
    }
    io.println(NAME)
}
"#;
        let program = parse_inline(source).unwrap();
        assert_eq!(program.consts.len(), 2);
        assert_eq!(program.consts[0].name, "LIMIT");
        assert_eq!(program.consts[1].name, "NAME");
    }

    #[test]
    fn rejects_const_non_literal_initializer() {
        let source = "package app.main\nfn one() -> i32 {\n    return 1\n}\nconst X: i32 = one()\nfn main() -> void {\n}\n";
        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0430");
    }

    #[test]
    fn rejects_const_duplicate() {
        let source =
            "package app.main\nconst A: i32 = 1\nconst A: i32 = 2\nfn main() -> void {\n}\n";
        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0304");
    }

    #[test]
    fn rejects_for_in_over_non_array() {
        let source = "package app.main\nfn main() -> void {\n    for n in 5 {\n    }\n}\n";
        let err = parse_inline(source).unwrap_err();
        assert!(err.message.contains("Array"));
    }

    #[test]
    fn rejects_for_iter_binding_outside_loop_body() {
        let source = r#"package app.main

import std.array
import std.io

fn main() -> void {
    let mut words: Array<string> = Array.new<string>()
    words.push("hello")
    for word in words {
        io.println(word)
    }
    io.println(word)
}
"#;
        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0303");
        assert!(err.message.contains("word"));
    }

    #[test]
    fn rejects_loop_local_let_outside_loop_body() {
        let source = r#"package app.main

import std.io

fn main() -> void {
    for {
        let message: string = "inside"
        break
    }
    io.println(message)
}
"#;
        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "E0303");
        assert!(err.message.contains("message"));
    }

    #[test]
    fn rejects_for_condition_must_be_bool() {
        let source = "package app.main\nfn main() -> void {\n    for 5 {\n    }\n}\n";
        let err = parse_inline(source).unwrap_err();
        assert!(err.message.contains("bool"));
    }
}

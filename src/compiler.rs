use crate::ast::{
    BinaryOp as AstBinaryOp, EnumDef as AstEnumDef, Expr as AstExpr, Function as AstFunction,
    SourceFile, Span, Stmt, StructDef as AstStructDef,
};
use crate::codegen;
use crate::diagnostic::Diagnostic;
use crate::lexer;
use crate::parser;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub package: String,
    pub imports: Vec<String>,
    pub structs: Vec<StructType>,
    pub enums: Vec<EnumType>,
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructType {
    pub name: String,
    pub fields: Vec<StructField>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructField {
    pub name: String,
    pub value_type: ValueType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumType {
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
    TryLet {
        name: String,
        value_type: ValueType,
        result_type: ValueType,
        return_type: ValueType,
        result_expr: ValueExpr,
    },
    Assign {
        name: String,
        value: ValueExpr,
    },
    Println(ValueExpr),
    Eprintln(ValueExpr),
    Panic(ValueExpr),
    Return(Option<ValueExpr>),
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
    Struct(String),
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
    },
    Cast {
        expr: Box<ValueExpr>,
        target_type: ValueType,
    },
    Call {
        name: String,
        args: Vec<ValueExpr>,
    },
    StringLen {
        value: Box<ValueExpr>,
    },
    StringConcat {
        left: Box<ValueExpr>,
        right: Box<ValueExpr>,
    },
    FsReadToString {
        path: Box<ValueExpr>,
    },
    FsWriteString {
        path: Box<ValueExpr>,
        content: Box<ValueExpr>,
    },
    EnvGet {
        name: Box<ValueExpr>,
    },
    EnvArgs,
    ArrayNew {
        element_type: ValueType,
    },
    ArrayLen {
        array: Box<ValueExpr>,
    },
    ArrayGet {
        array: Box<ValueExpr>,
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
    StructLiteral {
        type_name: String,
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
    Add,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

#[derive(Debug, Clone)]
struct FunctionSignature {
    params: Vec<ValueType>,
    return_type: ValueType,
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
    EnumPayload { value: ValueExpr, variant: String },
}

pub fn check_source(path: &Path) -> Result<Program, Diagnostic> {
    let source = fs::read_to_string(path).map_err(|err| {
        Diagnostic::new(
            "N0001",
            format!("failed to read source file: {err}"),
            path,
            1,
            1,
            1,
            "",
        )
    })?;
    check_source_text(path, &source)
}

pub fn check_source_text(path: &Path, source: &str) -> Result<Program, Diagnostic> {
    let tokens = lexer::lex(path, source)?;
    let ast = parser::parse(path, &tokens)?;
    lower_program(path, ast)
}

pub fn compile_source_to_c(path: &Path) -> Result<String, Diagnostic> {
    let program = check_source(path)?;
    Ok(codegen::emit_c(&program))
}

fn lower_program(path: &Path, ast: SourceFile) -> Result<Program, Diagnostic> {
    let imports = ast
        .imports
        .iter()
        .map(|path| path.join("."))
        .collect::<Vec<_>>();
    let mut structs = lower_structs(path, &ast.structs)?;
    let mut enums = lower_enums(path, &ast.enums)?;
    inject_standard_types(
        StandardTypeNeeds {
            fs: imports.iter().any(|item| item == "std.fs") || source_uses_fs_builtin(&ast),
            env: imports.iter().any(|item| item == "std.env") || source_uses_env_builtin(&ast),
            result: imports
                .iter()
                .any(|item| item == "std.result" || item == "std.result.Result"),
            option: imports
                .iter()
                .any(|item| item == "std.option" || item == "std.option.Option"),
            array: imports
                .iter()
                .any(|item| item == "std.array" || item == "std.array.Array")
                || source_uses_array_builtin(&ast),
        },
        &mut structs,
        &mut enums,
    );
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
                "N0304",
                format!("function `{}` is already defined", function.name),
                path,
                1,
                1,
                1,
                "",
            ));
        }
        signatures.insert(
            function.name.clone(),
            function_signature(path, function, &struct_map, &enum_map)?,
        );
    }
    for impl_block in &ast.impls {
        let owner =
            parse_value_type(&impl_block.type_name, &struct_map, &enum_map).ok_or_else(|| {
                Diagnostic::new(
                    "N0309",
                    format!(
                        "unknown impl target `{}`",
                        impl_block.type_name.path.join(".")
                    ),
                    path,
                    1,
                    1,
                    1,
                    "",
                )
            })?;
        let ValueType::Struct(owner_name) = owner else {
            return Err(Diagnostic::new(
                "N0255",
                "v0.1 impl blocks can only target structs",
                path,
                1,
                1,
                1,
                "",
            ));
        };
        for method in &impl_block.methods {
            validate_method_self(path, method, &owner_name, &struct_map, &enum_map)?;
            let lowered_name = method_internal_name(&owner_name, &method.name);
            if signatures.contains_key(&lowered_name) {
                return Err(Diagnostic::new(
                    "N0304",
                    format!("method `{owner_name}.{}` is already defined", method.name),
                    path,
                    1,
                    1,
                    1,
                    "",
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
            "N0201",
            "expected `fn main() -> void { ... }`",
            path,
            1,
            1,
            1,
            "",
        ));
    };
    if !main_signature.params.is_empty() || main_signature.return_type != ValueType::Void {
        return Err(Diagnostic::new(
            "N0401",
            "stage-0 `main` must be `fn main() -> void`",
            path,
            1,
            1,
            1,
            "",
        ));
    }

    let mut functions = Vec::new();
    for function in &ast.functions {
        functions.push(lower_function_as(
            path,
            function,
            &function.name,
            &imports,
            &signatures,
            &struct_map,
            &enum_map,
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
            )?);
        }
    }

    Ok(Program {
        package: ast.package.join("."),
        imports,
        structs,
        enums,
        functions,
    })
}

fn lower_structs(path: &Path, structs: &[AstStructDef]) -> Result<Vec<StructType>, Diagnostic> {
    let mut lowered = Vec::new();
    let mut known = HashMap::new();
    for item in structs {
        if known.contains_key(&item.name) {
            return Err(Diagnostic::new(
                "N0306",
                format!("struct `{}` is already defined", item.name),
                path,
                1,
                1,
                1,
                "",
            ));
        }
        known.insert(item.name.clone(), ());
    }

    for item in structs {
        let mut fields = Vec::new();
        let mut field_names = HashMap::new();
        for field in &item.fields {
            if field_names.contains_key(&field.name) {
                return Err(Diagnostic::new(
                    "N0307",
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
                &known.keys().cloned().collect::<Vec<_>>(),
                &[],
                &[],
            )
            .ok_or_else(|| {
                Diagnostic::new(
                    "N0403",
                    format!(
                        "unsupported field type `{}` in v0.1 stage-0 subset",
                        field.type_ref.path.join(".")
                    ),
                    path,
                    1,
                    1,
                    1,
                    "",
                )
            })?;
            if value_type == ValueType::Void {
                return Err(Diagnostic::new(
                    "N0403",
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
            name: item.name.clone(),
            fields,
        });
    }

    Ok(lowered)
}

fn lower_enums(path: &Path, enums: &[AstEnumDef]) -> Result<Vec<EnumType>, Diagnostic> {
    let mut lowered = Vec::new();
    let mut known = HashMap::new();
    for item in enums {
        if known.contains_key(&item.name) {
            return Err(Diagnostic::new(
                "N0313",
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
                    "N0314",
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
                let known_structs = Vec::new();
                let known_enums = enums
                    .iter()
                    .map(|item| item.name.clone())
                    .collect::<Vec<_>>();
                let payload_type = parse_value_type_with_names(
                    type_ref,
                    &known_structs,
                    &known_enums,
                    &item.type_params,
                )
                .ok_or_else(|| {
                    Diagnostic::new(
                        "N0403",
                        format!(
                            "unsupported enum payload type `{}` in v0.1 stage-0 subset",
                            type_ref.path.join(".")
                        ),
                        path,
                        1,
                        1,
                        1,
                        "",
                    )
                })?;
                if payload_type == ValueType::Void {
                    return Err(Diagnostic::new(
                        "N0403",
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
            name: item.name.clone(),
            type_params: item.type_params.clone(),
            variants,
        });
    }
    Ok(lowered)
}

#[derive(Debug, Clone, Copy)]
struct StandardTypeNeeds {
    fs: bool,
    env: bool,
    result: bool,
    option: bool,
    array: bool,
}

fn inject_standard_types(
    needs: StandardTypeNeeds,
    structs: &mut Vec<StructType>,
    enums: &mut Vec<EnumType>,
) {
    if needs.fs && !structs.iter().any(|item| item.name == "FsError") {
        structs.push(StructType {
            name: "FsError".to_string(),
            fields: vec![StructField {
                name: "message".to_string(),
                value_type: ValueType::String,
            }],
        });
    }
    if (needs.fs || needs.result) && !enums.iter().any(|item| item.name == "Result") {
        enums.push(EnumType {
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
    if (needs.env || needs.option || needs.array) && !enums.iter().any(|item| item.name == "Option")
    {
        enums.push(EnumType {
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

fn source_uses_env_builtin(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_env_builtin)
}

fn source_uses_array_builtin(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_array_builtin)
}

fn ast_functions(ast: &SourceFile) -> impl Iterator<Item = &AstFunction> {
    ast.functions
        .iter()
        .chain(ast.impls.iter().flat_map(|item| item.methods.iter()))
}

fn stmt_uses_fs_builtin(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => expr_uses_fs_builtin(value),
        Stmt::Return { value, .. } => value.as_ref().is_some_and(expr_uses_fs_builtin),
        Stmt::Expr { expr, .. } => expr_uses_fs_builtin(expr),
    }
}

fn stmt_uses_env_builtin(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => expr_uses_env_builtin(value),
        Stmt::Return { value, .. } => value.as_ref().is_some_and(expr_uses_env_builtin),
        Stmt::Expr { expr, .. } => expr_uses_env_builtin(expr),
    }
}

fn stmt_uses_array_builtin(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => expr_uses_array_builtin(value),
        Stmt::Return { value, .. } => value.as_ref().is_some_and(expr_uses_array_builtin),
        Stmt::Expr { expr, .. } => expr_uses_array_builtin(expr),
    }
}

fn expr_uses_fs_builtin(expr: &AstExpr) -> bool {
    match expr {
        AstExpr::Call { callee, args, .. } => {
            (callee == &["fs", "read_to_string"] || callee == &["fs", "write_string"])
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
        AstExpr::Panic { message } | AstExpr::Try { expr: message } => {
            expr_uses_fs_builtin(message)
        }
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

fn expr_uses_env_builtin(expr: &AstExpr) -> bool {
    match expr {
        AstExpr::Call { callee, args, .. } => {
            callee == &["env", "get"]
                || callee == &["env", "args"]
                || args.iter().any(expr_uses_env_builtin)
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
        AstExpr::Panic { message } | AstExpr::Try { expr: message } => {
            expr_uses_env_builtin(message)
        }
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

fn expr_uses_array_builtin(expr: &AstExpr) -> bool {
    match expr {
        AstExpr::Call { callee, args, .. } => {
            callee == &["Array", "new"]
                || (callee.len() == 2
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
        AstExpr::Panic { message } | AstExpr::Try { expr: message } => {
            expr_uses_array_builtin(message)
        }
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

fn function_signature(
    path: &Path,
    function: &AstFunction,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Result<FunctionSignature, Diagnostic> {
    let params = function
        .params
        .iter()
        .map(|param| parse_value_type(&param.type_ref, structs, enums))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| {
            Diagnostic::new(
                "N0403",
                "unsupported parameter type in v0.1 stage-0 subset",
                path,
                1,
                1,
                1,
                "",
            )
        })?;
    let return_type = parse_value_type(&function.return_type, structs, enums).ok_or_else(|| {
        Diagnostic::new(
            "N0403",
            format!(
                "unsupported return type `{}` in v0.1 stage-0 subset",
                function.return_type.path.join(".")
            ),
            path,
            1,
            1,
            1,
            "",
        )
    })?;
    Ok(FunctionSignature {
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
) -> Result<Function, Diagnostic> {
    let signature = signatures
        .get(lowered_name)
        .expect("signature table is built before lowering");
    let mut scope = HashMap::new();
    let mut params = Vec::new();
    for (param, value_type) in function.params.iter().zip(signature.params.iter()) {
        if scope.contains_key(&param.name) {
            return Err(Diagnostic::new(
                "N0302",
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
                value_type: value_type.clone(),
                mutable: param.mutable,
                source: BindingSource::Local,
            },
        );
        params.push(Parameter {
            name: param.name.clone(),
            mutable: param.mutable,
            value_type: value_type.clone(),
        });
    }

    let mut body = Vec::new();
    for (index, stmt) in function.body.iter().enumerate() {
        let is_tail = index + 1 == function.body.len();
        body.push(lower_stmt(
            path,
            stmt,
            &mut scope,
            imports,
            signatures,
            structs,
            enums,
            &signature.return_type,
            is_tail,
        )?);
    }

    if signature.return_type != ValueType::Void
        && !matches!(body.last(), Some(Statement::Return(Some(_))))
    {
        return Err(Diagnostic::new(
            "N0406",
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
            "N0256",
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
            "N0256",
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
    let Some(ValueType::Struct(self_type)) = parse_value_type(&self_param.type_ref, structs, enums)
    else {
        return Err(Diagnostic::new(
            "N0257",
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
    if self_type != owner_name {
        return Err(Diagnostic::new(
            "N0257",
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
                    "N0302",
                    format!("variable `{name}` is already defined in this scope"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }

            if let AstExpr::Try { expr } = value {
                let annotated_type = type_annotation
                    .as_ref()
                    .and_then(|annotation| parse_non_void_type(annotation, structs, enums))
                    .ok_or_else(|| {
                        Diagnostic::new(
                            "N0403",
                            "`?` let bindings require an explicit non-void type annotation",
                            path,
                            span.line,
                            span.column,
                            span.length,
                            &span.text,
                        )
                    })?;
                let (result_type, result_expr) =
                    lower_value_expr(path, expr, scope, signatures, structs, enums, span)?;
                let (ok_type, err_type) = result_parts(&result_type).ok_or_else(|| {
                    Diagnostic::new(
                        "N0420",
                        "`?` can only be used with `Result<T, E>`",
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    )
                })?;
                let (_, return_err_type) = result_parts(return_type).ok_or_else(|| {
                    Diagnostic::new(
                        "N0421",
                        "`?` requires the current function to return `Result<T, E>`",
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    )
                })?;
                if ok_type != annotated_type {
                    return Err(type_mismatch(
                        path,
                        span,
                        format!(
                            "`?` unwraps `{}` but binding `{name}` is annotated as `{}`",
                            ok_type.name(),
                            annotated_type.name()
                        ),
                    ));
                }
                if err_type != return_err_type {
                    return Err(type_mismatch(
                        path,
                        span,
                        format!(
                            "`?` error type is `{}` but function returns `{}`",
                            err_type.name(),
                            return_err_type.name()
                        ),
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
                return Ok(Statement::TryLet {
                    name: name.clone(),
                    value_type: annotated_type,
                    result_type,
                    return_type: return_type.clone(),
                    result_expr,
                });
            }

            let annotated_type = if let Some(annotation) = type_annotation {
                Some(
                    parse_non_void_type(annotation, structs, enums).ok_or_else(|| {
                        Diagnostic::new(
                            "N0403",
                            format!(
                                "unsupported variable type `{}` in v0.1 stage-0 subset",
                                annotation.path.join(".")
                            ),
                            path,
                            span.line,
                            span.column,
                            span.length,
                            &span.text,
                        )
                    })?,
                )
            } else {
                None
            };
            let (inferred_type, initializer) = lower_value_expr_with_expected(
                path,
                value,
                scope,
                signatures,
                structs,
                enums,
                annotated_type.as_ref(),
                span,
            )?;
            let value_type = if let Some(annotated_type) = annotated_type {
                if annotated_type != inferred_type {
                    return Err(type_mismatch(
                        path,
                        span,
                        format!(
                            "cannot initialize `{name}` as `{}` from `{}`",
                            annotated_type.name(),
                            inferred_type.name()
                        ),
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
        Stmt::Assign { name, value, span } => {
            let Some(binding) = scope.get(name) else {
                return Err(Diagnostic::new(
                    "N0303",
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
                    "N0410",
                    format!("cannot assign to immutable variable `{name}`"),
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
                signatures,
                structs,
                enums,
                Some(&expected_type),
                span,
            )?;
            if actual_type != expected_type {
                return Err(type_mismatch(
                    path,
                    span,
                    format!(
                        "cannot assign `{}` to variable `{name}` of type `{}`",
                        actual_type.name(),
                        expected_type.name()
                    ),
                ));
            }
            Ok(Statement::Assign {
                name: name.clone(),
                value,
            })
        }
        Stmt::Return { value, span } => lower_return_stmt(
            path,
            value.as_ref(),
            scope,
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
        } if callee == &["io", "println"] || callee == &["io", "eprintln"] => {
            let function_name = callee[1].as_str();
            if !imports.iter().any(|item| item == "std.io") {
                return Err(Diagnostic::new(
                    "N0301",
                    format!("stage-0 subset requires `import std.io` for `io.{function_name}`"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            let [arg] = args.as_slice() else {
                return Err(println_type_error(path, span, function_name));
            };
            let (arg_type, lowered) =
                lower_value_expr(path, arg, scope, signatures, structs, enums, span)?;
            if arg_type != ValueType::String {
                return Err(println_type_error(path, span, function_name));
            }
            if function_name == "eprintln" {
                Ok(Statement::Eprintln(lowered))
            } else {
                Ok(Statement::Println(lowered))
            }
        }
        Stmt::Expr {
            expr: AstExpr::Panic { message },
            span,
        } => {
            let lowered =
                lower_panic_message(path, message, scope, signatures, structs, enums, span)?;
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
            && (callee[1] == "push" || callee[1] == "set")
            && type_args.is_empty() =>
        {
            let lowered =
                lower_array_mutation(path, callee, args, scope, signatures, structs, enums, span)?;
            Ok(Statement::Assign {
                name: callee[0].clone(),
                value: lowered,
            })
        }
        Stmt::Expr { span, .. } => Err(Diagnostic::new(
            "N0203",
            "unsupported statement in v0.1 stage-0 subset; expected `let ... = ...`, `return ...`, `panic(...)`, `io.println(...)`, or `io.eprintln(...)`",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        )),
    }
}

fn lower_return_stmt(
    path: &Path,
    value: Option<&AstExpr>,
    scope: &HashMap<String, Binding>,
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
            let (actual, lowered) = lower_value_expr_with_expected(
                path,
                value,
                scope,
                signatures,
                structs,
                enums,
                Some(expected),
                span,
            )?;
            if &actual != expected {
                return Err(type_mismatch(
                    path,
                    span,
                    format!(
                        "return value is `{}` but function expects `{}`",
                        actual.name(),
                        expected.name()
                    ),
                ));
            }
            Ok(Statement::Return(Some(lowered)))
        }
    }
}

fn lower_value_expr(
    path: &Path,
    expr: &AstExpr,
    scope: &HashMap<String, Binding>,
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    lower_value_expr_with_expected(path, expr, scope, signatures, structs, enums, None, span)
}

fn lower_value_expr_with_expected(
    path: &Path,
    expr: &AstExpr,
    scope: &HashMap<String, Binding>,
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
        AstExpr::Name(name) if name.len() == 1 => {
            let name = &name[0];
            let Some(binding) = scope.get(name) else {
                return Err(Diagnostic::new(
                    "N0303",
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
                        "N0315",
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
                        "N0320",
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
                            "N0324",
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
                    "N0303",
                    format!("unknown variable `{base}`"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let ValueType::Struct(type_name) = &binding.value_type else {
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
                .map(|item| item.value_type.clone())
            else {
                return Err(Diagnostic::new(
                    "N0308",
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
                BindingSource::Local => ValueExpr::FieldAccess {
                    base: base.clone(),
                    field: field.clone(),
                },
            };
            Ok((field_type, value))
        }
        AstExpr::Match { value, arms } => {
            let (value_type, lowered_value) =
                lower_value_expr(path, value, scope, signatures, structs, enums, span)?;
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
                if arm.pattern.len() != 2 || arm.pattern[0] != enum_name {
                    return Err(Diagnostic::new(
                        "N0316",
                        format!("match arm must use `{enum_name}.Variant`"),
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
                let variant = &arm.pattern[1];
                let Some(variant_type) =
                    enum_type.variants.iter().find(|item| item.name == *variant)
                else {
                    return Err(Diagnostic::new(
                        "N0315",
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
                            "N0321",
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
                            "N0322",
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
                        "N0317",
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
                    variant: variant.clone(),
                    binding: arm.binding.clone(),
                    value: arm_value,
                });
            }
            for variant in &enum_type.variants {
                if !seen.contains_key(&variant.name) {
                    return Err(Diagnostic::new(
                        "N0318",
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
                    "N0319",
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
            let (condition_type, lowered_condition) =
                lower_value_expr(path, condition, scope, signatures, structs, enums, span)?;
            if condition_type != ValueType::Bool {
                return Err(type_mismatch(path, span, "`if` condition must be `bool`"));
            }

            let (then_type, mut lowered_then) = lower_value_expr_with_expected(
                path,
                then_branch,
                scope,
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
            let message =
                lower_panic_message(path, message, scope, signatures, structs, enums, span)?;
            let fallback_type = expected.cloned().unwrap_or(ValueType::Never);
            Ok((
                fallback_type.clone(),
                ValueExpr::Panic {
                    message: Box::new(message),
                    fallback_type,
                },
            ))
        }
        AstExpr::Try { .. } => Err(Diagnostic::new(
            "N0422",
            "`?` is currently supported only in `let name: T = expr?` initializers",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        )),
        AstExpr::Cast { expr, target } => {
            let Some(target_type) = parse_value_type(target, structs, enums) else {
                return Err(Diagnostic::new(
                    "N0403",
                    "unknown cast target type",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (source_type, lowered) =
                lower_value_expr(path, expr, scope, signatures, structs, enums, span)?;
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
        AstExpr::Binary { left, op, right } => {
            let ((left_type, left), (right_type, right)) =
                lower_binary_operands(path, left, right, scope, signatures, structs, enums, span)?;
            let lowered_op = match op {
                AstBinaryOp::Add => BinaryOp::Add,
                AstBinaryOp::Equal => BinaryOp::Equal,
                AstBinaryOp::NotEqual => BinaryOp::NotEqual,
                AstBinaryOp::Less => BinaryOp::Less,
                AstBinaryOp::LessEqual => BinaryOp::LessEqual,
                AstBinaryOp::Greater => BinaryOp::Greater,
                AstBinaryOp::GreaterEqual => BinaryOp::GreaterEqual,
            };
            let value_type = match (lowered_op, &left_type, &right_type) {
                (BinaryOp::Add, ValueType::Int, ValueType::Int) => ValueType::Int,
                (BinaryOp::Add, ValueType::I32, ValueType::I32) => ValueType::I32,
                (BinaryOp::Add, ValueType::U32, ValueType::U32) => ValueType::U32,
                (BinaryOp::Add, ValueType::U64, ValueType::U64) => ValueType::U64,
                (BinaryOp::Add, ValueType::Float, ValueType::Float) => ValueType::Float,
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
                    return Err(type_mismatch(
                        path,
                        span,
                        format!(
                            "`{}` expects two matching numeric operands",
                            ast_binary_symbol(op)
                        ),
                    ));
                }
            };
            Ok((
                value_type,
                ValueExpr::Binary {
                    left: Box::new(left),
                    op: lowered_op,
                    right: Box::new(right),
                },
            ))
        }
        AstExpr::Call {
            callee,
            args,
            type_args,
        } if callee.len() == 1 && type_args.is_empty() => {
            let name = &callee[0];
            let Some(signature) = signatures.get(name) else {
                return Err(Diagnostic::new(
                    "N0305",
                    format!("unknown function `{name}`"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            if signature.return_type == ValueType::Void {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("function `{name}` returns `void` and cannot be used as a value"),
                ));
            }
            if args.len() != signature.params.len() {
                return Err(Diagnostic::new(
                    "N0407",
                    format!(
                        "function `{name}` expects {} argument(s), got {}",
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
            for (index, (arg, expected)) in args.iter().zip(signature.params.iter()).enumerate() {
                let (actual, lowered) = lower_value_expr_with_expected(
                    path,
                    arg,
                    scope,
                    signatures,
                    structs,
                    enums,
                    Some(expected),
                    span,
                )?;
                if &actual != expected {
                    return Err(type_mismatch(
                        path,
                        span,
                        format!(
                            "argument {} to `{name}` is `{}` but expected `{}`",
                            index + 1,
                            actual.name(),
                            expected.name()
                        ),
                    ));
                }
                lowered_args.push(lowered);
            }

            Ok((
                signature.return_type.clone(),
                ValueExpr::Call {
                    name: name.clone(),
                    args: lowered_args,
                },
            ))
        }
        AstExpr::Call {
            callee,
            args,
            type_args,
        } if callee.len() == 2 => {
            if callee == &["Array", "new"] {
                return lower_array_new(path, type_args, args, structs, enums, span);
            }
            if callee == &["string", "len"] || callee == &["string", "concat"] {
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "string builtins do not accept type arguments",
                    ));
                }
                return lower_string_builtin(
                    path, callee, args, scope, signatures, structs, enums, span,
                );
            }
            if callee == &["fs", "read_to_string"] || callee == &["fs", "write_string"] {
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "fs builtins do not accept type arguments",
                    ));
                }
                return lower_fs_builtin(
                    path, callee, args, scope, signatures, structs, enums, span,
                );
            }
            if callee == &["env", "get"] || callee == &["env", "args"] {
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "env builtins do not accept type arguments",
                    ));
                }
                return lower_env_builtin(
                    path, callee, args, scope, signatures, structs, enums, span,
                );
            }
            if type_args.is_empty() && is_array_value_method(callee, scope) {
                return lower_array_value_method(
                    path, callee, args, scope, signatures, structs, enums, span,
                );
            }
            if type_args.is_empty() {
                if let Some(lowered) = lower_struct_value_method(
                    path, callee, args, scope, signatures, structs, enums, span,
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
                    "N0305",
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
                    "N0315",
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
                    "N0323",
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
                    "N0407",
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
                        "N0324",
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
                    "N0309",
                    format!("unknown struct `{type_name}`"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let mut seen = HashMap::new();
            for (field_name, _) in fields {
                if seen.insert(field_name.clone(), ()).is_some() {
                    return Err(Diagnostic::new(
                        "N0311",
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
                let Some((_, value)) = fields
                    .iter()
                    .find(|(field_name, _)| field_name == &expected_field.name)
                else {
                    return Err(Diagnostic::new(
                        "N0310",
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
                    signatures,
                    structs,
                    enums,
                    Some(&expected_field.value_type),
                    span,
                )?;
                if actual_type != expected_field.value_type {
                    return Err(type_mismatch(
                        path,
                        span,
                        format!(
                            "field `{}` is `{}` but expected `{}`",
                            expected_field.name,
                            actual_type.name(),
                            expected_field.value_type.name()
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
                        "N0312",
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
                ValueType::Struct(type_name.clone()),
                ValueExpr::StructLiteral {
                    type_name: type_name.clone(),
                    fields: lowered_fields,
                },
            ))
        }
        _ => Err(Diagnostic::new(
            "N0405",
            "expression is not supported as a value in v0.1 stage-0 subset",
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
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<ValueExpr, Diagnostic> {
    let (message_type, lowered) =
        lower_value_expr(path, message, scope, signatures, structs, enums, span)?;
    if message_type != ValueType::String {
        return Err(type_mismatch(
            path,
            span,
            "`panic` expects a string message",
        ));
    }
    Ok(lowered)
}

fn lower_string_builtin(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    match callee {
        [module, name] if module == "string" && name == "len" => {
            let [arg] = args else {
                return Err(Diagnostic::new(
                    "N0407",
                    "`string.len` expects exactly one string argument",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (arg_type, lowered) =
                lower_value_expr(path, arg, scope, signatures, structs, enums, span)?;
            if arg_type != ValueType::String {
                return Err(type_mismatch(path, span, "`string.len` expects a string"));
            }
            Ok((
                ValueType::U64,
                ValueExpr::StringLen {
                    value: Box::new(lowered),
                },
            ))
        }
        [module, name] if module == "string" && name == "concat" => {
            let [left, right] = args else {
                return Err(Diagnostic::new(
                    "N0407",
                    "`string.concat` expects exactly two string arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (left_type, lowered_left) =
                lower_value_expr(path, left, scope, signatures, structs, enums, span)?;
            let (right_type, lowered_right) =
                lower_value_expr(path, right, scope, signatures, structs, enums, span)?;
            if left_type != ValueType::String || right_type != ValueType::String {
                return Err(type_mismatch(
                    path,
                    span,
                    "`string.concat` expects two strings",
                ));
            }
            Ok((
                ValueType::String,
                ValueExpr::StringConcat {
                    left: Box::new(lowered_left),
                    right: Box::new(lowered_right),
                },
            ))
        }
        _ => unreachable!("string builtin dispatcher only passes known calls"),
    }
}

fn lower_fs_builtin(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let fs_error = ValueType::Struct("FsError".to_string());
    match callee {
        [module, name] if module == "fs" && name == "read_to_string" => {
            let [path_arg] = args else {
                return Err(Diagnostic::new(
                    "N0407",
                    "`fs.read_to_string` expects exactly one path string",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (path_type, lowered_path) =
                lower_value_expr(path, path_arg, scope, signatures, structs, enums, span)?;
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
                    "N0407",
                    "`fs.write_string` expects path and content strings",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (path_type, lowered_path) =
                lower_value_expr(path, path_arg, scope, signatures, structs, enums, span)?;
            let (content_type, lowered_content) =
                lower_value_expr(path, content_arg, scope, signatures, structs, enums, span)?;
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
        _ => unreachable!("fs builtin dispatcher only passes known calls"),
    }
}

fn lower_env_builtin(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    match callee {
        [module, name] if module == "env" && name == "get" => {
            let [name_arg] = args else {
                return Err(Diagnostic::new(
                    "N0407",
                    "`env.get` expects exactly one environment variable name",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (name_type, lowered_name) =
                lower_value_expr(path, name_arg, scope, signatures, structs, enums, span)?;
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
        [module, name] if module == "env" && name == "args" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "N0407",
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
            "N0407",
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
            "N0407",
            "`Array.new<T>()` does not accept value arguments",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    let element_type = parse_value_type(type_arg, structs, enums).ok_or_else(|| {
        Diagnostic::new(
            "N0403",
            "unsupported Array element type",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
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
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let name = &callee[0];
    let method = &callee[1];
    let binding = scope.get(name).expect("array method receiver is in scope");
    let ValueType::Array(element_type) = &binding.value_type else {
        unreachable!("array method dispatcher only passes arrays");
    };
    ensure_supported_array_element(path, element_type, span)?;
    match method.as_str() {
        "len" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "N0407",
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
                    array: Box::new(ValueExpr::Variable(name.clone())),
                },
            ))
        }
        "get" => {
            let [index] = args else {
                return Err(Diagnostic::new(
                    "N0407",
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
                    array: Box::new(ValueExpr::Variable(name.clone())),
                    index: Box::new(lowered_index),
                    element_type: element_type.as_ref().clone(),
                },
            ))
        }
        _ => Err(Diagnostic::new(
            "N0305",
            format!("unknown Array method `{method}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        )),
    }
}

fn lower_struct_value_method(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<Option<(ValueType, ValueExpr)>, Diagnostic> {
    if callee.len() != 2 {
        return Ok(None);
    }
    let receiver_name = &callee[0];
    let method_name = &callee[1];
    let Some(binding) = scope.get(receiver_name) else {
        return Ok(None);
    };
    let ValueType::Struct(owner_name) = &binding.value_type else {
        return Ok(None);
    };
    let lowered_name = method_internal_name(owner_name, method_name);
    let Some(signature) = signatures.get(&lowered_name) else {
        return Err(Diagnostic::new(
            "N0314",
            format!("struct `{owner_name}` has no method `{method_name}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    if signature.return_type == ValueType::Void {
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
            "N0407",
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
    if signature.params.first() != Some(&binding.value_type) {
        return Err(Diagnostic::new(
            "N0257",
            format!("method `{owner_name}.{method_name}` has invalid receiver type"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }

    let mut lowered_args = vec![ValueExpr::Variable(receiver_name.clone())];
    for (index, (arg, expected)) in args.iter().zip(signature.params.iter().skip(1)).enumerate() {
        let (actual, lowered) = lower_value_expr_with_expected(
            path,
            arg,
            scope,
            signatures,
            structs,
            enums,
            Some(expected),
            span,
        )?;
        if &actual != expected {
            return Err(type_mismatch(
                path,
                span,
                format!(
                    "argument {} to `{owner_name}.{method_name}` is `{}` but expected `{}`",
                    index + 1,
                    actual.name(),
                    expected.name()
                ),
            ));
        }
        lowered_args.push(lowered);
    }

    Ok(Some((
        signature.return_type.clone(),
        ValueExpr::Call {
            name: lowered_name,
            args: lowered_args,
        },
    )))
}

fn lower_array_mutation(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<ValueExpr, Diagnostic> {
    let name = &callee[0];
    let method = &callee[1];
    let Some(binding) = scope.get(name) else {
        return Err(Diagnostic::new(
            "N0303",
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
            "N0410",
            format!("cannot call mutating Array method on immutable variable `{name}`"),
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
                    "N0407",
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
                    "N0407",
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
        _ => Err(Diagnostic::new(
            "N0305",
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
    if element_type == &ValueType::String {
        Ok(())
    } else {
        Err(type_mismatch(
            path,
            span,
            format!(
                "stage-0 Array currently supports `string` elements, got `{}`",
                element_type.name()
            ),
        ))
    }
}

type LoweredValue = (ValueType, ValueExpr);

fn lower_binary_operands(
    path: &Path,
    left: &AstExpr,
    right: &AstExpr,
    scope: &HashMap<String, Binding>,
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<(LoweredValue, LoweredValue), Diagnostic> {
    let left_default = lower_value_expr(path, left, scope, signatures, structs, enums, span)?;
    let right_with_left = lower_value_expr_with_expected(
        path,
        right,
        scope,
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
        let right_default = lower_value_expr(path, right, scope, signatures, structs, enums, span)?;
        if right_default.0.is_integer() {
            let left_with_right = lower_value_expr_with_expected(
                path,
                left,
                scope,
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
        ValueType::Array(element) => {
            ValueType::Array(Box::new(substitute_type_params(element, type_params, args)))
        }
        _ => value_type.clone(),
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

fn ast_binary_symbol(op: &AstBinaryOp) -> &'static str {
    match op {
        AstBinaryOp::Add => "+",
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
    let struct_names = structs.keys().cloned().collect::<Vec<_>>();
    let enum_names = enums.keys().cloned().collect::<Vec<_>>();
    parse_value_type_with_names(type_ref, &struct_names, &enum_names, &[])
}

fn parse_value_type_with_names(
    type_ref: &crate::ast::TypeRef,
    struct_names: &[String],
    enum_names: &[String],
    type_params: &[String],
) -> Option<ValueType> {
    match type_ref.path.as_slice() {
        [name] if name == "string" => Some(ValueType::String),
        [name] if name == "i64" => Some(ValueType::Int),
        [name] if name == "i32" => Some(ValueType::I32),
        [name] if name == "u32" => Some(ValueType::U32),
        [name] if name == "u64" => Some(ValueType::U64),
        [name] if name == "f64" => Some(ValueType::Float),
        [name] if name == "char" => Some(ValueType::Char),
        [name] if name == "bool" => Some(ValueType::Bool),
        [name] if name == "void" => Some(ValueType::Void),
        [name] if name == "Array" => {
            let [element] = type_ref.args.as_slice() else {
                return None;
            };
            let element_type =
                parse_value_type_with_names(element, struct_names, enum_names, type_params)?;
            Some(ValueType::Array(Box::new(element_type)))
        }
        [name] if struct_names.iter().any(|item| item == name) => {
            if !type_ref.args.is_empty() {
                return None;
            }
            Some(ValueType::Struct(name.to_string()))
        }
        [name] if enum_names.iter().any(|item| item == name) => {
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

fn println_type_error(path: &Path, span: &Span, function_name: &str) -> Diagnostic {
    Diagnostic::new(
        "N0402",
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
        "N0404",
        message,
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    )
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
            ValueType::Struct(name) => name,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_inline(source: &str) -> Result<Program, Diagnostic> {
        let path = Path::new("main.nomo");
        let tokens = lexer::lex(path, source)?;
        let ast = parser::parse(path, &tokens)?;
        lower_program(path, ast)
    }

    #[test]
    fn parses_stage0_hello() {
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
    fn accepts_fs_read_and_write_builtins() {
        let source = r#"package app.main

import std.fs
import std.io

fn load(path: string) -> Result<string, FsError> {
    let text: string = fs.read_to_string(path)?
    return Result.Ok(text)
}

fn save(path: string, content: string) -> Result<void, FsError> {
    return fs.write_string(path, content)
}

fn main() -> void {
    let write_result: Result<void, FsError> = save("/tmp/nomo-fs-test.txt", "hello")
    let read_result: Result<string, FsError> = load("/tmp/nomo-fs-test.txt")
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
                vec![ValueType::String, ValueType::Struct("FsError".to_string())],
            )
        );
        assert!(matches!(
            load.body[0],
            Statement::TryLet {
                result_expr: ValueExpr::FsReadToString { .. },
                ..
            }
        ));
        let save = program.functions.iter().find(|f| f.name == "save").unwrap();
        assert!(matches!(
            save.body[0],
            Statement::Return(Some(ValueExpr::FsWriteString { .. }))
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
    fn rejects_string_len_as_i64() {
        let source = r#"package app.main

import std.string

fn main() -> void {
    let count: i64 = string.len("hello")
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "N0404");
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
    fn rejects_i32_literal_overflow() {
        let source = r#"package app.main

fn main() -> void {
    let value: i32 = 2147483648
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "N0404");
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
        assert_eq!(err.code, "N0404");
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
        assert_eq!(err.code, "N0404");
    }

    #[test]
    fn rejects_char_string_mismatch() {
        let source = r#"package app.main

fn main() -> void {
    let text: string = 'N'
}
"#;

        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "N0404");
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
        assert_eq!(err.code, "N0404");
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
        assert_eq!(err.code, "N0404");
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
        assert_eq!(err.code, "N0303");
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
        assert_eq!(err.code, "N0404");
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
        assert_eq!(err.code, "N0404");
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
        assert_eq!(err.code, "N0410");
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
        assert_eq!(err.code, "N0404");
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
            ValueType::Struct("Point".to_string())
        );
        assert!(matches!(
            sum.body[0],
            Statement::Return(Some(ValueExpr::Binary { .. }))
        ));
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
            ValueType::Struct("User".to_string())
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
        assert_eq!(err.code, "N0404");
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
        assert_eq!(err.code, "N0308");
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
        assert_eq!(err.code, "N0318");
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
    fn accepts_result_try_let_binding() {
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
            Statement::TryLet {
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
    fn rejects_try_in_non_result_function() {
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
        assert_eq!(err.code, "N0421");
    }

    #[test]
    fn rejects_try_let_without_type_annotation() {
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
        assert_eq!(err.code, "N0403");
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
        assert_eq!(err.code, "N0321");
    }

    #[test]
    fn rejects_missing_main() {
        let source = "package app.main\nimport std.io\n";
        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "N0201");
    }

    #[test]
    fn rejects_missing_io_import() {
        let source = r#"package app.main

fn main() -> void {
    io.println("Hello")
}
"#;
        let err = parse_inline(source).unwrap_err();
        assert_eq!(err.code, "N0301");
    }
}

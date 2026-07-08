#![allow(
    clippy::collapsible_if,
    clippy::large_enum_variant,
    clippy::needless_borrow,
    clippy::needless_option_as_deref,
    clippy::redundant_closure,
    clippy::result_large_err,
    clippy::too_many_arguments
)]

pub use nomo_syntax::{ast, diagnostic, lexer, parser};

use crate::ast::{
    AssignOp, BinaryOp as AstBinaryOp, EnumDef as AstEnumDef, Expr as AstExpr, ForVariant,
    Function as AstFunction, FunctionSignature as AstFunctionSignature,
    InterfaceDef as AstInterfaceDef, MatchArm as AstMatchArm, PostfixOp, SourceFile, Span, Stmt,
    StructDef as AstStructDef, TypeRef as AstTypeRef, UnaryOp as AstUnaryOp,
};
use crate::diagnostic::{Diagnostic, Suggestion};
use nomo_codegen_c as codegen;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

mod analysis;
mod externs;
mod imports;
mod interfaces;
mod modules;
mod typing;
use analysis::*;
use externs::*;
use imports::*;
use interfaces::*;
use modules::merge_imported_public_api;
use typing::*;

const BUILTIN_PRINTLN_EXPR: &str = "__nomo_builtin_println";
const BUILTIN_PRINT_EXPR: &str = "__nomo_builtin_print";
const BUILTIN_EPRINTLN_EXPR: &str = "__nomo_builtin_eprintln";
const BUILTIN_EPRINT_EXPR: &str = "__nomo_builtin_eprint";
const BUILTIN_FFI_PUTS_EXPR: &str = "__nomo_ffi_puts";
const EXTERN_CALL_PREFIX: &str = "__nomo_extern::";
const BUILTIN_HTTP_GET_EXPR: &str = "__nomo_http_get";
const BUILTIN_HTTP_POST_EXPR: &str = "__nomo_http_post";
const BUILTIN_HTTP_LISTEN_EXPR: &str = "__nomo_http_listen";
const BUILTIN_HTTP_ACCEPT_EXPR: &str = "__nomo_http_accept";
const BUILTIN_HTTP_RESPOND_STRING_EXPR: &str = "__nomo_http_respond_string";
const BUILTIN_HTTP_CLOSE_SERVER_EXPR: &str = "__nomo_http_close_server";
const BUILTIN_HTTP_CLOSE_EXCHANGE_EXPR: &str = "__nomo_http_close_exchange";

pub use nomo_ir::{
    BinaryOp, Const, DeferredCall, EnumType, EnumVariantType, ExternFunction, Function, LoopKind,
    MatchStatementArm, MatchValueArm, MathBinaryFunction, MathUnaryFunction, NumBinaryFunction,
    Parameter, Program, QuestionCarrier, Statement, StructField, StructType, UnaryOp, ValueExpr,
    ValueType,
};

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

#[derive(Debug, Clone)]
struct FunctionSignature {
    type_params: Vec<String>,
    params: Vec<ParamSignature>,
    return_type: ValueType,
    extern_symbol: Option<String>,
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
    let interface_map = collect_interfaces(path, &ast.interfaces)?;
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
    let (extern_call_names, extern_functions) =
        collect_extern_signatures(path, &ast, &struct_map, &enum_map, &mut signatures)?;
    validate_extern_calls_are_unsafe(path, &ast, &extern_call_names)?;
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
        if let Some(interface_name) = &impl_block.interface_name {
            validate_interface_impl(
                path,
                impl_block,
                interface_name,
                &owner_name,
                &struct_map,
                &enum_map,
                &interface_map,
            )?;
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
        let const_scope = HashMap::new();
        let (init_type, initializer) = lower_value_expr_with_expected(
            path,
            &const_def.value,
            &const_scope,
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
        extern_functions,
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
        | Stmt::Defer { span, .. }
        | Stmt::Unsafe { span, .. } => span,
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
        Stmt::Unsafe { body, .. } => {
            for stmt in body {
                validate_stmt_type_imports(path, imports, stmt)?;
            }
            Ok(())
        }
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
            | "std.net"
            | "std.net.NetError"
            | "std.net.TcpListener"
            | "std.net.TcpStream"
            | "std.net.UdpDatagram"
            | "std.net.UdpSocket"
            | "std.net.connect"
            | "std.net.listen"
            | "std.net.udp_bind"
            | "std.http"
            | "std.http.HttpExchange"
            | "std.http.HttpError"
            | "std.http.HttpResponse"
            | "std.http.HttpServer"
            | "std.http.accept"
            | "std.http.close_exchange"
            | "std.http.close_server"
            | "std.http.get"
            | "std.http.listen"
            | "std.http.post"
            | "std.http.respond_string"
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
            | "std.hash.bytes"
            | "std.hash.new"
            | "std.hash.string"
            | "std.hash.write_bytes"
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
    if needs.net {
        reject_user_std_struct(path, structs, "NetError")?;
        reject_user_std_struct(path, structs, "TcpListener")?;
        reject_user_std_struct(path, structs, "TcpStream")?;
        reject_user_std_struct(path, structs, "UdpDatagram")?;
        reject_user_std_struct(path, structs, "UdpSocket")?;
    }
    if needs.http {
        reject_user_std_struct(path, structs, "HttpExchange")?;
        reject_user_std_struct(path, structs, "HttpError")?;
        reject_user_std_struct(path, structs, "HttpResponse")?;
        reject_user_std_struct(path, structs, "HttpServer")?;
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
    if needs.io || needs.fs || needs.net || needs.http || needs.num || needs.process || needs.result
    {
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
    net: bool,
    http: bool,
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
        extern_symbol: None,
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
        Stmt::Unsafe { body, span } => {
            let [stmt] = body.as_slice() else {
                return Err(Diagnostic::new(
                    "E1519",
                    "v0.1 unsafe blocks must contain exactly one statement",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            lower_stmt(
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
            )
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
            && question_expr_from_success_return(value, signatures).is_none() =>
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
        | Statement::QuestionReturn { .. }
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
        Statement::Return(Some(_)) | Statement::QuestionReturn { .. } | Statement::Panic(_) => true,
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
                let (carrier, ok_type) = question_payload(path, span, &result_type, expected)?;
                let return_payload_type = question_return_payload(expected, carrier);
                if ok_type != return_payload_type {
                    return Err(type_mismatch_expected_found(
                        path,
                        span,
                        format!(
                            "`?` unwraps `{}` but function returns `{}`",
                            ok_type.name(),
                            expected.name()
                        ),
                        &return_payload_type,
                        &ok_type,
                    ));
                }
                return Ok(Statement::QuestionReturn {
                    carrier,
                    ok_type,
                    result_type,
                    return_type: expected.clone(),
                    result_expr,
                });
            }
            if let Some((carrier, question_expr)) =
                question_expr_from_success_return(value, signatures)
            {
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
                let (actual_carrier, ok_type) =
                    question_payload(path, span, &result_type, expected)?;
                if actual_carrier != carrier {
                    return Err(Diagnostic::new(
                        "E0421",
                        "`?` carrier does not match the returned success variant",
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
                let return_payload_type = question_return_payload(expected, carrier);
                if ok_type != return_payload_type {
                    return Err(type_mismatch_expected_found(
                        path,
                        span,
                        format!(
                            "`?` unwraps `{}` but returned success variant expects `{}`",
                            ok_type.name(),
                            return_payload_type.name()
                        ),
                        &return_payload_type,
                        &ok_type,
                    ));
                }
                return Ok(Statement::QuestionReturn {
                    carrier,
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

fn question_expr_from_success_return<'a>(
    value: &'a AstExpr,
    signatures: &HashMap<String, FunctionSignature>,
) -> Option<(QuestionCarrier, &'a AstExpr)> {
    let AstExpr::Call { callee, args, .. } = value else {
        return None;
    };
    let [AstExpr::Question { expr }] = args.as_slice() else {
        return None;
    };
    if is_result_ok_callee(callee, signatures) {
        return Some((QuestionCarrier::Result, expr));
    }
    if is_option_some_callee(callee, signatures) {
        return Some((QuestionCarrier::Option, expr));
    }
    None
}

fn is_result_ok_callee(callee: &[String], signatures: &HashMap<String, FunctionSignature>) -> bool {
    match callee {
        [name] => name == "Ok" && !signatures.contains_key("Ok"),
        [enum_name, variant] => enum_name == "Result" && variant == "Ok",
        _ => false,
    }
}

fn is_option_some_callee(
    callee: &[String],
    signatures: &HashMap<String, FunctionSignature>,
) -> bool {
    match callee {
        [name] => name == "Some" && !signatures.contains_key("Some"),
        [enum_name, variant] => enum_name == "Option" && variant == "Some",
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
            let lowered_op = match op {
                AstUnaryOp::Not => UnaryOp::Not,
                AstUnaryOp::Negate => UnaryOp::Negate,
            };
            if matches!(lowered_op, UnaryOp::Negate) {
                return lower_negate_expr(
                    path, expr, scope, imports, signatures, structs, enums, expected, span,
                );
            }
            let (expr_type, expr) =
                lower_value_expr(path, expr, scope, imports, signatures, structs, enums, span)?;
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
                (UnaryOp::Negate, _) => unreachable!("negation is lowered before this match"),
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
                if qualified[0] == "http" {
                    return lower_http_builtin(
                        path, &qualified, args, scope, imports, signatures, structs, enums, span,
                    );
                }
                if qualified[0] == "net" {
                    return lower_net_builtin(
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
                if name == "puts" {
                    if !type_args.is_empty() {
                        return Err(type_mismatch(
                            path,
                            span,
                            "extern function `puts` does not accept type arguments",
                        ));
                    }
                    let [arg] = args.as_slice() else {
                        return Err(Diagnostic::new(
                            "E1519",
                            "extern function `puts` expects 1 argument",
                            path,
                            span.line,
                            span.column,
                            span.length,
                            &span.text,
                        ));
                    };
                    let (arg_type, lowered) = lower_value_expr_with_expected(
                        path,
                        arg,
                        scope,
                        imports,
                        signatures,
                        structs,
                        enums,
                        Some(&ValueType::String),
                        span,
                    )?;
                    if arg_type != ValueType::String {
                        return Err(type_mismatch(
                            path,
                            span,
                            "extern function `puts` expects a `string` argument",
                        ));
                    }
                    let return_type = if matches!(expected, Some(ValueType::Void)) {
                        ValueType::Void
                    } else {
                        ValueType::I32
                    };
                    return Ok((
                        return_type,
                        ValueExpr::Call {
                            name: BUILTIN_FFI_PUTS_EXPR.to_string(),
                            args: vec![lowered],
                        },
                    ));
                }
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
                    name: signature
                        .extern_symbol
                        .as_ref()
                        .map(|symbol| extern_call_name(symbol))
                        .unwrap_or(call_name),
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
            if is_http_builtin_call(callee) {
                require_import(path, imports, span, "std.http", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "http builtins do not accept type arguments",
                    ));
                }
                return lower_http_builtin(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if is_net_builtin_call(callee) {
                require_import(path, imports, span, "std.net", &callee.join("."))?;
                if !type_args.is_empty() {
                    return Err(type_mismatch(
                        path,
                        span,
                        "net builtins do not accept type arguments",
                    ));
                }
                return lower_net_builtin(
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
            if type_args.is_empty() && is_tcp_stream_value_method(callee, scope) {
                return lower_tcp_stream_value_method(
                    path, callee, args, scope, imports, signatures, structs, enums, span,
                );
            }
            if type_args.is_empty() && is_tcp_listener_value_method(callee, scope) {
                return lower_tcp_listener_value_method(path, callee, args, scope, span);
            }
            if type_args.is_empty() && is_udp_socket_value_method(callee, scope) {
                return lower_udp_socket_value_method(
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
                && matches!(
                    name.as_str(),
                    "new" | "string" | "bytes" | "write_string" | "write_bytes" | "finish"
                )
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

fn is_http_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "http"
                && matches!(
                    name.as_str(),
                    "get"
                        | "post"
                        | "listen"
                        | "accept"
                        | "respond_string"
                        | "close_server"
                        | "close_exchange"
                )
    )
}

fn is_net_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "net" && matches!(name.as_str(), "connect" | "listen" | "udp_bind")
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
        "bytes" => {
            let [value_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`hash.bytes` expects exactly one Array<u32> value",
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
            let expected_bytes = ValueType::Array(Box::new(ValueType::U32));
            if value_type != expected_bytes {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`hash.bytes` expects an Array<u32> value",
                    &expected_bytes,
                    &value_type,
                ));
            }
            Ok((
                ValueType::U64,
                ValueExpr::HashBytes {
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
        "write_bytes" => {
            let [state_arg, value_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`hash.write_bytes` expects a HashState and Array<u32> value",
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
                    "`hash.write_bytes` expects a HashState value",
                    &expected_state,
                    &state_type,
                ));
            }
            let (value_type, value) = lower_value_expr(
                path, value_arg, scope, imports, signatures, structs, enums, span,
            )?;
            let expected_bytes = ValueType::Array(Box::new(ValueType::U32));
            if value_type != expected_bytes {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`hash.write_bytes` expects an Array<u32> value",
                    &expected_bytes,
                    &value_type,
                ));
            }
            Ok((
                ValueType::Struct("HashState".to_string(), Vec::new()),
                ValueExpr::HashWriteBytes {
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

fn lower_http_builtin(
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
        unreachable!("http builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "http");
    let http_error = ValueType::Struct("HttpError".to_string(), Vec::new());
    let http_response = ValueType::Struct("HttpResponse".to_string(), Vec::new());
    let http_server = ValueType::Struct("HttpServer".to_string(), Vec::new());
    let http_exchange = ValueType::Struct("HttpExchange".to_string(), Vec::new());
    let response_result_type = ValueType::Enum(
        "Result".to_string(),
        vec![http_response, http_error.clone()],
    );
    match name.as_str() {
        "get" => {
            let [url_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`http.get` expects exactly one URL string",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (url_type, url) = lower_value_expr(
                path, url_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if url_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`http.get` expects a string URL",
                    &ValueType::String,
                    &url_type,
                ));
            }
            Ok((
                response_result_type,
                ValueExpr::Call {
                    name: BUILTIN_HTTP_GET_EXPR.to_string(),
                    args: vec![url],
                },
            ))
        }
        "post" => {
            let [url_arg, body_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`http.post` expects URL and body strings",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (url_type, url) = lower_value_expr(
                path, url_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if url_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`http.post` expects a string URL",
                    &ValueType::String,
                    &url_type,
                ));
            }
            let (body_type, body) = lower_value_expr(
                path, body_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if body_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`http.post` expects a string body",
                    &ValueType::String,
                    &body_type,
                ));
            }
            Ok((
                response_result_type,
                ValueExpr::Call {
                    name: BUILTIN_HTTP_POST_EXPR.to_string(),
                    args: vec![url, body],
                },
            ))
        }
        "listen" => {
            let [host_arg, port_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`http.listen` expects host and port arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (host_type, host) = lower_value_expr(
                path, host_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if host_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`http.listen` expects a string host",
                    &ValueType::String,
                    &host_type,
                ));
            }
            let (port_type, port) = lower_value_expr(
                path, port_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if port_type != ValueType::Int {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`http.listen` expects an i64 port",
                    &ValueType::Int,
                    &port_type,
                ));
            }
            Ok((
                ValueType::Enum("Result".to_string(), vec![http_server, http_error.clone()]),
                ValueExpr::Call {
                    name: BUILTIN_HTTP_LISTEN_EXPR.to_string(),
                    args: vec![host, port],
                },
            ))
        }
        "accept" => {
            let [server_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`http.accept` expects exactly one HttpServer",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (server_type, server) = lower_value_expr(
                path, server_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if server_type != http_server {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`http.accept` expects an HttpServer value",
                    &http_server,
                    &server_type,
                ));
            }
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![http_exchange, http_error.clone()],
                ),
                ValueExpr::Call {
                    name: BUILTIN_HTTP_ACCEPT_EXPR.to_string(),
                    args: vec![server],
                },
            ))
        }
        "respond_string" => {
            let [exchange_arg, status_arg, body_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`http.respond_string` expects exchange, status, and body arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (exchange_type, exchange) = lower_value_expr(
                path,
                exchange_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                span,
            )?;
            if exchange_type != http_exchange {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`http.respond_string` expects an HttpExchange value",
                    &http_exchange,
                    &exchange_type,
                ));
            }
            let (status_type, status) = lower_value_expr(
                path, status_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if status_type != ValueType::Int {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`http.respond_string` expects an i64 status",
                    &ValueType::Int,
                    &status_type,
                ));
            }
            let (body_type, body) = lower_value_expr(
                path, body_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if body_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`http.respond_string` expects a string body",
                    &ValueType::String,
                    &body_type,
                ));
            }
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![ValueType::Void, http_error.clone()],
                ),
                ValueExpr::Call {
                    name: BUILTIN_HTTP_RESPOND_STRING_EXPR.to_string(),
                    args: vec![exchange, status, body],
                },
            ))
        }
        "close_server" => {
            let [server_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`http.close_server` expects exactly one HttpServer",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (server_type, server) = lower_value_expr(
                path, server_arg, scope, imports, signatures, structs, enums, span,
            )?;
            if server_type != http_server {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`http.close_server` expects an HttpServer value",
                    &http_server,
                    &server_type,
                ));
            }
            Ok((
                ValueType::Void,
                ValueExpr::Call {
                    name: BUILTIN_HTTP_CLOSE_SERVER_EXPR.to_string(),
                    args: vec![server],
                },
            ))
        }
        "close_exchange" => {
            let [exchange_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`http.close_exchange` expects exactly one HttpExchange",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (exchange_type, exchange) = lower_value_expr(
                path,
                exchange_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                span,
            )?;
            if exchange_type != http_exchange {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`http.close_exchange` expects an HttpExchange value",
                    &http_exchange,
                    &exchange_type,
                ));
            }
            Ok((
                ValueType::Void,
                ValueExpr::Call {
                    name: BUILTIN_HTTP_CLOSE_EXCHANGE_EXPR.to_string(),
                    args: vec![exchange],
                },
            ))
        }
        _ => unreachable!("http builtin dispatcher only passes known calls"),
    }
}

fn lower_net_builtin(
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
        unreachable!("net builtin dispatcher only passes qualified calls")
    };
    debug_assert_eq!(module, "net");
    let net_error = ValueType::Struct("NetError".to_string(), Vec::new());
    match name.as_str() {
        "connect" | "listen" | "udp_bind" => {
            let [host_arg, port_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    format!("`net.{name}` expects host and port arguments"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (host_type, host) = lower_value_expr_with_expected(
                path,
                host_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&ValueType::String),
                span,
            )?;
            if host_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!("`net.{name}` expects a string host"),
                    &ValueType::String,
                    &host_type,
                ));
            }
            let (port_type, port) = lower_value_expr_with_expected(
                path,
                port_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&ValueType::Int),
                span,
            )?;
            if port_type != ValueType::Int {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!("`net.{name}` expects an i64 port"),
                    &ValueType::Int,
                    &port_type,
                ));
            }
            let ok_type = if name == "connect" {
                ValueType::Struct("TcpStream".to_string(), Vec::new())
            } else if name == "listen" {
                ValueType::Struct("TcpListener".to_string(), Vec::new())
            } else {
                ValueType::Struct("UdpSocket".to_string(), Vec::new())
            };
            let result_type = ValueType::Enum("Result".to_string(), vec![ok_type, net_error]);
            let expr = if name == "connect" {
                ValueExpr::NetConnect {
                    host: Box::new(host),
                    port: Box::new(port),
                }
            } else if name == "listen" {
                ValueExpr::NetListen {
                    host: Box::new(host),
                    port: Box::new(port),
                }
            } else {
                ValueExpr::NetUdpBind {
                    host: Box::new(host),
                    port: Box::new(port),
                }
            };
            Ok((result_type, expr))
        }
        _ => unreachable!("net builtin dispatcher only passes known calls"),
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

fn is_tcp_stream_value_method(callee: &[String], scope: &HashMap<String, Binding>) -> bool {
    if callee.len() != 2 {
        return false;
    }
    scope.get(&callee[0]).is_some_and(|binding| {
        binding.value_type == ValueType::Struct("TcpStream".to_string(), Vec::new())
    })
}

fn lower_tcp_stream_value_method(
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
    let binding = scope
        .get(name)
        .expect("tcp stream method receiver is in scope");
    let receiver_expr = binding_value_expr(name, binding);
    let net_error = ValueType::Struct("NetError".to_string(), Vec::new());
    match method.as_str() {
        "close" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`TcpStream.close` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::Void,
                ValueExpr::TcpStreamClose {
                    stream: Box::new(receiver_expr),
                },
            ))
        }
        "read_to_string" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`TcpStream.read_to_string` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::Enum("Result".to_string(), vec![ValueType::String, net_error]),
                ValueExpr::TcpStreamReadToString {
                    stream: Box::new(receiver_expr),
                },
            ))
        }
        "write_string" => {
            let [content_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`TcpStream.write_string` expects exactly one content string",
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
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`TcpStream.write_string` expects string content",
                    &ValueType::String,
                    &content_type,
                ));
            }
            Ok((
                ValueType::Enum("Result".to_string(), vec![ValueType::Void, net_error]),
                ValueExpr::TcpStreamWriteString {
                    stream: Box::new(receiver_expr),
                    content: Box::new(lowered_content),
                },
            ))
        }
        _ => Err(Diagnostic::new(
            "E0305",
            format!("unknown TcpStream method `{method}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        )),
    }
}

fn is_tcp_listener_value_method(callee: &[String], scope: &HashMap<String, Binding>) -> bool {
    if callee.len() != 2 {
        return false;
    }
    scope.get(&callee[0]).is_some_and(|binding| {
        binding.value_type == ValueType::Struct("TcpListener".to_string(), Vec::new())
    })
}

fn lower_tcp_listener_value_method(
    path: &Path,
    callee: &[String],
    args: &[AstExpr],
    scope: &HashMap<String, Binding>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let name = &callee[0];
    let method = &callee[1];
    let binding = scope
        .get(name)
        .expect("tcp listener method receiver is in scope");
    let receiver_expr = binding_value_expr(name, binding);
    let net_error = ValueType::Struct("NetError".to_string(), Vec::new());
    match method.as_str() {
        "accept" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`TcpListener.accept` does not accept arguments",
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
                        ValueType::Struct("TcpStream".to_string(), Vec::new()),
                        net_error,
                    ],
                ),
                ValueExpr::TcpListenerAccept {
                    listener: Box::new(receiver_expr),
                },
            ))
        }
        "close" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`TcpListener.close` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::Void,
                ValueExpr::TcpListenerClose {
                    listener: Box::new(receiver_expr),
                },
            ))
        }
        _ => Err(Diagnostic::new(
            "E0305",
            format!("unknown TcpListener method `{method}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        )),
    }
}

fn is_udp_socket_value_method(callee: &[String], scope: &HashMap<String, Binding>) -> bool {
    if callee.len() != 2 {
        return false;
    }
    scope.get(&callee[0]).is_some_and(|binding| {
        binding.value_type == ValueType::Struct("UdpSocket".to_string(), Vec::new())
    })
}

fn lower_udp_socket_value_method(
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
    let binding = scope
        .get(name)
        .expect("udp socket method receiver is in scope");
    let receiver_expr = binding_value_expr(name, binding);
    let net_error = ValueType::Struct("NetError".to_string(), Vec::new());
    match method.as_str() {
        "close" => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E0407",
                    "`UdpSocket.close` does not accept arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            Ok((
                ValueType::Void,
                ValueExpr::UdpSocketClose {
                    socket: Box::new(receiver_expr),
                },
            ))
        }
        "recv_from_string" => {
            let [max_bytes_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`UdpSocket.recv_from_string` expects a max byte count",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (max_bytes_type, max_bytes) = lower_value_expr_with_expected(
                path,
                max_bytes_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&ValueType::Int),
                span,
            )?;
            if max_bytes_type != ValueType::Int {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`UdpSocket.recv_from_string` expects an i64 max byte count",
                    &ValueType::Int,
                    &max_bytes_type,
                ));
            }
            Ok((
                ValueType::Enum(
                    "Result".to_string(),
                    vec![
                        ValueType::Struct("UdpDatagram".to_string(), Vec::new()),
                        net_error,
                    ],
                ),
                ValueExpr::UdpSocketRecvFromString {
                    socket: Box::new(receiver_expr),
                    max_bytes: Box::new(max_bytes),
                },
            ))
        }
        "send_to_string" => {
            let [content_arg, host_arg, port_arg] = args else {
                return Err(Diagnostic::new(
                    "E0407",
                    "`UdpSocket.send_to_string` expects content, host, and port arguments",
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (content_type, content) = lower_value_expr_with_expected(
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
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`UdpSocket.send_to_string` expects string content",
                    &ValueType::String,
                    &content_type,
                ));
            }
            let (host_type, host) = lower_value_expr_with_expected(
                path,
                host_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&ValueType::String),
                span,
            )?;
            if host_type != ValueType::String {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`UdpSocket.send_to_string` expects a string host",
                    &ValueType::String,
                    &host_type,
                ));
            }
            let (port_type, port) = lower_value_expr_with_expected(
                path,
                port_arg,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&ValueType::Int),
                span,
            )?;
            if port_type != ValueType::Int {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    "`UdpSocket.send_to_string` expects an i64 port",
                    &ValueType::Int,
                    &port_type,
                ));
            }
            Ok((
                ValueType::Enum("Result".to_string(), vec![ValueType::Void, net_error]),
                ValueExpr::UdpSocketSendToString {
                    socket: Box::new(receiver_expr),
                    content: Box::new(content),
                    host: Box::new(host),
                    port: Box::new(port),
                },
            ))
        }
        _ => Err(Diagnostic::new(
            "E0305",
            format!("unknown UdpSocket method `{method}`"),
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

#[cfg(test)]
mod tests;

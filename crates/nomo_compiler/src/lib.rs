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
mod analysis_generic;
mod analysis_usage;
mod builtins_core_std;
mod builtins_diagnostics;
mod builtins_extensions;
mod builtins_result_option;
mod builtins_string;
mod builtins_system;
mod builtins_value_methods;
mod declarations;
mod expression_calls;
mod expression_enums;
mod expression_helpers;
mod expression_if;
mod expression_match;
mod expression_ops;
mod expression_single_calls;
mod expression_structs;
mod expressions;
mod externs;
mod imports;
mod interfaces;
mod modules;
mod question_extraction;
mod question_lowering;
mod statement_assignments;
mod statement_patterns;
mod statements;
mod typing;
mod validation;
use analysis::*;
use analysis_generic::*;
use analysis_usage::*;
use builtins_core_std::*;
use builtins_diagnostics::*;
use builtins_extensions::*;
use builtins_result_option::*;
use builtins_string::*;
use builtins_system::*;
use builtins_value_methods::*;
use declarations::*;
use expression_calls::*;
use expression_enums::*;
use expression_helpers::*;
use expression_if::*;
use expression_match::*;
use expression_ops::*;
use expression_single_calls::*;
use expression_structs::*;
use expressions::*;
use externs::*;
use imports::*;
use interfaces::*;
use modules::merge_imported_public_api;
use question_extraction::*;
use question_lowering::*;
use statement_assignments::*;
use statement_patterns::*;
use statements::*;
use typing::*;
use validation::*;

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

#[cfg(test)]
mod tests;

use super::*;

pub(super) fn source_uses_fs_builtin(ast: &SourceFile) -> bool {
    source_uses_builtin(ast, |expr| expr_uses_builtin(expr, is_fs_builtin_call))
}

pub(super) fn source_uses_io_read_line(ast: &SourceFile) -> bool {
    source_uses_builtin(ast, |expr| expr_uses_builtin(expr, is_io_read_line_call))
}

pub(super) fn source_uses_env_builtin(ast: &SourceFile) -> bool {
    source_uses_builtin(ast, |expr| expr_uses_builtin(expr, is_env_builtin_call))
}

pub(super) fn source_uses_process_builtin(ast: &SourceFile) -> bool {
    source_uses_builtin(ast, |expr| expr_uses_builtin(expr, is_process_builtin_call))
}

pub(super) fn source_uses_hash_builtin(ast: &SourceFile) -> bool {
    source_uses_builtin(ast, |expr| expr_uses_builtin(expr, is_hash_builtin_call))
}

pub(super) fn source_uses_json_builtin(ast: &SourceFile) -> bool {
    source_uses_builtin(ast, |expr| expr_uses_builtin(expr, is_json_builtin_call))
}

pub(super) fn source_uses_regex_builtin(ast: &SourceFile) -> bool {
    source_uses_builtin(ast, |expr| expr_uses_builtin(expr, is_regex_builtin_call))
}

pub(super) fn source_uses_num_builtin(ast: &SourceFile) -> bool {
    source_uses_builtin(ast, |expr| expr_uses_builtin(expr, is_num_builtin_call))
}

pub(super) fn source_uses_time_builtin(ast: &SourceFile) -> bool {
    source_uses_builtin(ast, |expr| expr_uses_builtin(expr, is_time_builtin_call))
}

pub(super) fn source_uses_array_builtin(ast: &SourceFile) -> bool {
    source_uses_builtin(ast, |expr| expr_uses_builtin(expr, is_array_builtin_call))
}

fn source_uses_builtin(ast: &SourceFile, expr_uses: impl Fn(&AstExpr) -> bool) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(|stmt| stmt_uses_expr(stmt, &expr_uses))
}

fn stmt_uses_expr(stmt: &Stmt, expr_uses: &impl Fn(&AstExpr) -> bool) -> bool {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => expr_uses(value),
        Stmt::LetElse {
            value, else_body, ..
        } => expr_uses(value) || else_body.iter().any(|stmt| stmt_uses_expr(stmt, expr_uses)),
        Stmt::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            expr_uses(value)
                || body.iter().any(|stmt| stmt_uses_expr(stmt, expr_uses))
                || else_body.as_ref().is_some_and(|else_body| {
                    else_body.iter().any(|stmt| stmt_uses_expr(stmt, expr_uses))
                })
        }
        Stmt::Return { value, .. } => value.as_ref().is_some_and(expr_uses),
        Stmt::Expr { expr, .. } => expr_uses(expr),
        Stmt::Match { value, arms, .. } => {
            expr_uses(value)
                || arms
                    .iter()
                    .any(|arm| arm.body.iter().any(|stmt| stmt_uses_expr(stmt, expr_uses)))
        }
        Stmt::For { variant, .. } => match variant {
            ForVariant::Infinite { body } => {
                body.iter().any(|stmt| stmt_uses_expr(stmt, expr_uses))
            }
            ForVariant::While { condition, body } => {
                expr_uses(condition) || body.iter().any(|stmt| stmt_uses_expr(stmt, expr_uses))
            }
            ForVariant::Iterate { iterable, body, .. } => {
                expr_uses(iterable) || body.iter().any(|stmt| stmt_uses_expr(stmt, expr_uses))
            }
        },
        Stmt::Defer { stmt, .. } => stmt_uses_expr(stmt, expr_uses),
        Stmt::Unsafe { body, .. } => body.iter().any(|stmt| stmt_uses_expr(stmt, expr_uses)),
        Stmt::Postfix { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => false,
    }
}

fn expr_uses_builtin(expr: &AstExpr, is_builtin_call: impl Fn(&[String]) -> bool + Copy) -> bool {
    match expr {
        AstExpr::Call { callee, args, .. } => {
            is_builtin_call(callee)
                || args
                    .iter()
                    .any(|arg| expr_uses_builtin(arg, is_builtin_call))
        }
        AstExpr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_uses_builtin(value, is_builtin_call)),
        AstExpr::Match { value, arms } => {
            expr_uses_builtin(value, is_builtin_call)
                || arms
                    .iter()
                    .any(|arm| expr_uses_builtin(&arm.value, is_builtin_call))
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_uses_builtin(condition, is_builtin_call)
                || expr_uses_builtin(then_branch, is_builtin_call)
                || expr_uses_builtin(else_branch, is_builtin_call)
        }
        AstExpr::Panic { message }
        | AstExpr::Question { expr: message }
        | AstExpr::Unary { expr: message, .. } => expr_uses_builtin(message, is_builtin_call),
        AstExpr::Cast { expr, .. } => expr_uses_builtin(expr, is_builtin_call),
        AstExpr::Binary { left, right, .. } => {
            expr_uses_builtin(left, is_builtin_call) || expr_uses_builtin(right, is_builtin_call)
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

fn is_fs_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "fs"
                && matches!(
                    name.as_str(),
                    "read_to_string"
                        | "write_string"
                        | "read_bytes"
                        | "write_bytes"
                        | "exists"
                        | "metadata"
                        | "create_dir"
                        | "remove_dir"
                        | "read_dir"
                        | "open"
                )
    )
}

fn is_io_read_line_call(callee: &[String]) -> bool {
    matches!(callee, [module, name] if module == "io" && name == "read_line")
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

fn is_time_builtin_call(callee: &[String]) -> bool {
    matches!(
        callee,
        [module, name]
            if module == "time"
                && matches!(
                    name.as_str(),
                    "duration_millis"
                        | "duration_seconds"
                        | "duration_as_millis"
                        | "format_duration"
                        | "sleep"
                )
    )
}

fn is_array_builtin_call(callee: &[String]) -> bool {
    callee == ["Array", "new"]
        || matches!(
            callee,
            [receiver, method]
                if !is_known_std_value_module(receiver)
                    && matches!(method.as_str(), "len" | "get" | "push" | "set")
        )
}

fn is_known_std_value_module(name: &str) -> bool {
    matches!(
        name,
        "io" | "fs" | "env" | "process" | "string" | "path" | "math" | "collections" | "Array"
    )
}

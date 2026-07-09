use super::*;

pub(super) fn source_uses_fs_builtin(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_fs_builtin)
}

pub(super) fn source_uses_io_read_line(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_io_read_line)
}

pub(super) fn source_uses_env_builtin(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_env_builtin)
}

pub(super) fn source_uses_process_builtin(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_process_builtin)
}

pub(super) fn source_uses_hash_builtin(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_hash_builtin)
}

pub(super) fn source_uses_json_builtin(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_json_builtin)
}

pub(super) fn source_uses_regex_builtin(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_regex_builtin)
}

pub(super) fn source_uses_num_builtin(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_num_builtin)
}

pub(super) fn source_uses_time_builtin(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_time_builtin)
}

pub(super) fn source_uses_array_builtin(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_array_builtin)
}

pub(super) fn source_uses_result_prelude_variant(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_result_prelude_variant)
        || ast
            .consts
            .iter()
            .any(|const_def| expr_uses_result_prelude_variant(&const_def.value))
}

pub(super) fn source_uses_option_prelude_variant(ast: &SourceFile) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(stmt_uses_option_prelude_variant)
        || ast
            .consts
            .iter()
            .any(|const_def| expr_uses_option_prelude_variant(&const_def.value))
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
        Stmt::Unsafe { body, .. } => body.iter().any(stmt_uses_fs_builtin),
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
        Stmt::Unsafe { body, .. } => body.iter().any(stmt_uses_io_read_line),
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
        Stmt::Unsafe { body, .. } => body.iter().any(stmt_uses_env_builtin),
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
        Stmt::Unsafe { body, .. } => body.iter().any(stmt_uses_process_builtin),
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
        Stmt::Unsafe { body, .. } => body.iter().any(stmt_uses_hash_builtin),
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
        Stmt::Unsafe { body, .. } => body.iter().any(stmt_uses_json_builtin),
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
        Stmt::Unsafe { body, .. } => body.iter().any(stmt_uses_regex_builtin),
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
        Stmt::Unsafe { body, .. } => body.iter().any(stmt_uses_num_builtin),
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
        Stmt::Unsafe { body, .. } => body.iter().any(stmt_uses_time_builtin),
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
        Stmt::Unsafe { body, .. } => body.iter().any(stmt_uses_array_builtin),
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
        Stmt::Unsafe { body, .. } => body
            .iter()
            .any(|stmt| stmt_uses_core_prelude_variant(stmt, enum_name)),
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

use super::*;

pub(super) fn source_uses_result_prelude_variant(ast: &SourceFile) -> bool {
    source_uses_core_prelude_variant(ast, "Result")
}

pub(super) fn source_uses_option_prelude_variant(ast: &SourceFile) -> bool {
    source_uses_core_prelude_variant(ast, "Option")
}

fn source_uses_core_prelude_variant(ast: &SourceFile, enum_name: &str) -> bool {
    ast_functions(ast)
        .flat_map(|function| function.body.iter())
        .any(|stmt| stmt_uses_core_prelude_variant(stmt, enum_name))
        || ast
            .consts
            .iter()
            .any(|const_def| expr_uses_core_prelude_variant(&const_def.value, enum_name))
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
            ForVariant::CStyle {
                initializer,
                condition,
                update,
                body,
                ..
            } => {
                expr_uses_core_prelude_variant(initializer, enum_name)
                    || expr_uses_core_prelude_variant(condition, enum_name)
                    || stmt_uses_core_prelude_variant(update, enum_name)
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

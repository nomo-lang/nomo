use super::*;

pub(super) fn validate_p1_yield_function(
    path: &Path,
    function: &AstFunction,
    imports: &[String],
) -> Result<(), Diagnostic> {
    let has_yield = function
        .body
        .iter()
        .any(|statement| ast_statement_contains_yield(statement, imports));
    if !has_yield || !function.is_suspend {
        return Ok(());
    }

    if function.package.as_slice() == ["std", "task"] && function.name == "yield_now" {
        return Ok(());
    }

    let supported_signature = function.name == "main"
        && function.params.is_empty()
        && function.return_type.path.as_slice() == ["void"]
        && function.return_type.args.is_empty();
    let supported_body = function.body.iter().all(|statement| match statement {
        Stmt::Expr { expr, .. } => {
            ast_expr_is_direct_yield(expr, imports) || !ast_expr_contains_yield(expr, imports)
        }
        _ => false,
    });

    if supported_signature && supported_body {
        return Ok(());
    }

    Err(Diagnostic::new(
        "E0876",
        "the first current-thread runtime slice supports `task.yield_now()` only as a standalone statement in parameterless `suspend fn main() -> void`; move local/control-flow suspension into a later supported slice",
        path,
        function.span.line,
        function.span.column,
        function.span.length,
        &function.span.text,
    ))
}

fn ast_statement_contains_yield(statement: &Stmt, imports: &[String]) -> bool {
    match statement {
        Stmt::Let { value, .. }
        | Stmt::Assign { value, .. }
        | Stmt::Return {
            value: Some(value), ..
        }
        | Stmt::Expr { expr: value, .. } => ast_expr_contains_yield(value, imports),
        Stmt::IndexAssign { indices, value, .. } => {
            indices
                .iter()
                .any(|index| ast_expr_contains_yield(index, imports))
                || ast_expr_contains_yield(value, imports)
        }
        Stmt::LetElse {
            value, else_body, ..
        } => {
            ast_expr_contains_yield(value, imports)
                || else_body
                    .iter()
                    .any(|statement| ast_statement_contains_yield(statement, imports))
        }
        Stmt::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            ast_expr_contains_yield(value, imports)
                || body
                    .iter()
                    .any(|statement| ast_statement_contains_yield(statement, imports))
                || else_body.as_ref().is_some_and(|body| {
                    body.iter()
                        .any(|statement| ast_statement_contains_yield(statement, imports))
                })
        }
        Stmt::Match { value, arms, .. } => {
            ast_expr_contains_yield(value, imports)
                || arms.iter().any(|arm| {
                    arm.body
                        .iter()
                        .any(|statement| ast_statement_contains_yield(statement, imports))
                })
        }
        Stmt::For { variant, .. } => match variant {
            ForVariant::Infinite { body } => body
                .iter()
                .any(|statement| ast_statement_contains_yield(statement, imports)),
            ForVariant::While { condition, body } => {
                ast_expr_contains_yield(condition, imports)
                    || body
                        .iter()
                        .any(|statement| ast_statement_contains_yield(statement, imports))
            }
            ForVariant::CStyle {
                initializer,
                condition,
                update,
                body,
                ..
            } => {
                ast_expr_contains_yield(initializer, imports)
                    || ast_expr_contains_yield(condition, imports)
                    || ast_statement_contains_yield(update, imports)
                    || body
                        .iter()
                        .any(|statement| ast_statement_contains_yield(statement, imports))
            }
            ForVariant::Iterate { iterable, body, .. } => {
                ast_expr_contains_yield(iterable, imports)
                    || body
                        .iter()
                        .any(|statement| ast_statement_contains_yield(statement, imports))
            }
        },
        Stmt::Defer { stmt, .. } => ast_statement_contains_yield(stmt, imports),
        Stmt::Unsafe { body, .. } => body
            .iter()
            .any(|statement| ast_statement_contains_yield(statement, imports)),
        Stmt::Postfix { .. }
        | Stmt::Return { value: None, .. }
        | Stmt::Break { .. }
        | Stmt::Continue { .. } => false,
    }
}

fn ast_expr_contains_yield(expr: &AstExpr, imports: &[String]) -> bool {
    if ast_expr_is_direct_yield(expr, imports) {
        return true;
    }
    match expr {
        AstExpr::ArrayLiteral { elements } => elements
            .iter()
            .any(|element| ast_expr_contains_yield(element, imports)),
        AstExpr::Index { base, index } => {
            ast_expr_contains_yield(base, imports) || ast_expr_contains_yield(index, imports)
        }
        AstExpr::Call { args, .. } => args
            .iter()
            .any(|argument| ast_expr_contains_yield(argument, imports)),
        AstExpr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| ast_expr_contains_yield(value, imports)),
        AstExpr::Match { value, arms } => {
            ast_expr_contains_yield(value, imports)
                || arms
                    .iter()
                    .any(|arm| ast_expr_contains_yield(&arm.value, imports))
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            ast_expr_contains_yield(condition, imports)
                || ast_expr_contains_yield(then_branch, imports)
                || ast_expr_contains_yield(else_branch, imports)
        }
        AstExpr::Panic { message }
        | AstExpr::Question { expr: message }
        | AstExpr::Cast { expr: message, .. }
        | AstExpr::Unary { expr: message, .. } => ast_expr_contains_yield(message, imports),
        AstExpr::Binary { left, right, .. } => {
            ast_expr_contains_yield(left, imports) || ast_expr_contains_yield(right, imports)
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

fn ast_expr_is_direct_yield(expr: &AstExpr, imports: &[String]) -> bool {
    let AstExpr::Call {
        callee,
        type_args,
        args,
    } = expr
    else {
        return false;
    };
    type_args.is_empty()
        && args.is_empty()
        && (callee.as_slice() == ["task", "yield_now"]
            || (callee.as_slice() == ["yield_now"]
                && imports.iter().any(|item| item == "std.task.yield_now")))
}

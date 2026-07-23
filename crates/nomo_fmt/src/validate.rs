use nomo_diagnostics::Diagnostic;
use nomo_syntax::ast::{ForVariant, SourceFile, Span, Stmt};
use std::path::Path;

pub(super) fn validate_format_ast(path: &Path, ast: &SourceFile) -> Result<(), Diagnostic> {
    for function in &ast.functions {
        validate_stmts(path, &function.body)?;
    }
    validate_stmts(path, &ast.script_body)?;
    for block in &ast.impls {
        for method in &block.methods {
            validate_stmts(path, &method.body)?;
        }
    }
    Ok(())
}

pub(super) fn stmt_line(stmt: &Stmt) -> usize {
    stmt_span(stmt).line
}

fn validate_stmts(path: &Path, stmts: &[Stmt]) -> Result<(), Diagnostic> {
    for stmt in stmts {
        match stmt {
            Stmt::LetElse { else_body, .. } => validate_stmts(path, else_body)?,
            Stmt::IfLet {
                body, else_body, ..
            } => {
                validate_stmts(path, body)?;
                if let Some(else_body) = else_body {
                    validate_stmts(path, else_body)?;
                }
            }
            Stmt::Match { arms, .. } => {
                for arm in arms {
                    validate_stmts(path, &arm.body)?;
                }
            }
            Stmt::For { variant, .. } => match variant {
                ForVariant::Infinite { body }
                | ForVariant::While { body, .. }
                | ForVariant::CStyle { body, .. }
                | ForVariant::Iterate { body, .. } => validate_stmts(path, body)?,
            },
            Stmt::Defer { stmt, .. } => {
                if !matches!(stmt.as_ref(), Stmt::Expr { .. }) {
                    let span = stmt_span(stmt);
                    return Err(Diagnostic::new(
                        "E0902",
                        "`nomo fmt` cannot safely format non-expression `defer` statements",
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
            }
            Stmt::Unsafe { body, .. } => validate_stmts(path, body)?,
            Stmt::Let { .. }
            | Stmt::Assign { .. }
            | Stmt::Postfix { .. }
            | Stmt::Return { .. }
            | Stmt::Expr { .. }
            | Stmt::Break { .. }
            | Stmt::Continue { .. } => {}
        }
    }
    Ok(())
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

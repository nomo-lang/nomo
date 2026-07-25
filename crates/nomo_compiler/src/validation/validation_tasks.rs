use super::*;

pub(super) fn validate_task_workers(
    path: &Path,
    ast: &SourceFile,
    imports: &[String],
    extern_calls: &HashSet<String>,
) -> Result<(), Diagnostic> {
    let functions = ast
        .functions
        .iter()
        .map(|function| (function.name.as_str(), function))
        .collect::<HashMap<_, _>>();
    let local_method_names = ast
        .impls
        .iter()
        .flat_map(|impl_block| impl_block.methods.iter())
        .map(|method| method.name.as_str())
        .collect::<HashSet<_>>();
    let mut workers = Vec::new();
    for function in &ast.functions {
        if function.package.as_slice() == ["std", "task"] && function.name == "spawn" {
            continue;
        }
        collect_spawn_workers(
            path,
            &function.body,
            function,
            imports,
            &functions,
            &mut workers,
        )?;
    }

    for worker in workers {
        let mut visiting = Vec::new();
        let mut checked = HashSet::new();
        validate_task_safe_function(
            path,
            worker,
            imports,
            extern_calls,
            &functions,
            &local_method_names,
            &mut visiting,
            &mut checked,
        )?;
    }
    Ok(())
}

fn collect_spawn_workers<'a>(
    path: &Path,
    statements: &[Stmt],
    caller: &'a AstFunction,
    imports: &[String],
    functions: &HashMap<&str, &'a AstFunction>,
    workers: &mut Vec<&'a AstFunction>,
) -> Result<(), Diagnostic> {
    for statement in statements {
        let span = statement_span(statement);
        visit_statement_expressions(statement, &mut |expression| {
            let AstExpr::Call { callee, args, .. } = expression else {
                return Ok(());
            };
            if !is_task_spawn_call(callee, imports) {
                return Ok(());
            }
            let Some(AstExpr::Name(worker_path)) = args.first() else {
                return Ok(());
            };
            let [worker_name] = worker_path.as_slice() else {
                return Ok(());
            };
            let Some(worker) = functions.get(worker_name.as_str()).copied() else {
                return Ok(());
            };
            if worker.package != caller.package {
                return Err(task_safety_diagnostic(
                    path,
                    span,
                    format!(
                        "task worker `{worker_name}` must be declared in package `{}`",
                        caller.package.join(".")
                    ),
                ));
            }
            if !workers.iter().any(|item| std::ptr::eq(*item, worker)) {
                workers.push(worker);
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_task_safe_function<'a>(
    path: &Path,
    function: &'a AstFunction,
    imports: &[String],
    extern_calls: &HashSet<String>,
    functions: &HashMap<&str, &'a AstFunction>,
    local_method_names: &HashSet<&str>,
    visiting: &mut Vec<&'a str>,
    checked: &mut HashSet<&'a str>,
) -> Result<(), Diagnostic> {
    if checked.contains(function.name.as_str()) {
        return Ok(());
    }
    if visiting.contains(&function.name.as_str()) {
        return Ok(());
    }
    visiting.push(function.name.as_str());
    for statement in &function.body {
        validate_task_safe_statement(
            path,
            statement,
            imports,
            extern_calls,
            functions,
            local_method_names,
            visiting,
            checked,
        )?;
    }
    visiting.pop();
    checked.insert(function.name.as_str());
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_task_safe_statement<'a>(
    path: &Path,
    statement: &Stmt,
    imports: &[String],
    extern_calls: &HashSet<String>,
    functions: &HashMap<&str, &'a AstFunction>,
    local_method_names: &HashSet<&str>,
    visiting: &mut Vec<&'a str>,
    checked: &mut HashSet<&'a str>,
) -> Result<(), Diagnostic> {
    if matches!(statement, Stmt::Unsafe { .. }) {
        return Err(task_safety_call_path_diagnostic(
            path,
            statement_span(statement),
            visiting,
            "unsafe block",
        ));
    }
    let span = statement_span(statement);
    match statement {
        Stmt::Let { value, .. }
        | Stmt::Assign { value, .. }
        | Stmt::Return {
            value: Some(value), ..
        }
        | Stmt::Expr { expr: value, .. } => validate_task_expression_tree(
            path,
            value,
            span,
            imports,
            extern_calls,
            functions,
            local_method_names,
            visiting,
            checked,
        ),
        Stmt::IndexAssign { indices, value, .. } => {
            for index in indices {
                validate_task_expression_tree(
                    path,
                    index,
                    span,
                    imports,
                    extern_calls,
                    functions,
                    local_method_names,
                    visiting,
                    checked,
                )?;
            }
            validate_task_expression_tree(
                path,
                value,
                span,
                imports,
                extern_calls,
                functions,
                local_method_names,
                visiting,
                checked,
            )
        }
        Stmt::LetElse {
            value, else_body, ..
        } => {
            validate_task_expression_tree(
                path,
                value,
                span,
                imports,
                extern_calls,
                functions,
                local_method_names,
                visiting,
                checked,
            )?;
            validate_task_safe_statements(
                path,
                else_body,
                imports,
                extern_calls,
                functions,
                local_method_names,
                visiting,
                checked,
            )
        }
        Stmt::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            validate_task_expression_tree(
                path,
                value,
                span,
                imports,
                extern_calls,
                functions,
                local_method_names,
                visiting,
                checked,
            )?;
            validate_task_safe_statements(
                path,
                body,
                imports,
                extern_calls,
                functions,
                local_method_names,
                visiting,
                checked,
            )?;
            if let Some(else_body) = else_body {
                validate_task_safe_statements(
                    path,
                    else_body,
                    imports,
                    extern_calls,
                    functions,
                    local_method_names,
                    visiting,
                    checked,
                )?;
            }
            Ok(())
        }
        Stmt::Match { value, arms, .. } => {
            validate_task_expression_tree(
                path,
                value,
                span,
                imports,
                extern_calls,
                functions,
                local_method_names,
                visiting,
                checked,
            )?;
            for arm in arms {
                validate_task_safe_statements(
                    path,
                    &arm.body,
                    imports,
                    extern_calls,
                    functions,
                    local_method_names,
                    visiting,
                    checked,
                )?;
            }
            Ok(())
        }
        Stmt::For { variant, .. } => match variant {
            ForVariant::Infinite { body } => validate_task_safe_statements(
                path,
                body,
                imports,
                extern_calls,
                functions,
                local_method_names,
                visiting,
                checked,
            ),
            ForVariant::While { condition, body } => {
                validate_task_expression_tree(
                    path,
                    condition,
                    span,
                    imports,
                    extern_calls,
                    functions,
                    local_method_names,
                    visiting,
                    checked,
                )?;
                validate_task_safe_statements(
                    path,
                    body,
                    imports,
                    extern_calls,
                    functions,
                    local_method_names,
                    visiting,
                    checked,
                )
            }
            ForVariant::CStyle {
                initializer,
                condition,
                update,
                body,
                ..
            } => {
                validate_task_expression_tree(
                    path,
                    initializer,
                    span,
                    imports,
                    extern_calls,
                    functions,
                    local_method_names,
                    visiting,
                    checked,
                )?;
                validate_task_expression_tree(
                    path,
                    condition,
                    span,
                    imports,
                    extern_calls,
                    functions,
                    local_method_names,
                    visiting,
                    checked,
                )?;
                validate_task_safe_statement(
                    path,
                    update,
                    imports,
                    extern_calls,
                    functions,
                    local_method_names,
                    visiting,
                    checked,
                )?;
                validate_task_safe_statements(
                    path,
                    body,
                    imports,
                    extern_calls,
                    functions,
                    local_method_names,
                    visiting,
                    checked,
                )
            }
            ForVariant::Iterate { iterable, body, .. } => {
                validate_task_expression_tree(
                    path,
                    iterable,
                    span,
                    imports,
                    extern_calls,
                    functions,
                    local_method_names,
                    visiting,
                    checked,
                )?;
                validate_task_safe_statements(
                    path,
                    body,
                    imports,
                    extern_calls,
                    functions,
                    local_method_names,
                    visiting,
                    checked,
                )
            }
        },
        Stmt::Defer { stmt, .. } => validate_task_safe_statement(
            path,
            stmt,
            imports,
            extern_calls,
            functions,
            local_method_names,
            visiting,
            checked,
        ),
        Stmt::Postfix { .. }
        | Stmt::Return { value: None, .. }
        | Stmt::Break { .. }
        | Stmt::Continue { .. } => Ok(()),
        Stmt::Unsafe { .. } => unreachable!("unsafe statements are rejected above"),
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_task_safe_statements<'a>(
    path: &Path,
    statements: &[Stmt],
    imports: &[String],
    extern_calls: &HashSet<String>,
    functions: &HashMap<&str, &'a AstFunction>,
    local_method_names: &HashSet<&str>,
    visiting: &mut Vec<&'a str>,
    checked: &mut HashSet<&'a str>,
) -> Result<(), Diagnostic> {
    for statement in statements {
        validate_task_safe_statement(
            path,
            statement,
            imports,
            extern_calls,
            functions,
            local_method_names,
            visiting,
            checked,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_task_expression_tree<'a>(
    path: &Path,
    expression: &AstExpr,
    span: &Span,
    imports: &[String],
    extern_calls: &HashSet<String>,
    functions: &HashMap<&str, &'a AstFunction>,
    local_method_names: &HashSet<&str>,
    visiting: &mut Vec<&'a str>,
    checked: &mut HashSet<&'a str>,
) -> Result<(), Diagnostic> {
    visit_expression(expression, &mut |expression| {
        validate_task_safe_expression(
            path,
            expression,
            span,
            imports,
            extern_calls,
            functions,
            local_method_names,
            visiting,
            checked,
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_task_safe_expression<'a>(
    path: &Path,
    expression: &AstExpr,
    span: &Span,
    imports: &[String],
    extern_calls: &HashSet<String>,
    functions: &HashMap<&str, &'a AstFunction>,
    local_method_names: &HashSet<&str>,
    visiting: &mut Vec<&'a str>,
    checked: &mut HashSet<&'a str>,
) -> Result<(), Diagnostic> {
    let AstExpr::Call { callee, .. } = expression else {
        return Ok(());
    };
    let qualified = resolve_task_call(callee, imports);
    if qualified.len() == 1 {
        let name = &qualified[0];
        if extern_calls.contains(name) {
            return Err(task_safety_call_path_diagnostic(
                path,
                span,
                visiting,
                &format!("extern function `{name}`"),
            ));
        }
        if let Some(callee_function) = functions.get(name.as_str()).copied() {
            return validate_task_safe_function(
                path,
                callee_function,
                imports,
                extern_calls,
                functions,
                local_method_names,
                visiting,
                checked,
            );
        }
        if is_constructor_like_call(name) {
            return Ok(());
        }
        return Err(task_safety_call_path_diagnostic(
            path,
            span,
            visiting,
            &format!("unknown-effect call `{name}`"),
        ));
    }

    if qualified.len() == 2 {
        let module = &qualified[0];
        let operation = &qualified[1];
        if task_safe_standard_call(module, operation) {
            return Ok(());
        }
        if is_constructor_like_call(operation) {
            return Ok(());
        }
        if local_method_names.contains(operation.as_str()) {
            return Err(task_safety_call_path_diagnostic(
                path,
                span,
                visiting,
                &format!("local method `{operation}` with unclassified effects"),
            ));
        }
        if task_safe_value_method(operation) && !is_forbidden_standard_module(module) {
            return Ok(());
        }
        return Err(task_safety_call_path_diagnostic(
            path,
            span,
            visiting,
            &format!("task-unsafe operation `{module}.{operation}`"),
        ));
    }

    Err(task_safety_call_path_diagnostic(
        path,
        span,
        visiting,
        &format!("unknown-effect call `{}`", qualified.join(".")),
    ))
}

fn resolve_task_call(callee: &[String], imports: &[String]) -> Vec<String> {
    if let [name] = callee
        && let Some(qualified) = resolve_specific_value_builtin(name, imports)
    {
        return qualified;
    }
    callee.to_vec()
}

fn is_task_spawn_call(callee: &[String], imports: &[String]) -> bool {
    resolve_task_call(callee, imports) == ["task", "spawn"]
}

fn task_safe_standard_call(module: &str, operation: &str) -> bool {
    match module {
        "array" | "char" | "collections" | "cron" | "crypto" | "hash" | "json" | "jsonrpc"
        | "math" | "num" | "option" | "os" | "path" | "regex" | "result" | "string" => true,
        "http" => matches!(operation, "get" | "post" | "send"),
        "time" => !matches!(operation, "now_millis"),
        "task" => operation == "is_cancelled",
        "Array" => operation == "new",
        _ => false,
    }
}

fn is_forbidden_standard_module(module: &str) -> bool {
    matches!(
        module,
        "debug"
            | "env"
            | "ffi"
            | "fmt"
            | "fs"
            | "io"
            | "log"
            | "net"
            | "process"
            | "sqlite"
            | "testing"
            | "task"
            | "http"
            | "time"
    )
}

fn task_safe_value_method(operation: &str) -> bool {
    matches!(
        operation,
        "and_then"
            | "clear"
            | "concat"
            | "contains"
            | "ends_with"
            | "get"
            | "insert"
            | "is_empty"
            | "is_err"
            | "is_none"
            | "is_ok"
            | "is_some"
            | "iter"
            | "len"
            | "map"
            | "map_err"
            | "pop"
            | "push"
            | "remove"
            | "set"
            | "split"
            | "starts_with"
            | "to_lower"
            | "to_upper"
            | "trim"
            | "unwrap_or"
    )
}

fn is_constructor_like_call(name: &str) -> bool {
    matches!(name, "Ok" | "Err" | "Some" | "None")
        || name.chars().next().is_some_and(char::is_uppercase)
}

pub(super) fn visit_statement_expressions(
    statement: &Stmt,
    visitor: &mut impl FnMut(&AstExpr) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    match statement {
        Stmt::Let { value, .. }
        | Stmt::Assign { value, .. }
        | Stmt::Return {
            value: Some(value), ..
        }
        | Stmt::Expr { expr: value, .. } => visit_expression(value, visitor),
        Stmt::IndexAssign { indices, value, .. } => {
            for index in indices {
                visit_expression(index, visitor)?;
            }
            visit_expression(value, visitor)
        }
        Stmt::LetElse {
            value, else_body, ..
        } => {
            visit_expression(value, visitor)?;
            visit_statements(else_body, visitor)
        }
        Stmt::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            visit_expression(value, visitor)?;
            visit_statements(body, visitor)?;
            if let Some(else_body) = else_body {
                visit_statements(else_body, visitor)?;
            }
            Ok(())
        }
        Stmt::Match { value, arms, .. } => {
            visit_expression(value, visitor)?;
            for arm in arms {
                visit_statements(&arm.body, visitor)?;
            }
            Ok(())
        }
        Stmt::For { variant, .. } => match variant {
            ForVariant::Infinite { body } => visit_statements(body, visitor),
            ForVariant::While { condition, body } => {
                visit_expression(condition, visitor)?;
                visit_statements(body, visitor)
            }
            ForVariant::CStyle {
                initializer,
                condition,
                update,
                body,
                ..
            } => {
                visit_expression(initializer, visitor)?;
                visit_expression(condition, visitor)?;
                visit_statement_expressions(update, visitor)?;
                visit_statements(body, visitor)
            }
            ForVariant::Iterate { iterable, body, .. } => {
                visit_expression(iterable, visitor)?;
                visit_statements(body, visitor)
            }
        },
        Stmt::Defer { stmt, .. } => visit_statement_expressions(stmt, visitor),
        Stmt::Unsafe { body, .. } => visit_statements(body, visitor),
        Stmt::Postfix { .. }
        | Stmt::Return { value: None, .. }
        | Stmt::Break { .. }
        | Stmt::Continue { .. } => Ok(()),
    }
}

fn visit_statements(
    statements: &[Stmt],
    visitor: &mut impl FnMut(&AstExpr) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    for statement in statements {
        visit_statement_expressions(statement, visitor)?;
    }
    Ok(())
}

pub(super) fn visit_expression(
    expression: &AstExpr,
    visitor: &mut impl FnMut(&AstExpr) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    visitor(expression)?;
    match expression {
        AstExpr::ArrayLiteral { elements } => {
            for element in elements {
                visit_expression(element, visitor)?;
            }
        }
        AstExpr::Index { base, index } => {
            visit_expression(base, visitor)?;
            visit_expression(index, visitor)?;
        }
        AstExpr::Call { args, .. } => {
            for arg in args {
                visit_expression(arg, visitor)?;
            }
        }
        AstExpr::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                visit_expression(value, visitor)?;
            }
        }
        AstExpr::Match { value, arms } => {
            visit_expression(value, visitor)?;
            for arm in arms {
                visit_expression(&arm.value, visitor)?;
            }
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            visit_expression(condition, visitor)?;
            visit_expression(then_branch, visitor)?;
            visit_expression(else_branch, visitor)?;
        }
        AstExpr::Panic { message }
        | AstExpr::Question { expr: message }
        | AstExpr::Cast { expr: message, .. }
        | AstExpr::Unary { expr: message, .. } => visit_expression(message, visitor)?,
        AstExpr::Binary { left, right, .. } => {
            visit_expression(left, visitor)?;
            visit_expression(right, visitor)?;
        }
        AstExpr::MutArg { .. }
        | AstExpr::Name(_)
        | AstExpr::String(_)
        | AstExpr::Int(_)
        | AstExpr::Float(_)
        | AstExpr::Char(_)
        | AstExpr::Bool(_)
        | AstExpr::Void => {}
    }
    Ok(())
}

pub(super) fn statement_span(statement: &Stmt) -> &Span {
    match statement {
        Stmt::Let { span, .. }
        | Stmt::LetElse { span, .. }
        | Stmt::IfLet { span, .. }
        | Stmt::Assign { span, .. }
        | Stmt::IndexAssign { span, .. }
        | Stmt::Postfix { span, .. }
        | Stmt::Return { span, .. }
        | Stmt::Match { span, .. }
        | Stmt::Expr { span, .. }
        | Stmt::For { span, .. }
        | Stmt::Break { span }
        | Stmt::Continue { span }
        | Stmt::Defer { span, .. }
        | Stmt::Unsafe { span, .. } => span,
    }
}

fn task_safety_call_path_diagnostic(
    path: &Path,
    span: &Span,
    visiting: &[&str],
    operation: &str,
) -> Diagnostic {
    let mut call_path = visiting.join(" -> ");
    if !call_path.is_empty() {
        call_path.push_str(" -> ");
    }
    call_path.push_str(operation);
    task_safety_diagnostic(
        path,
        span,
        format!("task worker reaches an unsafe effect via {call_path}"),
    )
}

fn task_safety_diagnostic(path: &Path, span: &Span, message: String) -> Diagnostic {
    Diagnostic::new(
        "E0821",
        message,
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    )
}

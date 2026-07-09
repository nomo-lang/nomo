use crate::ast::{AssignOp, BinaryOp, Expr, MatchArm, PostfixOp, TypeRef, UnaryOp};

pub(super) fn type_ref(ty: &TypeRef) -> String {
    if ty.args.is_empty() {
        path(&ty.path)
    } else {
        format!(
            "{}<{}>",
            path(&ty.path),
            ty.args.iter().map(type_ref).collect::<Vec<_>>().join(", ")
        )
    }
}

pub(super) fn expr(value: &Expr, indent: usize, parent_precedence: u8) -> String {
    let precedence = expr_precedence(value);
    let rendered = match value {
        Expr::Call {
            callee,
            type_args,
            args,
        } => {
            let type_args = if type_args.is_empty() {
                String::new()
            } else {
                format!(
                    "<{}>",
                    type_args
                        .iter()
                        .map(type_ref)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            format!(
                "{}{}({})",
                path(callee),
                type_args,
                args.iter()
                    .map(|arg| expr(arg, indent, 0))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        Expr::StructLiteral { type_name, fields } => {
            if fields.is_empty() {
                format!("{} {{}}", path(type_name))
            } else {
                format!(
                    "{} {{ {} }}",
                    path(type_name),
                    fields
                        .iter()
                        .map(|(name, value)| format!("{name}: {}", expr(value, indent, 0)))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        }
        Expr::Match { value, arms } => match_expr(value, arms, indent),
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => if_expr(condition, then_branch, else_branch, indent),
        Expr::Panic { message } => format!("panic({})", expr(message, indent, 0)),
        Expr::Question { expr: inner } => format!("{}?", expr(inner, indent, precedence)),
        Expr::MutArg { name } => format!("mut {}", path(name)),
        Expr::Cast {
            expr: inner,
            target,
        } => format!(
            "{} as {}",
            expr(inner, indent, precedence),
            type_ref(target)
        ),
        Expr::Unary { op, expr: inner } => {
            format!("{}{}", unary_op(op), expr(inner, indent, precedence))
        }
        Expr::Binary { left, op, right } => {
            let op_precedence = binary_precedence(op);
            let left = expr(left, indent, op_precedence);
            let right_precedence = match right.as_ref() {
                Expr::Binary { op: right_op, .. }
                    if binary_precedence(right_op) == op_precedence =>
                {
                    op_precedence + 1
                }
                _ => op_precedence,
            };
            let right = expr(right, indent, right_precedence);
            format!("{left} {} {right}", binary_op(op))
        }
        Expr::Name(name) => path(name),
        Expr::String(value) => format!("\"{}\"", escape_string(value)),
        Expr::Int(value) => value.to_string(),
        Expr::Float(value) => value.clone(),
        Expr::Char(value) => format!("'{}'", escape_char(*value)),
        Expr::Bool(value) => value.to_string(),
        Expr::Void => "void".to_string(),
    };
    if precedence < parent_precedence {
        format!("({rendered})")
    } else {
        rendered
    }
}

fn match_expr(value: &Expr, arms: &[MatchArm], indent: usize) -> String {
    let mut out = format!("match {} {{\n", expr(value, indent, 0));
    for arm in arms {
        out.push_str(&"    ".repeat(indent + 1));
        out.push_str(&format!(
            "{} => {}\n",
            pattern_with_binding(&arm.pattern, arm.binding.as_deref()),
            expr(&arm.value, indent + 1, 0)
        ));
    }
    out.push_str(&"    ".repeat(indent));
    out.push('}');
    out
}

fn if_expr(condition: &Expr, then_branch: &Expr, else_branch: &Expr, indent: usize) -> String {
    format!(
        "if {} {{\n{}{}\n{}}} else {{\n{}{}\n{}}}",
        expr(condition, indent, 0),
        "    ".repeat(indent + 1),
        expr(then_branch, indent + 1, 0),
        "    ".repeat(indent),
        "    ".repeat(indent + 1),
        expr(else_branch, indent + 1, 0),
        "    ".repeat(indent)
    )
}

fn expr_precedence(value: &Expr) -> u8 {
    match value {
        Expr::If { .. } | Expr::Match { .. } => 0,
        Expr::Binary { op, .. } => binary_precedence(op),
        Expr::Cast { .. } => 7,
        Expr::Unary { .. } => 8,
        Expr::Question { .. } => 9,
        Expr::Call { .. }
        | Expr::StructLiteral { .. }
        | Expr::Panic { .. }
        | Expr::MutArg { .. }
        | Expr::Name(_)
        | Expr::String(_)
        | Expr::Int(_)
        | Expr::Float(_)
        | Expr::Char(_)
        | Expr::Bool(_)
        | Expr::Void => 10,
    }
}

fn binary_precedence(op: &BinaryOp) -> u8 {
    match op {
        BinaryOp::LogicalOr => 1,
        BinaryOp::LogicalAnd => 2,
        BinaryOp::Equal | BinaryOp::NotEqual => 3,
        BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual => 4,
        BinaryOp::Add | BinaryOp::Subtract | BinaryOp::BitOr | BinaryOp::BitXor => 5,
        BinaryOp::Multiply
        | BinaryOp::Divide
        | BinaryOp::Remainder
        | BinaryOp::ShiftLeft
        | BinaryOp::ShiftRight
        | BinaryOp::BitAnd
        | BinaryOp::BitAndNot => 6,
    }
}

fn binary_op(op: &BinaryOp) -> &'static str {
    match op {
        BinaryOp::LogicalOr => "||",
        BinaryOp::LogicalAnd => "&&",
        BinaryOp::Add => "+",
        BinaryOp::Subtract => "-",
        BinaryOp::BitOr => "|",
        BinaryOp::BitXor => "^",
        BinaryOp::Multiply => "*",
        BinaryOp::Divide => "/",
        BinaryOp::Remainder => "%",
        BinaryOp::ShiftLeft => "<<",
        BinaryOp::ShiftRight => ">>",
        BinaryOp::BitAnd => "&",
        BinaryOp::BitAndNot => "&^",
        BinaryOp::Equal => "==",
        BinaryOp::NotEqual => "!=",
        BinaryOp::Less => "<",
        BinaryOp::LessEqual => "<=",
        BinaryOp::Greater => ">",
        BinaryOp::GreaterEqual => ">=",
    }
}

fn unary_op(op: &UnaryOp) -> &'static str {
    match op {
        UnaryOp::Not => "!",
        UnaryOp::Negate => "-",
    }
}

pub(super) fn assign_op(op: &AssignOp) -> &'static str {
    match op {
        AssignOp::Assign => "=",
        AssignOp::Add => "+=",
        AssignOp::Subtract => "-=",
        AssignOp::Multiply => "*=",
        AssignOp::Divide => "/=",
        AssignOp::Remainder => "%=",
        AssignOp::ShiftLeft => "<<=",
        AssignOp::ShiftRight => ">>=",
        AssignOp::BitAnd => "&=",
        AssignOp::BitXor => "^=",
        AssignOp::BitOr => "|=",
        AssignOp::BitAndNot => "&^=",
    }
}

pub(super) fn postfix_op(op: &PostfixOp) -> &'static str {
    match op {
        PostfixOp::Increment => "++",
        PostfixOp::Decrement => "--",
    }
}

pub(super) fn pattern_with_binding(pattern: &[String], binding: Option<&str>) -> String {
    match binding {
        Some(binding) => format!("{}({binding})", path(pattern)),
        None => path(pattern),
    }
}

pub(super) fn path(parts: &[String]) -> String {
    parts.join(".")
}

pub(super) fn escape_string(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            '\n' => "\\n".to_string(),
            '\r' => "\\r".to_string(),
            '\t' => "\\t".to_string(),
            '"' => "\\\"".to_string(),
            '\\' => "\\\\".to_string(),
            other => other.to_string(),
        })
        .collect::<String>()
}

fn escape_char(value: char) -> String {
    match value {
        '\n' => "\\n".to_string(),
        '\r' => "\\r".to_string(),
        '\t' => "\\t".to_string(),
        '\'' => "\\'".to_string(),
        '\\' => "\\\\".to_string(),
        other => other.to_string(),
    }
}

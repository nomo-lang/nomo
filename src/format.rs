use crate::ast::{
    BinaryOp, ConstDef, EnumDef, Expr, Field, ForVariant, Function, ImplBlock, MatchArm,
    MatchStmtArm, Param, SourceFile, Stmt, StructDef, TypeRef,
};
use crate::diagnostic::Diagnostic;
use crate::lexer::{Token, TokenKind, lex};
use crate::parser::parse;
use std::path::Path;

pub fn format_source(path: &Path, source: &str) -> Result<String, Diagnostic> {
    if let Some(comment) = first_comment_span(source) {
        return Err(Diagnostic::new(
            "N0109",
            "nomo fmt does not preserve comments yet",
            path,
            comment.line,
            comment.column,
            comment.length,
            &comment.text,
        ));
    }
    let tokens = lex(path, source)?;
    let ast = parse(path, &tokens)?;
    validate_format_ast(path, &ast)?;
    Ok(Formatter::new(&ast, &tokens).format())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CommentSpan {
    line: usize,
    column: usize,
    length: usize,
    text: String,
}

fn first_comment_span(source: &str) -> Option<CommentSpan> {
    for (line_index, line_text) in source.lines().enumerate() {
        let line = line_index + 1;
        let mut chars = line_text.char_indices().peekable();
        while let Some((index, ch)) = chars.next() {
            match ch {
                '"' => skip_string(&mut chars),
                '\'' => skip_char(&mut chars),
                '/' => match chars.peek() {
                    Some((_, '/')) => {
                        return Some(CommentSpan {
                            line,
                            column: index + 1,
                            length: 2,
                            text: line_text.to_string(),
                        });
                    }
                    Some((_, '*')) => {
                        return Some(CommentSpan {
                            line,
                            column: index + 1,
                            length: 2,
                            text: line_text.to_string(),
                        });
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }
    None
}

fn skip_string(chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>) {
    while let Some((_, ch)) = chars.next() {
        if ch == '\\' {
            chars.next();
        } else if ch == '"' {
            break;
        }
    }
}

fn skip_char(chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>) {
    while let Some((_, ch)) = chars.next() {
        if ch == '\\' {
            chars.next();
        } else if ch == '\'' {
            break;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TopLevelItem {
    Struct(usize),
    Enum(usize),
    Impl(usize),
    Const(usize),
    Function(usize),
}

struct Formatter<'a> {
    ast: &'a SourceFile,
    tokens: &'a [Token],
    out: String,
}

impl<'a> Formatter<'a> {
    fn new(ast: &'a SourceFile, tokens: &'a [Token]) -> Self {
        Self {
            ast,
            tokens,
            out: String::new(),
        }
    }

    fn format(mut self) -> String {
        self.line(0, &format!("package {}", path(&self.ast.package)));
        if !self.ast.imports.is_empty() || self.has_top_level_items() {
            self.blank();
        }

        for import in &self.ast.imports {
            self.line(0, &format!("import {}", path(import)));
        }
        if !self.ast.imports.is_empty() && self.has_top_level_items() {
            self.blank();
        }

        let items = top_level_items(self.tokens);
        for (index, item) in items.iter().enumerate() {
            if index > 0 {
                self.blank();
            }
            self.item(*item);
        }
        if !items.is_empty() && !self.ast.script_body.is_empty() {
            self.blank();
        }
        self.stmt_block(&self.ast.script_body, 0);

        if !self.out.ends_with('\n') {
            self.out.push('\n');
        }
        self.out
    }

    fn has_top_level_items(&self) -> bool {
        !(self.ast.structs.is_empty()
            && self.ast.enums.is_empty()
            && self.ast.impls.is_empty()
            && self.ast.consts.is_empty()
            && self.ast.functions.is_empty()
            && self.ast.script_body.is_empty())
    }

    fn item(&mut self, item: TopLevelItem) {
        match item {
            TopLevelItem::Struct(index) => self.struct_def(&self.ast.structs[index]),
            TopLevelItem::Enum(index) => self.enum_def(&self.ast.enums[index]),
            TopLevelItem::Impl(index) => self.impl_block(&self.ast.impls[index]),
            TopLevelItem::Const(index) => self.const_def(&self.ast.consts[index]),
            TopLevelItem::Function(index) => self.function(&self.ast.functions[index], 0, false),
        }
    }

    fn struct_def(&mut self, def: &StructDef) {
        let prefix = if def.public { "pub " } else { "" };
        self.line(
            0,
            &format!(
                "{prefix}struct {}{} {{",
                def.name,
                type_params(&def.type_params)
            ),
        );
        for field in &def.fields {
            self.field(field, 1);
        }
        self.line(0, "}");
    }

    fn field(&mut self, field: &Field, indent: usize) {
        let prefix = if field.public { "pub " } else { "" };
        self.line(
            indent,
            &format!("{prefix}{}: {}", field.name, type_ref(&field.type_ref)),
        );
    }

    fn enum_def(&mut self, def: &EnumDef) {
        let prefix = if def.public { "pub " } else { "" };
        self.line(
            0,
            &format!(
                "{prefix}enum {}{} {{",
                def.name,
                type_params(&def.type_params)
            ),
        );
        for variant in &def.variants {
            match &variant.payload {
                Some(payload) => self.line(1, &format!("{}({})", variant.name, type_ref(payload))),
                None => self.line(1, &variant.name),
            }
        }
        self.line(0, "}");
    }

    fn impl_block(&mut self, block: &ImplBlock) {
        self.line(0, &format!("impl {} {{", type_ref(&block.type_name)));
        for (index, method) in block.methods.iter().enumerate() {
            if index > 0 {
                self.blank();
            }
            self.function(method, 1, true);
        }
        self.line(0, "}");
    }

    fn const_def(&mut self, def: &ConstDef) {
        self.line(
            0,
            &format!(
                "{}const {}: {} = {}",
                if def.public { "pub " } else { "" },
                def.name,
                type_ref(&def.type_ref),
                expr(&def.value, 0, 0)
            ),
        );
    }

    fn function(&mut self, function: &Function, indent: usize, in_impl: bool) {
        let prefix = if function.public { "pub " } else { "" };
        self.line(
            indent,
            &format!(
                "{prefix}fn {}{}({}) -> {} {{",
                function.name,
                type_params(&function.type_params),
                params(&function.params, in_impl),
                type_ref(&function.return_type)
            ),
        );
        self.stmt_block(&function.body, indent + 1);
        self.line(indent, "}");
    }

    fn stmt_block(&mut self, body: &[Stmt], indent: usize) {
        for stmt in body {
            self.stmt(stmt, indent);
        }
    }

    fn stmt(&mut self, stmt: &Stmt, indent: usize) {
        match stmt {
            Stmt::Let {
                name,
                mutable,
                type_annotation,
                value,
                ..
            } => {
                let mutable = if *mutable { "mut " } else { "" };
                let annotation = type_annotation
                    .as_ref()
                    .map(|ty| format!(": {}", type_ref(ty)))
                    .unwrap_or_default();
                self.line(
                    indent,
                    &format!(
                        "let {mutable}{name}{annotation} = {}",
                        expr(value, indent, 0)
                    ),
                );
            }
            Stmt::LetElse {
                pattern,
                binding,
                value,
                else_body,
                ..
            } => {
                self.line(
                    indent,
                    &format!(
                        "let {}({}) = {} else {{",
                        path(pattern),
                        binding,
                        expr(value, indent, 0)
                    ),
                );
                self.stmt_block(else_body, indent + 1);
                self.line(indent, "}");
            }
            Stmt::IfLet {
                pattern,
                binding,
                value,
                body,
                else_body,
                ..
            } => {
                self.line(
                    indent,
                    &format!(
                        "if let {} = {} {{",
                        pattern_with_binding(pattern, binding.as_deref()),
                        expr(value, indent, 0)
                    ),
                );
                self.stmt_block(body, indent + 1);
                if let Some(else_body) = else_body {
                    self.line(indent, "} else {");
                    self.stmt_block(else_body, indent + 1);
                    self.line(indent, "}");
                } else {
                    self.line(indent, "}");
                }
            }
            Stmt::Assign { target, value, .. } => {
                self.line(
                    indent,
                    &format!("{} = {}", path(target), expr(value, indent, 0)),
                );
            }
            Stmt::Return { value, .. } => match value {
                Some(value) => self.line(indent, &format!("return {}", expr(value, indent, 0))),
                None => self.line(indent, "return"),
            },
            Stmt::Match { value, arms, .. } => {
                self.line(indent, &format!("match {} {{", expr(value, indent, 0)));
                for arm in arms {
                    self.match_stmt_arm(arm, indent + 1);
                }
                self.line(indent, "}");
            }
            Stmt::Expr { expr: value, .. } => self.line(indent, &expr(value, indent, 0)),
            Stmt::For { variant, .. } => self.for_stmt(variant, indent),
            Stmt::Break { .. } => self.line(indent, "break"),
            Stmt::Continue { .. } => self.line(indent, "continue"),
            Stmt::Defer { stmt, .. } => match stmt.as_ref() {
                Stmt::Expr { expr: value, .. } => {
                    self.line(indent, &format!("defer {}", expr(value, indent, 0)));
                }
                _ => unreachable!("formatter validates defer statements before printing"),
            },
        }
    }

    fn for_stmt(&mut self, variant: &ForVariant, indent: usize) {
        match variant {
            ForVariant::Infinite { body } => {
                self.line(indent, "for {");
                self.stmt_block(body, indent + 1);
                self.line(indent, "}");
            }
            ForVariant::While { condition, body } => {
                self.line(indent, &format!("for {} {{", expr(condition, indent, 0)));
                self.stmt_block(body, indent + 1);
                self.line(indent, "}");
            }
            ForVariant::Iterate {
                binding,
                iterable,
                body,
            } => {
                self.line(
                    indent,
                    &format!("for {binding} in {} {{", expr(iterable, indent, 0)),
                );
                self.stmt_block(body, indent + 1);
                self.line(indent, "}");
            }
        }
    }

    fn match_stmt_arm(&mut self, arm: &MatchStmtArm, indent: usize) {
        self.line(
            indent,
            &format!(
                "{} => {{",
                pattern_with_binding(&arm.pattern, arm.binding.as_deref())
            ),
        );
        self.stmt_block(&arm.body, indent + 1);
        self.line(indent, "}");
    }

    fn line(&mut self, indent: usize, text: &str) {
        self.out.push_str(&"    ".repeat(indent));
        self.out.push_str(text);
        self.out.push('\n');
    }

    fn blank(&mut self) {
        self.out.push('\n');
    }
}

fn top_level_items(tokens: &[Token]) -> Vec<TopLevelItem> {
    let mut items = Vec::new();
    let mut depth = 0usize;
    let mut structs = 0usize;
    let mut enums = 0usize;
    let mut impls = 0usize;
    let mut consts = 0usize;
    let mut functions = 0usize;
    let mut index = 0usize;

    while let Some(token) = tokens.get(index) {
        if matches!(token.kind, TokenKind::Eof) {
            break;
        }
        if depth == 0 {
            if matches!(token.kind, TokenKind::Pub) {
                if let Some(item) = public_top_level_item(
                    tokens.get(index + 1),
                    &mut structs,
                    &mut enums,
                    &mut consts,
                    &mut functions,
                ) {
                    items.push(item);
                    index += 2;
                    continue;
                }
            } else if let Some(item) = top_level_item(
                &token.kind,
                &mut structs,
                &mut enums,
                &mut impls,
                &mut consts,
                &mut functions,
            ) {
                items.push(item);
                index += 1;
                continue;
            }
        }

        match token.kind {
            TokenKind::LBrace => depth += 1,
            TokenKind::RBrace => depth = depth.saturating_sub(1),
            _ => {}
        }
        index += 1;
    }

    items
}

fn public_top_level_item(
    token: Option<&Token>,
    structs: &mut usize,
    enums: &mut usize,
    consts: &mut usize,
    functions: &mut usize,
) -> Option<TopLevelItem> {
    match token.map(|token| &token.kind) {
        Some(TokenKind::Struct) => {
            let index = *structs;
            *structs += 1;
            Some(TopLevelItem::Struct(index))
        }
        Some(TokenKind::Enum) => {
            let index = *enums;
            *enums += 1;
            Some(TopLevelItem::Enum(index))
        }
        Some(TokenKind::Const) => {
            let index = *consts;
            *consts += 1;
            Some(TopLevelItem::Const(index))
        }
        Some(TokenKind::Fn) => {
            let index = *functions;
            *functions += 1;
            Some(TopLevelItem::Function(index))
        }
        _ => None,
    }
}

fn top_level_item(
    kind: &TokenKind,
    structs: &mut usize,
    enums: &mut usize,
    impls: &mut usize,
    consts: &mut usize,
    functions: &mut usize,
) -> Option<TopLevelItem> {
    match kind {
        TokenKind::Struct => {
            let index = *structs;
            *structs += 1;
            Some(TopLevelItem::Struct(index))
        }
        TokenKind::Enum => {
            let index = *enums;
            *enums += 1;
            Some(TopLevelItem::Enum(index))
        }
        TokenKind::Impl => {
            let index = *impls;
            *impls += 1;
            Some(TopLevelItem::Impl(index))
        }
        TokenKind::Const => {
            let index = *consts;
            *consts += 1;
            Some(TopLevelItem::Const(index))
        }
        TokenKind::Fn => {
            let index = *functions;
            *functions += 1;
            Some(TopLevelItem::Function(index))
        }
        _ => None,
    }
}

fn validate_format_ast(path: &Path, ast: &SourceFile) -> Result<(), Diagnostic> {
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
                | ForVariant::Iterate { body, .. } => validate_stmts(path, body)?,
            },
            Stmt::Defer { stmt, .. } => {
                if !matches!(stmt.as_ref(), Stmt::Expr { .. }) {
                    let span = stmt_span(stmt);
                    return Err(Diagnostic::new(
                        "N0902",
                        "`nomo fmt` cannot safely format non-expression `defer` statements",
                        path,
                        span.line,
                        span.column,
                        span.length,
                        &span.text,
                    ));
                }
            }
            Stmt::Let { .. }
            | Stmt::Assign { .. }
            | Stmt::Return { .. }
            | Stmt::Expr { .. }
            | Stmt::Break { .. }
            | Stmt::Continue { .. } => {}
        }
    }
    Ok(())
}

fn stmt_span(stmt: &Stmt) -> &crate::ast::Span {
    match stmt {
        Stmt::Let { span, .. }
        | Stmt::LetElse { span, .. }
        | Stmt::IfLet { span, .. }
        | Stmt::Assign { span, .. }
        | Stmt::Return { span, .. }
        | Stmt::Match { span, .. }
        | Stmt::Expr { span, .. }
        | Stmt::For { span, .. }
        | Stmt::Break { span, .. }
        | Stmt::Continue { span, .. }
        | Stmt::Defer { span, .. } => span,
    }
}

fn params(params: &[Param], in_impl: bool) -> String {
    params
        .iter()
        .map(|param| {
            let mutable = if param.mutable { "mut " } else { "" };
            if in_impl && param.name == "self" {
                format!("{mutable}self")
            } else {
                format!("{mutable}{}: {}", param.name, type_ref(&param.type_ref))
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn type_params(params: &[String]) -> String {
    if params.is_empty() {
        String::new()
    } else {
        format!("<{}>", params.join(", "))
    }
}

fn type_ref(ty: &TypeRef) -> String {
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

fn expr(value: &Expr, indent: usize, parent_precedence: u8) -> String {
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
        Expr::Try { expr: inner } => format!("{}?", expr(inner, indent, precedence)),
        Expr::MutArg { name } => format!("mut {}", path(name)),
        Expr::Cast {
            expr: inner,
            target,
        } => format!(
            "{} as {}",
            expr(inner, indent, precedence),
            type_ref(target)
        ),
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
        Expr::Cast { .. } => 4,
        Expr::Try { .. } => 5,
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
        | Expr::Void => 6,
    }
}

fn binary_precedence(op: &BinaryOp) -> u8 {
    match op {
        BinaryOp::Equal | BinaryOp::NotEqual => 1,
        BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual => 2,
        BinaryOp::Add => 3,
    }
}

fn binary_op(op: &BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Equal => "==",
        BinaryOp::NotEqual => "!=",
        BinaryOp::Less => "<",
        BinaryOp::LessEqual => "<=",
        BinaryOp::Greater => ">",
        BinaryOp::GreaterEqual => ">=",
    }
}

fn pattern_with_binding(pattern: &[String], binding: Option<&str>) -> String {
    match binding {
        Some(binding) => format!("{}({binding})", path(pattern)),
        None => path(pattern),
    }
}

fn path(parts: &[String]) -> String {
    parts.join(".")
}

fn escape_string(value: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_messy_source_to_canonical_source() {
        let source = "package app . main\nimport std . io\n\npub struct User{\npub id:string\nemail:string\n}\nconst MAX:i32=100\npub enum State<T>{\nReady\nDone(T)\n}\nimpl User{\npub fn get_email(self)->string{\nreturn self.email\n}\n}\nfn label(value:State<i32>)->string{\nreturn match value{\nState.Ready=>\"ready\"\nState.Done(code)=>\"done\"\n}\n}\nfn main(){\nlet mut count:i32=1\ncount=count+1\nif let State.Done(code)=State.Done(count){\nreturn\n}else{\ndefer io.println(\"missing\")\n}\nfor item in items{\nbreak\ncontinue\n}\n}\n";
        let formatted = format_source(Path::new("main.nomo"), source).unwrap();

        assert_eq!(
            formatted,
            "package app.main\n\nimport std.io\n\npub struct User {\n    pub id: string\n    email: string\n}\n\nconst MAX: i32 = 100\n\npub enum State<T> {\n    Ready\n    Done(T)\n}\n\nimpl User {\n    pub fn get_email(self) -> string {\n        return self.email\n    }\n}\n\nfn label(value: State<i32>) -> string {\n    return match value {\n        State.Ready => \"ready\"\n        State.Done(code) => \"done\"\n    }\n}\n\nfn main() -> void {\n    let mut count: i32 = 1\n    count = count + 1\n    if let State.Done(code) = State.Done(count) {\n        return\n    } else {\n        defer io.println(\"missing\")\n    }\n    for item in items {\n        break\n        continue\n    }\n}\n"
        );
    }

    #[test]
    fn formatting_is_idempotent() {
        let source = "package app.main\n\nfn add(left: i32, right: i32) -> i32 {\n    return left + right\n}\n";
        let once = format_source(Path::new("main.nomo"), source).unwrap();
        let twice = format_source(Path::new("main.nomo"), &once).unwrap();

        assert_eq!(once, twice);
    }

    #[test]
    fn formats_expr_variants_and_escaping() {
        let source = "package app.main\n\nfn main() -> void {\n    let point: Point = Point {\n        x: 1,\n        y: 2,\n    }\n    let ok: bool = left < right == false\n    let ratio: f64 = total as f64\n    let text: string = \"a\\n\\\"b\"\n    let letter: char = '\\n'\n    panic(text)\n}\n";
        let formatted = format_source(Path::new("main.nomo"), source).unwrap();

        assert!(formatted.contains("Point { x: 1, y: 2 }"));
        assert!(formatted.contains("left < right == false"));
        assert!(formatted.contains("total as f64"));
        assert!(formatted.contains("\"a\\n\\\"b\""));
        assert!(formatted.contains("'\\n'"));
    }

    #[test]
    fn rejects_unformattable_non_expression_defer() {
        let source = "package app.main\n\nfn main() -> void {\n    defer let value: i32 = 1\n}\n";
        let err = format_source(Path::new("main.nomo"), source).unwrap_err();

        assert_eq!(err.code, "N0902");
        assert!(err.message.contains("non-expression `defer`"));
        assert_eq!(err.line, 4);
        assert_eq!(err.column, 11);
    }

    #[test]
    fn rejects_comments_without_dropping_them() {
        let source = "package app.main\n// keep me\nfn main() -> void {\n    return\n}\n";
        let err = format_source(Path::new("main.nomo"), source).unwrap_err();

        assert_eq!(err.code, "N0109");
        assert_eq!(err.message, "nomo fmt does not preserve comments yet");
        assert_eq!(err.line, 2);
        assert_eq!(err.column, 1);
    }

    #[test]
    fn comment_markers_inside_strings_are_formattable() {
        let source = "package app.main\n\nfn main() -> void {\nlet url:string=\"http://example.test/*literal*/\"\n}\n";
        let formatted = format_source(Path::new("main.nomo"), source).unwrap();

        assert!(formatted.contains("\"http://example.test/*literal*/\""));
    }

    #[test]
    fn formats_top_level_script_statements() {
        let source =
            "package app.main\nimport std.io\nlet message:string=\"hi\"\nio.println(message)\n";
        let formatted = format_source(Path::new("script.nomo"), source).unwrap();

        assert_eq!(
            formatted,
            "package app.main\n\nimport std.io\n\nlet message: string = \"hi\"\nio.println(message)\n"
        );
    }
}

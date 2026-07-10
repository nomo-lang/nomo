#![allow(clippy::result_large_err, clippy::too_many_arguments)]

use nomo_diagnostics::Diagnostic;
use nomo_syntax::ast::{
    ConstDef, EnumDef, ExternBlock, Field, ForVariant, Function, FunctionSignature, ImplBlock,
    InterfaceDef, MatchStmtArm, Param, SourceFile, Stmt, StructDef, TypeParamBound,
};
use nomo_syntax::lexer::{Token, lex};
use nomo_syntax::parser::parse;
use std::collections::BTreeSet;
use std::path::Path;

mod expr;
mod layout;
mod trivia;
mod validate;

use expr::{assign_op, escape_string, expr, path, pattern_with_binding, postfix_op, type_ref};
use layout::{TokenLayout, TopLevelItem, top_level_items};
use trivia::{FormatTrivia, collect_trivia};
use validate::{stmt_line, validate_format_ast};

pub fn format_source(path: &Path, source: &str) -> Result<String, Diagnostic> {
    let tokens = lex(path, source)?;
    let ast = parse(path, &tokens)?;
    validate_format_ast(path, &ast)?;
    Ok(Formatter::new(&ast, &tokens, collect_trivia(source)).format())
}

struct Formatter<'a> {
    ast: &'a SourceFile,
    tokens: &'a [Token],
    trivia: FormatTrivia,
    package_line: usize,
    import_lines: Vec<usize>,
    impl_lines: Vec<usize>,
    struct_field_lines: Vec<Vec<usize>>,
    enum_variant_lines: Vec<Vec<usize>>,
    emitted_leading: BTreeSet<usize>,
    out: String,
}

impl<'a> Formatter<'a> {
    fn new(ast: &'a SourceFile, tokens: &'a [Token], trivia: FormatTrivia) -> Self {
        let layout = TokenLayout::from_tokens(tokens);
        Self {
            ast,
            tokens,
            trivia,
            package_line: layout.package_line,
            import_lines: layout.import_lines,
            impl_lines: layout.impl_lines,
            struct_field_lines: layout.struct_field_lines,
            enum_variant_lines: layout.enum_variant_lines,
            emitted_leading: BTreeSet::new(),
            out: String::new(),
        }
    }

    fn format(mut self) -> String {
        self.line_at(
            0,
            &format!("package {}", path(&self.ast.package)),
            self.package_line,
        );
        if !self.ast.imports.is_empty() || self.has_top_level_items() {
            self.blank();
        }

        for (index, import) in self.ast.imports.iter().enumerate() {
            self.line_at(
                0,
                &format!("import {}", path(import)),
                self.import_lines
                    .get(index)
                    .copied()
                    .unwrap_or(self.package_line),
            );
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
        self.emit_remaining_comments(0);

        if !self.out.ends_with('\n') {
            self.out.push('\n');
        }
        self.out
    }

    fn has_top_level_items(&self) -> bool {
        !(self.ast.structs.is_empty()
            && self.ast.enums.is_empty()
            && self.ast.interfaces.is_empty()
            && self.ast.extern_blocks.is_empty()
            && self.ast.impls.is_empty()
            && self.ast.consts.is_empty()
            && self.ast.functions.is_empty()
            && self.ast.script_body.is_empty())
    }

    fn item(&mut self, item: TopLevelItem) {
        match item {
            TopLevelItem::Struct(index) => self.struct_def(index, &self.ast.structs[index]),
            TopLevelItem::Enum(index) => self.enum_def(index, &self.ast.enums[index]),
            TopLevelItem::Interface(index) => self.interface_def(&self.ast.interfaces[index]),
            TopLevelItem::ExternBlock(index) => self.extern_block(&self.ast.extern_blocks[index]),
            TopLevelItem::Impl(index) => self.impl_block(index, &self.ast.impls[index]),
            TopLevelItem::Const(index) => self.const_def(&self.ast.consts[index]),
            TopLevelItem::Function(index) => self.function(&self.ast.functions[index], 0, false),
        }
    }

    fn struct_def(&mut self, index: usize, def: &StructDef) {
        let prefix = if def.public { "pub " } else { "" };
        self.line_at(
            0,
            &format!(
                "{prefix}struct {}{} {{",
                def.name,
                type_params(&def.type_params)
            ),
            def.span.line,
        );
        for (field_index, field) in def.fields.iter().enumerate() {
            let source_line = self
                .struct_field_lines
                .get(index)
                .and_then(|lines| lines.get(field_index))
                .copied()
                .unwrap_or(def.span.line);
            self.field(field, 1, source_line);
        }
        self.line(0, "}");
    }

    fn field(&mut self, field: &Field, indent: usize, source_line: usize) {
        let prefix = if field.public { "pub " } else { "" };
        self.line_at(
            indent,
            &format!("{prefix}{}: {}", field.name, type_ref(&field.type_ref)),
            source_line,
        );
    }

    fn enum_def(&mut self, index: usize, def: &EnumDef) {
        let prefix = if def.public { "pub " } else { "" };
        self.line_at(
            0,
            &format!(
                "{prefix}enum {}{} {{",
                def.name,
                type_params(&def.type_params)
            ),
            def.span.line,
        );
        for (variant_index, variant) in def.variants.iter().enumerate() {
            let source_line = self
                .enum_variant_lines
                .get(index)
                .and_then(|lines| lines.get(variant_index))
                .copied()
                .unwrap_or(def.span.line);
            match &variant.payload {
                Some(payload) => self.line_at(
                    1,
                    &format!("{}({})", variant.name, type_ref(payload)),
                    source_line,
                ),
                None => self.line_at(1, &variant.name, source_line),
            }
        }
        self.line(0, "}");
    }

    fn interface_def(&mut self, def: &InterfaceDef) {
        let prefix = if def.public { "pub " } else { "" };
        self.line_at(
            0,
            &format!("{prefix}interface {} {{", def.name),
            def.span.line,
        );
        for method in &def.methods {
            self.signature(method, 1, true);
        }
        self.line(0, "}");
    }

    fn extern_block(&mut self, block: &ExternBlock) {
        self.line_at(
            0,
            &format!("extern \"{}\" {{", escape_string(&block.abi)),
            block.span.line,
        );
        for function in &block.functions {
            self.signature(function, 1, false);
        }
        self.line(0, "}");
    }

    fn signature(&mut self, signature: &FunctionSignature, indent: usize, in_interface: bool) {
        self.line_at(
            indent,
            &format!(
                "fn {}{}({}) -> {}",
                signature.name,
                type_params_with_bounds(&signature.type_params, &signature.type_param_bounds),
                params(&signature.params, in_interface),
                type_ref(&signature.return_type)
            ),
            signature.span.line,
        );
    }

    fn impl_block(&mut self, index: usize, block: &ImplBlock) {
        let target = match &block.interface_name {
            Some(interface_name) => {
                format!(
                    "{} for {}",
                    type_ref(interface_name),
                    type_ref(&block.type_name)
                )
            }
            None => type_ref(&block.type_name),
        };
        self.line_at(
            0,
            &format!("impl {target} {{"),
            self.impl_lines
                .get(index)
                .copied()
                .unwrap_or(self.package_line),
        );
        for (index, method) in block.methods.iter().enumerate() {
            if index > 0 {
                self.blank();
            }
            self.function(method, 1, true);
        }
        self.line(0, "}");
    }

    fn const_def(&mut self, def: &ConstDef) {
        self.line_at(
            0,
            &format!(
                "{}const {}: {} = {}",
                if def.public { "pub " } else { "" },
                def.name,
                type_ref(&def.type_ref),
                expr(&def.value, 0, 0)
            ),
            def.span.line,
        );
    }

    fn function(&mut self, function: &Function, indent: usize, in_impl: bool) {
        if function.is_test {
            self.emit_leading_comments(function.span.line, indent);
            self.line(indent, "#[test]");
        }
        let prefix = if function.public { "pub " } else { "" };
        self.line_at(
            indent,
            &format!(
                "{prefix}fn {}{}({}) -> {} {{",
                function.name,
                type_params_with_bounds(&function.type_params, &function.type_param_bounds),
                params(&function.params, in_impl),
                type_ref(&function.return_type)
            ),
            function.span.line,
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
                self.line_at(
                    indent,
                    &format!(
                        "let {mutable}{name}{annotation} = {}",
                        expr(value, indent, 0)
                    ),
                    stmt_line(stmt),
                );
            }
            Stmt::LetElse {
                pattern,
                binding,
                value,
                else_body,
                ..
            } => {
                self.line_at(
                    indent,
                    &format!(
                        "let {}({}) = {} else {{",
                        path(pattern),
                        binding,
                        expr(value, indent, 0)
                    ),
                    stmt_line(stmt),
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
                self.line_at(
                    indent,
                    &format!(
                        "if let {} = {} {{",
                        pattern_with_binding(pattern, binding.as_deref()),
                        expr(value, indent, 0)
                    ),
                    stmt_line(stmt),
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
            Stmt::Assign {
                target, op, value, ..
            } => {
                self.line_at(
                    indent,
                    &format!(
                        "{} {} {}",
                        path(target),
                        assign_op(op),
                        expr(value, indent, 0)
                    ),
                    stmt_line(stmt),
                );
            }
            Stmt::Postfix { target, op, .. } => {
                self.line_at(
                    indent,
                    &format!("{}{}", path(target), postfix_op(op)),
                    stmt_line(stmt),
                );
            }
            Stmt::Return { value, .. } => match value {
                Some(value) => self.line_at(
                    indent,
                    &format!("return {}", expr(value, indent, 0)),
                    stmt_line(stmt),
                ),
                None => self.line_at(indent, "return", stmt_line(stmt)),
            },
            Stmt::Match { value, arms, .. } => {
                self.line_at(
                    indent,
                    &format!("match {} {{", expr(value, indent, 0)),
                    stmt_line(stmt),
                );
                for arm in arms {
                    self.match_stmt_arm(arm, indent + 1);
                }
                self.line(indent, "}");
            }
            Stmt::Expr { expr: value, .. } => {
                self.line_at(indent, &expr(value, indent, 0), stmt_line(stmt));
            }
            Stmt::For { variant, .. } => self.for_stmt(variant, indent, stmt_line(stmt)),
            Stmt::Break { .. } => self.line_at(indent, "break", stmt_line(stmt)),
            Stmt::Continue { .. } => self.line_at(indent, "continue", stmt_line(stmt)),
            Stmt::Defer { stmt, .. } => match stmt.as_ref() {
                Stmt::Expr { expr: value, .. } => {
                    self.line_at(
                        indent,
                        &format!("defer {}", expr(value, indent, 0)),
                        stmt_line(stmt),
                    );
                }
                _ => unreachable!("formatter validates defer statements before printing"),
            },
            Stmt::Unsafe { body, .. } => {
                self.line_at(indent, "unsafe {", stmt_line(stmt));
                self.stmt_block(body, indent + 1);
                self.line(indent, "}");
            }
        }
    }

    fn for_stmt(&mut self, variant: &ForVariant, indent: usize, source_line: usize) {
        match variant {
            ForVariant::Infinite { body } => {
                self.line_at(indent, "for {", source_line);
                self.stmt_block(body, indent + 1);
                self.line(indent, "}");
            }
            ForVariant::While { condition, body } => {
                self.line_at(
                    indent,
                    &format!("for {} {{", expr(condition, indent, 0)),
                    source_line,
                );
                self.stmt_block(body, indent + 1);
                self.line(indent, "}");
            }
            ForVariant::Iterate {
                binding,
                iterable,
                body,
            } => {
                self.line_at(
                    indent,
                    &format!("for {binding} in {} {{", expr(iterable, indent, 0)),
                    source_line,
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

    fn line_at(&mut self, indent: usize, text: &str, source_line: usize) {
        self.emit_leading_comments(source_line, indent);
        self.out.push_str(&"    ".repeat(indent));
        self.out.push_str(text);
        if let Some(comment) = self.trivia.trailing.get(&source_line) {
            self.out.push(' ');
            self.out.push_str(comment);
        }
        self.out.push('\n');
    }

    fn emit_leading_comments(&mut self, source_line: usize, indent: usize) {
        let keys = self
            .trivia
            .leading
            .range(..=source_line)
            .map(|(line, _)| *line)
            .collect::<Vec<_>>();
        for key in keys {
            if !self.emitted_leading.insert(key) {
                continue;
            }
            let Some(lines) = self.trivia.leading.get(&key) else {
                continue;
            };
            for comment in lines.clone() {
                self.out.push_str(&"    ".repeat(indent));
                self.out.push_str(comment.trim_start());
                self.out.push('\n');
            }
        }
    }

    fn emit_remaining_comments(&mut self, indent: usize) {
        let keys = self.trivia.leading.keys().copied().collect::<Vec<_>>();
        for key in keys {
            if !self.emitted_leading.insert(key) {
                continue;
            }
            let Some(lines) = self.trivia.leading.get(&key) else {
                continue;
            };
            for comment in lines.clone() {
                self.out.push_str(&"    ".repeat(indent));
                self.out.push_str(comment.trim_start());
                self.out.push('\n');
            }
        }
    }

    fn blank(&mut self) {
        self.out.push('\n');
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

fn type_params_with_bounds(params: &[String], bounds: &[TypeParamBound]) -> String {
    if params.is_empty() {
        return String::new();
    }
    let params = params
        .iter()
        .map(|parameter| {
            bounds
                .iter()
                .find(|bound| &bound.parameter == parameter)
                .map(|bound| format!("{parameter}: {}", type_ref(&bound.interface)))
                .unwrap_or_else(|| parameter.clone())
        })
        .collect::<Vec<_>>();
    format!("<{}>", params.join(", "))
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
    fn preserves_test_attributes() {
        let source = "package app.main\n\n#[test]\nfn adds_numbers(){\n}\n";
        let formatted = format_source(Path::new("main.nomo"), source).unwrap();
        let twice = format_source(Path::new("main.nomo"), &formatted).unwrap();

        assert_eq!(formatted, twice);
        assert!(formatted.contains("#[test]\nfn adds_numbers() -> void"));
    }

    #[test]
    fn formats_expr_variants_and_escaping() {
        let source = "package app.main\n\nfn main() -> void {\n    let point: Point = Point {\n        x: 1,\n        y: 2,\n    }\n    let ok: bool = !left&&right||fallback\n    let value: i64 = a-b*c/d%e\n    let mask: i64 = a&b&^c<<d>>e|f^g\n    let ratio: f64 = total as f64\n    let grouped: i64 = (a+b)*-(c-d)\n    let text: string = \"a\\n\\\"b\"\n    let letter: char = '\\n'\n    panic(text)\n}\n";
        let formatted = format_source(Path::new("main.nomo"), source).unwrap();

        assert!(formatted.contains("Point { x: 1, y: 2 }"));
        assert!(formatted.contains("!left && right || fallback"));
        assert!(formatted.contains("a - b * c / d % e"));
        assert!(formatted.contains("a & b &^ c << d >> e | f ^ g"));
        assert!(formatted.contains("total as f64"));
        assert!(formatted.contains("(a + b) * -(c - d)"));
        assert!(formatted.contains("\"a\\n\\\"b\""));
        assert!(formatted.contains("'\\n'"));
    }

    #[test]
    fn formats_question_propagation() {
        let source = "package app.main\n\nfn load_value()->Result<string,string>{\nreturn Ok(\"value\")\n}\n\nfn compute()->Result<string,string>{\nlet value:string=load_value()?\ndefer cleanup(load_value()?)\nreturn Ok(load_value()?)\n}\n";
        let formatted = format_source(Path::new("main.nomo"), source).unwrap();
        let twice = format_source(Path::new("main.nomo"), &formatted).unwrap();

        assert_eq!(formatted, twice);
        assert_eq!(
            formatted,
            "package app.main\n\nfn load_value() -> Result<string, string> {\n    return Ok(\"value\")\n}\n\nfn compute() -> Result<string, string> {\n    let value: string = load_value()?\n    defer cleanup(load_value()?)\n    return Ok(load_value()?)\n}\n"
        );
    }

    #[test]
    fn formats_compound_assignment_operators() {
        let source = "package app.main\n\nfn main() -> void {\nlet mut value:i64=1\nvalue+=2\nvalue-=1\nvalue*=3\nvalue/=2\nvalue%=2\nvalue<<=1\nvalue>>=1\nvalue&=6\nvalue^=3\nvalue|=8\nvalue&^=1\n}\n";
        let formatted = format_source(Path::new("main.nomo"), source).unwrap();

        for line in [
            "value += 2",
            "value -= 1",
            "value *= 3",
            "value /= 2",
            "value %= 2",
            "value <<= 1",
            "value >>= 1",
            "value &= 6",
            "value ^= 3",
            "value |= 8",
            "value &^= 1",
        ] {
            assert!(formatted.contains(line), "{formatted}");
        }
    }

    #[test]
    fn formats_postfix_update_operators() {
        let source =
            "package app.main\n\nfn main() -> void {\nlet mut value:i64=1\nvalue++\nvalue--\n}\n";
        let formatted = format_source(Path::new("main.nomo"), source).unwrap();

        assert!(formatted.contains("value++"));
        assert!(formatted.contains("value--"));
    }

    #[test]
    fn rejects_unformattable_non_expression_defer() {
        let source = "package app.main\n\nfn main() -> void {\n    defer let value: i32 = 1\n}\n";
        let err = format_source(Path::new("main.nomo"), source).unwrap_err();

        assert_eq!(err.code, "E0902");
        assert!(err.message.contains("non-expression `defer`"));
        assert_eq!(err.line, 4);
        assert_eq!(err.column, 11);
    }

    #[test]
    fn preserves_line_comments_without_dropping_them() {
        let source = "package app.main\n// keep me\nfn main() -> void {\n    return\n}\n";
        let formatted = format_source(Path::new("main.nomo"), source).unwrap();

        assert_eq!(
            formatted,
            "package app.main\n\n// keep me\nfn main() -> void {\n    return\n}\n"
        );
    }

    #[test]
    fn preserves_doc_comments_field_comments_and_trailing_comments() {
        let source = "package app.main\n\n/// User record\npub struct User{\n/// Stable id\npub id:string // visible id\n}\n\nfn main(){\nlet value:i32=1 // one\n}\n";
        let formatted = format_source(Path::new("main.nomo"), source).unwrap();
        let twice = format_source(Path::new("main.nomo"), &formatted).unwrap();

        assert_eq!(formatted, twice);
        assert!(formatted.contains("/// User record\npub struct User {"));
        assert!(formatted.contains("    /// Stable id\n    pub id: string // visible id"));
        assert!(formatted.contains("    let value: i32 = 1 // one"));
    }

    #[test]
    fn preserves_nested_block_comments() {
        let source = "package app.main\n/* outer\n/* inner */\nend */\nfn main(){\nreturn\n}\n";
        let formatted = format_source(Path::new("main.nomo"), source).unwrap();

        assert!(formatted.contains("/* outer\n/* inner */\nend */\nfn main() -> void"));
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

    #[test]
    fn formats_interface_extern_impl_and_unsafe_blocks() {
        let source = "package app.main\nimport std.ffi\n\npub interface Display{\nfn to_string(self)->string\n}\n\nextern \"C\"{\nfn puts(message:CString)->i32\n}\n\nstruct User{\nname:string\n}\n\nimpl Display for User{\nfn to_string(self)->string{\nreturn self.name\n}\n}\n\nfn main(){\nlet user:User=User{name:\"ok\"}\nlet message:CString=CString.from_string(user.to_string())\nunsafe{\nputs(message)\n}\n}\n";
        let formatted = format_source(Path::new("main.nomo"), source).unwrap();
        let twice = format_source(Path::new("main.nomo"), &formatted).unwrap();

        assert_eq!(formatted, twice);
        assert_eq!(
            formatted,
            "package app.main\n\nimport std.ffi\n\npub interface Display {\n    fn to_string(self) -> string\n}\n\nextern \"C\" {\n    fn puts(message: CString) -> i32\n}\n\nstruct User {\n    name: string\n}\n\nimpl Display for User {\n    fn to_string(self) -> string {\n        return self.name\n    }\n}\n\nfn main() -> void {\n    let user: User = User { name: \"ok\" }\n    let message: CString = CString.from_string(user.to_string())\n    unsafe {\n        puts(message)\n    }\n}\n"
        );
    }

    #[test]
    fn preserves_generic_interface_bounds() {
        let source = "package app.main\n\ninterface Display{\nfn to_string(self)->string\n}\n\nfn render<T:Display>(value:T)->string{\nreturn value.to_string()\n}\n";

        let formatted = format_source(Path::new("main.nomo"), source).unwrap();

        assert_eq!(
            formatted,
            "package app.main\n\ninterface Display {\n    fn to_string(self) -> string\n}\n\nfn render<T: Display>(value: T) -> string {\n    return value.to_string()\n}\n"
        );
        assert_eq!(
            format_source(Path::new("main.nomo"), &formatted).unwrap(),
            formatted
        );
    }
}

use crate::ast::{
    AssignOp, BinaryOp, ConstDef, EnumDef, EnumVariant, Expr, ExternBlock, ExternOpaqueType, Field,
    ForVariant, Function, FunctionSignature, ImplBlock, InterfaceDef, MatchArm, MatchStmtArm,
    Param, PostfixOp, SourceFile, Span, Stmt, StructDef, TypeRef,
};
use crate::diagnostic::Diagnostic;
use crate::lexer::{Token, TokenKind};
use std::path::Path;

pub fn parse(path: &Path, tokens: &[Token]) -> Result<SourceFile, Diagnostic> {
    Parser {
        path,
        tokens,
        index: 0,
        allow_struct_literals: true,
        impl_self_type: None,
        pending_type_gt: 0,
    }
    .parse_source_file()
}

struct Parser<'a> {
    path: &'a Path,
    tokens: &'a [Token],
    index: usize,
    allow_struct_literals: bool,
    impl_self_type: Option<TypeRef>,
    pending_type_gt: usize,
}

#[path = "parser_decl.rs"]
mod parser_decl;
#[path = "parser_expr.rs"]
mod parser_expr;
#[path = "parser_stmt.rs"]
mod parser_stmt;

impl Parser<'_> {
    fn parse_source_file(&mut self) -> Result<SourceFile, Diagnostic> {
        self.skip_newlines();
        self.expect_kind(TokenKind::Package, "E0200", "expected `package <name>`")?;
        let package = self.parse_path()?;
        self.expect_newline("expected newline after package declaration")?;

        let mut imports = Vec::new();
        loop {
            self.skip_newlines();
            if !matches!(self.peek().kind, TokenKind::Import) {
                break;
            }
            self.advance();
            imports.push(self.parse_import_path()?);
            self.expect_newline("expected newline after import declaration")?;
        }

        let mut structs = Vec::new();
        let mut enums = Vec::new();
        let mut interfaces = Vec::new();
        let mut extern_opaque_types = Vec::new();
        let mut extern_blocks = Vec::new();
        let mut impls = Vec::new();
        let mut consts = Vec::new();
        let mut functions = Vec::new();
        let mut script_body = Vec::new();
        let mut parsing_script_body = false;
        loop {
            self.skip_newlines();
            let attributes = self.parse_declaration_attributes()?;
            let is_test = attributes.is_test;
            let public = self.consume_pub();
            if is_test && !matches!(self.peek().kind, TokenKind::Fn) {
                return Err(self.error(
                    "E1100",
                    "`#[test]` can only be applied to a function",
                    self.peek().length(),
                ));
            }
            if attributes.repr_c && !matches!(self.peek().kind, TokenKind::Struct) {
                return Err(self.error(
                    "E1100",
                    "`#[repr(C)]` can only be applied to a struct",
                    self.peek().length(),
                ));
            }
            if parsing_script_body && is_declaration_start(&self.peek().kind, public) {
                return Err(self.error(
                    "E0201",
                    "declarations must appear before top-level script statements",
                    self.peek().length(),
                ));
            }
            match self.peek().kind {
                TokenKind::Struct if !parsing_script_body => {
                    structs.push(self.parse_struct(public, attributes.repr_c)?)
                }
                TokenKind::Enum if !parsing_script_body => enums.push(self.parse_enum(public)?),
                TokenKind::Interface if !parsing_script_body => {
                    interfaces.push(self.parse_interface(public)?)
                }
                TokenKind::Extern if !public && !parsing_script_body => {
                    if self.is_extern_opaque_type() {
                        extern_opaque_types.push(self.parse_extern_opaque_type()?);
                    } else {
                        extern_blocks.push(self.parse_extern_block()?);
                    }
                }
                TokenKind::Impl if !public && !parsing_script_body => {
                    impls.push(self.parse_impl()?)
                }
                TokenKind::Const if !parsing_script_body => consts.push(self.parse_const(public)?),
                TokenKind::Fn if !parsing_script_body => {
                    functions.push(self.parse_function(public, is_test)?)
                }
                TokenKind::Eof if !public && !is_test && !attributes.repr_c => break,
                _ if public => {
                    return Err(self.error(
                        "E0201",
                        "expected struct, enum, impl, const, function declaration, or end of file",
                        self.peek().length(),
                    ));
                }
                _ if is_test => {
                    return Err(self.error(
                        "E1100",
                        "`#[test]` can only be applied to a function",
                        self.peek().length(),
                    ));
                }
                _ => {
                    parsing_script_body = true;
                    script_body.push(self.parse_stmt()?);
                    self.expect_newline("expected newline after top-level script statement")?;
                }
            }
        }

        let package_for_items = package.clone();
        for item in &mut structs {
            item.package = package_for_items.clone();
        }
        for item in &mut enums {
            item.package = package_for_items.clone();
        }
        for function in &mut functions {
            function.package = package_for_items.clone();
        }
        for impl_block in &mut impls {
            for method in &mut impl_block.methods {
                method.package = package_for_items.clone();
            }
        }

        Ok(SourceFile {
            package,
            imports,
            structs,
            enums,
            interfaces,
            extern_opaque_types,
            extern_blocks,
            impls,
            consts,
            functions,
            script_body,
        })
    }

    fn parse_type_ref(&mut self) -> Result<TypeRef, Diagnostic> {
        if matches!(self.peek().kind, TokenKind::Extern) {
            return self.parse_extern_c_callback_type_ref();
        }
        if matches!(self.peek().kind, TokenKind::Void) {
            self.advance();
            return Ok(TypeRef {
                path: vec!["void".to_string()],
                args: Vec::new(),
            });
        }
        let path = self.parse_path()?;
        let args = self.parse_type_args()?;
        Ok(TypeRef { path, args })
    }

    fn parse_extern_c_callback_type_ref(&mut self) -> Result<TypeRef, Diagnostic> {
        self.expect_kind(TokenKind::Extern, "E1524", "expected `extern`")?;
        match self.peek().kind.clone() {
            TokenKind::String(abi) if abi == "C" => self.advance(),
            _ => return Err(self.error("E1524", "expected callback ABI string `\"C\"`", 1)),
        }
        self.expect_kind(TokenKind::Fn, "E1524", "expected `fn` in callback type")?;
        self.expect_kind(
            TokenKind::LParen,
            "E1524",
            "expected `(` before callback parameter types",
        )?;
        let mut args = Vec::new();
        if !matches!(self.peek().kind, TokenKind::RParen) {
            loop {
                args.push(self.parse_type_ref()?);
                match self.peek().kind {
                    TokenKind::Comma => self.advance(),
                    TokenKind::RParen => break,
                    _ => {
                        return Err(self.error(
                            "E1524",
                            "expected `,` or `)` after callback parameter type",
                            self.peek().length(),
                        ));
                    }
                }
            }
        }
        self.expect_kind(
            TokenKind::RParen,
            "E1524",
            "expected `)` after callback parameter types",
        )?;
        self.expect_kind(TokenKind::Arrow, "E1524", "expected `->` in callback type")?;
        args.push(self.parse_type_ref()?);
        Ok(TypeRef {
            path: vec![crate::ast::EXTERN_C_CALLBACK_TYPE_PATH.to_string()],
            args,
        })
    }

    fn parse_type_args(&mut self) -> Result<Vec<TypeRef>, Diagnostic> {
        if !matches!(self.peek().kind, TokenKind::Less) {
            return Ok(Vec::new());
        }
        self.advance();
        let mut args = Vec::new();
        self.skip_newlines();
        loop {
            args.push(self.parse_type_ref()?);
            if self.pending_type_gt > 0 {
                self.pending_type_gt -= 1;
                break;
            }
            self.skip_newlines();
            match self.peek().kind {
                TokenKind::Comma => {
                    self.advance();
                    self.skip_newlines();
                }
                TokenKind::Greater => {
                    self.advance();
                    break;
                }
                TokenKind::GreaterGreater => {
                    self.advance();
                    self.pending_type_gt += 1;
                    break;
                }
                _ => {
                    return Err(self.error(
                        "E0236",
                        "expected `,` or `>` after generic type argument",
                        self.peek().length(),
                    ));
                }
            }
        }
        Ok(args)
    }

    fn parse_path(&mut self) -> Result<Vec<String>, Diagnostic> {
        let mut parts = vec![self.expect_ident("expected identifier")?];
        while self.consume_dot_path_separator() {
            self.advance();
            parts.push(self.expect_path_segment_after_dot()?);
        }
        Ok(parts)
    }

    fn parse_import_path(&mut self) -> Result<Vec<String>, Diagnostic> {
        let mut parts = vec![self.expect_ident("expected import path")?];
        while self.consume_dot_path_separator() {
            self.advance();
            if matches!(self.peek().kind, TokenKind::Star) {
                return Err(self.error(
                    "E0274",
                    "wildcard imports are not supported in v0.1",
                    self.peek().length(),
                ));
            }
            parts.push(self.expect_path_segment_after_dot()?);
        }
        Ok(parts)
    }

    fn consume_dot_path_separator(&mut self) -> bool {
        if matches!(self.peek().kind, TokenKind::Dot) {
            return true;
        }
        if matches!(self.peek().kind, TokenKind::Newline)
            && matches!(self.peek_n(1).kind, TokenKind::Dot)
        {
            self.consume_newline();
            return true;
        }
        false
    }

    fn expect_ident(&mut self, message: &'static str) -> Result<String, Diagnostic> {
        match self.peek().kind.clone() {
            TokenKind::Ident(value) => {
                self.advance();
                Ok(value)
            }
            _ => Err(self.error("E0300", message, self.peek().length())),
        }
    }

    fn expect_path_segment_after_dot(&mut self) -> Result<String, Diagnostic> {
        match self.peek().kind.clone() {
            TokenKind::Ident(value) => {
                self.advance();
                Ok(value)
            }
            TokenKind::Panic => {
                self.advance();
                Ok("panic".to_string())
            }
            _ => Err(self.error(
                "E0300",
                "expected identifier after `.`",
                self.peek().length(),
            )),
        }
    }

    fn expect_kind(
        &mut self,
        expected: TokenKind,
        code: &'static str,
        message: &'static str,
    ) -> Result<(), Diagnostic> {
        if same_variant(&self.peek().kind, &expected) {
            self.advance();
            Ok(())
        } else {
            Err(self.error(code, message, self.peek().length()))
        }
    }

    fn expect_newline(&mut self, message: &'static str) -> Result<(), Diagnostic> {
        match self.peek().kind {
            TokenKind::Newline => {
                self.consume_newline();
                Ok(())
            }
            TokenKind::Eof => Ok(()),
            _ => Err(self.error("E0211", message, self.peek().length())),
        }
    }

    fn consume_newline(&mut self) {
        while matches!(self.peek().kind, TokenKind::Newline) {
            self.advance();
        }
    }

    fn skip_newlines(&mut self) {
        self.consume_newline();
    }

    fn advance(&mut self) {
        if !matches!(self.peek().kind, TokenKind::Eof) {
            self.index += 1;
        }
    }

    fn peek(&self) -> &Token {
        self.tokens
            .get(self.index)
            .unwrap_or_else(|| self.tokens.last().expect("parser requires EOF token"))
    }

    fn peek_n(&self, offset: usize) -> &Token {
        self.tokens
            .get(self.index + offset)
            .unwrap_or_else(|| self.tokens.last().expect("parser requires EOF token"))
    }

    fn error(&self, code: &'static str, message: impl Into<String>, length: usize) -> Diagnostic {
        let token = self.peek();
        Diagnostic::new(
            code,
            message,
            self.path,
            token.line,
            token.column,
            length,
            &token.text,
        )
    }
}

impl Token {
    fn length(&self) -> usize {
        match &self.kind {
            TokenKind::Ident(value) | TokenKind::String(value) => value.len().max(1),
            TokenKind::Int(value) => value.to_string().len(),
            TokenKind::Float(value) => value.len(),
            TokenKind::Char(value) => value.len_utf8() + 2,
            TokenKind::Arrow
            | TokenKind::FatArrow
            | TokenKind::EqualEqual
            | TokenKind::BangEqual
            | TokenKind::AmpAmp
            | TokenKind::PipePipe
            | TokenKind::AmpCaret
            | TokenKind::PlusEqual
            | TokenKind::MinusEqual
            | TokenKind::StarEqual
            | TokenKind::SlashEqual
            | TokenKind::PercentEqual
            | TokenKind::PlusPlus
            | TokenKind::MinusMinus
            | TokenKind::AmpEqual
            | TokenKind::PipeEqual
            | TokenKind::CaretEqual
            | TokenKind::LessEqual
            | TokenKind::LessLess
            | TokenKind::GreaterEqual
            | TokenKind::GreaterGreater => 2,
            TokenKind::AmpCaretEqual
            | TokenKind::LessLessEqual
            | TokenKind::GreaterGreaterEqual => 3,
            TokenKind::Eof | TokenKind::Newline => 1,
            _ => 1,
        }
    }
}

fn token_span(token: &Token) -> Span {
    Span {
        line: token.line,
        column: token.column,
        length: token.length(),
        text: token.text.clone(),
    }
}

fn void_type_ref() -> TypeRef {
    TypeRef {
        path: vec!["void".to_string()],
        args: Vec::new(),
    }
}

fn same_variant(left: &TokenKind, right: &TokenKind) -> bool {
    std::mem::discriminant(left) == std::mem::discriminant(right)
}

fn assign_op_from_token(kind: &TokenKind) -> Option<AssignOp> {
    match kind {
        TokenKind::Equal => Some(AssignOp::Assign),
        TokenKind::PlusEqual => Some(AssignOp::Add),
        TokenKind::MinusEqual => Some(AssignOp::Subtract),
        TokenKind::StarEqual => Some(AssignOp::Multiply),
        TokenKind::SlashEqual => Some(AssignOp::Divide),
        TokenKind::PercentEqual => Some(AssignOp::Remainder),
        TokenKind::LessLessEqual => Some(AssignOp::ShiftLeft),
        TokenKind::GreaterGreaterEqual => Some(AssignOp::ShiftRight),
        TokenKind::AmpEqual => Some(AssignOp::BitAnd),
        TokenKind::CaretEqual => Some(AssignOp::BitXor),
        TokenKind::PipeEqual => Some(AssignOp::BitOr),
        TokenKind::AmpCaretEqual => Some(AssignOp::BitAndNot),
        _ => None,
    }
}

fn starts_try_operand(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::String(_)
            | TokenKind::Int(_)
            | TokenKind::Float(_)
            | TokenKind::Char(_)
            | TokenKind::True
            | TokenKind::False
            | TokenKind::Void
            | TokenKind::If
            | TokenKind::Match
            | TokenKind::Panic
            | TokenKind::Ident(_)
    )
}

fn postfix_op_from_token(kind: &TokenKind) -> Option<PostfixOp> {
    match kind {
        TokenKind::PlusPlus => Some(PostfixOp::Increment),
        TokenKind::MinusMinus => Some(PostfixOp::Decrement),
        _ => None,
    }
}

fn is_declaration_start(kind: &TokenKind, public: bool) -> bool {
    matches!(
        kind,
        TokenKind::Struct
            | TokenKind::Enum
            | TokenKind::Interface
            | TokenKind::Const
            | TokenKind::Fn
    ) || (!public && matches!(kind, TokenKind::Impl | TokenKind::Extern))
}

#[cfg(test)]
#[path = "parser_layout_tests.rs"]
mod layout_tests;
#[cfg(test)]
#[path = "parser_tests.rs"]
mod tests;

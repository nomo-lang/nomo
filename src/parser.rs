use crate::ast::{
    BinaryOp, EnumDef, EnumVariant, Expr, Field, Function, ImplBlock, MatchArm, Param, SourceFile,
    Span, Stmt, StructDef, TypeRef,
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
    }
    .parse_source_file()
}

struct Parser<'a> {
    path: &'a Path,
    tokens: &'a [Token],
    index: usize,
    allow_struct_literals: bool,
    impl_self_type: Option<TypeRef>,
}

impl Parser<'_> {
    fn parse_source_file(&mut self) -> Result<SourceFile, Diagnostic> {
        self.skip_newlines();
        self.expect_kind(TokenKind::Package, "N0200", "expected `package <name>`")?;
        let package = self.parse_path()?;
        self.expect_newline("expected newline after package declaration")?;

        let mut imports = Vec::new();
        loop {
            self.skip_newlines();
            if !matches!(self.peek().kind, TokenKind::Import) {
                break;
            }
            self.advance();
            imports.push(self.parse_path()?);
            self.expect_newline("expected newline after import declaration")?;
        }

        let mut structs = Vec::new();
        let mut enums = Vec::new();
        let mut impls = Vec::new();
        let mut functions = Vec::new();
        loop {
            self.skip_newlines();
            let public = self.consume_pub();
            match self.peek().kind {
                TokenKind::Struct => structs.push(self.parse_struct(public)?),
                TokenKind::Enum => enums.push(self.parse_enum(public)?),
                TokenKind::Impl if !public => impls.push(self.parse_impl()?),
                TokenKind::Fn => functions.push(self.parse_function(public)?),
                TokenKind::Eof if !public => break,
                _ => {
                    return Err(self.error(
                        "N0201",
                        "expected struct, enum, impl, function declaration, or end of file",
                        self.peek().length(),
                    ));
                }
            }
        }

        Ok(SourceFile {
            package,
            imports,
            structs,
            enums,
            impls,
            functions,
        })
    }

    fn parse_enum(&mut self, public: bool) -> Result<EnumDef, Diagnostic> {
        self.expect_kind(TokenKind::Enum, "N0226", "expected `enum`")?;
        let name = self.expect_ident("expected enum name")?;
        let type_params = self.parse_type_params()?;
        self.expect_kind(
            TokenKind::LBrace,
            "N0227",
            "expected `{` before enum variants",
        )?;
        self.expect_newline("expected newline after `{`")?;

        let mut variants = Vec::new();
        loop {
            self.skip_newlines();
            match self.peek().kind {
                TokenKind::RBrace => {
                    self.advance();
                    self.consume_newline();
                    break;
                }
                TokenKind::Eof => {
                    return Err(self.error("N0228", "unterminated enum body; expected `}`", 1));
                }
                _ => {
                    let name = self.expect_ident("expected enum variant name")?;
                    let payload = if matches!(self.peek().kind, TokenKind::LParen) {
                        self.advance();
                        let type_ref = self.parse_type_ref()?;
                        self.expect_kind(
                            TokenKind::RParen,
                            "N0233",
                            "expected `)` after enum variant payload type",
                        )?;
                        Some(type_ref)
                    } else {
                        None
                    };
                    variants.push(EnumVariant { name, payload });
                    if matches!(self.peek().kind, TokenKind::Comma) {
                        self.advance();
                    }
                    self.expect_newline("expected newline after enum variant")?;
                }
            }
        }

        Ok(EnumDef {
            public,
            name,
            type_params,
            variants,
        })
    }

    fn parse_type_params(&mut self) -> Result<Vec<String>, Diagnostic> {
        if !matches!(self.peek().kind, TokenKind::Less) {
            return Ok(Vec::new());
        }
        self.advance();
        let mut params = Vec::new();
        loop {
            params.push(self.expect_ident("expected generic type parameter name")?);
            match self.peek().kind {
                TokenKind::Comma => {
                    self.advance();
                }
                TokenKind::Greater => {
                    self.advance();
                    break;
                }
                _ => {
                    return Err(self.error(
                        "N0235",
                        "expected `,` or `>` after generic type parameter",
                        self.peek().length(),
                    ));
                }
            }
        }
        Ok(params)
    }

    fn parse_struct(&mut self, public: bool) -> Result<StructDef, Diagnostic> {
        self.expect_kind(TokenKind::Struct, "N0218", "expected `struct`")?;
        let name = self.expect_ident("expected struct name")?;
        self.expect_kind(
            TokenKind::LBrace,
            "N0219",
            "expected `{` before struct fields",
        )?;
        self.expect_newline("expected newline after `{`")?;

        let mut fields = Vec::new();
        loop {
            self.skip_newlines();
            match self.peek().kind {
                TokenKind::RBrace => {
                    self.advance();
                    self.consume_newline();
                    break;
                }
                TokenKind::Eof => {
                    return Err(self.error("N0220", "unterminated struct body; expected `}`", 1));
                }
                _ => {
                    let public = self.consume_pub();
                    let field_name = self.expect_ident("expected field name")?;
                    self.expect_kind(TokenKind::Colon, "N0221", "expected `:` after field name")?;
                    let type_ref = self.parse_type_ref()?;
                    fields.push(Field {
                        public,
                        name: field_name,
                        type_ref,
                    });
                    self.expect_newline("expected newline after struct field")?;
                }
            }
        }

        Ok(StructDef {
            public,
            name,
            fields,
        })
    }

    fn parse_function(&mut self, public: bool) -> Result<Function, Diagnostic> {
        self.expect_kind(TokenKind::Fn, "N0202", "expected `fn`")?;
        let name = self.expect_ident("expected function name")?;
        self.expect_kind(
            TokenKind::LParen,
            "N0203",
            "expected `(` after function name",
        )?;
        let params = self.parse_params()?;
        self.expect_kind(
            TokenKind::Arrow,
            "N0205",
            "expected `->` before return type",
        )?;
        let return_type = self.parse_type_ref()?;
        self.expect_kind(
            TokenKind::LBrace,
            "N0206",
            "expected `{` before function body",
        )?;
        self.expect_newline("expected newline after `{`")?;

        let mut body = Vec::new();
        loop {
            self.skip_newlines();
            match self.peek().kind {
                TokenKind::RBrace => {
                    self.advance();
                    self.consume_newline();
                    break;
                }
                TokenKind::Eof => {
                    return Err(self.error("N0207", "unterminated function body; expected `}`", 1));
                }
                _ => {
                    body.push(self.parse_stmt()?);
                    self.expect_newline("expected newline after statement")?;
                }
            }
        }

        Ok(Function {
            public,
            name,
            params,
            return_type,
            body,
        })
    }

    fn parse_impl(&mut self) -> Result<ImplBlock, Diagnostic> {
        self.expect_kind(TokenKind::Impl, "N0250", "expected `impl`")?;
        let type_name = self.parse_type_ref()?;
        if type_name.path.len() != 1 || !type_name.args.is_empty() {
            return Err(self.error(
                "N0251",
                "v0.1 impl blocks must target a local non-generic type",
                self.peek().length(),
            ));
        }
        self.expect_kind(
            TokenKind::LBrace,
            "N0252",
            "expected `{` before impl methods",
        )?;
        self.expect_newline("expected newline after `{`")?;

        let previous_self = self.impl_self_type.replace(type_name.clone());
        let mut methods = Vec::new();
        loop {
            self.skip_newlines();
            let public = self.consume_pub();
            match self.peek().kind {
                TokenKind::Fn => methods.push(self.parse_function(public)?),
                TokenKind::RBrace if !public => {
                    self.advance();
                    self.consume_newline();
                    break;
                }
                TokenKind::Eof => {
                    self.impl_self_type = previous_self;
                    return Err(self.error("N0253", "unterminated impl body; expected `}`", 1));
                }
                _ => {
                    self.impl_self_type = previous_self;
                    return Err(self.error(
                        "N0254",
                        "expected method declaration or `}` in impl body",
                        self.peek().length(),
                    ));
                }
            }
        }
        self.impl_self_type = previous_self;
        Ok(ImplBlock { type_name, methods })
    }

    fn consume_pub(&mut self) -> bool {
        if matches!(self.peek().kind, TokenKind::Pub) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, Diagnostic> {
        let mut params = Vec::new();
        if matches!(self.peek().kind, TokenKind::RParen) {
            self.advance();
            return Ok(params);
        }

        loop {
            let mutable = if matches!(self.peek().kind, TokenKind::Mut) {
                self.advance();
                true
            } else {
                false
            };
            let name = self.expect_ident("expected parameter name")?;
            let type_ref = if name == "self" && self.impl_self_type.is_some() {
                if matches!(self.peek().kind, TokenKind::Colon) {
                    self.advance();
                    self.parse_type_ref()?
                } else {
                    self.impl_self_type.clone().expect("checked above")
                }
            } else {
                self.expect_kind(
                    TokenKind::Colon,
                    "N0214",
                    "expected `:` after parameter name",
                )?;
                self.parse_type_ref()?
            };
            params.push(Param {
                name,
                mutable,
                type_ref,
            });

            match self.peek().kind {
                TokenKind::Comma => {
                    self.advance();
                }
                TokenKind::RParen => {
                    self.advance();
                    break;
                }
                _ => {
                    return Err(self.error(
                        "N0215",
                        "expected `,` or `)` after parameter",
                        self.peek().length(),
                    ));
                }
            }
        }

        Ok(params)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, Diagnostic> {
        let token = self.peek().clone();
        if matches!(token.kind, TokenKind::Let) {
            return self.parse_let_stmt(token);
        }
        if matches!(token.kind, TokenKind::Return) {
            return self.parse_return_stmt(token);
        }
        if matches!(token.kind, TokenKind::Ident(_))
            && matches!(self.peek_n(1).kind, TokenKind::Equal)
        {
            return self.parse_assign_stmt(token);
        }
        Ok(Stmt::Expr {
            expr: self.parse_expr()?,
            span: Span {
                line: token.line,
                column: token.column,
                length: token.length(),
                text: token.text,
            },
        })
    }

    fn parse_assign_stmt(&mut self, token: Token) -> Result<Stmt, Diagnostic> {
        let name = self.expect_ident("expected assignment target")?;
        self.expect_kind(TokenKind::Equal, "N0217", "expected `=` in assignment")?;
        let value = self.parse_expr()?;
        Ok(Stmt::Assign {
            name,
            value,
            span: Span {
                line: token.line,
                column: token.column,
                length: token.length(),
                text: token.text,
            },
        })
    }

    fn parse_return_stmt(&mut self, token: Token) -> Result<Stmt, Diagnostic> {
        self.expect_kind(TokenKind::Return, "N0216", "expected `return`")?;
        let value = if matches!(self.peek().kind, TokenKind::Newline | TokenKind::RBrace) {
            None
        } else {
            Some(self.parse_expr()?)
        };
        Ok(Stmt::Return {
            value,
            span: Span {
                line: token.line,
                column: token.column,
                length: token.length(),
                text: token.text,
            },
        })
    }

    fn parse_let_stmt(&mut self, token: Token) -> Result<Stmt, Diagnostic> {
        self.expect_kind(TokenKind::Let, "N0212", "expected `let`")?;
        let mutable = if matches!(self.peek().kind, TokenKind::Mut) {
            self.advance();
            true
        } else {
            false
        };
        let name = self.expect_ident("expected variable name after `let`")?;
        let type_annotation = if matches!(self.peek().kind, TokenKind::Colon) {
            self.advance();
            Some(self.parse_type_ref()?)
        } else {
            None
        };
        self.expect_kind(TokenKind::Equal, "N0213", "expected `=` before initializer")?;
        let value = self.parse_expr()?;
        Ok(Stmt::Let {
            name,
            mutable,
            type_annotation,
            value,
            span: Span {
                line: token.line,
                column: token.column,
                length: token.length(),
                text: token.text,
            },
        })
    }

    fn parse_expr(&mut self) -> Result<Expr, Diagnostic> {
        self.parse_equality_expr()
    }

    fn parse_equality_expr(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_comparison_expr()?;
        loop {
            let op = match self.peek().kind {
                TokenKind::EqualEqual => BinaryOp::Equal,
                TokenKind::BangEqual => BinaryOp::NotEqual,
                _ => break,
            };
            self.advance();
            let right = self.parse_comparison_expr()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_comparison_expr(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_additive_expr()?;
        loop {
            let op = match self.peek().kind {
                TokenKind::Less => BinaryOp::Less,
                TokenKind::LessEqual => BinaryOp::LessEqual,
                TokenKind::Greater => BinaryOp::Greater,
                TokenKind::GreaterEqual => BinaryOp::GreaterEqual,
                _ => break,
            };
            self.advance();
            let right = self.parse_additive_expr()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_additive_expr(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_cast_expr()?;
        while matches!(self.peek().kind, TokenKind::Plus) {
            self.advance();
            let right = self.parse_cast_expr()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op: BinaryOp::Add,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_cast_expr(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_postfix_expr()?;
        while matches!(self.peek().kind, TokenKind::As) {
            self.advance();
            let target = self.parse_type_ref()?;
            expr = Expr::Cast {
                expr: Box::new(expr),
                target,
            };
        }
        Ok(expr)
    }

    fn parse_postfix_expr(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_primary_expr()?;
        while matches!(self.peek().kind, TokenKind::Question) {
            self.advance();
            expr = Expr::Try {
                expr: Box::new(expr),
            };
        }
        Ok(expr)
    }

    fn parse_primary_expr(&mut self) -> Result<Expr, Diagnostic> {
        match self.peek().kind.clone() {
            TokenKind::String(value) => {
                self.advance();
                Ok(Expr::String(value))
            }
            TokenKind::Int(value) => {
                self.advance();
                Ok(Expr::Int(value))
            }
            TokenKind::Float(value) => {
                self.advance();
                Ok(Expr::Float(value))
            }
            TokenKind::Char(value) => {
                self.advance();
                Ok(Expr::Char(value))
            }
            TokenKind::True => {
                self.advance();
                Ok(Expr::Bool(true))
            }
            TokenKind::False => {
                self.advance();
                Ok(Expr::Bool(false))
            }
            TokenKind::Void => {
                self.advance();
                Ok(Expr::Void)
            }
            TokenKind::If => self.parse_if_expr(),
            TokenKind::Match => self.parse_match_expr(),
            TokenKind::Panic => self.parse_panic_expr(),
            TokenKind::Ident(_) => self.parse_name_or_call(),
            _ => Err(self.error("N0208", "expected expression", self.peek().length())),
        }
    }

    fn parse_panic_expr(&mut self) -> Result<Expr, Diagnostic> {
        self.expect_kind(TokenKind::Panic, "N0246", "expected `panic`")?;
        self.expect_kind(TokenKind::LParen, "N0247", "expected `(` after `panic`")?;
        let message = self.parse_expr()?;
        self.expect_kind(
            TokenKind::RParen,
            "N0248",
            "expected `)` after panic message",
        )?;
        Ok(Expr::Panic {
            message: Box::new(message),
        })
    }

    fn parse_if_expr(&mut self) -> Result<Expr, Diagnostic> {
        self.expect_kind(TokenKind::If, "N0240", "expected `if`")?;
        let previous = self.allow_struct_literals;
        self.allow_struct_literals = false;
        let condition = self.parse_expr();
        self.allow_struct_literals = previous;
        let condition = condition?;
        let then_branch = self.parse_expr_block("N0241", "expected `{` before if branch")?;
        self.skip_newlines();
        self.expect_kind(TokenKind::Else, "N0244", "expected `else` after if branch")?;
        let else_branch = self.parse_expr_block("N0245", "expected `{` before else branch")?;
        Ok(Expr::If {
            condition: Box::new(condition),
            then_branch: Box::new(then_branch),
            else_branch: Box::new(else_branch),
        })
    }

    fn parse_expr_block(
        &mut self,
        open_code: &'static str,
        open_message: &'static str,
    ) -> Result<Expr, Diagnostic> {
        self.expect_kind(TokenKind::LBrace, open_code, open_message)?;
        self.skip_newlines();
        let value = self.parse_expr()?;
        self.skip_newlines();
        self.expect_kind(
            TokenKind::RBrace,
            "N0242",
            "expected `}` after expression block",
        )?;
        Ok(value)
    }

    fn parse_match_expr(&mut self) -> Result<Expr, Diagnostic> {
        self.expect_kind(TokenKind::Match, "N0229", "expected `match`")?;
        let value = Expr::Name(self.parse_path()?);
        self.expect_kind(TokenKind::LBrace, "N0230", "expected `{` before match arms")?;
        self.expect_newline("expected newline after `{`")?;

        let mut arms = Vec::new();
        loop {
            self.skip_newlines();
            match self.peek().kind {
                TokenKind::RBrace => {
                    self.advance();
                    break;
                }
                TokenKind::Eof => {
                    return Err(self.error("N0231", "unterminated match body; expected `}`", 1));
                }
                _ => {
                    let pattern = self.parse_path()?;
                    let binding = if matches!(self.peek().kind, TokenKind::LParen) {
                        self.advance();
                        let binding =
                            self.expect_ident("expected binding name in match pattern")?;
                        self.expect_kind(
                            TokenKind::RParen,
                            "N0234",
                            "expected `)` after match pattern binding",
                        )?;
                        Some(binding)
                    } else {
                        None
                    };
                    self.expect_kind(
                        TokenKind::FatArrow,
                        "N0232",
                        "expected `=>` after match pattern",
                    )?;
                    let value = self.parse_expr()?;
                    arms.push(MatchArm {
                        pattern,
                        binding,
                        value,
                    });
                    self.expect_newline("expected newline after match arm")?;
                }
            }
        }
        Ok(Expr::Match {
            value: Box::new(value),
            arms,
        })
    }

    fn parse_name_or_call(&mut self) -> Result<Expr, Diagnostic> {
        let path = self.parse_path()?;
        if self.allow_struct_literals && matches!(self.peek().kind, TokenKind::LBrace) {
            return self.parse_struct_literal(path);
        }
        let type_args = if path == ["Array".to_string(), "new".to_string()]
            && matches!(self.peek().kind, TokenKind::Less)
        {
            self.parse_type_args()?
        } else {
            Vec::new()
        };
        if !matches!(self.peek().kind, TokenKind::LParen) {
            if !type_args.is_empty() {
                return Err(self.error(
                    "N0210",
                    "expected `(` after generic call type arguments",
                    self.peek().length(),
                ));
            }
            return Ok(Expr::Name(path));
        }
        self.advance();
        let mut args = Vec::new();
        if !matches!(self.peek().kind, TokenKind::RParen) {
            args.push(self.parse_expr()?);
            while matches!(self.peek().kind, TokenKind::Comma) {
                self.advance();
                args.push(self.parse_expr()?);
            }
        }
        self.expect_kind(
            TokenKind::RParen,
            "N0210",
            "expected `)` after call arguments",
        )?;
        Ok(Expr::Call {
            callee: path,
            type_args,
            args,
        })
    }

    fn parse_struct_literal(&mut self, type_name: Vec<String>) -> Result<Expr, Diagnostic> {
        self.expect_kind(
            TokenKind::LBrace,
            "N0222",
            "expected `{` before struct literal fields",
        )?;
        let mut fields = Vec::new();
        if !matches!(self.peek().kind, TokenKind::RBrace) {
            loop {
                let field_name = self.expect_ident("expected struct literal field name")?;
                self.expect_kind(
                    TokenKind::Colon,
                    "N0223",
                    "expected `:` after struct literal field name",
                )?;
                let value = self.parse_expr()?;
                fields.push((field_name, value));
                match self.peek().kind {
                    TokenKind::Comma => {
                        self.advance();
                    }
                    TokenKind::RBrace => break,
                    _ => {
                        return Err(self.error(
                            "N0224",
                            "expected `,` or `}` after struct literal field",
                            self.peek().length(),
                        ));
                    }
                }
            }
        }
        self.expect_kind(
            TokenKind::RBrace,
            "N0225",
            "expected `}` after struct literal",
        )?;
        Ok(Expr::StructLiteral { type_name, fields })
    }

    fn parse_type_ref(&mut self) -> Result<TypeRef, Diagnostic> {
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

    fn parse_type_args(&mut self) -> Result<Vec<TypeRef>, Diagnostic> {
        if !matches!(self.peek().kind, TokenKind::Less) {
            return Ok(Vec::new());
        }
        self.advance();
        let mut args = Vec::new();
        loop {
            args.push(self.parse_type_ref()?);
            match self.peek().kind {
                TokenKind::Comma => {
                    self.advance();
                }
                TokenKind::Greater => {
                    self.advance();
                    break;
                }
                _ => {
                    return Err(self.error(
                        "N0236",
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
        while matches!(self.peek().kind, TokenKind::Dot) {
            self.advance();
            parts.push(self.expect_ident("expected identifier after `.`")?);
        }
        Ok(parts)
    }

    fn expect_ident(&mut self, message: &'static str) -> Result<String, Diagnostic> {
        match self.peek().kind.clone() {
            TokenKind::Ident(value) => {
                self.advance();
                Ok(value)
            }
            _ => Err(self.error("N0300", message, self.peek().length())),
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
            _ => Err(self.error("N0211", message, self.peek().length())),
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
            TokenKind::Arrow | TokenKind::FatArrow => 2,
            TokenKind::Eof | TokenKind::Newline => 1,
            _ => 1,
        }
    }
}

fn same_variant(left: &TokenKind, right: &TokenKind) -> bool {
    std::mem::discriminant(left) == std::mem::discriminant(right)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;

    #[test]
    fn parses_stage0_ast() {
        let source = "package app.main\n\nimport std.io\n\nfn main() -> void {\n    io.println(\"Hello\")\n}\n";
        let tokens = lex(Path::new("main.nomo"), source).unwrap();
        let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

        assert_eq!(ast.package, vec!["app", "main"]);
        assert_eq!(ast.imports, vec![vec!["std", "io"]]);
        assert!(ast.structs.is_empty());
        assert!(ast.enums.is_empty());
        assert_eq!(ast.functions.len(), 1);
        assert!(ast.functions[0].params.is_empty());
    }

    #[test]
    fn parses_let_and_variable_reference() {
        let source = "package app.main\n\nimport std.io\n\nfn main() -> void {\n    let message: string = \"Hello\"\n    io.println(message)\n}\n";
        let tokens = lex(Path::new("main.nomo"), source).unwrap();
        let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

        assert!(matches!(
            ast.functions[0].body[0],
            Stmt::Let {
                ref name,
                ref type_annotation,
                value: Expr::String(_),
                ..
            } if name == "message"
                && type_annotation.as_ref().unwrap().path == ["string"]
        ));
        assert!(matches!(
            ast.functions[0].body[1],
            Stmt::Expr {
                expr: Expr::Call { ref args, .. },
                ..
            } if args == &[Expr::Name(vec!["message".to_string()])]
        ));
    }

    #[test]
    fn parses_function_params_return_and_addition() {
        let source = "package app.main\n\nfn add(a: i64, b: i64) -> i64 {\n    return a + b\n}\n";
        let tokens = lex(Path::new("main.nomo"), source).unwrap();
        let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

        assert_eq!(ast.functions[0].params.len(), 2);
        assert_eq!(ast.functions[0].params[0].name, "a");
        assert!(matches!(
            ast.functions[0].body[0],
            Stmt::Return {
                value: Some(Expr::Binary {
                    op: BinaryOp::Add,
                    ..
                }),
                ..
            }
        ));
    }

    #[test]
    fn parses_if_expression_and_comparison() {
        let source = "package app.main\n\nfn label(score: i64) -> string {\n    return if score >= 60 {\n        \"pass\"\n    } else {\n        \"fail\"\n    }\n}\n";
        let tokens = lex(Path::new("main.nomo"), source).unwrap();
        let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

        assert!(matches!(
            ast.functions[0].body[0],
            Stmt::Return {
                value: Some(Expr::If {
                    ref condition,
                    ref then_branch,
                    ref else_branch,
                }),
                ..
            } if matches!(
                condition.as_ref(),
                Expr::Binary {
                    op: BinaryOp::GreaterEqual,
                    ..
                }
            ) && then_branch.as_ref() == &Expr::String("pass".to_string())
                && else_branch.as_ref() == &Expr::String("fail".to_string())
        ));
    }

    #[test]
    fn parses_panic_expression() {
        let source = "package app.main\n\nfn main() -> void {\n    panic(\"boom\")\n}\n";
        let tokens = lex(Path::new("main.nomo"), source).unwrap();
        let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

        assert!(matches!(
            ast.functions[0].body[0],
            Stmt::Expr {
                expr: Expr::Panic { .. },
                ..
            }
        ));
    }

    #[test]
    fn parses_void_expression() {
        let source = "package app.main\n\nenum Result<T, E> {\n    Ok(T)\n    Err(E)\n}\n\nfn done() -> Result<void, string> {\n    return Result.Ok(void)\n}\n";
        let tokens = lex(Path::new("main.nomo"), source).unwrap();
        let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

        assert!(matches!(
            ast.functions[0].body[0],
            Stmt::Return {
                value: Some(Expr::Call { ref args, .. }),
                ..
            } if args == &[Expr::Void]
        ));
    }

    #[test]
    fn parses_assignment_statement() {
        let source = "package app.main\n\nimport std.io\n\nfn main() -> void {\n    let mut count: i64 = 1\n    count = count + 1\n}\n";
        let tokens = lex(Path::new("main.nomo"), source).unwrap();
        let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

        assert!(matches!(
            ast.functions[0].body[1],
            Stmt::Assign {
                ref name,
                value: Expr::Binary { .. },
                ..
            } if name == "count"
        ));
    }

    #[test]
    fn parses_struct_definition_and_literal() {
        let source = "package app.main\n\nstruct Point {\n    x: i64\n    y: i64\n}\n\nfn main() -> void {\n    let point: Point = Point { x: 1, y: 2 }\n}\n";
        let tokens = lex(Path::new("main.nomo"), source).unwrap();
        let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

        assert_eq!(ast.structs.len(), 1);
        assert_eq!(ast.structs[0].name, "Point");
        assert_eq!(ast.structs[0].fields.len(), 2);
        assert!(matches!(
            ast.functions[0].body[0],
            Stmt::Let {
                value: Expr::StructLiteral { ref type_name, .. },
                ..
            } if type_name == &["Point".to_string()]
        ));
    }

    #[test]
    fn parses_impl_method_with_self_parameter() {
        let source = "package app.main\n\nstruct User {\n    email: string\n}\n\nimpl User {\n    pub fn get_email(self) -> string {\n        return self.email\n    }\n}\n\nfn main() -> void {\n}\n";
        let tokens = lex(Path::new("main.nomo"), source).unwrap();
        let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

        assert_eq!(ast.impls.len(), 1);
        assert_eq!(ast.impls[0].type_name.path, ["User"]);
        assert_eq!(ast.impls[0].methods.len(), 1);
        assert!(ast.impls[0].methods[0].public);
        assert_eq!(ast.impls[0].methods[0].params[0].name, "self");
        assert_eq!(ast.impls[0].methods[0].params[0].type_ref.path, ["User"]);
    }

    #[test]
    fn parses_pub_declarations_and_fields() {
        let source = "package app.main\n\npub struct User {\n    pub id: string\n    email: string\n}\n\npub enum Color {\n    Red\n    Blue\n}\n\npub fn main() -> void {\n}\n";
        let tokens = lex(Path::new("main.nomo"), source).unwrap();
        let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

        assert!(ast.structs[0].public);
        assert!(ast.structs[0].fields[0].public);
        assert!(!ast.structs[0].fields[1].public);
        assert!(ast.enums[0].public);
        assert!(ast.functions[0].public);
    }

    #[test]
    fn parses_enum_and_match_expression() {
        let source = "package app.main\n\nenum Color {\n    Red\n    Blue\n}\n\nfn label(color: Color) -> string {\n    return match color {\n        Color.Red => \"red\"\n        Color.Blue => \"blue\"\n    }\n}\n\nfn main() -> void {\n}\n";
        let tokens = lex(Path::new("main.nomo"), source).unwrap();
        let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

        assert_eq!(ast.enums.len(), 1);
        assert_eq!(
            ast.enums[0]
                .variants
                .iter()
                .map(|variant| variant.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Red", "Blue"]
        );
        assert!(matches!(
            ast.functions[0].body[0],
            Stmt::Return {
                value: Some(Expr::Match { ref arms, .. }),
                ..
            } if arms.len() == 2
        ));
    }

    #[test]
    fn parses_payload_enum_and_match_binding() {
        let source = "package app.main\n\nenum MaybeInt {\n    Some(i64)\n    None\n}\n\nfn value(input: MaybeInt) -> i64 {\n    return match input {\n        MaybeInt.Some(n) => n\n        MaybeInt.None => 0\n    }\n}\n\nfn main() -> void {\n}\n";
        let tokens = lex(Path::new("main.nomo"), source).unwrap();
        let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

        assert!(ast.enums[0].variants[0].payload.is_some());
        assert!(matches!(
            ast.functions[0].body[0],
            Stmt::Return {
                value: Some(Expr::Match { ref arms, .. }),
                ..
            } if arms[0].binding.as_deref() == Some("n")
        ));
    }

    #[test]
    fn parses_generic_enum_type_reference() {
        let source = "package app.main\n\nenum Option<T> {\n    Some(T)\n    None\n}\n\nfn main() -> void {\n    let value: Option<i64> = Option.Some(1)\n}\n";
        let tokens = lex(Path::new("main.nomo"), source).unwrap();
        let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

        assert_eq!(ast.enums[0].type_params, vec!["T"]);
        assert_eq!(
            ast.enums[0].variants[0].payload.as_ref().unwrap().path,
            ["T"]
        );
        assert!(matches!(
            ast.functions[0].body[0],
            Stmt::Let {
                ref type_annotation,
                ..
            } if type_annotation.as_ref().unwrap().args.len() == 1
        ));
    }

    #[test]
    fn parses_try_postfix() {
        let source = "package app.main\n\nenum Result<T, E> {\n    Ok(T)\n    Err(E)\n}\n\nfn parse() -> Result<i64, string> {\n    return Result.Ok(1)\n}\n\nfn main() -> void {\n    let value: i64 = parse()?\n}\n";
        let tokens = lex(Path::new("main.nomo"), source).unwrap();
        let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

        assert!(matches!(
            ast.functions[1].body[0],
            Stmt::Let {
                value: Expr::Try { .. },
                ..
            }
        ));
    }

    #[test]
    fn parses_float_literal_and_cast_expression() {
        let source = "package app.main\n\nfn ratio(age: i64) -> f64 {\n    return age as f64\n}\n\nfn main() -> void {\n    let pi: f64 = 3.14\n}\n";
        let tokens = lex(Path::new("main.nomo"), source).unwrap();
        let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

        assert!(matches!(
            ast.functions[0].body[0],
            Stmt::Return {
                value: Some(Expr::Cast { ref target, .. }),
                ..
            } if target.path == ["f64"]
        ));
        assert!(matches!(
            ast.functions[1].body[0],
            Stmt::Let {
                value: Expr::Float(ref value),
                ..
            } if value == "3.14"
        ));
    }

    #[test]
    fn parses_char_literal() {
        let source = "package app.main\n\nfn main() -> void {\n    let letter: char = 'N'\n}\n";
        let tokens = lex(Path::new("main.nomo"), source).unwrap();
        let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

        assert!(matches!(
            ast.functions[0].body[0],
            Stmt::Let {
                value: Expr::Char('N'),
                ..
            }
        ));
    }

    #[test]
    fn parses_generic_array_new_call() {
        let source = "package app.main\n\nfn main() -> void {\n    let items: Array<string> = Array.new<string>()\n}\n";
        let tokens = lex(Path::new("main.nomo"), source).unwrap();
        let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

        assert!(matches!(
            ast.functions[0].body[0],
            Stmt::Let {
                value:
                    Expr::Call {
                        ref callee,
                        ref type_args,
                        ref args,
                    },
                ..
            } if callee == &["Array".to_string(), "new".to_string()]
                && type_args.len() == 1
                && type_args[0].path == ["string"]
                && args.is_empty()
        ));
    }
}

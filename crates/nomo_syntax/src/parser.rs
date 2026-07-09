use crate::ast::{
    AssignOp, BinaryOp, ConstDef, EnumDef, EnumVariant, Expr, ExternBlock, Field, ForVariant,
    Function, FunctionSignature, ImplBlock, InterfaceDef, MatchArm, MatchStmtArm, Param, PostfixOp,
    SourceFile, Span, Stmt, StructDef, TypeRef,
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
        let mut extern_blocks = Vec::new();
        let mut impls = Vec::new();
        let mut consts = Vec::new();
        let mut functions = Vec::new();
        let mut script_body = Vec::new();
        let mut parsing_script_body = false;
        loop {
            self.skip_newlines();
            let is_test = self.parse_test_attribute()?;
            let public = self.consume_pub();
            if is_test && !matches!(self.peek().kind, TokenKind::Fn) {
                return Err(self.error(
                    "E1100",
                    "`#[test]` can only be applied to a function",
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
                    structs.push(self.parse_struct(public)?)
                }
                TokenKind::Enum if !parsing_script_body => enums.push(self.parse_enum(public)?),
                TokenKind::Interface if !parsing_script_body => {
                    interfaces.push(self.parse_interface(public)?)
                }
                TokenKind::Extern if !public && !parsing_script_body => {
                    extern_blocks.push(self.parse_extern_block()?)
                }
                TokenKind::Impl if !public && !parsing_script_body => {
                    impls.push(self.parse_impl()?)
                }
                TokenKind::Const if !parsing_script_body => consts.push(self.parse_const(public)?),
                TokenKind::Fn if !parsing_script_body => {
                    functions.push(self.parse_function(public, is_test)?)
                }
                TokenKind::Eof if !public && !is_test => break,
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
            extern_blocks,
            impls,
            consts,
            functions,
            script_body,
        })
    }

    fn parse_test_attribute(&mut self) -> Result<bool, Diagnostic> {
        if !matches!(self.peek().kind, TokenKind::Hash) {
            return Ok(false);
        }
        let token = self.peek().clone();
        self.advance();
        self.expect_kind(TokenKind::LBracket, "E1100", "expected `[` after `#`")?;
        let name = self.expect_ident("expected attribute name")?;
        self.expect_kind(TokenKind::RBracket, "E1100", "expected `]` after attribute")?;
        self.expect_newline("expected newline after attribute")?;
        if name == "test" {
            Ok(true)
        } else {
            Err(Diagnostic::new(
                "E1100",
                format!("unsupported attribute `#[{name}]`"),
                self.path,
                token.line,
                token.column,
                token.length(),
                &token.text,
            ))
        }
    }

    fn parse_enum(&mut self, public: bool) -> Result<EnumDef, Diagnostic> {
        let enum_token = self.peek().clone();
        self.expect_kind(TokenKind::Enum, "E0226", "expected `enum`")?;
        let name = self.expect_ident("expected enum name")?;
        let type_params = self.parse_type_params()?;
        self.expect_kind(
            TokenKind::LBrace,
            "E0227",
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
                    return Err(self.error("E0228", "unterminated enum body; expected `}`", 1));
                }
                _ => {
                    let variant_token = self.peek().clone();
                    let name = self.expect_ident("expected enum variant name")?;
                    let payload = if matches!(self.peek().kind, TokenKind::LParen) {
                        self.advance();
                        let type_ref = self.parse_type_ref()?;
                        self.expect_kind(
                            TokenKind::RParen,
                            "E0233",
                            "expected `)` after enum variant payload type",
                        )?;
                        Some(type_ref)
                    } else {
                        None
                    };
                    variants.push(EnumVariant {
                        name,
                        payload,
                        span: token_span(&variant_token),
                    });
                    if matches!(self.peek().kind, TokenKind::Comma) {
                        self.advance();
                    }
                    self.expect_newline("expected newline after enum variant")?;
                }
            }
        }

        Ok(EnumDef {
            public,
            package: Vec::new(),
            name,
            type_params,
            variants,
            span: Span {
                line: enum_token.line,
                column: enum_token.column,
                length: enum_token.length(),
                text: enum_token.text,
            },
        })
    }

    fn parse_type_params(&mut self) -> Result<Vec<String>, Diagnostic> {
        if !matches!(self.peek().kind, TokenKind::Less) {
            return Ok(Vec::new());
        }
        self.advance();
        let mut params = Vec::new();
        loop {
            let name = self.expect_ident("expected generic type parameter name")?;
            if params.iter().any(|param| param == &name) {
                return Err(self.error(
                    "E0237",
                    format!("generic type parameter `{name}` is already defined"),
                    self.peek().length(),
                ));
            }
            params.push(name);
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
                        "E0235",
                        "expected `,` or `>` after generic type parameter",
                        self.peek().length(),
                    ));
                }
            }
        }
        Ok(params)
    }

    fn parse_struct(&mut self, public: bool) -> Result<StructDef, Diagnostic> {
        let struct_token = self.peek().clone();
        self.expect_kind(TokenKind::Struct, "E0218", "expected `struct`")?;
        let name = self.expect_ident("expected struct name")?;
        let type_params = self.parse_type_params()?;
        self.expect_kind(
            TokenKind::LBrace,
            "E0219",
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
                    return Err(self.error("E0220", "unterminated struct body; expected `}`", 1));
                }
                _ => {
                    let field_token = self.peek().clone();
                    let public = self.consume_pub();
                    let field_name = self.expect_ident("expected field name")?;
                    self.expect_kind(TokenKind::Colon, "E0221", "expected `:` after field name")?;
                    let type_ref = self.parse_type_ref()?;
                    fields.push(Field {
                        public,
                        name: field_name,
                        type_ref,
                        span: token_span(&field_token),
                    });
                    self.expect_newline("expected newline after struct field")?;
                }
            }
        }

        Ok(StructDef {
            public,
            package: Vec::new(),
            name,
            type_params,
            fields,
            span: Span {
                line: struct_token.line,
                column: struct_token.column,
                length: struct_token.length(),
                text: struct_token.text,
            },
        })
    }

    fn parse_function(&mut self, public: bool, is_test: bool) -> Result<Function, Diagnostic> {
        let function_token = self.peek().clone();
        self.expect_kind(TokenKind::Fn, "E0202", "expected `fn`")?;
        let name = self.expect_ident("expected function name")?;
        let type_params = self.parse_type_params()?;
        self.expect_kind(
            TokenKind::LParen,
            "E0203",
            "expected `(` after function name",
        )?;
        let params = self.parse_params()?;
        let return_type = if matches!(self.peek().kind, TokenKind::Arrow) {
            self.advance();
            self.parse_type_ref()?
        } else {
            TypeRef {
                path: vec!["void".to_string()],
                args: Vec::new(),
            }
        };
        self.expect_kind(
            TokenKind::LBrace,
            "E0206",
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
                    return Err(self.error("E0207", "unterminated function body; expected `}`", 1));
                }
                _ => {
                    body.push(self.parse_stmt()?);
                    self.expect_newline("expected newline after statement")?;
                }
            }
        }

        Ok(Function {
            public,
            is_test,
            package: Vec::new(),
            name,
            type_params,
            params,
            return_type,
            body,
            span: token_span(&function_token),
        })
    }

    fn parse_interface(&mut self, public: bool) -> Result<InterfaceDef, Diagnostic> {
        let interface_token = self.peek().clone();
        self.expect_kind(TokenKind::Interface, "E1500", "expected `interface`")?;
        let name = self.expect_ident("expected interface name")?;
        self.expect_kind(
            TokenKind::LBrace,
            "E1501",
            "expected `{` before interface methods",
        )?;
        self.expect_newline("expected newline after `{`")?;
        let mut methods = Vec::new();
        loop {
            self.skip_newlines();
            match self.peek().kind {
                TokenKind::RBrace => {
                    self.advance();
                    self.consume_newline();
                    return Ok(InterfaceDef {
                        public,
                        name,
                        methods,
                        span: token_span(&interface_token),
                    });
                }
                TokenKind::Fn => methods.push(self.parse_interface_method_signature()?),
                TokenKind::Eof => {
                    return Err(self.error(
                        "E1502",
                        "unterminated interface body; expected `}`",
                        1,
                    ));
                }
                _ => {
                    return Err(self.error(
                        "E1503",
                        "expected interface method signature or `}`",
                        self.peek().length(),
                    ));
                }
            }
        }
    }

    fn parse_interface_method_signature(&mut self) -> Result<FunctionSignature, Diagnostic> {
        let signature = self.parse_function_signature(
            "E1504",
            "expected `fn`",
            "expected interface method name",
            "E1505",
            "expected `(` after interface method name",
            true,
        )?;
        self.expect_newline("expected newline after interface method signature")
            .map(|_| signature)
    }

    fn parse_extern_block(&mut self) -> Result<ExternBlock, Diagnostic> {
        let extern_token = self.peek().clone();
        self.expect_kind(TokenKind::Extern, "E1510", "expected `extern`")?;
        let abi = match self.peek().kind.clone() {
            TokenKind::String(abi) if abi == "C" => {
                self.advance();
                abi
            }
            _ => {
                return Err(self.error("E1511", "expected extern ABI string `\"C\"`", 1));
            }
        };
        self.expect_kind(
            TokenKind::LBrace,
            "E1512",
            "expected `{` before extern declarations",
        )?;
        self.expect_newline("expected newline after `{`")?;
        let mut functions = Vec::new();
        loop {
            self.skip_newlines();
            match self.peek().kind {
                TokenKind::RBrace => {
                    self.advance();
                    self.consume_newline();
                    return Ok(ExternBlock {
                        abi,
                        functions,
                        span: token_span(&extern_token),
                    });
                }
                TokenKind::Fn => functions.push(self.parse_extern_function_signature()?),
                TokenKind::Eof => {
                    return Err(self.error("E1513", "unterminated extern block; expected `}`", 1));
                }
                _ => {
                    return Err(self.error(
                        "E1514",
                        "expected extern function declaration or `}`",
                        self.peek().length(),
                    ));
                }
            }
        }
    }

    fn parse_extern_function_signature(&mut self) -> Result<FunctionSignature, Diagnostic> {
        let signature = self.parse_function_signature(
            "E1515",
            "expected `fn`",
            "expected extern function name",
            "E1516",
            "expected `(` after extern function name",
            false,
        )?;
        self.expect_newline("expected newline after extern function declaration")
            .map(|_| signature)
    }

    fn parse_impl(&mut self) -> Result<ImplBlock, Diagnostic> {
        self.expect_kind(TokenKind::Impl, "E0250", "expected `impl`")?;
        let first_type = self.parse_type_ref()?;
        let (interface_name, type_name) = if matches!(self.peek().kind, TokenKind::For) {
            self.advance();
            (Some(first_type), self.parse_type_ref()?)
        } else {
            (None, first_type)
        };
        if type_name.path.len() != 1 || !type_name.args.is_empty() {
            return Err(self.error(
                "E0251",
                "v0.1 impl blocks must target a local non-generic type",
                self.peek().length(),
            ));
        }
        self.expect_kind(
            TokenKind::LBrace,
            "E0252",
            "expected `{` before impl methods",
        )?;
        self.expect_newline("expected newline after `{`")?;

        let previous_self = self.impl_self_type.replace(type_name.clone());
        let mut methods = Vec::new();
        loop {
            self.skip_newlines();
            let public = self.consume_pub();
            match self.peek().kind {
                TokenKind::Fn => methods.push(self.parse_function(public, false)?),
                TokenKind::RBrace if !public => {
                    self.advance();
                    self.consume_newline();
                    break;
                }
                TokenKind::Eof => {
                    self.impl_self_type = previous_self;
                    return Err(self.error("E0253", "unterminated impl body; expected `}`", 1));
                }
                _ => {
                    self.impl_self_type = previous_self;
                    return Err(self.error(
                        "E0254",
                        "expected method declaration or `}` in impl body",
                        self.peek().length(),
                    ));
                }
            }
        }
        self.impl_self_type = previous_self;
        Ok(ImplBlock {
            interface_name,
            type_name,
            methods,
        })
    }

    fn parse_function_signature(
        &mut self,
        fn_code: &'static str,
        fn_message: &'static str,
        name_message: &'static str,
        paren_code: &'static str,
        paren_message: &'static str,
        allow_bare_self: bool,
    ) -> Result<FunctionSignature, Diagnostic> {
        let function_token = self.peek().clone();
        self.expect_kind(TokenKind::Fn, fn_code, fn_message)?;
        let name = self.expect_ident(name_message)?;
        let type_params = self.parse_type_params()?;
        self.expect_kind(TokenKind::LParen, paren_code, paren_message)?;
        let params = self.parse_params_with_bare_self(allow_bare_self)?;
        let return_type = if matches!(self.peek().kind, TokenKind::Arrow) {
            self.advance();
            self.parse_type_ref()?
        } else {
            void_type_ref()
        };
        Ok(FunctionSignature {
            name,
            type_params,
            params,
            return_type,
            span: token_span(&function_token),
        })
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
        self.parse_params_with_bare_self(false)
    }

    fn parse_params_with_bare_self(
        &mut self,
        allow_bare_self: bool,
    ) -> Result<Vec<Param>, Diagnostic> {
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
            let type_ref = if name == "self" && (self.impl_self_type.is_some() || allow_bare_self) {
                if matches!(self.peek().kind, TokenKind::Colon) {
                    self.advance();
                    self.parse_type_ref()?
                } else if let Some(self_type) = &self.impl_self_type {
                    self_type.clone()
                } else {
                    TypeRef {
                        path: vec!["Self".to_string()],
                        args: Vec::new(),
                    }
                }
            } else {
                self.expect_kind(
                    TokenKind::Colon,
                    "E0214",
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
                        "E0215",
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
        if matches!(token.kind, TokenKind::If) && matches!(self.peek_n(1).kind, TokenKind::Let) {
            return self.parse_if_let_stmt(token);
        }
        if matches!(token.kind, TokenKind::Return) {
            return self.parse_return_stmt(token);
        }
        if matches!(token.kind, TokenKind::For) {
            return self.parse_for_stmt(token);
        }
        if matches!(token.kind, TokenKind::Match) {
            return self.parse_match_stmt(token);
        }
        if matches!(token.kind, TokenKind::Break) {
            self.advance();
            return Ok(Stmt::Break {
                span: Span {
                    line: token.line,
                    column: token.column,
                    length: token.length(),
                    text: token.text,
                },
            });
        }
        if matches!(token.kind, TokenKind::Continue) {
            self.advance();
            return Ok(Stmt::Continue {
                span: Span {
                    line: token.line,
                    column: token.column,
                    length: token.length(),
                    text: token.text,
                },
            });
        }
        if matches!(token.kind, TokenKind::Defer) {
            return self.parse_defer_stmt(token);
        }
        if matches!(token.kind, TokenKind::Unsafe) {
            return self.parse_unsafe_stmt(token);
        }
        if matches!(token.kind, TokenKind::Ident(_))
            && (assign_op_from_token(&self.peek_n(1).kind).is_some()
                || (matches!(self.peek_n(1).kind, TokenKind::Dot)
                    && assign_op_from_token(&self.peek_n(3).kind).is_some()))
        {
            return self.parse_assign_stmt(token);
        }
        if matches!(token.kind, TokenKind::Ident(_))
            && (postfix_op_from_token(&self.peek_n(1).kind).is_some()
                || (matches!(self.peek_n(1).kind, TokenKind::Dot)
                    && postfix_op_from_token(&self.peek_n(3).kind).is_some()))
        {
            return self.parse_postfix_stmt(token);
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
        let target = self.parse_path()?;
        if target.len() > 2 {
            return Err(self.error(
                "E0217",
                "assignment target must be a variable or field",
                token.length(),
            ));
        }
        let op = assign_op_from_token(&self.peek().kind).ok_or_else(|| {
            self.error(
                "E0217",
                "expected assignment operator in assignment",
                token.length(),
            )
        })?;
        self.advance();
        let value = self.parse_expr()?;
        Ok(Stmt::Assign {
            target,
            op,
            value,
            span: Span {
                line: token.line,
                column: token.column,
                length: token.length(),
                text: token.text,
            },
        })
    }

    fn parse_postfix_stmt(&mut self, token: Token) -> Result<Stmt, Diagnostic> {
        let target = self.parse_path()?;
        if target.len() > 2 {
            return Err(self.error(
                "E0217",
                "postfix update target must be a variable or field",
                token.length(),
            ));
        }
        let op = postfix_op_from_token(&self.peek().kind).ok_or_else(|| {
            self.error("E0217", "expected postfix update operator", token.length())
        })?;
        self.advance();
        Ok(Stmt::Postfix {
            target,
            op,
            span: Span {
                line: token.line,
                column: token.column,
                length: token.length(),
                text: token.text,
            },
        })
    }

    fn parse_return_stmt(&mut self, token: Token) -> Result<Stmt, Diagnostic> {
        self.expect_kind(TokenKind::Return, "E0216", "expected `return`")?;
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
        self.expect_kind(TokenKind::Let, "E0212", "expected `let`")?;
        let mutable = if matches!(self.peek().kind, TokenKind::Mut) {
            self.advance();
            true
        } else {
            false
        };
        let name_token = self.peek().clone();
        let path = self.parse_path()?;
        if !mutable && matches!(self.peek().kind, TokenKind::LParen) {
            let binding = self.parse_match_binding()?.ok_or_else(|| {
                Diagnostic::new(
                    "E0234",
                    "expected binding name in let-else pattern",
                    self.path,
                    name_token.line,
                    name_token.column,
                    name_token.length(),
                    &name_token.text,
                )
            })?;
            self.expect_kind(TokenKind::Equal, "E0213", "expected `=` before initializer")?;
            let value = self.parse_expr()?;
            self.expect_kind(
                TokenKind::Else,
                "E0267",
                "expected `else` after let-else initializer",
            )?;
            let else_body = self.parse_stmt_block("E0268", "expected `{` before let-else body")?;
            return Ok(Stmt::LetElse {
                pattern: path,
                binding,
                value,
                else_body,
                span: Span {
                    line: token.line,
                    column: token.column,
                    length: token.length(),
                    text: token.text,
                },
            });
        }
        let [name] = path.as_slice() else {
            return Err(Diagnostic::new(
                "E0212",
                "expected variable name after `let`",
                self.path,
                name_token.line,
                name_token.column,
                name_token.length(),
                &name_token.text,
            ));
        };
        let name = name.clone();
        let type_annotation = if matches!(self.peek().kind, TokenKind::Colon) {
            self.advance();
            Some(self.parse_type_ref()?)
        } else {
            None
        };
        self.expect_kind(TokenKind::Equal, "E0213", "expected `=` before initializer")?;
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

    fn parse_if_let_stmt(&mut self, token: Token) -> Result<Stmt, Diagnostic> {
        self.expect_kind(TokenKind::If, "E0269", "expected `if`")?;
        self.expect_kind(TokenKind::Let, "E0270", "expected `let` after `if`")?;
        let pattern = self.parse_match_pattern()?;
        let binding = self.parse_match_binding()?;
        self.expect_kind(
            TokenKind::Equal,
            "E0271",
            "expected `=` before if-let value",
        )?;
        let value = self.parse_expr_no_struct_literals()?;
        let body = self.parse_stmt_block("E0272", "expected `{` before if-let body")?;
        let else_body = if matches!(self.peek().kind, TokenKind::Else) {
            self.advance();
            Some(self.parse_stmt_block("E0273", "expected `{` before if-let else body")?)
        } else {
            None
        };
        Ok(Stmt::IfLet {
            pattern,
            binding,
            value,
            body,
            else_body,
            span: Span {
                line: token.line,
                column: token.column,
                length: token.length(),
                text: token.text,
            },
        })
    }

    fn parse_for_stmt(&mut self, token: Token) -> Result<Stmt, Diagnostic> {
        self.expect_kind(TokenKind::For, "E0260", "expected `for`")?;
        let variant = if matches!(self.peek().kind, TokenKind::LBrace) {
            // for {}
            ForVariant::Infinite {
                body: self.parse_stmt_block("E0261", "expected `{` before `for` body")?,
            }
        } else if matches!(self.peek().kind, TokenKind::Ident(_))
            && matches!(self.peek_n(1).kind, TokenKind::In)
        {
            // for binding in iterable {}
            let binding = self.expect_ident("expected binding name after `for`")?;
            self.expect_kind(TokenKind::In, "E0262", "expected `in` after `for` binding")?;
            let iterable = self.parse_expr_no_struct_literals()?;
            let body = self.parse_stmt_block("E0263", "expected `{` before `for` body")?;
            ForVariant::Iterate {
                binding,
                iterable,
                body,
            }
        } else {
            // for cond {}
            let condition = self.parse_expr_no_struct_literals()?;
            let body = self.parse_stmt_block("E0264", "expected `{` before `for` body")?;
            ForVariant::While { condition, body }
        };
        Ok(Stmt::For {
            variant,
            span: Span {
                line: token.line,
                column: token.column,
                length: token.length(),
                text: token.text,
            },
        })
    }

    fn parse_match_stmt(&mut self, token: Token) -> Result<Stmt, Diagnostic> {
        self.expect_kind(TokenKind::Match, "E0229", "expected `match`")?;
        let value = self.parse_expr_no_struct_literals()?;
        self.expect_kind(TokenKind::LBrace, "E0230", "expected `{` before match arms")?;
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
                    return Err(self.error("E0231", "unterminated match body; expected `}`", 1));
                }
                _ => {
                    let pattern = self.parse_match_pattern()?;
                    let binding = self.parse_match_binding()?;
                    self.expect_kind(
                        TokenKind::FatArrow,
                        "E0232",
                        "expected `=>` after match pattern",
                    )?;
                    self.skip_newlines();
                    let body =
                        self.parse_stmt_block("E0235", "expected `{` before match arm body")?;
                    arms.push(MatchStmtArm {
                        pattern,
                        binding,
                        body,
                    });
                    self.expect_newline("expected newline after match arm")?;
                }
            }
        }

        Ok(Stmt::Match {
            value,
            arms,
            span: Span {
                line: token.line,
                column: token.column,
                length: token.length(),
                text: token.text,
            },
        })
    }

    fn parse_expr_no_struct_literals(&mut self) -> Result<Expr, Diagnostic> {
        let previous = self.allow_struct_literals;
        self.allow_struct_literals = false;
        let expr = self.parse_expr();
        self.allow_struct_literals = previous;
        expr
    }

    fn parse_defer_stmt(&mut self, token: Token) -> Result<Stmt, Diagnostic> {
        self.expect_kind(TokenKind::Defer, "E0265", "expected `defer`")?;
        let stmt = self.parse_stmt()?;
        Ok(Stmt::Defer {
            stmt: Box::new(stmt),
            span: Span {
                line: token.line,
                column: token.column,
                length: token.length(),
                text: token.text,
            },
        })
    }

    fn parse_unsafe_stmt(&mut self, token: Token) -> Result<Stmt, Diagnostic> {
        self.expect_kind(TokenKind::Unsafe, "E1517", "expected `unsafe`")?;
        let body = self.parse_stmt_block("E1518", "expected `{` before unsafe block")?;
        if body.len() != 1 {
            return Err(Diagnostic::new(
                "E1519",
                "v0.1 unsafe blocks must contain exactly one statement",
                self.path,
                token.line,
                token.column,
                token.length(),
                &token.text,
            ));
        }
        Ok(Stmt::Unsafe {
            body,
            span: Span {
                line: token.line,
                column: token.column,
                length: token.length(),
                text: token.text,
            },
        })
    }

    fn parse_stmt_block(
        &mut self,
        open_code: &'static str,
        open_message: &'static str,
    ) -> Result<Vec<Stmt>, Diagnostic> {
        self.expect_kind(TokenKind::LBrace, open_code, open_message)?;
        self.skip_newlines();
        let mut body = Vec::new();
        loop {
            self.skip_newlines();
            match self.peek().kind {
                TokenKind::RBrace => {
                    self.advance();
                    break;
                }
                TokenKind::Eof => {
                    return Err(self.error("E0266", "unterminated block; expected `}`", 1));
                }
                _ => {
                    body.push(self.parse_stmt()?);
                    self.expect_newline("expected newline after statement")?;
                }
            }
        }
        Ok(body)
    }

    fn parse_const(&mut self, public: bool) -> Result<ConstDef, Diagnostic> {
        let token = self.peek().clone();
        self.expect_kind(TokenKind::Const, "E0267", "expected `const`")?;
        let name = self.expect_ident("expected constant name after `const`")?;
        self.expect_kind(
            TokenKind::Colon,
            "E0268",
            "expected `:` after constant name",
        )?;
        let type_ref = self.parse_type_ref()?;
        self.expect_kind(
            TokenKind::Equal,
            "E0269",
            "expected `=` before constant value",
        )?;
        let value = self.parse_expr()?;
        self.consume_newline();
        Ok(ConstDef {
            public,
            name,
            type_ref,
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
        self.parse_logical_or_expr()
    }

    fn parse_logical_or_expr(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_logical_and_expr()?;
        while matches!(self.peek().kind, TokenKind::PipePipe) {
            self.advance();
            self.skip_newlines();
            let right = self.parse_logical_and_expr()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op: BinaryOp::LogicalOr,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_logical_and_expr(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_equality_expr()?;
        while matches!(self.peek().kind, TokenKind::AmpAmp) {
            self.advance();
            self.skip_newlines();
            let right = self.parse_equality_expr()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op: BinaryOp::LogicalAnd,
                right: Box::new(right),
            };
        }
        Ok(expr)
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
            self.skip_newlines();
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
        let mut expr = self.parse_multiplicative_expr()?;
        loop {
            let op = match self.peek().kind {
                TokenKind::Plus => BinaryOp::Add,
                TokenKind::Minus => BinaryOp::Subtract,
                TokenKind::Pipe => BinaryOp::BitOr,
                TokenKind::Caret => BinaryOp::BitXor,
                _ => break,
            };
            self.advance();
            self.skip_newlines();
            let right = self.parse_multiplicative_expr()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_multiplicative_expr(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_cast_expr()?;
        loop {
            let op = match self.peek().kind {
                TokenKind::Star => BinaryOp::Multiply,
                TokenKind::Slash => BinaryOp::Divide,
                TokenKind::Percent => BinaryOp::Remainder,
                TokenKind::LessLess => BinaryOp::ShiftLeft,
                TokenKind::GreaterGreater => BinaryOp::ShiftRight,
                TokenKind::Amp => BinaryOp::BitAnd,
                TokenKind::AmpCaret => BinaryOp::BitAndNot,
                _ => break,
            };
            self.advance();
            self.skip_newlines();
            let right = self.parse_cast_expr()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_cast_expr(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_unary_expr()?;
        while matches!(self.peek().kind, TokenKind::As) {
            self.advance();
            self.skip_newlines();
            let target = self.parse_type_ref()?;
            expr = Expr::Cast {
                expr: Box::new(expr),
                target,
            };
        }
        Ok(expr)
    }

    fn parse_unary_expr(&mut self) -> Result<Expr, Diagnostic> {
        match self.peek().kind {
            TokenKind::Bang => {
                self.advance();
                let expr = self.parse_unary_expr()?;
                Ok(Expr::Unary {
                    op: crate::ast::UnaryOp::Not,
                    expr: Box::new(expr),
                })
            }
            TokenKind::Minus => {
                self.advance();
                let expr = self.parse_unary_expr()?;
                Ok(Expr::Unary {
                    op: crate::ast::UnaryOp::Negate,
                    expr: Box::new(expr),
                })
            }
            _ => self.parse_postfix_expr(),
        }
    }

    fn parse_postfix_expr(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_primary_expr()?;
        while matches!(self.peek().kind, TokenKind::Question) {
            self.advance();
            expr = Expr::Question {
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
            TokenKind::LParen => {
                self.advance();
                self.skip_newlines();
                let expr = self.parse_expr()?;
                self.skip_newlines();
                self.expect_kind(
                    TokenKind::RParen,
                    "E0209",
                    "expected `)` after parenthesized expression",
                )?;
                Ok(expr)
            }
            TokenKind::If => self.parse_if_expr(),
            TokenKind::Match => self.parse_match_expr(),
            TokenKind::Panic => self.parse_panic_expr(),
            TokenKind::Ident(_) => self.parse_name_or_call(),
            _ => Err(self.error("E0208", "expected expression", self.peek().length())),
        }
    }

    fn parse_panic_expr(&mut self) -> Result<Expr, Diagnostic> {
        self.expect_kind(TokenKind::Panic, "E0246", "expected `panic`")?;
        self.expect_kind(TokenKind::LParen, "E0247", "expected `(` after `panic`")?;
        let message = self.parse_expr()?;
        self.expect_kind(
            TokenKind::RParen,
            "E0248",
            "expected `)` after panic message",
        )?;
        Ok(Expr::Panic {
            message: Box::new(message),
        })
    }

    fn parse_if_expr(&mut self) -> Result<Expr, Diagnostic> {
        self.expect_kind(TokenKind::If, "E0240", "expected `if`")?;
        let previous = self.allow_struct_literals;
        self.allow_struct_literals = false;
        let condition = self.parse_expr();
        self.allow_struct_literals = previous;
        let condition = condition?;
        let then_branch = self.parse_expr_block("E0241", "expected `{` before if branch")?;
        self.skip_newlines();
        self.expect_kind(TokenKind::Else, "E0244", "expected `else` after if branch")?;
        let else_branch = self.parse_expr_block("E0245", "expected `{` before else branch")?;
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
            "E0242",
            "expected `}` after expression block",
        )?;
        Ok(value)
    }

    fn parse_match_expr(&mut self) -> Result<Expr, Diagnostic> {
        self.expect_kind(TokenKind::Match, "E0229", "expected `match`")?;
        let value = self.parse_expr_no_struct_literals()?;
        self.expect_kind(TokenKind::LBrace, "E0230", "expected `{` before match arms")?;
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
                    return Err(self.error("E0231", "unterminated match body; expected `}`", 1));
                }
                _ => {
                    let pattern = self.parse_match_pattern()?;
                    let binding = self.parse_match_binding()?;
                    self.expect_kind(
                        TokenKind::FatArrow,
                        "E0232",
                        "expected `=>` after match pattern",
                    )?;
                    self.skip_newlines();
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

    fn parse_match_binding(&mut self) -> Result<Option<String>, Diagnostic> {
        if !matches!(self.peek().kind, TokenKind::LParen) {
            return Ok(None);
        }
        self.advance();
        let binding_token = self.peek().clone();
        let binding = self.expect_ident("expected binding name in match pattern")?;
        if binding == "_" {
            return Err(Diagnostic::new(
                "E0238",
                "`_` match bindings are not supported in v0.1",
                self.path,
                binding_token.line,
                binding_token.column,
                binding_token.length(),
                &binding_token.text,
            ));
        }
        self.expect_kind(
            TokenKind::RParen,
            "E0234",
            "expected `)` after match pattern binding",
        )?;
        Ok(Some(binding))
    }

    fn parse_match_pattern(&mut self) -> Result<Vec<String>, Diagnostic> {
        let pattern_token = self.peek().clone();
        let pattern = self.parse_path()?;
        if pattern.len() == 1 && pattern[0] == "_" {
            return Err(Diagnostic::new(
                "E0238",
                "`_` match patterns are not supported in v0.1",
                self.path,
                pattern_token.line,
                pattern_token.column,
                pattern_token.length(),
                &pattern_token.text,
            ));
        }
        Ok(pattern)
    }

    fn parse_name_or_call(&mut self) -> Result<Expr, Diagnostic> {
        let path = self.parse_path()?;
        if path.len() == 1 && path[0] == "try" && starts_try_operand(&self.peek().kind) {
            return Err(self.error(
                "E0211",
                "`try` propagation syntax is not supported; use postfix `?` instead",
                self.peek().length(),
            ));
        }
        if self.allow_struct_literals && matches!(self.peek().kind, TokenKind::LBrace) {
            return self.parse_struct_literal(path);
        }
        let type_args = if self.next_tokens_are_call_type_args() {
            self.parse_type_args()?
        } else {
            Vec::new()
        };
        if !matches!(self.peek().kind, TokenKind::LParen) {
            if !type_args.is_empty() {
                return Err(self.error(
                    "E0210",
                    "expected `(` after generic call type arguments",
                    self.peek().length(),
                ));
            }
            return Ok(Expr::Name(path));
        }
        self.advance();
        let mut args = Vec::new();
        self.skip_newlines();
        if !matches!(self.peek().kind, TokenKind::RParen) {
            args.push(self.parse_call_arg()?);
            loop {
                self.skip_newlines();
                if !matches!(self.peek().kind, TokenKind::Comma) {
                    break;
                }
                self.advance();
                self.skip_newlines();
                args.push(self.parse_call_arg()?);
            }
        }
        self.skip_newlines();
        self.expect_kind(
            TokenKind::RParen,
            "E0210",
            "expected `)` after call arguments",
        )?;
        Ok(Expr::Call {
            callee: path,
            type_args,
            args,
        })
    }

    fn parse_call_arg(&mut self) -> Result<Expr, Diagnostic> {
        if matches!(self.peek().kind, TokenKind::Mut) {
            self.advance();
            return Ok(Expr::MutArg {
                name: self.parse_path()?,
            });
        }
        self.parse_expr()
    }

    fn next_tokens_are_call_type_args(&self) -> bool {
        if !matches!(self.peek().kind, TokenKind::Less) {
            return false;
        }
        let mut depth = 0usize;
        let mut index = self.index;
        while let Some(token) = self.tokens.get(index) {
            match token.kind {
                TokenKind::Less => depth += 1,
                TokenKind::Greater => {
                    if depth == 0 {
                        return false;
                    }
                    depth -= 1;
                    if depth == 0 {
                        let mut next_index = index + 1;
                        while self
                            .tokens
                            .get(next_index)
                            .is_some_and(|next| matches!(next.kind, TokenKind::Newline))
                        {
                            next_index += 1;
                        }
                        return self
                            .tokens
                            .get(next_index)
                            .is_some_and(|next| matches!(next.kind, TokenKind::LParen));
                    }
                }
                TokenKind::GreaterGreater => {
                    if depth < 2 {
                        return false;
                    }
                    depth -= 2;
                    if depth == 0 {
                        let mut next_index = index + 1;
                        while self
                            .tokens
                            .get(next_index)
                            .is_some_and(|next| matches!(next.kind, TokenKind::Newline))
                        {
                            next_index += 1;
                        }
                        return self
                            .tokens
                            .get(next_index)
                            .is_some_and(|next| matches!(next.kind, TokenKind::LParen));
                    }
                }
                TokenKind::Eof => return false,
                _ => {}
            }
            index += 1;
        }
        false
    }

    fn parse_struct_literal(&mut self, type_name: Vec<String>) -> Result<Expr, Diagnostic> {
        self.expect_kind(
            TokenKind::LBrace,
            "E0222",
            "expected `{` before struct literal fields",
        )?;
        let mut fields = Vec::new();
        self.skip_newlines();
        if !matches!(self.peek().kind, TokenKind::RBrace) {
            loop {
                let field_name = self.expect_ident("expected struct literal field name")?;
                self.expect_kind(
                    TokenKind::Colon,
                    "E0223",
                    "expected `:` after struct literal field name",
                )?;
                let value = self.parse_expr()?;
                fields.push((field_name, value));
                self.skip_newlines();
                match self.peek().kind {
                    TokenKind::Comma => {
                        self.advance();
                        self.skip_newlines();
                        if matches!(self.peek().kind, TokenKind::RBrace) {
                            break;
                        }
                    }
                    TokenKind::RBrace => break,
                    _ => {
                        return Err(self.error(
                            "E0224",
                            "expected `,` or `}` after struct literal field",
                            self.peek().length(),
                        ));
                    }
                }
            }
        }
        self.skip_newlines();
        self.expect_kind(
            TokenKind::RBrace,
            "E0225",
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
        self.skip_newlines();
        loop {
            args.push(self.parse_type_ref()?);
            self.skip_newlines();
            if self.pending_type_gt > 0 {
                self.pending_type_gt -= 1;
                break;
            }
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
#[path = "parser_tests.rs"]
mod tests;

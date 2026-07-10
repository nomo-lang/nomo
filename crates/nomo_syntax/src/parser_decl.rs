use super::*;
use crate::ast::TypeParamBound;

struct ParsedTypeParams {
    names: Vec<String>,
    bounds: Vec<TypeParamBound>,
}

impl Parser<'_> {
    pub(super) fn parse_test_attribute(&mut self) -> Result<bool, Diagnostic> {
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

    pub(super) fn parse_enum(&mut self, public: bool) -> Result<EnumDef, Diagnostic> {
        let enum_token = self.peek().clone();
        self.expect_kind(TokenKind::Enum, "E0226", "expected `enum`")?;
        let name = self.expect_ident("expected enum name")?;
        let type_params = self.parse_type_params(false)?.names;
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

    fn parse_type_params(&mut self, allow_bounds: bool) -> Result<ParsedTypeParams, Diagnostic> {
        if !matches!(self.peek().kind, TokenKind::Less) {
            return Ok(ParsedTypeParams {
                names: Vec::new(),
                bounds: Vec::new(),
            });
        }
        self.advance();
        let mut names = Vec::new();
        let mut bounds = Vec::new();
        loop {
            let name = self.expect_ident("expected generic type parameter name")?;
            if names.iter().any(|param| param == &name) {
                return Err(self.error(
                    "E0237",
                    format!("generic type parameter `{name}` is already defined"),
                    self.peek().length(),
                ));
            }
            names.push(name.clone());
            if matches!(self.peek().kind, TokenKind::Colon) {
                if !allow_bounds {
                    return Err(self.error(
                        "E0235",
                        "interface bounds are only supported on generic functions",
                        self.peek().length(),
                    ));
                }
                self.advance();
                bounds.push(TypeParamBound {
                    parameter: name,
                    interface: self.parse_type_ref()?,
                });
            }
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
        Ok(ParsedTypeParams { names, bounds })
    }

    pub(super) fn parse_struct(&mut self, public: bool) -> Result<StructDef, Diagnostic> {
        let struct_token = self.peek().clone();
        self.expect_kind(TokenKind::Struct, "E0218", "expected `struct`")?;
        let name = self.expect_ident("expected struct name")?;
        let type_params = self.parse_type_params(false)?.names;
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

    pub(super) fn parse_function(
        &mut self,
        public: bool,
        is_test: bool,
    ) -> Result<Function, Diagnostic> {
        let function_token = self.peek().clone();
        self.expect_kind(TokenKind::Fn, "E0202", "expected `fn`")?;
        let name = self.expect_ident("expected function name")?;
        let type_params = self.parse_type_params(true)?;
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
            type_params: type_params.names,
            type_param_bounds: type_params.bounds,
            params,
            return_type,
            body,
            span: token_span(&function_token),
        })
    }

    pub(super) fn parse_interface(&mut self, public: bool) -> Result<InterfaceDef, Diagnostic> {
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

    pub(super) fn parse_extern_block(&mut self) -> Result<ExternBlock, Diagnostic> {
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

    pub(super) fn parse_impl(&mut self) -> Result<ImplBlock, Diagnostic> {
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
        let type_params = self.parse_type_params(false)?;
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
            type_params: type_params.names,
            type_param_bounds: type_params.bounds,
            params,
            return_type,
            span: token_span(&function_token),
        })
    }

    pub(super) fn consume_pub(&mut self) -> bool {
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

    pub(super) fn parse_const(&mut self, public: bool) -> Result<ConstDef, Diagnostic> {
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
}

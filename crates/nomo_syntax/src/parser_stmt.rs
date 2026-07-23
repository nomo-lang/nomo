use super::*;

impl Parser<'_> {
    pub(super) fn parse_stmt(&mut self) -> Result<Stmt, Diagnostic> {
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
        let value = if matches!(
            self.peek().kind,
            TokenKind::Newline | TokenKind::RBrace | TokenKind::Semicolon
        ) {
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
        } else if matches!(self.peek().kind, TokenKind::Let) {
            // for let [mut] binding [: type] = initializer; condition; update {}
            self.advance();
            if matches!(self.peek().kind, TokenKind::Mut) {
                self.advance();
            }
            let binding = self.expect_ident("expected binding name after `for let`")?;
            let type_annotation = if matches!(self.peek().kind, TokenKind::Colon) {
                self.advance();
                Some(self.parse_type_ref()?)
            } else {
                None
            };
            self.expect_kind(
                TokenKind::Equal,
                "E0213",
                "expected `=` before for-loop initializer",
            )?;
            let initializer = self.parse_expr()?;
            self.expect_kind(
                TokenKind::Semicolon,
                "E0264",
                "expected `;` after for-loop initializer",
            )?;
            let condition = self.parse_expr_no_struct_literals()?;
            self.expect_kind(
                TokenKind::Semicolon,
                "E0264",
                "expected `;` after for-loop condition",
            )?;
            let update = self.parse_stmt()?;
            if !matches!(update, Stmt::Assign { .. } | Stmt::Postfix { .. }) {
                return Err(self.error(
                    "E0217",
                    "for-loop update must assign to or increment/decrement the loop binding",
                    self.peek().length(),
                ));
            }
            let body = self.parse_stmt_block("E0264", "expected `{` before `for` body")?;
            ForVariant::CStyle {
                binding,
                type_annotation,
                initializer,
                condition,
                update: Box::new(update),
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

    pub(super) fn parse_stmt_block(
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
}

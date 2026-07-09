use super::*;

impl Parser<'_> {
    pub(super) fn parse_expr_no_struct_literals(&mut self) -> Result<Expr, Diagnostic> {
        let previous = self.allow_struct_literals;
        self.allow_struct_literals = false;
        let expr = self.parse_expr();
        self.allow_struct_literals = previous;
        expr
    }

    pub(super) fn parse_expr(&mut self) -> Result<Expr, Diagnostic> {
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

    pub(super) fn parse_match_binding(&mut self) -> Result<Option<String>, Diagnostic> {
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

    pub(super) fn parse_match_pattern(&mut self) -> Result<Vec<String>, Diagnostic> {
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
}

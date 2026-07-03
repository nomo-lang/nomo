use crate::diagnostic::Diagnostic;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub column: usize,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Package,
    Import,
    Pub,
    Impl,
    Fn,
    Struct,
    Enum,
    If,
    Else,
    Match,
    Panic,
    As,
    Let,
    Mut,
    Return,
    Void,
    True,
    False,
    For,
    In,
    Break,
    Continue,
    Defer,
    Const,
    Ident(String),
    String(String),
    Int(i64),
    Float(String),
    Char(char),
    Dot,
    Comma,
    Colon,
    Equal,
    EqualEqual,
    PlusEqual,
    MinusEqual,
    StarEqual,
    SlashEqual,
    PercentEqual,
    Bang,
    BangEqual,
    Amp,
    AmpEqual,
    AmpAmp,
    AmpCaret,
    AmpCaretEqual,
    Pipe,
    PipeEqual,
    PipePipe,
    Caret,
    CaretEqual,
    Plus,
    PlusPlus,
    Minus,
    MinusMinus,
    Star,
    Slash,
    Percent,
    Question,
    Less,
    LessEqual,
    LessLess,
    LessLessEqual,
    Greater,
    GreaterEqual,
    GreaterGreater,
    GreaterGreaterEqual,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Arrow,
    FatArrow,
    Newline,
    Eof,
}

pub fn lex(path: &Path, source: &str) -> Result<Vec<Token>, Diagnostic> {
    let mut tokens = Vec::new();
    let mut block_comment_depth = 0usize;
    let mut block_comment_start: Option<(usize, usize, String)> = None;
    for (line_index, line_text) in source.lines().enumerate() {
        let line = line_index + 1;
        let mut chars = line_text.char_indices().peekable();

        while let Some((index, ch)) = chars.next() {
            let column = index + 1;
            if block_comment_depth > 0 {
                match ch {
                    '/' if matches!(chars.peek(), Some((_, '*'))) => {
                        chars.next();
                        block_comment_depth += 1;
                    }
                    '*' if matches!(chars.peek(), Some((_, '/'))) => {
                        chars.next();
                        block_comment_depth -= 1;
                        if block_comment_depth == 0 {
                            block_comment_start = None;
                        }
                    }
                    _ => {}
                }
                continue;
            }
            match ch {
                ' ' | '\t' | '\r' => {}
                '/' => {
                    if matches!(chars.peek(), Some((_, '/'))) {
                        break;
                    } else if matches!(chars.peek(), Some((_, '*'))) {
                        chars.next();
                        block_comment_depth = 1;
                        block_comment_start = Some((line, column, line_text.to_string()));
                    } else if matches!(chars.peek(), Some((_, '='))) {
                        chars.next();
                        tokens.push(token(TokenKind::SlashEqual, line, column, line_text));
                    } else {
                        tokens.push(token(TokenKind::Slash, line, column, line_text));
                    }
                }
                '.' => tokens.push(token(TokenKind::Dot, line, column, line_text)),
                ',' => tokens.push(token(TokenKind::Comma, line, column, line_text)),
                ':' => tokens.push(token(TokenKind::Colon, line, column, line_text)),
                '=' => {
                    if matches!(chars.peek(), Some((_, '>'))) {
                        chars.next();
                        tokens.push(token(TokenKind::FatArrow, line, column, line_text));
                    } else if matches!(chars.peek(), Some((_, '='))) {
                        chars.next();
                        tokens.push(token(TokenKind::EqualEqual, line, column, line_text));
                    } else {
                        tokens.push(token(TokenKind::Equal, line, column, line_text));
                    }
                }
                '!' => {
                    if matches!(chars.peek(), Some((_, '='))) {
                        chars.next();
                        tokens.push(token(TokenKind::BangEqual, line, column, line_text));
                    } else {
                        tokens.push(token(TokenKind::Bang, line, column, line_text));
                    }
                }
                '&' => {
                    if matches!(chars.peek(), Some((_, '&'))) {
                        chars.next();
                        tokens.push(token(TokenKind::AmpAmp, line, column, line_text));
                    } else if matches!(chars.peek(), Some((_, '^'))) {
                        chars.next();
                        if matches!(chars.peek(), Some((_, '='))) {
                            chars.next();
                            tokens.push(token(TokenKind::AmpCaretEqual, line, column, line_text));
                        } else {
                            tokens.push(token(TokenKind::AmpCaret, line, column, line_text));
                        }
                    } else if matches!(chars.peek(), Some((_, '='))) {
                        chars.next();
                        tokens.push(token(TokenKind::AmpEqual, line, column, line_text));
                    } else {
                        tokens.push(token(TokenKind::Amp, line, column, line_text));
                    }
                }
                '|' => {
                    if matches!(chars.peek(), Some((_, '|'))) {
                        chars.next();
                        tokens.push(token(TokenKind::PipePipe, line, column, line_text));
                    } else if matches!(chars.peek(), Some((_, '='))) {
                        chars.next();
                        tokens.push(token(TokenKind::PipeEqual, line, column, line_text));
                    } else {
                        tokens.push(token(TokenKind::Pipe, line, column, line_text));
                    }
                }
                '^' => {
                    if matches!(chars.peek(), Some((_, '='))) {
                        chars.next();
                        tokens.push(token(TokenKind::CaretEqual, line, column, line_text));
                    } else {
                        tokens.push(token(TokenKind::Caret, line, column, line_text));
                    }
                }
                '+' => {
                    if matches!(chars.peek(), Some((_, '+'))) {
                        chars.next();
                        tokens.push(token(TokenKind::PlusPlus, line, column, line_text));
                    } else if matches!(chars.peek(), Some((_, '='))) {
                        chars.next();
                        tokens.push(token(TokenKind::PlusEqual, line, column, line_text));
                    } else {
                        tokens.push(token(TokenKind::Plus, line, column, line_text));
                    }
                }
                '-' => {
                    if matches!(chars.peek(), Some((_, '>'))) {
                        chars.next();
                        tokens.push(token(TokenKind::Arrow, line, column, line_text));
                    } else if matches!(chars.peek(), Some((_, '-'))) {
                        chars.next();
                        tokens.push(token(TokenKind::MinusMinus, line, column, line_text));
                    } else if matches!(chars.peek(), Some((_, '='))) {
                        chars.next();
                        tokens.push(token(TokenKind::MinusEqual, line, column, line_text));
                    } else {
                        tokens.push(token(TokenKind::Minus, line, column, line_text));
                    }
                }
                '*' => {
                    if matches!(chars.peek(), Some((_, '='))) {
                        chars.next();
                        tokens.push(token(TokenKind::StarEqual, line, column, line_text));
                    } else {
                        tokens.push(token(TokenKind::Star, line, column, line_text));
                    }
                }
                '%' => {
                    if matches!(chars.peek(), Some((_, '='))) {
                        chars.next();
                        tokens.push(token(TokenKind::PercentEqual, line, column, line_text));
                    } else {
                        tokens.push(token(TokenKind::Percent, line, column, line_text));
                    }
                }
                '?' => tokens.push(token(TokenKind::Question, line, column, line_text)),
                '<' => {
                    if matches!(chars.peek(), Some((_, '<'))) {
                        chars.next();
                        if matches!(chars.peek(), Some((_, '='))) {
                            chars.next();
                            tokens.push(token(TokenKind::LessLessEqual, line, column, line_text));
                        } else {
                            tokens.push(token(TokenKind::LessLess, line, column, line_text));
                        }
                    } else if matches!(chars.peek(), Some((_, '='))) {
                        chars.next();
                        tokens.push(token(TokenKind::LessEqual, line, column, line_text));
                    } else {
                        tokens.push(token(TokenKind::Less, line, column, line_text));
                    }
                }
                '>' => {
                    if matches!(chars.peek(), Some((_, '>'))) {
                        chars.next();
                        if matches!(chars.peek(), Some((_, '='))) {
                            chars.next();
                            tokens.push(token(
                                TokenKind::GreaterGreaterEqual,
                                line,
                                column,
                                line_text,
                            ));
                        } else {
                            tokens.push(token(TokenKind::GreaterGreater, line, column, line_text));
                        }
                    } else if matches!(chars.peek(), Some((_, '='))) {
                        chars.next();
                        tokens.push(token(TokenKind::GreaterEqual, line, column, line_text));
                    } else {
                        tokens.push(token(TokenKind::Greater, line, column, line_text));
                    }
                }
                '(' => tokens.push(token(TokenKind::LParen, line, column, line_text)),
                ')' => tokens.push(token(TokenKind::RParen, line, column, line_text)),
                '{' => tokens.push(token(TokenKind::LBrace, line, column, line_text)),
                '}' => tokens.push(token(TokenKind::RBrace, line, column, line_text)),
                ';' => {
                    return Err(Diagnostic::new(
                        "E0102",
                        "semicolons are not supported in v0.1; use a newline to separate statements",
                        path,
                        line,
                        column,
                        1,
                        line_text,
                    ));
                }
                '"' => {
                    let mut literal = String::new();
                    let mut terminated = false;
                    while let Some((_, next)) = chars.next() {
                        match next {
                            '"' => {
                                terminated = true;
                                break;
                            }
                            '\\' => match chars.next() {
                                Some((_, 'n')) => literal.push('\n'),
                                Some((_, 'r')) => literal.push('\r'),
                                Some((_, 't')) => literal.push('\t'),
                                Some((_, '"')) => literal.push('"'),
                                Some((_, '\\')) => literal.push('\\'),
                                Some((_, other)) => {
                                    literal.push('\\');
                                    literal.push(other);
                                }
                                None => literal.push('\\'),
                            },
                            other => literal.push(other),
                        }
                    }
                    if !terminated {
                        return Err(Diagnostic::new(
                            "E0101",
                            "unterminated string literal",
                            path,
                            line,
                            column,
                            1,
                            line_text,
                        ));
                    }
                    tokens.push(token(TokenKind::String(literal), line, column, line_text));
                }
                '\'' => {
                    let Some((_, value)) = chars.next() else {
                        return Err(Diagnostic::new(
                            "E0104",
                            "unterminated char literal",
                            path,
                            line,
                            column,
                            1,
                            line_text,
                        ));
                    };
                    let literal = if value == '\\' {
                        match chars.next() {
                            Some((_, 'n')) => '\n',
                            Some((_, 'r')) => '\r',
                            Some((_, 't')) => '\t',
                            Some((_, '\'')) => '\'',
                            Some((_, '\\')) => '\\',
                            Some((_, other)) => {
                                return Err(Diagnostic::new(
                                    "E0105",
                                    format!("unknown char escape `\\{other}`"),
                                    path,
                                    line,
                                    column,
                                    1,
                                    line_text,
                                ));
                            }
                            None => {
                                return Err(Diagnostic::new(
                                    "E0104",
                                    "unterminated char literal",
                                    path,
                                    line,
                                    column,
                                    1,
                                    line_text,
                                ));
                            }
                        }
                    } else if value == '\'' {
                        return Err(Diagnostic::new(
                            "E0106",
                            "empty char literal",
                            path,
                            line,
                            column,
                            1,
                            line_text,
                        ));
                    } else {
                        value
                    };
                    if !matches!(chars.next(), Some((_, '\''))) {
                        return Err(Diagnostic::new(
                            "E0107",
                            "char literal must contain exactly one character",
                            path,
                            line,
                            column,
                            1,
                            line_text,
                        ));
                    }
                    tokens.push(token(TokenKind::Char(literal), line, column, line_text));
                }
                c if c.is_ascii_digit() => {
                    let mut value = String::from(c);
                    while let Some((_, next)) = chars.peek() {
                        if next.is_ascii_digit() {
                            value.push(*next);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    if matches!(chars.peek(), Some((_, '.'))) {
                        let mut lookahead = chars.clone();
                        lookahead.next();
                        if matches!(lookahead.peek(), Some((_, next)) if next.is_ascii_digit()) {
                            value.push('.');
                            chars.next();
                            while let Some((_, next)) = chars.peek() {
                                if next.is_ascii_digit() {
                                    value.push(*next);
                                    chars.next();
                                } else {
                                    break;
                                }
                            }
                            tokens.push(token(TokenKind::Float(value), line, column, line_text));
                            continue;
                        }
                    }
                    let parsed = value.parse::<i64>().map_err(|_| {
                        Diagnostic::new(
                            "E0103",
                            "integer literal is too large for `i64`",
                            path,
                            line,
                            column,
                            value.len(),
                            line_text,
                        )
                    })?;
                    tokens.push(token(TokenKind::Int(parsed), line, column, line_text));
                }
                c if is_ident_start(c) => {
                    let mut value = String::from(c);
                    while let Some((_, next)) = chars.peek() {
                        if is_ident_continue(*next) {
                            value.push(*next);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    if is_reserved_word(&value) {
                        return Err(Diagnostic::new(
                            "E0104",
                            format!("`{value}` is reserved for future Nomo versions"),
                            path,
                            line,
                            column,
                            value.len(),
                            line_text,
                        ));
                    }
                    let kind = match value.as_str() {
                        "package" => TokenKind::Package,
                        "import" => TokenKind::Import,
                        "pub" => TokenKind::Pub,
                        "impl" => TokenKind::Impl,
                        "fn" => TokenKind::Fn,
                        "struct" => TokenKind::Struct,
                        "enum" => TokenKind::Enum,
                        "if" => TokenKind::If,
                        "else" => TokenKind::Else,
                        "match" => TokenKind::Match,
                        "panic" => TokenKind::Panic,
                        "as" => TokenKind::As,
                        "let" => TokenKind::Let,
                        "mut" => TokenKind::Mut,
                        "return" => TokenKind::Return,
                        "void" => TokenKind::Void,
                        "true" => TokenKind::True,
                        "false" => TokenKind::False,
                        "for" => TokenKind::For,
                        "in" => TokenKind::In,
                        "break" => TokenKind::Break,
                        "continue" => TokenKind::Continue,
                        "defer" => TokenKind::Defer,
                        "const" => TokenKind::Const,
                        _ => TokenKind::Ident(value),
                    };
                    tokens.push(token(kind, line, column, line_text));
                }
                other => {
                    return Err(Diagnostic::new(
                        "E0102",
                        format!("unexpected character `{other}`"),
                        path,
                        line,
                        column,
                        other.len_utf8(),
                        line_text,
                    ));
                }
            }
        }

        tokens.push(token(
            TokenKind::Newline,
            line,
            line_text.len() + 1,
            line_text,
        ));
    }

    if let Some((line, column, text)) = block_comment_start {
        return Err(Diagnostic::new(
            "E0108",
            "unterminated block comment",
            path,
            line,
            column,
            2,
            &text,
        ));
    }

    let eof_line = source.lines().count().max(1);
    tokens.push(Token {
        kind: TokenKind::Eof,
        line: eof_line,
        column: 1,
        text: String::new(),
    });
    Ok(tokens)
}

fn token(kind: TokenKind, line: usize, column: usize, text: &str) -> Token {
    Token {
        kind,
        line,
        column,
        text: text.to_string(),
    }
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn is_reserved_word(value: &str) -> bool {
    matches!(
        value,
        "interface" | "unsafe" | "extern" | "export" | "go" | "chan" | "null"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexes_package_and_println() {
        let tokens = lex(
            Path::new("main.nomo"),
            "package app.main\nio.println(\"hi\")\n",
        )
        .unwrap();

        assert!(tokens.iter().any(|token| token.kind == TokenKind::Package));
        assert!(tokens.iter().any(|token| token.kind == TokenKind::Dot));
        assert!(
            tokens
                .iter()
                .any(|token| token.kind == TokenKind::String("hi".to_string()))
        );
    }

    #[test]
    fn lexes_let_and_basic_literals() {
        let tokens = lex(
            Path::new("main.nomo"),
            "let mut answer: i64 = 42\nlet ok = true\n",
        )
        .unwrap();

        assert!(tokens.iter().any(|token| token.kind == TokenKind::Let));
        assert!(tokens.iter().any(|token| token.kind == TokenKind::Mut));
        assert!(tokens.iter().any(|token| token.kind == TokenKind::Colon));
        assert!(tokens.iter().any(|token| token.kind == TokenKind::Equal));
        assert!(tokens.iter().any(|token| token.kind == TokenKind::Int(42)));
        assert!(tokens.iter().any(|token| token.kind == TokenKind::True));
    }

    #[test]
    fn lexes_return_and_plus() {
        let tokens = lex(Path::new("main.nomo"), "return a + b\n").unwrap();

        assert!(tokens.iter().any(|token| token.kind == TokenKind::Return));
        assert!(tokens.iter().any(|token| token.kind == TokenKind::Plus));
    }

    #[test]
    fn lexes_binary_arithmetic_operators() {
        let tokens = lex(Path::new("main.nomo"), "return a - b * c / d % e\n").unwrap();

        assert!(tokens.iter().any(|token| token.kind == TokenKind::Minus));
        assert!(tokens.iter().any(|token| token.kind == TokenKind::Star));
        assert!(tokens.iter().any(|token| token.kind == TokenKind::Slash));
        assert!(tokens.iter().any(|token| token.kind == TokenKind::Percent));
    }

    #[test]
    fn lexes_logical_operators() {
        let tokens = lex(Path::new("main.nomo"), "return !ready && ok || fallback\n").unwrap();

        assert!(tokens.iter().any(|token| token.kind == TokenKind::Bang));
        assert!(tokens.iter().any(|token| token.kind == TokenKind::AmpAmp));
        assert!(tokens.iter().any(|token| token.kind == TokenKind::PipePipe));
    }

    #[test]
    fn lexes_bitwise_operators() {
        let tokens = lex(
            Path::new("main.nomo"),
            "return a & b &^ c << d >> e | f ^ g\n",
        )
        .unwrap();

        assert!(tokens.iter().any(|token| token.kind == TokenKind::Amp));
        assert!(tokens.iter().any(|token| token.kind == TokenKind::AmpCaret));
        assert!(tokens.iter().any(|token| token.kind == TokenKind::LessLess));
        assert!(
            tokens
                .iter()
                .any(|token| token.kind == TokenKind::GreaterGreater)
        );
        assert!(tokens.iter().any(|token| token.kind == TokenKind::Pipe));
        assert!(tokens.iter().any(|token| token.kind == TokenKind::Caret));
    }

    #[test]
    fn lexes_compound_assignment_operators() {
        let tokens = lex(
            Path::new("main.nomo"),
            "a += b\na -= b\na *= b\na /= b\na %= b\na <<= b\na >>= b\na &= b\na ^= b\na |= b\na &^= b\n",
        )
        .unwrap();

        for kind in [
            TokenKind::PlusEqual,
            TokenKind::MinusEqual,
            TokenKind::StarEqual,
            TokenKind::SlashEqual,
            TokenKind::PercentEqual,
            TokenKind::LessLessEqual,
            TokenKind::GreaterGreaterEqual,
            TokenKind::AmpEqual,
            TokenKind::CaretEqual,
            TokenKind::PipeEqual,
            TokenKind::AmpCaretEqual,
        ] {
            assert!(
                tokens
                    .iter()
                    .any(|token| std::mem::discriminant(&token.kind)
                        == std::mem::discriminant(&kind))
            );
        }
    }

    #[test]
    fn lexes_postfix_update_operators() {
        let tokens = lex(Path::new("main.nomo"), "a++\na--\n").unwrap();

        assert!(tokens.iter().any(|token| token.kind == TokenKind::PlusPlus));
        assert!(
            tokens
                .iter()
                .any(|token| token.kind == TokenKind::MinusMinus)
        );
    }

    #[test]
    fn lexes_star_token() {
        let tokens = lex(Path::new("main.nomo"), "import std.io.*\n").unwrap();

        assert!(tokens.iter().any(|token| token.kind == TokenKind::Star));
    }

    #[test]
    fn lexes_if_else_and_comparison_operators() {
        let tokens = lex(
            Path::new("main.nomo"),
            "if score >= 60 { true } else { score != 0 }\n",
        )
        .unwrap();

        assert!(tokens.iter().any(|token| token.kind == TokenKind::If));
        assert!(tokens.iter().any(|token| token.kind == TokenKind::Else));
        assert!(
            tokens
                .iter()
                .any(|token| token.kind == TokenKind::GreaterEqual)
        );
        assert!(
            tokens
                .iter()
                .any(|token| token.kind == TokenKind::BangEqual)
        );
    }

    #[test]
    fn lexes_struct_keyword() {
        let tokens = lex(Path::new("main.nomo"), "struct Point {\n}\n").unwrap();

        assert!(tokens.iter().any(|token| token.kind == TokenKind::Struct));
    }

    #[test]
    fn lexes_pub_keyword() {
        let tokens = lex(Path::new("main.nomo"), "pub fn main() -> void {\n}\n").unwrap();

        assert!(tokens.iter().any(|token| token.kind == TokenKind::Pub));
    }

    #[test]
    fn lexes_panic_keyword() {
        let tokens = lex(Path::new("main.nomo"), "panic(\"boom\")\n").unwrap();

        assert!(tokens.iter().any(|token| token.kind == TokenKind::Panic));
    }

    #[test]
    fn lexes_float_literal_and_as_keyword() {
        let tokens = lex(
            Path::new("main.nomo"),
            "let ratio: f64 = 18 as f64\nlet pi: f64 = 3.14\n",
        )
        .unwrap();

        assert!(tokens.iter().any(|token| token.kind == TokenKind::As));
        assert!(
            tokens
                .iter()
                .any(|token| token.kind == TokenKind::Float("3.14".to_string()))
        );
    }

    #[test]
    fn lexes_char_literals() {
        let tokens = lex(
            Path::new("main.nomo"),
            "let a: char = 'A'\nlet nl = '\\n'\n",
        )
        .unwrap();

        assert!(
            tokens
                .iter()
                .any(|token| token.kind == TokenKind::Char('A'))
        );
        assert!(
            tokens
                .iter()
                .any(|token| token.kind == TokenKind::Char('\n'))
        );
    }

    #[test]
    fn skips_line_and_doc_comments() {
        let tokens = lex(
            Path::new("main.nomo"),
            "/// module docs\n//! crate docs\npackage app.main // trailing\nlet url = \"http://example.test/*literal*/\"\n",
        )
        .unwrap();

        assert!(tokens.iter().any(|token| token.kind == TokenKind::Package));
        assert!(tokens.iter().any(|token| {
            token.kind == TokenKind::String("http://example.test/*literal*/".to_string())
        }));
        assert!(tokens.iter().all(|token| {
            !matches!(
                &token.kind,
                TokenKind::Ident(value)
                    if value == "module" || value == "docs" || value == "trailing"
            )
        }));
    }

    #[test]
    fn skips_nested_block_and_doc_comments() {
        let tokens = lex(
            Path::new("main.nomo"),
            "/*! module docs */\npackage app.main\n/* outer\n   /* inner */\n   outer */\nfn main() -> void {\n    /** statement docs */ return\n}\n",
        )
        .unwrap();

        assert!(tokens.iter().any(|token| token.kind == TokenKind::Package));
        assert!(tokens.iter().any(|token| token.kind == TokenKind::Fn));
        assert!(tokens.iter().any(|token| token.kind == TokenKind::Return));
    }

    #[test]
    fn rejects_unterminated_block_comment() {
        let err = lex(Path::new("main.nomo"), "package app.main\n/* open\n").unwrap_err();

        assert_eq!(err.code, "E0108");
        assert_eq!(err.message, "unterminated block comment");
        assert_eq!(err.line, 2);
        assert_eq!(err.column, 1);
    }

    #[test]
    fn lexer_token_sequence_golden() {
        let source = "package app.main\n\nimport std.array.Array\n\nconst LIMIT: i32 = 3\n\nfn main() -> void {\n    let mut items = Array.new<i32>()\n    items.push(mut items, LIMIT)\n    for item in items {\n        if item >= 1 {\n            break\n        } else {\n            continue\n        }\n    }\n}\n";
        let tokens = lex(Path::new("main.nomo"), source).unwrap();
        let snapshot = tokens
            .iter()
            .map(|token| format!("{:?}", token.kind))
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(
            snapshot,
            r#"Package
Ident("app")
Dot
Ident("main")
Newline
Newline
Import
Ident("std")
Dot
Ident("array")
Dot
Ident("Array")
Newline
Newline
Const
Ident("LIMIT")
Colon
Ident("i32")
Equal
Int(3)
Newline
Newline
Fn
Ident("main")
LParen
RParen
Arrow
Void
LBrace
Newline
Let
Mut
Ident("items")
Equal
Ident("Array")
Dot
Ident("new")
Less
Ident("i32")
Greater
LParen
RParen
Newline
Ident("items")
Dot
Ident("push")
LParen
Mut
Ident("items")
Comma
Ident("LIMIT")
RParen
Newline
For
Ident("item")
In
Ident("items")
LBrace
Newline
If
Ident("item")
GreaterEqual
Int(1)
LBrace
Newline
Break
Newline
RBrace
Else
LBrace
Newline
Continue
Newline
RBrace
Newline
RBrace
Newline
RBrace
Newline
Eof"#
        );
    }

    #[test]
    fn rejects_reserved_future_words() {
        for word in ["interface", "unsafe", "extern", "export", "go", "chan"] {
            let source = format!("let {word}: i32 = 1\n");
            let err = lex(Path::new("main.nomo"), &source).unwrap_err();
            assert_eq!(err.code, "E0104");
            assert!(err.message.contains(word));
        }
    }

    #[test]
    fn rejects_null_special_name() {
        let err = lex(Path::new("main.nomo"), "let value = null\n").unwrap_err();

        assert_eq!(err.code, "E0104");
        assert!(err.message.contains("null"));
    }
}

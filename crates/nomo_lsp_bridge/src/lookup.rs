use nomo_diagnostics::Diagnostic;
use nomo_syntax::lexer::{Token, TokenKind, lex};
use std::path::Path;

use super::range::{TextPosition, TextRange, range_contains, source_line_range, token_range};
use super::{SemanticSymbol, SemanticSymbolKind};

pub(super) fn resolve_symbol(
    path: &Path,
    position: TextPosition,
    name: &str,
    symbols: Vec<SemanticSymbol>,
    preference: &[SemanticSymbolKind],
) -> Option<SemanticSymbol> {
    let mut matches = symbols
        .into_iter()
        .filter(|symbol| symbol.name == name)
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| {
        left.source_path
            .cmp(&right.source_path)
            .then(left.line.cmp(&right.line))
            .then(
                left.selection_range
                    .start
                    .line
                    .cmp(&right.selection_range.start.line),
            )
            .then(
                left.selection_range
                    .start
                    .character
                    .cmp(&right.selection_range.start.character),
            )
    });
    let fallback = matches
        .iter()
        .find(|symbol| {
            symbol.source_path == path && range_contains(symbol.selection_range, position)
        })
        .cloned()
        .or_else(|| {
            matches
                .iter()
                .find(|symbol| symbol.source_path == path)
                .cloned()
        })
        .or_else(|| matches.first().cloned());
    prefer_symbol_kind(fallback, &matches, preference)
}

pub(super) fn symbol_lookup_preference(
    path: &Path,
    source: &str,
    position: TextPosition,
) -> Result<Vec<SemanticSymbolKind>, Diagnostic> {
    let tokens = lex(path, source)?;
    let Some(index) = ident_token_at_position(&tokens, position) else {
        return Ok(Vec::new());
    };
    let TokenKind::Ident(name) = &tokens[index].kind else {
        return Ok(Vec::new());
    };
    let previous = previous_significant_token(&tokens, index);
    let next = next_significant_index(&tokens, index, tokens.len()).map(|index| &tokens[index]);
    let starts_upper = name
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase());

    if previous.is_some_and(|token| matches!(token.kind, TokenKind::Fn)) {
        return Ok(vec![
            SemanticSymbolKind::InterfaceMethod,
            SemanticSymbolKind::Method,
            SemanticSymbolKind::Function,
        ]);
    }
    if previous.is_some_and(|token| matches!(token.kind, TokenKind::Interface)) {
        return Ok(vec![SemanticSymbolKind::Interface]);
    }
    if previous.is_some_and(|token| matches!(token.kind, TokenKind::Dot)) && starts_upper {
        return Ok(vec![SemanticSymbolKind::Variant, SemanticSymbolKind::Field]);
    }
    if previous.is_some_and(|token| matches!(token.kind, TokenKind::Dot))
        && next.is_some_and(|token| matches!(token.kind, TokenKind::LParen))
    {
        return Ok(vec![
            SemanticSymbolKind::Method,
            SemanticSymbolKind::Function,
        ]);
    }
    if previous.is_some_and(|token| matches!(token.kind, TokenKind::Dot)) {
        return Ok(vec![SemanticSymbolKind::Field, SemanticSymbolKind::Method]);
    }
    if next.is_some_and(|token| matches!(token.kind, TokenKind::Colon)) {
        return Ok(vec![SemanticSymbolKind::Field]);
    }
    Ok(Vec::new())
}

fn prefer_symbol_kind(
    fallback: Option<SemanticSymbol>,
    matches: &[SemanticSymbol],
    preference: &[SemanticSymbolKind],
) -> Option<SemanticSymbol> {
    for kind in preference {
        if let Some(symbol) = matches.iter().find(|symbol| symbol.kind == *kind) {
            return Some(symbol.clone());
        }
    }
    fallback
}

fn ident_token_at_position(tokens: &[Token], position: TextPosition) -> Option<usize> {
    tokens.iter().position(|token| {
        matches!(token.kind, TokenKind::Ident(_))
            && range_contains(token_range_for_lookup(token), position)
    })
}

fn token_range_for_lookup(token: &Token) -> TextRange {
    match &token.kind {
        TokenKind::Ident(name) => token_range(token.line, token.column, name),
        _ => source_line_range(token.line, &token.text),
    }
}

fn previous_significant_token(tokens: &[Token], index: usize) -> Option<&Token> {
    (0..index)
        .rev()
        .map(|candidate| &tokens[candidate])
        .find(|token| !matches!(token.kind, TokenKind::Newline | TokenKind::Eof))
}

fn next_significant_index(tokens: &[Token], index: usize, end: usize) -> Option<usize> {
    (index + 1..end).find(|next| !matches!(tokens[*next].kind, TokenKind::Newline))
}

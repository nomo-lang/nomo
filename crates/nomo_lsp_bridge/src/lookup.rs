use nomo_diagnostics::Diagnostic;
use nomo_spans::{SourceFile, SourceMap};
use nomo_syntax::lexer::{Token, TokenKind, lex};
use std::path::Path;

use super::range::{
    TextPosition, TextRange, range_contains, source_line_range, token_range_in_file,
};
use super::{SemanticSymbol, SemanticSymbolKind};

pub(super) fn resolve_symbol(
    path: &Path,
    position: TextPosition,
    name: &str,
    symbols: &[SemanticSymbol],
    preference: &[SemanticSymbolKind],
) -> Option<SemanticSymbol> {
    let mut matches = symbols
        .iter()
        .filter(|symbol| symbol.name == name)
        .cloned()
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
    if let Some(exact) = matches.iter().find(|symbol| {
        symbol.source_path == path && range_contains(symbol.selection_range, position)
    }) {
        return Some(exact.clone());
    }

    let local_matches = matches
        .iter()
        .filter(|symbol| symbol.source_path == path)
        .cloned()
        .collect::<Vec<_>>();
    let candidates = if local_matches.is_empty() {
        matches.as_slice()
    } else {
        local_matches.as_slice()
    };
    prefer_symbol_kind(candidates.first().cloned(), candidates, preference)
}

pub(super) fn symbol_lookup_preference(
    path: &Path,
    source: &str,
    position: TextPosition,
) -> Result<Vec<SemanticSymbolKind>, Diagnostic> {
    let tokens = lex(path, source)?;
    let mut source_map = SourceMap::new();
    let file_id = source_map.add_file(path, source);
    let source_file = source_map
        .file(file_id)
        .expect("source file was just added to the source map");
    let Some(index) = ident_token_at_position(&tokens, source_file, position) else {
        return Ok(Vec::new());
    };
    let TokenKind::Ident(name) = &tokens[index].kind else {
        return Ok(Vec::new());
    };
    Ok(symbol_lookup_preference_for_token(&tokens, index, name))
}

pub(super) fn symbol_lookup_preference_for_token(
    tokens: &[Token],
    index: usize,
    name: &str,
) -> Vec<SemanticSymbolKind> {
    let previous = previous_significant_token(tokens, index);
    let next = next_significant_index(tokens, index, tokens.len()).map(|index| &tokens[index]);
    let starts_upper = name
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase());

    if previous.is_some_and(|token| matches!(token.kind, TokenKind::Fn)) {
        return vec![
            SemanticSymbolKind::InterfaceMethod,
            SemanticSymbolKind::Method,
            SemanticSymbolKind::Function,
        ];
    }
    if previous.is_some_and(|token| matches!(token.kind, TokenKind::Interface)) {
        return vec![SemanticSymbolKind::Interface];
    }
    if previous.is_some_and(|token| matches!(token.kind, TokenKind::Dot)) && starts_upper {
        return vec![SemanticSymbolKind::Variant, SemanticSymbolKind::Field];
    }
    if previous.is_some_and(|token| matches!(token.kind, TokenKind::Dot))
        && next.is_some_and(|token| matches!(token.kind, TokenKind::LParen))
    {
        return vec![SemanticSymbolKind::Method, SemanticSymbolKind::Function];
    }
    if previous.is_some_and(|token| matches!(token.kind, TokenKind::Dot)) {
        return vec![SemanticSymbolKind::Field, SemanticSymbolKind::Method];
    }
    if next.is_some_and(|token| matches!(token.kind, TokenKind::Colon)) {
        return vec![SemanticSymbolKind::Field];
    }
    Vec::new()
}

fn prefer_symbol_kind(
    fallback: Option<SemanticSymbol>,
    matches: &[SemanticSymbol],
    preference: &[SemanticSymbolKind],
) -> Option<SemanticSymbol> {
    for kind in preference {
        let mut preferred = matches.iter().filter(|symbol| symbol.kind == *kind);
        let symbol = preferred.next();
        if preferred.next().is_some() {
            return None;
        }
        if let Some(symbol) = symbol {
            return Some(symbol.clone());
        }
    }
    (matches.len() == 1).then_some(fallback).flatten()
}

fn ident_token_at_position(
    tokens: &[Token],
    source_file: &SourceFile,
    position: TextPosition,
) -> Option<usize> {
    tokens.iter().position(|token| {
        matches!(token.kind, TokenKind::Ident(_))
            && range_contains(token_range_for_lookup(source_file, token), position)
    })
}

fn token_range_for_lookup(source_file: &SourceFile, token: &Token) -> TextRange {
    match &token.kind {
        TokenKind::Ident(name) => token_range_in_file(source_file, token.line, token.column, name),
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

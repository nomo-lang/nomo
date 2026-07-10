#![allow(clippy::result_large_err)]

use nomo_diagnostics::Diagnostic;
use nomo_syntax::lexer::{TokenKind, lex};
use nomo_syntax::parser::parse;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

mod docs;
mod lookup;
mod range;
mod signature;
mod symbols;

use docs::extract_doc_comments;
use lookup::{resolve_symbol, symbol_lookup_preference};
pub use range::{TextPosition, TextRange, identifier_at_position, token_range};
use symbols::symbols_from_ast;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticLocation {
    pub path: PathBuf,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticSymbolKind {
    Struct,
    Enum,
    Field,
    Variant,
    Interface,
    InterfaceMethod,
    Const,
    Function,
    ExternFunction,
    Method,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticSymbol {
    pub source_path: PathBuf,
    pub name: String,
    pub kind: SemanticSymbolKind,
    pub signature: String,
    pub docs: String,
    pub line: usize,
    pub range: TextRange,
    pub selection_range: TextRange,
}

pub fn symbols_for_text(path: &Path, source: &str) -> Result<Vec<SemanticSymbol>, Diagnostic> {
    let tokens = lex(path, source)?;
    let ast = parse(path, &tokens)?;
    let docs = extract_doc_comments(source);
    Ok(symbols_from_ast(path, &ast, &docs))
}

pub fn public_symbols_for_text(
    path: &Path,
    source: &str,
) -> Result<Vec<SemanticSymbol>, Diagnostic> {
    let tokens = lex(path, source)?;
    let ast = parse(path, &tokens)?;
    let docs = extract_doc_comments(source);
    let public_structs = ast
        .structs
        .iter()
        .filter(|item| item.public)
        .map(|item| item.name.as_str())
        .collect::<BTreeSet<_>>();
    let public_enums = ast
        .enums
        .iter()
        .filter(|item| item.public)
        .map(|item| item.name.as_str())
        .collect::<BTreeSet<_>>();
    let public_interfaces = ast
        .interfaces
        .iter()
        .filter(|item| item.public)
        .map(|item| item.name.as_str())
        .collect::<BTreeSet<_>>();

    Ok(symbols_from_ast(path, &ast, &docs)
        .into_iter()
        .filter(|symbol| {
            public_dependency_symbol(symbol, &public_structs, &public_enums, &public_interfaces)
        })
        .collect())
}

pub fn symbol_at_position(
    path: &Path,
    source: &str,
    position: TextPosition,
) -> Result<Option<SemanticSymbol>, Diagnostic> {
    let Some(name) = identifier_at_position(source, position) else {
        return Ok(None);
    };
    let preference = symbol_lookup_preference(path, source, position)?;
    Ok(resolve_symbol(
        path,
        position,
        &name,
        symbols_for_text(path, source)?,
        &preference,
    ))
}

pub fn resolve_symbol_at_position(
    path: &Path,
    source: &str,
    position: TextPosition,
    symbols: Vec<SemanticSymbol>,
) -> Result<Option<SemanticSymbol>, Diagnostic> {
    let Some(name) = identifier_at_position(source, position) else {
        return Ok(None);
    };
    let preference = symbol_lookup_preference(path, source, position)?;
    Ok(resolve_symbol(path, position, &name, symbols, &preference))
}

pub fn definition_for_text(
    path: &Path,
    source: &str,
    position: TextPosition,
) -> Result<Option<TextRange>, Diagnostic> {
    Ok(symbol_at_position(path, source, position)?.map(|symbol| symbol.selection_range))
}

pub fn references_for_text(
    path: &Path,
    source: &str,
    position: TextPosition,
    include_declaration: bool,
) -> Result<Option<Vec<TextRange>>, Diagnostic> {
    let Some(symbol) = symbol_at_position(path, source, position)? else {
        return Ok(None);
    };
    let tokens = lex(path, source)?;
    Ok(Some(
        tokens
            .iter()
            .filter_map(|token| {
                let TokenKind::Ident(name) = &token.kind else {
                    return None;
                };
                if name != &symbol.name {
                    return None;
                }
                let range = token_range(token.line, token.column, name);
                if !include_declaration && range == symbol.selection_range {
                    return None;
                }
                Some(range)
            })
            .collect(),
    ))
}

fn public_dependency_symbol(
    symbol: &SemanticSymbol,
    public_structs: &BTreeSet<&str>,
    public_enums: &BTreeSet<&str>,
    public_interfaces: &BTreeSet<&str>,
) -> bool {
    match symbol.kind {
        SemanticSymbolKind::Struct
        | SemanticSymbolKind::Enum
        | SemanticSymbolKind::Interface
        | SemanticSymbolKind::Const
        | SemanticSymbolKind::Function => symbol.signature.starts_with("pub "),
        SemanticSymbolKind::ExternFunction => true,
        SemanticSymbolKind::Method => symbol.signature.starts_with("pub "),
        SemanticSymbolKind::Field => symbol
            .signature
            .strip_prefix("pub field ")
            .and_then(|rest| rest.split_once(':'))
            .and_then(|(path, _)| path.rsplit_once('.'))
            .is_some_and(|(owner, _)| public_structs.contains(owner)),
        SemanticSymbolKind::Variant => symbol
            .signature
            .strip_prefix("variant ")
            .and_then(|rest| rest.split('(').next())
            .and_then(|path| path.rsplit_once('.'))
            .is_some_and(|(owner, _)| public_enums.contains(owner)),
        SemanticSymbolKind::InterfaceMethod => symbol
            .signature
            .strip_prefix("fn ")
            .and_then(|rest| rest.split('(').next())
            .and_then(|path| path.rsplit_once('.'))
            .is_some_and(|(owner, _)| public_interfaces.contains(owner)),
    }
}

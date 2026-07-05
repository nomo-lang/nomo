use crate::ast::{
    ConstDef, EnumDef, EnumVariant, ExternBlock, Field, Function, FunctionSignature, ImplBlock,
    InterfaceDef, Param, SourceFile, Span, StructDef, TypeRef,
};
use crate::diagnostic::Diagnostic;
use crate::lexer::{TokenKind, lex};
use crate::parser::parse;
use crate::project::{Project, project_module_context, resolve_module_source_path};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextPosition {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextRange {
    pub start: TextPosition,
    pub end: TextPosition,
}

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

pub fn symbols_for_project_with_overrides(
    project: &Project,
    source_overrides: &[(PathBuf, String)],
) -> Result<Vec<SemanticSymbol>, Diagnostic> {
    let mut symbols = Vec::new();
    for (path, source) in project_sources(project, source_overrides)? {
        symbols.extend(symbols_for_text(&path, &source)?);
    }
    Ok(symbols)
}

pub fn dependency_symbols_for_project_with_overrides(
    project: &Project,
    source_overrides: &[(PathBuf, String)],
) -> Result<Vec<SemanticSymbol>, Diagnostic> {
    let Ok(context) = project_module_context(project) else {
        return Ok(Vec::new());
    };
    let mut files = Vec::new();
    for module in &context.external_modules {
        collect_nomo_files(&module.source_root, &mut files).map_err(|message| {
            Diagnostic::new("E0902", message, &module.source_root, 1, 1, 1, "")
        })?;
    }
    for (path, _) in source_overrides {
        if context
            .external_modules
            .iter()
            .any(|module| is_project_nomo_source(&module.source_root, path))
            && !files.iter().any(|file| file == path)
        {
            files.push(path.clone());
        }
    }
    files.sort();
    files.dedup();

    let overrides = source_overrides
        .iter()
        .map(|(path, source)| (path.clone(), source.clone()))
        .collect::<BTreeMap<_, _>>();

    let mut symbols = Vec::new();
    for path in files {
        let source = match overrides.get(&path) {
            Some(source) => source.clone(),
            None => fs::read_to_string(&path).map_err(|err| {
                Diagnostic::new(
                    "E0902",
                    format!("failed to read {}: {err}", path.display()),
                    &path,
                    1,
                    1,
                    1,
                    "",
                )
            })?,
        };
        symbols.extend(public_symbols_for_text(&path, &source)?);
    }
    Ok(symbols)
}

pub fn identifier_at_position(source: &str, position: TextPosition) -> Option<String> {
    let line = source.lines().nth(position.line as usize)?;
    let byte_index = utf16_character_to_byte_index(line, position.character);
    let bytes = line.as_bytes();
    if byte_index > bytes.len() {
        return None;
    }

    let mut start = byte_index;
    if start == bytes.len() && start > 0 {
        start -= 1;
    }
    if !is_ident_byte(bytes.get(start).copied()?) && start > 0 {
        start -= 1;
    }
    if !is_ident_byte(bytes.get(start).copied()?) {
        return None;
    }

    let mut end = start;
    while start > 0 && is_ident_byte(bytes[start - 1]) {
        start -= 1;
    }
    while end + 1 < bytes.len() && is_ident_byte(bytes[end + 1]) {
        end += 1;
    }
    Some(line[start..=end].to_string())
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

pub fn symbol_at_project_position(
    project: &Project,
    path: &Path,
    source: &str,
    position: TextPosition,
    source_overrides: &[(PathBuf, String)],
) -> Result<Option<SemanticSymbol>, Diagnostic> {
    let Some(name) = identifier_at_position(source, position) else {
        return Ok(None);
    };
    let overrides = overrides_with_current(path, source, source_overrides);
    let symbols = accessible_symbols_for_document(project, source, &overrides)?;
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

pub fn definition_for_project_text(
    project: &Project,
    path: &Path,
    source: &str,
    position: TextPosition,
    source_overrides: &[(PathBuf, String)],
) -> Result<Option<SemanticLocation>, Diagnostic> {
    Ok(
        symbol_at_project_position(project, path, source, position, source_overrides)?.map(
            |symbol| SemanticLocation {
                path: symbol.source_path,
                range: symbol.selection_range,
            },
        ),
    )
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

pub fn references_for_project_text(
    project: &Project,
    path: &Path,
    source: &str,
    position: TextPosition,
    include_declaration: bool,
    source_overrides: &[(PathBuf, String)],
) -> Result<Option<Vec<SemanticLocation>>, Diagnostic> {
    let Some(symbol) =
        symbol_at_project_position(project, path, source, position, source_overrides)?
    else {
        return Ok(None);
    };
    let local_source_root = project.root.join("src");
    if !is_project_nomo_source(&local_source_root, &symbol.source_path) {
        return Ok(None);
    }
    let overrides = overrides_with_current(path, source, source_overrides);
    let mut locations = Vec::new();
    for (source_path, source) in project_sources(project, &overrides)? {
        let tokens = lex(&source_path, &source)?;
        for token in &tokens {
            let TokenKind::Ident(name) = &token.kind else {
                continue;
            };
            if name != &symbol.name {
                continue;
            }
            let range = token_range(token.line, token.column, name);
            if !include_declaration
                && source_path == symbol.source_path
                && range == symbol.selection_range
            {
                continue;
            }
            locations.push(SemanticLocation {
                path: source_path.clone(),
                range,
            });
        }
    }
    Ok(Some(locations))
}

fn accessible_symbols_for_document(
    project: &Project,
    source: &str,
    source_overrides: &[(PathBuf, String)],
) -> Result<Vec<SemanticSymbol>, Diagnostic> {
    let mut symbols = symbols_for_project_with_overrides(project, source_overrides)?;
    symbols.extend(dependency_symbols_for_document(
        project,
        source,
        source_overrides,
    )?);
    Ok(symbols)
}

fn dependency_symbols_for_document(
    project: &Project,
    source: &str,
    source_overrides: &[(PathBuf, String)],
) -> Result<Vec<SemanticSymbol>, Diagnostic> {
    let Ok(context) = project_module_context(project) else {
        return Ok(Vec::new());
    };
    let Some(local_root) = package_root(source) else {
        return Ok(Vec::new());
    };
    let external_roots = context
        .external_modules
        .iter()
        .map(|module| module.source_root.as_path())
        .collect::<Vec<_>>();
    let mut files = source
        .lines()
        .filter_map(import_path)
        .filter(|import| {
            import
                .first()
                .is_some_and(|root| root != "std" && root != &local_root)
        })
        .filter_map(|import| resolve_module_source_path(&context, &local_root, &import))
        .filter(|path| {
            external_roots
                .iter()
                .any(|source_root| path.starts_with(source_root))
        })
        .collect::<Vec<_>>();
    files.sort();
    files.dedup();

    let overrides = source_overrides
        .iter()
        .map(|(path, source)| (path.clone(), source.clone()))
        .collect::<BTreeMap<_, _>>();

    let mut symbols = Vec::new();
    for path in files {
        let source = match overrides.get(&path) {
            Some(source) => source.clone(),
            None => fs::read_to_string(&path).map_err(|err| {
                Diagnostic::new(
                    "E0902",
                    format!("failed to read {}: {err}", path.display()),
                    &path,
                    1,
                    1,
                    1,
                    "",
                )
            })?,
        };
        symbols.extend(public_symbols_for_text(&path, &source)?);
    }
    Ok(symbols)
}

fn package_root(source: &str) -> Option<String> {
    source.lines().find_map(|line| {
        let package = line.trim().strip_prefix("package ")?;
        package
            .split('.')
            .next()
            .filter(|segment| !segment.is_empty())
            .map(str::to_string)
    })
}

fn import_path(line: &str) -> Option<Vec<String>> {
    let import = line.trim().strip_prefix("import ")?;
    let path = import.split_whitespace().next()?;
    let parts = path
        .split('.')
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    (parts.len() >= 2).then_some(parts)
}

fn public_symbols_for_text(path: &Path, source: &str) -> Result<Vec<SemanticSymbol>, Diagnostic> {
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

fn resolve_symbol(
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

fn symbol_lookup_preference(
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

fn ident_token_at_position(
    tokens: &[crate::lexer::Token],
    position: TextPosition,
) -> Option<usize> {
    tokens.iter().position(|token| {
        matches!(token.kind, TokenKind::Ident(_))
            && range_contains(token_range_for_lookup(token), position)
    })
}

fn token_range_for_lookup(token: &crate::lexer::Token) -> TextRange {
    match &token.kind {
        TokenKind::Ident(name) => token_range(token.line, token.column, name),
        _ => source_line_range(token.line, &token.text),
    }
}

fn previous_significant_token(
    tokens: &[crate::lexer::Token],
    index: usize,
) -> Option<&crate::lexer::Token> {
    (0..index)
        .rev()
        .map(|candidate| &tokens[candidate])
        .find(|token| !matches!(token.kind, TokenKind::Newline | TokenKind::Eof))
}

fn next_significant_index(
    tokens: &[crate::lexer::Token],
    index: usize,
    end: usize,
) -> Option<usize> {
    (index + 1..end).find(|next| !matches!(tokens[*next].kind, TokenKind::Newline))
}

fn range_contains(range: TextRange, position: TextPosition) -> bool {
    if position.line < range.start.line || position.line > range.end.line {
        return false;
    }
    if position.line == range.start.line && position.character < range.start.character {
        return false;
    }
    if position.line == range.end.line && position.character > range.end.character {
        return false;
    }
    true
}

fn overrides_with_current(
    path: &Path,
    source: &str,
    source_overrides: &[(PathBuf, String)],
) -> Vec<(PathBuf, String)> {
    let mut overrides = source_overrides.to_vec();
    if let Some(existing) = overrides
        .iter_mut()
        .find(|(entry_path, _)| entry_path == path)
    {
        existing.1 = source.to_string();
    } else {
        overrides.push((path.to_path_buf(), source.to_string()));
    }
    overrides
}

fn project_sources(
    project: &Project,
    source_overrides: &[(PathBuf, String)],
) -> Result<Vec<(PathBuf, String)>, Diagnostic> {
    let src = project.root.join("src");
    let mut files = Vec::new();
    collect_nomo_files(&src, &mut files)
        .map_err(|message| Diagnostic::new("E0902", message, &src, 1, 1, 1, ""))?;
    for (path, _) in source_overrides {
        if is_project_nomo_source(&src, path) && !files.iter().any(|file| file == path) {
            files.push(path.clone());
        }
    }
    files.sort();
    files.dedup();

    let overrides = source_overrides
        .iter()
        .map(|(path, source)| (path.clone(), source.clone()))
        .collect::<BTreeMap<_, _>>();

    files
        .into_iter()
        .map(|path| {
            if let Some(source) = overrides.get(&path) {
                return Ok((path, source.clone()));
            }
            let source = fs::read_to_string(&path).map_err(|err| {
                Diagnostic::new(
                    "E0902",
                    format!("failed to read {}: {err}", path.display()),
                    &path,
                    1,
                    1,
                    1,
                    "",
                )
            })?;
            Ok((path, source))
        })
        .collect()
}

fn is_project_nomo_source(source_root: &Path, path: &Path) -> bool {
    path.starts_with(source_root) && path.extension().and_then(|ext| ext.to_str()) == Some("nomo")
}

fn collect_nomo_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in
        fs::read_dir(dir).map_err(|err| format!("failed to read {}: {err}", dir.display()))?
    {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_nomo_files(&path, files)?;
        } else if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("nomo") {
            files.push(path);
        }
    }
    Ok(())
}

fn symbols_from_ast(path: &Path, ast: &SourceFile, docs: &DocComments) -> Vec<SemanticSymbol> {
    let mut symbols = Vec::new();
    for item in &ast.structs {
        symbols.push(SemanticSymbol {
            source_path: path.to_path_buf(),
            name: item.name.clone(),
            kind: SemanticSymbolKind::Struct,
            signature: struct_signature(item),
            docs: docs
                .item_docs
                .get(&item.span.line)
                .cloned()
                .unwrap_or_default(),
            line: item.span.line,
            range: line_range(&item.span),
            selection_range: name_selection_range(&item.span, &item.name),
        });
        symbols.extend(field_symbols(path, item, docs));
    }
    for item in &ast.enums {
        symbols.push(SemanticSymbol {
            source_path: path.to_path_buf(),
            name: item.name.clone(),
            kind: SemanticSymbolKind::Enum,
            signature: enum_signature(item),
            docs: docs
                .item_docs
                .get(&item.span.line)
                .cloned()
                .unwrap_or_default(),
            line: item.span.line,
            range: line_range(&item.span),
            selection_range: name_selection_range(&item.span, &item.name),
        });
        symbols.extend(variant_symbols(path, item, docs));
    }
    for item in &ast.interfaces {
        symbols.push(SemanticSymbol {
            source_path: path.to_path_buf(),
            name: item.name.clone(),
            kind: SemanticSymbolKind::Interface,
            signature: interface_signature(item),
            docs: docs
                .item_docs
                .get(&item.span.line)
                .cloned()
                .unwrap_or_default(),
            line: item.span.line,
            range: line_range(&item.span),
            selection_range: name_selection_range(&item.span, &item.name),
        });
        symbols.extend(interface_method_symbols(path, item, docs));
    }
    for item in &ast.consts {
        symbols.push(SemanticSymbol {
            source_path: path.to_path_buf(),
            name: item.name.clone(),
            kind: SemanticSymbolKind::Const,
            signature: const_signature(item),
            docs: docs
                .item_docs
                .get(&item.span.line)
                .cloned()
                .unwrap_or_default(),
            line: item.span.line,
            range: line_range(&item.span),
            selection_range: name_selection_range(&item.span, &item.name),
        });
    }
    for item in &ast.functions {
        symbols.push(SemanticSymbol {
            source_path: path.to_path_buf(),
            name: item.name.clone(),
            kind: SemanticSymbolKind::Function,
            signature: function_signature(item),
            docs: docs
                .item_docs
                .get(&item.span.line)
                .cloned()
                .unwrap_or_default(),
            line: item.span.line,
            range: line_range(&item.span),
            selection_range: name_selection_range(&item.span, &item.name),
        });
    }
    for item in &ast.extern_blocks {
        symbols.extend(extern_function_symbols(path, item, docs));
    }
    for impl_block in &ast.impls {
        symbols.extend(method_symbols(path, impl_block, docs));
    }
    symbols
}

fn field_symbols(path: &Path, item: &StructDef, docs: &DocComments) -> Vec<SemanticSymbol> {
    item.fields
        .iter()
        .map(|field| SemanticSymbol {
            source_path: path.to_path_buf(),
            name: field.name.clone(),
            kind: SemanticSymbolKind::Field,
            signature: field_signature(&item.name, field),
            docs: docs
                .item_docs
                .get(&field.span.line)
                .cloned()
                .unwrap_or_default(),
            line: field.span.line,
            range: line_range(&field.span),
            selection_range: name_selection_range(&field.span, &field.name),
        })
        .collect()
}

fn variant_symbols(path: &Path, item: &EnumDef, docs: &DocComments) -> Vec<SemanticSymbol> {
    item.variants
        .iter()
        .map(|variant| SemanticSymbol {
            source_path: path.to_path_buf(),
            name: variant.name.clone(),
            kind: SemanticSymbolKind::Variant,
            signature: variant_signature(&item.name, variant),
            docs: docs
                .item_docs
                .get(&variant.span.line)
                .cloned()
                .unwrap_or_default(),
            line: variant.span.line,
            range: line_range(&variant.span),
            selection_range: name_selection_range(&variant.span, &variant.name),
        })
        .collect()
}

fn interface_method_symbols(
    path: &Path,
    item: &InterfaceDef,
    docs: &DocComments,
) -> Vec<SemanticSymbol> {
    item.methods
        .iter()
        .map(|method| SemanticSymbol {
            source_path: path.to_path_buf(),
            name: method.name.clone(),
            kind: SemanticSymbolKind::InterfaceMethod,
            signature: interface_method_signature(&item.name, method),
            docs: docs
                .item_docs
                .get(&method.span.line)
                .cloned()
                .unwrap_or_default(),
            line: method.span.line,
            range: line_range(&method.span),
            selection_range: name_selection_range(&method.span, &method.name),
        })
        .collect()
}

fn method_symbols(path: &Path, impl_block: &ImplBlock, docs: &DocComments) -> Vec<SemanticSymbol> {
    let receiver = type_ref(&impl_block.type_name);
    impl_block
        .methods
        .iter()
        .map(|method| SemanticSymbol {
            source_path: path.to_path_buf(),
            name: method.name.clone(),
            kind: SemanticSymbolKind::Method,
            signature: method_signature(&receiver, method),
            docs: docs
                .item_docs
                .get(&method.span.line)
                .cloned()
                .unwrap_or_default(),
            line: method.span.line,
            range: line_range(&method.span),
            selection_range: name_selection_range(&method.span, &method.name),
        })
        .collect()
}

fn extern_function_symbols(
    path: &Path,
    block: &ExternBlock,
    docs: &DocComments,
) -> Vec<SemanticSymbol> {
    block
        .functions
        .iter()
        .map(|function| SemanticSymbol {
            source_path: path.to_path_buf(),
            name: function.name.clone(),
            kind: SemanticSymbolKind::ExternFunction,
            signature: extern_function_signature(&block.abi, function),
            docs: docs
                .item_docs
                .get(&function.span.line)
                .cloned()
                .unwrap_or_default(),
            line: function.span.line,
            range: line_range(&function.span),
            selection_range: name_selection_range(&function.span, &function.name),
        })
        .collect()
}

fn line_range(span: &Span) -> TextRange {
    let line = span.line.saturating_sub(1) as u32;
    TextRange {
        start: TextPosition { line, character: 0 },
        end: TextPosition {
            line,
            character: span.text.chars().map(|ch| ch.len_utf16() as u32).sum(),
        },
    }
}

fn source_line_range(line: usize, text: &str) -> TextRange {
    let line = line.saturating_sub(1) as u32;
    TextRange {
        start: TextPosition { line, character: 0 },
        end: TextPosition {
            line,
            character: text.chars().map(|ch| ch.len_utf16() as u32).sum(),
        },
    }
}

fn name_selection_range(span: &Span, name: &str) -> TextRange {
    let line = span.line.saturating_sub(1) as u32;
    let fallback_start = span.column.saturating_sub(1) as u32;
    let start = span
        .text
        .find(name)
        .map(|byte_index| span.text[..byte_index].encode_utf16().count() as u32)
        .unwrap_or(fallback_start);
    let end = start + name.encode_utf16().count() as u32;
    TextRange {
        start: TextPosition {
            line,
            character: start,
        },
        end: TextPosition {
            line,
            character: end,
        },
    }
}

fn token_range(line: usize, column: usize, text: &str) -> TextRange {
    let line = line.saturating_sub(1) as u32;
    let start = column.saturating_sub(1) as u32;
    let end = start + text.encode_utf16().count() as u32;
    TextRange {
        start: TextPosition {
            line,
            character: start,
        },
        end: TextPosition {
            line,
            character: end,
        },
    }
}

fn utf16_character_to_byte_index(line: &str, character: u32) -> usize {
    let mut utf16_count = 0u32;
    for (byte_index, ch) in line.char_indices() {
        if utf16_count >= character {
            return byte_index;
        }
        utf16_count += ch.len_utf16() as u32;
    }
    line.len()
}

fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn struct_signature(item: &StructDef) -> String {
    format!(
        "{}struct {}{}",
        visibility_prefix(item.public),
        item.name,
        type_params(&item.type_params)
    )
}

fn enum_signature(item: &EnumDef) -> String {
    format!(
        "{}enum {}{}",
        visibility_prefix(item.public),
        item.name,
        type_params(&item.type_params)
    )
}

fn interface_signature(item: &InterfaceDef) -> String {
    format!("{}interface {}", visibility_prefix(item.public), item.name)
}

fn const_signature(item: &ConstDef) -> String {
    format!(
        "{}const {}: {}",
        visibility_prefix(item.public),
        item.name,
        type_ref(&item.type_ref)
    )
}

fn function_signature(function: &Function) -> String {
    let params = function
        .params
        .iter()
        .map(param)
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{}fn {}{}({}) -> {}",
        visibility_prefix(function.public),
        function.name,
        type_params(&function.type_params),
        params,
        type_ref(&function.return_type)
    )
}

fn method_signature(receiver: &str, function: &Function) -> String {
    let params = function
        .params
        .iter()
        .map(param)
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{}fn {receiver}.{}{}({}) -> {}",
        visibility_prefix(function.public),
        function.name,
        type_params(&function.type_params),
        params,
        type_ref(&function.return_type)
    )
}

fn extern_function_signature(abi: &str, function: &FunctionSignature) -> String {
    let params = function
        .params
        .iter()
        .map(param)
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "extern \"{}\" fn {}{}({}) -> {}",
        abi,
        function.name,
        type_params(&function.type_params),
        params,
        type_ref(&function.return_type)
    )
}

fn interface_method_signature(owner: &str, method: &FunctionSignature) -> String {
    let params = method
        .params
        .iter()
        .map(param)
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "fn {owner}.{}{}({}) -> {}",
        method.name,
        type_params(&method.type_params),
        params,
        type_ref(&method.return_type)
    )
}

fn field_signature(owner: &str, field: &Field) -> String {
    format!(
        "{}field {owner}.{}: {}",
        visibility_prefix(field.public),
        field.name,
        type_ref(&field.type_ref)
    )
}

fn variant_signature(owner: &str, variant: &EnumVariant) -> String {
    match &variant.payload {
        Some(payload) => format!("variant {owner}.{}({})", variant.name, type_ref(payload)),
        None => format!("variant {owner}.{}", variant.name),
    }
}

fn param(param: &Param) -> String {
    let mutable = if param.mutable { "mut " } else { "" };
    format!("{mutable}{}: {}", param.name, type_ref(&param.type_ref))
}

fn type_params(params: &[String]) -> String {
    if params.is_empty() {
        String::new()
    } else {
        format!("<{}>", params.join(", "))
    }
}

fn type_ref(type_ref_value: &TypeRef) -> String {
    let base = type_ref_value.path.join(".");
    if type_ref_value.args.is_empty() {
        base
    } else {
        format!(
            "{base}<{}>",
            type_ref_value
                .args
                .iter()
                .map(type_ref)
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

fn visibility_prefix(public: bool) -> &'static str {
    if public { "pub " } else { "" }
}

#[derive(Debug, Default)]
struct DocComments {
    item_docs: BTreeMap<usize, String>,
}

fn extract_doc_comments(source: &str) -> DocComments {
    let lines = source.lines().collect::<Vec<_>>();
    let mut comments = DocComments::default();
    let mut pending = Vec::new();
    let mut index = 0usize;
    while index < lines.len() {
        let line = lines[index];
        let trimmed = line.trim_start();
        if let Some(text) = trimmed.strip_prefix("///") {
            pending.push(text.trim_start().to_string());
            index += 1;
            continue;
        }
        if trimmed.starts_with("/**") {
            let (doc, next_index) = collect_block_doc(&lines, index);
            pending.push(doc);
            index = next_index;
            continue;
        }
        if !trimmed.is_empty() && !trimmed.starts_with("//") && !trimmed.starts_with("/*") {
            if !pending.is_empty() {
                comments.item_docs.insert(index + 1, pending.join("\n"));
                pending.clear();
            }
        }
        index += 1;
    }
    comments
}

fn collect_block_doc(lines: &[&str], start: usize) -> (String, usize) {
    let mut raw = String::new();
    let mut index = start;
    while index < lines.len() {
        if !raw.is_empty() {
            raw.push('\n');
        }
        raw.push_str(lines[index]);
        if lines[index].contains("*/") {
            index += 1;
            break;
        }
        index += 1;
    }
    let raw = raw.trim().trim_start_matches("/**").trim_end_matches("*/");
    let doc = raw
        .lines()
        .map(|line| line.trim().trim_start_matches('*').trim_start())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    (doc, index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn symbols_include_signatures_docs_and_ranges() {
        let source = "package app.main\n\n/// Adds numbers.\npub fn add(a: i64, b: i64) -> i64 {\n    return a + b\n}\n\nstruct User {\n    /// User email address.\n    pub email: string\n}\n\nenum Status {\n    /// Ready state.\n    Ready\n    /// Done state.\n    Done(i32)\n}\n\n/// Displayable values.\npub interface Display {\n    /// Converts to text.\n    fn to_string(self) -> string\n}\n\nextern \"C\" {\n    /// Writes a C string.\n    fn puts(message: string) -> i32\n}\n\nimpl User {\n    pub fn email(self) -> string {\n        return self.email\n    }\n}\n";

        let symbols = symbols_for_text(Path::new("main.nomo"), source).unwrap();

        assert_eq!(
            symbols
                .iter()
                .map(|symbol| symbol.name.as_str())
                .collect::<Vec<_>>(),
            vec![
                "User",
                "email",
                "Status",
                "Ready",
                "Done",
                "Display",
                "to_string",
                "add",
                "puts",
                "email"
            ]
        );
        assert_eq!(symbols[1].kind, SemanticSymbolKind::Field);
        assert_eq!(symbols[1].signature, "pub field User.email: string");
        assert_eq!(symbols[1].docs, "User email address.");
        assert_eq!(
            symbols[1].selection_range,
            TextRange {
                start: TextPosition {
                    line: 9,
                    character: 8,
                },
                end: TextPosition {
                    line: 9,
                    character: 13,
                },
            }
        );
        assert_eq!(symbols[3].kind, SemanticSymbolKind::Variant);
        assert_eq!(symbols[3].signature, "variant Status.Ready");
        assert_eq!(symbols[3].docs, "Ready state.");
        assert_eq!(symbols[4].signature, "variant Status.Done(i32)");
        assert_eq!(symbols[4].docs, "Done state.");
        assert_eq!(symbols[5].kind, SemanticSymbolKind::Interface);
        assert_eq!(symbols[5].signature, "pub interface Display");
        assert_eq!(symbols[5].docs, "Displayable values.");
        assert_eq!(symbols[6].kind, SemanticSymbolKind::InterfaceMethod);
        assert_eq!(
            symbols[6].signature,
            "fn Display.to_string(self: Self) -> string"
        );
        assert_eq!(symbols[6].docs, "Converts to text.");
        assert_eq!(
            symbols[6].selection_range,
            TextRange {
                start: TextPosition {
                    line: 22,
                    character: 7,
                },
                end: TextPosition {
                    line: 22,
                    character: 16,
                },
            }
        );
        assert_eq!(symbols[7].kind, SemanticSymbolKind::Function);
        assert_eq!(symbols[7].signature, "pub fn add(a: i64, b: i64) -> i64");
        assert_eq!(symbols[7].docs, "Adds numbers.");
        assert_eq!(
            symbols[7].selection_range,
            TextRange {
                start: TextPosition {
                    line: 3,
                    character: 7,
                },
                end: TextPosition {
                    line: 3,
                    character: 10,
                },
            }
        );
        assert_eq!(symbols[8].kind, SemanticSymbolKind::ExternFunction);
        assert_eq!(
            symbols[8].signature,
            "extern \"C\" fn puts(message: string) -> i32"
        );
        assert_eq!(symbols[8].docs, "Writes a C string.");
        assert_eq!(
            symbols[8].selection_range,
            TextRange {
                start: TextPosition {
                    line: 27,
                    character: 7,
                },
                end: TextPosition {
                    line: 27,
                    character: 11,
                },
            }
        );
        assert_eq!(
            symbols[9].signature,
            "pub fn User.email(self: User) -> string"
        );
    }

    #[test]
    fn definition_returns_declaration_range() {
        let source = "package app.main\n\nfn add(a: i64, b: i64) -> i64 {\n    return a + b\n}\n\nfn main() -> void {\n    let total: i64 = add(1, 2)\n}\n";

        let definition = definition_for_text(
            Path::new("main.nomo"),
            source,
            TextPosition {
                line: 7,
                character: 22,
            },
        )
        .unwrap()
        .unwrap();

        assert_eq!(
            definition,
            TextRange {
                start: TextPosition {
                    line: 2,
                    character: 3,
                },
                end: TextPosition {
                    line: 2,
                    character: 6,
                },
            }
        );
    }

    #[test]
    fn definition_returns_field_declaration_range() {
        let source = "package app.main\n\nstruct User {\n    email: string\n}\n\nfn main() -> void {\n    let user: User = User { email: \"hi\" }\n}\n";

        let definition = definition_for_text(
            Path::new("main.nomo"),
            source,
            TextPosition {
                line: 7,
                character: 30,
            },
        )
        .unwrap()
        .unwrap();

        assert_eq!(
            definition,
            TextRange {
                start: TextPosition {
                    line: 3,
                    character: 4,
                },
                end: TextPosition {
                    line: 3,
                    character: 9,
                },
            }
        );
    }

    #[test]
    fn definition_returns_enum_variant_declaration_range() {
        let source = "package app.main\n\nenum Status {\n    Ok\n    Err(string)\n}\n\nfn main() -> void {\n    let status: Status = Status.Err(\"bad\")\n}\n";

        let definition = definition_for_text(
            Path::new("main.nomo"),
            source,
            TextPosition {
                line: 8,
                character: 33,
            },
        )
        .unwrap()
        .unwrap();

        assert_eq!(
            definition,
            TextRange {
                start: TextPosition {
                    line: 4,
                    character: 4,
                },
                end: TextPosition {
                    line: 4,
                    character: 7,
                },
            }
        );
    }

    #[test]
    fn references_can_exclude_declaration() {
        let source = "package app.main\n\nstruct User {\n    email: string\n}\n\nfn main() -> void {\n    let user: User = User { email: \"hi\" }\n}\n";

        let references = references_for_text(
            Path::new("main.nomo"),
            source,
            TextPosition {
                line: 7,
                character: 14,
            },
            false,
        )
        .unwrap()
        .unwrap();

        assert_eq!(
            references,
            vec![
                TextRange {
                    start: TextPosition {
                        line: 7,
                        character: 14,
                    },
                    end: TextPosition {
                        line: 7,
                        character: 18,
                    },
                },
                TextRange {
                    start: TextPosition {
                        line: 7,
                        character: 21,
                    },
                    end: TextPosition {
                        line: 7,
                        character: 25,
                    },
                },
            ]
        );
    }

    #[test]
    fn project_definition_returns_cross_file_location() {
        let project = test_project("semantic_definition");
        let main = project.root.join("src/main.nomo");
        let math = project.root.join("src/math.nomo");
        write_source(
            &main,
            "package app.main\n\nimport app.math\n\nfn main() -> void {\n    let total: i64 = add(1, 2)\n}\n",
        );
        write_source(
            &math,
            "package app.math\n\n/// Adds numbers.\npub fn add(a: i64, b: i64) -> i64 {\n    return a + b\n}\n",
        );

        let source = fs::read_to_string(&main).unwrap();
        let definition = definition_for_project_text(
            &project,
            &main,
            &source,
            TextPosition {
                line: 5,
                character: 23,
            },
            &[],
        )
        .unwrap()
        .unwrap();

        assert_eq!(definition.path, math);
        assert_eq!(
            definition.range,
            TextRange {
                start: TextPosition {
                    line: 3,
                    character: 7,
                },
                end: TextPosition {
                    line: 3,
                    character: 10,
                },
            }
        );
    }

    #[test]
    fn project_definition_resolves_imported_dependency_public_symbol() {
        let root = env::temp_dir().join(format!(
            "nomo_semantic_dependency_definition_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let project_root = root.join("hello");
        let dependency_root = root.join("utils");
        fs::create_dir_all(project_root.join("src")).unwrap();
        fs::create_dir_all(dependency_root.join("src")).unwrap();
        fs::write(
            project_root.join("nomo.toml"),
            "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\nlocal_utils = { package = \"fynn/utils\", path = \"../utils\" }\n",
        )
        .unwrap();
        fs::write(
            dependency_root.join("nomo.toml"),
            "[package]\nnamespace = \"fynn\"\nname = \"utils\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
        )
        .unwrap();
        let main = project_root.join("src/main.nomo");
        let dep_module = dependency_root.join("src/path.nomo");
        let main_source = "package app.main\n\nimport local_utils.path\n\nfn main() -> void {\n    let total: i64 = join(1, 2)\n}\n";
        write_source(&main, main_source);
        write_source(
            &dep_module,
            "package local_utils.path\n\n/// Joins values.\npub fn join(a: i64, b: i64) -> i64 {\n    return a + b\n}\n\nfn hidden() -> i64 {\n    return 1\n}\n",
        );
        let project = Project {
            main: main.clone(),
            root: project_root,
            name: "hello".to_string(),
            workspace_root: None,
        };

        let definition = definition_for_project_text(
            &project,
            &main,
            main_source,
            TextPosition {
                line: 5,
                character: 23,
            },
            &[],
        )
        .unwrap()
        .unwrap();

        assert_eq!(
            fs::canonicalize(&definition.path).unwrap(),
            fs::canonicalize(&dep_module).unwrap()
        );
        assert_eq!(
            definition.range,
            TextRange {
                start: TextPosition {
                    line: 3,
                    character: 7,
                },
                end: TextPosition {
                    line: 3,
                    character: 11,
                },
            }
        );
        let missing_private = symbol_at_project_position(
            &project,
            &main,
            "package app.main\n\nimport local_utils.path\n\nfn main() -> void {\n    let total: i64 = hidden()\n}\n",
            TextPosition {
                line: 5,
                character: 23,
            },
            &[],
        )
        .unwrap();
        assert!(missing_private.is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn dependency_symbols_for_project_include_public_dependency_api_only() {
        let root = env::temp_dir().join(format!(
            "nomo_semantic_dependency_symbols_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let project_root = root.join("hello");
        let dependency_root = root.join("utils");
        fs::create_dir_all(project_root.join("src")).unwrap();
        fs::create_dir_all(dependency_root.join("src")).unwrap();
        fs::write(
            project_root.join("nomo.toml"),
            "[package]\nnamespace = \"fynn\"\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\nlocal_utils = { package = \"fynn/utils\", path = \"../utils\" }\n",
        )
        .unwrap();
        fs::write(
            dependency_root.join("nomo.toml"),
            "[package]\nnamespace = \"fynn\"\nname = \"utils\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
        )
        .unwrap();
        write_source(&project_root.join("src/main.nomo"), "package app.main\n");
        write_source(
            &dependency_root.join("src/path.nomo"),
            "package local_utils.path\n\npub struct PathInfo {\n    pub name: string\n    hidden: string\n}\n\npub fn join(a: string, b: string) -> string {\n    return a\n}\n\nfn hidden() -> string {\n    return \"hidden\"\n}\n",
        );
        let project = Project {
            main: project_root.join("src/main.nomo"),
            root: project_root,
            name: "hello".to_string(),
            workspace_root: None,
        };

        let symbols = dependency_symbols_for_project_with_overrides(&project, &[]).unwrap();

        let names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();
        assert!(names.contains(&"PathInfo"), "{names:?}");
        assert!(names.contains(&"name"), "{names:?}");
        assert!(names.contains(&"join"), "{names:?}");
        assert!(!names.contains(&"hidden"), "{names:?}");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_references_include_cross_file_identifier_locations() {
        let project = test_project("semantic_references");
        let main = project.root.join("src/main.nomo");
        let math = project.root.join("src/math.nomo");
        write_source(
            &main,
            "package app.main\n\nimport app.math\n\nfn main() -> void {\n    let total: i64 = add(1, 2)\n}\n",
        );
        write_source(
            &math,
            "package app.math\n\npub fn add(a: i64, b: i64) -> i64 {\n    return a + b\n}\n",
        );

        let source = fs::read_to_string(&main).unwrap();
        let references = references_for_project_text(
            &project,
            &main,
            &source,
            TextPosition {
                line: 5,
                character: 23,
            },
            true,
            &[],
        )
        .unwrap()
        .unwrap();

        assert!(references.iter().any(|location| {
            location.path == main
                && location.range
                    == TextRange {
                        start: TextPosition {
                            line: 5,
                            character: 21,
                        },
                        end: TextPosition {
                            line: 5,
                            character: 24,
                        },
                    }
        }));
        assert!(references.iter().any(|location| {
            location.path == math
                && location.range
                    == TextRange {
                        start: TextPosition {
                            line: 2,
                            character: 7,
                        },
                        end: TextPosition {
                            line: 2,
                            character: 10,
                        },
                    }
        }));
    }

    #[test]
    fn project_symbols_use_source_overlays() {
        let project = test_project("semantic_overlays");
        let main = project.root.join("src/main.nomo");
        let math = project.root.join("src/math.nomo");
        write_source(
            &main,
            "package app.main\n\nimport app.math\n\nfn main() -> void {\n    let total: i64 = add(1, 2)\n}\n",
        );
        write_source(
            &math,
            "package app.math\n\npub fn sub(a: i64, b: i64) -> i64 {\n    return a - b\n}\n",
        );

        let source = fs::read_to_string(&main).unwrap();
        let overlay =
            "package app.math\n\npub fn add(a: i64, b: i64) -> i64 {\n    return a + b\n}\n";
        let definition = definition_for_project_text(
            &project,
            &main,
            &source,
            TextPosition {
                line: 5,
                character: 23,
            },
            &[(math.clone(), overlay.to_string())],
        )
        .unwrap()
        .unwrap();

        assert_eq!(definition.path, math);
        assert_eq!(definition.range.start.line, 2);
    }

    fn test_project(name: &str) -> Project {
        let root = env::temp_dir().join(format!(
            "nomo_{name}_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("nomo.toml"),
            "[package]\nnamespace = \"app\"\nname = \"main\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
        )
        .unwrap();
        Project {
            main: root.join("src/main.nomo"),
            root,
            name: "main".to_string(),
            workspace_root: None,
        }
    }

    fn write_source(path: &Path, source: &str) {
        fs::write(path, source).unwrap();
    }
}

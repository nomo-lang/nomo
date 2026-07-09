use crate::diagnostic::Diagnostic;
use crate::lexer::lex;
use crate::parser::parse;
use crate::project::{Project, project_module_context, resolve_module_source_path};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::docs::extract_doc_comments;
use super::symbols::symbols_from_ast;
use super::{SemanticSymbol, SemanticSymbolKind, symbols_for_text};

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

pub(super) fn accessible_symbols_for_document(
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

pub(super) fn overrides_with_current(
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

pub(super) fn project_sources(
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

pub(super) fn is_project_nomo_source(source_root: &Path, path: &Path) -> bool {
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

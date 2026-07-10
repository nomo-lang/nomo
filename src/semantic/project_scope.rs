use crate::compiler::build_module_graph_with_overrides;
use crate::diagnostic::Diagnostic;
use crate::project::{Project, project_module_context};
use nomo_lsp_bridge::{public_symbols_for_text, symbols_for_text};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::SemanticSymbol;

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
    path: &Path,
    source: &str,
    source_overrides: &[(PathBuf, String)],
) -> Result<Vec<SemanticSymbol>, Diagnostic> {
    let context = project_module_context(project).map_err(|message| {
        Diagnostic::new(
            "E0901",
            message,
            &project.root.join("nomo.toml"),
            1,
            1,
            1,
            "",
        )
    })?;
    let graph = build_module_graph_with_overrides(
        path,
        source,
        Some(&context.local_source_root),
        &context.external_modules,
        source_overrides,
    )?;
    let overrides = source_overrides
        .iter()
        .map(|(path, source)| (path.clone(), source.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut symbols = Vec::new();
    for module in graph.modules() {
        let module_source = if module.source_path == path {
            source.to_string()
        } else if let Some(source) = overrides.get(&module.source_path) {
            source.clone()
        } else {
            fs::read_to_string(&module.source_path).map_err(|err| {
                Diagnostic::new(
                    "E0902",
                    format!("failed to read {}: {err}", module.source_path.display()),
                    &module.source_path,
                    1,
                    1,
                    1,
                    "",
                )
            })?
        };
        if module.source_path == path {
            symbols.extend(symbols_for_text(&module.source_path, &module_source)?);
        } else {
            symbols.extend(public_symbols_for_text(
                &module.source_path,
                &module_source,
            )?);
        }
    }
    Ok(symbols)
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

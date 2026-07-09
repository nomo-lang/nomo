use nomo::project::{discover_project, discover_workspace};
use nomo::{Diagnostic, format_source};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub(super) enum FormatError {
    Diagnostic(Diagnostic),
    Message(String),
}

impl FormatError {
    pub(super) fn human(&self) -> String {
        match self {
            FormatError::Diagnostic(diagnostic) => diagnostic.human(),
            FormatError::Message(message) => message.clone(),
        }
    }
}

pub(super) fn format_path(path: &Path, check: bool) -> Result<bool, FormatError> {
    let files = format_targets(path)?;
    let mut changed = false;
    for file in files {
        let source = fs::read_to_string(&file).map_err(|err| {
            FormatError::Message(format!("failed to read {}: {err}", file.display()))
        })?;
        let formatted = format_source(&file, &source).map_err(FormatError::Diagnostic)?;
        if formatted != source {
            changed = true;
            if check {
                println!("would format {}", file.display());
            } else {
                fs::write(&file, formatted).map_err(|err| {
                    FormatError::Message(format!("failed to write {}: {err}", file.display()))
                })?;
                println!("formatted {}", file.display());
            }
        }
    }
    Ok(changed)
}

fn format_targets(path: &Path) -> Result<Vec<PathBuf>, FormatError> {
    if path.extension().and_then(|ext| ext.to_str()) == Some("nomo") {
        if !path.is_file() {
            return Err(FormatError::Message(format!(
                "source file not found: {}",
                path.display()
            )));
        }
        return Ok(vec![path.to_path_buf()]);
    }

    match discover_project(path) {
        Ok(project) => return format_project_targets(&project.root),
        Err(project_err) => {
            if let Ok(workspace) = discover_workspace(path) {
                let mut files = Vec::new();
                for project in workspace.members {
                    files.extend(format_project_targets(&project.root)?);
                }
                files.sort();
                files.dedup();
                return Ok(files);
            }
            if !is_missing_manifest_error(&project_err) || !path.is_dir() {
                return Err(FormatError::Message(project_err));
            }
        }
    }

    let mut files = Vec::new();
    collect_nomo_files(path, &mut files)?;
    files.sort();
    if files.is_empty() {
        return Err(FormatError::Message(format!(
            "no .nomo files found under {}",
            path.display()
        )));
    }
    Ok(files)
}

fn format_project_targets(root: &Path) -> Result<Vec<PathBuf>, FormatError> {
    let src = root.join("src");
    if !src.is_dir() {
        return Err(FormatError::Message(format!(
            "source directory not found: {}",
            src.display()
        )));
    }
    let mut files = Vec::new();
    collect_nomo_files(&src, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_nomo_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), FormatError> {
    for entry in fs::read_dir(dir).map_err(|err| {
        FormatError::Message(format!("failed to read directory {}: {err}", dir.display()))
    })? {
        let entry = entry.map_err(|err| FormatError::Message(err.to_string()))?;
        let path = entry.path();
        if path.is_dir() {
            collect_nomo_files(&path, files)?;
        } else if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("nomo") {
            files.push(path);
        }
    }
    Ok(())
}

fn is_missing_manifest_error(message: &str) -> bool {
    message.contains("could not find nomo.toml")
}

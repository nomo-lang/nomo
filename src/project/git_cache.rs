use nomo_manifest::parse_manifest_at_root;
use nomo_resolver::hex_lower;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(super) fn resolve_git_source(
    base_root: &Path,
    alias: &str,
    package: &str,
    git: &str,
    branch: Option<&str>,
    tag: Option<&str>,
    rev: Option<&str>,
) -> Result<PathBuf, String> {
    let cache_root = base_root.join(".nomo/deps/git");
    fs::create_dir_all(&cache_root).map_err(|err| err.to_string())?;
    let checkout = cache_root.join(git_cache_key(package, git));
    if checkout.exists() {
        run_git_fetch(&checkout, alias)?;
    } else {
        let clone_source = git_clone_source(base_root, git);
        let output = Command::new("git")
            .arg("clone")
            .arg("--quiet")
            .arg(&clone_source)
            .arg(&checkout)
            .output()
            .map_err(|err| format!("failed to run git clone for dependency `{alias}`: {err}"))?;
        if !output.status.success() {
            return Err(format!(
                "failed to clone git dependency `{alias}` from {git}:\n{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }

    if let Some(branch) = branch {
        git_checkout(&checkout, alias, "branch", branch)?;
        git_pull_ff_only(&checkout, alias, branch)?;
    } else if let Some(tag) = tag {
        git_checkout(&checkout, alias, "tag", &format!("refs/tags/{tag}"))?;
    } else if let Some(rev) = rev {
        git_checkout(&checkout, alias, "rev", rev)?;
    } else if checkout.exists() {
        checkout_default_branch(&checkout, alias)?;
    }

    fs::canonicalize(&checkout).map_err(|err| err.to_string())
}

pub(super) fn resolve_git_source_offline(
    base_root: &Path,
    alias: &str,
    package: &str,
    git: &str,
    _branch: Option<&str>,
    _tag: Option<&str>,
    _rev: Option<&str>,
) -> Result<PathBuf, String> {
    let checkout = base_root
        .join(".nomo/deps/git")
        .join(git_cache_key(package, git));
    if checkout.exists() {
        fs::canonicalize(&checkout).map_err(|err| err.to_string())
    } else {
        Err(format!(
            "offline mode cannot fetch git dependency `{alias}` from {git}; missing cached checkout at {}",
            checkout.display()
        ))
    }
}

pub(super) fn git_head_rev(root: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .map_err(|err| format!("failed to resolve git HEAD at {}: {err}", root.display()))?;
    if !output.status.success() {
        return Err(format!(
            "failed to resolve git HEAD at {}:\n{}{}",
            root.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub(super) fn locked_git_root(
    base_root: &Path,
    package: &str,
    git: &str,
) -> Result<Option<PathBuf>, String> {
    let cache_root = base_root.join(".nomo/deps/git");
    if !cache_root.is_dir() {
        return Ok(None);
    }
    let path = cache_root.join(git_cache_key(package, git));
    if !path.is_dir() {
        return Ok(None);
    }
    let Ok(manifest) = parse_manifest_at_root(&path) else {
        return Ok(None);
    };
    let actual_id = format!("{}/{}", manifest.package.namespace, manifest.package.name);
    if actual_id != package {
        return Ok(None);
    }
    let Some(remote_url) = git_remote_url(&path) else {
        return Ok(None);
    };
    let clone_source = git_clone_source(base_root, git)
        .to_string_lossy()
        .replace('\\', "/");
    if remote_url != git && remote_url.replace('\\', "/") != clone_source {
        return Ok(None);
    }
    fs::canonicalize(&path)
        .map(Some)
        .map_err(|err| err.to_string())
}

fn git_clone_source(base_root: &Path, git: &str) -> PathBuf {
    let path = Path::new(git);
    if path.is_absolute() || git.contains("://") || git.contains(':') {
        path.to_path_buf()
    } else {
        base_root.join(path)
    }
}

fn run_git_fetch(checkout: &Path, alias: &str) -> Result<(), String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(checkout)
        .arg("fetch")
        .arg("--tags")
        .arg("--prune")
        .arg("origin")
        .output()
        .map_err(|err| format!("failed to run git fetch for dependency `{alias}`: {err}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "failed to fetch git dependency `{alias}`:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

fn git_checkout(
    checkout: &Path,
    alias: &str,
    selector_name: &str,
    selector: &str,
) -> Result<(), String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(checkout)
        .arg("checkout")
        .arg("--quiet")
        .arg(selector)
        .output()
        .map_err(|err| {
            format!(
                "failed to run git checkout for dependency `{alias}` at {selector_name} `{selector}`: {err}"
            )
        })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "failed to checkout git dependency `{alias}` at {selector_name} `{selector}`:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

fn git_pull_ff_only(checkout: &Path, alias: &str, branch: &str) -> Result<(), String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(checkout)
        .arg("pull")
        .arg("--ff-only")
        .arg("--quiet")
        .output()
        .map_err(|err| {
            format!("failed to run git pull for dependency `{alias}` at branch `{branch}`: {err}")
        })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "failed to pull git dependency `{alias}` at branch `{branch}`:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

fn checkout_default_branch(checkout: &Path, alias: &str) -> Result<(), String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(checkout)
        .arg("symbolic-ref")
        .arg("--short")
        .arg("refs/remotes/origin/HEAD")
        .output()
        .map_err(|err| {
            format!("failed to resolve default branch for git dependency `{alias}`: {err}")
        })?;
    if !output.status.success() {
        return Ok(());
    }
    let remote_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let branch = remote_branch
        .strip_prefix("origin/")
        .unwrap_or(&remote_branch)
        .to_string();
    if branch.is_empty() {
        return Ok(());
    }
    git_checkout(checkout, alias, "branch", &branch)?;
    git_pull_ff_only(checkout, alias, &branch)
}

fn git_cache_key(package: &str, git: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(package.as_bytes());
    hasher.update(b"\0");
    hasher.update(git.as_bytes());
    format!("git-{}", hex_lower(&hasher.finalize()))
}

fn git_remote_url(root: &Path) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("remote")
        .arg("get-url")
        .arg("origin")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

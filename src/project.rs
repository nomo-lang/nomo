#[cfg(test)]
use crate::compiler::ExternalModule;
use crate::compiler::check_source_text_with_project_modules;
use crate::diagnostic::Diagnostic;
#[cfg(test)]
use nomo_lockfile::parse_lockfile_text;
pub use nomo_lockfile::{DependencyGraph, ResolvedDependency};
#[cfg(test)]
use nomo_manifest::parse_manifest_text;
pub use nomo_manifest::{
    Dependency, DependencyAddSpec, DependencySource, FfiLinkMetadata, Manifest, PackageMetadata,
    parse_manifest_at_root,
};
use nomo_manifest::{is_package_name, workspace_root_for_package};
use std::fs;
use std::path::{Path, PathBuf};

mod build;
mod dependency_resolution;
mod dependency_tree;
mod ffi;
mod git_cache;
mod modules;
mod package;
mod registry_http;
mod resolve;
mod running;
mod testing;
mod update;
mod vendor;
mod workspace;

use build::configure_c_compile_command;
pub use build::{
    build_project, build_project_with_diagnostics, build_project_with_options, clean_project,
};
use dependency_resolution::{
    dependency_graph_from_lockfile, resolve_dependency_graph, resolve_dependency_graph_for_lock,
};
use dependency_tree::render_dependency_tree;
use ffi::project_ffi_link_metadata_with_options;
pub use modules::{
    ProjectModuleContext, project_module_context, project_module_context_with_options,
    resolve_module_source_path,
};
pub use package::{
    PublishPackage, add_registry_dependency, prepare_publish_package, remove_dependency,
};
pub use registry_http::{
    RegistryLogin, RegistrySearchResult, add_registry_package_owner, login_registry,
    publish_package_archive, remove_registry_package_owner, search_registry_packages,
    yank_registry_package,
};
pub use resolve::{
    resolve_project_dependencies, resolve_project_dependencies_with_options,
    resolve_workspace_dependencies, resolve_workspace_dependencies_with_options,
};
pub use running::{
    run_project, run_project_with_args, run_project_with_args_and_diagnostics,
    run_standalone_script_with_args_and_diagnostics,
};
pub use testing::{
    ProjectTestCaseResult, ProjectTestOptions, ProjectTestReport, ProjectTestStatus,
    run_project_tests_with_options,
};
pub use update::{update_project_dependencies, update_workspace_dependencies};
pub use vendor::{vendor_project_dependencies, vendor_workspace_dependencies};
pub use workspace::{WorkspaceGraph, discover_workspace};

#[derive(Debug, Clone)]
pub struct Project {
    pub root: PathBuf,
    pub name: String,
    pub main: PathBuf,
    pub workspace_root: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DependencyResolutionOptions {
    pub locked: bool,
    pub offline: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DependencyUpdateOptions {
    pub resolution: DependencyResolutionOptions,
    pub precise: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyVendorOptions {
    pub dir: PathBuf,
    pub sync: bool,
}

impl Default for DependencyVendorOptions {
    fn default() -> Self {
        Self {
            dir: PathBuf::from("vendor"),
            sync: false,
        }
    }
}

#[derive(Debug)]
pub enum BuildError {
    Diagnostic(Diagnostic),
    Message(String),
}

impl BuildError {
    pub fn human(&self) -> String {
        match self {
            BuildError::Diagnostic(diagnostic) => diagnostic.human(),
            BuildError::Message(message) => message.clone(),
        }
    }
}

pub fn create_project(root: &Path, name: &str) -> Result<Project, String> {
    if !is_package_name(name) {
        return Err(format!("invalid project name `{name}`"));
    }
    let project_root = root.join(name);
    if project_root.exists() {
        return Err(format!(
            "destination already exists: {}",
            project_root.display()
        ));
    }
    fs::create_dir_all(project_root.join("src")).map_err(|err| err.to_string())?;
    fs::write(
        project_root.join("nomo.toml"),
        format!(
            "[package]\nnamespace = \"local\"\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2026\"\n"
        ),
    )
    .map_err(|err| err.to_string())?;
    fs::write(
        project_root.join("src/main.nomo"),
        "package app.main\n\nimport std.io\n\nfn greeting() -> string {\n    return \"Hello, Nomo\"\n}\n\nfn main() -> void {\n    let message: string = greeting()\n    io.println(message)\n}\n",
    )
    .map_err(|err| err.to_string())?;
    discover_project(&project_root)
}

pub fn discover_project(path: &Path) -> Result<Project, String> {
    let source_file = path.extension().and_then(|ext| ext.to_str()) == Some("nomo");
    let search_root = if source_file {
        path.parent()
            .ok_or_else(|| format!("source file has no parent: {}", path.display()))?
    } else {
        path
    };
    let root = find_manifest_root(search_root).ok_or_else(|| {
        format!(
            "could not find nomo.toml for {}; use `nomoc` for standalone source files",
            path.display()
        )
    })?;
    let main = if source_file {
        path.to_path_buf()
    } else {
        root.join("src/main.nomo")
    };
    let manifest = parse_manifest_at_root(&root)?;
    let workspace_root = workspace_root_for_package(&root)?;
    Ok(Project {
        root,
        name: manifest.package.name,
        main,
        workspace_root,
    })
}

fn find_manifest_root(start: &Path) -> Option<PathBuf> {
    for candidate in start.ancestors() {
        if candidate.join("nomo.toml").exists() {
            return Some(candidate.to_path_buf());
        }
    }
    None
}

pub fn dependency_tree(project: &Project) -> Result<String, String> {
    dependency_tree_with_options(project, DependencyResolutionOptions::default())
}

pub fn dependency_tree_with_options(
    project: &Project,
    options: DependencyResolutionOptions,
) -> Result<String, String> {
    let lock_root = project.lock_root();
    let graph = if lock_root.join("nomo.lock").is_file() {
        dependency_graph_from_lockfile(&project.root, &lock_root)?
    } else if options.locked {
        return Err(format!(
            "nomo.lock is required for locked mode at {}",
            lock_root.join("nomo.lock").display()
        ));
    } else {
        resolve_dependency_graph_for_lock(&project.root, None, None, options.offline)?
    };
    Ok(render_dependency_tree(&graph))
}

pub fn dependency_tree_current_sources(project: &Project) -> Result<String, String> {
    Ok(render_dependency_tree(&resolve_dependency_graph(
        &project.root,
    )?))
}

impl Project {
    fn lock_root(&self) -> PathBuf {
        self.workspace_root
            .clone()
            .unwrap_or_else(|| self.root.clone())
    }
}

pub fn check_project(project: &Project) -> Result<(), Diagnostic> {
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
    let source = fs::read_to_string(&project.main).map_err(|err| {
        Diagnostic::new(
            "E0001",
            format!("failed to read source file: {err}"),
            &project.main,
            1,
            1,
            1,
            "",
        )
    })?;
    check_source_text_with_project_modules(
        &project.main,
        &source,
        Some(&context.local_source_root),
        &context.external_import_roots,
        &context.external_modules,
    )
    .map(|_| ())
}

pub fn project_package_id(project: &Project) -> Result<String, String> {
    let manifest = parse_manifest_at_root(&project.root)?;
    Ok(package_id(&manifest.package))
}

fn package_id(package: &PackageMetadata) -> String {
    format!("{}/{}", package.namespace, package.name)
}

pub fn clean_dependency_cache(path: &Path) -> Result<PathBuf, String> {
    let root = match discover_project(path) {
        Ok(project) => project.lock_root(),
        Err(project_err) => match discover_workspace(path) {
            Ok(workspace) => workspace.root,
            Err(_) => return Err(project_err),
        },
    };
    let cache_dir = root.join(".nomo/deps/git");
    if cache_dir.exists() {
        fs::remove_dir_all(&cache_dir).map_err(|err| err.to_string())?;
    }
    Ok(cache_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_namespace_first_manifest() {
        let manifest = "[package]\nnamespace = \"fynn\"\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\nstd = { package = \"nomo-lang/std\", version = \"0.1.0\" }\nutils = { package = \"fynn/utils\", path = \"../utils\" }\n";
        let parsed = parse_manifest_text(manifest, Path::new("demo")).unwrap();

        assert_eq!(parsed.package.namespace, "fynn");
        assert_eq!(parsed.package.name, "demo");
        assert_eq!(parsed.dependencies.len(), 1);
        assert_eq!(parsed.dependencies[0].alias, "utils");
        assert_eq!(parsed.dependencies[0].package, "fynn/utils");
    }

    #[test]
    fn parses_legacy_std_dependency_as_implicit_builtin() {
        let manifest =
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n\n[dependencies]\nstd = \"0.1.0\"\n";
        let parsed = parse_manifest_text(manifest, Path::new("demo")).unwrap();

        assert_eq!(parsed.package.namespace, "local");
        assert_eq!(parsed.package.name, "demo");
        assert!(parsed.dependencies.is_empty());
    }

    #[test]
    fn parses_standard_toml_comments_and_escaped_strings() {
        let manifest = r#"# package comment
[package]
namespace = "fynn"
name = "escaped-demo"
version = "0.1.0"
edition = "2026"

[dependencies]
json = { package = "nomo-lang/json", version = "0.1.0", registry = "https://packages.example.com/v1?token=\"dev\"" }

[dependencies.local_utils]
package = "fynn/utils"
path = "../utils"
"#;
        let parsed = parse_manifest_text(manifest, Path::new("demo")).unwrap();

        assert_eq!(parsed.dependencies.len(), 2);
        assert_eq!(parsed.dependencies[0].alias, "json");
        assert_eq!(
            parsed.dependencies[0].source,
            DependencySource::Registry {
                version: "0.1.0".to_string(),
                registry: Some("https://packages.example.com/v1?token=\"dev\"".to_string()),
            }
        );
        assert_eq!(parsed.dependencies[1].alias, "local_utils");
        assert_eq!(
            parsed.dependencies[1].source,
            DependencySource::Path {
                path: "../utils".to_string(),
            }
        );
    }

    #[test]
    fn parses_ffi_link_metadata_and_rebases_library_paths() {
        let root = Path::new("/tmp/nomo-ffi-demo");
        let manifest = r#"[package]
namespace = "local"
name = "ffi-demo"
version = "0.1.0"
edition = "2026"

[ffi]
libraries = ["z"]
library_paths = ["native/lib"]
frameworks = ["Security"]
link_args = ["-Wl,-rpath,@loader_path"]
"#;
        let parsed = parse_manifest_text(manifest, root).unwrap();

        assert_eq!(parsed.ffi.libraries, vec!["z"]);
        assert_eq!(parsed.ffi.library_paths, vec![root.join("native/lib")]);
        assert_eq!(parsed.ffi.frameworks, vec!["Security"]);
        assert_eq!(parsed.ffi.link_args, vec!["-Wl,-rpath,@loader_path"]);
    }

    #[test]
    fn parses_lockfile_as_standard_toml() {
        let packages = parse_lockfile_text(
            r#"# This file is generated by `nomo deps resolve`.

[[package]]
id = "fynn/utils"
alias = "local_utils"
source = "path+../utils"
dependencies = [
  "cli -> nomo-lang/cli",
]

[[package]]
id = "nomo-lang/cli"
alias = "cli"
version = "0.2.1"
source = "registry+nomo-lang/cli"
"#,
        )
        .unwrap();

        assert_eq!(packages.len(), 2);
        assert_eq!(packages[0].package, "fynn/utils");
        assert_eq!(packages[0].dependencies[0].alias, "cli");
        assert_eq!(packages[0].dependencies[0].package, "nomo-lang/cli");
        assert_eq!(
            packages[1].source,
            DependencySource::Registry {
                version: "0.2.1".to_string(),
                registry: None,
            }
        );
    }

    #[test]
    fn rejects_unknown_lockfile_fields() {
        let err = parse_lockfile_text(
            "[[package]]\nid = \"nomo-lang/json\"\nalias = \"json\"\nversion = \"0.1.0\"\nsource = \"registry+nomo-lang/json\"\nextra = \"nope\"\n",
        )
        .unwrap_err();

        assert!(err.contains("unknown field"), "{err}");
    }

    #[test]
    fn rejects_dependency_alias_that_overrides_std() {
        let manifest = "[package]\nname = \"demo\"\n\n[dependencies]\nstd = { package = \"other/std\", version = \"0.1.0\" }\n";
        let err = parse_manifest_text(manifest, Path::new("demo")).unwrap_err();

        assert!(err.contains("alias `std` is reserved"), "{err}");
    }

    #[test]
    fn parses_workspace_member_package_and_dependency_inheritance() {
        let root = temp_test_root("workspace-inheritance");
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        let app = root.join("apps/cli");
        let core = root.join("packages/core");
        fs::create_dir_all(app.join("src")).unwrap();
        fs::create_dir_all(core.join("src")).unwrap();
        fs::write(
            root.join("nomo.toml"),
            "[workspace]\nmembers = [\"apps/*\", \"packages/*\"]\n\n[workspace.package]\nnamespace = \"fynn\"\nedition = \"2026\"\n\n[workspace.dependencies]\njson = { package = \"nomo-lang/json\", version = \"0.1.0\" }\ncore = { package = \"fynn/core\", path = \"packages/core\" }\n",
        )
        .unwrap();
        fs::write(
            app.join("nomo.toml"),
            "[package]\nname = \"cli\"\nversion = \"0.1.0\"\nnamespace.workspace = true\nedition.workspace = true\n\n[dependencies]\njson.workspace = true\ncore.workspace = true\n",
        )
        .unwrap();
        fs::write(
            core.join("nomo.toml"),
            "[package]\nname = \"core\"\nversion = \"0.1.0\"\nnamespace.workspace = true\nedition.workspace = true\n",
        )
        .unwrap();

        let manifest = parse_manifest_at_root(&app).unwrap();

        assert_eq!(manifest.package.namespace, "fynn");
        assert_eq!(manifest.package.name, "cli");
        assert_eq!(manifest.package.edition, "2026");
        assert_eq!(manifest.dependencies.len(), 2);
        assert_eq!(manifest.dependencies[0].alias, "core");
        assert_eq!(manifest.dependencies[0].package, "fynn/core");
        assert_eq!(
            manifest.dependencies[0].source,
            DependencySource::Path {
                path: "../../packages/core".to_string(),
            }
        );
        assert_eq!(manifest.dependencies[1].alias, "json");
        assert_eq!(manifest.dependencies[1].package, "nomo-lang/json");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn rejects_workspace_root_without_package_as_project() {
        let root = temp_test_root("workspace-root-without-package");
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("nomo.toml"),
            "[workspace]\nmembers = [\"apps/*\"]\n",
        )
        .unwrap();

        let err = discover_project(&root).unwrap_err();

        assert!(err.contains("workspace manifest"), "{err}");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn discovers_workspace_members_defaults_and_excludes() {
        let root = temp_test_root("workspace-discovery");
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        let app = root.join("apps/cli");
        let core = root.join("packages/core");
        let skipped = root.join("target/generated");
        fs::create_dir_all(app.join("src")).unwrap();
        fs::create_dir_all(core.join("src")).unwrap();
        fs::create_dir_all(skipped.join("src")).unwrap();
        fs::write(
            root.join("nomo.toml"),
            "[workspace]\nmembers = [\"apps/*\", \"packages/*\", \"target/*\"]\ndefault-members = [\"apps/cli\"]\nexclude = [\"target\"]\n\n[workspace.package]\nnamespace = \"fynn\"\nedition = \"2026\"\n",
        )
        .unwrap();
        for (dir, name) in [(&app, "cli"), (&core, "core"), (&skipped, "generated")] {
            fs::write(
                dir.join("nomo.toml"),
                format!(
                    "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nnamespace.workspace = true\nedition.workspace = true\n"
                ),
            )
            .unwrap();
            fs::write(dir.join("src/main.nomo"), "package app.main\n").unwrap();
        }

        let workspace = discover_workspace(&app.join("src/main.nomo")).unwrap();

        assert_eq!(workspace.root, root);
        assert_eq!(
            workspace
                .members
                .iter()
                .map(|project| project.name.as_str())
                .collect::<Vec<_>>(),
            vec!["cli", "core"]
        );
        assert_eq!(
            workspace
                .default_members
                .iter()
                .map(|project| project.name.as_str())
                .collect::<Vec<_>>(),
            vec!["cli"]
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn falls_back_to_directory_name_when_package_name_is_missing() {
        let root = temp_test_root("manifest-fallback");
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        let project_root = root.join("fallback-demo");
        fs::create_dir_all(project_root.join("src")).unwrap();
        fs::write(project_root.join("nomo.toml"), "[dependencies]\n").unwrap();
        fs::write(project_root.join("src/main.nomo"), "package app.main\n").unwrap();

        let project = discover_project(&project_root).unwrap();

        assert_eq!(project.name, "fallback-demo");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn discovers_project_root_from_source_file_path() {
        let root = temp_test_root("discover-source-file");
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        fs::create_dir_all(&root).unwrap();
        let project = create_project(&root, "source-demo").unwrap();

        let discovered = discover_project(&project.root.join("src/main.nomo")).unwrap();

        assert_eq!(discovered.root, project.root);
        assert_eq!(discovered.name, "source-demo");
        assert_eq!(discovered.main, project.root.join("src/main.nomo"));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn builds_source_file_path_under_project_root() {
        let root = temp_test_root("source-file-build");
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        fs::create_dir_all(&root).unwrap();
        let project = create_project(&root, "source-build-demo").unwrap();
        let discovered = discover_project(&project.root.join("src/main.nomo")).unwrap();

        let artifact = build_project(&discovered, true).unwrap();

        assert_eq!(artifact, project.root.join("build/c/main.c"));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn cleans_project_build_directory() {
        let root = temp_test_root("clean-project");
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        fs::create_dir_all(&root).unwrap();
        let project = create_project(&root, "clean-demo").unwrap();
        let artifact = build_project(&project, true).unwrap();
        assert!(artifact.exists());

        let cleaned = clean_project(&project).unwrap();

        assert_eq!(cleaned, project.root.join("build"));
        assert!(!cleaned.exists());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn rejects_directory_without_manifest() {
        let root = temp_test_root("missing-manifest-dir");
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.nomo"), "package app.main\n").unwrap();

        let err = discover_project(&root).unwrap_err();

        assert!(err.contains("nomo.toml"));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn rejects_standalone_source_file_without_manifest() {
        let root = temp_test_root("standalone-source");
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        fs::create_dir_all(&root).unwrap();
        let source = root.join("main.nomo");
        fs::write(&source, "package app.main\n").unwrap();

        let err = discover_project(&source).unwrap_err();

        assert!(err.contains("nomo.toml"));
        assert!(err.contains("nomoc"));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn runs_project_with_forwarded_args() {
        let root = temp_test_root("forwarded-args");
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        fs::create_dir_all(&root).unwrap();
        let project = create_project(&root, "args-demo").unwrap();
        fs::write(
            project.root.join("src/main.nomo"),
            r#"package app.main

import std.env
import std.io
import std.array

fn main() -> void {
    let args: Array<string> = env.args()
    let size: u64 = args.len()
    let status: string = if size == 2 {
        "ok"
    } else {
        panic("expected one forwarded arg")
    }
    io.println(status)
}
"#,
        )
        .unwrap();

        let status = run_project_with_args(&project, &["hello".to_string()]).unwrap();
        assert_eq!(status, 0);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn resolves_module_source_paths_for_local_and_dependency_imports() {
        let root = temp_test_root("module-source-resolution");
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        fs::create_dir_all(&root).unwrap();
        let project = create_project(&root, "module-demo").unwrap();
        fs::write(
            project.root.join("src/math.nomo"),
            "package app.math\n\npub fn add() -> i64 {\n    return 1\n}\n",
        )
        .unwrap();

        let dependency = root.join("local-utils");
        fs::create_dir_all(dependency.join("src/path")).unwrap();
        fs::write(
            dependency.join("nomo.toml"),
            "[package]\nnamespace = \"local\"\nname = \"utils\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
        )
        .unwrap();
        fs::write(
            dependency.join("src/path/main.nomo"),
            "package local_utils.path\n\npub fn join() -> i64 {\n    return 1\n}\n",
        )
        .unwrap();

        let context = ProjectModuleContext {
            local_source_root: project.root.join("src"),
            external_import_roots: vec!["local_utils".to_string()],
            external_modules: vec![ExternalModule {
                import_root: "local_utils".to_string(),
                source_root: dependency.join("src"),
            }],
        };

        assert_eq!(
            resolve_module_source_path(&context, "app", &["app".to_string(), "math".to_string()]),
            Some(project.root.join("src/math.nomo"))
        );
        assert_eq!(
            resolve_module_source_path(
                &context,
                "app",
                &["local_utils".to_string(), "path".to_string()]
            ),
            Some(dependency.join("src/path/main.nomo"))
        );
        assert_eq!(
            resolve_module_source_path(&context, "app", &["std".to_string(), "io".to_string()]),
            None
        );
        fs::remove_dir_all(&root).unwrap();
    }

    fn temp_test_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("nomo-project-test-{name}-{}", std::process::id()))
    }
}

use super::{
    DependencyResolutionOptions, Project,
    dependency_resolution::{
        locked_dependency_graph_and_source_base, resolve_dependency_graph_for_lock,
        validate_project_lock_direct_dependencies,
    },
    modules::resolved_dependency_module_root,
    package_id,
};
use nomo_graph::{Cycle, DirectedGraph};
use nomo_lockfile::{ResolvedDependency, filter_dependency_graph_for_target};
use nomo_lsp_bridge::{SemanticSymbol, public_symbols_for_text};
use nomo_manifest::{DependencySource, PackageMetadata, parse_manifest_at_root};
use nomo_target::TargetTriple;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackageId {
    namespace: String,
    name: String,
}

impl PackageId {
    pub fn new(namespace: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            name: name.into(),
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        nomo_manifest::validate_package_id(value)?;
        let (namespace, name) = value
            .split_once('/')
            .expect("validated package id must contain one slash");
        Ok(Self::new(namespace, name))
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn canonical(&self) -> String {
        format!("{}/{}", self.namespace, self.name)
    }

    fn from_metadata(package: &PackageMetadata) -> Self {
        Self::new(&package.namespace, &package.name)
    }
}

impl fmt::Display for PackageId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}/{}", self.namespace, self.name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageSource {
    Root {
        path: PathBuf,
    },
    Registry {
        version: String,
        registry: Option<String>,
    },
    Path {
        path: String,
    },
    Git {
        git: String,
        branch: Option<String>,
        tag: Option<String>,
        rev: Option<String>,
    },
}

impl From<&DependencySource> for PackageSource {
    fn from(source: &DependencySource) -> Self {
        match source {
            DependencySource::Registry { version, registry } => Self::Registry {
                version: version.clone(),
                registry: registry.clone(),
            },
            DependencySource::Path { path } => Self::Path { path: path.clone() },
            DependencySource::Git {
                git,
                branch,
                tag,
                rev,
            } => Self::Git {
                git: git.clone(),
                branch: branch.clone(),
                tag: tag.clone(),
                rev: rev.clone(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageNode {
    pub id: PackageId,
    pub version: Option<String>,
    pub source: PackageSource,
    pub source_root: Option<PathBuf>,
    pub public_api: Vec<SemanticSymbol>,
}

impl PackageNode {
    pub fn has_source(&self) -> bool {
        self.source_root.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageDependency {
    pub alias: String,
    pub package: PackageId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageGraph {
    root: PackageId,
    packages: BTreeMap<PackageId, PackageNode>,
    dependency_edges: BTreeMap<PackageId, BTreeSet<PackageDependency>>,
    dependencies: DirectedGraph<PackageId>,
}

impl PackageGraph {
    fn new(root: PackageNode) -> Self {
        let root_id = root.id.clone();
        let mut packages = BTreeMap::new();
        packages.insert(root_id.clone(), root);
        let mut dependency_edges = BTreeMap::new();
        dependency_edges.insert(root_id.clone(), BTreeSet::new());
        let mut dependencies = DirectedGraph::new();
        dependencies.add_node(root_id.clone());
        Self {
            root: root_id,
            packages,
            dependency_edges,
            dependencies,
        }
    }

    pub fn root(&self) -> &PackageId {
        &self.root
    }

    pub fn package(&self, id: &PackageId) -> Option<&PackageNode> {
        self.packages.get(id)
    }

    pub fn package_by_id(&self, id: &str) -> Option<&PackageNode> {
        PackageId::parse(id).ok().and_then(|id| self.package(&id))
    }

    pub fn packages(&self) -> impl ExactSizeIterator<Item = &PackageNode> {
        self.packages.values()
    }

    pub fn dependencies<'a>(
        &'a self,
        id: &PackageId,
    ) -> impl Iterator<Item = &'a PackageDependency> {
        self.dependency_edges.get(id).into_iter().flatten()
    }

    pub fn package_count(&self) -> usize {
        self.packages.len()
    }

    pub fn contains(&self, id: &PackageId) -> bool {
        self.packages.contains_key(id)
    }

    pub fn topological_order(&self) -> Vec<PackageId> {
        self.dependencies
            .topological_sort()
            .expect("validated package graph must remain acyclic")
    }

    fn add_package(&mut self, package: PackageNode) -> Result<bool, String> {
        if let Some(existing) = self.packages.get(&package.id) {
            if existing != &package {
                return Err(format!(
                    "package `{}` resolved with conflicting graph metadata",
                    package.id
                ));
            }
            return Ok(false);
        }
        self.dependencies.add_node(package.id.clone());
        self.dependency_edges.entry(package.id.clone()).or_default();
        self.packages.insert(package.id.clone(), package);
        Ok(true)
    }

    fn add_dependency(
        &mut self,
        package: PackageId,
        dependency: PackageDependency,
    ) -> Option<Cycle<PackageId>> {
        self.dependency_edges
            .entry(package.clone())
            .or_default()
            .insert(dependency.clone());
        self.dependencies
            .add_edge(package, dependency.package.clone());
        self.dependencies.find_cycle()
    }
}

pub fn project_package_graph(project: &Project) -> Result<PackageGraph, String> {
    project_package_graph_with_options(project, DependencyResolutionOptions::default())
}

pub fn project_package_graph_with_options(
    project: &Project,
    options: DependencyResolutionOptions,
) -> Result<PackageGraph, String> {
    let target = TargetTriple::host()?;
    project_package_graph_for_target_with_options(project, options, &target)
}

pub fn project_package_graph_for_target_with_options(
    project: &Project,
    options: DependencyResolutionOptions,
    target: &TargetTriple,
) -> Result<PackageGraph, String> {
    let lock_root = project.lock_root();
    let (dependency_graph, source_base) =
        if options.locked || (options.offline && lock_root.join("nomo.lock").is_file()) {
            let (graph, source_base) = locked_dependency_graph_and_source_base(project)?;
            validate_project_lock_direct_dependencies(project, &graph)?;
            (graph, source_base)
        } else {
            let source_base = fs::canonicalize(&lock_root).map_err(|err| err.to_string())?;
            let graph = resolve_dependency_graph_for_lock(
                &project.root,
                Some(&source_base),
                Some(&source_base),
                options.offline,
            )?;
            (graph, source_base)
        };
    let dependency_graph = filter_dependency_graph_for_target(&dependency_graph, target);

    let root_path = fs::canonicalize(&project.root).map_err(|err| err.to_string())?;
    let root_id = PackageId::from_metadata(&dependency_graph.root);
    let root = PackageNode {
        id: root_id.clone(),
        version: Some(dependency_graph.root.version.clone()),
        source: PackageSource::Root {
            path: root_path.clone(),
        },
        source_root: Some(root_path.clone()),
        public_api: collect_public_api(&root_path.join("src"))?,
    };
    let mut graph = PackageGraph::new(root);
    let mut expanded = BTreeSet::from([root_id.clone()]);
    add_resolved_dependencies(
        &mut graph,
        &root_id,
        &dependency_graph.dependencies,
        &source_base,
        &mut expanded,
    )?;
    Ok(graph)
}

fn add_resolved_dependencies(
    graph: &mut PackageGraph,
    package: &PackageId,
    dependencies: &[ResolvedDependency],
    source_base: &Path,
    expanded: &mut BTreeSet<PackageId>,
) -> Result<(), String> {
    for dependency in dependencies {
        let dependency_id = PackageId::parse(&dependency.package)?;
        let edge = PackageDependency {
            alias: dependency.alias.clone(),
            package: dependency_id.clone(),
        };
        if let Some(cycle) = graph.add_dependency(package.clone(), edge) {
            return Err(format!("cyclic package dependency: {cycle}"));
        }

        let source_root = resolved_dependency_module_root(source_base, dependency)?;
        let (version, public_api) = match source_root.as_deref() {
            Some(source_root) => {
                let manifest = parse_manifest_at_root(source_root)?;
                let actual_id = package_id(&manifest.package);
                if actual_id != dependency.package {
                    return Err(format!(
                        "dependency `{}` expected package `{}`, found `{actual_id}`",
                        dependency.alias, dependency.package
                    ));
                }
                (
                    Some(manifest.package.version),
                    collect_public_api(&source_root.join("src"))?,
                )
            }
            None => (dependency_version(&dependency.source), Vec::new()),
        };
        graph.add_package(PackageNode {
            id: dependency_id.clone(),
            version,
            source: PackageSource::from(&dependency.source),
            source_root,
            public_api,
        })?;

        if expanded.insert(dependency_id.clone()) {
            add_resolved_dependencies(
                graph,
                &dependency_id,
                &dependency.dependencies,
                source_base,
                expanded,
            )?;
        }
    }
    Ok(())
}

fn dependency_version(source: &DependencySource) -> Option<String> {
    match source {
        DependencySource::Registry { version, .. } => Some(version.clone()),
        DependencySource::Path { .. } | DependencySource::Git { .. } => None,
    }
}

fn collect_public_api(source_root: &Path) -> Result<Vec<SemanticSymbol>, String> {
    if !source_root.is_dir() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    collect_nomo_files(source_root, &mut files)?;
    files.sort();

    let mut symbols = Vec::new();
    for path in files {
        let source = fs::read_to_string(&path)
            .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
        symbols.extend(public_symbols_for_text(&path, &source).map_err(|error| error.human())?);
    }
    Ok(symbols)
}

fn collect_nomo_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in
        fs::read_dir(dir).map_err(|err| format!("failed to read {}: {err}", dir.display()))?
    {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_nomo_files(&path, files)?;
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("nomo") {
            files.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{discover_project, resolve_project_dependencies};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn package_id_parses_and_formats_canonical_identity() {
        let id = PackageId::parse("fynn/utils").unwrap();

        assert_eq!(id.namespace(), "fynn");
        assert_eq!(id.name(), "utils");
        assert_eq!(id.canonical(), "fynn/utils");
        assert_eq!(id.to_string(), "fynn/utils");
        assert!(PackageId::parse("fynn/utils/extra").is_err());
    }

    #[test]
    fn builds_typed_transitive_package_graph_with_public_api() {
        let root = temp_test_root("typed-package-graph");
        reset_dir(&root);
        let app = root.join("app");
        let utils = root.join("utils");
        let core = root.join("core");
        for package in [&app, &utils, &core] {
            fs::create_dir_all(package.join("src")).unwrap();
        }

        fs::write(
            app.join("nomo.toml"),
            "[package]\nnamespace = \"fynn\"\nname = \"app\"\nversion = \"0.3.0\"\nedition = \"2026\"\n\n[dependencies]\nlocal_utils = { package = \"fynn/utils\", path = \"../utils\" }\nutils_again = { package = \"fynn/utils\", path = \"../utils\" }\n",
        )
        .unwrap();
        fs::write(
            utils.join("nomo.toml"),
            "[package]\nnamespace = \"fynn\"\nname = \"utils\"\nversion = \"1.2.0\"\nedition = \"2026\"\n\n[dependencies]\ncore = { package = \"fynn/core\", path = \"../core\" }\n",
        )
        .unwrap();
        fs::write(
            core.join("nomo.toml"),
            "[package]\nnamespace = \"fynn\"\nname = \"core\"\nversion = \"2.0.0\"\nedition = \"2026\"\n",
        )
        .unwrap();
        fs::write(
            app.join("src/main.nomo"),
            "package app.main\n\npub fn root_api() -> i64 {\n    return 0\n}\n",
        )
        .unwrap();
        fs::write(
            utils.join("src/main.nomo"),
            "package utils.main\n\n/// Exported utility.\npub fn exposed() -> i64 {\n    return 1\n}\n\nfn hidden() -> i64 {\n    return 2\n}\n",
        )
        .unwrap();
        fs::write(
            core.join("src/main.nomo"),
            "package core.main\n\npub struct Value {\n    pub raw: i64\n    secret: i64\n}\n",
        )
        .unwrap();

        let project = discover_project(&app).unwrap();
        let graph = project_package_graph(&project).unwrap();

        assert_eq!(graph.root().canonical(), "fynn/app");
        assert_eq!(graph.package_count(), 3);
        assert_eq!(
            graph
                .topological_order()
                .iter()
                .map(PackageId::canonical)
                .collect::<Vec<_>>(),
            vec!["fynn/core", "fynn/utils", "fynn/app"]
        );

        let app_id = PackageId::parse("fynn/app").unwrap();
        assert_eq!(
            graph
                .dependencies(&app_id)
                .map(|dependency| (dependency.alias.as_str(), dependency.package.canonical()))
                .collect::<Vec<_>>(),
            vec![
                ("local_utils", "fynn/utils".to_string()),
                ("utils_again", "fynn/utils".to_string())
            ]
        );
        let utils_node = graph.package_by_id("fynn/utils").unwrap();
        assert_eq!(utils_node.version.as_deref(), Some("1.2.0"));
        assert_eq!(
            utils_node.source,
            PackageSource::Path {
                path: "../utils".to_string()
            }
        );
        assert_eq!(
            utils_node
                .public_api
                .iter()
                .map(|symbol| symbol.name.as_str())
                .collect::<Vec<_>>(),
            vec!["exposed"]
        );
        assert_eq!(utils_node.public_api[0].docs, "Exported utility.");

        let core_node = graph.package_by_id("fynn/core").unwrap();
        assert_eq!(core_node.version.as_deref(), Some("2.0.0"));
        assert_eq!(
            core_node.source,
            PackageSource::Path {
                path: "../core".to_string()
            }
        );
        assert_eq!(
            core_node
                .public_api
                .iter()
                .map(|symbol| symbol.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Value", "raw"]
        );

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn represents_registry_leaf_without_local_source() {
        let root = temp_test_root("registry-leaf-package-graph");
        reset_dir(&root);
        let app = root.join("app");
        fs::create_dir_all(app.join("src")).unwrap();
        fs::write(
            app.join("nomo.toml"),
            "[package]\nnamespace = \"fynn\"\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\njson = { package = \"nomo-lang/json\", version = \"0.8.0\" }\n",
        )
        .unwrap();
        fs::write(app.join("src/main.nomo"), "package app.main\n").unwrap();

        let project = discover_project(&app).unwrap();
        let graph = project_package_graph(&project).unwrap();
        let json = graph.package_by_id("nomo-lang/json").unwrap();

        assert_eq!(json.version.as_deref(), Some("0.8.0"));
        assert_eq!(
            json.source,
            PackageSource::Registry {
                version: "0.8.0".to_string(),
                registry: None
            }
        );
        assert!(!json.has_source());
        assert!(json.public_api.is_empty());

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn locked_package_graph_uses_the_resolved_lockfile() {
        let root = temp_test_root("locked-package-graph");
        reset_dir(&root);
        let app = root.join("app");
        let utils = root.join("utils");
        let core = root.join("core");
        fs::create_dir_all(app.join("src")).unwrap();
        fs::create_dir_all(utils.join("src")).unwrap();
        fs::create_dir_all(core.join("src")).unwrap();
        fs::write(
            app.join("nomo.toml"),
            "[package]\nnamespace = \"fynn\"\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[dependencies]\nutils = { package = \"fynn/utils\", path = \"../utils\" }\n",
        )
        .unwrap();
        fs::write(
            utils.join("nomo.toml"),
            "[package]\nnamespace = \"fynn\"\nname = \"utils\"\nversion = \"0.4.0\"\nedition = \"2026\"\n\n[dependencies]\ncore = { package = \"fynn/core\", path = \"../core\" }\n",
        )
        .unwrap();
        fs::write(
            core.join("nomo.toml"),
            "[package]\nnamespace = \"fynn\"\nname = \"core\"\nversion = \"0.2.0\"\nedition = \"2026\"\n",
        )
        .unwrap();
        fs::write(app.join("src/main.nomo"), "package app.main\n").unwrap();
        fs::write(
            utils.join("src/main.nomo"),
            "package utils.main\n\npub fn value() -> i64 {\n    return 4\n}\n",
        )
        .unwrap();
        fs::write(core.join("src/main.nomo"), "package core.main\n").unwrap();

        let project = discover_project(&app).unwrap();
        resolve_project_dependencies(&project).unwrap();
        let graph = project_package_graph_with_options(
            &project,
            DependencyResolutionOptions {
                locked: true,
                offline: false,
            },
        )
        .unwrap();

        let utils = graph.package_by_id("fynn/utils").unwrap();
        assert_eq!(utils.version.as_deref(), Some("0.4.0"));
        assert_eq!(utils.public_api[0].name, "value");
        let core = graph.package_by_id("fynn/core").unwrap();
        assert_eq!(core.version.as_deref(), Some("0.2.0"));
        assert_eq!(
            core.source,
            PackageSource::Path {
                path: "../core".to_string()
            }
        );

        fs::remove_dir_all(&root).unwrap();
    }

    fn temp_test_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("nomo-{name}-{}-{nonce}", std::process::id()))
    }

    fn reset_dir(path: &Path) {
        if path.exists() {
            fs::remove_dir_all(path).unwrap();
        }
        fs::create_dir_all(path).unwrap();
    }
}

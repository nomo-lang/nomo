use super::{
    DependencyResolutionOptions, PackageDependency, PackageGraph, PackageId, PackageNode, Project,
    discover_project, project_package_graph_for_target_with_options,
};
use nomo_graph::DirectedGraph;
use nomo_manifest::{
    Dependency, DependencySource, ManifestSchema, WorkspaceContext,
    manifest_document_has_workspace, manifest_schema, parse_manifest_at_root,
    parse_manifest_document, parse_workspace_context,
};
use nomo_target::TargetTriple;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct WorkspaceMember {
    pub id: PackageId,
    pub version: String,
    pub project: Project,
    pub dependencies: Vec<Dependency>,
}

#[derive(Debug, Clone)]
pub struct WorkspaceGraph {
    pub root: PathBuf,
    pub members: Vec<Project>,
    pub default_members: Vec<Project>,
    lockfile: PathBuf,
    resolver: Option<String>,
    workspace_dependencies: BTreeMap<String, Dependency>,
    member_nodes: BTreeMap<PackageId, WorkspaceMember>,
    default_member_ids: BTreeSet<PackageId>,
    member_edges: BTreeMap<PackageId, BTreeSet<PackageDependency>>,
    member_graph: DirectedGraph<PackageId>,
    package_graphs: BTreeMap<PackageId, PackageGraph>,
    resolved_packages: BTreeMap<PackageId, PackageNode>,
    package_graphs_resolved: bool,
    target: TargetTriple,
}

impl WorkspaceGraph {
    pub fn lockfile_path(&self) -> &Path {
        &self.lockfile
    }

    pub fn resolver(&self) -> Option<&str> {
        self.resolver.as_deref()
    }

    pub fn workspace_dependency(&self, alias: &str) -> Option<&Dependency> {
        self.workspace_dependencies.get(alias)
    }

    pub fn workspace_dependencies(&self) -> impl ExactSizeIterator<Item = &Dependency> {
        self.workspace_dependencies.values()
    }

    pub fn member(&self, id: &PackageId) -> Option<&WorkspaceMember> {
        self.member_nodes.get(id)
    }

    pub fn member_by_id(&self, id: &str) -> Option<&WorkspaceMember> {
        PackageId::parse(id).ok().and_then(|id| self.member(&id))
    }

    pub fn member_nodes(&self) -> impl ExactSizeIterator<Item = &WorkspaceMember> {
        self.member_nodes.values()
    }

    pub fn member_count(&self) -> usize {
        self.member_nodes.len()
    }

    pub fn default_member_nodes(&self) -> impl Iterator<Item = &WorkspaceMember> {
        self.default_member_ids
            .iter()
            .filter_map(|id| self.member_nodes.get(id))
    }

    pub fn is_default_member(&self, id: &PackageId) -> bool {
        self.default_member_ids.contains(id)
    }

    pub fn member_dependencies<'a>(
        &'a self,
        id: &PackageId,
    ) -> impl Iterator<Item = &'a PackageDependency> {
        self.member_edges.get(id).into_iter().flatten()
    }

    pub fn topological_order(&self) -> Vec<PackageId> {
        self.member_graph
            .topological_sort()
            .expect("validated workspace member graph must remain acyclic")
    }

    pub fn package_graph(&self, id: &PackageId) -> Option<&PackageGraph> {
        self.package_graphs.get(id)
    }

    pub fn package_graphs(&self) -> impl ExactSizeIterator<Item = &PackageGraph> {
        self.package_graphs.values()
    }

    pub fn resolved_package(&self, id: &PackageId) -> Option<&PackageNode> {
        self.resolved_packages.get(id)
    }

    pub fn resolved_packages(&self) -> impl ExactSizeIterator<Item = &PackageNode> {
        self.resolved_packages.values()
    }

    pub fn resolved_package_count(&self) -> usize {
        self.resolved_packages.len()
    }

    pub fn has_resolved_package_graphs(&self) -> bool {
        self.package_graphs_resolved
    }

    pub fn resolve_package_graphs_with_options(
        &mut self,
        options: DependencyResolutionOptions,
    ) -> Result<(), String> {
        let mut package_graphs = BTreeMap::new();
        for member in self.member_nodes.values() {
            let package_graph = project_package_graph_for_target_with_options(
                &member.project,
                options,
                &self.target,
            )?;
            if package_graph.root() != &member.id {
                return Err(format!(
                    "workspace member `{}` produced package graph rooted at `{}`",
                    member.id,
                    package_graph.root()
                ));
            }
            package_graphs.insert(member.id.clone(), package_graph);
        }
        let resolved_packages = aggregate_resolved_packages(&self.member_nodes, &package_graphs)?;
        self.package_graphs = package_graphs;
        self.resolved_packages = resolved_packages;
        self.package_graphs_resolved = true;
        Ok(())
    }
}

pub fn discover_workspace(path: &Path) -> Result<WorkspaceGraph, String> {
    let target = TargetTriple::host()?;
    discover_workspace_for_target(path, &target)
}

pub fn discover_workspace_for_target(
    path: &Path,
    target: &TargetTriple,
) -> Result<WorkspaceGraph, String> {
    let source_file = path.extension().and_then(|ext| ext.to_str()) == Some("nomo");
    let search_root = if source_file {
        path.parent()
            .ok_or_else(|| format!("source file has no parent: {}", path.display()))?
    } else {
        path
    };
    let root = find_workspace_root(search_root)
        .ok_or_else(|| format!("could not find workspace nomo.toml for {}", path.display()))?;
    let text = fs::read_to_string(root.join("nomo.toml")).map_err(|err| err.to_string())?;
    let document = parse_manifest_document(&text)?;
    let context = parse_workspace_context(&root, &document)?;
    let members = workspace_projects_from_patterns(&context, &context.members)?;
    let default_members = if context.default_members.is_empty() {
        Vec::new()
    } else {
        workspace_projects_from_patterns(&context, &context.default_members)?
    };
    if !context.default_members.is_empty() && default_members.is_empty() {
        return Err("workspace default-members did not select any included package".to_string());
    }
    let member_model = build_workspace_member_model(&members, &default_members, target)?;
    Ok(WorkspaceGraph {
        lockfile: root.join("nomo.lock"),
        root,
        members,
        default_members,
        resolver: context.resolver,
        workspace_dependencies: context.dependencies,
        member_nodes: member_model.nodes,
        default_member_ids: member_model.default_ids,
        member_edges: member_model.edges,
        member_graph: member_model.graph,
        package_graphs: BTreeMap::new(),
        resolved_packages: BTreeMap::new(),
        package_graphs_resolved: false,
        target: *target,
    })
}

pub fn build_workspace_graph(path: &Path) -> Result<WorkspaceGraph, String> {
    build_workspace_graph_with_options(path, DependencyResolutionOptions::default())
}

pub fn build_workspace_graph_with_options(
    path: &Path,
    options: DependencyResolutionOptions,
) -> Result<WorkspaceGraph, String> {
    let mut workspace = discover_workspace(path)?;
    workspace.resolve_package_graphs_with_options(options)?;
    Ok(workspace)
}

pub fn build_workspace_graph_for_target_with_options(
    path: &Path,
    options: DependencyResolutionOptions,
    target: &TargetTriple,
) -> Result<WorkspaceGraph, String> {
    let mut workspace = discover_workspace_for_target(path, target)?;
    workspace.resolve_package_graphs_with_options(options)?;
    Ok(workspace)
}

struct WorkspaceMemberModel {
    nodes: BTreeMap<PackageId, WorkspaceMember>,
    default_ids: BTreeSet<PackageId>,
    edges: BTreeMap<PackageId, BTreeSet<PackageDependency>>,
    graph: DirectedGraph<PackageId>,
}

fn aggregate_resolved_packages(
    members: &BTreeMap<PackageId, WorkspaceMember>,
    package_graphs: &BTreeMap<PackageId, PackageGraph>,
) -> Result<BTreeMap<PackageId, PackageNode>, String> {
    let mut packages = BTreeMap::new();

    for (member_id, package_graph) in package_graphs {
        let root = package_graph
            .package(member_id)
            .expect("member package graph must contain its root");
        packages.insert(member_id.clone(), root.clone());
    }

    for package_graph in package_graphs.values() {
        for package in package_graph.packages() {
            if members.contains_key(&package.id) {
                continue;
            }
            match packages.get(&package.id) {
                Some(existing) if existing != package => {
                    return Err(format!(
                        "workspace package `{}` resolved with conflicting versions or sources",
                        package.id
                    ));
                }
                Some(_) => {}
                None => {
                    packages.insert(package.id.clone(), package.clone());
                }
            }
        }
    }
    Ok(packages)
}

fn build_workspace_member_model(
    members: &[Project],
    default_members: &[Project],
    target: &TargetTriple,
) -> Result<WorkspaceMemberModel, String> {
    let mut nodes = BTreeMap::new();
    let mut roots = BTreeMap::new();
    let mut graph = DirectedGraph::new();
    let mut edges = BTreeMap::new();

    for project in members {
        let manifest = parse_manifest_at_root(&project.root)?;
        let id = PackageId::new(&manifest.package.namespace, &manifest.package.name);
        if let Some(existing) = nodes.get(&id) {
            let existing: &WorkspaceMember = existing;
            return Err(format!(
                "workspace contains duplicate package `{id}` at {} and {}",
                existing.project.root.display(),
                project.root.display()
            ));
        }
        let canonical_root = fs::canonicalize(&project.root).map_err(|err| {
            format!(
                "failed to resolve workspace member {}: {err}",
                project.root.display()
            )
        })?;
        roots.insert(canonical_root, id.clone());
        graph.add_node(id.clone());
        edges.insert(id.clone(), BTreeSet::new());
        nodes.insert(
            id.clone(),
            WorkspaceMember {
                id,
                version: manifest.package.version,
                project: project.clone(),
                dependencies: manifest.dependencies,
            },
        );
    }

    for member in nodes.values() {
        for dependency in member
            .dependencies
            .iter()
            .filter(|dependency| dependency.target.matches(target))
        {
            let DependencySource::Path { path } = &dependency.source else {
                continue;
            };
            let Ok(dependency_root) = fs::canonicalize(member.project.root.join(path)) else {
                continue;
            };
            let Some(dependency_id) = roots.get(&dependency_root) else {
                continue;
            };
            if dependency.package != dependency_id.canonical() {
                return Err(format!(
                    "workspace member dependency `{}` expected package `{}`, found `{dependency_id}`",
                    dependency.alias, dependency.package
                ));
            }
            edges
                .get_mut(&member.id)
                .expect("workspace member edge set must exist")
                .insert(PackageDependency {
                    alias: dependency.alias.clone(),
                    package: dependency_id.clone(),
                });
            graph.add_edge(member.id.clone(), dependency_id.clone());
        }
    }
    if let Some(cycle) = graph.find_cycle() {
        return Err(format!("cyclic workspace member dependency: {cycle}"));
    }

    let mut default_ids = BTreeSet::new();
    for project in default_members {
        let root = fs::canonicalize(&project.root).map_err(|err| err.to_string())?;
        let Some(id) = roots.get(&root) else {
            return Err(format!(
                "workspace default member `{}` is not included in workspace members",
                project.root.display()
            ));
        };
        default_ids.insert(id.clone());
    }

    Ok(WorkspaceMemberModel {
        nodes,
        default_ids,
        edges,
        graph,
    })
}

fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    for candidate in start.ancestors() {
        let manifest = candidate.join("nomo.toml");
        if !manifest.is_file() {
            continue;
        }
        let text = fs::read_to_string(&manifest).ok()?;
        let document = parse_manifest_document(&text).ok()?;
        if manifest_document_has_workspace(&document).ok()? {
            return Some(candidate.to_path_buf());
        }
    }
    None
}

fn workspace_projects_from_patterns(
    context: &WorkspaceContext,
    patterns: &[String],
) -> Result<Vec<Project>, String> {
    if patterns.is_empty() {
        return Ok(Vec::new());
    }

    let mut member_roots = BTreeSet::new();
    let mut canonical_roots = BTreeMap::new();
    let canonical_workspace_root = if context.schema == ManifestSchema::V2 {
        Some(fs::canonicalize(&context.root).map_err(|err| {
            format!(
                "failed to resolve workspace root {}: {err}",
                context.root.display()
            )
        })?)
    } else {
        None
    };
    for pattern in patterns {
        let mut expanded = expand_workspace_pattern(&context.root, pattern)?;
        expanded.sort();
        if expanded.is_empty() {
            return Err(format!(
                "workspace member pattern `{pattern}` did not match any package"
            ));
        }
        for root in expanded {
            let relative = root
                .strip_prefix(&context.root)
                .unwrap_or(&root)
                .to_string_lossy()
                .replace('\\', "/");
            if workspace_path_is_excluded(&relative, &context.exclude) {
                continue;
            }
            if !root.join("nomo.toml").is_file() {
                return Err(format!(
                    "workspace member `{relative}` is missing nomo.toml"
                ));
            }
            if let Some(workspace_root) = &canonical_workspace_root {
                let canonical_root = fs::canonicalize(&root).map_err(|err| {
                    format!("failed to resolve workspace member `{relative}`: {err}")
                })?;
                if !canonical_root.starts_with(workspace_root) {
                    return Err(format!(
                        "workspace member `{relative}` resolves outside the workspace root"
                    ));
                }
                if let Some(existing) = canonical_roots.insert(canonical_root.clone(), root.clone())
                {
                    return Err(format!(
                        "workspace members {} and {} resolve to the same canonical path {}",
                        existing.display(),
                        root.display(),
                        canonical_root.display()
                    ));
                }
                let text = fs::read_to_string(root.join("nomo.toml"))
                    .map_err(|err| format!("failed to read member `{relative}`: {err}"))?;
                let document = parse_manifest_document(&text)?;
                if manifest_schema(&document)? != ManifestSchema::V2 {
                    return Err(format!(
                        "manifest v2 workspace member `{relative}` must define `manifest-version = 2`; run `nomo manifest migrate`"
                    ));
                }
                if root != context.root && manifest_document_has_workspace(&document)? {
                    return Err(format!(
                        "workspace member `{relative}` defines a nested workspace; nested workspace membership is ambiguous"
                    ));
                }
            }
            member_roots.insert(root);
        }
    }

    member_roots
        .into_iter()
        .map(|root| discover_project(&root))
        .collect()
}

fn expand_workspace_pattern(root: &Path, pattern: &str) -> Result<Vec<PathBuf>, String> {
    let normalized = pattern.replace('\\', "/");
    let parts = normalized
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>();
    let mut out = Vec::new();
    expand_workspace_pattern_parts(root, &parts, &mut out)?;
    Ok(out)
}

fn expand_workspace_pattern_parts(
    base: &Path,
    parts: &[&str],
    out: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let Some((part, rest)) = parts.split_first() else {
        if base.is_dir() {
            out.push(base.to_path_buf());
        }
        return Ok(());
    };

    if part.contains('*') {
        if !base.is_dir() {
            return Ok(());
        }
        for entry in fs::read_dir(base).map_err(|err| err.to_string())? {
            let entry = entry.map_err(|err| err.to_string())?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if wildcard_match(part, name) {
                expand_workspace_pattern_parts(&path, rest, out)?;
            }
        }
    } else {
        expand_workspace_pattern_parts(&base.join(part), rest, out)?;
    }
    Ok(())
}

fn wildcard_match(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    let parts = pattern.split('*').collect::<Vec<_>>();
    if parts.len() == 1 {
        return pattern == value;
    }
    let mut remaining = value;
    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if index == 0 {
            let Some(stripped) = remaining.strip_prefix(part) else {
                return false;
            };
            remaining = stripped;
        } else if index == parts.len() - 1 && !pattern.ends_with('*') {
            return remaining.ends_with(part);
        } else {
            let Some(pos) = remaining.find(part) else {
                return false;
            };
            remaining = &remaining[pos + part.len()..];
        }
    }
    true
}

fn workspace_path_is_excluded(relative: &str, exclude: &[String]) -> bool {
    exclude.iter().any(|pattern| {
        let pattern = pattern.trim_matches('/');
        relative == pattern || relative.starts_with(&format!("{pattern}/"))
    })
}

pub(super) fn validate_workspace_update_target(
    workspace: &WorkspaceGraph,
    target: &str,
) -> Result<(), String> {
    let mut package_ids = Vec::new();
    for project in &workspace.members {
        let manifest = parse_manifest_at_root(&project.root)?;
        if manifest
            .dependencies
            .iter()
            .any(|dependency| dependency.alias == target || dependency.package == target)
        {
            return Ok(());
        }
        package_ids.push(format!(
            "{}/{}",
            manifest.package.namespace, manifest.package.name
        ));
    }
    Err(format!(
        "dependency update target `{target}` is not a direct dependency of workspace members: {}",
        package_ids.join(", ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn manifest_v2_workspace_rejects_legacy_and_nested_members() {
        let root = temp_test_root("workspace-v2-member-contract");
        reset_dir(&root);
        let app = root.join("apps/app");
        fs::create_dir_all(app.join("src")).unwrap();
        fs::write(
            root.join("nomo.toml"),
            "manifest-version = 2\n\n[workspace]\nmembers = [\"apps/*\"]\n",
        )
        .unwrap();
        fs::write(
            app.join("nomo.toml"),
            "[package]\nnamespace = \"acme\"\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2026\"\n",
        )
        .unwrap();
        fs::write(app.join("src/main.nomo"), "package app.main\n").unwrap();

        let legacy = discover_workspace(&root).unwrap_err();
        assert!(legacy.contains("manifest-version = 2"), "{legacy}");

        fs::write(
            app.join("nomo.toml"),
            "manifest-version = 2\n\n[package]\nnamespace = \"acme\"\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2026\"\n\n[workspace]\nmembers = [\".\"]\n",
        )
        .unwrap();
        let nested = discover_workspace(&root).unwrap_err();
        assert!(nested.contains("nested workspace"), "{nested}");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn builds_workspace_member_topology_and_resolved_package_graphs() {
        let root = temp_test_root("workspace-graph");
        reset_dir(&root);
        let app = root.join("apps/cli");
        let core = root.join("packages/core");
        let util = root.join("packages/util");
        for package in [&app, &core, &util] {
            fs::create_dir_all(package.join("src")).unwrap();
        }
        fs::write(
            root.join("nomo.toml"),
            "[workspace]\nmembers = [\"apps/*\", \"packages/*\"]\ndefault-members = [\"apps/cli\"]\nresolver = \"2\"\n\n[workspace.package]\nnamespace = \"fynn\"\nedition = \"2026\"\n\n[workspace.dependencies]\ncore = { package = \"fynn/core\", path = \"packages/core\" }\njson = { package = \"nomo-lang/json\", version = \"0.8.0\" }\n",
        )
        .unwrap();
        fs::write(
            app.join("nomo.toml"),
            "[package]\nname = \"cli\"\nversion = \"0.1.0\"\nnamespace.workspace = true\nedition.workspace = true\n\n[dependencies]\ncore.workspace = true\njson.workspace = true\n",
        )
        .unwrap();
        fs::write(
            core.join("nomo.toml"),
            "[package]\nname = \"core\"\nversion = \"0.2.0\"\nnamespace.workspace = true\nedition.workspace = true\n\n[dependencies]\nutil = { package = \"fynn/util\", path = \"../util\" }\n",
        )
        .unwrap();
        fs::write(
            util.join("nomo.toml"),
            "[package]\nname = \"util\"\nversion = \"0.3.0\"\nnamespace.workspace = true\nedition.workspace = true\n",
        )
        .unwrap();
        fs::write(app.join("src/main.nomo"), "package app.main\n").unwrap();
        fs::write(
            core.join("src/main.nomo"),
            "package core.main\n\n/// Adds values.\npub fn add(a: i64, b: i64) -> i64 {\n    return a + b\n}\n",
        )
        .unwrap();
        fs::write(
            util.join("src/main.nomo"),
            "package util.main\n\npub fn identity(value: i64) -> i64 {\n    return value\n}\n",
        )
        .unwrap();

        let discovered = discover_workspace(&app).unwrap();

        assert_eq!(discovered.member_count(), 3);
        assert_eq!(discovered.lockfile_path(), root.join("nomo.lock"));
        assert_eq!(discovered.resolver(), Some("2"));
        assert_eq!(
            discovered
                .workspace_dependencies()
                .map(|dependency| dependency.alias.as_str())
                .collect::<Vec<_>>(),
            vec!["core", "json"]
        );
        assert_eq!(
            discovered
                .topological_order()
                .iter()
                .map(PackageId::canonical)
                .collect::<Vec<_>>(),
            vec!["fynn/util", "fynn/core", "fynn/cli"]
        );
        let cli_id = PackageId::parse("fynn/cli").unwrap();
        assert!(discovered.is_default_member(&cli_id));
        assert_eq!(
            discovered
                .default_member_nodes()
                .map(|member| member.id.canonical())
                .collect::<Vec<_>>(),
            vec!["fynn/cli"]
        );
        assert_eq!(
            discovered
                .member_dependencies(&cli_id)
                .map(|dependency| (dependency.alias.as_str(), dependency.package.canonical()))
                .collect::<Vec<_>>(),
            vec![("core", "fynn/core".to_string())]
        );
        assert!(!discovered.has_resolved_package_graphs());
        assert_eq!(discovered.package_graphs().len(), 0);

        let resolved = build_workspace_graph(&root).unwrap();

        assert!(resolved.has_resolved_package_graphs());
        assert_eq!(resolved.package_graphs().len(), 3);
        assert_eq!(resolved.resolved_package_count(), 4);
        assert_eq!(
            resolved
                .resolved_packages()
                .map(|package| package.id.canonical())
                .collect::<Vec<_>>(),
            vec!["fynn/cli", "fynn/core", "fynn/util", "nomo-lang/json"]
        );
        let cli_graph = resolved.package_graph(&cli_id).unwrap();
        assert_eq!(cli_graph.root(), &cli_id);
        assert_eq!(cli_graph.package_count(), 4);
        let core_node = cli_graph.package_by_id("fynn/core").unwrap();
        assert_eq!(core_node.version.as_deref(), Some("0.2.0"));
        assert_eq!(core_node.public_api[0].name, "add");
        assert_eq!(core_node.public_api[0].docs, "Adds values.");

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn rejects_conflicting_external_packages_across_members() {
        let root = temp_test_root("workspace-package-conflict");
        reset_dir(&root);
        let first = root.join("packages/first");
        let second = root.join("packages/second");
        for package in [&first, &second] {
            fs::create_dir_all(package.join("src")).unwrap();
            fs::write(package.join("src/main.nomo"), "package app.main\n").unwrap();
        }
        fs::write(
            root.join("nomo.toml"),
            "[workspace]\nmembers = [\"packages/*\"]\n",
        )
        .unwrap();
        fs::write(
            first.join("nomo.toml"),
            "[package]\nnamespace = \"fynn\"\nname = \"first\"\nversion = \"0.1.0\"\n\n[dependencies]\njson = { package = \"nomo-lang/json\", version = \"0.8.0\" }\n",
        )
        .unwrap();
        fs::write(
            second.join("nomo.toml"),
            "[package]\nnamespace = \"fynn\"\nname = \"second\"\nversion = \"0.1.0\"\n\n[dependencies]\njson = { package = \"nomo-lang/json\", version = \"0.9.0\" }\n",
        )
        .unwrap();

        let discovered = discover_workspace(&root).unwrap();
        assert!(!discovered.has_resolved_package_graphs());

        let error = build_workspace_graph(&root).unwrap_err();

        assert_eq!(
            error,
            "workspace package `nomo-lang/json` resolved with conflicting versions or sources"
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn rejects_workspace_member_path_cycle() {
        let root = temp_test_root("workspace-member-cycle");
        reset_dir(&root);
        let first = root.join("packages/first");
        let second = root.join("packages/second");
        for package in [&first, &second] {
            fs::create_dir_all(package.join("src")).unwrap();
            fs::write(package.join("src/main.nomo"), "package app.main\n").unwrap();
        }
        fs::write(
            root.join("nomo.toml"),
            "[workspace]\nmembers = [\"packages/*\"]\n",
        )
        .unwrap();
        fs::write(
            first.join("nomo.toml"),
            "[package]\nnamespace = \"fynn\"\nname = \"first\"\nversion = \"0.1.0\"\n\n[dependencies]\nsecond = { package = \"fynn/second\", path = \"../second\" }\n",
        )
        .unwrap();
        fs::write(
            second.join("nomo.toml"),
            "[package]\nnamespace = \"fynn\"\nname = \"second\"\nversion = \"0.1.0\"\n\n[dependencies]\nfirst = { package = \"fynn/first\", path = \"../first\" }\n",
        )
        .unwrap();

        let error = discover_workspace(&root).unwrap_err();

        assert_eq!(
            error,
            "cyclic workspace member dependency: fynn/first -> fynn/second -> fynn/first"
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn rejects_duplicate_workspace_package_identity() {
        let root = temp_test_root("workspace-duplicate-package");
        reset_dir(&root);
        let first = root.join("packages/first");
        let second = root.join("packages/second");
        for package in [&first, &second] {
            fs::create_dir_all(package.join("src")).unwrap();
            fs::write(package.join("src/main.nomo"), "package app.main\n").unwrap();
            fs::write(
                package.join("nomo.toml"),
                "[package]\nnamespace = \"fynn\"\nname = \"shared\"\nversion = \"0.1.0\"\n",
            )
            .unwrap();
        }
        fs::write(
            root.join("nomo.toml"),
            "[workspace]\nmembers = [\"packages/*\"]\n",
        )
        .unwrap();

        let error = discover_workspace(&root).unwrap_err();

        assert!(
            error.contains("workspace contains duplicate package `fynn/shared`"),
            "{error}"
        );
        assert!(error.contains(&first.display().to_string()), "{error}");
        assert!(error.contains(&second.display().to_string()), "{error}");
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

use nomo_graph::DirectedGraph;
use nomo_manifest::{
    Dependency, DependencySource, PackageMetadata, validate_dependency_alias, validate_package_id,
    validate_version_like,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyGraph {
    pub root: PackageMetadata,
    pub dependencies: Vec<ResolvedDependency>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedDependency {
    pub alias: String,
    pub package: String,
    pub source: DependencySource,
    pub checksum: Option<String>,
    pub dependencies: Vec<ResolvedDependency>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedLockfileRoot {
    pub dependencies: Vec<ResolvedDependency>,
    pub has_workspace_roots: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyEdge {
    pub alias: String,
    pub package: String,
}

pub fn render_lockfile(graph: &DependencyGraph) -> String {
    let document = LockfileDocument {
        root: Vec::new(),
        package: flatten_dependencies(&graph.dependencies)
            .into_iter()
            .map(LockPackage::from_resolved)
            .collect(),
    };
    render_lockfile_document(&document)
}

pub fn render_workspace_lockfile(graphs: &[DependencyGraph]) -> Result<String, String> {
    let mut root_ids = BTreeSet::new();
    let mut packages = BTreeMap::new();
    let mut package_sources = BTreeMap::new();
    let mut roots = Vec::new();

    for graph in graphs {
        let root_id = package_id(&graph.root);
        if !root_ids.insert(root_id.clone()) {
            return Err(format!(
                "workspace lockfile has duplicate root package `{root_id}`"
            ));
        }
        roots.push(LockRoot::from_graph(graph));
        for dependency in flatten_dependencies(&graph.dependencies) {
            remember_package_source(
                &mut package_sources,
                &dependency.package,
                &dependency.source,
            )?;
            let package = LockPackage::from_resolved(dependency);
            let key = (package.alias.clone(), package.id.clone());
            match packages.get(&key) {
                Some(existing) if existing != &package => {
                    return Err(format!(
                        "workspace lockfile has conflicting entries for `{} -> {}`",
                        package.alias, package.id
                    ));
                }
                Some(_) => {}
                None => {
                    packages.insert(key, package);
                }
            }
        }
    }

    let document = LockfileDocument {
        root: roots,
        package: packages.into_values().collect(),
    };
    Ok(render_lockfile_document(&document))
}

pub fn parse_lockfile_root(lockfile: &str, root_id: &str) -> Result<ParsedLockfileRoot, String> {
    let document = parse_lockfile_document(lockfile)?;
    let root_edges = document
        .root
        .iter()
        .find(|root| root.id == root_id)
        .map(LockRoot::dependency_edges)
        .transpose()?;
    let has_workspace_roots = !document.root.is_empty();
    let packages = document
        .package
        .into_iter()
        .map(LockPackage::into_resolved)
        .collect::<Result<Vec<_>, _>>()?;
    validate_locked_dependency_graph(&packages)?;
    let dependencies = match root_edges {
        Some(edges) => build_locked_dependencies_from_edges(&edges, &packages)?,
        None if has_workspace_roots => {
            return Err(format!(
                "nomo.lock does not contain workspace root `{root_id}`"
            ));
        }
        None => {
            let referenced_packages = packages
                .iter()
                .flat_map(|package| {
                    package
                        .dependencies
                        .iter()
                        .map(|dependency| dependency.package.clone())
                })
                .collect::<BTreeSet<_>>();
            packages
                .iter()
                .filter(|package| !referenced_packages.contains(&package.package))
                .map(|package| build_locked_dependency(package, &packages))
                .collect::<Result<Vec<_>, _>>()?
        }
    };
    Ok(ParsedLockfileRoot {
        dependencies,
        has_workspace_roots,
    })
}

pub fn parse_lockfile_text(lockfile: &str) -> Result<Vec<ResolvedDependency>, String> {
    parse_lockfile_document(lockfile)?
        .package
        .into_iter()
        .map(LockPackage::into_resolved)
        .collect()
}

pub fn validate_locked_source_matches_manifest(
    manifest: &Dependency,
    locked: &ResolvedDependency,
) -> Result<(), String> {
    match (&manifest.source, &locked.source) {
        (
            DependencySource::Registry { version, registry },
            DependencySource::Registry {
                version: locked_version,
                registry: locked_registry,
            },
        ) if version == locked_version && registry == locked_registry => Ok(()),
        (DependencySource::Path { .. }, DependencySource::Path { .. }) => Ok(()),
        (
            DependencySource::Git {
                git,
                branch,
                tag,
                rev,
            },
            DependencySource::Git {
                git: locked_git,
                branch: locked_branch,
                tag: locked_tag,
                rev: locked_rev,
            },
        ) if git == locked_git
            && branch == locked_branch
            && tag == locked_tag
            && rev
                .as_ref()
                .is_none_or(|rev| Some(rev) == locked_rev.as_ref()) =>
        {
            Ok(())
        }
        _ => Err(format!(
            "nomo.lock is out of date: dependency `{}` source no longer matches manifest",
            manifest.alias
        )),
    }
}

pub fn lock_source_string(dependency: &ResolvedDependency) -> String {
    LockPackage::from_resolved(dependency).source
}

pub fn flatten_dependencies(dependencies: &[ResolvedDependency]) -> Vec<&ResolvedDependency> {
    let mut flattened = Vec::new();
    for dependency in dependencies {
        flattened.push(dependency);
        flattened.extend(flatten_dependencies(&dependency.dependencies));
    }
    flattened
}

fn render_lockfile_document(document: &LockfileDocument) -> String {
    let mut out = String::from("# This file is generated by `nomo deps resolve`.\n\n");
    out.push_str(&toml::to_string(document).expect("lockfile document should serialize"));
    out
}

fn parse_lockfile_document(lockfile: &str) -> Result<LockfileDocument, String> {
    toml::from_str(lockfile).map_err(|err| format!("failed to parse nomo.lock as TOML: {err}"))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct LockfileDocument {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    root: Vec<LockRoot>,
    #[serde(default)]
    package: Vec<LockPackage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct LockRoot {
    id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    dependencies: Vec<String>,
}

impl LockRoot {
    fn from_graph(graph: &DependencyGraph) -> Self {
        Self {
            id: package_id(&graph.root),
            dependencies: graph
                .dependencies
                .iter()
                .map(|dependency| format!("{} -> {}", dependency.alias, dependency.package))
                .collect(),
        }
    }

    fn dependency_edges(&self) -> Result<Vec<DependencyEdge>, String> {
        validate_package_id(&self.id)?;
        self.dependencies
            .iter()
            .map(|entry| parse_lock_dependency_entry(entry))
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct LockPackage {
    id: String,
    alias: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rev: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    checksum: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    dependencies: Vec<String>,
}

impl LockPackage {
    fn from_resolved(dependency: &ResolvedDependency) -> Self {
        let (version, source, branch, tag, rev) = match &dependency.source {
            DependencySource::Registry { version, registry } => {
                let registry_source = registry.as_deref().unwrap_or(&dependency.package);
                (
                    Some(version.clone()),
                    format!("registry+{registry_source}"),
                    None,
                    None,
                    None,
                )
            }
            DependencySource::Path { path } => (None, format!("path+{path}"), None, None, None),
            DependencySource::Git {
                git,
                branch,
                tag,
                rev,
            } => (
                None,
                format!("git+{git}"),
                branch.clone(),
                tag.clone(),
                rev.clone(),
            ),
        };
        Self {
            id: dependency.package.clone(),
            alias: dependency.alias.clone(),
            version,
            source,
            branch,
            tag,
            rev,
            checksum: dependency.checksum.clone(),
            dependencies: dependency
                .dependencies
                .iter()
                .map(|child| format!("{} -> {}", child.alias, child.package))
                .collect(),
        }
    }

    fn into_resolved(self) -> Result<ResolvedDependency, String> {
        validate_package_id(&self.id)?;
        validate_dependency_alias(&self.alias)?;
        let source = parse_lock_source(
            &self.id,
            &self.source,
            self.version,
            self.branch,
            self.tag,
            self.rev,
        )?;
        if let Some(checksum) = self.checksum.as_deref() {
            validate_checksum(&self.id, checksum)?;
        }
        let dependencies = self
            .dependencies
            .into_iter()
            .map(|entry| parse_lock_dependency_entry(&entry))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|edge| ResolvedDependency {
                alias: edge.alias,
                package: edge.package,
                source: DependencySource::Registry {
                    version: "locked".to_string(),
                    registry: None,
                },
                checksum: None,
                dependencies: Vec::new(),
            })
            .collect();
        Ok(ResolvedDependency {
            alias: self.alias,
            package: self.id,
            source,
            checksum: self.checksum,
            dependencies,
        })
    }
}

fn build_locked_dependency(
    dependency: &ResolvedDependency,
    packages: &[ResolvedDependency],
) -> Result<ResolvedDependency, String> {
    let mut children = Vec::new();
    for child in &dependency.dependencies {
        let locked_child = packages
            .iter()
            .find(|package| package.package == child.package && package.alias == child.alias)
            .or_else(|| {
                packages
                    .iter()
                    .find(|package| package.package == child.package)
            })
            .ok_or_else(|| {
                format!(
                    "nomo.lock references missing dependency `{} -> {}`",
                    child.alias, child.package
                )
            })?;
        children.push(build_locked_dependency(locked_child, packages)?);
    }

    Ok(ResolvedDependency {
        alias: dependency.alias.clone(),
        package: dependency.package.clone(),
        source: dependency.source.clone(),
        checksum: dependency.checksum.clone(),
        dependencies: children,
    })
}

fn validate_locked_dependency_graph(packages: &[ResolvedDependency]) -> Result<(), String> {
    let mut graph = DirectedGraph::new();
    for package in packages {
        graph.add_node(package.package.clone());
        for dependency in &package.dependencies {
            graph.add_edge(package.package.clone(), dependency.package.clone());
        }
    }
    if let Some(cycle) = graph.find_cycle() {
        return Err(format!(
            "cyclic dependency in nomo.lock: {}",
            cycle
                .path()
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(" -> ")
        ));
    }
    Ok(())
}

fn build_locked_dependencies_from_edges(
    edges: &[DependencyEdge],
    packages: &[ResolvedDependency],
) -> Result<Vec<ResolvedDependency>, String> {
    edges
        .iter()
        .map(|edge| {
            let locked_child = packages
                .iter()
                .find(|package| package.package == edge.package && package.alias == edge.alias)
                .or_else(|| {
                    packages
                        .iter()
                        .find(|package| package.package == edge.package)
                })
                .ok_or_else(|| {
                    format!(
                        "nomo.lock references missing dependency `{} -> {}`",
                        edge.alias, edge.package
                    )
                })?;
            build_locked_dependency(locked_child, packages)
        })
        .collect()
}

fn parse_lock_dependency_entry(entry: &str) -> Result<DependencyEdge, String> {
    let Some((alias, package)) = entry.split_once(" -> ") else {
        return Err(format!(
            "lockfile dependency entry `{entry}` must use `alias -> owner/package`"
        ));
    };
    validate_dependency_alias(alias)?;
    validate_package_id(package)?;
    Ok(DependencyEdge {
        alias: alias.to_string(),
        package: package.to_string(),
    })
}

fn validate_lock_source_selectors(package: &str, source: &DependencySource) -> Result<(), String> {
    if let DependencySource::Git {
        branch, tag, rev, ..
    } = source
    {
        let selector_count = [branch, tag, rev]
            .iter()
            .filter(|selector| selector.is_some())
            .count();
        if selector_count > 2 || (selector_count == 2 && rev.is_none()) {
            return Err(format!(
                "lockfile package `{package}` git source can only combine one selector with resolved `rev`"
            ));
        }
    }
    Ok(())
}

fn validate_lock_version_shape(package: &str, source: &DependencySource) -> Result<(), String> {
    if let DependencySource::Registry { version, .. } = source {
        validate_version_like(&format!("lockfile package `{package}` version"), version)?;
    }
    Ok(())
}

fn validate_checksum(package: &str, checksum: &str) -> Result<(), String> {
    let Some(hex) = checksum.strip_prefix("sha256:") else {
        return Err(format!(
            "lockfile package `{package}` checksum must use `sha256:<hex>`"
        ));
    };
    let valid = hex.len() == 64 && hex.chars().all(|ch| ch.is_ascii_hexdigit());
    if valid {
        Ok(())
    } else {
        Err(format!(
            "lockfile package `{package}` checksum must contain 64 hexadecimal digits"
        ))
    }
}

fn parse_lock_source(
    package: &str,
    source: &str,
    version: Option<String>,
    branch: Option<String>,
    tag: Option<String>,
    rev: Option<String>,
) -> Result<DependencySource, String> {
    let source = if let Some(registry_source) = source.strip_prefix("registry+") {
        if branch.is_some() || tag.is_some() || rev.is_some() {
            return Err(format!(
                "lockfile package `{package}` registry source must not declare git selectors"
            ));
        }
        let version =
            version.ok_or_else(|| format!("registry package `{package}` is missing `version`"))?;
        let registry = if registry_source == package {
            None
        } else {
            Some(registry_source.to_string())
        };
        DependencySource::Registry { version, registry }
    } else if let Some(path) = source.strip_prefix("path+") {
        if version.is_some() {
            return Err(format!(
                "lockfile package `{package}` path source must not declare `version`"
            ));
        }
        if branch.is_some() || tag.is_some() || rev.is_some() {
            return Err(format!(
                "lockfile package `{package}` path source must not declare git selectors"
            ));
        }
        DependencySource::Path {
            path: path.to_string(),
        }
    } else if let Some(git) = source.strip_prefix("git+") {
        if version.is_some() {
            return Err(format!(
                "lockfile package `{package}` git source must not declare `version`"
            ));
        }
        DependencySource::Git {
            git: git.to_string(),
            branch,
            tag,
            rev,
        }
    } else {
        return Err(format!(
            "lockfile package `{package}` has unsupported source `{source}`"
        ));
    };
    validate_lock_version_shape(package, &source)?;
    validate_lock_source_selectors(package, &source)?;
    Ok(source)
}

fn remember_package_source(
    seen: &mut BTreeMap<String, DependencySource>,
    package: &str,
    source: &DependencySource,
) -> Result<(), String> {
    match seen.get(package) {
        Some(existing) if existing != source => Err(format!(
            "workspace lockfile has conflicting sources for package `{package}`"
        )),
        Some(_) => Ok(()),
        None => {
            seen.insert(package.to_string(), source.clone());
            Ok(())
        }
    }
}

fn package_id(package: &PackageMetadata) -> String {
    format!("{}/{}", package.namespace, package.name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lockfile_cycle_reports_full_package_path() {
        let lockfile = r#"[[root]]
id = "app/root"
dependencies = ["a -> app/a"]

[[package]]
id = "app/a"
alias = "a"
version = "1.0.0"
source = "registry+app/a"
dependencies = ["b -> app/b"]

[[package]]
id = "app/b"
alias = "b"
version = "1.0.0"
source = "registry+app/b"
dependencies = ["a -> app/a"]
"#;

        let error = parse_lockfile_root(lockfile, "app/root").unwrap_err();
        assert_eq!(
            error,
            "cyclic dependency in nomo.lock: app/a -> app/b -> app/a"
        );
    }
}

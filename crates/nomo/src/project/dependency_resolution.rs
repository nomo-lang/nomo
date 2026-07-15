use super::{
    Project,
    dependency_tree::source_description,
    git_cache::{git_head_rev, resolve_git_source, resolve_git_source_offline},
    registry_http::registry_dependency_authorization,
    vendor::locked_or_vendor_source_root,
};
use nomo_graph::DirectedGraph;
use nomo_lockfile::{
    DependencyGraph, ResolvedDependency, parse_lockfile_root,
    validate_locked_source_matches_manifest,
};
use nomo_manifest::{
    Dependency, DependencySource, Manifest, PackageVersion, RegistryTrustPolicy, VersionConstraint,
    parse_manifest_at_root, relative_path,
};
use nomo_resolver::{
    ConstraintOrigin, VersionCandidate, load_registry_version_candidates, package_checksum,
    resolve_registry_source_with_policy, select_highest_version,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Default)]
struct RegistrySolveState {
    selections: BTreeMap<String, String>,
    constraints: BTreeMap<String, Vec<ConstraintOrigin>>,
    registries: BTreeMap<String, Option<String>>,
    candidates: BTreeMap<(String, String), Vec<VersionCandidate>>,
    changed: bool,
}

impl RegistrySolveState {
    fn begin_pass(&mut self) {
        self.constraints.clear();
        self.registries.clear();
        self.changed = false;
    }

    fn select(
        &mut self,
        base_root: &Path,
        dependency: &Dependency,
        requirement_source: &str,
        registry: Option<&str>,
        offline: bool,
        authorization: Option<&str>,
        dependency_path: &[String],
    ) -> Result<String, String> {
        let requirement = VersionConstraint::parse(requirement_source)?;
        let registry_value = registry.map(str::to_string);
        match self.registries.get(&dependency.package) {
            Some(existing) if existing != &registry_value => {
                return Err(format!(
                    "package `{}` is required from conflicting registries: {} and {}",
                    dependency.package,
                    existing.as_deref().unwrap_or("default"),
                    registry.unwrap_or("default")
                ));
            }
            Some(_) => {}
            None => {
                self.registries
                    .insert(dependency.package.clone(), registry_value);
            }
        }
        let origin = ConstraintOrigin {
            requirement,
            dependency_path: dependency_path.to_vec(),
        };
        let constraints = self
            .constraints
            .entry(dependency.package.clone())
            .or_default();
        if !constraints.contains(&origin) {
            constraints.push(origin);
        }
        constraints.sort_by_key(|constraint| {
            (
                constraint.dependency_path.join(" -> "),
                constraint.requirement.normalized(),
            )
        });

        let candidates = if constraints
            .iter()
            .all(|constraint| constraint.requirement.is_exact())
        {
            exact_constraint_candidates(constraints)
        } else {
            let registry = registry.ok_or_else(|| {
                format!(
                    "registry dependency `{}` package `{}` uses a version range but no registry endpoint is configured",
                    dependency.alias, dependency.package
                )
            })?;
            let key = (dependency.package.clone(), registry.to_string());
            if !self.candidates.contains_key(&key) {
                let loaded = load_registry_version_candidates(
                    base_root,
                    &dependency.alias,
                    &dependency.package,
                    registry,
                    offline,
                    authorization,
                )?;
                self.candidates.insert(key.clone(), loaded);
            }
            self.candidates
                .get(&key)
                .expect("registry candidates were inserted")
                .clone()
        };

        if let Some(selected) = self.selections.get(&dependency.package) {
            let selected_version = PackageVersion::parse(selected)?;
            let is_available = candidates
                .iter()
                .any(|candidate| candidate.version == selected_version && !candidate.yanked);
            if is_available
                && constraints
                    .iter()
                    .all(|constraint| constraint.requirement.matches(&selected_version))
            {
                return Ok(selected.clone());
            }
        }

        let selected = select_highest_version(&dependency.package, &candidates, constraints)
            .map_err(|conflict| conflict.render())?
            .to_string();
        if self
            .selections
            .insert(dependency.package.clone(), selected.clone())
            .is_some_and(|previous| previous != selected)
        {
            self.changed = true;
        }
        Ok(selected)
    }

    fn finalize_pass(&mut self) -> Result<(), String> {
        for (package, constraints) in &self.constraints {
            let candidates = if constraints
                .iter()
                .all(|constraint| constraint.requirement.is_exact())
            {
                exact_constraint_candidates(constraints)
            } else {
                let registry = self
                    .registries
                    .get(package)
                    .and_then(|registry| registry.as_ref())
                    .expect("range constraints require a registry endpoint");
                self.candidates
                    .get(&(package.clone(), registry.clone()))
                    .expect("range candidates are loaded during traversal")
                    .clone()
            };
            let selected = select_highest_version(package, &candidates, constraints)
                .map_err(|conflict| conflict.render())?
                .to_string();
            if self.selections.get(package) != Some(&selected) {
                self.selections.insert(package.clone(), selected);
                self.changed = true;
            }
        }
        Ok(())
    }
}

fn exact_constraint_candidates(constraints: &[ConstraintOrigin]) -> Vec<VersionCandidate> {
    constraints
        .iter()
        .filter_map(|constraint| match &constraint.requirement {
            VersionConstraint::Exact(version) => Some(VersionCandidate {
                version: version.clone(),
                yanked: false,
            }),
            VersionConstraint::Range { .. } => None,
        })
        .collect()
}

pub(super) fn resolve_dependency_graph(root: &Path) -> Result<DependencyGraph, String> {
    resolve_dependency_graph_for_lock(root, None, None, false)
}

pub(super) fn resolve_dependency_graph_for_lock(
    root: &Path,
    lock_source_base: Option<&Path>,
    dependency_cache_base: Option<&Path>,
    offline: bool,
) -> Result<DependencyGraph, String> {
    let manifest = parse_manifest_at_root(root)?;
    resolve_dependency_graph_for_manifest(
        root,
        manifest,
        lock_source_base,
        dependency_cache_base,
        offline,
    )
}

pub(super) fn resolve_dependency_graph_for_manifest(
    root: &Path,
    manifest: Manifest,
    lock_source_base: Option<&Path>,
    dependency_cache_base: Option<&Path>,
    offline: bool,
) -> Result<DependencyGraph, String> {
    let mut graphs = resolve_dependency_graphs_for_manifests(
        vec![(root, manifest)],
        lock_source_base,
        dependency_cache_base,
        offline,
    )?;
    Ok(graphs
        .pop()
        .expect("one manifest produces one dependency graph"))
}

pub(super) fn resolve_dependency_graphs_for_manifests(
    manifests: Vec<(&Path, Manifest)>,
    lock_source_base: Option<&Path>,
    dependency_cache_base: Option<&Path>,
    offline: bool,
) -> Result<Vec<DependencyGraph>, String> {
    let lock_source_base = lock_source_base
        .map(fs::canonicalize)
        .transpose()
        .map_err(|err| err.to_string())?;
    let dependency_cache_base = dependency_cache_base
        .map(fs::canonicalize)
        .transpose()
        .map_err(|err| err.to_string())?;
    let manifests = manifests
        .into_iter()
        .map(|(root, manifest)| {
            let root = fs::canonicalize(root).map_err(|err| err.to_string())?;
            let root_package = format!("{}/{}", manifest.package.namespace, manifest.package.name);
            Ok((root, manifest, root_package))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut registry_solver = RegistrySolveState::default();
    let mut seen_selections = BTreeSet::new();
    for _ in 0..64 {
        registry_solver.begin_pass();
        let mut graphs = Vec::with_capacity(manifests.len());
        for (root, manifest, root_package) in &manifests {
            let mut package_sources = BTreeMap::new();
            let mut package_graph = DirectedGraph::new();
            package_graph.add_node(root_package.clone());
            let dependencies = resolve_dependencies(
                &manifest.dependencies,
                root,
                root_package,
                &mut package_graph,
                &mut package_sources,
                lock_source_base.as_deref(),
                dependency_cache_base.as_deref(),
                offline,
                std::slice::from_ref(root_package),
                &mut registry_solver,
                manifest.trust,
                &manifest.transparency_keys,
            )?;
            graphs.push(DependencyGraph {
                root: manifest.package.clone(),
                dependencies,
            });
        }
        registry_solver.finalize_pass()?;
        if !registry_solver.changed {
            return Ok(graphs);
        }
        let fingerprint = registry_solver
            .selections
            .iter()
            .map(|(package, version)| format!("{package}={version}"))
            .collect::<Vec<_>>()
            .join("\n");
        if !seen_selections.insert(fingerprint) {
            return Err(
                "registry dependency solver repeated a selection state without converging"
                    .to_string(),
            );
        }
    }
    Err("registry dependency solver did not converge after 64 passes".to_string())
}

pub(super) fn dependency_graph_from_lockfile(
    root: &Path,
    lock_root: &Path,
) -> Result<DependencyGraph, String> {
    let (graph, _) = dependency_graph_and_source_base_from_lockfile(root, lock_root)?;
    Ok(graph)
}

pub(super) fn locked_dependency_graph_and_source_base(
    project: &Project,
) -> Result<(DependencyGraph, PathBuf), String> {
    dependency_graph_and_source_base_from_lockfile(&project.root, &project.lock_root())
}

pub(super) fn validate_project_lock(project: &Project) -> Result<(), String> {
    let (graph, _) = locked_dependency_graph_and_source_base(project)?;
    validate_project_lock_direct_dependencies(project, &graph)
}

pub(super) fn validate_project_lock_direct_dependencies(
    project: &Project,
    graph: &DependencyGraph,
) -> Result<(), String> {
    let manifest = parse_manifest_at_root(&project.root)?;
    let locked_by_alias = graph
        .dependencies
        .iter()
        .map(|dependency| (dependency.alias.as_str(), dependency))
        .collect::<BTreeMap<_, _>>();
    for dependency in manifest
        .dependencies
        .iter()
        .filter(|dep| dep.alias != "std")
    {
        let locked = locked_by_alias
            .get(dependency.alias.as_str())
            .ok_or_else(|| {
                format!(
                    "nomo.lock is out of date: missing dependency `{}`",
                    dependency.alias
                )
            })?;
        if locked.package != dependency.package {
            return Err(format!(
                "nomo.lock is out of date: dependency `{}` expected package `{}`, found `{}`",
                dependency.alias, dependency.package, locked.package
            ));
        }
        validate_locked_source_matches_manifest(dependency, locked)?;
    }
    validate_locked_trust_policy(&graph.dependencies, manifest.trust)?;
    Ok(())
}

fn validate_locked_trust_policy(
    dependencies: &[ResolvedDependency],
    policy: RegistryTrustPolicy,
) -> Result<(), String> {
    for dependency in dependencies {
        if matches!(dependency.source, DependencySource::Registry { .. }) {
            match policy {
                RegistryTrustPolicy::ChecksumOnly => {}
                RegistryTrustPolicy::Signed if dependency.supply_chain.is_none() => {
                    return Err(format!(
                        "nomo.lock package `{}` has no publisher-signature evidence required by `signed` trust policy",
                        dependency.package
                    ));
                }
                RegistryTrustPolicy::SignedTransparent => {
                    let evidence = dependency.supply_chain.as_ref().ok_or_else(|| {
                        format!(
                            "nomo.lock package `{}` has no supply-chain evidence required by `signed+transparent` trust policy",
                            dependency.package
                        )
                    })?;
                    if evidence.transparency_root.is_none() || evidence.transparency_size.is_none()
                    {
                        return Err(format!(
                            "nomo.lock package `{}` has no transparency evidence required by `signed+transparent` trust policy",
                            dependency.package
                        ));
                    }
                }
                RegistryTrustPolicy::Signed => {}
            }
        }
        validate_locked_trust_policy(&dependency.dependencies, policy)?;
    }
    Ok(())
}

fn resolve_dependencies(
    dependencies: &[Dependency],
    base_root: &Path,
    current_package: &str,
    package_graph: &mut DirectedGraph<String>,
    package_sources: &mut BTreeMap<String, DependencySource>,
    lock_source_base: Option<&Path>,
    dependency_cache_base: Option<&Path>,
    offline: bool,
    current_path: &[String],
    registry_solver: &mut RegistrySolveState,
    trust_policy: RegistryTrustPolicy,
    trusted_transparency_keys: &[String],
) -> Result<Vec<ResolvedDependency>, String> {
    let mut resolved = Vec::new();
    for dependency in dependencies {
        let mut dependency_path = current_path.to_vec();
        dependency_path.push(dependency.package.clone());
        package_graph.add_edge(current_package.to_string(), dependency.package.clone());
        if let Some(cycle) = package_graph.find_cycle() {
            return Err(format!(
                "cyclic package dependency: {}",
                cycle
                    .path()
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(" -> ")
            ));
        }
        let (resolved_source, checksum, supply_chain, child_dependencies) = match &dependency.source
        {
            DependencySource::Path { path } => {
                let dep_root = fs::canonicalize(base_root.join(path)).map_err(|err| {
                    format!(
                        "failed to resolve path dependency `{}` at {}: {err}",
                        dependency.alias,
                        base_root.join(path).display()
                    )
                })?;
                let dep_manifest = parse_manifest_at_root(&dep_root)?;
                let actual_id = format!(
                    "{}/{}",
                    dep_manifest.package.namespace, dep_manifest.package.name
                );
                if actual_id != dependency.package {
                    return Err(format!(
                        "path dependency `{}` expected package `{}`, found `{}`",
                        dependency.alias, dependency.package, actual_id
                    ));
                }
                let child_dependencies = resolve_dependencies(
                    &dep_manifest.dependencies,
                    &dep_root,
                    &dependency.package,
                    package_graph,
                    package_sources,
                    lock_source_base,
                    dependency_cache_base,
                    offline,
                    &dependency_path,
                    registry_solver,
                    trust_policy,
                    trusted_transparency_keys,
                )?;
                let checksum = package_checksum(&dep_root)?;
                let resolved_source = match lock_source_base {
                    Some(lock_source_base) => DependencySource::Path {
                        path: relative_path(lock_source_base, &dep_root)
                            .unwrap_or_else(|| dep_root.clone())
                            .to_string_lossy()
                            .replace('\\', "/"),
                    },
                    None => dependency.source.clone(),
                };
                (resolved_source, Some(checksum), None, child_dependencies)
            }
            DependencySource::Git {
                git,
                branch,
                tag,
                rev,
            } => {
                let dep_root = if offline {
                    resolve_git_source_offline(
                        dependency_cache_base.unwrap_or(base_root),
                        &dependency.alias,
                        &dependency.package,
                        git,
                        branch.as_deref(),
                        tag.as_deref(),
                        rev.as_deref(),
                    )?
                } else {
                    resolve_git_source(
                        dependency_cache_base.unwrap_or(base_root),
                        &dependency.alias,
                        &dependency.package,
                        git,
                        branch.as_deref(),
                        tag.as_deref(),
                        rev.as_deref(),
                    )?
                };
                let dep_manifest = parse_manifest_at_root(&dep_root)?;
                let actual_id = format!(
                    "{}/{}",
                    dep_manifest.package.namespace, dep_manifest.package.name
                );
                if actual_id != dependency.package {
                    return Err(format!(
                        "git dependency `{}` expected package `{}`, found `{}`",
                        dependency.alias, dependency.package, actual_id
                    ));
                }
                let actual_rev = git_head_rev(&dep_root)?;
                let child_dependencies = resolve_dependencies(
                    &dep_manifest.dependencies,
                    &dep_root,
                    &dependency.package,
                    package_graph,
                    package_sources,
                    lock_source_base,
                    dependency_cache_base,
                    offline,
                    &dependency_path,
                    registry_solver,
                    trust_policy,
                    trusted_transparency_keys,
                )?;
                let checksum = package_checksum(&dep_root)?;
                (
                    DependencySource::Git {
                        git: git.clone(),
                        branch: branch.clone(),
                        tag: tag.clone(),
                        rev: Some(actual_rev),
                    },
                    Some(checksum),
                    None,
                    child_dependencies,
                )
            }
            DependencySource::Registry { version, registry } => {
                let authorization = registry_dependency_authorization(registry.as_deref())?;
                let selected_version = registry_solver.select(
                    dependency_cache_base.unwrap_or(base_root),
                    dependency,
                    version,
                    registry.as_deref(),
                    offline,
                    authorization.as_deref(),
                    &dependency_path,
                )?;
                let selected_source = DependencySource::Registry {
                    version: selected_version.clone(),
                    registry: registry.clone(),
                };
                let verified = resolve_registry_source_with_policy(
                    dependency_cache_base.unwrap_or(base_root),
                    &dependency.alias,
                    &dependency.package,
                    &selected_version,
                    registry.as_deref(),
                    offline,
                    authorization.as_deref(),
                    trust_policy,
                    trusted_transparency_keys,
                )?;
                match verified.root {
                    Some(dep_root) => {
                        let dep_manifest = parse_manifest_at_root(&dep_root)?;
                        let actual_id = format!(
                            "{}/{}",
                            dep_manifest.package.namespace, dep_manifest.package.name
                        );
                        if actual_id != dependency.package {
                            return Err(format!(
                                "registry dependency `{}` expected package `{}`, found `{}`",
                                dependency.alias, dependency.package, actual_id
                            ));
                        }
                        if dep_manifest.package.version != selected_version {
                            return Err(format!(
                                "registry dependency `{}` selected version `{selected_version}`, but the package manifest declares `{}`",
                                dependency.alias, dep_manifest.package.version
                            ));
                        }
                        let child_dependencies = resolve_dependencies(
                            &dep_manifest.dependencies,
                            &dep_root,
                            &dependency.package,
                            package_graph,
                            package_sources,
                            lock_source_base,
                            dependency_cache_base,
                            offline,
                            &dependency_path,
                            registry_solver,
                            trust_policy,
                            trusted_transparency_keys,
                        )?;
                        let checksum = package_checksum(&dep_root)?;
                        (
                            selected_source,
                            Some(checksum),
                            verified.evidence,
                            child_dependencies,
                        )
                    }
                    None => (selected_source, None, verified.evidence, Vec::new()),
                }
            }
        };
        remember_package_source(
            package_sources,
            &dependency.package,
            &resolved_source,
            registry_solver.changed,
        )?;

        resolved.push(ResolvedDependency {
            alias: dependency.alias.clone(),
            package: dependency.package.clone(),
            source: resolved_source,
            checksum,
            supply_chain,
            dependencies: child_dependencies,
        });
    }
    Ok(resolved)
}

fn remember_package_source(
    package_sources: &mut BTreeMap<String, DependencySource>,
    package: &str,
    source: &DependencySource,
    registry_selection_changed: bool,
) -> Result<(), String> {
    if let Some(existing) = package_sources.get(package) {
        if existing != source {
            if registry_selection_changed
                && matches!(existing, DependencySource::Registry { .. })
                && matches!(source, DependencySource::Registry { .. })
            {
                package_sources.insert(package.to_string(), source.clone());
                return Ok(());
            }
            return Err(format!(
                "package `{}` is required with conflicting sources: {} and {}",
                package,
                source_description(existing),
                source_description(source)
            ));
        }
    } else {
        package_sources.insert(package.to_string(), source.clone());
    }
    Ok(())
}

fn dependency_graph_and_source_base_from_lockfile(
    root: &Path,
    lock_root: &Path,
) -> Result<(DependencyGraph, PathBuf), String> {
    let manifest = parse_manifest_at_root(root)?;
    let lock_path = lock_root.join("nomo.lock");
    if !lock_path.is_file() {
        return Err(format!(
            "nomo.lock is required for locked mode at {}",
            lock_path.display()
        ));
    }
    let text = fs::read_to_string(&lock_path).map_err(|err| err.to_string())?;
    let root_id = format!("{}/{}", manifest.package.namespace, manifest.package.name);
    let parsed = parse_lockfile_root(&text, &root_id)?;
    let checksum_base = if parsed.has_workspace_roots {
        lock_root
    } else {
        root
    };
    let dependencies = parsed.dependencies;
    verify_locked_source_checksums(checksum_base, &dependencies)?;
    Ok((
        DependencyGraph {
            root: manifest.package,
            dependencies,
        },
        checksum_base.to_path_buf(),
    ))
}

fn verify_locked_source_checksums(
    base_root: &Path,
    dependencies: &[ResolvedDependency],
) -> Result<(), String> {
    for dependency in dependencies {
        let Some(dep_root) = locked_or_vendor_source_root(base_root, dependency)? else {
            continue;
        };
        if let Some(expected) = &dependency.checksum {
            let actual = package_checksum(&dep_root)?;
            if &actual != expected {
                return Err(format!(
                    "checksum mismatch for locked package `{}`: expected {}, found {}",
                    dependency.package, expected, actual
                ));
            }
        }
        verify_locked_source_checksums(&dep_root, &dependency.dependencies)?;
    }
    Ok(())
}

#[cfg(test)]
mod registry_solver_tests {
    use super::RegistrySolveState;
    use nomo_manifest::{Dependency, DependencySource};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_registry() -> (PathBuf, String) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "nomo-project-registry-solver-{}-{nonce}",
            std::process::id()
        ));
        let index = root.join("registry/api/v1/packages/nomo-lang/json/index.json");
        fs::create_dir_all(index.parent().unwrap()).unwrap();
        fs::write(
            index,
            format!(
                r#"{{"package":"nomo-lang/json","versions":[{{"version":"1.4.0","checksum":"sha256:{}","yanked":false}},{{"version":"1.9.0","checksum":"sha256:{}","yanked":false}},{{"version":"2.1.0","checksum":"sha256:{}","yanked":false}}]}}"#,
                "a".repeat(64),
                "b".repeat(64),
                "c".repeat(64)
            ),
        )
        .unwrap();
        let endpoint = format!("file://{}", root.join("registry").display());
        (root, endpoint)
    }

    fn dependency(requirement: &str, registry: &str) -> Dependency {
        Dependency {
            alias: "json".to_string(),
            package: "nomo-lang/json".to_string(),
            source: DependencySource::Registry {
                version: requirement.to_string(),
                registry: Some(registry.to_string()),
            },
        }
    }

    #[test]
    fn solver_converges_after_a_later_constraint_lowers_the_selection() {
        let (root, registry) = temp_registry();
        let mut solver = RegistrySolveState::default();
        solver.begin_pass();
        let broad = dependency("^1.0.0", &registry);
        let narrow = dependency(">=1.0, <1.5", &registry);

        let first = solver
            .select(
                &root,
                &broad,
                "^1.0.0",
                Some(&registry),
                false,
                None,
                &["fynn/app".to_string(), "api".to_string()],
            )
            .unwrap();
        let lowered = solver
            .select(
                &root,
                &narrow,
                ">=1.0, <1.5",
                Some(&registry),
                false,
                None,
                &["fynn/app".to_string(), "worker".to_string()],
            )
            .unwrap();
        solver.finalize_pass().unwrap();

        assert_eq!(first, "1.9.0");
        assert_eq!(lowered, "1.4.0");
        assert!(solver.changed);

        solver.begin_pass();
        let retained = solver
            .select(
                &root,
                &broad,
                "^1.0.0",
                Some(&registry),
                false,
                None,
                &["fynn/app".to_string(), "api".to_string()],
            )
            .unwrap();
        solver
            .select(
                &root,
                &narrow,
                ">=1.0, <1.5",
                Some(&registry),
                false,
                None,
                &["fynn/app".to_string(), "worker".to_string()],
            )
            .unwrap();
        solver.finalize_pass().unwrap();

        assert_eq!(retained, "1.4.0");
        assert!(!solver.changed);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn solver_reports_dependency_paths_for_incompatible_constraints() {
        let (root, registry) = temp_registry();
        let mut solver = RegistrySolveState::default();
        solver.begin_pass();
        let one = dependency("^1.0.0", &registry);
        let two = dependency("^2.0.0", &registry);
        solver
            .select(
                &root,
                &one,
                "^1.0.0",
                Some(&registry),
                false,
                None,
                &["fynn/app".to_string(), "api".to_string()],
            )
            .unwrap();
        let error = solver
            .select(
                &root,
                &two,
                "^2.0.0",
                Some(&registry),
                false,
                None,
                &["fynn/app".to_string(), "worker".to_string()],
            )
            .unwrap_err();

        assert!(error.contains("`^1.0.0` required by fynn/app -> api"));
        assert!(error.contains("`^2.0.0` required by fynn/app -> worker"));
        fs::remove_dir_all(root).unwrap();
    }
}

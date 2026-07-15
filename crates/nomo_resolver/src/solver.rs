use crate::{PackageVersion, VersionConstraint};
use std::collections::BTreeSet;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionCandidate {
    pub version: PackageVersion,
    pub yanked: bool,
}

impl VersionCandidate {
    pub fn available(version: &str) -> Result<Self, String> {
        Ok(Self {
            version: PackageVersion::parse(version)?,
            yanked: false,
        })
    }

    pub fn yanked(version: &str) -> Result<Self, String> {
        Ok(Self {
            version: PackageVersion::parse(version)?,
            yanked: true,
        })
    }
}

/// The dependency edge that introduced one constraint for a canonical package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstraintOrigin {
    pub requirement: VersionConstraint,
    pub dependency_path: Vec<String>,
}

impl ConstraintOrigin {
    pub fn new(
        requirement: &str,
        dependency_path: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, String> {
        let dependency_path: Vec<String> = dependency_path.into_iter().map(Into::into).collect();
        if dependency_path.is_empty() {
            return Err("a version constraint must have a dependency path".to_string());
        }
        Ok(Self {
            requirement: VersionConstraint::parse(requirement)?,
            dependency_path,
        })
    }

    fn sort_key(&self) -> (String, String) {
        (
            self.dependency_path.join(" -> "),
            self.requirement.normalized(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionConflict {
    pub package: String,
    pub constraints: Vec<ConstraintOrigin>,
    pub available_versions: Vec<PackageVersion>,
    pub yanked_versions: Vec<PackageVersion>,
}

impl ResolutionConflict {
    pub fn render(&self) -> String {
        let mut message = format!(
            "failed to resolve package `{}`: no available version satisfies all constraints",
            self.package
        );
        for constraint in &self.constraints {
            message.push_str(&format!(
                "\n- `{}` required by {}",
                constraint.requirement,
                constraint.dependency_path.join(" -> ")
            ));
        }
        if self.available_versions.is_empty() {
            message.push_str("\n- the registry has no non-yanked versions available");
        } else {
            let versions = self
                .available_versions
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            message.push_str(&format!("\n- available versions: {versions}"));
        }
        if !self.yanked_versions.is_empty() {
            let versions = self
                .yanked_versions
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            message.push_str(&format!("\n- yanked versions excluded: {versions}"));
        }
        message.push_str(
            "\nhelp: align the listed dependency requirements or use `nomo deps update <package> --precise <version>`",
        );
        message
    }
}

impl fmt::Display for ResolutionConflict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.render())
    }
}

/// Select the highest non-yanked candidate satisfying every constraint.
///
/// Inputs are normalized before selection, so registry response order cannot
/// affect the result. Duplicate versions are rejected instead of relying on
/// ambiguous metadata.
pub fn select_highest_version(
    package: &str,
    candidates: &[VersionCandidate],
    constraints: &[ConstraintOrigin],
) -> Result<PackageVersion, ResolutionConflict> {
    let (available, yanked) = normalized_candidates(candidates);
    if let Some(selected) = available.iter().rev().find(|version| {
        constraints
            .iter()
            .all(|item| item.requirement.matches(version))
    }) {
        return Ok(selected.clone());
    }

    let mut constraints = constraints.to_vec();
    constraints.sort_by_key(ConstraintOrigin::sort_key);
    constraints = minimal_unsatisfied_constraints(&available, constraints);
    Err(ResolutionConflict {
        package: package.to_string(),
        constraints,
        available_versions: available,
        yanked_versions: yanked,
    })
}

fn normalized_candidates(
    candidates: &[VersionCandidate],
) -> (Vec<PackageVersion>, Vec<PackageVersion>) {
    let mut available = BTreeSet::new();
    let mut yanked = BTreeSet::new();
    for candidate in candidates {
        if candidate.yanked {
            yanked.insert(candidate.version.clone());
            available.remove(&candidate.version);
        } else if !yanked.contains(&candidate.version) {
            available.insert(candidate.version.clone());
        }
    }
    (
        available.into_iter().collect(),
        yanked.into_iter().collect(),
    )
}

fn minimal_unsatisfied_constraints(
    available: &[PackageVersion],
    mut constraints: Vec<ConstraintOrigin>,
) -> Vec<ConstraintOrigin> {
    let mut index = 0;
    while index < constraints.len() {
        let mut without = constraints.clone();
        without.remove(index);
        if !has_satisfying_version(available, &without) {
            constraints = without;
        } else {
            index += 1;
        }
    }
    constraints
}

fn has_satisfying_version(available: &[PackageVersion], constraints: &[ConstraintOrigin]) -> bool {
    available.iter().any(|version| {
        constraints
            .iter()
            .all(|item| item.requirement.matches(version))
    })
}

#[cfg(test)]
mod tests {
    use super::{ConstraintOrigin, VersionCandidate, select_highest_version};

    fn candidates(versions: &[&str]) -> Vec<VersionCandidate> {
        versions
            .iter()
            .map(|version| VersionCandidate::available(version).unwrap())
            .collect()
    }

    fn constraint(requirement: &str, path: &[&str]) -> ConstraintOrigin {
        ConstraintOrigin::new(requirement, path.iter().copied()).unwrap()
    }

    #[test]
    fn selects_the_highest_matching_version_independent_of_registry_order() {
        let requirements = [constraint("^1.2.0", &["fynn/app", "nomo-lang/json"])];
        let ascending = candidates(&["1.2.0", "2.0.0", "1.9.1"]);
        let shuffled = candidates(&["1.9.1", "1.2.0", "2.0.0"]);

        let first = select_highest_version("nomo-lang/json", &ascending, &requirements).unwrap();
        let second = select_highest_version("nomo-lang/json", &shuffled, &requirements).unwrap();

        assert_eq!(first.to_string(), "1.9.1");
        assert_eq!(first, second);
    }

    #[test]
    fn excludes_yanked_and_implicit_prerelease_candidates() {
        let candidates = vec![
            VersionCandidate::available("1.2.0").unwrap(),
            VersionCandidate::yanked("1.3.0").unwrap(),
            VersionCandidate::available("1.4.0-alpha.1").unwrap(),
        ];
        let requirements = [constraint("^1.0.0", &["fynn/app", "nomo-lang/json"])];

        let selected =
            select_highest_version("nomo-lang/json", &candidates, &requirements).unwrap();

        assert_eq!(selected.to_string(), "1.2.0");
    }

    #[test]
    fn selects_an_explicit_snapshot_prerelease() {
        let candidates = candidates(&["0.0.0-20260713145859", "0.0.0-20260715120000", "0.1.0"]);
        let requirements = [constraint(
            "0.0.0-20260715120000",
            &["fynn/app", "nomo-lang/std"],
        )];

        let selected = select_highest_version("nomo-lang/std", &candidates, &requirements).unwrap();

        assert_eq!(selected.to_string(), "0.0.0-20260715120000");
    }

    #[test]
    fn conflict_contains_a_stable_minimal_constraint_set() {
        let candidates = candidates(&["1.4.0", "1.8.0", "2.1.0"]);
        let requirements = [
            constraint(">=1.0, <3.0", &["fynn/app", "nomo-lang/json"]),
            constraint("^1.4.0", &["fynn/app", "api", "nomo-lang/json"]),
            constraint("^2.0.0", &["fynn/app", "worker", "nomo-lang/json"]),
        ];

        let conflict =
            select_highest_version("nomo-lang/json", &candidates, &requirements).unwrap_err();

        assert_eq!(conflict.constraints.len(), 2);
        assert_eq!(
            conflict.render(),
            "failed to resolve package `nomo-lang/json`: no available version satisfies all constraints\n- `^1.4.0` required by fynn/app -> api -> nomo-lang/json\n- `^2.0.0` required by fynn/app -> worker -> nomo-lang/json\n- available versions: 1.4.0, 1.8.0, 2.1.0\nhelp: align the listed dependency requirements or use `nomo deps update <package> --precise <version>`"
        );
    }

    #[test]
    fn conflict_explains_when_only_yanked_versions_exist() {
        let candidates = vec![VersionCandidate::yanked("1.2.0").unwrap()];
        let requirements = [constraint("^1.0.0", &["fynn/app", "nomo-lang/json"])];

        let conflict =
            select_highest_version("nomo-lang/json", &candidates, &requirements).unwrap_err();

        assert!(conflict.constraints.is_empty());
        assert!(
            conflict
                .render()
                .contains("the registry has no non-yanked versions available")
        );
        assert!(
            conflict
                .render()
                .contains("yanked versions excluded: 1.2.0")
        );
    }
}

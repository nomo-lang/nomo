use semver::{Op, Version, VersionReq};
use std::fmt;
use std::str::FromStr;

/// A canonical package version used by manifests, registries, and lockfiles.
///
/// Nomo accepts SemVer 2.0 versions, including timestamped development
/// snapshots such as `0.0.0-20260713145859`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackageVersion(Version);

impl PackageVersion {
    pub fn parse(input: &str) -> Result<Self, String> {
        let input = input.trim();
        if input.is_empty() {
            return Err("package version must not be empty".to_string());
        }
        Version::parse(input)
            .map(Self)
            .map_err(|err| format!("invalid semantic version `{input}`: {err}"))
    }

    pub fn is_prerelease(&self) -> bool {
        !self.0.pre.is_empty()
    }

    pub fn as_semver(&self) -> &Version {
        &self.0
    }
}

impl fmt::Display for PackageVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for PackageVersion {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::parse(input)
    }
}

/// A manifest dependency constraint.
///
/// A bare complete version is exact in Nomo. This deliberately differs from
/// libraries that interpret bare versions as caret requirements. Ranges must
/// therefore opt in with `^`, `~`, or explicit comparison operators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionConstraint {
    Exact(PackageVersion),
    Range {
        normalized: String,
        requirement: VersionReq,
    },
}

impl VersionConstraint {
    pub fn parse(input: &str) -> Result<Self, String> {
        let input = input.trim();
        if input.is_empty() {
            return Err("package version constraint must not be empty".to_string());
        }

        if let Ok(version) = PackageVersion::parse(input) {
            return Ok(Self::Exact(version));
        }

        validate_range_syntax(input)?;
        let requirement = VersionReq::parse(input)
            .map_err(|err| format!("invalid semantic version constraint `{input}`: {err}"))?;
        validate_range_shape(input, &requirement)?;
        Ok(Self::Range {
            normalized: requirement.to_string(),
            requirement,
        })
    }

    pub fn matches(&self, version: &PackageVersion) -> bool {
        match self {
            Self::Exact(expected) => expected == version,
            Self::Range { requirement, .. } => requirement.matches(version.as_semver()),
        }
    }

    pub fn is_exact(&self) -> bool {
        matches!(self, Self::Exact(_))
    }

    pub fn explicitly_allows_prerelease(&self) -> bool {
        match self {
            Self::Exact(version) => version.is_prerelease(),
            Self::Range { requirement, .. } => requirement
                .comparators
                .iter()
                .any(|comparator| !comparator.pre.is_empty()),
        }
    }

    pub fn normalized(&self) -> String {
        match self {
            Self::Exact(version) => version.to_string(),
            Self::Range { normalized, .. } => normalized.clone(),
        }
    }
}

fn validate_range_shape(input: &str, requirement: &VersionReq) -> Result<(), String> {
    match input.as_bytes().first() {
        Some(b'^')
            if requirement.comparators.len() == 1 && requirement.comparators[0].op == Op::Caret =>
        {
            Ok(())
        }
        Some(b'~')
            if requirement.comparators.len() == 1 && requirement.comparators[0].op == Op::Tilde =>
        {
            Ok(())
        }
        Some(b'>' | b'<') => {
            let has_lower = requirement
                .comparators
                .iter()
                .any(|comparator| matches!(comparator.op, Op::Greater | Op::GreaterEq));
            let has_upper = requirement
                .comparators
                .iter()
                .any(|comparator| matches!(comparator.op, Op::Less | Op::LessEq));
            let comparisons_only = requirement.comparators.iter().all(|comparator| {
                matches!(
                    comparator.op,
                    Op::Greater | Op::GreaterEq | Op::Less | Op::LessEq
                )
            });
            if has_lower && has_upper && comparisons_only {
                Ok(())
            } else {
                Err(format!(
                    "unsupported semantic version constraint `{input}`: comparison ranges must include both lower and upper bounds"
                ))
            }
        }
        _ => Err(format!(
            "unsupported semantic version constraint `{input}`: caret and tilde constraints must contain exactly one comparator"
        )),
    }
}

impl fmt::Display for VersionConstraint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.normalized())
    }
}

impl FromStr for VersionConstraint {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::parse(input)
    }
}

fn validate_range_syntax(input: &str) -> Result<(), String> {
    let lowered = input.to_ascii_lowercase();
    if lowered.contains('*') || contains_wildcard_x(&lowered) {
        return Err(format!(
            "unsupported semantic version constraint `{input}`: wildcards are not allowed"
        ));
    }
    if input.contains("||") {
        return Err(format!(
            "unsupported semantic version constraint `{input}`: alternatives are not allowed"
        ));
    }
    if input.starts_with('=') {
        return Err(format!(
            "unsupported semantic version constraint `{input}`: use a bare version for an exact requirement"
        ));
    }
    if !matches!(input.as_bytes().first(), Some(b'^' | b'~' | b'>' | b'<')) {
        return Err(format!(
            "unsupported semantic version constraint `{input}`: expected an exact version, caret, tilde, or comparison range"
        ));
    }
    Ok(())
}

fn contains_wildcard_x(input: &str) -> bool {
    input
        .split(|character: char| {
            character == '.' || character == ',' || character.is_ascii_whitespace()
        })
        .any(|part| part == "x")
}

#[cfg(test)]
mod tests {
    use super::{PackageVersion, VersionConstraint};
    use crate::{DependencySource, parse_manifest_text};
    use std::path::Path;

    fn version(input: &str) -> PackageVersion {
        PackageVersion::parse(input).unwrap()
    }

    fn requirement(input: &str) -> VersionConstraint {
        VersionConstraint::parse(input).unwrap()
    }

    #[test]
    fn parses_and_orders_stable_and_snapshot_versions() {
        let stable = version("1.0.5");
        let older_snapshot = version("0.0.0-20260713145859");
        let newer_snapshot = version("0.0.0-20260715120000");

        assert!(!stable.is_prerelease());
        assert!(older_snapshot.is_prerelease());
        assert!(older_snapshot < newer_snapshot);
        assert!(newer_snapshot < stable);
        assert_eq!(older_snapshot.to_string(), "0.0.0-20260713145859");
    }

    #[test]
    fn treats_a_bare_complete_version_as_exact() {
        let constraint = requirement("1.2.3");

        assert!(constraint.is_exact());
        assert!(constraint.matches(&version("1.2.3")));
        assert!(!constraint.matches(&version("1.2.4")));
        assert_eq!(constraint.normalized(), "1.2.3");
    }

    #[test]
    fn supports_caret_tilde_and_bounded_comparison_ranges() {
        let caret = requirement("^1.2.3");
        assert!(caret.matches(&version("1.9.0")));
        assert!(!caret.matches(&version("2.0.0")));

        let tilde = requirement("~1.2.3");
        assert!(tilde.matches(&version("1.2.9")));
        assert!(!tilde.matches(&version("1.3.0")));

        let bounded = requirement(">=1.2, <2.0");
        assert!(bounded.matches(&version("1.2.0")));
        assert!(bounded.matches(&version("1.99.0")));
        assert!(!bounded.matches(&version("2.0.0")));
    }

    #[test]
    fn prereleases_participate_only_when_explicitly_named() {
        let stable_range = requirement("^1.2.3");
        let prerelease_range = requirement(">=1.3.0-alpha.1, <2.0.0");
        let prerelease = version("1.3.0-alpha.2");

        assert!(!stable_range.explicitly_allows_prerelease());
        assert!(!stable_range.matches(&prerelease));
        assert!(prerelease_range.explicitly_allows_prerelease());
        assert!(prerelease_range.matches(&prerelease));
    }

    #[test]
    fn rejects_wildcards_alternatives_and_implicit_exact_operators() {
        for invalid in [
            "*",
            "1.x",
            "^1 || ^2",
            "=1.2.3",
            "latest",
            "1.2",
            ">=1.2",
            "<2.0",
            "^1.2, <2.0",
        ] {
            assert!(
                VersionConstraint::parse(invalid).is_err(),
                "constraint `{invalid}` should be rejected"
            );
        }
    }

    #[test]
    fn normalized_constraints_round_trip_over_representative_versions() {
        let versions = [
            version("0.0.0-20260713145859"),
            version("0.1.0"),
            version("1.2.3-alpha.1"),
            version("1.2.3"),
            version("1.9.9"),
            version("2.0.0"),
        ];

        for source in ["1.2.3", "^1.2.3", "~1.2.3", ">=1.2, <2.0"] {
            let parsed = requirement(source);
            let reparsed = requirement(&parsed.normalized());
            for candidate in &versions {
                assert_eq!(
                    parsed.matches(candidate),
                    reparsed.matches(candidate),
                    "normalized `{}` changed the meaning of `{source}` for `{candidate}`",
                    parsed.normalized()
                );
            }
        }
    }

    #[test]
    fn manifest_accepts_supported_ranges_and_rejects_wildcards() {
        let manifest = parse_manifest_text(
            "[package]\nnamespace = \"fynn\"\nname = \"app\"\nversion = \"0.0.0-20260713145859\"\n\n[dependencies]\njson = { package = \"nomo-lang/json\", version = \">=1.2, <2.0\" }\n",
            Path::new("app"),
        )
        .unwrap();
        assert!(matches!(
            &manifest.dependencies[0].source,
            DependencySource::Registry { version, .. } if version == ">=1.2, <2.0"
        ));

        let error = parse_manifest_text(
            "[package]\nnamespace = \"fynn\"\nname = \"app\"\nversion = \"1.0.0\"\n\n[dependencies]\njson = { package = \"nomo-lang/json\", version = \"1.x\" }\n",
            Path::new("app"),
        )
        .unwrap_err();
        assert!(error.contains("wildcards are not allowed"), "{error}");
    }
}

#![allow(dead_code)]

use std::collections::HashSet;
use std::process::Command;

use serde::Deserialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReleaseCheckMode {
    Full,
    VersionOnly,
    PackagesOnly,
    ChangelogOnly,
}

pub(crate) fn run_release_check(mode: ReleaseCheckMode) -> Result<(), String> {
    match mode {
        ReleaseCheckMode::VersionOnly => validate_repository_version_policy(),
        ReleaseCheckMode::PackagesOnly => validate_repository_package_targets(),
        ReleaseCheckMode::ChangelogOnly => validate_repository_changelog(),
        ReleaseCheckMode::Full => run_full_release_check(),
    }
}

fn run_full_release_check() -> Result<(), String> {
    validate_repository_release_criteria()?;
    validate_repository_version_policy()?;
    validate_repository_package_targets()?;
    validate_repository_dependency_policy()?;
    validate_repository_changelog()?;
    run_script("scripts/check.sh")
}

fn validate_repository_release_criteria() -> Result<(), String> {
    ReleaseCriteria::parse(include_str!("../../release/release-criteria.toml"))
        .map(|_| ())
        .map_err(|error| format!("release criteria validation failed: {error:?}"))
}

fn validate_repository_version_policy() -> Result<(), String> {
    VersionPolicy::parse(include_str!("../../release/version-policy.toml"))
        .map(|_| ())
        .map_err(|error| format!("version policy validation failed: {error:?}"))
}

fn validate_repository_package_targets() -> Result<(), String> {
    PackageTargets::parse(include_str!("../../release/package-targets.toml"))
        .map(|_| ())
        .map_err(|error| format!("package target validation failed: {error:?}"))
}

fn validate_repository_dependency_policy() -> Result<(), String> {
    DependencyPolicy::parse(include_str!("../../release/dependency-policy.toml"))
        .map(|_| ())
        .map_err(|error| format!("dependency policy validation failed: {error:?}"))
}

fn validate_repository_changelog() -> Result<(), String> {
    Changelog::parse(include_str!("../../CHANGELOG.md"))
        .map(|_| ())
        .map_err(|error| format!("changelog validation failed: {error:?}"))
}

fn run_script(script: &str) -> Result<(), String> {
    let status = Command::new("bash")
        .arg(script)
        .status()
        .map_err(|error| format!("failed to run {script}: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("{script} failed with {status}"))
    }
}

#[derive(Debug, Deserialize)]
struct ReleaseCriteria {
    criteria: Vec<ReleaseCriterion>,
}

impl ReleaseCriteria {
    fn parse(input: &str) -> Result<Self, ReleaseCriteriaError> {
        let criteria: Self = toml::from_str(input)
            .map_err(|error| ReleaseCriteriaError::Parse(error.to_string()))?;
        criteria.validate()?;
        Ok(criteria)
    }

    fn contains(&self, id: &str) -> bool {
        self.criteria.iter().any(|criterion| criterion.id == id)
    }

    fn release_blockers(&self) -> impl Iterator<Item = &ReleaseCriterion> {
        self.criteria
            .iter()
            .filter(|criterion| criterion.blocking == BlockingLevel::Release)
    }

    fn validate(&self) -> Result<(), ReleaseCriteriaError> {
        let mut ids = HashSet::new();

        for criterion in &self.criteria {
            criterion.require_field("id", &criterion.id)?;
            criterion.require_field("title", &criterion.title)?;
            criterion.require_field("owner", &criterion.owner)?;
            criterion.require_field("command", &criterion.command)?;
            criterion.require_field("evidence", &criterion.evidence)?;

            if !ids.insert(criterion.id.clone()) {
                return Err(ReleaseCriteriaError::DuplicateCriterion(
                    criterion.id.clone(),
                ));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct ReleaseCriterion {
    id: String,
    title: String,
    owner: String,
    command: String,
    blocking: BlockingLevel,
    evidence: String,
}

impl ReleaseCriterion {
    fn require_field(&self, field: &'static str, value: &str) -> Result<(), ReleaseCriteriaError> {
        if value.trim().is_empty() {
            Err(ReleaseCriteriaError::MissingRequiredField {
                id: self.id.clone(),
                field,
            })
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum BlockingLevel {
    Advisory,
    Release,
}

#[derive(Debug, PartialEq, Eq)]
enum ReleaseCriteriaError {
    Parse(String),
    DuplicateCriterion(String),
    MissingRequiredField { id: String, field: &'static str },
}

#[derive(Debug, Deserialize)]
struct VersionPolicy {
    crate_version: String,
    artifact_schema_version: u32,
    package_version: String,
    manual_version: String,
    compatibility_notes_required: bool,
}

impl VersionPolicy {
    fn parse(input: &str) -> Result<Self, VersionPolicyError> {
        let policy: Self =
            toml::from_str(input).map_err(|error| VersionPolicyError::Parse(error.to_string()))?;
        policy.validate()?;
        Ok(policy)
    }

    fn validate(&self) -> Result<(), VersionPolicyError> {
        require_policy_field("crate_version", &self.crate_version)?;
        require_policy_field("package_version", &self.package_version)?;
        require_policy_field("manual_version", &self.manual_version)?;

        if self.artifact_schema_version == 0 {
            return Err(VersionPolicyError::InvalidArtifactSchemaVersion(0));
        }

        require_matching_version(
            "crate_version",
            &self.crate_version,
            "package_version",
            &self.package_version,
        )?;
        require_matching_version(
            "crate_version",
            &self.crate_version,
            "manual_version",
            &self.manual_version,
        )?;

        if !self.compatibility_notes_required {
            return Err(VersionPolicyError::CompatibilityNotesNotRequired);
        }

        Ok(())
    }
}

fn require_policy_field(field: &'static str, value: &str) -> Result<(), VersionPolicyError> {
    if value.trim().is_empty() {
        Err(VersionPolicyError::MissingRequiredField(field))
    } else {
        Ok(())
    }
}

fn require_matching_version(
    left: &'static str,
    left_value: &str,
    right: &'static str,
    right_value: &str,
) -> Result<(), VersionPolicyError> {
    if left_value == right_value {
        Ok(())
    } else {
        Err(VersionPolicyError::MismatchedVersion {
            left: left.into(),
            left_value: left_value.into(),
            right: right.into(),
            right_value: right_value.into(),
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
enum VersionPolicyError {
    Parse(String),
    MissingRequiredField(&'static str),
    InvalidArtifactSchemaVersion(u32),
    CompatibilityNotesNotRequired,
    MismatchedVersion {
        left: String,
        left_value: String,
        right: String,
        right_value: String,
    },
}

#[derive(Debug, Deserialize)]
struct PackageTargets {
    targets: Vec<PackageTarget>,
}

impl PackageTargets {
    fn parse(input: &str) -> Result<Self, PackageTargetsError> {
        let targets: Self =
            toml::from_str(input).map_err(|error| PackageTargetsError::Parse(error.to_string()))?;
        targets.validate()?;
        Ok(targets)
    }

    fn contains(&self, id: &str) -> bool {
        self.targets.iter().any(|target| target.id == id)
    }

    fn release_blockers(&self) -> impl Iterator<Item = &PackageTarget> {
        self.targets.iter().filter(|target| target.release_gate)
    }

    fn validate(&self) -> Result<(), PackageTargetsError> {
        let mut ids = HashSet::new();

        for target in &self.targets {
            target.require_field("id", &target.id)?;
            target.require_field("kind", &target.kind)?;
            target.require_field("platform", &target.platform)?;
            target.require_field("command", &target.command)?;
            target.require_field("evidence", &target.evidence)?;

            if !ids.insert(target.id.clone()) {
                return Err(PackageTargetsError::DuplicateTarget(target.id.clone()));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct PackageTarget {
    id: String,
    kind: String,
    platform: String,
    command: String,
    evidence: String,
    release_gate: bool,
}

impl PackageTarget {
    fn require_field(&self, field: &'static str, value: &str) -> Result<(), PackageTargetsError> {
        if value.trim().is_empty() {
            Err(PackageTargetsError::MissingRequiredField {
                id: self.id.clone(),
                field,
            })
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum PackageTargetsError {
    Parse(String),
    DuplicateTarget(String),
    MissingRequiredField { id: String, field: &'static str },
}

#[derive(Debug, Deserialize)]
struct DependencyPolicy {
    dependencies: Vec<DependencyPolicyEntry>,
}

impl DependencyPolicy {
    fn parse(input: &str) -> Result<Self, DependencyPolicyError> {
        let policy: Self = toml::from_str(input)
            .map_err(|error| DependencyPolicyError::Parse(error.to_string()))?;
        policy.validate()?;
        Ok(policy)
    }

    fn contains(&self, name: &str) -> bool {
        self.dependencies
            .iter()
            .any(|dependency| dependency.name == name)
    }

    fn release_blockers(&self) -> impl Iterator<Item = &DependencyPolicyEntry> {
        self.dependencies
            .iter()
            .filter(|dependency| dependency.release_blocker)
    }

    fn validate(&self) -> Result<(), DependencyPolicyError> {
        let mut names = HashSet::new();
        for dependency in &self.dependencies {
            dependency.require_field("name", &dependency.name)?;
            dependency.require_field("version", &dependency.version)?;
            dependency.require_field("owner", &dependency.owner)?;
            dependency.require_field("risk", &dependency.risk)?;
            dependency.require_field("upgrade_gate", &dependency.upgrade_gate)?;

            if !names.insert(dependency.name.clone()) {
                return Err(DependencyPolicyError::DuplicateDependency(
                    dependency.name.clone(),
                ));
            }

            if dependency.source == DependencySource::Git && !dependency.release_blocker {
                return Err(DependencyPolicyError::GitDependencyNotReleaseBlocked(
                    dependency.name.clone(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct DependencyPolicyEntry {
    name: String,
    source: DependencySource,
    version: String,
    owner: String,
    risk: String,
    release_blocker: bool,
    upgrade_gate: String,
}

impl DependencyPolicyEntry {
    fn require_field(&self, field: &'static str, value: &str) -> Result<(), DependencyPolicyError> {
        if value.trim().is_empty() {
            Err(DependencyPolicyError::MissingRequiredField {
                name: self.name.clone(),
                field,
            })
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum DependencySource {
    CratesIo,
    Git,
}

#[derive(Debug, PartialEq, Eq)]
enum DependencyPolicyError {
    Parse(String),
    DuplicateDependency(String),
    GitDependencyNotReleaseBlocked(String),
    MissingRequiredField { name: String, field: &'static str },
}

#[derive(Debug)]
struct Changelog<'a> {
    text: &'a str,
}

impl<'a> Changelog<'a> {
    fn parse(input: &'a str) -> Result<Self, ChangelogError> {
        let changelog = Self { text: input };

        if !changelog
            .text
            .lines()
            .any(|line| line.trim() == "# Changelog")
        {
            return Err(ChangelogError::MissingTitle);
        }

        if !changelog.has_verification_evidence() {
            return Err(ChangelogError::MissingVerificationEvidence);
        }

        Ok(changelog)
    }

    fn has_section(&self, section: &str) -> bool {
        let heading = format!("### {section}");
        self.text.lines().any(|line| line.trim() == heading)
    }

    fn has_verification_evidence(&self) -> bool {
        self.text.contains("Verification Evidence:")
            && self.text.contains("target/release-evidence/")
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ChangelogError {
    MissingTitle,
    MissingVerificationEvidence,
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_CRITERIA: &str = r#"
[[criteria]]
id = "api-stability"
title = "API stability"
owner = "release"
command = "rtk cargo test -p hawk2ui-api"
blocking = "release"
evidence = "target/release-evidence/api-stability.txt"

[[criteria]]
id = "manuals"
title = "Manual completion"
owner = "docs"
command = "rtk cargo test --workspace manual"
blocking = "release"
evidence = "target/release-evidence/manuals.txt"
"#;

    #[test]
    fn repository_release_criteria_covers_all_required_release_gates() {
        let criteria = ReleaseCriteria::parse(include_str!("../../release/release-criteria.toml"))
            .expect("repository release criteria must parse");

        for id in [
            "api-stability",
            "artifact-compatibility",
            "ci-pass",
            "dependency-health",
            "compatibility-matrix",
            "performance-budgets",
            "plugin-realtime-safety",
            "security-gates",
            "smoke-apps",
            "manuals",
            "packaging",
        ] {
            assert!(criteria.contains(id), "missing release criterion {id}");
        }
    }

    #[test]
    fn parses_release_criteria_with_required_fields() {
        let criteria = ReleaseCriteria::parse(VALID_CRITERIA).expect("valid criteria must parse");

        assert_eq!(criteria.criteria.len(), 2);
        assert!(criteria.contains("api-stability"));
        assert!(criteria.release_blockers().all(|criterion| {
            criterion.blocking == BlockingLevel::Release && !criterion.evidence.as_str().is_empty()
        }));
    }

    #[test]
    fn rejects_criteria_without_required_evidence() {
        let input = r#"
[[criteria]]
id = "api-stability"
title = "API stability"
owner = "release"
command = "rtk cargo test -p hawk2ui-api"
blocking = "release"
evidence = ""
"#;

        let error = ReleaseCriteria::parse(input).expect_err("empty evidence path must fail");

        assert_eq!(
            error,
            ReleaseCriteriaError::MissingRequiredField {
                id: "api-stability".into(),
                field: "evidence"
            }
        );
    }

    #[test]
    fn rejects_duplicate_criterion_ids() {
        let input = r#"
[[criteria]]
id = "api-stability"
title = "API stability"
owner = "release"
command = "rtk cargo test -p hawk2ui-api"
blocking = "release"
evidence = "target/release-evidence/api-stability.txt"

[[criteria]]
id = "api-stability"
title = "Duplicate"
owner = "release"
command = "rtk cargo test -p hawk2ui-api"
blocking = "release"
evidence = "target/release-evidence/duplicate.txt"
"#;

        let error = ReleaseCriteria::parse(input).expect_err("duplicate IDs must fail");

        assert_eq!(
            error,
            ReleaseCriteriaError::DuplicateCriterion("api-stability".into())
        );
    }

    #[test]
    fn repository_version_policy_declares_all_version_domains() {
        let policy = VersionPolicy::parse(include_str!("../../release/version-policy.toml"))
            .expect("repository version policy must parse");

        assert_eq!(policy.crate_version, "0.1.0");
        assert_eq!(policy.artifact_schema_version, 1);
        assert_eq!(policy.package_version, "0.1.0");
        assert_eq!(policy.manual_version, "0.1.0");
        assert!(policy.compatibility_notes_required);
    }

    #[test]
    fn rejects_version_policy_with_mismatched_crate_and_package_versions() {
        let input = r#"
crate_version = "0.1.0"
artifact_schema_version = 1
package_version = "0.2.0"
manual_version = "0.1.0"
compatibility_notes_required = true
"#;

        let error = VersionPolicy::parse(input).expect_err("version mismatch must fail");

        assert_eq!(
            error,
            VersionPolicyError::MismatchedVersion {
                left: "crate_version".into(),
                left_value: "0.1.0".into(),
                right: "package_version".into(),
                right_value: "0.2.0".into(),
            }
        );
    }

    #[test]
    fn repository_package_targets_cover_required_outputs() {
        let targets = PackageTargets::parse(include_str!("../../release/package-targets.toml"))
            .expect("repository package targets must parse");

        for id in [
            "desktop-linux",
            "desktop-windows",
            "desktop-macos",
            "plugin-clap",
            "plugin-vst3",
            "plugin-au",
            "sealed-artifact",
            "debug-package",
            "release-package",
        ] {
            assert!(targets.contains(id), "missing package target {id}");
        }

        assert!(targets.release_blockers().all(|target| target.release_gate));
    }

    #[test]
    fn repository_dependency_policy_tracks_release_blocking_dependency_risks() {
        let policy = DependencyPolicy::parse(include_str!("../../release/dependency-policy.toml"))
            .expect("repository dependency policy must parse");

        for dependency in [
            "boa_engine",
            "oxc_allocator",
            "lightningcss",
            "taffy",
            "skia-safe",
        ] {
            assert!(
                policy.contains(dependency),
                "missing dependency policy entry for {dependency}"
            );
        }
        assert!(
            policy.release_blockers().any(|entry| {
                entry.name == "boa_engine" && entry.source == DependencySource::Git
            })
        );
    }

    #[test]
    fn rejects_duplicate_package_target_ids() {
        let input = r#"
[[targets]]
id = "desktop-linux"
kind = "desktop-bundle"
platform = "linux"
command = "rtk cargo test -p hawk2ui-build package_desktop_linux"
evidence = "target/release-evidence/desktop-linux.txt"
release_gate = true

[[targets]]
id = "desktop-linux"
kind = "desktop-bundle"
platform = "linux"
command = "rtk cargo test -p hawk2ui-build duplicate"
evidence = "target/release-evidence/duplicate.txt"
release_gate = true
"#;

        let error = PackageTargets::parse(input).expect_err("duplicate target IDs must fail");

        assert_eq!(
            error,
            PackageTargetsError::DuplicateTarget("desktop-linux".into())
        );
    }

    #[test]
    fn repository_changelog_has_required_release_sections() {
        let changelog = Changelog::parse(include_str!("../../CHANGELOG.md"))
            .expect("repository changelog must parse");

        for section in [
            "Added",
            "Changed",
            "Fixed",
            "Security",
            "Compatibility",
            "Migration",
            "Known Limitations",
        ] {
            assert!(
                changelog.has_section(section),
                "missing changelog section {section}"
            );
        }

        assert!(changelog.has_verification_evidence());
    }

    #[test]
    fn rejects_changelog_without_verification_evidence() {
        let input = r#"
# Changelog

## 0.1.0 - 2026-05-22

### Added

- Initial release.
"#;

        let error = Changelog::parse(input).expect_err("missing evidence must fail");

        assert_eq!(error, ChangelogError::MissingVerificationEvidence);
    }

    #[test]
    fn release_documentation_lists_required_commands() {
        let checklist = include_str!("../../release/checklist.md");
        let manual = include_str!("../../docs/development/releasing.md");

        for command in [
            "rtk bash scripts/release-check.sh",
            "rtk bash scripts/release-check.sh --version-only",
            "rtk bash scripts/release-check.sh --packages-only",
            "rtk bash scripts/release-check.sh --changelog-only",
        ] {
            assert!(checklist.contains(command), "checklist missing {command}");
            assert!(manual.contains(command), "manual missing {command}");
        }
    }
}

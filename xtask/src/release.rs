#![allow(dead_code)]

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde::Deserialize;

const REQUIRED_RUNTIME_BUNDLE_EVIDENCE: &[&str] = &[
    "embedded-deno-runtime",
    "rusty-v8-static-archive",
    "rusty-v8-source-binding",
    "sealed-js-module-graph",
    "runtime-assets",
    "package-manager-metadata",
    "lockfile-hash",
    "dependency-graph-metadata",
    "sealed-module-dependency-origin",
    "sealed-module-source-map-hash",
    "sealed-module-entrypoint",
    "sealed-module-import-metadata",
    "bundle-content-hash",
];

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
        ReleaseCheckMode::PackagesOnly => {
            validate_repository_package_targets()?;
            crate::npm_packages::verify_generated_packages()
        }
        ReleaseCheckMode::ChangelogOnly => validate_repository_changelog(),
        ReleaseCheckMode::Full => run_full_release_check(),
    }
}

fn run_full_release_check() -> Result<(), String> {
    validate_repository_release_criteria()?;
    validate_repository_version_policy()?;
    validate_repository_package_targets()?;
    validate_repository_dependency_policy()?;
    validate_repository_release_evidence()?;
    validate_repository_changelog()?;
    run_repository_release_evidence_commands()
}

fn run_repository_release_evidence_commands() -> Result<(), String> {
    let criteria = ReleaseCriteria::parse(include_str!("../../release/release-criteria.toml"))
        .map_err(|error| format!("release criteria validation failed: {error:?}"))?;
    let targets = PackageTargets::parse(include_str!("../../release/package-targets.toml"))
        .map_err(|error| format!("package target validation failed: {error:?}"))?;
    let commands = release_evidence_commands(&criteria, &targets);
    run_release_evidence_commands_in_workspace(&workspace_root(), &commands)
}

fn workspace_root() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .map_or_else(|| manifest_dir.to_path_buf(), Path::to_path_buf)
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

fn validate_repository_release_evidence() -> Result<(), String> {
    ReleaseEvidence::parse(
        include_str!("../../README.md"),
        include_str!("../../manual/SUMMARY.md"),
        include_str!("../../manual/packaging.md"),
        include_str!("../../Cargo.toml"),
        include_str!("../../release/release-criteria.toml"),
        include_str!("../../release/package-targets.toml"),
    )
    .map(|_| ())
    .map_err(|error| format!("release evidence validation failed: {error:?}"))
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct ReleaseEvidenceCommand {
    kind: &'static str,
    id: String,
    command: String,
    evidence: String,
}

impl ReleaseEvidenceCommand {
    fn new(
        kind: &'static str,
        id: impl Into<String>,
        command: impl Into<String>,
        evidence: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            id: id.into(),
            command: command.into(),
            evidence: evidence.into(),
        }
    }
}

fn release_evidence_commands(
    criteria: &ReleaseCriteria,
    targets: &PackageTargets,
) -> Vec<ReleaseEvidenceCommand> {
    let mut commands: Vec<_> = criteria
        .release_blockers()
        .map(|criterion| {
            ReleaseEvidenceCommand::new(
                "criterion",
                criterion.id.clone(),
                criterion.command.clone(),
                criterion.evidence.clone(),
            )
        })
        .collect();
    commands.extend(targets.release_blockers().map(|target| {
        ReleaseEvidenceCommand::new(
            "target",
            target.id.clone(),
            target.command.clone(),
            target.evidence.clone(),
        )
    }));
    commands
}

fn run_release_evidence_commands_in_workspace(
    workspace: &Path,
    commands: &[ReleaseEvidenceCommand],
) -> Result<(), String> {
    for command in commands {
        run_release_evidence_command(workspace, command)?;
    }
    Ok(())
}

fn run_release_evidence_command(
    workspace: &Path,
    command: &ReleaseEvidenceCommand,
) -> Result<(), String> {
    let output = Command::new("bash")
        .arg("-lc")
        .arg(&command.command)
        .current_dir(workspace)
        .output()
        .map_err(|error| {
            format!(
                "failed to launch release evidence command {} `{}`: {error}",
                command.id, command.command
            )
        })?;
    let evidence_path = workspace.join(&command.evidence);
    if let Some(parent) = evidence_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create release evidence directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let status = if output.status.success() {
        "success"
    } else {
        "failure"
    };
    let exit_code = output.status.code().map_or_else(
        || "terminated-by-signal".to_owned(),
        |code| code.to_string(),
    );
    let payload = format!(
        "kind={}\nid={}\ncommand={}\nstatus={status}\nexit_code={exit_code}\n\n[stdout]\n{}\n[stderr]\n{}\n",
        command.kind,
        command.id,
        command.command,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    fs::write(&evidence_path, payload).map_err(|error| {
        format!(
            "failed to write release evidence file {}: {error}",
            evidence_path.display()
        )
    })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "release evidence command {} `{}` failed with status {exit_code}; evidence written to {}",
            command.id,
            command.command,
            evidence_path.display()
        ))
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

        // Anchor the declared version to a real compiled crate version, not just to the sibling
        // fields in this file: comparing against `env!("CARGO_PKG_VERSION")` (xtask's own version,
        // which moves with a workspace version bump) makes the gate fail on real policy-vs-crate
        // drift instead of passing whenever the three in-file strings agree. Validating every
        // member crate via `cargo metadata` is the fuller check; this anchors to the compiled
        // version with no new dependency.
        require_matching_version(
            "crate_version",
            &self.crate_version,
            "compiled crate version",
            env!("CARGO_PKG_VERSION"),
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

        for target in &self.targets {
            if target.requires_runtime_bundle_evidence() && !target.has_runtime_bundle_evidence() {
                return Err(PackageTargetsError::MissingRuntimeBundleEvidence(
                    target.id.clone(),
                ));
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
    #[serde(default)]
    runtime_bundle_evidence: Vec<String>,
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

    fn requires_runtime_bundle_evidence(&self) -> bool {
        self.release_gate
            && matches!(
                self.kind.as_str(),
                "desktop-bundle" | "plugin-bundle" | "desktop-smoke" | "plugin-smoke"
            )
    }

    fn has_runtime_bundle_evidence(&self) -> bool {
        REQUIRED_RUNTIME_BUNDLE_EVIDENCE.iter().all(|required| {
            self.runtime_bundle_evidence
                .iter()
                .any(|item| item == required)
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
enum PackageTargetsError {
    Parse(String),
    DuplicateTarget(String),
    MissingRuntimeBundleEvidence(String),
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

/// Sections every release changelog must contain; enforced by [`Changelog::parse`].
const REQUIRED_CHANGELOG_SECTIONS: [&str; 7] = [
    "Added",
    "Changed",
    "Fixed",
    "Security",
    "Compatibility",
    "Migration",
    "Known Limitations",
];

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

        // Enforce the mandated sections in production, not only in tests: `has_section` was
        // previously test-only (silenced by the module `allow(dead_code)`), so `--changelog-only`
        // passed on a changelog with the title + the two evidence substrings but none of the
        // required sections. Checked after the evidence gate to preserve existing error ordering.
        for section in REQUIRED_CHANGELOG_SECTIONS {
            if !changelog.has_section(section) {
                return Err(ChangelogError::MissingSection(section.to_owned()));
            }
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
// The shared `Missing` prefix is intentional — each variant names exactly what the changelog is
// missing — and renaming would churn the existing `MissingTitle`/`MissingVerificationEvidence`
// variants that callers and tests already match on.
#[allow(clippy::enum_variant_names)]
enum ChangelogError {
    MissingTitle,
    MissingSection(String),
    MissingVerificationEvidence,
}

#[derive(Debug)]
struct ReleaseEvidence;

impl ReleaseEvidence {
    fn parse(
        readme: &str,
        manual_summary: &str,
        manual_packaging: &str,
        workspace_manifest: &str,
        release_criteria: &str,
        package_targets: &str,
    ) -> Result<Self, ReleaseEvidenceError> {
        let workspace = WorkspaceManifest::parse(workspace_manifest)?;
        let criteria = ReleaseCriteria::parse(release_criteria)
            .map_err(|error| ReleaseEvidenceError::InvalidReleaseCriteria(format!("{error:?}")))?;
        let targets = PackageTargets::parse(package_targets)
            .map_err(|error| ReleaseEvidenceError::InvalidPackageTargets(format!("{error:?}")))?;
        Self::validate_workspace_crates(&workspace)?;
        Self::validate_public_readme_claims(readme, &targets)?;
        Self::validate_manual_index(manual_summary)?;
        Self::validate_release_criteria(&workspace, &criteria)?;
        Self::validate_package_targets(&workspace, manual_packaging, &targets)?;
        Ok(Self)
    }

    fn validate_workspace_crates(
        workspace: &WorkspaceManifest,
    ) -> Result<(), ReleaseEvidenceError> {
        for package in REQUIRED_PRODUCTION_PACKAGES {
            if !workspace.contains_package(package) {
                return Err(ReleaseEvidenceError::MissingProductionPackage(
                    (*package).to_owned(),
                ));
            }
        }
        Ok(())
    }

    fn validate_public_readme_claims(
        readme: &str,
        targets: &PackageTargets,
    ) -> Result<(), ReleaseEvidenceError> {
        for claim in [
            "desktop",
            "VST3",
            "CLAP",
            "AU",
            "build-release",
            "verify-artifact",
            "package-plugin",
            "cargo run -p xtask -- check-fast",
            "cargo run -p xtask -- check",
        ] {
            if !readme.contains(claim) {
                return Err(ReleaseEvidenceError::MissingReadmeClaim(claim.to_owned()));
            }
        }

        for target_id in [
            "desktop-linux-wayland",
            "desktop-linux-x11",
            "plugin-clap",
            "plugin-vst3",
            "plugin-au",
            "vue-desktop-smoke",
            "vue-plugin-smoke",
        ] {
            if !targets.contains(target_id) {
                return Err(ReleaseEvidenceError::MissingPackageTarget(
                    target_id.to_owned(),
                ));
            }
        }
        Ok(())
    }

    fn validate_manual_index(manual_summary: &str) -> Result<(), ReleaseEvidenceError> {
        for page in [
            "Desktop Apps",
            "Plugin Editors",
            "Runtime APIs",
            "Packaging",
            "Security",
            "Troubleshooting",
        ] {
            if !manual_summary.contains(page) {
                return Err(ReleaseEvidenceError::MissingManualPage(page.to_owned()));
            }
        }
        Ok(())
    }

    fn validate_release_criteria(
        workspace: &WorkspaceManifest,
        criteria: &ReleaseCriteria,
    ) -> Result<(), ReleaseEvidenceError> {
        for criterion in &criteria.criteria {
            Self::validate_evidence_path("criterion", &criterion.id, &criterion.evidence)?;
            if let Some(package) = cargo_package_from_command(&criterion.command)
                && !workspace.contains_package(package.as_str())
            {
                return Err(ReleaseEvidenceError::UnknownCommandPackage {
                    owner_id: criterion.id.clone(),
                    package,
                });
            }
        }
        Ok(())
    }

    fn validate_package_targets(
        workspace: &WorkspaceManifest,
        manual_packaging: &str,
        targets: &PackageTargets,
    ) -> Result<(), ReleaseEvidenceError> {
        for target in &targets.targets {
            Self::validate_evidence_path("target", &target.id, &target.evidence)?;
            if let Some(package) = cargo_package_from_command(&target.command)
                && !workspace.contains_package(package.as_str())
            {
                return Err(ReleaseEvidenceError::UnknownCommandPackage {
                    owner_id: target.id.clone(),
                    package,
                });
            }

            if !manual_packaging.contains(format!("`{}`", target.id).as_str()) {
                return Err(ReleaseEvidenceError::ManualMissingPackageTarget(
                    target.id.clone(),
                ));
            }

            if !manual_packaging.contains(&target.command) {
                return Err(ReleaseEvidenceError::ManualMissingPackageCommand {
                    target_id: target.id.clone(),
                    command: target.command.clone(),
                });
            }

            if !target.release_gate && !manual_packaging.contains("not a release-gated output") {
                return Err(ReleaseEvidenceError::ManualMissingNonGatedTarget(
                    target.id.clone(),
                ));
            }

            if target.requires_runtime_bundle_evidence() {
                for evidence in &target.runtime_bundle_evidence {
                    if !manual_packaging.contains(evidence) {
                        return Err(ReleaseEvidenceError::ManualMissingRuntimeBundleEvidence {
                            target_id: target.id.clone(),
                            evidence: evidence.clone(),
                        });
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_evidence_path(
        kind: &'static str,
        id: &str,
        evidence: &str,
    ) -> Result<(), ReleaseEvidenceError> {
        if evidence.starts_with("target/release-evidence/")
            && std::path::Path::new(evidence)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("txt"))
        {
            Ok(())
        } else {
            Err(ReleaseEvidenceError::InvalidEvidencePath {
                kind,
                id: id.to_owned(),
                evidence: evidence.to_owned(),
            })
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ReleaseEvidenceError {
    InvalidReleaseCriteria(String),
    InvalidPackageTargets(String),
    InvalidWorkspaceManifest(String),
    MissingProductionPackage(String),
    MissingReadmeClaim(String),
    MissingPackageTarget(String),
    MissingManualPage(String),
    UnknownCommandPackage {
        owner_id: String,
        package: String,
    },
    InvalidEvidencePath {
        kind: &'static str,
        id: String,
        evidence: String,
    },
    ManualMissingPackageTarget(String),
    ManualMissingPackageCommand {
        target_id: String,
        command: String,
    },
    ManualMissingNonGatedTarget(String),
    ManualMissingRuntimeBundleEvidence {
        target_id: String,
        evidence: String,
    },
}

#[derive(Debug, Deserialize)]
struct WorkspaceManifest {
    workspace: WorkspaceMembers,
}

impl WorkspaceManifest {
    fn parse(input: &str) -> Result<Self, ReleaseEvidenceError> {
        toml::from_str(input)
            .map_err(|error| ReleaseEvidenceError::InvalidWorkspaceManifest(error.to_string()))
    }

    fn contains_package(&self, package: &str) -> bool {
        self.workspace.members.iter().any(|member| {
            member
                .rsplit('/')
                .next()
                .is_some_and(|name| name == package)
        })
    }
}

#[derive(Debug, Deserialize)]
struct WorkspaceMembers {
    members: Vec<String>,
}

fn cargo_package_from_command(command: &str) -> Option<String> {
    let mut words = command.split_whitespace();
    while let Some(word) = words.next() {
        if word == "-p" || word == "--package" {
            return words.next().map(ToOwned::to_owned);
        }
        if let Some(package) = word
            .strip_prefix("-p=")
            .or_else(|| word.strip_prefix("--package="))
        {
            return Some(package.to_owned());
        }
    }
    None
}

const REQUIRED_PRODUCTION_PACKAGES: &[&str] = &[
    "hawk2ui-a11y",
    "hawk2ui-api",
    "hawk2ui-assets",
    "hawk2ui-authoring",
    "hawk2ui-build",
    "hawk2ui-cli",
    "hawk2ui-compat",
    "hawk2ui-conformance",
    "hawk2ui-framework-conformance",
    "hawk2ui-framework-react",
    "hawk2ui-framework-solid",
    "hawk2ui-framework-svelte",
    "hawk2ui-framework-vue",
    "hawk2ui-host",
    "hawk2ui-host-baseview",
    "hawk2ui-host-winit",
    "hawk2ui-layout",
    "hawk2ui-perf",
    "hawk2ui-platform",
    "hawk2ui-plugin",
    "hawk2ui-plugin-adapters",
    "hawk2ui-plugin-truce",
    "hawk2ui-render",
    "hawk2ui-render-skia",
    "hawk2ui-runtime",
    "hawk2ui-schema",
    "hawk2ui-script",
    "hawk2ui-security",
    "hawk2ui-security-model",
    "hawk2ui-smoke",
    "hawk2ui-style",
    "hawk2ui-testkit",
    "hawk2ui-text",
    "hawk2ui-vst3",
    "xtask",
];

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
            "framework-compilers",
            "react-deno-runtime",
            "vue-deno-runtime",
            "developer-experience",
            "capability-apis",
            "v8-artifact-policy",
            "generated-npm-packages",
            "generated-npm-packages-publish-dry-run",
            "performance-budgets",
            "visual-regression",
            "plugin-realtime-safety",
            "security-gates",
            "smoke-apps",
            "manuals",
            "packaging",
        ] {
            assert!(criteria.contains(id), "missing release criterion {id}");
        }
        let framework_compilers = criteria
            .criteria
            .iter()
            .find(|criterion| criterion.id == "framework-compilers")
            .expect("framework-compilers criterion must exist");
        assert_eq!(
            framework_compilers.blocking,
            BlockingLevel::Advisory,
            "incubating framework compiler examples must not block the React/Deno release"
        );
    }

    #[test]
    fn parses_release_criteria_with_required_fields() {
        let criteria = ReleaseCriteria::parse(VALID_CRITERIA).expect("valid criteria must parse");

        assert_eq!(criteria.criteria.len(), 2);
        assert!(criteria.contains("api-stability"));
        assert!(criteria.release_blockers().all(|criterion| {
            criterion.blocking == BlockingLevel::Release && !criterion.evidence.is_empty()
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
            "desktop-linux-wayland",
            "desktop-linux-x11",
            "desktop-windows",
            "desktop-macos",
            "plugin-clap",
            "plugin-vst3",
            "plugin-au",
            "react-desktop-smoke",
            "react-plugin-smoke",
            "vue-desktop-smoke",
            "vue-plugin-smoke",
            "sealed-artifact",
            "debug-package",
            "release-package",
        ] {
            assert!(targets.contains(id), "missing package target {id}");
        }

        for id in ["plugin-clap", "plugin-vst3", "plugin-au"] {
            let target = targets
                .targets
                .iter()
                .find(|target| target.id == id)
                .unwrap_or_else(|| panic!("{id} remains a tracked release-gated target"));
            assert!(target.release_gate, "{id} must be a release-gated target");
        }
        assert!(targets.release_blockers().all(|target| target.release_gate));

        for id in [
            "desktop-linux-wayland",
            "desktop-linux-x11",
            "desktop-windows",
            "desktop-macos",
            "plugin-clap",
            "plugin-vst3",
            "plugin-au",
            "react-desktop-smoke",
            "react-plugin-smoke",
            "vue-desktop-smoke",
            "vue-plugin-smoke",
        ] {
            let target = targets
                .targets
                .iter()
                .find(|target| target.id == id)
                .unwrap_or_else(|| panic!("missing package target {id}"));
            assert!(
                target.has_runtime_bundle_evidence(),
                "{id} must record embedded runtime and sealed module graph evidence"
            );
            for evidence in [
                "package-manager-metadata",
                "lockfile-hash",
                "dependency-graph-metadata",
                "sealed-module-dependency-origin",
                "sealed-module-source-map-hash",
                "sealed-module-entrypoint",
                "sealed-module-import-metadata",
                "bundle-content-hash",
            ] {
                assert!(
                    target
                        .runtime_bundle_evidence
                        .iter()
                        .any(|item| item == evidence),
                    "{id} must record {evidence} release evidence"
                );
            }
        }
    }

    #[test]
    fn repository_dependency_policy_tracks_release_blocking_dependency_risks() {
        let policy = DependencyPolicy::parse(include_str!("../../release/dependency-policy.toml"))
            .expect("repository dependency policy must parse");

        for dependency in [
            "deno_core",
            "v8",
            "parley",
            "oxc_allocator",
            "lightningcss",
            "taffy",
            "skia-safe",
            "notify",
            "truce",
            "truce-core",
            "truce-params",
        ] {
            assert!(
                policy.contains(dependency),
                "missing dependency policy entry for {dependency}"
            );
        }
        assert!(
            policy
                .dependencies
                .iter()
                .any(|entry| entry.name == "deno_core"
                    && entry.source == DependencySource::CratesIo
                    && !entry.release_blocker),
            "Deno core must use a crates.io dependency contract"
        );
        assert!(
            policy
                .release_blockers()
                .all(|entry| entry.source != DependencySource::Git),
            "Git dependencies must be removed or isolated before release"
        );
        for dependency in ["truce", "truce-core", "truce-params"] {
            let entry = policy
                .dependencies
                .iter()
                .find(|entry| entry.name == dependency)
                .unwrap_or_else(|| panic!("missing dependency policy entry for {dependency}"));
            assert_eq!(
                entry.version, "0.56.0",
                "{dependency} must track the accepted truce.audio 0.56.0 line"
            );
        }
        let v8 = policy
            .dependencies
            .iter()
            .find(|entry| entry.name == "v8")
            .expect("missing dependency policy entry for v8");
        assert!(
            v8.upgrade_gate
                .contains("cargo test -p hawk2ui-js-runtime --test v8_artifacts -- --nocapture"),
            "v8 upgrade gate must run the v8_artifacts integration test target"
        );
    }

    #[test]
    fn ci_workflow_caches_native_binary_release_dependencies() {
        let workflow = include_str!("../../.github/workflows/ci.yml");

        for cache_path in [
            "~/.cargo/registry",
            "~/.cargo/git",
            "~/.cargo/.rusty_v8",
            "target/*/build/skia-bindings-*/out",
            "target/*/build/v8-*/out",
        ] {
            assert!(
                workflow.contains(cache_path),
                "CI workflow must cache {cache_path}"
            );
        }

        for cache_key in ["Cargo.lock", "rusty-v8-skia", "${{ runner.os }}"] {
            assert!(
                workflow.contains(cache_key),
                "CI workflow cache key must include {cache_key}"
            );
        }
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
    fn rejects_release_gated_package_without_runtime_bundle_evidence() {
        let input = r#"
[[targets]]
id = "desktop-linux-wayland"
kind = "desktop-bundle"
platform = "linux-wayland"
command = "rtk cargo test -p hawk2ui-build package_desktop_linux"
evidence = "target/release-evidence/desktop-linux-wayland.txt"
release_gate = true
"#;

        let error =
            PackageTargets::parse(input).expect_err("runtime bundle evidence must be required");

        assert_eq!(
            error,
            PackageTargetsError::MissingRuntimeBundleEvidence("desktop-linux-wayland".into())
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
        let input = r"
# Changelog

## 0.1.0 - 2026-05-22

### Added

- Initial release.
";

        let error = Changelog::parse(input).expect_err("missing evidence must fail");

        assert_eq!(error, ChangelogError::MissingVerificationEvidence);
    }

    #[test]
    fn release_documentation_lists_required_commands() {
        let checklist = include_str!("../../release/checklist.md");

        for command in [
            "rtk bash scripts/release-check.sh",
            "rtk bash scripts/release-check.sh --version-only",
            "rtk bash scripts/release-check.sh --packages-only",
            "rtk bash scripts/release-check.sh --changelog-only",
        ] {
            assert!(checklist.contains(command), "checklist missing {command}");
        }
    }

    #[test]
    fn repository_release_evidence_links_public_claims_to_release_metadata() {
        ReleaseEvidence::parse(
            include_str!("../../README.md"),
            include_str!("../../manual/SUMMARY.md"),
            include_str!("../../manual/packaging.md"),
            include_str!("../../Cargo.toml"),
            include_str!("../../release/release-criteria.toml"),
            include_str!("../../release/package-targets.toml"),
        )
        .expect("repository release evidence must link public claims to release metadata");
    }

    #[test]
    fn release_evidence_command_writes_success_artifact_from_real_command() {
        let root = temp_release_root("success");
        let command = ReleaseEvidenceCommand::new(
            "criterion",
            "api-stability",
            "printf 'proof\\n'",
            "target/release-evidence/api-stability.txt",
        );

        run_release_evidence_commands_in_workspace(&root, &[command])
            .expect("successful evidence command should pass");

        let evidence =
            std::fs::read_to_string(root.join("target/release-evidence/api-stability.txt"))
                .expect("evidence file should be written");
        assert!(evidence.contains("kind=criterion"));
        assert!(evidence.contains("id=api-stability"));
        assert!(evidence.contains("command=printf 'proof\\n'"));
        assert!(evidence.contains("status=success"));
        assert!(evidence.contains("proof"));
    }

    #[test]
    fn release_evidence_command_writes_failure_artifact_and_returns_error() {
        let root = temp_release_root("failure");
        let command = ReleaseEvidenceCommand::new(
            "target",
            "plugin-vst3",
            "printf 'bad\\n'; exit 7",
            "target/release-evidence/plugin-vst3.txt",
        );

        let error = run_release_evidence_commands_in_workspace(&root, &[command])
            .expect_err("failing evidence command should fail release evidence");

        assert!(error.contains("plugin-vst3"));
        let evidence =
            std::fs::read_to_string(root.join("target/release-evidence/plugin-vst3.txt"))
                .expect("failure evidence file should be written");
        assert!(evidence.contains("kind=target"));
        assert!(evidence.contains("id=plugin-vst3"));
        assert!(evidence.contains("status=failure"));
        assert!(evidence.contains("bad"));
    }

    #[test]
    fn rejects_manual_package_command_drift_from_release_targets() {
        let manual = r"
# Hawk2UI Packaging

## Package Outputs

- `desktop-linux-wayland`: `rtk cargo test -p hawk2ui-build package_desktop_linux`
- `desktop-linux-x11`: `rtk cargo test -p hawk2ui-build package_desktop_linux`
- `plugin-clap`: `rtk cargo test -p hawk2ui-plugin-adapters stale_command`

    Runtime bundle evidence: `embedded-deno-runtime`, `rusty-v8-static-archive`, `rusty-v8-source-binding`, `sealed-js-module-graph`, `runtime-assets`, `package-manager-metadata`, `lockfile-hash`, `dependency-graph-metadata`, `sealed-module-dependency-origin`, `sealed-module-source-map-hash`, `sealed-module-entrypoint`, `sealed-module-import-metadata`, `bundle-content-hash`.
";

        let error = ReleaseEvidence::parse(
            "# Hawk2UI\n\ndesktop VST3 CLAP AU build-release verify-artifact package-plugin cargo run -p xtask -- check-fast cargo run -p xtask -- check",
            "- [Desktop Apps](desktop-apps.md)\n- [Plugin Editors](plugin-editors.md)\n- [Runtime APIs](runtime-apis.md)\n- [Packaging](packaging.md)\n- [Security](security.md)\n- [Troubleshooting](troubleshooting.md)",
            manual,
            include_str!("../../Cargo.toml"),
            VALID_CRITERIA,
            r#"
[[targets]]
id = "desktop-linux-wayland"
kind = "desktop-bundle"
platform = "linux-wayland"
command = "rtk cargo test -p hawk2ui-build package_desktop_linux"
evidence = "target/release-evidence/desktop-linux-wayland.txt"
release_gate = true
  runtime_bundle_evidence = ["embedded-deno-runtime", "rusty-v8-static-archive", "rusty-v8-source-binding", "sealed-js-module-graph", "runtime-assets", "package-manager-metadata", "lockfile-hash", "dependency-graph-metadata", "sealed-module-dependency-origin", "sealed-module-source-map-hash", "sealed-module-entrypoint", "sealed-module-import-metadata", "bundle-content-hash"]

[[targets]]
id = "desktop-linux-x11"
kind = "desktop-bundle"
platform = "linux-x11"
command = "rtk cargo test -p hawk2ui-build package_desktop_linux"
evidence = "target/release-evidence/desktop-linux-x11.txt"
release_gate = true
  runtime_bundle_evidence = ["embedded-deno-runtime", "rusty-v8-static-archive", "rusty-v8-source-binding", "sealed-js-module-graph", "runtime-assets", "package-manager-metadata", "lockfile-hash", "dependency-graph-metadata", "sealed-module-dependency-origin", "sealed-module-source-map-hash", "sealed-module-entrypoint", "sealed-module-import-metadata", "bundle-content-hash"]

[[targets]]
id = "plugin-clap"
kind = "plugin-bundle"
platform = "cross-platform"
command = "rtk cargo test -p hawk2ui-plugin-adapters package_clap"
evidence = "target/release-evidence/plugin-clap.txt"
release_gate = true
  runtime_bundle_evidence = ["embedded-deno-runtime", "rusty-v8-static-archive", "rusty-v8-source-binding", "sealed-js-module-graph", "runtime-assets", "package-manager-metadata", "lockfile-hash", "dependency-graph-metadata", "sealed-module-dependency-origin", "sealed-module-source-map-hash", "sealed-module-entrypoint", "sealed-module-import-metadata", "bundle-content-hash"]

[[targets]]
id = "plugin-vst3"
kind = "plugin-bundle"
platform = "cross-platform"
command = "rtk cargo test -p hawk2ui-plugin-adapters package_vst3"
evidence = "target/release-evidence/plugin-vst3.txt"
release_gate = false

[[targets]]
id = "plugin-au"
kind = "plugin-bundle"
platform = "macos"
command = "rtk cargo test -p hawk2ui-plugin-adapters package_au"
evidence = "target/release-evidence/plugin-au.txt"
release_gate = true
  runtime_bundle_evidence = ["embedded-deno-runtime", "rusty-v8-static-archive", "rusty-v8-source-binding", "sealed-js-module-graph", "runtime-assets", "package-manager-metadata", "lockfile-hash", "dependency-graph-metadata", "sealed-module-dependency-origin", "sealed-module-source-map-hash", "sealed-module-entrypoint", "sealed-module-import-metadata", "bundle-content-hash"]

[[targets]]
id = "vue-desktop-smoke"
kind = "desktop-smoke"
platform = "linux"
command = "rtk cargo test -p hawk2ui-smoke vue_desktop_basic_runs_sealed_deno_graph_through_winit_smoke"
evidence = "target/release-evidence/vue-desktop-smoke.txt"
release_gate = true
  runtime_bundle_evidence = ["embedded-deno-runtime", "rusty-v8-static-archive", "rusty-v8-source-binding", "sealed-js-module-graph", "runtime-assets", "package-manager-metadata", "lockfile-hash", "dependency-graph-metadata", "sealed-module-dependency-origin", "sealed-module-source-map-hash", "sealed-module-entrypoint", "sealed-module-import-metadata", "bundle-content-hash"]

[[targets]]
id = "vue-plugin-smoke"
kind = "plugin-smoke"
platform = "linux"
command = "rtk cargo test -p hawk2ui-smoke vue_plugin_basic_runs_deno_ui_parameters_and_realtime_denial"
evidence = "target/release-evidence/vue-plugin-smoke.txt"
release_gate = true
  runtime_bundle_evidence = ["embedded-deno-runtime", "rusty-v8-static-archive", "rusty-v8-source-binding", "sealed-js-module-graph", "runtime-assets", "package-manager-metadata", "lockfile-hash", "dependency-graph-metadata", "sealed-module-dependency-origin", "sealed-module-source-map-hash", "sealed-module-entrypoint", "sealed-module-import-metadata", "bundle-content-hash"]
"#,
        )
        .expect_err("manual package command drift must fail");

        assert_eq!(
            error,
            ReleaseEvidenceError::ManualMissingPackageCommand {
                target_id: "plugin-clap".to_owned(),
                command: "rtk cargo test -p hawk2ui-plugin-adapters package_clap".to_owned(),
            }
        );
    }

    #[test]
    fn rejects_manual_runtime_bundle_evidence_drift_from_release_targets() {
        let manual = r"
# Hawk2UI Packaging

## Package Outputs

- `desktop-linux-wayland`: `rtk cargo test -p hawk2ui-build package_desktop_linux`
- `desktop-linux-x11`: `rtk cargo test -p hawk2ui-build package_desktop_linux`
- `plugin-clap`: `rtk cargo test -p hawk2ui-plugin-adapters package_clap`
- `plugin-vst3`: `rtk cargo test -p hawk2ui-plugin-adapters package_vst3`
- `plugin-au`: `rtk cargo test -p hawk2ui-plugin-adapters package_au`
";

        let error = ReleaseEvidence::parse(
            "# Hawk2UI\n\ndesktop VST3 CLAP AU build-release verify-artifact package-plugin cargo run -p xtask -- check-fast cargo run -p xtask -- check",
            "- [Desktop Apps](desktop-apps.md)\n- [Plugin Editors](plugin-editors.md)\n- [Runtime APIs](runtime-apis.md)\n- [Packaging](packaging.md)\n- [Security](security.md)\n- [Troubleshooting](troubleshooting.md)",
            manual,
            include_str!("../../Cargo.toml"),
            VALID_CRITERIA,
            r#"
[[targets]]
id = "desktop-linux-wayland"
kind = "desktop-bundle"
platform = "linux-wayland"
command = "rtk cargo test -p hawk2ui-build package_desktop_linux"
evidence = "target/release-evidence/desktop-linux-wayland.txt"
release_gate = true
  runtime_bundle_evidence = ["embedded-deno-runtime", "rusty-v8-static-archive", "rusty-v8-source-binding", "sealed-js-module-graph", "runtime-assets", "package-manager-metadata", "lockfile-hash", "dependency-graph-metadata", "sealed-module-dependency-origin", "sealed-module-source-map-hash", "sealed-module-entrypoint", "sealed-module-import-metadata", "bundle-content-hash"]

[[targets]]
id = "desktop-linux-x11"
kind = "desktop-bundle"
platform = "linux-x11"
command = "rtk cargo test -p hawk2ui-build package_desktop_linux"
evidence = "target/release-evidence/desktop-linux-x11.txt"
release_gate = true
  runtime_bundle_evidence = ["embedded-deno-runtime", "rusty-v8-static-archive", "rusty-v8-source-binding", "sealed-js-module-graph", "runtime-assets", "package-manager-metadata", "lockfile-hash", "dependency-graph-metadata", "sealed-module-dependency-origin", "sealed-module-source-map-hash", "sealed-module-entrypoint", "sealed-module-import-metadata", "bundle-content-hash"]

[[targets]]
id = "plugin-clap"
kind = "plugin-bundle"
platform = "cross-platform"
command = "rtk cargo test -p hawk2ui-plugin-adapters package_clap"
evidence = "target/release-evidence/plugin-clap.txt"
release_gate = true
  runtime_bundle_evidence = ["embedded-deno-runtime", "rusty-v8-static-archive", "rusty-v8-source-binding", "sealed-js-module-graph", "runtime-assets", "package-manager-metadata", "lockfile-hash", "dependency-graph-metadata", "sealed-module-dependency-origin", "sealed-module-source-map-hash", "sealed-module-entrypoint", "sealed-module-import-metadata", "bundle-content-hash"]

[[targets]]
id = "plugin-vst3"
kind = "plugin-bundle"
platform = "cross-platform"
command = "rtk cargo test -p hawk2ui-plugin-adapters package_vst3"
evidence = "target/release-evidence/plugin-vst3.txt"
release_gate = true
  runtime_bundle_evidence = ["embedded-deno-runtime", "rusty-v8-static-archive", "rusty-v8-source-binding", "sealed-js-module-graph", "runtime-assets", "package-manager-metadata", "lockfile-hash", "dependency-graph-metadata", "sealed-module-dependency-origin", "sealed-module-source-map-hash", "sealed-module-entrypoint", "sealed-module-import-metadata", "bundle-content-hash"]

[[targets]]
id = "plugin-au"
kind = "plugin-bundle"
platform = "macos"
command = "rtk cargo test -p hawk2ui-plugin-adapters package_au"
evidence = "target/release-evidence/plugin-au.txt"
release_gate = true
  runtime_bundle_evidence = ["embedded-deno-runtime", "rusty-v8-static-archive", "rusty-v8-source-binding", "sealed-js-module-graph", "runtime-assets", "package-manager-metadata", "lockfile-hash", "dependency-graph-metadata", "sealed-module-dependency-origin", "sealed-module-source-map-hash", "sealed-module-entrypoint", "sealed-module-import-metadata", "bundle-content-hash"]

[[targets]]
id = "vue-desktop-smoke"
kind = "desktop-smoke"
platform = "linux"
command = "rtk cargo test -p hawk2ui-smoke vue_desktop_basic_runs_sealed_deno_graph_through_winit_smoke"
evidence = "target/release-evidence/vue-desktop-smoke.txt"
release_gate = true
  runtime_bundle_evidence = ["embedded-deno-runtime", "rusty-v8-static-archive", "rusty-v8-source-binding", "sealed-js-module-graph", "runtime-assets", "package-manager-metadata", "lockfile-hash", "dependency-graph-metadata", "sealed-module-dependency-origin", "sealed-module-source-map-hash", "sealed-module-entrypoint", "sealed-module-import-metadata", "bundle-content-hash"]

[[targets]]
id = "vue-plugin-smoke"
kind = "plugin-smoke"
platform = "linux"
command = "rtk cargo test -p hawk2ui-smoke vue_plugin_basic_runs_deno_ui_parameters_and_realtime_denial"
evidence = "target/release-evidence/vue-plugin-smoke.txt"
release_gate = true
  runtime_bundle_evidence = ["embedded-deno-runtime", "rusty-v8-static-archive", "rusty-v8-source-binding", "sealed-js-module-graph", "runtime-assets", "package-manager-metadata", "lockfile-hash", "dependency-graph-metadata", "sealed-module-dependency-origin", "sealed-module-source-map-hash", "sealed-module-entrypoint", "sealed-module-import-metadata", "bundle-content-hash"]
"#,
        )
        .expect_err("manual runtime bundle evidence drift must fail");

        assert_eq!(
            error,
            ReleaseEvidenceError::ManualMissingRuntimeBundleEvidence {
                target_id: "desktop-linux-wayland".to_owned(),
                evidence: "embedded-deno-runtime".to_owned(),
            }
        );
    }

    #[test]
    fn rejects_policy_version_diverging_from_compiled_crate_version() {
        // The three in-file version fields agree with each other but not with the real compiled
        // crate version, so the gate must reject it (the former tautology accepted it).
        let input = r#"
crate_version = "9.9.9"
artifact_schema_version = 1
package_version = "9.9.9"
manual_version = "9.9.9"
compatibility_notes_required = true
"#;

        let error = VersionPolicy::parse(input)
            .expect_err("a policy version diverging from the compiled crate version must fail");

        assert!(matches!(
            error,
            VersionPolicyError::MismatchedVersion { .. }
        ));
    }

    #[test]
    fn rejects_changelog_missing_required_sections() {
        // Has the title and verification evidence, but omits the mandated sections beyond `Added`.
        let input = "\
# Changelog

## 0.1.0 - 2026-05-22

### Added

- Initial release.

Verification Evidence: target/release-evidence/
";

        let error =
            Changelog::parse(input).expect_err("a changelog missing required sections must fail");

        assert_eq!(error, ChangelogError::MissingSection("Changed".to_owned()));
    }

    fn temp_release_root(label: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "hawk2ui-xtask-release-evidence-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("temp release root should be created");
        root
    }
}

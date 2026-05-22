#![allow(dead_code)]

use std::collections::HashSet;

use serde::Deserialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReleaseCheckMode {
    Full,
    VersionOnly,
}

pub(crate) fn run_release_check(mode: ReleaseCheckMode) -> Result<(), String> {
    match mode {
        ReleaseCheckMode::VersionOnly => validate_repository_version_policy(),
        ReleaseCheckMode::Full => Err("full release check is not wired yet".into()),
    }
}

fn validate_repository_version_policy() -> Result<(), String> {
    VersionPolicy::parse(include_str!("../../release/version-policy.toml"))
        .map(|_| ())
        .map_err(|error| format!("version policy validation failed: {error:?}"))
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
}

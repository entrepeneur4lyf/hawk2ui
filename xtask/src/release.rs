#![allow(dead_code)]

use std::collections::HashSet;

use serde::Deserialize;

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
}

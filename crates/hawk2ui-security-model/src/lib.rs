#![forbid(unsafe_code)]
//! Threat model registry, security rejection cases, and package trust validation for `Hawk2UI`.

use std::collections::BTreeSet;

use serde::Deserialize;

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-security-model";

/// Returns the canonical Cargo package name for diagnostics and conformance checks.
#[must_use]
pub const fn crate_name() -> &'static str {
    CRATE_NAME
}

/// Threat severity used by the release-blocking threat registry.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    /// High severity threat.
    High,
    /// Critical severity threat.
    Critical,
}

/// One machine-readable threat record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct Threat {
    /// Stable threat identifier.
    pub id: String,
    /// Threat severity.
    pub severity: Severity,
    /// Primary affected domain.
    pub affected_domain: String,
    /// Required mitigation.
    pub mitigation: String,
    /// Required test or fixture identifier.
    pub required_test: String,
}

impl Threat {
    fn require_field(&self, field: &'static str, value: &str) -> Result<(), ThreatModelError> {
        if value.trim().is_empty() {
            Err(ThreatModelError::MissingRequiredField {
                id: self.id.clone(),
                field,
            })
        } else {
            Ok(())
        }
    }
}

/// Machine-readable threat model registry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThreatModel {
    /// Threat rows.
    pub threats: Vec<Threat>,
}

#[derive(Debug, Deserialize)]
struct RawThreatModel {
    threats: Vec<Threat>,
}

impl ThreatModel {
    /// Parses and validates a threat registry from TOML.
    ///
    /// # Errors
    ///
    /// Returns [`ThreatModelError`] when parsing fails, IDs are duplicated,
    /// or required fields are empty.
    pub fn parse(input: &str) -> Result<Self, ThreatModelError> {
        let raw: RawThreatModel =
            toml::from_str(input).map_err(|error| ThreatModelError::Parse(error.to_string()))?;
        let model = Self {
            threats: raw.threats,
        };
        model.validate()?;
        Ok(model)
    }

    /// Returns true when a threat ID exists.
    #[must_use]
    pub fn contains(&self, id: &str) -> bool {
        self.threats.iter().any(|threat| threat.id == id)
    }

    fn validate(&self) -> Result<(), ThreatModelError> {
        let mut ids = BTreeSet::new();
        for threat in &self.threats {
            threat.require_field("id", &threat.id)?;
            threat.require_field("affected_domain", &threat.affected_domain)?;
            threat.require_field("mitigation", &threat.mitigation)?;
            threat.require_field("required_test", &threat.required_test)?;

            if !ids.insert(threat.id.clone()) {
                return Err(ThreatModelError::DuplicateThreat(threat.id.clone()));
            }
        }
        Ok(())
    }
}

/// Threat model validation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ThreatModelError {
    /// TOML parsing failed.
    Parse(String),
    /// Two or more threats share the same ID.
    DuplicateThreat(String),
    /// Required field is empty.
    MissingRequiredField {
        /// Threat ID.
        id: String,
        /// Missing field name.
        field: &'static str,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_identity() {
        assert_eq!(crate_name(), "hawk2ui-security-model");
    }
}

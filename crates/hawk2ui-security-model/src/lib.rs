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

/// Allow or deny outcome for a capability fixture.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
pub enum CapabilityVerdict {
    /// Capability use is allowed by policy.
    Allow,
    /// Capability use is denied by policy.
    Deny,
}

/// One capability boundary case.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct CapabilityCase {
    /// Stable case ID.
    pub id: String,
    /// Capability key being exercised.
    pub capability: String,
    /// Expected policy verdict.
    pub verdict: CapabilityVerdict,
    /// Diagnostic rule attached to this case.
    pub diagnostic_rule: String,
    /// Fixture path for this case.
    pub fixture: String,
}

impl CapabilityCase {
    fn require_field(
        &self,
        field: &'static str,
        value: &str,
    ) -> Result<(), CapabilityRejectionsError> {
        if value.trim().is_empty() {
            Err(CapabilityRejectionsError::MissingRequiredField {
                id: self.id.clone(),
                field,
            })
        } else {
            Ok(())
        }
    }
}

/// Capability rejection registry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityRejections {
    /// Capability test cases.
    pub cases: Vec<CapabilityCase>,
}

#[derive(Debug, Deserialize)]
struct RawCapabilityRejections {
    cases: Vec<CapabilityCase>,
}

impl CapabilityRejections {
    /// Parses and validates capability rejection cases from TOML.
    ///
    /// # Errors
    ///
    /// Returns [`CapabilityRejectionsError`] for parse failures, duplicate IDs,
    /// missing fields, or a capability without both allow and deny cases.
    pub fn parse(input: &str) -> Result<Self, CapabilityRejectionsError> {
        let raw: RawCapabilityRejections = toml::from_str(input)
            .map_err(|error| CapabilityRejectionsError::Parse(error.to_string()))?;
        let cases = Self { cases: raw.cases };
        cases.validate()?;
        Ok(cases)
    }

    /// Returns true when a capability has both allow and deny cases.
    #[must_use]
    pub fn has_allow_and_deny(&self, capability: &str) -> bool {
        self.has_verdict(capability, CapabilityVerdict::Allow)
            && self.has_verdict(capability, CapabilityVerdict::Deny)
    }

    fn has_verdict(&self, capability: &str, verdict: CapabilityVerdict) -> bool {
        self.cases
            .iter()
            .any(|case| case.capability == capability && case.verdict == verdict)
    }

    fn validate(&self) -> Result<(), CapabilityRejectionsError> {
        let mut ids = BTreeSet::new();
        let mut capabilities = BTreeSet::new();

        for case in &self.cases {
            case.require_field("id", &case.id)?;
            case.require_field("capability", &case.capability)?;
            case.require_field("diagnostic_rule", &case.diagnostic_rule)?;
            case.require_field("fixture", &case.fixture)?;

            if !ids.insert(case.id.clone()) {
                return Err(CapabilityRejectionsError::DuplicateCase(case.id.clone()));
            }
            capabilities.insert(case.capability.clone());
        }

        for capability in capabilities {
            for verdict in [CapabilityVerdict::Allow, CapabilityVerdict::Deny] {
                if !self.has_verdict(&capability, verdict) {
                    return Err(CapabilityRejectionsError::MissingVerdict {
                        capability,
                        verdict,
                    });
                }
            }
        }

        Ok(())
    }
}

/// Capability rejection registry validation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CapabilityRejectionsError {
    /// TOML parsing failed.
    Parse(String),
    /// Two or more cases share the same ID.
    DuplicateCase(String),
    /// Required field is empty.
    MissingRequiredField {
        /// Case ID.
        id: String,
        /// Missing field.
        field: &'static str,
    },
    /// Capability lacks one required verdict.
    MissingVerdict {
        /// Capability key.
        capability: String,
        /// Missing verdict.
        verdict: CapabilityVerdict,
    },
}

/// One malicious or denied source/asset fixture.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct AttackFixture {
    /// Stable fixture ID.
    pub id: String,
    /// Fixture path.
    pub path: String,
    /// Diagnostic rule expected for this fixture.
    pub diagnostic_rule: String,
}

impl AttackFixture {
    fn require_field(&self, field: &'static str, value: &str) -> Result<(), AttackFixturesError> {
        if value.trim().is_empty() {
            Err(AttackFixturesError::MissingRequiredField {
                id: self.id.clone(),
                field,
            })
        } else {
            Ok(())
        }
    }
}

/// Registry of source and asset attack fixtures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttackFixtures {
    /// Fixture rows.
    pub fixtures: Vec<AttackFixture>,
}

#[derive(Debug, Deserialize)]
struct RawAttackFixtures {
    fixtures: Vec<AttackFixture>,
}

impl AttackFixtures {
    /// Parses and validates source/asset attack fixtures from TOML.
    ///
    /// # Errors
    ///
    /// Returns [`AttackFixturesError`] when parsing fails, IDs are duplicated,
    /// or required fields are empty.
    pub fn parse(input: &str) -> Result<Self, AttackFixturesError> {
        let raw: RawAttackFixtures =
            toml::from_str(input).map_err(|error| AttackFixturesError::Parse(error.to_string()))?;
        let fixtures = Self {
            fixtures: raw.fixtures,
        };
        fixtures.validate()?;
        Ok(fixtures)
    }

    /// Returns a fixture by ID.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&AttackFixture> {
        self.fixtures.iter().find(|fixture| fixture.id == id)
    }

    fn validate(&self) -> Result<(), AttackFixturesError> {
        let mut ids = BTreeSet::new();
        for fixture in &self.fixtures {
            fixture.require_field("id", &fixture.id)?;
            fixture.require_field("path", &fixture.path)?;
            fixture.require_field("diagnostic_rule", &fixture.diagnostic_rule)?;

            if !ids.insert(fixture.id.clone()) {
                return Err(AttackFixturesError::DuplicateFixture(fixture.id.clone()));
            }
        }
        Ok(())
    }
}

/// Source and asset attack fixture validation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttackFixturesError {
    /// TOML parsing failed.
    Parse(String),
    /// Two or more fixtures share the same ID.
    DuplicateFixture(String),
    /// Required field is empty.
    MissingRequiredField {
        /// Fixture ID.
        id: String,
        /// Missing field.
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

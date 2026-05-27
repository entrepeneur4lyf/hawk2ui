#![forbid(unsafe_code)]
//! Threat model registry, security rejection cases, and package trust validation for `Hawk2UI`.

use std::collections::BTreeSet;

use hawk2ui_api::Diagnostic;
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

/// Runtime operation checked against sandbox authority.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RuntimeOperation {
    /// String-to-code execution such as eval or Function constructors.
    StringToCode,
    /// Host API use that was not declared by capability.
    UndeclaredHostApi,
    /// Direct filesystem access.
    DirectFilesystem,
    /// Direct network access.
    DirectNetwork,
    /// Process spawning.
    ProcessSpawn,
    /// Direct native module loading.
    NativeModuleLoading,
}

/// Runtime authority policy for sandbox validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAuthorityPolicy {
    denied: BTreeSet<RuntimeOperation>,
}

impl RuntimeAuthorityPolicy {
    /// Creates the default sandbox policy.
    #[must_use]
    pub fn sandboxed() -> Self {
        Self {
            denied: [
                RuntimeOperation::StringToCode,
                RuntimeOperation::UndeclaredHostApi,
                RuntimeOperation::DirectFilesystem,
                RuntimeOperation::DirectNetwork,
                RuntimeOperation::ProcessSpawn,
                RuntimeOperation::NativeModuleLoading,
            ]
            .into_iter()
            .collect(),
        }
    }

    /// Returns true when an operation is denied.
    #[must_use]
    pub fn is_denied(&self, operation: RuntimeOperation) -> bool {
        self.denied.contains(&operation)
    }

    /// Redacts secret-looking values and executable source payloads from diagnostics.
    #[must_use]
    pub fn redact_diagnostic(&self, diagnostic: &str) -> String {
        diagnostic
            .split_whitespace()
            .map(redact_token)
            .collect::<Vec<_>>()
            .join(" ")
            .replace("Function('return secrets')", "[redacted-source]")
    }
}

fn redact_token(token: &str) -> &str {
    if token.starts_with("sk_") {
        "[redacted-secret]"
    } else {
        token
    }
}

/// Package signature verification state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageSignatureStatus {
    /// Package signature is verified.
    Verified,
    /// Package signature is missing.
    Missing,
    /// Package signature is invalid.
    Invalid,
}

/// Package verification report state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerificationReportStatus {
    /// Verification report is present.
    Present,
    /// Verification report is missing.
    Missing,
}

/// Trust record for a package or sealed artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageTrustRecord {
    /// Artifact schema version.
    pub artifact_schema_version: u32,
    /// Hash of the manifest snapshot.
    pub manifest_snapshot_hash: String,
    /// Hashes for compiled assets.
    pub compiled_asset_hashes: Vec<String>,
    /// Hashes for compiled scripts.
    pub compiled_script_hashes: Vec<String>,
    /// Target metadata identifier.
    pub target_metadata: String,
    /// Package signature status.
    pub signature_status: PackageSignatureStatus,
    /// Verification report status.
    pub verification_report_status: VerificationReportStatus,
}

/// Package trust validator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackageTrustValidator {
    expected_artifact_schema_version: u32,
}

impl PackageTrustValidator {
    /// Creates a validator for an expected artifact schema version.
    #[must_use]
    pub const fn new(expected_artifact_schema_version: u32) -> Self {
        Self {
            expected_artifact_schema_version,
        }
    }

    /// Validates a package trust record.
    ///
    /// # Errors
    ///
    /// Returns [`PackageTrustViolation`] when the record is incomplete, tampered,
    /// unsigned, or missing verification evidence.
    pub fn validate(&self, record: &PackageTrustRecord) -> Result<(), PackageTrustViolation> {
        if record.artifact_schema_version != self.expected_artifact_schema_version {
            return Err(PackageTrustViolation::ArtifactSchemaMismatch {
                expected: self.expected_artifact_schema_version,
                actual: record.artifact_schema_version,
            });
        }

        require_hash(
            "manifest_snapshot_hash",
            &record.manifest_snapshot_hash,
            PackageTrustViolation::MissingManifestSnapshotHash,
        )?;

        if record.compiled_asset_hashes.is_empty() {
            return Err(PackageTrustViolation::MissingCompiledAssetHashes);
        }
        for hash in &record.compiled_asset_hashes {
            require_hash(
                "compiled_asset_hashes",
                hash,
                PackageTrustViolation::MissingCompiledAssetHashes,
            )?;
        }

        if record.compiled_script_hashes.is_empty() {
            return Err(PackageTrustViolation::MissingCompiledScriptHashes);
        }
        for hash in &record.compiled_script_hashes {
            require_hash(
                "compiled_script_hashes",
                hash,
                PackageTrustViolation::MissingCompiledScriptHashes,
            )?;
        }

        if record.target_metadata.trim().is_empty() {
            return Err(PackageTrustViolation::MissingTargetMetadata);
        }

        match record.signature_status {
            PackageSignatureStatus::Verified => {}
            PackageSignatureStatus::Missing => return Err(PackageTrustViolation::MissingSignature),
            PackageSignatureStatus::Invalid => return Err(PackageTrustViolation::InvalidSignature),
        }

        if record.verification_report_status == VerificationReportStatus::Missing {
            return Err(PackageTrustViolation::MissingVerificationReport);
        }

        Ok(())
    }
}

fn require_hash(
    field: &'static str,
    hash: &str,
    violation: PackageTrustViolation,
) -> Result<(), PackageTrustViolation> {
    if hash.trim().is_empty() {
        return Err(violation);
    }
    if is_supported_hash(hash) {
        Ok(())
    } else {
        Err(PackageTrustViolation::InvalidHash {
            field: field.into(),
        })
    }
}

fn is_supported_hash(hash: &str) -> bool {
    let Some(hex) = hash
        .strip_prefix("sha256:")
        .or_else(|| hash.strip_prefix("blake3:"))
    else {
        return false;
    };
    hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Package trust validation failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PackageTrustViolation {
    /// Artifact schema version does not match the expected release schema.
    ArtifactSchemaMismatch {
        /// Expected schema version.
        expected: u32,
        /// Actual schema version.
        actual: u32,
    },
    /// Manifest snapshot hash is missing.
    MissingManifestSnapshotHash,
    /// Compiled asset hashes are missing.
    MissingCompiledAssetHashes,
    /// Compiled script hashes are missing.
    MissingCompiledScriptHashes,
    /// Target metadata is missing.
    MissingTargetMetadata,
    /// Package signature is missing.
    MissingSignature,
    /// Package signature is invalid.
    InvalidSignature,
    /// Verification report is missing.
    MissingVerificationReport,
    /// Hash field is malformed or uses an unsupported algorithm.
    InvalidHash {
        /// Field that carried the invalid hash.
        field: String,
    },
}

impl From<PackageTrustViolation> for Diagnostic {
    fn from(violation: PackageTrustViolation) -> Self {
        match violation {
            PackageTrustViolation::ArtifactSchemaMismatch { expected, actual } => Self::error(
                "security.package.schema-mismatch",
                format!("artifact schema version mismatch: expected {expected}, actual {actual}"),
            ),
            PackageTrustViolation::MissingManifestSnapshotHash => Self::error(
                "security.package.manifest-hash-missing",
                "package trust record is missing the manifest snapshot hash",
            ),
            PackageTrustViolation::MissingCompiledAssetHashes => Self::error(
                "security.package.asset-hashes-missing",
                "package trust record is missing compiled asset hashes",
            ),
            PackageTrustViolation::MissingCompiledScriptHashes => Self::error(
                "security.package.script-hashes-missing",
                "package trust record is missing compiled script hashes",
            ),
            PackageTrustViolation::MissingTargetMetadata => Self::error(
                "security.package.target-metadata-missing",
                "package trust record is missing target metadata",
            ),
            PackageTrustViolation::MissingSignature => Self::error(
                "security.package.signature-missing",
                "package signature is missing",
            ),
            PackageTrustViolation::InvalidSignature => Self::error(
                "security.package.signature-invalid",
                "package signature is invalid",
            ),
            PackageTrustViolation::MissingVerificationReport => Self::error(
                "security.package.verification-report-missing",
                "package verification report is missing",
            ),
            PackageTrustViolation::InvalidHash { field } => Self::error(
                "security.package.hash-invalid",
                format!("package trust hash is invalid: {field}"),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_identity() {
        assert_eq!(crate_name(), "hawk2ui-security-model");
    }
}

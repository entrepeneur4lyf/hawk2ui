#![forbid(unsafe_code)]
//! Security diagnostic evidence records and secret redaction helpers for `Hawk2UI`.
//!
//! The policy-shaped record types in this crate are evidence vocabulary: they label a decision that
//! a concrete validator has already made. Enforcement lives in the owning subsystems such as
//! `hawk2ui-build`, `hawk2ui-assets`, `hawk2ui-script`, and `hawk2ui-platform`.

pub mod assets;
pub mod diagnostic;
pub mod sandbox;
pub mod secrets;
pub mod source;
pub mod trust;

pub use assets::{
    AssetHashVerification, AssetImageMetadataStatus, AssetSecurityPolicy, AssetSecurityRecord,
    AssetSecurityRejection, AssetSecurityRule, VectorSafetyStatus,
};
pub use diagnostic::{SecurityDiagnostic, SecuritySeverity};
pub use sandbox::{ScriptSandboxDenial, ScriptSandboxOperation, ScriptSandboxPolicy};
pub use secrets::{
    SecretDiagnostic, SecretScanFinding, SecretValue, SecretVerificationReport,
    ShippedArtifactSecretCheck,
};
pub use source::{SourceValidationPolicy, SourceValidationRecord, SourceValidationRule};
pub use trust::{TrustBoundary, TrustRecord};

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-security";

/// Returns the canonical Cargo package name for diagnostics and conformance checks.
#[must_use]
pub const fn crate_name() -> &'static str {
    CRATE_NAME
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_identity() {
        assert_eq!(crate_name(), "hawk2ui-security");
    }
}

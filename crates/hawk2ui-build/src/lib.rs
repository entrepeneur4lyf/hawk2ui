#![forbid(unsafe_code)]
//! Manifest validation, build pipeline orchestration, sealed artifacts, package metadata, and verification reports for `Hawk2UI`.

pub mod artifact;
pub mod assets;
pub mod diagnostic;
pub mod manifest;
pub mod pipeline;
pub mod report;
pub mod workspace;

pub use artifact::{
    ARTIFACT_SIGNATURE_ALGORITHM_ED25519_SHA256_V1, ArtifactHash, ArtifactHashes,
    ArtifactSchemaVersion, ArtifactSignature, ArtifactSignaturePolicy, ArtifactSignatureStatus,
    ArtifactSignatureVerificationKey, ArtifactSignatureVerifier, AssetManifestEntry, BuildMetadata,
    CompiledAssetRecord, CompiledScriptRecord, CompiledStyleRecord, SealedArtifact,
    SealedArtifactError, TargetArtifactMetadata,
};
pub use assets::{
    AssetCompilationError, AssetCompilationPlan, AssetCompilationRecord, AssetDimensions,
    AssetKind, AssetPackageMetadata, AssetSanitizationStatus, AssetSource, AssetSourceIndex,
};
pub use diagnostic::{BuildDiagnostic, BuildDiagnosticSeverity, DiagnosticLocation, SourceSpan};
pub use manifest::{HawkManifest, ManifestError, PackageTarget};
pub use pipeline::{BuildPhase, BuildPhaseDiagnostic, BuildPipeline, BuildPipelineError};
pub use report::{PackageTargetRecord, VerificationReport};
pub use workspace::{BuildWorkspace, BuildWorkspaceError, BuildWorkspaceOutput};

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-build";

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
        assert_eq!(crate_name(), "hawk2ui-build");
    }
}

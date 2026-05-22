//! Sealed artifact records and compatibility checks.

use crate::{BuildDiagnostic, BuildDiagnosticSeverity, HawkManifest};

/// Artifact schema version.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactSchemaVersion {
    /// Major schema version.
    pub major: u32,
    /// Minor schema version.
    pub minor: u32,
}

impl ArtifactSchemaVersion {
    /// Creates an artifact schema version.
    #[must_use]
    pub const fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }

    /// Returns true when two versions are schema-compatible.
    #[must_use]
    pub const fn is_compatible_with(self, other: Self) -> bool {
        self.major == other.major
    }
}

/// Stable artifact hash wrapper.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactHash(pub String);

impl ArtifactHash {
    /// Creates a deterministic hash string from bytes.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Self(format!("fnv1a64:{hash:016x}"))
    }
}

/// Sealed artifact record consumed by runtime code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SealedArtifact {
    /// Artifact schema version.
    pub schema_version: ArtifactSchemaVersion,
    /// Hash of the manifest snapshot.
    pub manifest_snapshot_hash: ArtifactHash,
    /// Target metadata names.
    pub target_metadata: Vec<String>,
}

impl SealedArtifact {
    /// Creates a sealed artifact from a validated manifest.
    #[must_use]
    pub fn from_manifest(schema_version: ArtifactSchemaVersion, manifest: &HawkManifest) -> Self {
        Self {
            schema_version,
            manifest_snapshot_hash: ArtifactHash::from_bytes(manifest.snapshot().as_bytes()),
            target_metadata: manifest
                .targets
                .iter()
                .map(|target| target.name.clone())
                .collect(),
        }
    }

    /// Returns true when the artifact schema is compatible with an expected schema.
    #[must_use]
    pub const fn is_compatible_with(&self, expected: ArtifactSchemaVersion) -> bool {
        self.schema_version.is_compatible_with(expected)
    }

    /// Ensures the artifact schema is compatible with an expected schema.
    ///
    /// # Errors
    ///
    /// Returns [`SealedArtifactError`] when the major schema version differs.
    pub fn ensure_compatible_with(
        &self,
        expected: ArtifactSchemaVersion,
    ) -> Result<(), SealedArtifactError> {
        if self.is_compatible_with(expected) {
            Ok(())
        } else {
            Err(SealedArtifactError::IncompatibleSchema {
                expected,
                actual: self.schema_version,
                diagnostic: BuildDiagnostic::new(
                    BuildDiagnosticSeverity::Error,
                    "artifact.schema.incompatible",
                    "sealed artifact schema version is incompatible",
                ),
            })
        }
    }
}

/// Sealed artifact validation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SealedArtifactError {
    /// Artifact schema version is incompatible.
    IncompatibleSchema {
        /// Expected schema version.
        expected: ArtifactSchemaVersion,
        /// Actual schema version.
        actual: ArtifactSchemaVersion,
        /// Structured diagnostic.
        diagnostic: BuildDiagnostic,
    },
}

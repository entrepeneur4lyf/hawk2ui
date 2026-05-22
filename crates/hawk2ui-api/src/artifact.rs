//! Versioned artifact API contracts.

/// Stable artifact identifier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactId(String);

impl ArtifactId {
    /// Creates an artifact identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Hash attached to a compiled artifact or artifact member.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactHash(String);

impl ArtifactHash {
    /// Creates an artifact hash string.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the hash as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Semantic version for sealed artifact schemas.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactSchemaVersion {
    /// Major schema version.
    pub major: u16,
    /// Minor schema version.
    pub minor: u16,
    /// Patch schema version.
    pub patch: u16,
}

impl ArtifactSchemaVersion {
    /// Creates a schema version.
    #[must_use]
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Verifies that this runtime can read an artifact with the provided schema version.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactVersionError`] when the major versions differ or when the
    /// artifact requires a newer minor schema than the runtime supports.
    pub const fn ensure_can_read(self, artifact: Self) -> Result<(), ArtifactVersionError> {
        if self.major != artifact.major || artifact.minor > self.minor {
            Err(ArtifactVersionError {
                runtime: self,
                artifact,
            })
        } else {
            Ok(())
        }
    }
}

/// Error returned when runtime and artifact schema versions are incompatible.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactVersionError {
    /// Runtime-supported schema version.
    pub runtime: ArtifactSchemaVersion,
    /// Artifact schema version.
    pub artifact: ArtifactSchemaVersion,
}

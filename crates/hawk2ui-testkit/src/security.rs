//! Security rejection matrix helpers.

use std::path::{Path, PathBuf};

use crate::SecurityRejection;

/// Required security rejection fixture family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecurityFixtureKind {
    /// Malformed manifest fixture.
    InvalidManifest,
    /// Missing asset fixture.
    MissingAsset,
    /// Unsafe asset fixture.
    UnsafeAsset,
    /// Asset hash mismatch fixture.
    AssetHashMismatch,
    /// Oversized asset fixture.
    OversizedAsset,
    /// Unsupported script fixture.
    UnsupportedScript,
    /// Unsupported style fixture.
    UnsupportedStyle,
}

/// Security rejection fixture metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecurityFixture {
    kind: SecurityFixtureKind,
    path: String,
    diagnostic_rule: String,
}

impl SecurityFixture {
    /// Creates a security fixture metadata record.
    #[must_use]
    pub fn new(
        kind: SecurityFixtureKind,
        path: impl Into<String>,
        diagnostic_rule: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            path: path.into(),
            diagnostic_rule: diagnostic_rule.into(),
        }
    }

    /// Returns the fixture kind.
    #[must_use]
    pub const fn kind(&self) -> SecurityFixtureKind {
        self.kind
    }

    /// Returns the fixture path.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the expected diagnostic rule.
    #[must_use]
    pub fn diagnostic_rule(&self) -> &str {
        &self.diagnostic_rule
    }
}

/// Error returned when security fixture records are invalid.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SecurityRejectionFixtureSetError {
    /// A fixture path does not resolve to a file under the supplied root.
    MissingFixturePath {
        /// Missing fixture path relative to the supplied root.
        path: PathBuf,
    },
}

/// Production baseline of security rejection fixture records.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecurityRejectionFixtureSet {
    fixtures: Vec<SecurityFixture>,
}

impl SecurityRejectionFixtureSet {
    /// Creates a security fixture set.
    #[must_use]
    pub fn new(fixtures: impl IntoIterator<Item = SecurityFixture>) -> Self {
        Self {
            fixtures: fixtures.into_iter().collect(),
        }
    }

    /// Creates the production baseline security fixture set.
    #[must_use]
    pub fn production_baseline() -> Self {
        use SecurityFixtureKind::{
            AssetHashMismatch, InvalidManifest, MissingAsset, OversizedAsset, UnsafeAsset,
            UnsupportedScript, UnsupportedStyle,
        };

        Self::new([
            SecurityFixture::new(
                InvalidManifest,
                "fixtures/security/malformed-manifest.toml",
                "manifest.malformed",
            ),
            SecurityFixture::new(
                MissingAsset,
                "fixtures/security/missing-asset.manifest",
                "asset.missing",
            ),
            SecurityFixture::new(
                UnsafeAsset,
                "fixtures/security/unsafe-vector.svg",
                "asset.vector.unsafe-content",
            ),
            SecurityFixture::new(
                AssetHashMismatch,
                "fixtures/security/hash-mismatch.manifest",
                "asset.hash.mismatch",
            ),
            SecurityFixture::new(
                OversizedAsset,
                "fixtures/security/oversized-asset.manifest",
                "asset.limit.bytes-exceeded",
            ),
            SecurityFixture::new(
                UnsupportedScript,
                "fixtures/security/unsupported-script.ts",
                "script.eval.failed",
            ),
            SecurityFixture::new(
                UnsupportedStyle,
                "fixtures/security/unsupported-style.css",
                "style.property.unknown",
            ),
        ])
    }

    /// Returns all security fixtures.
    #[must_use]
    pub fn fixtures(&self) -> &[SecurityFixture] {
        &self.fixtures
    }

    /// Returns a fixture by kind.
    #[must_use]
    pub fn fixture(&self, kind: SecurityFixtureKind) -> Option<&SecurityFixture> {
        self.fixtures.iter().find(|fixture| fixture.kind == kind)
    }

    /// Verifies that every fixture path resolves to a file below the supplied root.
    ///
    /// # Errors
    ///
    /// Returns [`SecurityRejectionFixtureSetError::MissingFixturePath`] for the
    /// first catalog entry that does not exist.
    pub fn verify_fixture_paths(
        &self,
        root: impl AsRef<Path>,
    ) -> Result<(), SecurityRejectionFixtureSetError> {
        let root = root.as_ref();
        for fixture in &self.fixtures {
            let path = root.join(fixture.path());
            if !path.is_file() {
                return Err(SecurityRejectionFixtureSetError::MissingFixturePath {
                    path: PathBuf::from(fixture.path()),
                });
            }
        }
        Ok(())
    }
}

/// Error returned when a security rejection matrix is incomplete.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SecurityRejectionMatrixError {
    /// A required capability has no rejection case.
    MissingCapability(String),
}

/// Security rejection matrix for capability-boundary tests.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SecurityRejectionMatrix {
    cases: Vec<SecurityRejection>,
}

impl SecurityRejectionMatrix {
    /// Creates a security rejection matrix.
    #[must_use]
    pub fn new(cases: impl IntoIterator<Item = SecurityRejection>) -> Self {
        Self {
            cases: cases.into_iter().collect(),
        }
    }

    /// Returns all rejection cases.
    #[must_use]
    pub fn cases(&self) -> &[SecurityRejection] {
        &self.cases
    }

    /// Verifies every required capability has a rejection case.
    ///
    /// # Errors
    ///
    /// Returns [`SecurityRejectionMatrixError::MissingCapability`] when a
    /// required capability is not covered.
    pub fn require_capabilities(
        &self,
        capabilities: &[&str],
    ) -> Result<(), SecurityRejectionMatrixError> {
        for capability in capabilities {
            if !self
                .cases
                .iter()
                .any(|case| case.capability() == *capability)
            {
                return Err(SecurityRejectionMatrixError::MissingCapability(
                    (*capability).to_string(),
                ));
            }
        }
        Ok(())
    }
}

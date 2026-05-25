//! Security rejection matrix helpers.

use crate::SecurityRejection;

/// Required security rejection fixture family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecurityFixtureKind {
    /// Undeclared capability fixture.
    UndeclaredCapability,
    /// Unsupported source feature fixture.
    UnsupportedSourceFeature,
    /// Unsafe asset fixture.
    UnsafeAsset,
    /// Invalid manifest fixture.
    InvalidManifest,
    /// Denied host API fixture.
    DeniedHostApi,
    /// Secret leak fixture.
    SecretLeak,
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

/// Production baseline of security rejection fixtures.
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
            DeniedHostApi, InvalidManifest, SecretLeak, UndeclaredCapability, UnsafeAsset,
            UnsupportedSourceFeature,
        };

        Self::new([
            SecurityFixture::new(
                UndeclaredCapability,
                "fixtures/security/undeclared-capability.toml",
                "security.capability.denied",
            ),
            SecurityFixture::new(
                UnsupportedSourceFeature,
                "fixtures/security/unsupported-source-feature.hawk",
                "security.source.unsupported-feature",
            ),
            SecurityFixture::new(
                UnsafeAsset,
                "fixtures/security/unsafe-asset.svg",
                "security.asset.unsafe",
            ),
            SecurityFixture::new(
                InvalidManifest,
                "fixtures/security/invalid-manifest.toml",
                "manifest.invalid",
            ),
            SecurityFixture::new(
                DeniedHostApi,
                "fixtures/security/denied-host-api.toml",
                "security.host-api.denied",
            ),
            SecurityFixture::new(
                SecretLeak,
                "fixtures/security/secret-leak.toml",
                "security.secret.leak",
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

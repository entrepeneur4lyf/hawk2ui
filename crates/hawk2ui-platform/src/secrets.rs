//! Capability-scoped secret API records.

use std::fmt;

use crate::PlatformDiagnostic;

/// Platform secret manifest declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformSecretManifest {
    /// Declared secret keys.
    pub declared_keys: Vec<String>,
}

impl PlatformSecretManifest {
    /// Creates a platform secret manifest declaration.
    #[must_use]
    pub fn new(keys: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            declared_keys: keys.into_iter().map(Into::into).collect(),
        }
    }

    fn contains(&self, key: &str) -> bool {
        self.declared_keys.iter().any(|declared| declared == key)
    }
}

/// Redacted secret handle.
#[derive(Clone, Eq, PartialEq)]
pub struct PlatformSecretHandle {
    key: String,
    value: String,
}

impl PlatformSecretHandle {
    /// Returns the stable redaction marker.
    #[must_use]
    pub fn redacted(&self) -> String {
        format!("[REDACTED:{}]", self.key)
    }

    /// Returns true when text does not contain the raw secret value.
    #[must_use]
    pub fn is_absent_from(&self, text: &str) -> bool {
        !text.contains(&self.value)
    }
}

impl fmt::Debug for PlatformSecretHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("PlatformSecretHandle")
            .field(&self.redacted())
            .finish()
    }
}

/// Secret access denial.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformSecretDenied {
    /// Secret key.
    pub key: String,
    /// Structured diagnostic.
    pub diagnostic: PlatformDiagnostic,
}

/// Platform secret policy.
pub struct PlatformSecretPolicy;

impl PlatformSecretPolicy {
    /// Reads a declared secret into a redacted handle.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformSecretDenied`] when the secret key was not declared.
    pub fn read(
        manifest: &PlatformSecretManifest,
        key: &str,
        value: impl Into<String>,
    ) -> Result<PlatformSecretHandle, PlatformSecretDenied> {
        if !is_valid_secret_key(key) {
            return Err(PlatformSecretDenied {
                key: key.into(),
                diagnostic: PlatformDiagnostic::error(
                    "secret.key.invalid",
                    "secret key is structurally invalid",
                ),
            });
        }
        if !manifest.contains(key) {
            return Err(PlatformSecretDenied {
                key: key.into(),
                diagnostic: PlatformDiagnostic::error(
                    "secret.declaration.missing",
                    format!("secret is not declared: {key}"),
                ),
            });
        }
        Ok(PlatformSecretHandle {
            key: key.into(),
            value: value.into(),
        })
    }
}

fn is_valid_secret_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
}

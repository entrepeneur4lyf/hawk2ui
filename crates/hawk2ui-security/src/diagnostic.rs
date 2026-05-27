//! Security diagnostic records.

use hawk2ui_api::Diagnostic;

/// Security diagnostic severity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecuritySeverity {
    /// Release-blocking security error.
    Error,
    /// Non-blocking security warning.
    Warning,
}

/// Structured security diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecurityDiagnostic {
    /// Diagnostic severity.
    pub severity: SecuritySeverity,
    /// Stable diagnostic rule.
    pub rule: String,
    /// Human-readable message.
    pub message: String,
}

impl SecurityDiagnostic {
    /// Creates a security diagnostic.
    #[must_use]
    pub fn new(
        severity: SecuritySeverity,
        rule: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            rule: rule.into(),
            message: message.into(),
        }
    }
}

impl From<SecurityDiagnostic> for Diagnostic {
    fn from(diagnostic: SecurityDiagnostic) -> Self {
        match diagnostic.severity {
            SecuritySeverity::Error => Self::error(diagnostic.rule, diagnostic.message),
            SecuritySeverity::Warning => Self::warning(diagnostic.rule, diagnostic.message),
        }
    }
}

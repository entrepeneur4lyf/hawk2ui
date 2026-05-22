//! Build diagnostic records.

/// Build diagnostic severity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuildDiagnosticSeverity {
    /// Release-blocking error.
    Error,
    /// Non-blocking warning.
    Warning,
}

/// Structured build diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildDiagnostic {
    /// Diagnostic severity.
    pub severity: BuildDiagnosticSeverity,
    /// Stable diagnostic rule.
    pub rule: String,
    /// Human-readable message.
    pub message: String,
}

impl BuildDiagnostic {
    /// Creates a build diagnostic.
    #[must_use]
    pub fn new(
        severity: BuildDiagnosticSeverity,
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

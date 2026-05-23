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
    /// Optional source location.
    pub location: Option<DiagnosticLocation>,
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
            location: None,
        }
    }

    /// Adds a source location.
    #[must_use]
    pub fn with_location(mut self, file_path: impl Into<String>, span: SourceSpan) -> Self {
        self.location = Some(DiagnosticLocation {
            file_path: file_path.into(),
            span,
        });
        self
    }
}

/// Half-open source span.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceSpan {
    /// Start byte offset.
    pub start: usize,
    /// End byte offset.
    pub end: usize,
}

impl SourceSpan {
    /// Creates a source span.
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

/// Diagnostic source location.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticLocation {
    /// Source file path.
    pub file_path: String,
    /// Source span.
    pub span: SourceSpan,
}

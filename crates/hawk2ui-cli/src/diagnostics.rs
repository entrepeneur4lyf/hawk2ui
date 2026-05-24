//! Shared CLI diagnostic rendering.

use serde::{Deserialize, Serialize};

/// Diagnostic severity.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum DiagnosticSeverity {
    /// Warning diagnostic.
    Warning,
    /// Error diagnostic.
    Error,
}

/// Source span using one-based line/column coordinates.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceSpan {
    /// Start line.
    pub start_line: usize,
    /// Start column.
    pub start_column: usize,
    /// End line.
    pub end_line: usize,
    /// End column.
    pub end_column: usize,
}

impl SourceSpan {
    /// Creates a source span.
    #[must_use]
    pub const fn new(
        start_line: usize,
        start_column: usize,
        end_line: usize,
        end_column: usize,
    ) -> Self {
        Self {
            start_line,
            start_column,
            end_line,
            end_column,
        }
    }
}

/// Structured CLI diagnostic.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CliDiagnostic {
    /// Severity.
    pub severity: DiagnosticSeverity,
    /// Optional file path.
    pub file_path: Option<String>,
    /// Optional source span.
    pub span: Option<SourceSpan>,
    /// Stable rule name.
    pub rule: String,
    /// Human-readable message.
    pub message: String,
    /// Suggested fix.
    pub suggested_fix: Option<String>,
    /// Related capability.
    pub related_capability: Option<String>,
    /// Related target.
    pub related_target: Option<String>,
}

impl CliDiagnostic {
    /// Creates a warning diagnostic.
    #[must_use]
    pub fn warning(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(DiagnosticSeverity::Warning, rule, message)
    }

    /// Creates an error diagnostic.
    #[must_use]
    pub fn error(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(DiagnosticSeverity::Error, rule, message)
    }

    /// Creates a capability-denial diagnostic.
    #[must_use]
    pub fn capability_denial(capability: impl Into<String>, message: impl Into<String>) -> Self {
        let capability = capability.into();
        Self::error("capability.denied", message).related_capability(capability)
    }

    /// Creates a target-incompatibility diagnostic.
    #[must_use]
    pub fn target_incompatibility(target: impl Into<String>, message: impl Into<String>) -> Self {
        Self::error("target.incompatible", message).related_target(target)
    }

    /// Sets file path.
    #[must_use]
    pub fn file(mut self, file_path: impl Into<String>) -> Self {
        self.file_path = Some(file_path.into());
        self
    }

    /// Sets source span.
    #[must_use]
    pub const fn span(mut self, span: SourceSpan) -> Self {
        self.span = Some(span);
        self
    }

    /// Sets suggested fix.
    #[must_use]
    pub fn suggested_fix(mut self, suggested_fix: impl Into<String>) -> Self {
        self.suggested_fix = Some(suggested_fix.into());
        self
    }

    /// Sets related capability.
    #[must_use]
    pub fn related_capability(mut self, capability: impl Into<String>) -> Self {
        self.related_capability = Some(capability.into());
        self
    }

    /// Sets related target.
    #[must_use]
    pub fn related_target(mut self, target: impl Into<String>) -> Self {
        self.related_target = Some(target.into());
        self
    }

    /// Renders the diagnostic for CLI output.
    #[must_use]
    pub fn render(&self) -> String {
        let mut parts = vec![format!(
            "{:?}: {}: {}",
            self.severity, self.rule, self.message
        )];
        if let Some(file_path) = &self.file_path {
            if let Some(span) = self.span {
                parts.push(format!(
                    "{file_path}:{}:{}",
                    span.start_line, span.start_column
                ));
            } else {
                parts.push(file_path.clone());
            }
        }
        if let Some(fix) = &self.suggested_fix {
            parts.push(format!("fix={fix}"));
        }
        if let Some(capability) = &self.related_capability {
            parts.push(format!("capability={capability}"));
        }
        if let Some(target) = &self.related_target {
            parts.push(format!("target={target}"));
        }
        parts.join("\n")
    }

    fn new(
        severity: DiagnosticSeverity,
        rule: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            file_path: None,
            span: None,
            rule: rule.into(),
            message: message.into(),
            suggested_fix: None,
            related_capability: None,
            related_target: None,
        }
    }
}

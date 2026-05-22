//! Shared diagnostic contract for build, runtime, packaging, and developer tooling.

/// Severity level for a `Hawk2UI` diagnostic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticSeverity {
    /// Informational diagnostic that does not block work.
    Info,
    /// Warning diagnostic that should be corrected before release.
    Warning,
    /// Error diagnostic that blocks the current operation.
    Error,
}

/// Stable identifier for a validation, runtime, or release rule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuleId(String);

impl RuleId {
    /// Creates a rule identifier from a stable dotted name.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the rule identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Location in an author source file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceSpan {
    /// Source file path as provided by the build graph.
    pub path: String,
    /// One-based starting line.
    pub line: u32,
    /// One-based starting column.
    pub column: u32,
    /// One-based ending line.
    pub end_line: u32,
    /// One-based ending column.
    pub end_column: u32,
}

impl SourceSpan {
    /// Creates a source span with one-based coordinates.
    #[must_use]
    pub fn new(
        path: impl Into<String>,
        line: u32,
        column: u32,
        end_line: u32,
        end_column: u32,
    ) -> Self {
        Self {
            path: path.into(),
            line,
            column,
            end_line,
            end_column,
        }
    }
}

/// Suggested correction attached to a diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuggestedFix {
    /// Human-readable correction message.
    pub message: String,
}

impl SuggestedFix {
    /// Creates a suggested fix.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Additional related context for a diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelatedContext {
    /// Context label, such as a capability or target name.
    pub label: String,
    /// Context value.
    pub value: String,
}

impl RelatedContext {
    /// Creates related diagnostic context.
    #[must_use]
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }
}

/// Shared diagnostic record used by all `Hawk2UI` tooling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    /// Diagnostic severity.
    pub severity: DiagnosticSeverity,
    /// Stable rule identifier.
    pub rule: RuleId,
    /// Human-readable diagnostic message.
    pub message: String,
    /// Optional source span.
    pub source: Option<SourceSpan>,
    /// Suggested fixes.
    pub fixes: Vec<SuggestedFix>,
    /// Related context values.
    pub related: Vec<RelatedContext>,
    /// Whether sensitive values were redacted.
    pub redacted: bool,
}

impl Diagnostic {
    /// Creates an error diagnostic.
    #[must_use]
    pub fn error(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(DiagnosticSeverity::Error, rule, message)
    }

    /// Creates a warning diagnostic.
    #[must_use]
    pub fn warning(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(DiagnosticSeverity::Warning, rule, message)
    }

    /// Creates an informational diagnostic.
    #[must_use]
    pub fn info(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(DiagnosticSeverity::Info, rule, message)
    }

    /// Attaches source location information.
    #[must_use]
    pub fn with_source(mut self, source: SourceSpan) -> Self {
        self.source = Some(source);
        self
    }

    /// Attaches a suggested fix.
    #[must_use]
    pub fn with_fix(mut self, fix: SuggestedFix) -> Self {
        self.fixes.push(fix);
        self
    }

    /// Attaches related context.
    #[must_use]
    pub fn with_related(mut self, related: RelatedContext) -> Self {
        self.related.push(related);
        self
    }

    /// Marks this diagnostic as having redacted sensitive data.
    #[must_use]
    pub const fn redacted(mut self) -> Self {
        self.redacted = true;
        self
    }

    fn new(
        severity: DiagnosticSeverity,
        rule: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            rule: RuleId::new(rule),
            message: message.into(),
            source: None,
            fixes: Vec::new(),
            related: Vec::new(),
            redacted: false,
        }
    }
}

//! Build verification report records.

use crate::{BuildDiagnostic, BuildDiagnosticSeverity, PackageTarget, SourceSpan};

/// Package target entry in a verification report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageTargetRecord {
    /// Package target kind.
    pub target: PackageTarget,
    /// Target name.
    pub name: String,
}

impl PackageTargetRecord {
    /// Creates a package target report record.
    #[must_use]
    pub fn new(target: PackageTarget, name: impl Into<String>) -> Self {
        Self {
            target,
            name: name.into(),
        }
    }
}

/// Verification report emitted by build validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationReport {
    /// Product ID being verified.
    pub product_id: String,
    /// Package targets covered by the report.
    pub package_targets: Vec<PackageTargetRecord>,
    /// Diagnostics emitted by verification.
    pub diagnostics: Vec<BuildDiagnostic>,
}

impl VerificationReport {
    /// Creates an empty verification report.
    #[must_use]
    pub fn new(product_id: impl Into<String>) -> Self {
        Self {
            product_id: product_id.into(),
            package_targets: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    /// Adds a package target record.
    #[must_use]
    pub fn with_package_target(mut self, target: PackageTargetRecord) -> Self {
        self.package_targets.push(target);
        self
    }

    /// Adds a diagnostic.
    #[must_use]
    pub fn with_diagnostic(mut self, diagnostic: BuildDiagnostic) -> Self {
        self.diagnostics.push(diagnostic);
        self
    }

    /// Adds an invalid manifest diagnostic.
    #[must_use]
    pub fn with_invalid_manifest(
        self,
        file_path: impl Into<String>,
        span: SourceSpan,
        message: impl Into<String>,
    ) -> Self {
        self.with_diagnostic(
            BuildDiagnostic::new(BuildDiagnosticSeverity::Error, "manifest.invalid", message)
                .with_location(file_path, span),
        )
    }

    /// Adds an unsupported style diagnostic.
    #[must_use]
    pub fn with_unsupported_style(self, file_path: impl Into<String>, span: SourceSpan) -> Self {
        self.with_diagnostic(
            BuildDiagnostic::new(
                BuildDiagnosticSeverity::Error,
                "style.unsupported",
                "style entrypoint is unsupported",
            )
            .with_location(file_path, span),
        )
    }

    /// Adds an unsupported script diagnostic.
    #[must_use]
    pub fn with_unsupported_script(self, file_path: impl Into<String>, span: SourceSpan) -> Self {
        self.with_diagnostic(
            BuildDiagnostic::new(
                BuildDiagnosticSeverity::Error,
                "script.unsupported",
                "script entrypoint is unsupported",
            )
            .with_location(file_path, span),
        )
    }

    /// Adds an unsafe asset diagnostic.
    #[must_use]
    pub fn with_unsafe_asset(self, file_path: impl Into<String>, span: SourceSpan) -> Self {
        self.with_diagnostic(
            BuildDiagnostic::new(
                BuildDiagnosticSeverity::Error,
                "asset.unsafe",
                "asset failed safety validation",
            )
            .with_location(file_path, span),
        )
    }

    /// Adds a missing asset diagnostic.
    #[must_use]
    pub fn with_missing_asset(self, file_path: impl Into<String>, span: SourceSpan) -> Self {
        self.with_diagnostic(
            BuildDiagnostic::new(
                BuildDiagnosticSeverity::Error,
                "asset.missing",
                "asset source is missing",
            )
            .with_location(file_path, span),
        )
    }

    /// Adds an undeclared capability diagnostic.
    #[must_use]
    pub fn with_undeclared_capability(
        self,
        capability: impl Into<String>,
        span: SourceSpan,
    ) -> Self {
        let capability = capability.into();
        self.with_diagnostic(
            BuildDiagnostic::new(
                BuildDiagnosticSeverity::Error,
                "capability.undeclared",
                format!("capability is not declared: {capability}"),
            )
            .with_location("<manifest>", span),
        )
    }

    /// Adds a target incompatibility diagnostic.
    #[must_use]
    pub fn with_target_incompatibility(self, target: impl Into<String>, span: SourceSpan) -> Self {
        let target = target.into();
        self.with_diagnostic(
            BuildDiagnostic::new(
                BuildDiagnosticSeverity::Error,
                "target.incompatible",
                format!("target is incompatible: {target}"),
            )
            .with_location("<manifest>", span),
        )
    }

    /// Returns true when the report has no error diagnostics.
    #[must_use]
    pub fn is_release_ready(&self) -> bool {
        self.diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != BuildDiagnosticSeverity::Error)
    }

    /// Renders a deterministic plain-text report suitable for golden snapshots.
    #[must_use]
    pub fn render_text(&self) -> String {
        let mut rendered = format!("product: {}\n", self.product_id);
        rendered.push_str("targets:\n");
        for target in &self.package_targets {
            rendered.push_str("- ");
            rendered.push_str(target.target.as_str());
            rendered.push(' ');
            rendered.push_str(&target.name);
            rendered.push('\n');
        }
        rendered.push_str("diagnostics:\n");
        for diagnostic in &self.diagnostics {
            rendered.push_str("- ");
            rendered.push_str(diagnostic.severity.as_str());
            rendered.push(' ');
            rendered.push_str(&diagnostic.rule);
            rendered.push(' ');
            if let Some(location) = &diagnostic.location {
                rendered.push_str(&location.file_path);
                rendered.push(':');
                rendered.push_str(&location.span.start.to_string());
                rendered.push_str("..");
                rendered.push_str(&location.span.end.to_string());
            } else {
                rendered.push_str("<unknown>");
            }
            rendered.push(' ');
            rendered.push_str(&diagnostic.message);
            rendered.push('\n');
        }
        rendered
    }
}

impl PackageTarget {
    fn as_str(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Plugin => "plugin",
        }
    }
}

impl BuildDiagnosticSeverity {
    fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
        }
    }
}

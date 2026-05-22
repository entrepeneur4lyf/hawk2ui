//! Build verification report records.

use crate::{BuildDiagnostic, BuildDiagnosticSeverity, PackageTarget};

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

    /// Returns true when the report has no error diagnostics.
    #[must_use]
    pub fn is_release_ready(&self) -> bool {
        self.diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != BuildDiagnosticSeverity::Error)
    }
}

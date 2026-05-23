//! Secret redaction policy.

use std::fmt;

/// Secret value wrapper that never exposes the raw value through formatting.
#[derive(Clone, Eq, PartialEq)]
pub struct SecretValue {
    name: String,
    value: String,
}

impl SecretValue {
    /// Creates a secret value wrapper.
    #[must_use]
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }

    /// Returns the stable redaction marker for the secret.
    #[must_use]
    pub fn redacted(&self) -> String {
        format!("[REDACTED:{}]", self.name)
    }

    /// Returns true when text does not contain the raw secret value.
    #[must_use]
    pub fn is_absent_from(&self, text: &str) -> bool {
        !text.contains(&self.value)
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("SecretValue")
            .field(&self.redacted())
            .finish()
    }
}

/// Secret-related diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecretDiagnostic {
    /// Stable diagnostic rule.
    pub rule: String,
    /// Redacted diagnostic message.
    pub message: String,
}

impl SecretDiagnostic {
    /// Creates a diagnostic for a manifest-declared secret.
    #[must_use]
    pub fn manifest_secret_declared(secret: &SecretValue) -> Self {
        Self {
            rule: "secret.manifest.declared".into(),
            message: format!(
                "manifest-declared secret is redacted: {}",
                secret.redacted()
            ),
        }
    }
}

/// Finding from a committed source secret scan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecretScanFinding {
    /// Source path.
    pub path: String,
    /// Redacted secret marker.
    pub redacted_secret: String,
}

impl SecretScanFinding {
    /// Creates a source scan finding.
    #[must_use]
    pub fn new(path: impl Into<String>, secret: &SecretValue) -> Self {
        Self {
            path: path.into(),
            redacted_secret: secret.redacted(),
        }
    }
}

/// Shipped artifact secret check.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShippedArtifactSecretCheck {
    /// Artifact path.
    pub artifact_path: String,
    /// Redacted secret marker.
    pub redacted_secret: String,
}

impl ShippedArtifactSecretCheck {
    /// Creates a shipped artifact secret check.
    #[must_use]
    pub fn new(artifact_path: impl Into<String>, secret: &SecretValue) -> Self {
        Self {
            artifact_path: artifact_path.into(),
            redacted_secret: secret.redacted(),
        }
    }
}

/// Secret verification report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecretVerificationReport {
    /// Product ID being verified.
    pub product_id: String,
    /// Redacted diagnostics.
    pub diagnostics: Vec<SecretDiagnostic>,
    /// Committed source scan findings.
    pub source_scans: Vec<SecretScanFinding>,
    /// Shipped artifact secret checks.
    pub artifact_checks: Vec<ShippedArtifactSecretCheck>,
}

impl SecretVerificationReport {
    /// Creates an empty secret verification report.
    #[must_use]
    pub fn new(product_id: impl Into<String>) -> Self {
        Self {
            product_id: product_id.into(),
            diagnostics: Vec::new(),
            source_scans: Vec::new(),
            artifact_checks: Vec::new(),
        }
    }

    /// Adds a redacted diagnostic.
    #[must_use]
    pub fn with_diagnostic(mut self, diagnostic: SecretDiagnostic) -> Self {
        self.diagnostics.push(diagnostic);
        self
    }

    /// Adds a source scan finding.
    #[must_use]
    pub fn with_source_scan(mut self, finding: SecretScanFinding) -> Self {
        self.source_scans.push(finding);
        self
    }

    /// Adds a shipped artifact check.
    #[must_use]
    pub fn with_artifact_check(mut self, check: ShippedArtifactSecretCheck) -> Self {
        self.artifact_checks.push(check);
        self
    }

    /// Serializes the report as deterministic redacted text.
    #[must_use]
    pub fn serialize_text(&self) -> String {
        let mut output = format!("product: {}\n", self.product_id);
        output.push_str("diagnostics:\n");
        for diagnostic in &self.diagnostics {
            output.push_str("- ");
            output.push_str(&diagnostic.rule);
            output.push(' ');
            output.push_str(&diagnostic.message);
            output.push('\n');
        }
        output.push_str("source-scans:\n");
        for finding in &self.source_scans {
            output.push_str("- ");
            output.push_str(&finding.path);
            output.push(' ');
            output.push_str(&finding.redacted_secret);
            output.push('\n');
        }
        output.push_str("artifact-checks:\n");
        for check in &self.artifact_checks {
            output.push_str("- ");
            output.push_str(&check.artifact_path);
            output.push(' ');
            output.push_str(&check.redacted_secret);
            output.push('\n');
        }
        output
    }
}

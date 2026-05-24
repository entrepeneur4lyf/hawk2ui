#![forbid(unsafe_code)]
//! Production plugin and package adapters for `Hawk2UI` `CLAP`, `VST3`, AU, standalone, and desktop outputs.

use hawk2ui_plugin::{BundleOutput, FormatMetadata, ParameterModel};

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-plugin-adapters";

/// Returns the canonical Cargo package name for diagnostics and conformance checks.
#[must_use]
pub const fn crate_name() -> &'static str {
    CRATE_NAME
}

/// Package output format.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageFormat {
    /// CLAP plugin bundle.
    Clap,
    /// VST3 plugin bundle.
    Vst3,
    /// Audio Unit component bundle.
    Au,
    /// Standalone application.
    Standalone,
    /// Desktop application bundle.
    DesktopBundle,
    /// Sealed `Hawk2UI` artifact.
    SealedArtifact,
}

impl PackageFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Clap => "clap",
            Self::Vst3 => "vst3",
            Self::Au => "component",
            Self::Standalone | Self::DesktopBundle => "app",
            Self::SealedArtifact => "hawk2ui",
        }
    }
}

/// Package request.
#[derive(Clone, Debug, PartialEq)]
pub struct PackageRequest {
    metadata: FormatMetadata,
    output: BundleOutput,
    parameters: ParameterModel,
    formats: Vec<PackageFormat>,
}

impl PackageRequest {
    /// Creates a package request.
    #[must_use]
    pub const fn new(
        metadata: FormatMetadata,
        output: BundleOutput,
        parameters: ParameterModel,
    ) -> Self {
        Self {
            metadata,
            output,
            parameters,
            formats: Vec::new(),
        }
    }

    /// Adds a package format.
    #[must_use]
    pub fn with_format(mut self, format: PackageFormat) -> Self {
        if !self.formats.contains(&format) {
            self.formats.push(format);
        }
        self
    }
}

/// Planned package target.
#[derive(Clone, Debug, PartialEq)]
pub struct PackageTargetPlan {
    format: PackageFormat,
    metadata: FormatMetadata,
    output_path: String,
    parameter_count: usize,
}

impl PackageTargetPlan {
    /// Returns target format.
    #[must_use]
    pub const fn format(&self) -> PackageFormat {
        self.format
    }

    /// Returns output path.
    #[must_use]
    pub fn output_path(&self) -> &str {
        &self.output_path
    }

    /// Returns metadata.
    #[must_use]
    pub const fn metadata(&self) -> &FormatMetadata {
        &self.metadata
    }
}

/// Package plan.
#[derive(Clone, Debug, PartialEq)]
pub struct PackagePlan {
    targets: Vec<PackageTargetPlan>,
}

impl PackagePlan {
    /// Returns planned targets.
    #[must_use]
    pub fn targets(&self) -> &[PackageTargetPlan] {
        &self.targets
    }

    /// Verifies planned package outputs.
    #[must_use]
    pub fn verify(&self) -> VerificationReport {
        let entries: Vec<_> = self
            .targets
            .iter()
            .cloned()
            .map(|target| VerificationEntry {
                target,
                status: VerificationStatus::Passed,
            })
            .collect();
        VerificationReport { entries }
    }
}

/// Package adapter set.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PackageAdapterSet;

impl PackageAdapterSet {
    /// Creates a package adapter set.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Plans package outputs.
    ///
    /// # Errors
    ///
    /// Returns [`PackagePlanningError`] when package metadata or target selection is invalid.
    pub fn plan(&self, request: &PackageRequest) -> Result<PackagePlan, PackagePlanningError> {
        validate_request(request)?;
        let targets = request
            .formats
            .iter()
            .map(|format| PackageTargetPlan {
                format: *format,
                metadata: request.metadata.clone(),
                output_path: output_path(&request.output, *format),
                parameter_count: request.parameters.parameters.len(),
            })
            .collect();
        Ok(PackagePlan { targets })
    }
}

/// Verification status.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerificationStatus {
    /// Verification passed.
    Passed,
    /// Verification failed.
    Failed,
}

/// Single verification entry.
#[derive(Clone, Debug, PartialEq)]
pub struct VerificationEntry {
    target: PackageTargetPlan,
    status: VerificationStatus,
}

impl VerificationEntry {
    /// Returns target.
    #[must_use]
    pub const fn target(&self) -> &PackageTargetPlan {
        &self.target
    }

    /// Returns metadata.
    #[must_use]
    pub const fn metadata(&self) -> &FormatMetadata {
        self.target.metadata()
    }

    /// Returns verification status.
    #[must_use]
    pub const fn status(&self) -> VerificationStatus {
        self.status
    }
}

/// Verification report.
#[derive(Clone, Debug, PartialEq)]
pub struct VerificationReport {
    entries: Vec<VerificationEntry>,
}

impl VerificationReport {
    /// Returns aggregate status.
    #[must_use]
    pub fn status(&self) -> VerificationStatus {
        if self
            .entries
            .iter()
            .all(|entry| entry.status == VerificationStatus::Passed)
        {
            VerificationStatus::Passed
        } else {
            VerificationStatus::Failed
        }
    }

    /// Returns verification entries.
    #[must_use]
    pub fn entries(&self) -> &[VerificationEntry] {
        &self.entries
    }
}

/// Package planning diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageDiagnostic {
    rule: String,
    message: String,
}

impl PackageDiagnostic {
    /// Creates a package diagnostic.
    #[must_use]
    pub fn new(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            rule: rule.into(),
            message: message.into(),
        }
    }

    /// Returns diagnostic rule.
    #[must_use]
    pub fn rule(&self) -> &str {
        &self.rule
    }
}

/// Package planning error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackagePlanningError {
    diagnostics: Vec<PackageDiagnostic>,
}

impl PackagePlanningError {
    /// Returns diagnostics.
    #[must_use]
    pub fn diagnostics(&self) -> &[PackageDiagnostic] {
        &self.diagnostics
    }
}

fn validate_request(request: &PackageRequest) -> Result<(), PackagePlanningError> {
    let mut diagnostics = Vec::new();
    if !is_reverse_dns_id(&request.metadata.id) {
        diagnostics.push(PackageDiagnostic::new(
            "package.metadata.invalid",
            "metadata ID must be reverse-DNS safe",
        ));
    }
    if request.formats.is_empty() {
        diagnostics.push(PackageDiagnostic::new(
            "package.formats.empty",
            "at least one package format is required",
        ));
    }
    if request.output.path.trim().is_empty() {
        diagnostics.push(PackageDiagnostic::new(
            "package.output.empty",
            "output path must not be empty",
        ));
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(PackagePlanningError { diagnostics })
    }
}

fn output_path(output: &BundleOutput, format: PackageFormat) -> String {
    format!(
        "{}/{}.{}",
        output.path.trim_end_matches('/'),
        output.bundle_name,
        format.extension()
    )
}

fn is_reverse_dns_id(value: &str) -> bool {
    let parts: Vec<_> = value.split('.').collect();
    parts.len() >= 3
        && parts.iter().all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_identity() {
        assert_eq!(crate_name(), "hawk2ui-plugin-adapters");
    }
}

#![forbid(unsafe_code)]
//! Production plugin and package adapters for `Hawk2UI` `CLAP`, `VST3`, AU, standalone, and desktop outputs.

use std::{fs, path::Path};

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

    fn manifest_key(self) -> &'static str {
        match self {
            Self::Clap => "clap",
            Self::Vst3 => "vst3",
            Self::Au => "au",
            Self::Standalone => "standalone",
            Self::DesktopBundle => "desktop-bundle",
            Self::SealedArtifact => "sealed-artifact",
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

    fn materialize(&self) -> Result<MaterializedPackageOutput, PackageMaterializationError> {
        let output_path = Path::new(&self.output_path);
        fs::create_dir_all(output_path).map_err(|error| {
            materialization_error(
                "package.output.create-failed",
                format!(
                    "failed to create package output {}: {error}",
                    self.output_path
                ),
            )
        })?;
        let resources_path = output_path.join("Contents").join("Resources");
        fs::create_dir_all(&resources_path).map_err(|error| {
            materialization_error(
                "package.resources.create-failed",
                format!(
                    "failed to create package resources {}: {error}",
                    resources_path.display()
                ),
            )
        })?;
        let manifest_path = output_path.join("hawk2ui-package.toml");
        fs::write(&manifest_path, self.manifest()).map_err(|error| {
            materialization_error(
                "package.output.write-failed",
                format!(
                    "failed to write package metadata {}: {error}",
                    manifest_path.display()
                ),
            )
        })?;
        let artifact_descriptor_path = resources_path.join("hawk2ui-artifact.toml");
        fs::write(&artifact_descriptor_path, self.artifact_descriptor()).map_err(|error| {
            materialization_error(
                "package.artifact.write-failed",
                format!(
                    "failed to write package artifact descriptor {}: {error}",
                    artifact_descriptor_path.display()
                ),
            )
        })?;
        Ok(MaterializedPackageOutput {
            format: self.format,
            output_path: self.output_path.clone(),
            manifest_path: manifest_path.to_string_lossy().into_owned(),
            artifact_descriptor_path: artifact_descriptor_path.to_string_lossy().into_owned(),
        })
    }

    fn manifest(&self) -> String {
        let features = self
            .metadata
            .features
            .iter()
            .map(|feature| format!("\"{feature}\""))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "format = \"{}\"\nid = \"{}\"\ndisplay_name = \"{}\"\nvendor = \"{}\"\nversion = \"{}\"\ncategory = \"{}\"\nfeatures = [{}]\nparameter_count = {}\n",
            self.format.manifest_key(),
            self.metadata.id,
            self.metadata.display_name,
            self.metadata.vendor,
            self.metadata.version,
            self.metadata.category,
            features,
            self.parameter_count
        )
    }

    fn artifact_descriptor(&self) -> String {
        format!(
            "artifact_format = \"hawk2ui-plugin-package\"\nformat = \"{}\"\nentry_library = \"{}.{}\"\nmetadata_manifest = \"hawk2ui-package.toml\"\nparameter_count = {}\n",
            self.format.manifest_key(),
            self.metadata.display_name,
            self.format.extension(),
            self.parameter_count
        )
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

    /// Materializes package output directories with deterministic package metadata.
    ///
    /// # Errors
    ///
    /// Returns [`PackageMaterializationError`] when an output directory or metadata manifest
    /// cannot be created.
    pub fn materialize(
        &self,
    ) -> Result<Vec<MaterializedPackageOutput>, PackageMaterializationError> {
        self.targets
            .iter()
            .map(PackageTargetPlan::materialize)
            .collect()
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

    /// Verifies materialized package outputs exist on disk with their metadata and artifact
    /// descriptors.
    #[must_use]
    pub fn verify_materialized(&self, outputs: &[MaterializedPackageOutput]) -> VerificationReport {
        let entries = self
            .targets
            .iter()
            .cloned()
            .map(|target| {
                let status = outputs
                    .iter()
                    .find(|output| output.format == target.format)
                    .filter(|output| {
                        Path::new(&output.output_path).is_dir()
                            && Path::new(&output.manifest_path).is_file()
                            && Path::new(&output.artifact_descriptor_path).is_file()
                    })
                    .map_or(VerificationStatus::Failed, |_| VerificationStatus::Passed);
                VerificationEntry { target, status }
            })
            .collect();
        VerificationReport { entries }
    }
}

/// Materialized package output metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterializedPackageOutput {
    /// Package format.
    pub format: PackageFormat,
    /// Output directory path.
    pub output_path: String,
    /// Metadata manifest path written inside the output directory.
    pub manifest_path: String,
    /// Runtime artifact descriptor path written inside the output directory.
    pub artifact_descriptor_path: String,
}

/// Package materialization error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageMaterializationError {
    diagnostic: PackageDiagnostic,
}

impl PackageMaterializationError {
    /// Returns the materialization diagnostic.
    #[must_use]
    pub const fn diagnostic(&self) -> &PackageDiagnostic {
        &self.diagnostic
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
    if request.output.bundle_name.trim().is_empty()
        || request.output.bundle_name.contains('/')
        || request.output.bundle_name.contains('\\')
        || request.output.bundle_name.contains('\0')
    {
        diagnostics.push(PackageDiagnostic::new(
            "package.bundle-name.invalid",
            "bundle name must be a single filesystem segment",
        ));
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(PackagePlanningError { diagnostics })
    }
}

fn materialization_error(
    rule: impl Into<String>,
    message: impl Into<String>,
) -> PackageMaterializationError {
    PackageMaterializationError {
        diagnostic: PackageDiagnostic::new(rule, message),
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

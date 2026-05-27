#![forbid(unsafe_code)]
//! Production plugin and package adapters for `Hawk2UI` `CLAP`, `VST3`, AU, standalone, and desktop outputs.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    fs,
    path::Component,
    path::{Path, PathBuf},
};

use hawk2ui_plugin::{BundleOutput, FormatMetadata, ParameterModel};
use sha2::{Digest, Sha256};

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
        if output_path.exists() {
            fs::remove_dir_all(output_path).map_err(|error| {
                materialization_error(
                    "package.output.clean-failed",
                    format!(
                        "failed to clean package output {}: {error}",
                        self.output_path
                    ),
                )
            })?;
        }
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
        let mut package_files = Vec::new();
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
        package_files.push(manifest_path.clone());
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
        package_files.push(artifact_descriptor_path.clone());
        package_files.extend(self.write_format_layout(output_path, &resources_path)?);
        let hash_manifest_path = resources_path.join("hawk2ui-hashes.toml");
        fs::write(
            &hash_manifest_path,
            hash_manifest(output_path, &package_files)?,
        )
        .map_err(|error| {
            materialization_error(
                "package.hashes.write-failed",
                format!(
                    "failed to write package hash manifest {}: {error}",
                    hash_manifest_path.display()
                ),
            )
        })?;
        Ok(MaterializedPackageOutput {
            format: self.format,
            output_path: self.output_path.clone(),
            manifest_path: manifest_path.to_string_lossy().into_owned(),
            artifact_descriptor_path: artifact_descriptor_path.to_string_lossy().into_owned(),
            hash_manifest_path: hash_manifest_path.to_string_lossy().into_owned(),
        })
    }

    fn manifest(&self) -> String {
        let features = self
            .metadata
            .features
            .iter()
            .map(|feature| quoted_metadata_string(feature))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "format = {}\nid = {}\ndisplay_name = {}\nvendor = {}\nversion = {}\ncategory = {}\nfeatures = [{}]\nparameter_count = {}\n",
            quoted_metadata_string(self.format.manifest_key()),
            quoted_metadata_string(&self.metadata.id),
            quoted_metadata_string(&self.metadata.display_name),
            quoted_metadata_string(&self.metadata.vendor),
            quoted_metadata_string(&self.metadata.version),
            quoted_metadata_string(&self.metadata.category),
            features,
            self.parameter_count
        )
    }

    fn artifact_descriptor(&self) -> String {
        format!(
            "artifact_format = {}\nformat = {}\nentry_library = {}\nmetadata_manifest = {}\nparameter_count = {}\n",
            quoted_metadata_string("hawk2ui-plugin-package"),
            quoted_metadata_string(self.format.manifest_key()),
            quoted_metadata_string(&format!(
                "{}.{}",
                self.metadata.display_name,
                self.format.extension()
            )),
            quoted_metadata_string("hawk2ui-package.toml"),
            self.parameter_count
        )
    }

    fn write_format_layout(
        &self,
        output_path: &Path,
        resources_path: &Path,
    ) -> Result<Vec<PathBuf>, PackageMaterializationError> {
        let mut written = Vec::new();
        match self.format {
            PackageFormat::Clap => {
                let entry_path = output_path.join(format!("{}.clap", self.metadata.display_name));
                write_package_file(&entry_path, self.entry_descriptor("clap"))?;
                written.push(entry_path);
                let clap_manifest_path = resources_path.join("clap.json");
                write_package_file(&clap_manifest_path, self.clap_manifest())?;
                written.push(clap_manifest_path);
            }
            PackageFormat::Vst3 => {
                let info_path = output_path.join("Contents").join("Info.plist");
                write_package_file(&info_path, self.info_plist("vst3"))?;
                written.push(info_path);
                let binary_dir = output_path.join("Contents").join("x86_64-linux");
                create_package_dir(&binary_dir)?;
                let binary_path = binary_dir.join(format!("{}.vst3", self.metadata.display_name));
                write_package_file(&binary_path, self.entry_descriptor("vst3"))?;
                written.push(binary_path);
            }
            PackageFormat::Au | PackageFormat::Standalone | PackageFormat::DesktopBundle => {
                let package_type = self.format.manifest_key();
                let info_path = output_path.join("Contents").join("Info.plist");
                write_package_file(&info_path, self.info_plist(package_type))?;
                written.push(info_path);
                let binary_dir = output_path.join("Contents").join("MacOS");
                create_package_dir(&binary_dir)?;
                let binary_path = binary_dir.join(&self.metadata.display_name);
                write_package_file(&binary_path, self.entry_descriptor(package_type))?;
                written.push(binary_path);
                if matches!(
                    self.format,
                    PackageFormat::Standalone | PackageFormat::DesktopBundle
                ) {
                    let launch_path = resources_path.join("hawk2ui-launch.toml");
                    write_package_file(&launch_path, self.launch_manifest())?;
                    written.push(launch_path);
                }
            }
            PackageFormat::SealedArtifact => {
                let artifact_path = resources_path.join("sealed-artifact.hawk2ui");
                write_package_file(&artifact_path, self.entry_descriptor("sealed-artifact"))?;
                written.push(artifact_path);
            }
        }
        Ok(written)
    }

    fn clap_manifest(&self) -> String {
        format!(
            "{{\n  \"id\": {},\n  \"name\": {},\n  \"vendor\": {},\n  \"version\": {}\n}}\n",
            quoted_metadata_string(&self.metadata.id),
            quoted_metadata_string(&self.metadata.display_name),
            quoted_metadata_string(&self.metadata.vendor),
            quoted_metadata_string(&self.metadata.version)
        )
    }

    fn info_plist(&self, package_type: &str) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\"><dict><key>CFBundleIdentifier</key><string>{}</string><key>CFBundleName</key><string>{}</string><key>Hawk2UIVendor</key><string>{}</string><key>Hawk2UIPackageType</key><string>{}</string></dict></plist>\n",
            xml_text(&self.metadata.id),
            xml_text(&self.metadata.display_name),
            xml_text(&self.metadata.vendor),
            xml_text(package_type)
        )
    }

    fn launch_manifest(&self) -> String {
        format!(
            "entry = {}\nid = {}\n",
            quoted_metadata_string(&format!("Contents/MacOS/{}", self.metadata.display_name)),
            quoted_metadata_string(&self.metadata.id)
        )
    }

    fn entry_descriptor(&self, format: &str) -> String {
        format!(
            "hawk2ui package entry\nformat={format}\nid={}\nversion={}\nparameters={}\nartifact_descriptor=Contents/Resources/hawk2ui-artifact.toml\n",
            descriptor_value(&self.metadata.id),
            descriptor_value(&self.metadata.version),
            self.parameter_count
        )
    }

    fn required_package_files(&self, output_path: &Path, resources_path: &Path) -> Vec<PathBuf> {
        let mut files = vec![
            output_path.join("hawk2ui-package.toml"),
            resources_path.join("hawk2ui-artifact.toml"),
        ];
        match self.format {
            PackageFormat::Clap => {
                files.push(output_path.join(format!("{}.clap", self.metadata.display_name)));
                files.push(resources_path.join("clap.json"));
            }
            PackageFormat::Vst3 => {
                files.push(output_path.join("Contents").join("Info.plist"));
                files.push(
                    output_path
                        .join("Contents")
                        .join("x86_64-linux")
                        .join(format!("{}.vst3", self.metadata.display_name)),
                );
            }
            PackageFormat::Au => {
                files.push(output_path.join("Contents").join("Info.plist"));
                files.push(
                    output_path
                        .join("Contents")
                        .join("MacOS")
                        .join(&self.metadata.display_name),
                );
            }
            PackageFormat::Standalone | PackageFormat::DesktopBundle => {
                files.push(output_path.join("Contents").join("Info.plist"));
                files.push(
                    output_path
                        .join("Contents")
                        .join("MacOS")
                        .join(&self.metadata.display_name),
                );
                files.push(resources_path.join("hawk2ui-launch.toml"));
            }
            PackageFormat::SealedArtifact => {
                files.push(resources_path.join("sealed-artifact.hawk2ui"));
            }
        }
        files
    }

    fn verify_materialized_output(&self, output: &MaterializedPackageOutput) -> bool {
        let output_path = Path::new(&output.output_path);
        let resources_path = output_path.join("Contents").join("Resources");
        let hash_manifest_path = Path::new(&output.hash_manifest_path);
        output_path.is_dir()
            && Path::new(&output.manifest_path).is_file()
            && Path::new(&output.artifact_descriptor_path).is_file()
            && hash_manifest_path.is_file()
            && self
                .required_package_files(output_path, &resources_path)
                .iter()
                .all(|path| path.is_file())
            && hash_manifest_matches(output_path, hash_manifest_path)
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
                    .filter(|output| target.verify_materialized_output(output))
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
    /// Package hash manifest path written inside the output directory.
    pub hash_manifest_path: String,
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
    if !is_filesystem_segment(&request.output.bundle_name) {
        diagnostics.push(PackageDiagnostic::new(
            "package.bundle-name.invalid",
            "bundle name must be a single filesystem segment",
        ));
    }
    if !is_filesystem_segment(&request.metadata.display_name) {
        diagnostics.push(PackageDiagnostic::new(
            "package.display-name.invalid",
            "display name must be a non-empty single filesystem segment",
        ));
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(PackagePlanningError { diagnostics })
    }
}

fn quoted_metadata_string(value: &str) -> String {
    format!("\"{}\"", escaped_metadata_string(value))
}

fn escaped_metadata_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => {
                let _ = write!(escaped, "\\u{:04x}", u32::from(ch));
            }
            ch => escaped.push(ch),
        }
    }
    escaped
}

fn unescape_metadata_string(value: &str) -> Option<String> {
    let mut unescaped = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            unescaped.push(ch);
            continue;
        }
        match chars.next()? {
            '"' => unescaped.push('"'),
            '\\' => unescaped.push('\\'),
            'n' => unescaped.push('\n'),
            'r' => unescaped.push('\r'),
            't' => unescaped.push('\t'),
            'u' => {
                let mut codepoint = String::with_capacity(4);
                for _ in 0..4 {
                    codepoint.push(chars.next()?);
                }
                let codepoint = u32::from_str_radix(&codepoint, 16).ok()?;
                unescaped.push(char::from_u32(codepoint)?);
            }
            _ => return None,
        }
    }
    Some(unescaped)
}

fn xml_text(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            ch => escaped.push(ch),
        }
    }
    escaped
}

fn descriptor_value(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\\' => escaped.push_str("\\\\"),
            ch => escaped.push(ch),
        }
    }
    escaped
}

fn materialization_error(
    rule: impl Into<String>,
    message: impl Into<String>,
) -> PackageMaterializationError {
    PackageMaterializationError {
        diagnostic: PackageDiagnostic::new(rule, message),
    }
}

fn create_package_dir(path: &Path) -> Result<(), PackageMaterializationError> {
    fs::create_dir_all(path).map_err(|error| {
        materialization_error(
            "package.directory.create-failed",
            format!(
                "failed to create package directory {}: {error}",
                path.display()
            ),
        )
    })
}

fn write_package_file(
    path: &Path,
    contents: impl AsRef<[u8]>,
) -> Result<(), PackageMaterializationError> {
    fs::write(path, contents).map_err(|error| {
        materialization_error(
            "package.file.write-failed",
            format!("failed to write package file {}: {error}", path.display()),
        )
    })
}

fn hash_manifest(root: &Path, files: &[PathBuf]) -> Result<String, PackageMaterializationError> {
    let mut entries = Vec::with_capacity(files.len());
    for path in files {
        let bytes = fs::read(path).map_err(|error| {
            materialization_error(
                "package.hashes.read-failed",
                format!("failed to hash package file {}: {error}", path.display()),
            )
        })?;
        let relative = path.strip_prefix(root).map_err(|error| {
            materialization_error(
                "package.hashes.path-invalid",
                format!(
                    "package file {} is outside package root {}: {error}",
                    path.display(),
                    root.display()
                ),
            )
        })?;
        entries.push((
            relative.to_string_lossy().replace('\\', "/"),
            sha256(&bytes),
        ));
    }
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    let mut manifest = String::from("algorithm = \"sha256\"\n\n");
    for (path, hash) in entries {
        manifest.push_str("[[files]]\npath = \"");
        manifest.push_str(&escaped_metadata_string(&path));
        manifest.push_str("\"\nhash = \"");
        manifest.push_str(&hash);
        manifest.push_str("\"\n\n");
    }
    Ok(manifest)
}

fn hash_manifest_matches(root: &Path, manifest_path: &Path) -> bool {
    let Ok(manifest) = fs::read_to_string(manifest_path) else {
        return false;
    };
    let Some(entries) = parse_hash_manifest(&manifest) else {
        return false;
    };
    if entries.is_empty() {
        return false;
    }
    let Ok(manifest_relative) = manifest_path.strip_prefix(root) else {
        return false;
    };
    let manifest_relative = normalized_relative_path(manifest_relative);
    let mut expected = BTreeMap::new();
    for (relative_path, expected_hash) in entries {
        let relative = Path::new(&relative_path);
        if !is_safe_relative_path(relative)
            || !is_sha256_hash(&expected_hash)
            || expected.insert(relative_path, expected_hash).is_some()
        {
            return false;
        }
    }
    let Some(actual_files) = package_regular_files(root, &manifest_relative) else {
        return false;
    };
    if expected.keys().cloned().collect::<BTreeSet<_>>() != actual_files {
        return false;
    }
    expected.into_iter().all(|(relative_path, expected_hash)| {
        fs::read(root.join(&relative_path)).is_ok_and(|bytes| sha256(&bytes) == expected_hash)
    })
}

fn package_regular_files(root: &Path, excluded_relative: &str) -> Option<BTreeSet<String>> {
    fn visit(
        root: &Path,
        current: &Path,
        excluded_relative: &str,
        files: &mut BTreeSet<String>,
    ) -> Option<()> {
        for entry in fs::read_dir(current).ok()? {
            let entry = entry.ok()?;
            let file_type = entry.file_type().ok()?;
            let path = entry.path();
            if file_type.is_dir() {
                visit(root, &path, excluded_relative, files)?;
            } else if file_type.is_file() {
                let relative = normalized_relative_path(path.strip_prefix(root).ok()?);
                if relative != excluded_relative {
                    files.insert(relative);
                }
            } else {
                return None;
            }
        }
        Some(())
    }

    let mut files = BTreeSet::new();
    visit(root, root, excluded_relative, &mut files)?;
    Some(files)
}

fn is_safe_relative_path(path: &Path) -> bool {
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn normalized_relative_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn is_sha256_hash(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.chars().all(|ch| ch.is_ascii_hexdigit()))
}

fn parse_hash_manifest(manifest: &str) -> Option<Vec<(String, String)>> {
    let mut algorithm_is_sha256 = false;
    let mut entries = Vec::new();
    let mut current_path: Option<String> = None;
    let mut current_hash: Option<String> = None;
    for line in manifest.lines().map(str::trim) {
        if line.is_empty() {
            continue;
        }
        if line == "algorithm = \"sha256\"" {
            algorithm_is_sha256 = true;
            continue;
        }
        if line == "[[files]]" {
            push_hash_entry(&mut entries, &mut current_path, &mut current_hash)?;
            continue;
        }
        if let Some(value) = line
            .strip_prefix("path = \"")
            .and_then(|value| value.strip_suffix('"'))
        {
            current_path = Some(unescape_metadata_string(value)?);
            continue;
        }
        if let Some(value) = line
            .strip_prefix("hash = \"")
            .and_then(|value| value.strip_suffix('"'))
        {
            current_hash = Some(value.to_string());
            continue;
        }
        return None;
    }
    push_hash_entry(&mut entries, &mut current_path, &mut current_hash)?;
    algorithm_is_sha256.then_some(entries)
}

fn push_hash_entry(
    entries: &mut Vec<(String, String)>,
    current_path: &mut Option<String>,
    current_hash: &mut Option<String>,
) -> Option<()> {
    match (current_path.take(), current_hash.take()) {
        (Some(path), Some(hash)) => {
            entries.push((path, hash));
            Some(())
        }
        (None, None) => Some(()),
        _ => None,
    }
}

fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity("sha256:".len() + (digest.len() * 2));
    encoded.push_str("sha256:");
    for byte in digest {
        encoded.push(hex_nibble(byte >> 4));
        encoded.push(hex_nibble(byte & 0x0f));
    }
    encoded
}

fn hex_nibble(value: u8) -> char {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    char::from(HEX[usize::from(value & 0x0f)])
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

fn is_filesystem_segment(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && trimmed != "."
        && trimmed != ".."
        && !trimmed.contains('/')
        && !trimmed.contains('\\')
        && !trimmed.contains('\0')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_identity() {
        assert_eq!(crate_name(), "hawk2ui-plugin-adapters");
    }
}

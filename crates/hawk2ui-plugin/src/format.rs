//! Plugin format target records.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Supported plugin package format.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub enum PluginFormat {
    /// CLAP plugin package.
    Clap,
    /// VST3 plugin package.
    Vst3,
    /// Audio Unit component package.
    Au,
    /// Standalone application package.
    Standalone,
}

/// Generated plugin metadata shared by all package formats.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FormatMetadata {
    /// Stable reverse-DNS plugin identifier.
    pub id: String,
    /// Display name shown to hosts and users.
    pub display_name: String,
    /// Vendor or author shown to hosts and users.
    pub vendor: String,
    /// Semantic version or package version string.
    pub version: String,
    /// Format category such as instrument, delay, equalizer, or utility.
    pub category: String,
    /// Host-discoverable feature tags.
    pub features: Vec<String>,
}

impl FormatMetadata {
    /// Creates generated plugin metadata.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        display_name: impl Into<String>,
        vendor: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            vendor: vendor.into(),
            version: "0.1.0".into(),
            category: "utility".into(),
            features: Vec::new(),
        }
    }

    /// Sets the version.
    #[must_use]
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    /// Sets the category.
    #[must_use]
    pub fn category(mut self, category: impl Into<String>) -> Self {
        self.category = category.into();
        self
    }

    /// Adds a feature tag.
    #[must_use]
    pub fn feature(mut self, feature: impl Into<String>) -> Self {
        self.features.push(feature.into());
        self
    }

    /// Returns the generated display name including version.
    #[must_use]
    pub fn generated_display_name(&self) -> String {
        format!("{} {}", self.display_name, self.version)
    }
}

/// Bundle output fields for a package target.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BundleOutput {
    /// Filesystem output path for the generated package.
    pub path: String,
    /// Bundle/package file name.
    pub bundle_name: String,
}

impl BundleOutput {
    /// Creates bundle output fields.
    #[must_use]
    pub fn new(path: impl Into<String>, bundle_name: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            bundle_name: bundle_name.into(),
        }
    }
}

/// Plugin format validation error.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FormatValidationError {
    /// Stable validation code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
}

impl FormatValidationError {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

/// Single plugin package target.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PluginFormatTarget {
    /// Plugin format.
    pub format: PluginFormat,
    /// Generated metadata for the format.
    pub metadata: FormatMetadata,
    /// Bundle output fields.
    pub output: BundleOutput,
}

impl PluginFormatTarget {
    /// Creates a CLAP target.
    #[must_use]
    pub const fn clap(metadata: FormatMetadata, output: BundleOutput) -> Self {
        Self {
            format: PluginFormat::Clap,
            metadata,
            output,
        }
    }

    /// Creates a VST3 target.
    #[must_use]
    pub const fn vst3(metadata: FormatMetadata, output: BundleOutput) -> Self {
        Self {
            format: PluginFormat::Vst3,
            metadata,
            output,
        }
    }

    /// Creates an Audio Unit target.
    #[must_use]
    pub const fn au(metadata: FormatMetadata, output: BundleOutput) -> Self {
        Self {
            format: PluginFormat::Au,
            metadata,
            output,
        }
    }

    /// Creates a standalone application target.
    #[must_use]
    pub const fn standalone(metadata: FormatMetadata, output: BundleOutput) -> Self {
        Self {
            format: PluginFormat::Standalone,
            metadata,
            output,
        }
    }

    /// Validates format metadata and package output fields.
    ///
    /// # Errors
    ///
    /// Returns all validation errors when metadata or output fields are invalid.
    pub fn validate(&self) -> Result<(), Vec<FormatValidationError>> {
        let mut errors = Vec::new();
        if !is_reverse_dns_id(&self.metadata.id) {
            errors.push(FormatValidationError::new(
                "format.metadata-id-invalid",
                format!(
                    "plugin metadata id is not reverse-DNS safe: {}",
                    self.metadata.id
                ),
            ));
        }
        if self.metadata.display_name.trim().is_empty() {
            errors.push(FormatValidationError::new(
                "format.metadata-name-empty",
                "plugin display name must not be empty",
            ));
        }
        if self.metadata.vendor.trim().is_empty() {
            errors.push(FormatValidationError::new(
                "format.metadata-vendor-empty",
                "plugin vendor must not be empty",
            ));
        }
        if self.output.path.trim().is_empty() {
            errors.push(FormatValidationError::new(
                "format.output-path-empty",
                "plugin output path must not be empty",
            ));
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Generates the JSON Schema used to validate plugin package target metadata.
    ///
    /// # Errors
    ///
    /// Returns [`FormatValidationError`] when the generated schema cannot be represented as JSON.
    pub fn json_schema() -> Result<serde_json::Value, FormatValidationError> {
        serde_json::to_value(schemars::schema_for!(Self)).map_err(|error| {
            FormatValidationError::new(
                "format.schema.generate-failed",
                format!("plugin format target schema could not be serialized: {error}"),
            )
        })
    }

    /// Validates JSON against the generated plugin package target schema.
    ///
    /// # Errors
    ///
    /// Returns [`FormatValidationError`] when schema compilation or validation fails.
    pub fn validate_json(value: &serde_json::Value) -> Result<(), FormatValidationError> {
        let schema = Self::json_schema()?;
        let validator = jsonschema::Validator::new(&schema).map_err(|error| {
            FormatValidationError::new(
                "format.schema.compile-failed",
                format!("plugin format target schema could not be compiled: {error}"),
            )
        })?;
        validator.validate(value).map_err(|error| {
            FormatValidationError::new(
                "format.schema.invalid",
                format!(
                    "plugin format target failed schema validation at {}: {error}",
                    error.instance_path()
                ),
            )
        })
    }
}

/// Package target collection.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PackageTarget {
    /// Format targets in package order.
    pub targets: Vec<PluginFormatTarget>,
}

impl PackageTarget {
    /// Creates a package target collection.
    #[must_use]
    pub fn new(targets: impl IntoIterator<Item = PluginFormatTarget>) -> Self {
        Self {
            targets: targets.into_iter().collect(),
        }
    }

    /// Returns target formats in package order.
    #[must_use]
    pub fn formats(&self) -> Vec<PluginFormat> {
        self.targets.iter().map(|target| target.format).collect()
    }

    /// Returns bundle output paths in package order.
    #[must_use]
    pub fn bundle_paths(&self) -> Vec<&str> {
        self.targets
            .iter()
            .map(|target| target.output.path.as_str())
            .collect()
    }
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

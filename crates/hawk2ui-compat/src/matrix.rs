//! Machine-readable compatibility matrix records and validation.

use std::collections::BTreeSet;

use serde::Deserialize;

/// Compatibility support status for a release target.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ReleaseStatus {
    /// Target is supported by release gates.
    Supported,
    /// Target is tracked but blocked from release.
    Blocked,
    /// Target is known but not supported.
    Unsupported,
}

/// Host surface kind represented by a target.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum SurfaceKind {
    /// Owned desktop surface.
    Desktop,
    /// Embedded plugin editor surface.
    Plugin,
}

/// One compatibility target row.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct TargetCompatibility {
    /// Stable target name.
    pub name: String,
    /// Operating system family.
    pub family: String,
    /// Supported operating system version range.
    pub os_version: String,
    /// Host surface kind.
    pub surface: SurfaceKind,
    /// CPU architecture.
    pub architecture: String,
    /// Windowing or embedding mechanism.
    pub windowing: String,
    /// Accessibility support path.
    pub accessibility: String,
    /// Package output kind.
    pub packaging: String,
    /// Release support status.
    pub release: ReleaseStatus,
    /// Whether this row is covered by automated CI.
    pub ci_coverage: bool,
}

/// Top-level compatibility matrix.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompatibilityMatrix {
    /// Target rows.
    pub targets: Vec<TargetCompatibility>,
}

#[derive(Debug, Deserialize)]
struct RawMatrix {
    targets: Vec<TargetCompatibility>,
}

/// One graphics backend compatibility row.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct GraphicsBackendCompatibility {
    /// Stable backend identifier.
    pub backend: String,
    /// Whether this backend is supported for release.
    pub supported: bool,
    /// Supported rendering features.
    pub features: Vec<String>,
    /// Diagnostic emitted for unsupported capabilities.
    pub diagnostic: String,
}

/// Graphics compatibility matrix.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphicsCompatibilityMatrix {
    /// Backend rows.
    pub backends: Vec<GraphicsBackendCompatibility>,
}

#[derive(Debug, Deserialize)]
struct RawGraphicsMatrix {
    backends: Vec<GraphicsBackendCompatibility>,
}

/// Compatibility coverage state for host lifecycle behaviors.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum CoverageStatus {
    /// Behavior is covered by compatibility fixtures and release gates.
    Covered,
    /// Behavior is intentionally missing and must not be treated as release-ready.
    Missing,
}

impl CoverageStatus {
    /// Returns whether this behavior is covered.
    #[must_use]
    pub const fn is_covered(self) -> bool {
        matches!(self, Self::Covered)
    }
}

/// One plugin host compatibility row.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct PluginHostCompatibility {
    /// Stable plugin format identifier.
    pub format: String,
    /// Whether DAW-owned parent attachment is covered.
    pub host_attachment: CoverageStatus,
    /// Whether resize is covered.
    pub resize: CoverageStatus,
    /// Whether DPI changes are covered.
    pub dpi: CoverageStatus,
    /// Whether keyboard focus is covered.
    pub keyboard_focus: CoverageStatus,
    /// Whether accessibility export is covered.
    pub accessibility: CoverageStatus,
    /// Whether state save/load is covered.
    pub state: CoverageStatus,
    /// Whether automation gestures are covered.
    pub automation: CoverageStatus,
    /// Whether realtime visual data transport is covered.
    pub realtime_visual_data: CoverageStatus,
}

/// Plugin host compatibility matrix.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostCompatibilityMatrix {
    /// Plugin host rows.
    pub hosts: Vec<PluginHostCompatibility>,
}

#[derive(Debug, Deserialize)]
struct RawHostMatrix {
    hosts: Vec<PluginHostCompatibility>,
}

/// One package output compatibility row.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct PackageCompatibility {
    /// Stable package output identifier.
    pub output: String,
    /// Package kind.
    pub kind: String,
    /// Target platform or artifact class.
    pub platform: String,
    /// Whether signing is required.
    pub signing: bool,
    /// Whether notarization is required.
    pub notarization: bool,
    /// Whether installer generation is required.
    pub installer: bool,
    /// Local verification command.
    pub verify_command: String,
}

/// Package compatibility matrix.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageCompatibilityMatrix {
    /// Package output rows.
    pub packages: Vec<PackageCompatibility>,
}

#[derive(Debug, Deserialize)]
struct RawPackageMatrix {
    packages: Vec<PackageCompatibility>,
}

impl CompatibilityMatrix {
    /// Parses and validates a compatibility matrix from TOML.
    ///
    /// # Errors
    ///
    /// Returns [`MatrixError`] when TOML parsing fails or when target names are duplicated.
    pub fn parse(input: &str) -> Result<Self, MatrixError> {
        let raw: RawMatrix =
            toml::from_str(input).map_err(|error| MatrixError::Parse(error.to_string()))?;
        let matrix = Self {
            targets: raw.targets,
        };
        matrix.validate()?;
        Ok(matrix)
    }

    /// Returns true when a target exists.
    #[must_use]
    pub fn contains_target(&self, target: &str) -> bool {
        self.targets.iter().any(|row| row.name == target)
    }

    /// Returns an unsupported-target diagnostic for missing targets.
    #[must_use]
    pub fn unsupported_target_diagnostic(&self, target: &str) -> Option<String> {
        if self.contains_target(target) {
            return None;
        }

        let supported = self
            .targets
            .iter()
            .map(|row| row.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        Some(format!(
            "unsupported target '{target}'. Supported targets: {supported}"
        ))
    }

    fn validate(&self) -> Result<(), MatrixError> {
        let mut names = BTreeSet::new();
        for target in &self.targets {
            require_field("target", &target.name, "name")?;
            require_field(&target.name, &target.family, "family")?;
            require_field(&target.name, &target.os_version, "os_version")?;
            require_field(&target.name, &target.architecture, "architecture")?;
            require_field(&target.name, &target.windowing, "windowing")?;
            require_field(&target.name, &target.accessibility, "accessibility")?;
            require_field(&target.name, &target.packaging, "packaging")?;
            if !names.insert(target.name.clone()) {
                return Err(MatrixError::DuplicateTarget(target.name.clone()));
            }
        }
        Ok(())
    }
}

impl GraphicsCompatibilityMatrix {
    /// Parses and validates graphics backend compatibility rows from TOML.
    ///
    /// # Errors
    ///
    /// Returns [`MatrixError`] when TOML parsing fails, a backend is duplicated, or a required
    /// field is missing.
    pub fn parse(input: &str) -> Result<Self, MatrixError> {
        let raw: RawGraphicsMatrix =
            toml::from_str(input).map_err(|error| MatrixError::Parse(error.to_string()))?;
        let matrix = Self {
            backends: raw.backends,
        };
        matrix.validate()?;
        Ok(matrix)
    }

    /// Returns true when any supported backend declares the requested feature.
    #[must_use]
    pub fn supports_feature(&self, feature: &str) -> bool {
        self.backends.iter().any(|backend| {
            backend.supported
                && backend
                    .features
                    .iter()
                    .any(|candidate| candidate == feature)
        })
    }

    fn validate(&self) -> Result<(), MatrixError> {
        let mut names = BTreeSet::new();
        for backend in &self.backends {
            require_field("graphics backend", &backend.backend, "backend")?;
            require_field(&backend.backend, &backend.diagnostic, "diagnostic")?;
            if backend.features.is_empty() {
                return Err(MatrixError::MissingRequiredField {
                    row: backend.backend.clone(),
                    field: "features",
                });
            }
            if !names.insert(backend.backend.clone()) {
                return Err(MatrixError::DuplicateTarget(backend.backend.clone()));
            }
        }
        Ok(())
    }
}

impl HostCompatibilityMatrix {
    /// Parses and validates plugin host compatibility rows from TOML.
    ///
    /// # Errors
    ///
    /// Returns [`MatrixError`] when TOML parsing fails, a format is duplicated, or a required field
    /// is missing.
    pub fn parse(input: &str) -> Result<Self, MatrixError> {
        let raw: RawHostMatrix =
            toml::from_str(input).map_err(|error| MatrixError::Parse(error.to_string()))?;
        let matrix = Self { hosts: raw.hosts };
        matrix.validate()?;
        Ok(matrix)
    }

    /// Returns host compatibility by plugin format.
    #[must_use]
    pub fn host(&self, format: &str) -> Option<&PluginHostCompatibility> {
        self.hosts.iter().find(|host| host.format == format)
    }

    fn validate(&self) -> Result<(), MatrixError> {
        let mut formats = BTreeSet::new();
        for host in &self.hosts {
            require_field("host", &host.format, "format")?;
            if !formats.insert(host.format.clone()) {
                return Err(MatrixError::DuplicateTarget(host.format.clone()));
            }
        }
        Ok(())
    }
}

impl PackageCompatibilityMatrix {
    /// Parses and validates package output compatibility rows from TOML.
    ///
    /// # Errors
    ///
    /// Returns [`MatrixError`] when TOML parsing fails, an output is duplicated, or a required field
    /// is missing.
    pub fn parse(input: &str) -> Result<Self, MatrixError> {
        let raw: RawPackageMatrix =
            toml::from_str(input).map_err(|error| MatrixError::Parse(error.to_string()))?;
        let matrix = Self {
            packages: raw.packages,
        };
        matrix.validate()?;
        Ok(matrix)
    }

    /// Returns package compatibility by output identifier.
    #[must_use]
    pub fn package(&self, output: &str) -> Option<&PackageCompatibility> {
        self.packages
            .iter()
            .find(|package| package.output == output)
    }

    fn validate(&self) -> Result<(), MatrixError> {
        let mut outputs = BTreeSet::new();
        for package in &self.packages {
            require_field("package", &package.output, "output")?;
            require_field(&package.output, &package.kind, "kind")?;
            require_field(&package.output, &package.platform, "platform")?;
            require_field(&package.output, &package.verify_command, "verify_command")?;
            if !outputs.insert(package.output.clone()) {
                return Err(MatrixError::DuplicateTarget(package.output.clone()));
            }
        }
        Ok(())
    }
}

fn require_field(row: &str, value: &str, field: &'static str) -> Result<(), MatrixError> {
    if value.trim().is_empty() {
        Err(MatrixError::MissingRequiredField {
            row: row.to_owned(),
            field,
        })
    } else {
        Ok(())
    }
}

/// Compatibility matrix validation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MatrixError {
    /// TOML parsing failed.
    Parse(String),
    /// Two or more target rows use the same stable name.
    DuplicateTarget(String),
    /// A matrix row is missing a required field.
    MissingRequiredField {
        /// Row identifier.
        row: String,
        /// Missing field name.
        field: &'static str,
    },
}

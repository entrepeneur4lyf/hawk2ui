//! Machine-readable compatibility matrix records and validation.

use std::collections::BTreeSet;

use hawk2ui_api::Diagnostic;
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

impl ReleaseStatus {
    /// Returns true when this target is accepted by release gates.
    #[must_use]
    pub const fn is_supported(self) -> bool {
        matches!(self, Self::Supported)
    }

    /// Returns a stable status label for diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Supported => "supported",
            Self::Blocked => "blocked",
            Self::Unsupported => "unsupported",
        }
    }
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

    /// Returns a stable coverage label for diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Covered => "covered",
            Self::Missing => "missing",
        }
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

impl PluginHostCompatibility {
    /// Returns host lifecycle coverage fields that are not release-covered.
    #[must_use]
    pub fn missing_coverage_fields(&self) -> Vec<&'static str> {
        [
            ("host_attachment", self.host_attachment),
            ("resize", self.resize),
            ("dpi", self.dpi),
            ("keyboard_focus", self.keyboard_focus),
            ("accessibility", self.accessibility),
            ("state", self.state),
            ("automation", self.automation),
            ("realtime_visual_data", self.realtime_visual_data),
        ]
        .into_iter()
        .filter_map(|(field, status)| (!status.is_covered()).then_some(field))
        .collect()
    }
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

    /// Returns true when a release-supported target exists.
    #[must_use]
    pub fn contains_target(&self, target: &str) -> bool {
        self.targets
            .iter()
            .any(|row| row.name == target && row.release.is_supported())
    }

    /// Returns a target row by stable target name.
    #[must_use]
    pub fn target(&self, target: &str) -> Option<&TargetCompatibility> {
        self.targets.iter().find(|row| row.name == target)
    }

    /// Returns release-supported target names.
    #[must_use]
    pub fn supported_targets(&self) -> Vec<&str> {
        self.targets
            .iter()
            .filter(|row| row.release.is_supported())
            .map(|row| row.name.as_str())
            .collect()
    }

    /// Returns an unsupported-target diagnostic for missing or non-release-supported targets.
    #[must_use]
    pub fn unsupported_target_diagnostic(&self, target: &str) -> Option<String> {
        if let Some(row) = self.target(target) {
            if row.release.is_supported() {
                return None;
            }
            return Some(format!(
                "unsupported target '{target}' is declared as {}. Supported targets: {}",
                row.release.as_str(),
                self.supported_targets().join(", ")
            ));
        }

        let supported = self.supported_targets().join(", ");
        Some(format!(
            "unsupported target '{target}'. Supported targets: {supported}"
        ))
    }

    /// Returns a shared unsupported-target diagnostic for missing or non-release-supported targets.
    #[must_use]
    pub fn unsupported_target_shared_diagnostic(&self, target: &str) -> Option<Diagnostic> {
        self.unsupported_target_diagnostic(target)
            .map(|message| Diagnostic::error("compat.target.unsupported", message))
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

    /// Returns an unsupported-feature diagnostic for a backend/feature pair.
    #[must_use]
    pub fn unsupported_feature_diagnostic(
        &self,
        backend: &str,
        feature: &str,
    ) -> Option<Diagnostic> {
        let Some(row) = self.backends.iter().find(|row| row.backend == backend) else {
            return Some(Diagnostic::error(
                "compat.graphics.backend-unknown",
                format!("graphics backend '{backend}' is not declared"),
            ));
        };
        let feature_supported =
            row.supported && row.features.iter().any(|candidate| candidate == feature);
        (!feature_supported).then(|| {
            Diagnostic::error(
                &row.diagnostic,
                format!("graphics backend '{backend}' does not support feature '{feature}'"),
            )
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
                return Err(MatrixError::DuplicateKey {
                    kind: "graphics backend",
                    key: backend.backend.clone(),
                });
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

    /// Returns diagnostics for missing host lifecycle coverage.
    #[must_use]
    pub fn missing_coverage_diagnostics(&self, format: &str) -> Vec<Diagnostic> {
        let Some(host) = self.host(format) else {
            return vec![Diagnostic::error(
                "compat.host.missing",
                format!("plugin host compatibility row is missing for format '{format}'"),
            )];
        };
        host.missing_coverage_fields()
            .into_iter()
            .map(|field| {
                Diagnostic::error(
                    "compat.host.coverage-missing",
                    format!("plugin host format '{format}' is missing coverage for {field}"),
                )
            })
            .collect()
    }

    fn validate(&self) -> Result<(), MatrixError> {
        let mut formats = BTreeSet::new();
        for host in &self.hosts {
            require_field("host", &host.format, "format")?;
            if !formats.insert(host.format.clone()) {
                return Err(MatrixError::DuplicateKey {
                    kind: "plugin host format",
                    key: host.format.clone(),
                });
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

    /// Returns a diagnostic when a package output is not declared.
    #[must_use]
    pub fn missing_package_diagnostic(&self, output: &str) -> Option<Diagnostic> {
        self.package(output).is_none().then(|| {
            Diagnostic::error(
                "compat.package.missing",
                format!("package output '{output}' is not declared"),
            )
        })
    }

    fn validate(&self) -> Result<(), MatrixError> {
        let mut outputs = BTreeSet::new();
        for package in &self.packages {
            require_field("package", &package.output, "output")?;
            require_field(&package.output, &package.kind, "kind")?;
            require_field(&package.output, &package.platform, "platform")?;
            require_field(&package.output, &package.verify_command, "verify_command")?;
            if !outputs.insert(package.output.clone()) {
                return Err(MatrixError::DuplicateKey {
                    kind: "package output",
                    key: package.output.clone(),
                });
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
    /// Two or more non-target rows use the same stable key.
    DuplicateKey {
        /// Matrix row kind.
        kind: &'static str,
        /// Duplicated stable key.
        key: String,
    },
    /// A matrix row is missing a required field.
    MissingRequiredField {
        /// Row identifier.
        row: String,
        /// Missing field name.
        field: &'static str,
    },
}

impl From<MatrixError> for Diagnostic {
    fn from(error: MatrixError) -> Self {
        match error {
            MatrixError::Parse(message) => Diagnostic::error(
                "compat.matrix.parse-failed",
                format!("compatibility matrix could not be parsed: {message}"),
            ),
            MatrixError::DuplicateTarget(target) => Diagnostic::error(
                "compat.matrix.duplicate-target",
                format!("duplicate compatibility target row: {target}"),
            ),
            MatrixError::DuplicateKey { kind, key } => Diagnostic::error(
                "compat.matrix.duplicate-key",
                format!("duplicate {kind} row: {key}"),
            ),
            MatrixError::MissingRequiredField { row, field } => Diagnostic::error(
                "compat.matrix.missing-field",
                format!("compatibility matrix row '{row}' is missing required field '{field}'"),
            ),
        }
    }
}

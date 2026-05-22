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
            if !names.insert(target.name.clone()) {
                return Err(MatrixError::DuplicateTarget(target.name.clone()));
            }
        }
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
}

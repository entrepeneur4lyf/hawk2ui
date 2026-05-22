//! Hawk manifest schema and validation.

use std::collections::BTreeSet;

use serde::Deserialize;

/// Supported package target class.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum PackageTarget {
    /// Desktop package target.
    Desktop,
    /// Plugin package target.
    Plugin,
}

/// Validated Hawk manifest.
#[derive(Clone, Debug, PartialEq)]
pub struct HawkManifest {
    /// App identity.
    pub identity: ManifestIdentity,
    /// Source entrypoint.
    pub source: SourceEntrypoint,
    /// Capability keys.
    pub capabilities: Vec<String>,
    /// Package targets.
    pub targets: Vec<TargetDeclaration>,
    /// Optional plugin identity.
    pub plugin: Option<PluginIdentity>,
    /// Optional editor metadata.
    pub editor: Option<EditorMetadata>,
    /// Plugin parameters.
    pub parameters: Vec<PluginParameter>,
}

#[derive(Debug, Deserialize)]
struct RawManifest {
    identity: Option<ManifestIdentity>,
    source: Option<SourceEntrypoint>,
    capabilities: Option<CapabilityDeclaration>,
    #[serde(default)]
    targets: Vec<TargetDeclaration>,
    plugin: Option<PluginIdentity>,
    editor: Option<EditorMetadata>,
    #[serde(default)]
    parameters: Vec<PluginParameter>,
}

/// App identity metadata.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct ManifestIdentity {
    /// Stable product ID.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Product version.
    pub version: String,
}

/// Source entrypoint declaration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct SourceEntrypoint {
    /// Source entry path.
    pub entry: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct CapabilityDeclaration {
    keys: Vec<String>,
}

/// Target declaration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct TargetDeclaration {
    /// Target kind.
    pub kind: PackageTarget,
    /// Stable target name.
    pub name: String,
}

/// Plugin identity metadata.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct PluginIdentity {
    /// Plugin ID.
    pub id: String,
    /// Plugin display name.
    pub name: String,
}

/// Editor metadata.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct EditorMetadata {
    /// Initial editor width.
    pub width: u32,
    /// Initial editor height.
    pub height: u32,
}

/// Plugin parameter metadata.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct PluginParameter {
    /// Parameter ID.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Default normalized value.
    pub default: f64,
}

impl HawkManifest {
    /// Parses and validates a Hawk manifest from TOML.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestError`] when parsing fails or validation rejects the manifest.
    pub fn parse(input: &str) -> Result<Self, ManifestError> {
        let raw: RawManifest =
            toml::from_str(input).map_err(|error| ManifestError::Parse(error.to_string()))?;
        let manifest = Self {
            identity: raw
                .identity
                .ok_or(ManifestError::MissingSection("identity"))?,
            source: raw.source.ok_or(ManifestError::MissingSection("source"))?,
            capabilities: raw.capabilities.map_or_else(Vec::new, |cap| cap.keys),
            targets: raw.targets,
            plugin: raw.plugin,
            editor: raw.editor,
            parameters: raw.parameters,
        };
        manifest.validate()?;
        Ok(manifest)
    }

    /// Returns true when the manifest declares a capability key.
    #[must_use]
    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|key| key == capability)
    }

    /// Returns true when the manifest declares a target kind.
    #[must_use]
    pub fn has_target(&self, target: PackageTarget) -> bool {
        self.targets.iter().any(|decl| decl.kind == target)
    }

    /// Creates a stable manifest snapshot string for hashing.
    #[must_use]
    pub fn snapshot(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.identity.id, self.identity.name, self.identity.version, self.source.entry
        )
    }

    fn validate(&self) -> Result<(), ManifestError> {
        require_non_empty("identity.id", &self.identity.id)?;
        require_non_empty("identity.name", &self.identity.name)?;
        require_non_empty("identity.version", &self.identity.version)?;
        require_non_empty("source.entry", &self.source.entry)?;

        let mut target_names = BTreeSet::new();
        for target in &self.targets {
            require_non_empty("target.name", &target.name)?;
            if !target_names.insert(target.name.clone()) {
                return Err(ManifestError::DuplicateTarget(target.name.clone()));
            }
        }

        for capability in &self.capabilities {
            if capability.trim().is_empty() || capability.contains(' ') {
                return Err(ManifestError::InvalidCapability(capability.clone()));
            }
        }

        if !self.parameters.is_empty() && self.plugin.is_none() {
            return Err(ManifestError::InvalidPluginMetadata(
                "parameters require [plugin] metadata",
            ));
        }

        Ok(())
    }
}

fn require_non_empty(field: &'static str, value: &str) -> Result<(), ManifestError> {
    if value.trim().is_empty() {
        Err(ManifestError::MissingField(field))
    } else {
        Ok(())
    }
}

/// Manifest validation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManifestError {
    /// TOML parsing failed.
    Parse(String),
    /// Required section is missing.
    MissingSection(&'static str),
    /// Required field is empty.
    MissingField(&'static str),
    /// Duplicate target name.
    DuplicateTarget(String),
    /// Invalid capability key.
    InvalidCapability(String),
    /// Invalid plugin metadata.
    InvalidPluginMetadata(&'static str),
}

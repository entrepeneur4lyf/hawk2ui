//! Versioned artifact API contracts.

use serde::{Deserialize, Serialize};

/// Stable artifact identifier.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArtifactId(String);

impl ArtifactId {
    /// Creates an artifact identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Hash attached to a compiled artifact or artifact member.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArtifactHash(String);

impl ArtifactHash {
    /// Creates an artifact hash string.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the hash as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Semantic version for sealed artifact schemas.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArtifactSchemaVersion {
    /// Major schema version.
    pub major: u16,
    /// Minor schema version.
    pub minor: u16,
    /// Patch schema version.
    pub patch: u16,
}

impl ArtifactSchemaVersion {
    /// Creates a schema version.
    #[must_use]
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Verifies that this runtime can read an artifact with the provided schema version.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactVersionError`] when the major versions differ or when the
    /// artifact requires a newer minor schema than the runtime supports.
    pub const fn ensure_can_read(self, artifact: Self) -> Result<(), ArtifactVersionError> {
        if self.major != artifact.major || artifact.minor > self.minor {
            Err(ArtifactVersionError {
                runtime: self,
                artifact,
            })
        } else {
            Ok(())
        }
    }
}

/// Error returned when runtime and artifact schema versions are incompatible.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArtifactVersionError {
    /// Runtime-supported schema version.
    pub runtime: ArtifactSchemaVersion,
    /// Artifact schema version.
    pub artifact: ArtifactSchemaVersion,
}

/// Capability declared by a sealed artifact.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArtifactCapability(String);

impl ArtifactCapability {
    /// Creates an artifact capability declaration.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the capability as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Compiled asset kind.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CompiledAssetKind {
    /// Raster image asset.
    Image,
    /// Vector asset.
    Vector,
    /// Font asset.
    Font,
}

/// Compiled asset record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CompiledAssetRecord {
    id: String,
    hash: ArtifactHash,
    kind: CompiledAssetKind,
    width: Option<u32>,
    height: Option<u32>,
}

impl CompiledAssetRecord {
    /// Creates a compiled image asset record.
    #[must_use]
    pub fn image(id: impl Into<String>, hash: ArtifactHash, width: u32, height: u32) -> Self {
        Self {
            id: id.into(),
            hash,
            kind: CompiledAssetKind::Image,
            width: Some(width),
            height: Some(height),
        }
    }

    /// Returns the asset ID.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns a deterministic asset key.
    #[must_use]
    pub fn stable_key(&self) -> String {
        let dimensions = self.width.zip(self.height).map_or_else(
            || "unbounded".to_string(),
            |(width, height)| format!("{width}x{height}"),
        );
        format!(
            "{}:{}:{}:{dimensions}",
            self.kind_key(),
            self.id,
            self.hash.as_str()
        )
    }

    fn kind_key(&self) -> &'static str {
        match self.kind {
            CompiledAssetKind::Image => "image",
            CompiledAssetKind::Vector => "vector",
            CompiledAssetKind::Font => "font",
        }
    }
}

/// Compiled style record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CompiledStyleRecord {
    id: String,
    hash: ArtifactHash,
}

impl CompiledStyleRecord {
    /// Creates a compiled style record.
    #[must_use]
    pub fn new(id: impl Into<String>, hash: ArtifactHash) -> Self {
        Self {
            id: id.into(),
            hash,
        }
    }

    /// Returns the style ID.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
}

/// Compiled script record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CompiledScriptRecord {
    id: String,
    hash: ArtifactHash,
}

impl CompiledScriptRecord {
    /// Creates a compiled script module record.
    #[must_use]
    pub fn module(id: impl Into<String>, hash: ArtifactHash) -> Self {
        Self {
            id: id.into(),
            hash,
        }
    }

    /// Returns the script ID.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
}

/// Target surface family.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum TargetKind {
    /// Desktop application target.
    Desktop,
    /// Plugin package target.
    Plugin,
}

/// Target metadata record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TargetMetadata {
    name: String,
    kind: TargetKind,
}

impl TargetMetadata {
    /// Creates a desktop target metadata record.
    #[must_use]
    pub fn desktop(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: TargetKind::Desktop,
        }
    }

    /// Creates a plugin target metadata record.
    #[must_use]
    pub fn plugin(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: TargetKind::Plugin,
        }
    }

    /// Returns the target name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Manifest snapshot embedded in a sealed artifact.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArtifactManifestSnapshot {
    id: ArtifactId,
    schema_version: ArtifactSchemaVersion,
    manifest_hash: ArtifactHash,
    capabilities: Vec<ArtifactCapability>,
    assets: Vec<CompiledAssetRecord>,
    styles: Vec<CompiledStyleRecord>,
    scripts: Vec<CompiledScriptRecord>,
    targets: Vec<TargetMetadata>,
}

impl ArtifactManifestSnapshot {
    /// Creates an artifact manifest snapshot.
    #[must_use]
    pub fn new(
        id: ArtifactId,
        schema_version: ArtifactSchemaVersion,
        manifest_hash: ArtifactHash,
    ) -> Self {
        Self {
            id,
            schema_version,
            manifest_hash,
            capabilities: Vec::new(),
            assets: Vec::new(),
            styles: Vec::new(),
            scripts: Vec::new(),
            targets: Vec::new(),
        }
    }

    /// Adds a capability declaration.
    #[must_use]
    pub fn with_capability(mut self, capability: ArtifactCapability) -> Self {
        self.capabilities.push(capability);
        self
    }

    /// Adds a compiled asset.
    #[must_use]
    pub fn with_asset(mut self, asset: CompiledAssetRecord) -> Self {
        self.assets.push(asset);
        self
    }

    /// Adds a compiled style.
    #[must_use]
    pub fn with_style(mut self, style: CompiledStyleRecord) -> Self {
        self.styles.push(style);
        self
    }

    /// Adds a compiled script.
    #[must_use]
    pub fn with_script(mut self, script: CompiledScriptRecord) -> Self {
        self.scripts.push(script);
        self
    }

    /// Adds target metadata.
    #[must_use]
    pub fn with_target(mut self, target: TargetMetadata) -> Self {
        self.targets.push(target);
        self
    }

    /// Returns artifact ID.
    #[must_use]
    pub const fn id(&self) -> &ArtifactId {
        &self.id
    }

    /// Returns whether a capability is declared.
    #[must_use]
    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities
            .iter()
            .any(|candidate| candidate.as_str() == capability)
    }

    /// Returns compiled assets.
    #[must_use]
    pub fn assets(&self) -> &[CompiledAssetRecord] {
        &self.assets
    }

    /// Returns compiled styles.
    #[must_use]
    pub fn styles(&self) -> &[CompiledStyleRecord] {
        &self.styles
    }

    /// Returns compiled scripts.
    #[must_use]
    pub fn scripts(&self) -> &[CompiledScriptRecord] {
        &self.scripts
    }

    /// Returns target metadata.
    #[must_use]
    pub fn targets(&self) -> &[TargetMetadata] {
        &self.targets
    }
}

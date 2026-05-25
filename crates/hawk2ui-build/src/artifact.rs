//! Sealed artifact records and compatibility checks.

use crate::{BuildDiagnostic, BuildDiagnosticSeverity, HawkManifest, PackageTarget};
use sha2::{Digest, Sha256};

/// Artifact schema version.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactSchemaVersion {
    /// Major schema version.
    pub major: u32,
    /// Minor schema version.
    pub minor: u32,
}

impl ArtifactSchemaVersion {
    /// Creates an artifact schema version.
    #[must_use]
    pub const fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }

    /// Returns true when two versions are schema-compatible.
    #[must_use]
    pub const fn is_compatible_with(self, other: Self) -> bool {
        self.major == other.major
    }
}

/// Stable artifact hash wrapper.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactHash(pub String);

impl ArtifactHash {
    /// Creates a deterministic SHA-256 hash string from bytes.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let digest = Sha256::digest(bytes);
        let mut encoded = String::with_capacity("sha256:".len() + (digest.len() * 2));
        encoded.push_str("sha256:");
        for byte in digest {
            encoded.push(hex_nibble(byte >> 4));
            encoded.push(hex_nibble(byte & 0x0f));
        }
        Self(encoded)
    }
}

fn hex_nibble(value: u8) -> char {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    char::from(HEX[usize::from(value & 0x0f)])
}

/// Compiled script payload recorded in a sealed artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledScriptRecord {
    /// Stable entrypoint ID.
    pub entrypoint_id: String,
    /// Source file path.
    pub source_path: String,
    /// Artifact-local payload path.
    pub artifact_path: String,
    /// Source content hash.
    pub source_hash: ArtifactHash,
}

impl CompiledScriptRecord {
    /// Creates a compiled script record.
    #[must_use]
    pub fn new(
        entrypoint_id: impl Into<String>,
        source_path: impl Into<String>,
        artifact_path: impl Into<String>,
        source_hash: ArtifactHash,
    ) -> Self {
        Self {
            entrypoint_id: entrypoint_id.into(),
            source_path: source_path.into(),
            artifact_path: artifact_path.into(),
            source_hash,
        }
    }
}

/// Compiled style payload recorded in a sealed artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledStyleRecord {
    /// Stable entrypoint ID.
    pub entrypoint_id: String,
    /// Source file path.
    pub source_path: String,
    /// Artifact-local payload path.
    pub artifact_path: String,
    /// Source content hash.
    pub source_hash: ArtifactHash,
}

impl CompiledStyleRecord {
    /// Creates a compiled style record.
    #[must_use]
    pub fn new(
        entrypoint_id: impl Into<String>,
        source_path: impl Into<String>,
        artifact_path: impl Into<String>,
        source_hash: ArtifactHash,
    ) -> Self {
        Self {
            entrypoint_id: entrypoint_id.into(),
            source_path: source_path.into(),
            artifact_path: artifact_path.into(),
            source_hash,
        }
    }
}

/// Asset manifest entry exposed to runtime code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetManifestEntry {
    /// Stable asset ID.
    pub id: String,
    /// Asset kind.
    pub kind: String,
    /// Artifact-local payload path.
    pub artifact_path: String,
    /// Compiled payload hash.
    pub hash: ArtifactHash,
}

impl AssetManifestEntry {
    /// Creates an asset manifest entry.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        kind: impl Into<String>,
        artifact_path: impl Into<String>,
        hash: ArtifactHash,
    ) -> Self {
        Self {
            id: id.into(),
            kind: kind.into(),
            artifact_path: artifact_path.into(),
            hash,
        }
    }
}

/// Compiled asset payload recorded in a sealed artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledAssetRecord {
    /// Stable asset ID.
    pub id: String,
    /// Source file path.
    pub source_path: String,
    /// Artifact-local payload path.
    pub artifact_path: String,
    /// Source content hash.
    pub source_hash: ArtifactHash,
}

impl CompiledAssetRecord {
    /// Creates a compiled asset record.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        source_path: impl Into<String>,
        artifact_path: impl Into<String>,
        source_hash: ArtifactHash,
    ) -> Self {
        Self {
            id: id.into(),
            source_path: source_path.into(),
            artifact_path: artifact_path.into(),
            source_hash,
        }
    }
}

/// Artifact hash summary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactHashes {
    /// Manifest snapshot hash.
    pub manifest: ArtifactHash,
    /// Full artifact content hash.
    pub content: ArtifactHash,
}

/// Build metadata embedded into a sealed artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildMetadata {
    /// Build tool that produced the artifact.
    pub generator: String,
    /// Build profile.
    pub profile: String,
}

impl Default for BuildMetadata {
    fn default() -> Self {
        Self {
            generator: "hawk2ui-build".into(),
            profile: "production".into(),
        }
    }
}

/// Target metadata embedded into a sealed artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetArtifactMetadata {
    /// Package target kind.
    pub kind: PackageTarget,
    /// Stable target name.
    pub name: String,
}

/// Sealed artifact record consumed by runtime code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SealedArtifact {
    /// Artifact schema version.
    pub schema_version: ArtifactSchemaVersion,
    /// Stable validated manifest snapshot.
    pub manifest_snapshot: String,
    /// Hash of the manifest snapshot.
    pub manifest_snapshot_hash: ArtifactHash,
    /// Compiled script payloads.
    pub compiled_scripts: Vec<CompiledScriptRecord>,
    /// Compiled style payloads.
    pub compiled_styles: Vec<CompiledStyleRecord>,
    /// Runtime asset manifest.
    pub asset_manifest: Vec<AssetManifestEntry>,
    /// Compiled asset payloads.
    pub compiled_assets: Vec<CompiledAssetRecord>,
    /// Declared runtime capabilities.
    pub capabilities: Vec<String>,
    /// Stable artifact hash summary.
    pub hashes: ArtifactHashes,
    /// Build metadata.
    pub build_metadata: BuildMetadata,
    /// Target metadata.
    pub target_metadata: Vec<TargetArtifactMetadata>,
}

impl SealedArtifact {
    /// Creates a sealed artifact from a validated manifest.
    #[must_use]
    pub fn from_manifest(schema_version: ArtifactSchemaVersion, manifest: &HawkManifest) -> Self {
        let manifest_snapshot = manifest.snapshot();
        let manifest_snapshot_hash = ArtifactHash::from_bytes(manifest_snapshot.as_bytes());
        let mut artifact = Self {
            schema_version,
            manifest_snapshot,
            manifest_snapshot_hash: manifest_snapshot_hash.clone(),
            compiled_scripts: Vec::new(),
            compiled_styles: Vec::new(),
            asset_manifest: Vec::new(),
            compiled_assets: Vec::new(),
            capabilities: manifest.capabilities.clone(),
            hashes: ArtifactHashes {
                manifest: manifest_snapshot_hash,
                content: ArtifactHash::from_bytes(&[]),
            },
            build_metadata: BuildMetadata::default(),
            target_metadata: manifest
                .targets
                .iter()
                .map(|target| TargetArtifactMetadata {
                    kind: target.kind,
                    name: target.name.clone(),
                })
                .collect(),
        };
        artifact.hashes.content = artifact.content_hash();
        artifact
    }

    /// Adds a compiled script record and refreshes the content hash.
    #[must_use]
    pub fn with_compiled_script(mut self, script: CompiledScriptRecord) -> Self {
        self.compiled_scripts.push(script);
        self.hashes.content = self.content_hash();
        self
    }

    /// Adds a compiled style record and refreshes the content hash.
    #[must_use]
    pub fn with_compiled_style(mut self, style: CompiledStyleRecord) -> Self {
        self.compiled_styles.push(style);
        self.hashes.content = self.content_hash();
        self
    }

    /// Adds an asset manifest entry and refreshes the content hash.
    #[must_use]
    pub fn with_asset_manifest_entry(mut self, entry: AssetManifestEntry) -> Self {
        self.asset_manifest.push(entry);
        self.hashes.content = self.content_hash();
        self
    }

    /// Adds a compiled asset record and refreshes the content hash.
    #[must_use]
    pub fn with_compiled_asset(mut self, asset: CompiledAssetRecord) -> Self {
        self.compiled_assets.push(asset);
        self.hashes.content = self.content_hash();
        self
    }

    /// Computes the stable hash for all content-addressed artifact records.
    #[must_use]
    pub fn content_hash(&self) -> ArtifactHash {
        ArtifactHash::from_bytes(self.stable_payload().as_bytes())
    }

    /// Returns true when the artifact schema is compatible with an expected schema.
    #[must_use]
    pub const fn is_compatible_with(&self, expected: ArtifactSchemaVersion) -> bool {
        self.schema_version.is_compatible_with(expected)
    }

    /// Ensures the artifact schema is compatible with an expected schema.
    ///
    /// # Errors
    ///
    /// Returns [`SealedArtifactError`] when the major schema version differs.
    pub fn ensure_compatible_with(
        &self,
        expected: ArtifactSchemaVersion,
    ) -> Result<(), SealedArtifactError> {
        if self.is_compatible_with(expected) {
            Ok(())
        } else {
            Err(SealedArtifactError::IncompatibleSchema {
                expected,
                actual: self.schema_version,
                diagnostic: BuildDiagnostic::new(
                    BuildDiagnosticSeverity::Error,
                    "artifact.schema.incompatible",
                    "sealed artifact schema version is incompatible",
                ),
            })
        }
    }

    fn stable_payload(&self) -> String {
        let mut payload = format!(
            "schema={}.{};manifest={};generator={};profile={};",
            self.schema_version.major,
            self.schema_version.minor,
            self.manifest_snapshot_hash.0,
            self.build_metadata.generator,
            self.build_metadata.profile
        );
        for capability in &self.capabilities {
            payload.push_str("capability=");
            payload.push_str(capability);
            payload.push(';');
        }
        for target in &self.target_metadata {
            payload.push_str("target=");
            payload.push_str(match target.kind {
                PackageTarget::Desktop => "desktop",
                PackageTarget::Plugin => "plugin",
            });
            payload.push(':');
            payload.push_str(&target.name);
            payload.push(';');
        }
        for script in &self.compiled_scripts {
            payload.push_str("script=");
            payload.push_str(&script.entrypoint_id);
            payload.push(':');
            payload.push_str(&script.source_path);
            payload.push(':');
            payload.push_str(&script.artifact_path);
            payload.push(':');
            payload.push_str(&script.source_hash.0);
            payload.push(';');
        }
        for style in &self.compiled_styles {
            payload.push_str("style=");
            payload.push_str(&style.entrypoint_id);
            payload.push(':');
            payload.push_str(&style.source_path);
            payload.push(':');
            payload.push_str(&style.artifact_path);
            payload.push(':');
            payload.push_str(&style.source_hash.0);
            payload.push(';');
        }
        for entry in &self.asset_manifest {
            payload.push_str("asset-manifest=");
            payload.push_str(&entry.id);
            payload.push(':');
            payload.push_str(&entry.kind);
            payload.push(':');
            payload.push_str(&entry.artifact_path);
            payload.push(':');
            payload.push_str(&entry.hash.0);
            payload.push(';');
        }
        for asset in &self.compiled_assets {
            payload.push_str("compiled-asset=");
            payload.push_str(&asset.id);
            payload.push(':');
            payload.push_str(&asset.source_path);
            payload.push(':');
            payload.push_str(&asset.artifact_path);
            payload.push(':');
            payload.push_str(&asset.source_hash.0);
            payload.push(';');
        }
        payload
    }
}

/// Sealed artifact validation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SealedArtifactError {
    /// Artifact schema version is incompatible.
    IncompatibleSchema {
        /// Expected schema version.
        expected: ArtifactSchemaVersion,
        /// Actual schema version.
        actual: ArtifactSchemaVersion,
        /// Structured diagnostic.
        diagnostic: BuildDiagnostic,
    },
}

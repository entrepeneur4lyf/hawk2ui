//! Sealed artifact records and compatibility checks.

use crate::{BuildDiagnostic, BuildDiagnosticSeverity, HawkManifest, PackageTarget};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const SEALED_ARTIFACT_CONTAINER_MAGIC: &[u8] = b"HAWK2UI-ARTIFACT-V1\n";

/// Artifact schema version.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
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
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(transparent)]
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
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompiledScriptRecord {
    /// Stable entrypoint ID.
    pub entrypoint_id: String,
    /// Source file path.
    pub source_path: String,
    /// Artifact-local payload path.
    pub artifact_path: String,
    /// Source content hash.
    pub source_hash: ArtifactHash,
    /// Compiled JavaScript or TypeScript payload carried by the sealed artifact.
    pub compiled_source: String,
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
            compiled_source: String::new(),
        }
    }

    /// Sets the compiled script payload carried by the sealed artifact.
    #[must_use]
    pub fn with_compiled_source(mut self, compiled_source: impl Into<String>) -> Self {
        self.compiled_source = compiled_source.into();
        self
    }
}

/// Compiled style payload recorded in a sealed artifact.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
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
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
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
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
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
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactHashes {
    /// Manifest snapshot hash.
    pub manifest: ArtifactHash,
    /// Full artifact content hash.
    pub content: ArtifactHash,
}

/// Artifact signature verification state.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub enum ArtifactSignatureStatus {
    /// Artifact has no signature and is only acceptable for development policies.
    Unsigned,
    /// Artifact has signature metadata accepted by the release verification policy.
    Verified,
}

/// Signature metadata attached to a sealed artifact.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactSignature {
    /// Signature verification state.
    pub status: ArtifactSignatureStatus,
    /// Signature algorithm label.
    pub algorithm: String,
    /// Signing key identifier.
    pub key_id: String,
    /// Encoded signature payload.
    pub signature: String,
}

impl ArtifactSignature {
    /// Creates unsigned development-only signature metadata.
    #[must_use]
    pub fn unsigned() -> Self {
        Self {
            status: ArtifactSignatureStatus::Unsigned,
            algorithm: "none".into(),
            key_id: String::new(),
            signature: String::new(),
        }
    }

    /// Creates verified signature metadata recorded by an external signing step.
    #[must_use]
    pub fn verified(
        algorithm: impl Into<String>,
        key_id: impl Into<String>,
        signature: impl Into<String>,
    ) -> Self {
        Self {
            status: ArtifactSignatureStatus::Verified,
            algorithm: algorithm.into(),
            key_id: key_id.into(),
            signature: signature.into(),
        }
    }

    fn satisfies_release_policy(&self) -> bool {
        self.status == ArtifactSignatureStatus::Verified
            && !self.algorithm.trim().is_empty()
            && self.algorithm != "none"
            && !self.key_id.trim().is_empty()
            && !self.signature.trim().is_empty()
    }
}

/// Signature policy enforced when serializing or loading sealed artifact containers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactSignaturePolicy {
    /// Allows unsigned artifacts for local development and tests.
    AllowUnsignedDevelopment,
    /// Requires verified signature metadata for release artifacts.
    RequireVerifiedSignature,
}

/// Build metadata embedded into a sealed artifact.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
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
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TargetArtifactMetadata {
    /// Package target kind.
    pub kind: PackageTarget,
    /// Stable target name.
    pub name: String,
}

/// Sealed artifact record consumed by runtime code.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
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
    /// Artifact signature metadata.
    pub signature: ArtifactSignature,
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
            signature: ArtifactSignature::unsigned(),
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

    /// Sets artifact signature metadata and refreshes the content hash.
    #[must_use]
    pub fn with_signature(mut self, signature: ArtifactSignature) -> Self {
        self.signature = signature;
        self.hashes.content = self.content_hash();
        self
    }

    /// Serializes the sealed artifact into deterministic container bytes.
    ///
    /// # Errors
    ///
    /// Returns [`SealedArtifactError`] when the signature policy fails or JSON serialization fails.
    pub fn to_container_bytes(
        &self,
        policy: ArtifactSignaturePolicy,
    ) -> Result<Vec<u8>, SealedArtifactError> {
        self.ensure_signature_policy(policy)?;
        let container = SealedArtifactContainer {
            format: "hawk2ui-sealed-artifact".into(),
            content_hash: self.content_hash(),
            artifact: self.clone(),
        };
        let mut bytes = Vec::from(SEALED_ARTIFACT_CONTAINER_MAGIC);
        let payload = serde_json::to_vec(&container).map_err(|error| {
            SealedArtifactError::ContainerSerialization {
                diagnostic: BuildDiagnostic::new(
                    BuildDiagnosticSeverity::Error,
                    "artifact.container.serialize-failed",
                    format!("sealed artifact container could not be serialized: {error}"),
                ),
            }
        })?;
        bytes.extend(payload);
        Ok(bytes)
    }

    /// Deserializes and verifies deterministic sealed artifact container bytes.
    ///
    /// # Errors
    ///
    /// Returns [`SealedArtifactError`] when the header, schema version, content hash, signature
    /// policy, or JSON payload is invalid.
    pub fn from_container_bytes(
        bytes: &[u8],
        expected_schema_version: ArtifactSchemaVersion,
        policy: ArtifactSignaturePolicy,
    ) -> Result<Self, SealedArtifactError> {
        let Some(payload) = bytes.strip_prefix(SEALED_ARTIFACT_CONTAINER_MAGIC) else {
            return Err(container_verification_error(
                "artifact.container.invalid-header",
                "sealed artifact container header is invalid",
            ));
        };
        let container: SealedArtifactContainer =
            serde_json::from_slice(payload).map_err(|error| {
                container_verification_error(
                    "artifact.container.parse-failed",
                    format!("sealed artifact container payload could not be parsed: {error}"),
                )
            })?;
        if container.format != "hawk2ui-sealed-artifact" {
            return Err(container_verification_error(
                "artifact.container.invalid-format",
                "sealed artifact container format is invalid",
            ));
        }
        container
            .artifact
            .ensure_compatible_with(expected_schema_version)?;
        container.artifact.ensure_signature_policy(policy)?;
        let actual_hash = container.artifact.content_hash();
        if container.content_hash != actual_hash || container.artifact.hashes.content != actual_hash
        {
            return Err(container_verification_error(
                "artifact.container.hash-mismatch",
                "sealed artifact container content hash does not match payload",
            ));
        }
        Ok(container.artifact)
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

    /// Ensures the artifact satisfies the requested signature policy.
    ///
    /// # Errors
    ///
    /// Returns [`SealedArtifactError`] when release policy requires a verified signature and the
    /// artifact does not carry one.
    pub fn ensure_signature_policy(
        &self,
        policy: ArtifactSignaturePolicy,
    ) -> Result<(), SealedArtifactError> {
        match policy {
            ArtifactSignaturePolicy::AllowUnsignedDevelopment => Ok(()),
            ArtifactSignaturePolicy::RequireVerifiedSignature => {
                if self.signature.satisfies_release_policy() {
                    Ok(())
                } else {
                    Err(SealedArtifactError::SignaturePolicy {
                        diagnostic: BuildDiagnostic::new(
                            BuildDiagnosticSeverity::Error,
                            "artifact.signature.required",
                            "release artifacts require verified signature metadata",
                        ),
                    })
                }
            }
        }
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

    /// Generates the JSON Schema used to validate sealed artifact records.
    ///
    /// # Errors
    ///
    /// Returns [`SealedArtifactError`] when the generated schema cannot be represented as JSON.
    pub fn json_schema() -> Result<serde_json::Value, SealedArtifactError> {
        serde_json::to_value(schemars::schema_for!(Self)).map_err(|error| {
            SealedArtifactError::SchemaGeneration {
                diagnostic: BuildDiagnostic::new(
                    BuildDiagnosticSeverity::Error,
                    "artifact.schema.generate-failed",
                    format!("sealed artifact schema could not be serialized: {error}"),
                ),
            }
        })
    }

    /// Validates JSON against the generated sealed artifact schema.
    ///
    /// # Errors
    ///
    /// Returns [`SealedArtifactError`] when schema compilation or validation fails.
    pub fn validate_json(value: &serde_json::Value) -> Result<(), SealedArtifactError> {
        let schema = Self::json_schema()?;
        let validator = jsonschema::Validator::new(&schema).map_err(|error| {
            SealedArtifactError::SchemaValidation {
                diagnostic: BuildDiagnostic::new(
                    BuildDiagnosticSeverity::Error,
                    "artifact.schema.compile-failed",
                    format!("sealed artifact schema could not be compiled: {error}"),
                ),
            }
        })?;
        validator
            .validate(value)
            .map_err(|error| SealedArtifactError::SchemaValidation {
                diagnostic: BuildDiagnostic::new(
                    BuildDiagnosticSeverity::Error,
                    "artifact.schema.invalid",
                    format!(
                        "sealed artifact failed schema validation at {}: {error}",
                        error.instance_path()
                    ),
                ),
            })
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
            payload.push(':');
            payload.push_str(&ArtifactHash::from_bytes(script.compiled_source.as_bytes()).0);
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
        payload.push_str("signature=");
        payload.push_str(match self.signature.status {
            ArtifactSignatureStatus::Unsigned => "unsigned",
            ArtifactSignatureStatus::Verified => "verified",
        });
        payload.push(':');
        payload.push_str(&self.signature.algorithm);
        payload.push(':');
        payload.push_str(&self.signature.key_id);
        payload.push(':');
        payload.push_str(&self.signature.signature);
        payload.push(';');
        payload
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SealedArtifactContainer {
    format: String,
    content_hash: ArtifactHash,
    artifact: SealedArtifact,
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
    /// Artifact schema generation failed.
    SchemaGeneration {
        /// Structured diagnostic.
        diagnostic: BuildDiagnostic,
    },
    /// Artifact schema validation failed.
    SchemaValidation {
        /// Structured diagnostic.
        diagnostic: BuildDiagnostic,
    },
    /// Container serialization failed.
    ContainerSerialization {
        /// Structured diagnostic.
        diagnostic: BuildDiagnostic,
    },
    /// Container verification failed.
    ContainerVerification {
        /// Structured diagnostic.
        diagnostic: BuildDiagnostic,
    },
    /// Signature policy failed.
    SignaturePolicy {
        /// Structured diagnostic.
        diagnostic: BuildDiagnostic,
    },
}

fn container_verification_error(
    rule: impl Into<String>,
    message: impl Into<String>,
) -> SealedArtifactError {
    SealedArtifactError::ContainerVerification {
        diagnostic: BuildDiagnostic::new(BuildDiagnosticSeverity::Error, rule, message),
    }
}

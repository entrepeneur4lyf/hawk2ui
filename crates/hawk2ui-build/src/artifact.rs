//! Sealed artifact records and compatibility checks.

use crate::{
    BuildDiagnostic, BuildDiagnosticSeverity, HawkManifest, PackageTarget,
    SealedJsDependencyOrigin, SealedJsModuleGraph, SourceFramework,
};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

const SEALED_ARTIFACT_CONTAINER_MAGIC: &[u8] = b"HAWK2UI-ARTIFACT-V1\n";
const SEALED_ARTIFACT_SIGNATURE_PAYLOAD_MAGIC: &[u8] = b"HAWK2UI-ARTIFACT-SIGNATURE-V1\n";

/// Supported release artifact signature algorithm.
pub const ARTIFACT_SIGNATURE_ALGORITHM_ED25519_SHA256_V1: &str = "ed25519-sha256-v1";

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
        push_hex(&mut encoded, &digest);
        Self(encoded)
    }
}

fn push_hex(output: &mut String, bytes: &[u8]) {
    for byte in bytes {
        output.push(hex_nibble(byte >> 4));
        output.push(hex_nibble(byte & 0x0f));
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    push_hex(&mut encoded, bytes);
    encoded
}

fn hex_nibble(value: u8) -> char {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    char::from(HEX[usize::from(value & 0x0f)])
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn decode_hex_array<const N: usize>(
    value: &str,
    rule: &'static str,
    message: &'static str,
) -> Result<[u8; N], SealedArtifactError> {
    if value.len() != N * 2 {
        return Err(signature_verification_error(rule, message));
    }

    let mut decoded = [0_u8; N];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let Some(high) = hex_value(chunk[0]) else {
            return Err(signature_verification_error(rule, message));
        };
        let Some(low) = hex_value(chunk[1]) else {
            return Err(signature_verification_error(rule, message));
        };
        decoded[index] = (high << 4) | low;
    }
    Ok(decoded)
}

/// Trusted public key used to verify release artifact signatures.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactSignatureVerificationKey {
    /// Signature algorithm accepted for this key.
    pub algorithm: String,
    /// Stable signing key identifier.
    pub key_id: String,
    /// Hex-encoded Ed25519 public key bytes.
    pub public_key: String,
}

impl ArtifactSignatureVerificationKey {
    /// Creates an Ed25519 release verification key from raw public key bytes.
    #[must_use]
    pub fn ed25519_sha256_v1(key_id: impl Into<String>, public_key: [u8; 32]) -> Self {
        let mut encoded = String::with_capacity(64);
        push_hex(&mut encoded, &public_key);
        Self {
            algorithm: ARTIFACT_SIGNATURE_ALGORITHM_ED25519_SHA256_V1.into(),
            key_id: key_id.into(),
            public_key: encoded,
        }
    }

    /// Creates an Ed25519 release verification key from a hex-encoded public key.
    #[must_use]
    pub fn ed25519_sha256_v1_hex(key_id: impl Into<String>, public_key: impl Into<String>) -> Self {
        Self {
            algorithm: ARTIFACT_SIGNATURE_ALGORITHM_ED25519_SHA256_V1.into(),
            key_id: key_id.into(),
            public_key: public_key.into(),
        }
    }
}

/// Trusted keyring verifier for release artifact signatures.
#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactSignatureVerifier {
    keys: Vec<ArtifactSignatureVerificationKey>,
}

impl ArtifactSignatureVerifier {
    /// Creates a verifier from trusted public keys.
    #[must_use]
    pub fn new(keys: impl IntoIterator<Item = ArtifactSignatureVerificationKey>) -> Self {
        Self {
            keys: keys.into_iter().collect(),
        }
    }

    /// Verifies a sealed artifact signature against the trusted keyring.
    ///
    /// # Errors
    ///
    /// Returns [`SealedArtifactError`] when signature metadata is incomplete, unsupported, untrusted,
    /// malformed, or cryptographically invalid for the artifact signing payload.
    pub fn verify(&self, artifact: &SealedArtifact) -> Result<(), SealedArtifactError> {
        artifact.ensure_signature_policy(ArtifactSignaturePolicy::RequireVerifiedSignature)?;
        let signature = &artifact.signature;
        if signature.algorithm != ARTIFACT_SIGNATURE_ALGORITHM_ED25519_SHA256_V1 {
            return Err(signature_verification_error(
                "artifact.signature.unsupported-algorithm",
                "sealed artifact signature algorithm is not supported",
            ));
        }
        let Some(key) = self
            .keys
            .iter()
            .find(|key| key.algorithm == signature.algorithm && key.key_id == signature.key_id)
        else {
            return Err(signature_verification_error(
                "artifact.signature.untrusted-key",
                "sealed artifact signature key is not trusted",
            ));
        };

        let public_key = decode_hex_array::<32>(
            &key.public_key,
            "artifact.signature.invalid-public-key",
            "sealed artifact signature public key is not valid hex Ed25519 key material",
        )?;
        let signature_bytes = decode_hex_array::<64>(
            &signature.signature,
            "artifact.signature.invalid-signature",
            "sealed artifact signature is not valid hex Ed25519 signature material",
        )?;
        let verifying_key = VerifyingKey::from_bytes(&public_key).map_err(|_| {
            signature_verification_error(
                "artifact.signature.invalid-public-key",
                "sealed artifact signature public key is not valid Ed25519 key material",
            )
        })?;
        let signature = Signature::from_bytes(&signature_bytes);
        verifying_key
            .verify(&artifact.signature_payload_bytes(), &signature)
            .map_err(|_| {
                signature_verification_error(
                    "artifact.signature.invalid",
                    "sealed artifact signature does not match artifact payload",
                )
            })
    }
}

/// Ed25519 signing key used to produce release artifact signatures.
#[derive(Clone, Eq, PartialEq)]
pub struct ArtifactSigningKey {
    key_id: String,
    signing_key: [u8; 32],
}

impl ArtifactSigningKey {
    /// Creates an Ed25519 release artifact signing key from raw private key bytes.
    #[must_use]
    pub fn ed25519_sha256_v1(key_id: impl Into<String>, signing_key: [u8; 32]) -> Self {
        Self {
            key_id: key_id.into(),
            signing_key,
        }
    }

    /// Creates an Ed25519 release artifact signing key from hex private key bytes.
    ///
    /// # Errors
    ///
    /// Returns [`SealedArtifactError`] when the private key is not 32 bytes of hex-encoded
    /// Ed25519 signing material.
    pub fn ed25519_sha256_v1_hex(
        key_id: impl Into<String>,
        signing_key: impl Into<String>,
    ) -> Result<Self, SealedArtifactError> {
        let signing_key = signing_key.into();
        let signing_key = decode_hex_array::<32>(
            &signing_key,
            "artifact.signature.invalid-signing-key",
            "sealed artifact signing key is not valid hex Ed25519 key material",
        )?;
        Ok(Self::ed25519_sha256_v1(key_id, signing_key))
    }

    /// Returns the public verification key corresponding to this signing key.
    #[must_use]
    pub fn verification_key(&self) -> ArtifactSignatureVerificationKey {
        let signing_key = SigningKey::from_bytes(&self.signing_key);
        ArtifactSignatureVerificationKey::ed25519_sha256_v1(
            self.key_id.clone(),
            signing_key.verifying_key().to_bytes(),
        )
    }

    /// Signs a sealed artifact and returns a copy carrying verified release metadata.
    #[must_use]
    pub fn sign(&self, artifact: &SealedArtifact) -> SealedArtifact {
        let signing_key = SigningKey::from_bytes(&self.signing_key);
        let signature = signing_key.sign(&artifact.signature_payload_bytes());
        artifact.clone().with_signature(ArtifactSignature::verified(
            ARTIFACT_SIGNATURE_ALGORITHM_ED25519_SHA256_V1,
            self.key_id.clone(),
            encode_hex(&signature.to_bytes()),
        ))
    }
}

impl fmt::Debug for ArtifactSigningKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ArtifactSigningKey")
            .field("key_id", &self.key_id)
            .field("signing_key", &"<redacted>")
            .finish()
    }
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

/// Compiled framework-native artifact recorded in a sealed artifact.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompiledFrameworkRecord {
    /// Stable entrypoint ID.
    pub entrypoint_id: String,
    /// Source framework that produced the compiler artifact.
    pub framework: SourceFramework,
    /// Source file path.
    pub source_path: String,
    /// Artifact-local payload path.
    pub artifact_path: String,
    /// Source content hash.
    pub source_hash: ArtifactHash,
    /// Canonical framework compiler artifact JSON.
    pub compiler_artifact_json: String,
}

impl CompiledFrameworkRecord {
    /// Creates a compiled framework record.
    #[must_use]
    pub fn new(
        entrypoint_id: impl Into<String>,
        framework: SourceFramework,
        source_path: impl Into<String>,
        artifact_path: impl Into<String>,
        source_hash: ArtifactHash,
    ) -> Self {
        Self {
            entrypoint_id: entrypoint_id.into(),
            framework,
            source_path: source_path.into(),
            artifact_path: artifact_path.into(),
            source_hash,
            compiler_artifact_json: String::new(),
        }
    }

    /// Sets the canonical framework compiler artifact JSON.
    #[must_use]
    pub fn with_compiler_artifact_json(
        mut self,
        compiler_artifact_json: impl Into<String>,
    ) -> Self {
        self.compiler_artifact_json = compiler_artifact_json.into();
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
            && self.algorithm == ARTIFACT_SIGNATURE_ALGORITHM_ED25519_SHA256_V1
            && !self.key_id.trim().is_empty()
            && self.signature.len() == 128
            && self.signature.bytes().all(|byte| byte.is_ascii_hexdigit())
    }
}

/// Signature policy enforced when serializing or loading sealed artifact containers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactSignaturePolicy {
    /// Allows unsigned artifacts for local development and tests.
    AllowUnsignedDevelopment,
    /// Requires structurally valid release signature metadata.
    ///
    /// This does not prove key trust by itself; production loading must use
    /// [`SealedArtifact::from_trusted_container_bytes`] or
    /// [`SealedArtifact::verify_trusted_signature`] with an [`ArtifactSignatureVerifier`].
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
    /// Compiled framework-native payloads.
    pub compiled_frameworks: Vec<CompiledFrameworkRecord>,
    /// Sealed JavaScript module graphs for runtime-rendered applications.
    pub js_module_graphs: Vec<SealedJsModuleGraph>,
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
    /// Optional compiled runtime scene payload for native host/plugin rendering.
    pub runtime_scene: Option<serde_json::Value>,
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
            compiled_frameworks: Vec::new(),
            js_module_graphs: Vec::new(),
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
            runtime_scene: None,
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

    /// Adds a compiled framework record and refreshes the content hash.
    #[must_use]
    pub fn with_compiled_framework(mut self, framework: CompiledFrameworkRecord) -> Self {
        self.compiled_frameworks.push(framework);
        self.hashes.content = self.content_hash();
        self
    }

    /// Adds a sealed JavaScript module graph and refreshes the content hash.
    #[must_use]
    pub fn with_js_module_graph(mut self, graph: SealedJsModuleGraph) -> Self {
        self.js_module_graphs.push(graph);
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

    /// Adds a compiled runtime scene payload and refreshes the content hash.
    #[must_use]
    pub fn with_runtime_scene_payload(mut self, runtime_scene: serde_json::Value) -> Self {
        self.runtime_scene = Some(runtime_scene);
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
        container.artifact.ensure_manifest_snapshot_integrity()?;
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

    /// Deserializes a container and verifies the release signature against a trusted keyring.
    ///
    /// # Errors
    ///
    /// Returns [`SealedArtifactError`] when container verification fails or when the artifact
    /// signature is missing, malformed, untrusted, or cryptographically invalid.
    pub fn from_trusted_container_bytes(
        bytes: &[u8],
        expected_schema_version: ArtifactSchemaVersion,
        verifier: &ArtifactSignatureVerifier,
    ) -> Result<Self, SealedArtifactError> {
        let artifact = Self::from_container_bytes(
            bytes,
            expected_schema_version,
            ArtifactSignaturePolicy::RequireVerifiedSignature,
        )?;
        artifact.verify_trusted_signature(verifier)?;
        Ok(artifact)
    }

    /// Computes the stable hash for all content-addressed artifact records.
    #[must_use]
    pub fn content_hash(&self) -> ArtifactHash {
        ArtifactHash::from_bytes(self.stable_payload().as_bytes())
    }

    /// Returns the canonical bytes that release signing keys must sign.
    #[must_use]
    pub fn signature_payload_bytes(&self) -> Vec<u8> {
        let mut payload = Vec::from(SEALED_ARTIFACT_SIGNATURE_PAYLOAD_MAGIC);
        payload.extend(self.stable_payload_without_signature().as_bytes());
        payload
    }

    /// Verifies the artifact signature against a trusted keyring.
    ///
    /// # Errors
    ///
    /// Returns [`SealedArtifactError`] when signature metadata is missing, unsupported, untrusted,
    /// malformed, or cryptographically invalid for the artifact signing payload.
    pub fn verify_trusted_signature(
        &self,
        verifier: &ArtifactSignatureVerifier,
    ) -> Result<(), SealedArtifactError> {
        verifier.verify(self)
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

    fn ensure_manifest_snapshot_integrity(&self) -> Result<(), SealedArtifactError> {
        let actual_manifest_hash = ArtifactHash::from_bytes(self.manifest_snapshot.as_bytes());
        if self.manifest_snapshot_hash != actual_manifest_hash
            || self.hashes.manifest != actual_manifest_hash
        {
            return Err(container_verification_error(
                "artifact.container.manifest-hash-mismatch",
                "sealed artifact manifest snapshot hash does not match payload",
            ));
        }
        Ok(())
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
        self.stable_payload_with_signature(true)
    }

    fn stable_payload_without_signature(&self) -> String {
        self.stable_payload_with_signature(false)
    }

    fn stable_payload_with_signature(&self, include_signature: bool) -> String {
        let mut payload = format!(
            "schema={}.{};manifest={};manifest-snapshot={};generator={};profile={};",
            self.schema_version.major,
            self.schema_version.minor,
            self.manifest_snapshot_hash.0,
            ArtifactHash::from_bytes(self.manifest_snapshot.as_bytes()).0,
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
        append_script_payloads(&mut payload, &self.compiled_scripts);
        append_framework_payloads(&mut payload, &self.compiled_frameworks);
        append_js_module_graph_payloads(&mut payload, &self.js_module_graphs);
        append_style_payloads(&mut payload, &self.compiled_styles);
        append_asset_manifest_payloads(&mut payload, &self.asset_manifest);
        append_compiled_asset_payloads(&mut payload, &self.compiled_assets);
        if let Some(runtime_scene) = &self.runtime_scene {
            payload.push_str("runtime-scene=");
            payload.push_str(&ArtifactHash::from_bytes(runtime_scene.to_string().as_bytes()).0);
            payload.push(';');
        }
        if include_signature {
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
        }
        payload
    }
}

fn append_script_payloads(payload: &mut String, scripts: &[CompiledScriptRecord]) {
    for script in scripts {
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
}

fn append_framework_payloads(payload: &mut String, frameworks: &[CompiledFrameworkRecord]) {
    for framework in frameworks {
        payload.push_str("framework=");
        payload.push_str(&framework.entrypoint_id);
        payload.push(':');
        payload.push_str(source_framework_label(framework.framework));
        payload.push(':');
        payload.push_str(&framework.source_path);
        payload.push(':');
        payload.push_str(&framework.artifact_path);
        payload.push(':');
        payload.push_str(&framework.source_hash.0);
        payload.push(':');
        payload.push_str(&ArtifactHash::from_bytes(framework.compiler_artifact_json.as_bytes()).0);
        payload.push(';');
    }
}

fn append_js_module_graph_payloads(payload: &mut String, graphs: &[SealedJsModuleGraph]) {
    for graph in graphs {
        payload.push_str("js-graph=");
        payload.push_str(graph.entrypoint());
        payload.push(':');
        payload.push_str(graph.package_manager().kind.as_str());
        if let Some(lockfile_sha256) = &graph.package_manager().lockfile_sha256 {
            payload.push(':');
            payload.push_str(lockfile_sha256);
        }
        if let Some(package_manager_version) = &graph.package_manager().package_manager_version {
            payload.push(':');
            payload.push_str(package_manager_version);
        }
        payload.push(';');

        for module in graph.modules() {
            payload.push_str("js-module=");
            payload.push_str(module.specifier());
            payload.push(':');
            payload.push_str(module.sha256());
            append_js_dependency_origin_payload(payload, module.dependency_origin());
            if let Some(chunk) = module.chunk() {
                payload.push(':');
                payload.push_str(chunk);
            }
            if let Some(source_map) = module.source_map() {
                payload.push_str(":source-map=");
                payload.push_str(&source_map.sha256());
            }
            payload.push(':');
            payload.push_str(&ArtifactHash::from_bytes(module.source().as_bytes()).0);
            payload.push(';');
            for target in module.static_imports() {
                payload.push_str("js-static-import=");
                payload.push_str(module.specifier());
                payload.push_str("->");
                payload.push_str(target);
                payload.push(';');
            }
            for target in module.dynamic_imports() {
                payload.push_str("js-dynamic-import=");
                payload.push_str(module.specifier());
                payload.push_str("->");
                payload.push_str(target);
                payload.push(';');
            }
        }

        for chunk in graph.chunks() {
            payload.push_str("js-chunk=");
            payload.push_str(chunk.id());
            for module in chunk.modules() {
                payload.push(':');
                payload.push_str(module);
            }
            payload.push(';');
        }
    }
}

fn append_js_dependency_origin_payload(
    payload: &mut String,
    dependency_origin: &SealedJsDependencyOrigin,
) {
    payload.push_str(":origin=");
    match dependency_origin {
        SealedJsDependencyOrigin::Workspace { path } => {
            payload.push_str("workspace:");
            payload.push_str(path);
        }
        SealedJsDependencyOrigin::Package { name, version } => {
            payload.push_str("package:");
            payload.push_str(name);
            if let Some(version) = version {
                payload.push('@');
                payload.push_str(version);
            }
        }
        SealedJsDependencyOrigin::Generated { tool } => {
            payload.push_str("generated:");
            payload.push_str(tool);
        }
    }
}

fn append_style_payloads(payload: &mut String, styles: &[CompiledStyleRecord]) {
    for style in styles {
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
}

fn append_asset_manifest_payloads(payload: &mut String, entries: &[AssetManifestEntry]) {
    for entry in entries {
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
}

fn append_compiled_asset_payloads(payload: &mut String, assets: &[CompiledAssetRecord]) {
    for asset in assets {
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
    /// Artifact signature verification failed.
    SignatureVerification {
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

fn source_framework_label(framework: SourceFramework) -> &'static str {
    match framework {
        SourceFramework::Native => "native",
        SourceFramework::React => "react",
        SourceFramework::Solid => "solid",
        SourceFramework::Svelte => "svelte",
        SourceFramework::Vue => "vue",
    }
}

fn signature_verification_error(
    rule: impl Into<String>,
    message: impl Into<String>,
) -> SealedArtifactError {
    SealedArtifactError::SignatureVerification {
        diagnostic: BuildDiagnostic::new(BuildDiagnosticSeverity::Error, rule, message),
    }
}

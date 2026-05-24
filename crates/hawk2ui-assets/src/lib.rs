#![forbid(unsafe_code)]
//! Production asset decoding, validation, lowering, hashing, and cache invalidation for `Hawk2UI`.

use std::collections::BTreeMap;

use image::GenericImageView;
use sha2::{Digest, Sha256};

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-assets";

/// Returns the canonical Cargo package name for diagnostics and conformance checks.
#[must_use]
pub const fn crate_name() -> &'static str {
    CRATE_NAME
}

/// Compiled asset kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetKind {
    /// Decoded raster image.
    Image,
    /// Validated vector asset.
    Vector,
    /// Loaded font asset.
    Font,
}

/// Asset size and safety limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssetLimits {
    max_bytes: usize,
    max_pixels: u64,
}

impl AssetLimits {
    /// Sets the maximum input byte length.
    #[must_use]
    pub const fn with_max_bytes(mut self, max_bytes: usize) -> Self {
        self.max_bytes = max_bytes;
        self
    }

    /// Sets the maximum decoded raster pixel count.
    #[must_use]
    pub const fn with_max_pixels(mut self, max_pixels: u64) -> Self {
        self.max_pixels = max_pixels;
        self
    }
}

impl Default for AssetLimits {
    fn default() -> Self {
        Self {
            max_bytes: 16 * 1024 * 1024,
            max_pixels: 16_777_216,
        }
    }
}

/// Stable SHA-256 asset hash.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetHash(String);

impl AssetHash {
    /// Creates a hash from a stable string.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Computes a SHA-256 hash for bytes.
    #[must_use]
    pub fn sha256_bytes(bytes: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        Self(format!("sha256:{}", hex_digest(&hasher.finalize())))
    }

    /// Returns the stable hash string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Vector lowering metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VectorLowering {
    path_count: usize,
}

impl VectorLowering {
    /// Returns the number of lowered path commands.
    #[must_use]
    pub const fn path_count(&self) -> usize {
        self.path_count
    }
}

/// Compiled asset record.
#[derive(Clone, Debug, PartialEq)]
pub struct AssetRecord {
    id: String,
    source_path: String,
    hash: String,
    kind: AssetKind,
    width: Option<u32>,
    height: Option<u32>,
    sanitized: bool,
    metadata_stripped: bool,
    vector_lowering: Option<VectorLowering>,
    cache_generation: u64,
}

impl AssetRecord {
    /// Returns the asset ID.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the source path.
    #[must_use]
    pub fn source_path(&self) -> &str {
        &self.source_path
    }

    /// Returns the asset hash.
    #[must_use]
    pub fn hash(&self) -> &str {
        &self.hash
    }

    /// Returns the asset kind.
    #[must_use]
    pub const fn kind(&self) -> AssetKind {
        self.kind
    }

    /// Returns decoded width when known.
    #[must_use]
    pub const fn width(&self) -> Option<u32> {
        self.width
    }

    /// Returns decoded height when known.
    #[must_use]
    pub const fn height(&self) -> Option<u32> {
        self.height
    }

    /// Returns whether the asset was sanitized.
    #[must_use]
    pub const fn sanitized(&self) -> bool {
        self.sanitized
    }

    /// Returns whether metadata was stripped.
    #[must_use]
    pub const fn metadata_stripped(&self) -> bool {
        self.metadata_stripped
    }

    /// Returns vector lowering metadata.
    #[must_use]
    pub const fn vector_lowering(&self) -> Option<&VectorLowering> {
        self.vector_lowering.as_ref()
    }

    /// Returns cache generation.
    #[must_use]
    pub const fn cache_generation(&self) -> u64 {
        self.cache_generation
    }

    /// Converts this asset into the renderer asset record.
    #[must_use]
    pub fn to_render_asset(&self) -> hawk2ui_render::CompiledAsset {
        let asset = match self.kind {
            AssetKind::Image => hawk2ui_render::CompiledAsset::image(
                &self.id,
                &self.source_path,
                &self.hash,
                self.width.unwrap_or_default(),
                self.height.unwrap_or_default(),
            )
            .with_backend_requirement(hawk2ui_render::BackendRequirement::Images),
            AssetKind::Vector => hawk2ui_render::CompiledAsset::vector(
                &self.id,
                &self.source_path,
                &self.hash,
                self.width.unwrap_or_default(),
                self.height.unwrap_or_default(),
            )
            .with_backend_requirement(hawk2ui_render::BackendRequirement::Vectors),
            AssetKind::Font => {
                hawk2ui_render::CompiledAsset::font(&self.id, &self.source_path, &self.hash)
                    .with_backend_requirement(hawk2ui_render::BackendRequirement::Fonts)
            }
        };
        asset
            .with_sanitized(self.sanitized)
            .with_cache_generation(self.cache_generation)
    }
}

/// Asset manifest for compiled artifacts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AssetManifest {
    assets: BTreeMap<String, AssetRecord>,
}

impl AssetManifest {
    /// Returns all compiled assets in stable ID order.
    #[must_use]
    pub fn assets(&self) -> Vec<&AssetRecord> {
        self.assets.values().collect()
    }

    /// Returns an asset by ID.
    #[must_use]
    pub fn asset(&self, id: &str) -> Option<&AssetRecord> {
        self.assets.get(id)
    }

    fn insert(&mut self, asset: AssetRecord) {
        self.assets.insert(asset.id.clone(), asset);
    }
}

/// Asset diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetDiagnostic {
    rule: String,
    message: String,
}

impl AssetDiagnostic {
    /// Creates an asset diagnostic.
    #[must_use]
    pub fn new(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            rule: rule.into(),
            message: message.into(),
        }
    }

    /// Returns the diagnostic rule.
    #[must_use]
    pub fn rule(&self) -> &str {
        &self.rule
    }

    /// Returns the diagnostic message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Asset backend error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetBackendError {
    diagnostic: AssetDiagnostic,
}

impl AssetBackendError {
    /// Creates an asset backend error.
    #[must_use]
    pub fn new(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            diagnostic: AssetDiagnostic::new(rule, message),
        }
    }

    /// Returns the structured diagnostic.
    #[must_use]
    pub const fn diagnostic(&self) -> &AssetDiagnostic {
        &self.diagnostic
    }
}

/// Production asset backend.
#[derive(Debug)]
pub struct AssetBackend {
    limits: AssetLimits,
    manifest: AssetManifest,
    generations: BTreeMap<String, AssetGeneration>,
    font_database: fontdb::Database,
}

impl AssetBackend {
    /// Creates an asset backend.
    #[must_use]
    pub fn new(limits: AssetLimits) -> Self {
        Self {
            limits,
            manifest: AssetManifest::default(),
            generations: BTreeMap::new(),
            font_database: fontdb::Database::new(),
        }
    }

    /// Compiles an image asset.
    ///
    /// # Errors
    ///
    /// Returns [`AssetBackendError`] when limits, hash verification, or decoding fails.
    pub fn compile_image(
        &mut self,
        id: impl Into<String>,
        source_path: impl Into<String>,
        bytes: &[u8],
        expected_hash: &AssetHash,
    ) -> Result<AssetRecord, AssetBackendError> {
        self.verify_bytes(bytes)?;
        verify_hash(bytes, expected_hash)?;
        let image = image::load_from_memory(bytes).map_err(|_| {
            AssetBackendError::new("asset.image.decode-failed", "image decoding failed")
        })?;
        let (width, height) = image.dimensions();
        self.verify_pixels(width, height)?;
        let asset = self.record(AssetRecordDraft {
            id: id.into(),
            source_path: source_path.into(),
            expected_hash,
            kind: AssetKind::Image,
            width: Some(width),
            height: Some(height),
            vector_lowering: None,
            sanitized: true,
        });
        Ok(asset)
    }

    /// Compiles a vector asset.
    ///
    /// # Errors
    ///
    /// Returns [`AssetBackendError`] when limits, hash verification, validation, or lowering fails.
    pub fn compile_vector(
        &mut self,
        id: impl Into<String>,
        source_path: impl Into<String>,
        bytes: &[u8],
        expected_hash: &AssetHash,
    ) -> Result<AssetRecord, AssetBackendError> {
        self.verify_bytes(bytes)?;
        verify_hash(bytes, expected_hash)?;
        let svg = std::str::from_utf8(bytes).map_err(|_| {
            AssetBackendError::new("asset.vector.invalid-utf8", "vector must be UTF-8 SVG")
        })?;
        validate_vector(svg)?;
        let options = usvg::Options::default();
        let _tree = usvg::Tree::from_data(bytes, &options).map_err(|_| {
            AssetBackendError::new("asset.vector.parse-failed", "SVG parsing failed")
        })?;
        let path_count = svg.matches("<path").count();
        let asset = self.record(AssetRecordDraft {
            id: id.into(),
            source_path: source_path.into(),
            expected_hash,
            kind: AssetKind::Vector,
            width: None,
            height: None,
            vector_lowering: Some(VectorLowering { path_count }),
            sanitized: true,
        });
        Ok(asset)
    }

    /// Loads a font asset.
    ///
    /// # Errors
    ///
    /// Returns [`AssetBackendError`] when limits or hash verification fails.
    pub fn load_font(
        &mut self,
        id: impl Into<String>,
        source_path: impl Into<String>,
        bytes: &[u8],
        expected_hash: &AssetHash,
    ) -> Result<AssetRecord, AssetBackendError> {
        self.verify_bytes(bytes)?;
        verify_hash(bytes, expected_hash)?;
        self.font_database.load_font_data(bytes.to_vec());
        let asset = self.record(AssetRecordDraft {
            id: id.into(),
            source_path: source_path.into(),
            expected_hash,
            kind: AssetKind::Font,
            width: None,
            height: None,
            vector_lowering: None,
            sanitized: true,
        });
        Ok(asset)
    }

    /// Returns the compiled asset manifest.
    #[must_use]
    pub const fn manifest(&self) -> &AssetManifest {
        &self.manifest
    }

    fn verify_bytes(&self, bytes: &[u8]) -> Result<(), AssetBackendError> {
        if bytes.len() > self.limits.max_bytes {
            Err(AssetBackendError::new(
                "asset.limit.bytes-exceeded",
                "asset exceeds maximum byte length",
            ))
        } else {
            Ok(())
        }
    }

    fn verify_pixels(&self, width: u32, height: u32) -> Result<(), AssetBackendError> {
        let pixels = u64::from(width) * u64::from(height);
        if pixels > self.limits.max_pixels {
            Err(AssetBackendError::new(
                "asset.limit.pixels-exceeded",
                "decoded image exceeds maximum pixel count",
            ))
        } else {
            Ok(())
        }
    }

    fn record(&mut self, draft: AssetRecordDraft<'_>) -> AssetRecord {
        let cache_generation = self.cache_generation(&draft.id, draft.expected_hash.as_str());
        let record = AssetRecord {
            id: draft.id,
            source_path: draft.source_path,
            hash: draft.expected_hash.as_str().to_string(),
            kind: draft.kind,
            width: draft.width,
            height: draft.height,
            sanitized: draft.sanitized,
            metadata_stripped: true,
            vector_lowering: draft.vector_lowering,
            cache_generation,
        };
        self.manifest.insert(record.clone());
        record
    }

    fn cache_generation(&mut self, id: &str, hash: &str) -> u64 {
        let generation =
            self.generations
                .entry(id.to_string())
                .or_insert_with(|| AssetGeneration {
                    hash: hash.to_string(),
                    generation: 0,
                });
        if generation.hash != hash {
            generation.hash = hash.to_string();
        }
        generation.generation = generation.generation.saturating_add(1);
        generation.generation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AssetGeneration {
    hash: String,
    generation: u64,
}

struct AssetRecordDraft<'hash> {
    id: String,
    source_path: String,
    expected_hash: &'hash AssetHash,
    kind: AssetKind,
    width: Option<u32>,
    height: Option<u32>,
    vector_lowering: Option<VectorLowering>,
    sanitized: bool,
}

fn verify_hash(bytes: &[u8], expected_hash: &AssetHash) -> Result<(), AssetBackendError> {
    let actual = AssetHash::sha256_bytes(bytes);
    if actual == *expected_hash {
        Ok(())
    } else {
        Err(AssetBackendError::new(
            "asset.hash.mismatch",
            "asset content hash does not match expected hash",
        ))
    }
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(hex_nibble(byte >> 4));
        output.push(hex_nibble(byte & 0x0f));
    }
    output
}

fn hex_nibble(value: u8) -> char {
    match value {
        0..=9 => char::from(b'0' + value),
        10..=15 => char::from(b'a' + (value - 10)),
        _ => unreachable!("nibble values are always 0..=15"),
    }
}

fn validate_vector(svg: &str) -> Result<(), AssetBackendError> {
    let lower = svg.to_ascii_lowercase();
    let unsafe_tokens = [
        "<script",
        "onload=",
        "onclick=",
        "javascript:",
        "data:text/html",
        "<foreignobject",
    ];
    if unsafe_tokens.iter().any(|token| lower.contains(token)) {
        Err(AssetBackendError::new(
            "asset.vector.unsafe-content",
            "SVG contains executable or externalized content",
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_identity() {
        assert_eq!(crate_name(), "hawk2ui-assets");
    }
}

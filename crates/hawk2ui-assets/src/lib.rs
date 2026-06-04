#![forbid(unsafe_code)]
//! Production asset decoding, validation, lowering, hashing, and cache invalidation for `Hawk2UI`.

use std::{collections::BTreeMap, io::Cursor, sync::Arc};

use image::{ExtendedColorType, GenericImageView, ImageReader, Limits, codecs::webp::WebPEncoder};
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
    compiled_hash: String,
    compiled_bytes: Vec<u8>,
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

    /// Returns the hash of the sanitized compiled payload.
    #[must_use]
    pub fn compiled_hash(&self) -> &str {
        &self.compiled_hash
    }

    /// Returns the sanitized compiled payload bytes.
    #[must_use]
    pub fn compiled_bytes(&self) -> &[u8] {
        &self.compiled_bytes
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
                &self.compiled_hash,
                self.width.unwrap_or_default(),
                self.height.unwrap_or_default(),
            )
            .with_backend_requirement(hawk2ui_render::BackendRequirement::Images),
            AssetKind::Vector => hawk2ui_render::CompiledAsset::vector(
                &self.id,
                &self.source_path,
                &self.compiled_hash,
                self.width.unwrap_or_default(),
                self.height.unwrap_or_default(),
            )
            .with_backend_requirement(hawk2ui_render::BackendRequirement::Vectors),
            AssetKind::Font => hawk2ui_render::CompiledAsset::font(
                &self.id,
                &self.source_path,
                &self.compiled_hash,
            )
            .with_backend_requirement(hawk2ui_render::BackendRequirement::Fonts),
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

impl From<AssetBackendError> for hawk2ui_api::Diagnostic {
    fn from(error: AssetBackendError) -> Self {
        hawk2ui_api::Diagnostic::error(error.diagnostic.rule, error.diagnostic.message)
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
        let image = decode_limited_image(bytes, self.limits)?;
        let (width, height) = image.dimensions();
        self.verify_pixels(width, height)?;
        let compiled_bytes = encode_lossless_webp(&image)?;
        let compiled_hash = AssetHash::sha256_bytes(&compiled_bytes);
        let asset = self.record(AssetRecordDraft {
            id: id.into(),
            source_path: source_path.into(),
            expected_hash,
            compiled_hash: compiled_hash.as_str().to_string(),
            compiled_bytes,
            kind: AssetKind::Image,
            width: Some(width),
            height: Some(height),
            vector_lowering: None,
            sanitized: true,
            metadata_stripped: true,
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
        let options = hardened_svg_options();
        let tree = usvg::Tree::from_data(bytes, &options).map_err(|_| {
            AssetBackendError::new("asset.vector.parse-failed", "SVG parsing failed")
        })?;
        let vector_size = tree.size();
        let path_count = count_vector_paths(tree.root())?;
        let compiled_payload = normalize_svg_payload(&tree)?;
        validate_vector(&compiled_payload)?;
        let compiled_bytes = compiled_payload.into_bytes();
        let compiled_hash = AssetHash::sha256_bytes(&compiled_bytes);
        let asset = self.record(AssetRecordDraft {
            id: id.into(),
            source_path: source_path.into(),
            expected_hash,
            compiled_hash: compiled_hash.as_str().to_string(),
            compiled_bytes,
            kind: AssetKind::Vector,
            width: vector_dimension_to_u32(vector_size.width()),
            height: vector_dimension_to_u32(vector_size.height()),
            vector_lowering: Some(VectorLowering { path_count }),
            sanitized: true,
            metadata_stripped: false,
        });
        Ok(asset)
    }

    /// Loads a font asset.
    ///
    /// # Errors
    ///
    /// Returns [`AssetBackendError`] when limits, hash verification, or font parsing fails.
    pub fn load_font(
        &mut self,
        id: impl Into<String>,
        source_path: impl Into<String>,
        bytes: &[u8],
        expected_hash: &AssetHash,
    ) -> Result<AssetRecord, AssetBackendError> {
        self.verify_bytes(bytes)?;
        verify_hash(bytes, expected_hash)?;
        let loaded_faces = self
            .font_database
            .load_font_source(fontdb::Source::Binary(Arc::new(bytes.to_vec())));
        if loaded_faces.is_empty() {
            return Err(AssetBackendError::new(
                "asset.font.parse-failed",
                "font parsing produced no usable faces",
            ));
        }
        let compiled_hash = AssetHash::sha256_bytes(bytes);
        let asset = self.record(AssetRecordDraft {
            id: id.into(),
            source_path: source_path.into(),
            expected_hash,
            compiled_hash: compiled_hash.as_str().to_string(),
            compiled_bytes: bytes.to_vec(),
            kind: AssetKind::Font,
            width: None,
            height: None,
            vector_lowering: None,
            sanitized: false,
            metadata_stripped: false,
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
        let cache_generation = self.cache_generation(&draft.id, &draft.compiled_hash);
        let record = AssetRecord {
            id: draft.id,
            source_path: draft.source_path,
            hash: draft.expected_hash.as_str().to_string(),
            compiled_hash: draft.compiled_hash,
            compiled_bytes: draft.compiled_bytes,
            kind: draft.kind,
            width: draft.width,
            height: draft.height,
            sanitized: draft.sanitized,
            metadata_stripped: draft.metadata_stripped,
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
                    generation: 1,
                });
        if generation.hash != hash {
            generation.hash = hash.to_string();
            generation.generation = generation.generation.saturating_add(1);
        }
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
    compiled_hash: String,
    compiled_bytes: Vec<u8>,
    kind: AssetKind,
    width: Option<u32>,
    height: Option<u32>,
    vector_lowering: Option<VectorLowering>,
    sanitized: bool,
    metadata_stripped: bool,
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
    const HEX: &[u8; 16] = b"0123456789abcdef";
    char::from(HEX[usize::from(value & 0x0f)])
}

fn validate_vector(svg: &str) -> Result<(), AssetBackendError> {
    let lower = svg.to_ascii_lowercase();
    let unsafe_tokens = [
        "<script",
        "<iframe",
        "<object",
        "<embed",
        "<audio",
        "<video",
        "<image",
        "<animate",
        "onload=",
        "onclick=",
        "onmouseover=",
        "onerror=",
        "javascript:",
        "data:text/html",
        "@import",
        "<foreignobject",
    ];
    if unsafe_tokens.iter().any(|token| lower.contains(token)) {
        Err(AssetBackendError::new(
            "asset.vector.unsafe-content",
            "SVG contains executable or externalized content",
        ))
    } else if contains_external_reference(&lower) {
        Err(AssetBackendError::new(
            "asset.vector.external-reference",
            "SVG contains an external reference",
        ))
    } else {
        Ok(())
    }
}

fn contains_external_reference(svg: &str) -> bool {
    contains_external_url(svg)
        || contains_external_href(svg, "href")
        || contains_external_href(svg, "xlink:href")
}

fn contains_external_url(svg: &str) -> bool {
    let mut remaining = svg;
    while let Some(index) = remaining.find("url(") {
        let after = &remaining[index + 4..];
        let trimmed = after.trim_start_matches([' ', '\t', '\n', '\r', '\'', '"']);
        if !trimmed.starts_with('#') {
            return true;
        }
        remaining = after;
    }
    false
}

fn contains_external_href(svg: &str, attribute: &str) -> bool {
    let mut remaining = svg;
    let needle = format!("{attribute}=");
    while let Some(index) = remaining.find(&needle) {
        let after = &remaining[index + needle.len()..];
        let trimmed = after.trim_start_matches([' ', '\t', '\n', '\r']);
        let Some(quote) = trimmed
            .chars()
            .next()
            .filter(|quote| matches!(quote, '"' | '\''))
        else {
            return true;
        };
        let value = &trimmed[quote.len_utf8()..];
        if !value.starts_with('#') {
            return true;
        }
        remaining = value;
    }
    false
}

fn hardened_svg_options() -> usvg::Options<'static> {
    usvg::Options {
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(|_, _, _| None),
            resolve_string: Box::new(|_, _| None),
        },
        ..Default::default()
    }
}

fn decode_limited_image(
    bytes: &[u8],
    limits: AssetLimits,
) -> Result<image::DynamicImage, AssetBackendError> {
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| {
            AssetBackendError::new("asset.image.decode-failed", "image decoding failed")
        })?;
    let Some(format) = reader.format() else {
        return Err(AssetBackendError::new(
            "asset.image.decode-failed",
            "image decoding failed",
        ));
    };
    let (width, height) = reader.into_dimensions().map_err(|_| {
        AssetBackendError::new("asset.image.decode-failed", "image decoding failed")
    })?;
    verify_pixels_against_limits(width, height, limits)?;

    let image_limits = image_limits(limits);
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(image_limits);
    reader
        .decode()
        .map_err(|_| AssetBackendError::new("asset.image.decode-failed", "image decoding failed"))
}

fn image_limits(limits: AssetLimits) -> Limits {
    let max_dimension = u32::try_from(limits.max_pixels).unwrap_or(u32::MAX);
    let mut image_limits = Limits::default();
    image_limits.max_image_width = Some(max_dimension);
    image_limits.max_image_height = Some(max_dimension);
    image_limits.max_alloc = limits.max_pixels.checked_mul(4);
    image_limits
}

fn verify_pixels_against_limits(
    width: u32,
    height: u32,
    limits: AssetLimits,
) -> Result<(), AssetBackendError> {
    let pixels = u64::from(width) * u64::from(height);
    if pixels > limits.max_pixels {
        Err(AssetBackendError::new(
            "asset.limit.pixels-exceeded",
            "decoded image exceeds maximum pixel count",
        ))
    } else {
        Ok(())
    }
}

fn vector_dimension_to_u32(value: f32) -> Option<u32> {
    const MAX_U32_AS_F32: f32 = 4_294_967_040.0;
    if !value.is_finite() || value <= 0.0 {
        return None;
    }
    let rounded = value.ceil();
    if rounded > MAX_U32_AS_F32 {
        return None;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Some(rounded as u32)
}

fn encode_lossless_webp(image: &image::DynamicImage) -> Result<Vec<u8>, AssetBackendError> {
    let rgba = image.to_rgba8();
    let (width, height) = image.dimensions();
    let mut encoded = Vec::new();
    WebPEncoder::new_lossless(&mut encoded)
        .encode(rgba.as_raw(), width, height, ExtendedColorType::Rgba8)
        .map_err(|_| {
            AssetBackendError::new(
                "asset.image.encode-failed",
                "image sanitization WebP encoding failed",
            )
        })?;
    Ok(encoded)
}

fn count_vector_paths(root: &usvg::Group) -> Result<usize, AssetBackendError> {
    const MAX_VECTOR_GROUP_DEPTH: usize = 256;
    let mut count = 0usize;
    let mut stack = vec![(root, 0usize)];
    while let Some((group, depth)) = stack.pop() {
        if depth > MAX_VECTOR_GROUP_DEPTH {
            return Err(AssetBackendError::new(
                "asset.vector.max-depth",
                "SVG group nesting exceeds maximum depth",
            ));
        }
        for node in group.children() {
            match node {
                usvg::Node::Group(child) => stack.push((child, depth + 1)),
                usvg::Node::Path(_) => count = count.saturating_add(1),
                usvg::Node::Image(_) | usvg::Node::Text(_) => {}
            }
        }
    }
    Ok(count)
}

fn normalize_svg_payload(tree: &usvg::Tree) -> Result<String, AssetBackendError> {
    let payload = tree.to_string(&usvg::WriteOptions::default());
    if payload.trim().is_empty() {
        return Err(AssetBackendError::new(
            "asset.vector.empty-output",
            "SVG lowering produced an empty payload",
        ));
    }
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_identity() {
        assert_eq!(crate_name(), "hawk2ui-assets");
    }
}

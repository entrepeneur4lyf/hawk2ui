//! Asset compilation records and validation.

use std::collections::BTreeMap;

use crate::{
    ArtifactHash, BuildDiagnostic, BuildDiagnosticSeverity, HawkManifest,
    manifest::AssetDeclaration,
};
use hawk2ui_assets::{AssetBackend, AssetHash as BackendAssetHash, AssetLimits};

/// Supported asset kinds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetKind {
    /// Raster image asset.
    Image,
    /// Vector graphic asset.
    Vector,
    /// Font asset.
    Font,
    /// Design token document.
    DesignToken,
}

impl AssetKind {
    fn parse(value: &str) -> Result<Self, AssetCompilationError> {
        match value {
            "image" => Ok(Self::Image),
            "vector" => Ok(Self::Vector),
            "font" => Ok(Self::Font),
            "design-token" => Ok(Self::DesignToken),
            other => Err(AssetCompilationError::UnsupportedAssetKind {
                kind: other.into(),
                diagnostic: Box::new(BuildDiagnostic::new(
                    BuildDiagnosticSeverity::Error,
                    "asset.kind.unsupported",
                    "declared asset kind is unsupported",
                )),
            }),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Vector => "vector",
            Self::Font => "font",
            Self::DesignToken => "design-token",
        }
    }
}

/// Asset dimensions in physical pixels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssetDimensions {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl AssetDimensions {
    /// Creates asset dimensions.
    #[must_use]
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

/// Sanitization status for a compiled asset.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetSanitizationStatus {
    /// Asset was accepted without modification.
    Clean,
    /// Asset was rewritten by a sanitizer.
    Sanitized,
}

/// Package metadata for a compiled asset.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetPackageMetadata {
    /// Artifact-local package path.
    pub package_path: String,
    /// Cache key that changes when content or compilation parameters change.
    pub cache_key: String,
    /// Monotonic cache generation format version.
    pub cache_generation: u32,
}

/// Complete asset compilation record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetCompilationRecord {
    /// Stable asset ID.
    pub id: String,
    /// Asset kind.
    pub kind: AssetKind,
    /// Source file path.
    pub source_path: String,
    /// Source content hash.
    pub source_hash: ArtifactHash,
    /// Optional source dimensions.
    pub dimensions: Option<AssetDimensions>,
    /// Sanitization result.
    pub sanitization: AssetSanitizationStatus,
    /// Package metadata.
    pub package: AssetPackageMetadata,
}

/// In-memory asset source used by deterministic build tests and adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetSource {
    path: String,
    bytes: Vec<u8>,
    dimensions: Option<AssetDimensions>,
    safe: bool,
}

impl AssetSource {
    /// Creates a safe asset source.
    #[must_use]
    pub fn new(path: impl Into<String>, bytes: impl AsRef<[u8]>) -> Self {
        Self {
            path: path.into(),
            bytes: bytes.as_ref().to_vec(),
            dimensions: None,
            safe: true,
        }
    }

    /// Adds source dimensions.
    #[must_use]
    pub const fn with_dimensions(mut self, dimensions: AssetDimensions) -> Self {
        self.dimensions = Some(dimensions);
        self
    }

    /// Marks the source as failing safety validation.
    #[must_use]
    pub const fn unsafe_asset(mut self) -> Self {
        self.safe = false;
        self
    }
}

/// Lookup table for declared asset sources.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AssetSourceIndex {
    sources: BTreeMap<String, AssetSource>,
}

impl AssetSourceIndex {
    /// Creates an asset source index.
    #[must_use]
    pub fn new(sources: impl IntoIterator<Item = AssetSource>) -> Self {
        Self {
            sources: sources
                .into_iter()
                .map(|source| (source.path.clone(), source))
                .collect(),
        }
    }

    /// Creates an empty asset source index.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    fn get(&self, path: &str) -> Option<&AssetSource> {
        self.sources.get(path)
    }
}

/// Asset compilation plan.
pub struct AssetCompilationPlan;

impl AssetCompilationPlan {
    /// Compiles all assets declared in a validated manifest.
    ///
    /// # Errors
    ///
    /// Returns [`AssetCompilationError`] when an asset is missing, unsafe, or unsupported.
    pub fn compile_manifest(
        manifest: &HawkManifest,
        sources: &AssetSourceIndex,
    ) -> Result<Vec<AssetCompilationRecord>, AssetCompilationError> {
        let mut backend = AssetBackend::new(AssetLimits::default());
        manifest
            .assets
            .iter()
            .map(|asset| compile_asset(asset, sources, &mut backend))
            .collect()
    }
}

fn compile_asset(
    asset: &AssetDeclaration,
    sources: &AssetSourceIndex,
    backend: &mut AssetBackend,
) -> Result<AssetCompilationRecord, AssetCompilationError> {
    let kind = AssetKind::parse(&asset.kind)?;
    let source = asset_source(asset, sources)?;
    let source_hash = ArtifactHash::from_bytes(&source.bytes);
    let backend_record = compile_with_backend(asset, source, kind, backend)?;
    let dimensions =
        backend_record
            .as_ref()
            .and_then(|record| match (record.width(), record.height()) {
                (Some(width), Some(height)) => Some(AssetDimensions::new(width, height)),
                _ => source.dimensions,
            });
    let sanitization = backend_record
        .as_ref()
        .map_or(AssetSanitizationStatus::Clean, |record| {
            if record.sanitized() {
                AssetSanitizationStatus::Sanitized
            } else {
                AssetSanitizationStatus::Clean
            }
        });
    let cache_generation = backend_record.as_ref().map_or(1, |record| {
        u32::try_from(record.cache_generation()).unwrap_or(u32::MAX)
    });
    let cache_key = format!("{}:{}:{}", kind.as_str(), asset.id, source_hash.0);
    Ok(AssetCompilationRecord {
        id: asset.id.clone(),
        kind,
        source_path: asset.path.clone(),
        source_hash,
        dimensions,
        sanitization,
        package: AssetPackageMetadata {
            package_path: package_path_for(&asset.path),
            cache_key,
            cache_generation,
        },
    })
}

fn asset_source<'source>(
    asset: &AssetDeclaration,
    sources: &'source AssetSourceIndex,
) -> Result<&'source AssetSource, AssetCompilationError> {
    let source = sources
        .get(&asset.path)
        .ok_or_else(|| AssetCompilationError::MissingAsset {
            id: asset.id.clone(),
            path: asset.path.clone(),
            diagnostic: Box::new(BuildDiagnostic::new(
                BuildDiagnosticSeverity::Error,
                "asset.missing",
                "declared asset source is missing",
            )),
        })?;
    if source.safe {
        Ok(source)
    } else {
        Err(AssetCompilationError::UnsafeAsset {
            id: asset.id.clone(),
            path: asset.path.clone(),
            diagnostic: Box::new(BuildDiagnostic::new(
                BuildDiagnosticSeverity::Error,
                "asset.unsafe",
                "declared asset failed safety validation",
            )),
        })
    }
}

fn compile_with_backend(
    asset: &AssetDeclaration,
    source: &AssetSource,
    kind: AssetKind,
    backend: &mut AssetBackend,
) -> Result<Option<hawk2ui_assets::AssetRecord>, AssetCompilationError> {
    let backend_hash = BackendAssetHash::sha256_bytes(&source.bytes);
    match kind {
        AssetKind::Image => {
            Some(backend.compile_image(&asset.id, &asset.path, &source.bytes, &backend_hash))
        }
        AssetKind::Vector => {
            Some(backend.compile_vector(&asset.id, &asset.path, &source.bytes, &backend_hash))
        }
        AssetKind::Font => {
            Some(backend.load_font(&asset.id, &asset.path, &source.bytes, &backend_hash))
        }
        AssetKind::DesignToken => {
            validate_design_token(&source.bytes).map_err(|diagnostic| {
                AssetCompilationError::UnsafeAsset {
                    id: asset.id.clone(),
                    path: asset.path.clone(),
                    diagnostic: Box::new(diagnostic),
                }
            })?;
            None
        }
    }
    .transpose()
    .map_err(|error| AssetCompilationError::UnsafeAsset {
        id: asset.id.clone(),
        path: asset.path.clone(),
        diagnostic: Box::new(BuildDiagnostic::new(
            BuildDiagnosticSeverity::Error,
            error.diagnostic().rule(),
            error.diagnostic().message(),
        )),
    })
}

/// Asset compilation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AssetCompilationError {
    /// Declared asset source is missing.
    MissingAsset {
        /// Stable asset ID.
        id: String,
        /// Declared source path.
        path: String,
        /// Structured diagnostic.
        diagnostic: Box<BuildDiagnostic>,
    },
    /// Declared asset failed safety validation.
    UnsafeAsset {
        /// Stable asset ID.
        id: String,
        /// Declared source path.
        path: String,
        /// Structured diagnostic.
        diagnostic: Box<BuildDiagnostic>,
    },
    /// Declared asset kind is unsupported.
    UnsupportedAssetKind {
        /// Unsupported kind string.
        kind: String,
        /// Structured diagnostic.
        diagnostic: Box<BuildDiagnostic>,
    },
}

fn package_path_for(path: &str) -> String {
    match path.rsplit_once('.') {
        Some((stem, _extension)) => format!("{stem}.pack"),
        None => format!("{path}.pack"),
    }
}

fn validate_design_token(bytes: &[u8]) -> Result<(), BuildDiagnostic> {
    let source = std::str::from_utf8(bytes).map_err(|_| {
        BuildDiagnostic::new(
            BuildDiagnosticSeverity::Error,
            "asset.design-token.invalid-utf8",
            "design token assets must be UTF-8 JSON or TOML",
        )
    })?;
    if serde_json::from_str::<serde_json::Value>(source).is_ok()
        || toml::from_str::<toml::Value>(source).is_ok()
    {
        Ok(())
    } else {
        Err(BuildDiagnostic::new(
            BuildDiagnosticSeverity::Error,
            "asset.design-token.parse-failed",
            "design token assets must parse as JSON or TOML",
        ))
    }
}

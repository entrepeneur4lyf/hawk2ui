//! Compiled asset render records.

/// Compiled asset kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetKind {
    /// Image asset.
    Image,
    /// Vector asset.
    Vector,
    /// Font asset.
    Font,
}

/// Backend requirement for an asset.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackendRequirement {
    /// Image rendering support.
    Images,
    /// Vector rendering support.
    Vectors,
    /// Font rendering support.
    Fonts,
}

/// Compiled asset record.
#[derive(Clone, Debug, PartialEq)]
pub struct CompiledAsset {
    id: String,
    source_path: String,
    hash: String,
    kind: AssetKind,
    width: Option<u32>,
    height: Option<u32>,
    sanitized: bool,
    backend_requirements: Vec<BackendRequirement>,
    package_path: Option<String>,
    cache_generation: u64,
}

impl CompiledAsset {
    /// Creates a compiled image asset.
    #[must_use]
    pub fn image(
        id: impl Into<String>,
        source_path: impl Into<String>,
        hash: impl Into<String>,
        width: u32,
        height: u32,
    ) -> Self {
        Self::new(
            id,
            source_path,
            hash,
            AssetKind::Image,
            Some(width),
            Some(height),
        )
    }

    /// Creates a compiled vector asset.
    #[must_use]
    pub fn vector(
        id: impl Into<String>,
        source_path: impl Into<String>,
        hash: impl Into<String>,
        width: u32,
        height: u32,
    ) -> Self {
        Self::new(
            id,
            source_path,
            hash,
            AssetKind::Vector,
            Some(width),
            Some(height),
        )
    }

    /// Creates a compiled font asset.
    #[must_use]
    pub fn font(
        id: impl Into<String>,
        source_path: impl Into<String>,
        hash: impl Into<String>,
    ) -> Self {
        Self::new(id, source_path, hash, AssetKind::Font, None, None)
    }

    fn new(
        id: impl Into<String>,
        source_path: impl Into<String>,
        hash: impl Into<String>,
        kind: AssetKind,
        width: Option<u32>,
        height: Option<u32>,
    ) -> Self {
        Self {
            id: id.into(),
            source_path: source_path.into(),
            hash: hash.into(),
            kind,
            width,
            height,
            sanitized: false,
            backend_requirements: Vec::new(),
            package_path: None,
            cache_generation: 0,
        }
    }

    /// Marks sanitization status.
    #[must_use]
    pub const fn with_sanitized(mut self, sanitized: bool) -> Self {
        self.sanitized = sanitized;
        self
    }

    /// Adds a backend requirement.
    #[must_use]
    pub fn with_backend_requirement(mut self, requirement: BackendRequirement) -> Self {
        if !self.backend_requirements.contains(&requirement) {
            self.backend_requirements.push(requirement);
        }
        self
    }

    /// Adds package path metadata.
    #[must_use]
    pub fn with_package_path(mut self, package_path: impl Into<String>) -> Self {
        self.package_path = Some(package_path.into());
        self
    }

    /// Sets cache generation.
    #[must_use]
    pub const fn with_cache_generation(mut self, cache_generation: u64) -> Self {
        self.cache_generation = cache_generation;
        self
    }

    /// Returns asset ID.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns asset kind.
    #[must_use]
    pub const fn kind(&self) -> AssetKind {
        self.kind
    }

    /// Returns package path.
    #[must_use]
    pub fn package_path(&self) -> Option<&str> {
        self.package_path.as_deref()
    }

    /// Returns stable asset key.
    #[must_use]
    pub fn stable_key(&self) -> String {
        let dimensions = self.width.zip(self.height).map_or_else(
            || "unbounded".to_string(),
            |(width, height)| format!("{width}x{height}"),
        );
        format!(
            "{}:{}:{}:{}:sanitized={}:cache={}",
            self.kind_key(),
            self.id,
            self.hash,
            dimensions,
            self.sanitized,
            self.cache_generation
        )
    }

    fn kind_key(&self) -> &'static str {
        match self.kind {
            AssetKind::Image => "image",
            AssetKind::Vector => "vector",
            AssetKind::Font => "font",
        }
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

    /// Returns the stable diagnostic rule.
    #[must_use]
    pub fn rule(&self) -> &str {
        &self.rule
    }
}

/// Asset error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetError {
    diagnostic: AssetDiagnostic,
}

impl AssetError {
    /// Creates an asset error.
    #[must_use]
    pub fn new(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            diagnostic: AssetDiagnostic::new(rule, message),
        }
    }

    /// Returns structured diagnostic.
    #[must_use]
    pub const fn diagnostic(&self) -> &AssetDiagnostic {
        &self.diagnostic
    }
}

/// Asset draw record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetDrawRecord {
    asset_id: String,
}

impl AssetDrawRecord {
    /// Creates an asset draw record from a compiled asset.
    #[must_use]
    pub fn from_compiled(asset: &CompiledAsset) -> Self {
        Self {
            asset_id: asset.id.clone(),
        }
    }

    /// Rejects raw path drawing.
    ///
    /// # Errors
    ///
    /// Always returns [`AssetError`] because renderers must consume compiled asset records.
    pub fn from_raw_path(_path: &str) -> Result<Self, AssetError> {
        Err(AssetError::new(
            "asset.raw-reference.rejected",
            "renderers must draw compiled assets, not raw paths",
        ))
    }

    /// Returns asset ID.
    #[must_use]
    pub fn asset_id(&self) -> &str {
        &self.asset_id
    }
}

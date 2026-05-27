//! Renderer backend boundary and recording test backend.

use crate::{Color, Geometry, Stroke, Transform};

/// Renderer backend capabilities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackendCapabilities {
    /// GPU acceleration support.
    pub gpu: bool,
    /// Text rendering support.
    pub text: bool,
    /// Image rendering support.
    pub images: bool,
}

impl BackendCapabilities {
    /// Creates an empty capability report.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            gpu: false,
            text: false,
            images: false,
        }
    }

    /// Sets GPU support.
    #[must_use]
    pub const fn with_gpu(mut self, gpu: bool) -> Self {
        self.gpu = gpu;
        self
    }

    /// Sets text support.
    #[must_use]
    pub const fn with_text(mut self, text: bool) -> Self {
        self.text = text;
        self
    }

    /// Sets image support.
    #[must_use]
    pub const fn with_images(mut self, images: bool) -> Self {
        self.images = images;
        self
    }
}

impl Default for BackendCapabilities {
    fn default() -> Self {
        Self::new()
    }
}

/// Backend diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendDiagnostic {
    rule: String,
    message: String,
}

impl BackendDiagnostic {
    /// Creates a backend diagnostic.
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
}

/// Backend error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendError {
    diagnostic: BackendDiagnostic,
}

impl BackendError {
    /// Creates a backend error.
    #[must_use]
    pub fn new(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            diagnostic: BackendDiagnostic::new(rule, message),
        }
    }

    /// Returns the structured diagnostic.
    #[must_use]
    pub const fn diagnostic(&self) -> &BackendDiagnostic {
        &self.diagnostic
    }
}

/// Backend cache handle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendCacheHandle(String);

impl BackendCacheHandle {
    /// Creates a backend cache handle.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the cache handle as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Renderer backend trait.
pub trait RendererBackend {
    /// Creates a render surface.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when surface creation fails.
    fn create_surface(&mut self, id: &str, width: u32, height: u32) -> Result<(), BackendError>;
    /// Tears down a render surface.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when surface teardown fails.
    fn teardown_surface(&mut self, id: &str) -> Result<(), BackendError>;
    /// Resizes a render surface and updates DPI scale.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when resize or DPI update fails.
    fn resize_surface(
        &mut self,
        id: &str,
        width: u32,
        height: u32,
        dpi_scale: f32,
    ) -> Result<(), BackendError>;
    /// Begins a frame.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when frame setup fails.
    fn begin_frame(&mut self, id: &str) -> Result<(), BackendError>;
    /// Ends a frame.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when frame finalization fails.
    fn end_frame(&mut self, id: &str) -> Result<(), BackendError>;
    /// Clears the surface.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when clearing fails.
    fn clear(&mut self, color: Color) -> Result<(), BackendError>;
    /// Fills geometry.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when fill drawing fails.
    fn fill(&mut self, geometry: Geometry, color: Color) -> Result<(), BackendError>;
    /// Strokes geometry.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when stroke drawing fails.
    fn stroke(&mut self, geometry: Geometry, stroke: Stroke) -> Result<(), BackendError>;
    /// Draws a path.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when path drawing fails.
    fn draw_path(&mut self, path: &str) -> Result<(), BackendError>;
    /// Draws text.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when text drawing fails or text support is unavailable.
    fn draw_text(&mut self, text: &str) -> Result<(), BackendError>;
    /// Draws image by asset ID.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when image drawing fails or image support is unavailable.
    fn draw_image(&mut self, image: &str) -> Result<(), BackendError>;
    /// Pushes a clip.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when clip setup fails.
    fn push_clip(&mut self, geometry: Geometry) -> Result<(), BackendError>;
    /// Pushes a transform.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when transform setup fails.
    fn push_transform(&mut self, transform: Transform) -> Result<(), BackendError>;
    /// Applies a layer effect.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the layer effect cannot be applied.
    fn apply_layer_effect(&mut self, effect: &str) -> Result<(), BackendError>;
    /// Creates a cache handle.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when cache allocation fails.
    fn create_cache_handle(&mut self, id: &str) -> Result<BackendCacheHandle, BackendError>;
    /// Marks a dirty region.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when dirty-region tracking fails.
    fn mark_dirty(&mut self, geometry: Geometry) -> Result<(), BackendError>;
    /// Returns backend capabilities.
    fn capabilities(&self) -> BackendCapabilities;
}

/// Renderer backend extension for explicit cache invalidation.
pub trait RendererCacheInvalidator {
    /// Invalidates a backend cache entry by stable cache ID.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the cache ID is invalid or the backend cannot invalidate the
    /// entry.
    fn invalidate_backend_cache(&mut self, id: &str) -> Result<(), BackendError>;
}

/// Recording renderer backend used by tests.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordingBackend {
    capabilities: BackendCapabilities,
    commands: Vec<String>,
    dirty_regions: Vec<Geometry>,
    cache_invalidation_keys: Vec<String>,
}

impl RecordingBackend {
    /// Creates a recording backend.
    #[must_use]
    pub const fn new(capabilities: BackendCapabilities) -> Self {
        Self {
            capabilities,
            commands: Vec::new(),
            dirty_regions: Vec::new(),
            cache_invalidation_keys: Vec::new(),
        }
    }

    /// Returns recorded command keys.
    #[must_use]
    pub fn command_keys(&self) -> &[String] {
        &self.commands
    }

    /// Returns recorded dirty regions.
    #[must_use]
    pub fn dirty_regions(&self) -> &[Geometry] {
        &self.dirty_regions
    }

    /// Returns cache IDs invalidated explicitly through the cache invalidator extension.
    #[must_use]
    pub fn cache_invalidation_keys(&self) -> &[String] {
        &self.cache_invalidation_keys
    }

    /// Returns backend capabilities.
    #[must_use]
    pub const fn capabilities(&self) -> BackendCapabilities {
        self.capabilities
    }
}

impl RendererBackend for RecordingBackend {
    fn create_surface(&mut self, id: &str, width: u32, height: u32) -> Result<(), BackendError> {
        validate_surface_id(id)?;
        validate_surface_size(width, height)?;
        self.commands
            .push(format!("create-surface:{id}:{width}x{height}"));
        Ok(())
    }

    fn teardown_surface(&mut self, id: &str) -> Result<(), BackendError> {
        self.commands.push(format!("teardown-surface:{id}"));
        Ok(())
    }

    fn resize_surface(
        &mut self,
        id: &str,
        width: u32,
        height: u32,
        dpi_scale: f32,
    ) -> Result<(), BackendError> {
        validate_surface_id(id)?;
        validate_surface_size(width, height)?;
        validate_dpi_scale(dpi_scale)?;
        self.commands
            .push(format!("resize-surface:{id}:{width}x{height}@{dpi_scale}"));
        Ok(())
    }

    fn begin_frame(&mut self, id: &str) -> Result<(), BackendError> {
        self.commands.push(format!("begin-frame:{id}"));
        Ok(())
    }

    fn end_frame(&mut self, id: &str) -> Result<(), BackendError> {
        self.commands.push(format!("end-frame:{id}"));
        Ok(())
    }

    fn clear(&mut self, color: Color) -> Result<(), BackendError> {
        self.commands.push(format!(
            "clear:{},{},{},{}",
            color.r, color.g, color.b, color.a
        ));
        Ok(())
    }

    fn fill(&mut self, geometry: Geometry, color: Color) -> Result<(), BackendError> {
        validate_geometry(geometry)?;
        self.commands.push(format!(
            "fill:{},{},{},{}:{},{},{},{}",
            geometry.x,
            geometry.y,
            geometry.width,
            geometry.height,
            color.r,
            color.g,
            color.b,
            color.a
        ));
        Ok(())
    }

    fn stroke(&mut self, geometry: Geometry, stroke: Stroke) -> Result<(), BackendError> {
        validate_geometry(geometry)?;
        self.commands.push(format!(
            "stroke:{},{},{},{}:{}",
            geometry.x, geometry.y, geometry.width, geometry.height, stroke.width
        ));
        Ok(())
    }

    fn draw_path(&mut self, path: &str) -> Result<(), BackendError> {
        self.commands.push(format!("path:{path}"));
        Ok(())
    }

    fn draw_text(&mut self, text: &str) -> Result<(), BackendError> {
        if !self.capabilities.text {
            return Err(BackendError::new(
                "backend.capability.text.missing",
                "backend does not support text rendering",
            ));
        }
        self.commands.push(format!("text:{text}"));
        Ok(())
    }

    fn draw_image(&mut self, image: &str) -> Result<(), BackendError> {
        if !self.capabilities.images {
            return Err(BackendError::new(
                "backend.capability.image.missing",
                "backend does not support image rendering",
            ));
        }
        self.commands.push(format!("image:{image}"));
        Ok(())
    }

    fn push_clip(&mut self, geometry: Geometry) -> Result<(), BackendError> {
        validate_geometry(geometry)?;
        self.commands.push(format!(
            "clip:{},{},{},{}",
            geometry.x, geometry.y, geometry.width, geometry.height
        ));
        Ok(())
    }

    fn push_transform(&mut self, transform: Transform) -> Result<(), BackendError> {
        self.commands.push(format!(
            "transform:{},{},{},{},{},{}",
            transform.scale_x,
            transform.skew_x,
            transform.skew_y,
            transform.scale_y,
            transform.translate_x,
            transform.translate_y
        ));
        Ok(())
    }

    fn apply_layer_effect(&mut self, effect: &str) -> Result<(), BackendError> {
        self.commands.push(format!("effect:{effect}"));
        Ok(())
    }

    fn create_cache_handle(&mut self, id: &str) -> Result<BackendCacheHandle, BackendError> {
        self.commands.push(format!("cache:{id}"));
        Ok(BackendCacheHandle::new(id))
    }

    fn mark_dirty(&mut self, geometry: Geometry) -> Result<(), BackendError> {
        validate_geometry(geometry)?;
        self.commands.push(format!(
            "dirty:{},{},{},{}",
            geometry.x, geometry.y, geometry.width, geometry.height
        ));
        self.dirty_regions.push(geometry);
        Ok(())
    }

    fn capabilities(&self) -> BackendCapabilities {
        self.capabilities
    }
}

impl RendererCacheInvalidator for RecordingBackend {
    fn invalidate_backend_cache(&mut self, id: &str) -> Result<(), BackendError> {
        validate_surface_id(id)?;
        self.commands.push(format!("invalidate-cache:{id}"));
        self.cache_invalidation_keys.push(id.to_string());
        Ok(())
    }
}

fn validate_surface_id(id: &str) -> Result<(), BackendError> {
    if id.trim().is_empty() {
        Err(BackendError::new(
            "backend.surface.id.invalid",
            "surface ID must not be empty",
        ))
    } else {
        Ok(())
    }
}

fn validate_surface_size(width: u32, height: u32) -> Result<(), BackendError> {
    if width == 0 || height == 0 {
        Err(BackendError::new(
            "backend.surface.size.invalid",
            "surface dimensions must be greater than zero",
        ))
    } else {
        Ok(())
    }
}

fn validate_dpi_scale(dpi_scale: f32) -> Result<(), BackendError> {
    if !dpi_scale.is_finite() || dpi_scale <= 0.0 {
        Err(BackendError::new(
            "backend.surface.dpi.invalid",
            "surface DPI scale must be finite and greater than zero",
        ))
    } else {
        Ok(())
    }
}

fn validate_geometry(geometry: Geometry) -> Result<(), BackendError> {
    if [geometry.x, geometry.y, geometry.width, geometry.height]
        .iter()
        .all(|value| value.is_finite())
        && geometry.width >= 0.0
        && geometry.height >= 0.0
    {
        Ok(())
    } else {
        Err(BackendError::new(
            "backend.geometry.invalid",
            "geometry values must be finite with non-negative dimensions",
        ))
    }
}

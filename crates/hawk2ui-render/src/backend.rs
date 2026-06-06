//! Renderer backend boundary and recording test backend.

use hawk2ui_api::Diagnostic;

use crate::{Color, Geometry, Stroke, Transform};

/// Renderer backend capabilities.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackendCapabilities {
    /// GPU acceleration support.
    pub gpu: bool,
    /// Text rendering support.
    pub text: bool,
    /// Image rendering support.
    pub images: bool,
    /// Runtime shader effect support.
    pub runtime_effects: bool,
}

impl BackendCapabilities {
    /// Creates an empty capability report.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            gpu: false,
            text: false,
            images: false,
            runtime_effects: false,
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

    /// Sets runtime shader effect support.
    #[must_use]
    pub const fn with_runtime_effects(mut self, runtime_effects: bool) -> Self {
        self.runtime_effects = runtime_effects;
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

    /// Returns the diagnostic message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
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

impl From<BackendError> for Diagnostic {
    fn from(error: BackendError) -> Self {
        Self::error(error.diagnostic.rule, error.diagnostic.message)
    }
}

/// Numeric uniform binding for a backend-neutral runtime shader effect.
#[derive(Clone, Debug, PartialEq)]
pub struct ShaderEffectUniform {
    name: String,
    value: ShaderEffectUniformValue,
}

impl ShaderEffectUniform {
    /// Creates a scalar float uniform binding.
    #[must_use]
    pub fn float(name: impl Into<String>, value: f32) -> Self {
        Self {
            name: name.into(),
            value: ShaderEffectUniformValue::Float(vec![value]),
        }
    }

    /// Creates a `float2` uniform binding.
    #[must_use]
    pub fn float2(name: impl Into<String>, value: [f32; 2]) -> Self {
        Self {
            name: name.into(),
            value: ShaderEffectUniformValue::Float(value.to_vec()),
        }
    }

    /// Creates a `float3` uniform binding.
    #[must_use]
    pub fn float3(name: impl Into<String>, value: [f32; 3]) -> Self {
        Self {
            name: name.into(),
            value: ShaderEffectUniformValue::Float(value.to_vec()),
        }
    }

    /// Creates a `float4` uniform binding.
    #[must_use]
    pub fn float4(name: impl Into<String>, value: [f32; 4]) -> Self {
        Self {
            name: name.into(),
            value: ShaderEffectUniformValue::Float(value.to_vec()),
        }
    }

    /// Creates a float or float-array uniform binding with caller-supplied arity.
    #[must_use]
    pub fn floats(name: impl Into<String>, values: impl Into<Vec<f32>>) -> Self {
        Self {
            name: name.into(),
            value: ShaderEffectUniformValue::Float(values.into()),
        }
    }

    /// Creates a scalar int uniform binding.
    #[must_use]
    pub fn int(name: impl Into<String>, value: i32) -> Self {
        Self {
            name: name.into(),
            value: ShaderEffectUniformValue::Int(vec![value]),
        }
    }

    /// Creates an `int2` uniform binding.
    #[must_use]
    pub fn int2(name: impl Into<String>, value: [i32; 2]) -> Self {
        Self {
            name: name.into(),
            value: ShaderEffectUniformValue::Int(value.to_vec()),
        }
    }

    /// Creates an int or int-array uniform binding with caller-supplied arity.
    #[must_use]
    pub fn ints(name: impl Into<String>, values: impl Into<Vec<i32>>) -> Self {
        Self {
            name: name.into(),
            value: ShaderEffectUniformValue::Int(values.into()),
        }
    }

    /// Returns the shader uniform name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the typed uniform value.
    #[must_use]
    pub const fn value(&self) -> &ShaderEffectUniformValue {
        &self.value
    }
}

/// Backend-neutral runtime shader uniform value.
#[derive(Clone, Debug, PartialEq)]
pub enum ShaderEffectUniformValue {
    /// Floating-point scalar, vector, matrix, or array data.
    Float(Vec<f32>),
    /// Signed integer scalar, vector, or array data.
    Int(Vec<i32>),
}

/// Image child binding for a backend-neutral runtime shader effect.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShaderEffectChildInput {
    name: String,
    asset_id: String,
}

impl ShaderEffectChildInput {
    /// Creates an image child shader binding by child name and registered image asset ID.
    #[must_use]
    pub fn image(name: impl Into<String>, asset_id: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            asset_id: asset_id.into(),
        }
    }

    /// Returns the shader child name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the registered image asset ID used as this child shader.
    #[must_use]
    pub fn asset_id(&self) -> &str {
        &self.asset_id
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
    /// Registers a runtime shader effect with backend-neutral source.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when runtime shader effects are unsupported or the source cannot
    /// be accepted by the backend.
    fn register_runtime_shader_effect(
        &mut self,
        id: &str,
        source: &str,
    ) -> Result<(), BackendError>;
    /// Draws geometry filled by a registered runtime shader effect.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when runtime shader effects are unsupported, geometry is invalid,
    /// the effect is missing, or uniform/child bindings are invalid for the backend.
    fn draw_runtime_effect(
        &mut self,
        effect_id: &str,
        geometry: Geometry,
        uniforms: &[ShaderEffectUniform],
        children: &[ShaderEffectChildInput],
    ) -> Result<(), BackendError>;
    /// Begins an opacity compositing group.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the opacity is invalid or the backend cannot allocate a group.
    fn begin_opacity_group(&mut self, opacity: f32) -> Result<(), BackendError>;
    /// Ends the current opacity compositing group.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when no opacity group is active.
    fn end_opacity_group(&mut self) -> Result<(), BackendError>;
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
    opacity_group_depth: usize,
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
            opacity_group_depth: 0,
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

    fn register_runtime_shader_effect(
        &mut self,
        id: &str,
        source: &str,
    ) -> Result<(), BackendError> {
        if !self.capabilities.runtime_effects {
            return Err(BackendError::new(
                "backend.capability.runtime-effect.missing",
                "backend does not support runtime shader effects",
            ));
        }
        validate_surface_id(id)?;
        if source.trim().is_empty() {
            return Err(BackendError::new(
                "backend.runtime-effect.source.invalid",
                "runtime shader effect source must not be empty",
            ));
        }
        self.commands.push(format!(
            "runtime-effect-register:{id}:bytes={}",
            source.len()
        ));
        Ok(())
    }

    fn draw_runtime_effect(
        &mut self,
        effect_id: &str,
        geometry: Geometry,
        uniforms: &[ShaderEffectUniform],
        children: &[ShaderEffectChildInput],
    ) -> Result<(), BackendError> {
        if !self.capabilities.runtime_effects {
            return Err(BackendError::new(
                "backend.capability.runtime-effect.missing",
                "backend does not support runtime shader effects",
            ));
        }
        validate_surface_id(effect_id)?;
        validate_geometry(geometry)?;
        for uniform in uniforms {
            validate_surface_id(uniform.name())?;
        }
        for child in children {
            validate_surface_id(child.name())?;
            validate_surface_id(child.asset_id())?;
        }
        self.commands.push(format!(
            "runtime-effect:{effect_id}:{},{},{},{}:uniforms={}:children={}",
            geometry.x,
            geometry.y,
            geometry.width,
            geometry.height,
            uniforms.len(),
            children.len()
        ));
        Ok(())
    }

    fn begin_opacity_group(&mut self, opacity: f32) -> Result<(), BackendError> {
        validate_opacity(opacity)?;
        self.opacity_group_depth = self.opacity_group_depth.saturating_add(1);
        self.commands.push(format!("begin-opacity-group:{opacity}"));
        Ok(())
    }

    fn end_opacity_group(&mut self) -> Result<(), BackendError> {
        if self.opacity_group_depth == 0 {
            return Err(BackendError::new(
                "backend.opacity-group.unbalanced",
                "cannot end opacity group because none is active",
            ));
        }
        self.opacity_group_depth -= 1;
        self.commands.push("end-opacity-group".to_string());
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

fn validate_opacity(opacity: f32) -> Result<(), BackendError> {
    if opacity.is_finite() && (0.0..=1.0).contains(&opacity) {
        Ok(())
    } else {
        Err(BackendError::new(
            "backend.opacity-group.invalid",
            "opacity group alpha must be finite and within 0.0..=1.0",
        ))
    }
}

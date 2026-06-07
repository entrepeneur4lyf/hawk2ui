//! Test doubles for renderer backend conformance.

use crate::{
    BackendCacheHandle, BackendCapabilities, BackendError, Color, Geometry, RendererBackend,
    RendererCacheInvalidator, ShaderEffectChildInput, ShaderEffectUniform, Stroke, Transform,
    backend::{
        validate_dpi_scale, validate_geometry, validate_opacity, validate_surface_id,
        validate_surface_size,
    },
};

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

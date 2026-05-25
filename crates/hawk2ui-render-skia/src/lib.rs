#![forbid(unsafe_code)]
//! `Skia`-backed production renderer backend for `Hawk2UI`.

use std::collections::BTreeMap;

use hawk2ui_render::{
    BackendCacheHandle, BackendCapabilities, BackendDiagnostic, BackendError, Color, Geometry,
    RendererBackend, Stroke, Transform,
};
use skia_safe::{
    Canvas, ClipOp, Color as SkiaColor, Data, Font, Image, Paint, PaintStyle, Path, Rect, Surface,
    surfaces,
};

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-render-skia";

/// Returns the canonical Cargo package name for diagnostics and conformance checks.
#[must_use]
pub const fn crate_name() -> &'static str {
    CRATE_NAME
}

/// Backing surface implementation used by the renderer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SkiaSurfaceKind {
    /// CPU raster surface. This is the required first production backend.
    CpuRaster,
}

impl SkiaSurfaceKind {
    /// Returns the stable surface-kind key.
    #[must_use]
    pub const fn stable_key(self) -> &'static str {
        match self {
            Self::CpuRaster => "cpu-raster",
        }
    }
}

/// Detailed capability report for the Skia adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SkiaRendererCapabilities {
    /// CPU raster surface support.
    pub cpu_raster: SkiaCapabilitySupport,
    /// GPU surface support.
    pub gpu: SkiaCapabilitySupport,
    /// Arbitrary path rendering support.
    pub paths: SkiaCapabilitySupport,
    /// Clip stack support.
    pub clips: SkiaCapabilitySupport,
    /// Transform stack support.
    pub transforms: SkiaCapabilitySupport,
    /// Text rendering support.
    pub text: SkiaCapabilitySupport,
    /// Compiled image asset rendering support.
    pub images: SkiaCapabilitySupport,
    /// Compiled vector asset rendering support.
    pub vectors: SkiaCapabilitySupport,
    /// Layer effect command support.
    pub effects: SkiaCapabilitySupport,
    /// Dirty-region tracking support.
    pub dirty_regions: SkiaCapabilitySupport,
}

/// Support state for a Skia adapter capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SkiaCapabilitySupport {
    /// Capability is supported.
    Supported,
    /// Capability is not supported by this backend configuration.
    Unsupported,
}

impl SkiaCapabilitySupport {
    /// Returns whether the capability is supported.
    #[must_use]
    pub const fn is_supported(self) -> bool {
        matches!(self, Self::Supported)
    }
}

impl SkiaRendererCapabilities {
    /// Creates the CPU raster capability report.
    #[must_use]
    pub const fn cpu_raster() -> Self {
        Self {
            cpu_raster: SkiaCapabilitySupport::Supported,
            gpu: SkiaCapabilitySupport::Unsupported,
            paths: SkiaCapabilitySupport::Supported,
            clips: SkiaCapabilitySupport::Supported,
            transforms: SkiaCapabilitySupport::Supported,
            text: SkiaCapabilitySupport::Supported,
            images: SkiaCapabilitySupport::Supported,
            vectors: SkiaCapabilitySupport::Supported,
            effects: SkiaCapabilitySupport::Supported,
            dirty_regions: SkiaCapabilitySupport::Supported,
        }
    }
}

/// Surface creation configuration.
#[derive(Clone, Debug, PartialEq)]
pub struct SkiaSurfaceConfig {
    id: String,
    width: u32,
    height: u32,
    dpi_scale: f32,
    kind: SkiaSurfaceKind,
}

impl SkiaSurfaceConfig {
    /// Creates a CPU raster surface configuration.
    #[must_use]
    pub fn cpu_raster(id: impl Into<String>, width: u32, height: u32) -> Self {
        Self {
            id: id.into(),
            width,
            height,
            dpi_scale: 1.0,
            kind: SkiaSurfaceKind::CpuRaster,
        }
    }

    /// Sets the initial DPI scale.
    #[must_use]
    pub const fn with_dpi_scale(mut self, dpi_scale: f32) -> Self {
        self.dpi_scale = dpi_scale;
        self
    }

    /// Returns the surface identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the logical width.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Returns the logical height.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Returns the DPI scale.
    #[must_use]
    pub const fn dpi_scale(&self) -> f32 {
        self.dpi_scale
    }

    /// Returns the backing surface kind.
    #[must_use]
    pub const fn kind(&self) -> SkiaSurfaceKind {
        self.kind
    }
}

/// Runtime renderer surface state.
pub struct SkiaSurface {
    id: String,
    width: u32,
    height: u32,
    dpi_scale: f32,
    kind: SkiaSurfaceKind,
    raster_surface: Surface,
    frame_active: bool,
    presented_frames: u64,
    dirty_regions: Vec<Geometry>,
}

impl SkiaSurface {
    fn new(config: SkiaSurfaceConfig) -> Result<Self, BackendError> {
        let raster_surface = create_raster_surface(config.width, config.height, config.dpi_scale)?;
        Ok(Self {
            id: config.id,
            width: config.width,
            height: config.height,
            dpi_scale: config.dpi_scale,
            kind: config.kind,
            raster_surface,
            frame_active: false,
            presented_frames: 0,
            dirty_regions: Vec::new(),
        })
    }

    /// Returns the surface identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the logical width.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Returns the logical height.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Returns the DPI scale.
    #[must_use]
    pub const fn dpi_scale(&self) -> f32 {
        self.dpi_scale
    }

    /// Returns the backing surface kind.
    #[must_use]
    pub const fn kind(&self) -> SkiaSurfaceKind {
        self.kind
    }

    /// Returns the current pixel width.
    #[must_use]
    pub fn pixel_width(&self) -> u32 {
        scaled_pixels(self.width, self.dpi_scale)
    }

    /// Returns the current pixel height.
    #[must_use]
    pub fn pixel_height(&self) -> u32 {
        scaled_pixels(self.height, self.dpi_scale)
    }

    /// Returns whether a frame is active.
    #[must_use]
    pub const fn frame_active(&self) -> bool {
        self.frame_active
    }

    /// Returns the number of presented frames.
    #[must_use]
    pub const fn presented_frames(&self) -> u64 {
        self.presented_frames
    }

    /// Returns submitted dirty regions.
    #[must_use]
    pub fn dirty_regions(&self) -> &[Geometry] {
        &self.dirty_regions
    }

    fn resize(&mut self, width: u32, height: u32, dpi_scale: f32) -> Result<(), BackendError> {
        self.width = width;
        self.height = height;
        self.dpi_scale = dpi_scale;
        self.raster_surface = create_raster_surface(width, height, dpi_scale)?;
        Ok(())
    }

    fn canvas(&mut self) -> &Canvas {
        self.raster_surface.canvas()
    }
}

/// Deterministic Skia renderer backend.
pub struct SkiaRendererBackend {
    capabilities: BackendCapabilities,
    surfaces: BTreeMap<String, SkiaSurface>,
    active_surface: Option<String>,
    commands: Vec<String>,
    dirty_regions: Vec<Geometry>,
    diagnostics: Vec<BackendDiagnostic>,
    skia_capabilities: SkiaRendererCapabilities,
    image_assets: BTreeMap<String, Image>,
}

impl SkiaRendererBackend {
    /// Creates a CPU raster Skia renderer backend.
    #[must_use]
    pub fn new() -> Self {
        Self {
            capabilities: BackendCapabilities::new()
                .with_gpu(false)
                .with_text(true)
                .with_images(true),
            surfaces: BTreeMap::new(),
            active_surface: None,
            commands: Vec::new(),
            dirty_regions: Vec::new(),
            diagnostics: Vec::new(),
            skia_capabilities: SkiaRendererCapabilities::cpu_raster(),
            image_assets: BTreeMap::new(),
        }
    }

    /// Creates a configured surface.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the surface ID, size, DPI scale, or duplicate lifecycle is
    /// invalid.
    pub fn create_surface_with_config(
        &mut self,
        config: SkiaSurfaceConfig,
    ) -> Result<(), BackendError> {
        validate_surface_id(config.id())?;
        validate_surface_size(config.width(), config.height())?;
        validate_dpi_scale(config.dpi_scale())?;
        if self.surfaces.contains_key(config.id()) {
            return self.fail("skia.surface.duplicate", "surface already exists");
        }
        let id = config.id().to_string();
        self.commands.push(format!(
            "create-surface:{}:{}x{}",
            config.id(),
            config.width(),
            config.height()
        ));
        self.surfaces.insert(id, SkiaSurface::new(config)?);
        Ok(())
    }

    /// Returns a surface by ID.
    #[must_use]
    pub fn surface(&self, id: &str) -> Option<&SkiaSurface> {
        self.surfaces.get(id)
    }

    /// Returns all recorded command keys.
    #[must_use]
    pub fn command_keys(&self) -> &[String] {
        &self.commands
    }

    /// Returns all submitted dirty regions.
    #[must_use]
    pub fn dirty_regions(&self) -> &[Geometry] {
        &self.dirty_regions
    }

    /// Returns accumulated diagnostics.
    #[must_use]
    pub fn diagnostics(&self) -> &[BackendDiagnostic] {
        &self.diagnostics
    }

    /// Returns detailed Skia-specific capabilities without exposing `skia-safe` types.
    #[must_use]
    pub const fn skia_capabilities(&self) -> SkiaRendererCapabilities {
        self.skia_capabilities
    }

    /// Registers an encoded compiled image payload for later drawing by asset ID.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the image payload cannot be decoded by `Skia`.
    pub fn register_image_asset(
        &mut self,
        id: impl Into<String>,
        encoded_bytes: &[u8],
    ) -> Result<(), BackendError> {
        let id = id.into();
        let data = Data::new_copy(encoded_bytes);
        let Some(image) = Image::from_encoded(data) else {
            return self.fail(
                "skia.image.decode-failed",
                "compiled image payload could not be decoded",
            );
        };
        self.image_assets.insert(id.clone(), image);
        self.commands.push(format!("register-image:{id}"));
        Ok(())
    }

    /// Draws a compiled vector asset by stable asset ID.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when there is no active frame.
    pub fn draw_vector(&mut self, vector: &str) -> Result<(), BackendError> {
        self.require_active_frame()?;
        self.commands.push(format!("vector:{vector}"));
        Ok(())
    }

    fn fail<T>(&mut self, rule: &str, message: &str) -> Result<T, BackendError> {
        let diagnostic = BackendDiagnostic::new(rule, message);
        self.diagnostics.push(diagnostic);
        Err(BackendError::new(rule, message))
    }

    fn require_surface(&mut self, id: &str) -> Result<(), BackendError> {
        if self.surfaces.contains_key(id) {
            Ok(())
        } else {
            self.fail("skia.surface.missing", "surface does not exist")
        }
    }

    fn surface_mut(&mut self, id: &str) -> Result<&mut SkiaSurface, BackendError> {
        if let Some(surface) = self.surfaces.get_mut(id) {
            Ok(surface)
        } else {
            let message = "surface does not exist";
            self.diagnostics
                .push(BackendDiagnostic::new("skia.surface.missing", message));
            Err(BackendError::new("skia.surface.missing", message))
        }
    }

    fn with_active_surface<T>(
        &mut self,
        draw: impl FnOnce(&mut SkiaSurface) -> T,
    ) -> Result<T, BackendError> {
        let active_id = self.require_active_frame()?;
        if let Some(surface) = self.surfaces.get_mut(&active_id) {
            Ok(draw(surface))
        } else {
            let message = "active surface does not exist";
            self.diagnostics
                .push(BackendDiagnostic::new("skia.surface.missing", message));
            Err(BackendError::new("skia.surface.missing", message))
        }
    }

    fn require_active_frame(&mut self) -> Result<String, BackendError> {
        let Some(id) = self.active_surface.clone() else {
            return self.fail("skia.frame.missing", "drawing requires an active frame");
        };
        let Some(surface) = self.surfaces.get(&id) else {
            return self.fail("skia.surface.missing", "active surface does not exist");
        };
        if surface.frame_active {
            Ok(id)
        } else {
            self.fail("skia.frame.missing", "drawing requires an active frame")
        }
    }
}

impl Default for SkiaRendererBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl RendererBackend for SkiaRendererBackend {
    fn create_surface(&mut self, id: &str, width: u32, height: u32) -> Result<(), BackendError> {
        self.create_surface_with_config(SkiaSurfaceConfig::cpu_raster(id, width, height))
    }

    fn teardown_surface(&mut self, id: &str) -> Result<(), BackendError> {
        self.require_surface(id)?;
        if self.active_surface.as_deref() == Some(id) {
            return self.fail(
                "skia.surface.teardown-active",
                "cannot tear down a surface with an active frame",
            );
        }
        self.commands.push(format!("teardown-surface:{id}"));
        self.surfaces.remove(id);
        Ok(())
    }

    fn resize_surface(
        &mut self,
        id: &str,
        width: u32,
        height: u32,
        dpi_scale: f32,
    ) -> Result<(), BackendError> {
        self.require_surface(id)?;
        validate_surface_size(width, height).inspect_err(|error| {
            self.diagnostics.push(error.diagnostic().clone());
        })?;
        validate_dpi_scale(dpi_scale).inspect_err(|error| {
            self.diagnostics.push(error.diagnostic().clone());
        })?;
        let surface = self.surface_mut(id)?;
        surface.resize(width, height, dpi_scale)?;
        self.commands
            .push(format!("resize-surface:{id}:{width}x{height}@{dpi_scale}"));
        Ok(())
    }

    fn begin_frame(&mut self, id: &str) -> Result<(), BackendError> {
        self.require_surface(id)?;
        if self.active_surface.is_some() {
            return self.fail("skia.frame.active", "a frame is already active");
        }
        let surface = self.surface_mut(id)?;
        surface.frame_active = true;
        self.active_surface = Some(id.to_string());
        self.commands.push(format!("begin-frame:{id}"));
        Ok(())
    }

    fn end_frame(&mut self, id: &str) -> Result<(), BackendError> {
        self.require_surface(id)?;
        if self.active_surface.as_deref() != Some(id) {
            return self.fail(
                "skia.frame.mismatch",
                "frame end does not match active surface",
            );
        }
        let surface = self.surface_mut(id)?;
        surface.frame_active = false;
        surface.presented_frames = surface.presented_frames.saturating_add(1);
        self.active_surface = None;
        self.commands.push(format!("end-frame:{id}"));
        Ok(())
    }

    fn clear(&mut self, color: Color) -> Result<(), BackendError> {
        self.with_active_surface(|surface| {
            surface.canvas().clear(to_skia_color(color));
        })?;
        self.commands.push(format!(
            "clear:{},{},{},{}",
            color.r, color.g, color.b, color.a
        ));
        Ok(())
    }

    fn fill(&mut self, geometry: Geometry, color: Color) -> Result<(), BackendError> {
        self.with_active_surface(|surface| {
            let mut paint = paint(color, PaintStyle::Fill);
            paint.set_anti_alias(true);
            surface.canvas().draw_rect(rect(geometry), &paint);
        })?;
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
        self.with_active_surface(|surface| {
            let mut paint = paint(Color::rgba(255, 255, 255, 255), PaintStyle::Stroke);
            paint.set_stroke_width(stroke.width);
            paint.set_anti_alias(true);
            surface.canvas().draw_rect(rect(geometry), &paint);
        })?;
        self.commands.push(format!(
            "stroke:{},{},{},{}:{}",
            geometry.x, geometry.y, geometry.width, geometry.height, stroke.width
        ));
        Ok(())
    }

    fn draw_path(&mut self, path: &str) -> Result<(), BackendError> {
        self.require_active_frame()?;
        let Some(skia_path) = Path::from_svg(path) else {
            return self.fail(
                "skia.path.invalid",
                "path data is not valid SVG path syntax",
            );
        };
        self.with_active_surface(|surface| {
            let mut paint = paint(Color::rgba(255, 255, 255, 255), PaintStyle::Stroke);
            paint.set_anti_alias(true);
            surface.canvas().draw_path(&skia_path, &paint);
        })?;
        self.commands.push(format!("path:{path}"));
        Ok(())
    }

    fn draw_text(&mut self, text: &str) -> Result<(), BackendError> {
        self.require_active_frame()?;
        if !self.capabilities.text {
            return self.fail(
                "backend.capability.text.missing",
                "backend does not support text rendering",
            );
        }
        self.with_active_surface(|surface| {
            let mut paint = paint(Color::rgba(255, 255, 255, 255), PaintStyle::Fill);
            paint.set_anti_alias(true);
            surface
                .canvas()
                .draw_str(text, (0.0, 0.0), &Font::default(), &paint);
        })?;
        self.commands.push(format!("text:{text}"));
        Ok(())
    }

    fn draw_image(&mut self, image: &str) -> Result<(), BackendError> {
        self.require_active_frame()?;
        if !self.capabilities.images {
            return self.fail(
                "backend.capability.image.missing",
                "backend does not support image rendering",
            );
        }
        let Some(asset) = self.image_assets.get(image).cloned() else {
            return self.fail(
                "skia.image.missing",
                "compiled image asset is not registered with the renderer",
            );
        };
        self.with_active_surface(|surface| {
            let mut paint = Paint::default();
            paint.set_anti_alias(true);
            surface.canvas().draw_image(asset, (0.0, 0.0), Some(&paint));
        })?;
        self.commands.push(format!("image:{image}"));
        Ok(())
    }

    fn push_clip(&mut self, geometry: Geometry) -> Result<(), BackendError> {
        self.with_active_surface(|surface| {
            surface
                .canvas()
                .clip_rect(rect(geometry), ClipOp::Intersect, true);
        })?;
        self.commands.push(format!(
            "clip:{},{},{},{}",
            geometry.x, geometry.y, geometry.width, geometry.height
        ));
        Ok(())
    }

    fn push_transform(&mut self, transform: Transform) -> Result<(), BackendError> {
        self.with_active_surface(|surface| {
            surface
                .canvas()
                .translate((transform.translate_x, transform.translate_y));
        })?;
        self.commands.push(format!(
            "transform:{},{}",
            transform.translate_x, transform.translate_y
        ));
        Ok(())
    }

    fn apply_layer_effect(&mut self, effect: &str) -> Result<(), BackendError> {
        self.require_active_frame()?;
        self.commands.push(format!("effect:{effect}"));
        Ok(())
    }

    fn create_cache_handle(&mut self, id: &str) -> Result<BackendCacheHandle, BackendError> {
        self.require_active_frame()?;
        self.commands.push(format!("cache:{id}"));
        Ok(BackendCacheHandle::new(id))
    }

    fn mark_dirty(&mut self, geometry: Geometry) -> Result<(), BackendError> {
        self.with_active_surface(|surface| {
            surface.dirty_regions.push(geometry);
        })?;
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

fn validate_surface_id(id: &str) -> Result<(), BackendError> {
    if id.trim().is_empty() {
        Err(BackendError::new(
            "skia.surface.invalid-id",
            "surface ID must not be empty",
        ))
    } else {
        Ok(())
    }
}

fn validate_surface_size(width: u32, height: u32) -> Result<(), BackendError> {
    if width == 0 || height == 0 {
        Err(BackendError::new(
            "skia.surface.invalid-size",
            "surface dimensions must be greater than zero",
        ))
    } else {
        Ok(())
    }
}

fn validate_dpi_scale(dpi_scale: f32) -> Result<(), BackendError> {
    if dpi_scale.is_finite() && dpi_scale > 0.0 {
        Ok(())
    } else {
        Err(BackendError::new(
            "skia.surface.invalid-dpi",
            "DPI scale must be finite and greater than zero",
        ))
    }
}

fn create_raster_surface(width: u32, height: u32, dpi_scale: f32) -> Result<Surface, BackendError> {
    let pixel_width = i32::try_from(scaled_pixels(width, dpi_scale)).map_err(|_| {
        BackendError::new(
            "skia.surface.pixel-size-overflow",
            "surface pixel width exceeds Skia raster limits",
        )
    })?;
    let pixel_height = i32::try_from(scaled_pixels(height, dpi_scale)).map_err(|_| {
        BackendError::new(
            "skia.surface.pixel-size-overflow",
            "surface pixel height exceeds Skia raster limits",
        )
    })?;
    surfaces::raster_n32_premul((pixel_width, pixel_height)).ok_or_else(|| {
        BackendError::new(
            "skia.surface.allocation-failed",
            "failed to allocate Skia CPU raster surface",
        )
    })
}

fn paint(color: Color, style: PaintStyle) -> Paint {
    let mut paint = Paint::default();
    paint.set_style(style);
    paint.set_color(to_skia_color(color));
    paint
}

fn rect(geometry: Geometry) -> Rect {
    Rect::from_xywh(geometry.x, geometry.y, geometry.width, geometry.height)
}

fn to_skia_color(color: Color) -> SkiaColor {
    SkiaColor::from_argb(color.a, color.r, color.g, color.b)
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn scaled_pixels(logical: u32, dpi_scale: f32) -> u32 {
    ((logical as f32) * dpi_scale)
        .round()
        .clamp(1.0, u32::MAX as f32) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_identity() {
        assert_eq!(crate_name(), "hawk2ui-render-skia");
    }
}

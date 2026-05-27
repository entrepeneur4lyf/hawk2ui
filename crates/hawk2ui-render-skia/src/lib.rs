#![forbid(unsafe_code)]
//! `Skia`-backed production renderer backend for `Hawk2UI`.

use std::collections::BTreeMap;

use hawk2ui_assets::{AssetKind as CompiledAssetKind, AssetRecord};
use hawk2ui_render::{
    BackendCacheHandle, BackendCapabilities, BackendDiagnostic, BackendError, Color,
    CustomSurfaceCategory, CustomSurfaceDrawRequest, Geometry, RendererBackend,
    RendererCacheInvalidator, Stroke, Transform,
};
use skia_safe::{
    AlphaType, BlurStyle, Canvas, ClipOp, Color as SkiaColor, Color4f, ColorType, Data, Font,
    FontMgr, FontStyle, IRect, Image, ImageInfo, MaskFilter, Matrix, Paint, PaintStyle, Path,
    PathBuilder, Rect, Surface, TileMode, Typeface, gradient, images, surfaces,
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
    last_presented_frame: Option<SkiaFrameSnapshot>,
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
            last_presented_frame: None,
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

    /// Returns the last fully-presented frame snapshot.
    #[must_use]
    pub const fn last_presented_frame(&self) -> Option<&SkiaFrameSnapshot> {
        self.last_presented_frame.as_ref()
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

/// CPU-readable snapshot of a fully-presented Skia frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkiaFrameSnapshot {
    width: u32,
    height: u32,
    pixels: Vec<u32>,
}

/// Cached Skia raster layer entry.
pub struct SkiaLayerCacheEntry {
    id: String,
    source: Geometry,
    width: u32,
    height: u32,
    generation: u64,
    valid: bool,
    image: Image,
}

impl SkiaLayerCacheEntry {
    fn new(
        id: impl Into<String>,
        source: Geometry,
        width: u32,
        height: u32,
        generation: u64,
        image: Image,
    ) -> Self {
        Self {
            id: id.into(),
            source,
            width,
            height,
            generation,
            valid: true,
            image,
        }
    }

    /// Returns the cache identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns cached layer width in pixels.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Returns cached layer height in pixels.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Returns cache generation.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns whether the cache entry is valid for replay.
    #[must_use]
    pub const fn valid(&self) -> bool {
        self.valid
    }

    fn invalidate(&mut self) {
        if self.valid {
            self.valid = false;
            self.generation = self.generation.saturating_add(1);
        }
    }
}

#[derive(Clone, Debug)]
struct SkiaVectorPathRecord {
    path: Path,
    fill: Color,
}

#[derive(Clone, Debug)]
struct SkiaVectorAsset {
    paths: Vec<SkiaVectorPathRecord>,
}

/// Default text placement and style used by the trait-level text draw call.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkiaTextStyle {
    /// Logical x coordinate for the text origin.
    pub x: f32,
    /// Logical y coordinate for the text baseline.
    pub baseline_y: f32,
    /// Font size in logical pixels.
    pub font_size: f32,
    /// Text fill color.
    pub color: Color,
}

impl SkiaTextStyle {
    /// Creates a default text style for trait-level drawing.
    #[must_use]
    pub const fn new(x: f32, baseline_y: f32, font_size: f32, color: Color) -> Self {
        Self {
            x,
            baseline_y,
            font_size,
            color,
        }
    }
}

impl SkiaFrameSnapshot {
    fn new(width: u32, height: u32, pixels: Vec<u32>) -> Result<Self, BackendError> {
        let expected = usize::try_from(u64::from(width) * u64::from(height)).map_err(|_| {
            BackendError::new(
                "skia.snapshot.pixel-count-overflow",
                "frame snapshot pixel count exceeds addressable memory",
            )
        })?;
        if pixels.len() != expected {
            return Err(BackendError::new(
                "skia.snapshot.pixel-count-mismatch",
                "frame snapshot pixel count does not match dimensions",
            ));
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    /// Returns snapshot width in physical pixels.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Returns snapshot height in physical pixels.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Returns pixels in `0x00RRGGBB` order.
    #[must_use]
    pub fn pixels(&self) -> &[u32] {
        &self.pixels
    }

    /// Returns a pixel by physical coordinate.
    #[must_use]
    pub fn pixel_at(&self, x: u32, y: u32) -> Option<u32> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = usize::try_from(u64::from(y) * u64::from(self.width) + u64::from(x)).ok()?;
        self.pixels.get(index).copied()
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
    vector_assets: BTreeMap<String, SkiaVectorAsset>,
    layer_caches: BTreeMap<String, SkiaLayerCacheEntry>,
    cache_invalidation_keys: Vec<String>,
    opacity_group_depth: usize,
    default_typeface: Option<Typeface>,
    default_text_style: SkiaTextStyle,
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
            vector_assets: BTreeMap::new(),
            layer_caches: BTreeMap::new(),
            cache_invalidation_keys: Vec::new(),
            opacity_group_depth: 0,
            default_typeface: FontMgr::new().legacy_make_typeface(None, FontStyle::normal()),
            default_text_style: SkiaTextStyle::new(
                0.0,
                18.0,
                16.0,
                Color::rgba(255, 255, 255, 255),
            ),
        }
    }

    /// Begins an opacity compositing group.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when there is no active frame or opacity is invalid.
    pub fn begin_opacity_group(&mut self, opacity: f32) -> Result<(), BackendError> {
        <Self as RendererBackend>::begin_opacity_group(self, opacity)
    }

    /// Ends the current opacity compositing group.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when there is no active frame or no opacity group is active.
    pub fn end_opacity_group(&mut self) -> Result<(), BackendError> {
        <Self as RendererBackend>::end_opacity_group(self)
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

    /// Returns a layer cache entry by ID.
    #[must_use]
    pub fn layer_cache(&self, id: &str) -> Option<&SkiaLayerCacheEntry> {
        self.layer_caches.get(id)
    }

    /// Returns cache IDs invalidated explicitly through the cache invalidator extension.
    #[must_use]
    pub fn cache_invalidation_keys(&self) -> &[String] {
        &self.cache_invalidation_keys
    }

    /// Returns detailed Skia-specific capabilities without exposing `skia-safe` types.
    #[must_use]
    pub const fn skia_capabilities(&self) -> SkiaRendererCapabilities {
        self.skia_capabilities
    }

    /// Returns the last fully-presented frame snapshot for a surface.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the surface is missing, a frame is still active, or no frame
    /// has been presented yet.
    pub fn frame_snapshot(&mut self, id: &str) -> Result<&SkiaFrameSnapshot, BackendError> {
        let surface = self.surface_mut(id)?;
        if surface.frame_active {
            return Err(BackendError::new(
                "skia.frame.active",
                "cannot read a presented snapshot while a frame is active",
            ));
        }
        surface.last_presented_frame.as_ref().ok_or_else(|| {
            BackendError::new(
                "skia.frame.not-presented",
                "surface does not have a presented frame snapshot",
            )
        })
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
        let Some(image) = Image::from_encoded(data).or_else(|| decode_raster_image(encoded_bytes))
        else {
            return self.fail(
                "skia.image.decode-failed",
                "compiled image payload could not be decoded",
            );
        };
        self.image_assets.insert(id.clone(), image);
        self.commands.push(format!("register-image:{id}"));
        Ok(())
    }

    /// Registers a compiled asset payload from the production asset backend.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the asset kind cannot be rendered or its compiled payload is
    /// not accepted by Skia.
    pub fn register_compiled_asset(&mut self, asset: &AssetRecord) -> Result<(), BackendError> {
        match asset.kind() {
            CompiledAssetKind::Image => {
                self.register_image_asset(asset.id(), asset.compiled_bytes())
            }
            CompiledAssetKind::Vector => {
                let svg = std::str::from_utf8(asset.compiled_bytes()).map_err(|_| {
                    BackendError::new(
                        "skia.vector.invalid-utf8",
                        "compiled vector payload must be UTF-8 SVG",
                    )
                })?;
                let paths = extract_svg_path_data(svg);
                self.register_vector_paths(asset.id(), paths)
            }
            CompiledAssetKind::Font => self.fail(
                "skia.asset.unsupported-kind",
                "font assets are registered through the text/font stack, not drawn directly",
            ),
        }
    }

    /// Registers a compiled vector asset made of filled SVG path records.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the asset ID is empty, no paths are provided, or any path data
    /// is invalid SVG path syntax.
    pub fn register_vector_asset<P, I>(
        &mut self,
        id: impl Into<String>,
        records: I,
    ) -> Result<(), BackendError>
    where
        P: AsRef<str>,
        I: IntoIterator<Item = (P, Color)>,
    {
        let id = id.into();
        validate_asset_id(&id).inspect_err(|error| {
            self.diagnostics.push(error.diagnostic().clone());
        })?;
        let mut paths = Vec::new();
        for (path, fill) in records {
            let Some(path) = Path::from_svg(path.as_ref()) else {
                return self.fail(
                    "skia.vector.invalid-path",
                    "compiled vector path data is not valid SVG path syntax",
                );
            };
            paths.push(SkiaVectorPathRecord { path, fill });
        }
        if paths.is_empty() {
            return self.fail(
                "skia.vector.empty",
                "compiled vector asset must contain at least one path",
            );
        }
        self.vector_assets
            .insert(id.clone(), SkiaVectorAsset { paths });
        self.commands.push(format!("register-vector:{id}"));
        Ok(())
    }

    /// Registers a compiled vector asset from path data using an opaque white fill.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when registration fails.
    pub fn register_vector_paths<P, I>(
        &mut self,
        id: impl Into<String>,
        paths: I,
    ) -> Result<(), BackendError>
    where
        P: AsRef<str>,
        I: IntoIterator<Item = P>,
    {
        self.register_vector_asset(
            id,
            paths
                .into_iter()
                .map(|path| (path, Color::rgba(255, 255, 255, 255))),
        )
    }

    /// Sets default text placement and style for [`RendererBackend::draw_text`].
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when coordinates or font size cannot produce a valid text draw.
    pub fn set_default_text_style(
        &mut self,
        x: f32,
        baseline_y: f32,
        font_size: f32,
        color: Color,
    ) -> Result<(), BackendError> {
        validate_text_placement(x, baseline_y, font_size).inspect_err(|error| {
            self.diagnostics.push(error.diagnostic().clone());
        })?;
        self.default_text_style = SkiaTextStyle::new(x, baseline_y, font_size, color);
        Ok(())
    }

    /// Returns default text placement and style for [`RendererBackend::draw_text`].
    #[must_use]
    pub const fn default_text_style(&self) -> SkiaTextStyle {
        self.default_text_style
    }

    /// Draws text at a concrete baseline position.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when there is no active frame or the font size is invalid.
    pub fn draw_text_at(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        font_size: f32,
        color: Color,
    ) -> Result<(), BackendError> {
        self.render_text_at(text, x, y, font_size, color)?;
        self.commands
            .push(format!("text-at:{text}:{x},{y}:{font_size}"));
        Ok(())
    }

    /// Draws a registered image asset into a destination rectangle.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when there is no active frame, the image is missing, or the
    /// destination geometry is invalid.
    pub fn draw_image_rect(&mut self, image: &str, geometry: Geometry) -> Result<(), BackendError> {
        self.require_active_frame()?;
        validate_geometry("skia.image.invalid-geometry", geometry).inspect_err(|error| {
            self.diagnostics.push(error.diagnostic().clone());
        })?;
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
            let dst = rect(geometry);
            surface.canvas().draw_image_rect(asset, None, dst, &paint);
        })?;
        self.commands.push(format!(
            "image-rect:{image}:{},{},{},{}",
            geometry.x, geometry.y, geometry.width, geometry.height
        ));
        Ok(())
    }

    /// Draws a filled SVG path string.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when there is no active frame or path syntax is invalid.
    pub fn draw_filled_path(&mut self, path: &str, color: Color) -> Result<(), BackendError> {
        self.require_active_frame()?;
        let Some(skia_path) = Path::from_svg(path) else {
            return self.fail(
                "skia.path.invalid",
                "path data is not valid SVG path syntax",
            );
        };
        self.with_active_surface(|surface| {
            let mut paint = paint(color, PaintStyle::Fill);
            paint.set_anti_alias(true);
            surface.canvas().draw_path(&skia_path, &paint);
        })?;
        self.commands.push(format!("filled-path:{path}"));
        Ok(())
    }

    /// Draws a filled rounded rectangle.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when there is no active frame or geometry/radius is invalid.
    pub fn draw_rounded_rect(
        &mut self,
        geometry: Geometry,
        radius: f32,
        color: Color,
    ) -> Result<(), BackendError> {
        self.require_active_frame()?;
        validate_geometry("skia.rounded-rect.invalid-geometry", geometry).inspect_err(|error| {
            self.diagnostics.push(error.diagnostic().clone());
        })?;
        if !radius.is_finite() || radius < 0.0 {
            return self.fail(
                "skia.rounded-rect.invalid-radius",
                "rounded rectangle radius must be finite and non-negative",
            );
        }
        self.with_active_surface(|surface| {
            let mut paint = paint(color, PaintStyle::Fill);
            paint.set_anti_alias(true);
            surface
                .canvas()
                .draw_round_rect(rect(geometry), radius, radius, &paint);
        })?;
        self.commands.push(format!(
            "rounded-rect:{},{},{},{}:{radius}",
            geometry.x, geometry.y, geometry.width, geometry.height
        ));
        Ok(())
    }

    /// Draws a left-to-right linear gradient inside a rectangle.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when there is no active frame, geometry is invalid, or Skia cannot
    /// create the gradient shader.
    pub fn draw_linear_gradient(
        &mut self,
        geometry: Geometry,
        start: Color,
        end: Color,
    ) -> Result<(), BackendError> {
        self.require_active_frame()?;
        validate_geometry("skia.gradient.invalid-geometry", geometry).inspect_err(|error| {
            self.diagnostics.push(error.diagnostic().clone());
        })?;
        self.with_active_surface(|surface| {
            let colors = [to_skia_color4f(start), to_skia_color4f(end)];
            let gradient_colors =
                gradient::Colors::new_evenly_spaced(&colors, TileMode::Clamp, None);
            let shader_gradient =
                gradient::Gradient::new(gradient_colors, gradient::Interpolation::default());
            let shader = gradient::shaders::linear_gradient(
                (
                    (geometry.x, geometry.y),
                    (geometry.x + geometry.width, geometry.y),
                ),
                &shader_gradient,
                None::<&skia_safe::Matrix>,
            );
            let mut paint = Paint::default();
            paint.set_anti_alias(true);
            paint.set_shader(shader);
            surface.canvas().draw_rect(rect(geometry), &paint);
        })?;
        self.commands.push(format!(
            "linear-gradient:{},{},{},{}",
            geometry.x, geometry.y, geometry.width, geometry.height
        ));
        Ok(())
    }

    /// Draws a blurred shadow rectangle offset from the source rectangle.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when there is no active frame or geometry/effect parameters are
    /// invalid.
    pub fn draw_shadow_rect(
        &mut self,
        geometry: Geometry,
        offset_x: f32,
        offset_y: f32,
        blur_radius: f32,
        color: Color,
    ) -> Result<(), BackendError> {
        self.require_active_frame()?;
        validate_geometry("skia.shadow.invalid-geometry", geometry).inspect_err(|error| {
            self.diagnostics.push(error.diagnostic().clone());
        })?;
        validate_blur("skia.shadow.invalid-blur", blur_radius)?;
        if !offset_x.is_finite() || !offset_y.is_finite() {
            return self.fail(
                "skia.shadow.invalid-offset",
                "shadow offsets must be finite",
            );
        }
        let shadow = Geometry::new(
            geometry.x + offset_x,
            geometry.y + offset_y,
            geometry.width,
            geometry.height,
        );
        self.with_active_surface(|surface| {
            let mut paint = paint(color, PaintStyle::Fill);
            paint.set_anti_alias(true);
            paint.set_mask_filter(MaskFilter::blur(
                BlurStyle::Normal,
                blur_radius,
                Some(false),
            ));
            surface.canvas().draw_rect(rect(shadow), &paint);
        })?;
        self.commands.push(format!(
            "shadow-rect:{},{},{},{}:{offset_x},{offset_y}:{blur_radius}",
            geometry.x, geometry.y, geometry.width, geometry.height
        ));
        Ok(())
    }

    /// Draws a blurred glow around a rectangle.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when there is no active frame or geometry/effect parameters are
    /// invalid.
    pub fn draw_glow_rect(
        &mut self,
        geometry: Geometry,
        blur_radius: f32,
        color: Color,
    ) -> Result<(), BackendError> {
        self.require_active_frame()?;
        validate_geometry("skia.glow.invalid-geometry", geometry).inspect_err(|error| {
            self.diagnostics.push(error.diagnostic().clone());
        })?;
        validate_blur("skia.glow.invalid-blur", blur_radius)?;
        self.with_active_surface(|surface| {
            let mut paint = paint(color, PaintStyle::Fill);
            paint.set_anti_alias(true);
            paint.set_mask_filter(MaskFilter::blur(
                BlurStyle::Normal,
                blur_radius,
                Some(false),
            ));
            surface.canvas().draw_rect(rect(geometry), &paint);
        })?;
        self.commands.push(format!(
            "glow-rect:{},{},{},{}:{blur_radius}",
            geometry.x, geometry.y, geometry.width, geometry.height
        ));
        Ok(())
    }

    /// Captures a region of the active frame into a reusable Skia layer cache.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when there is no active frame, the geometry is invalid, or the
    /// surface cannot create a snapshot for the requested region.
    pub fn cache_current_frame_region(
        &mut self,
        id: &str,
        geometry: Geometry,
    ) -> Result<BackendCacheHandle, BackendError> {
        validate_surface_id(id).inspect_err(|error| {
            self.diagnostics.push(error.diagnostic().clone());
        })?;
        let active_id = self.require_active_frame()?;
        validate_geometry("skia.cache.invalid-geometry", geometry).inspect_err(|error| {
            self.diagnostics.push(error.diagnostic().clone());
        })?;
        let bounds = geometry_to_irect(geometry).inspect_err(|error| {
            self.diagnostics.push(error.diagnostic().clone());
        })?;
        let Some(surface) = self.surfaces.get_mut(&active_id) else {
            return self.fail("skia.surface.missing", "active surface does not exist");
        };
        let Some(image) = surface.raster_surface.image_snapshot_with_bounds(bounds) else {
            return self.fail(
                "skia.cache.snapshot-failed",
                "failed to capture Skia cache region",
            );
        };
        let generation = self
            .layer_caches
            .get(id)
            .map_or(1, |entry| entry.generation.saturating_add(1));
        let width = u32::try_from(bounds.width()).map_err(|_| {
            BackendError::new(
                "skia.cache.size-overflow",
                "cached layer width exceeds supported size",
            )
        })?;
        let height = u32::try_from(bounds.height()).map_err(|_| {
            BackendError::new(
                "skia.cache.size-overflow",
                "cached layer height exceeds supported size",
            )
        })?;
        self.layer_caches.insert(
            id.to_string(),
            SkiaLayerCacheEntry::new(id, geometry, width, height, generation, image),
        );
        self.commands.push(format!(
            "cache-region:{id}:{},{},{},{}:gen={generation}",
            geometry.x, geometry.y, geometry.width, geometry.height
        ));
        Ok(BackendCacheHandle::new(id))
    }

    /// Replays a valid cached layer into the active frame.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when there is no active frame, the cache is missing/invalid, or the
    /// destination geometry is invalid.
    pub fn draw_cached_layer(
        &mut self,
        id: &str,
        destination: Geometry,
    ) -> Result<(), BackendError> {
        self.require_active_frame()?;
        validate_geometry("skia.cache.invalid-destination", destination).inspect_err(|error| {
            self.diagnostics.push(error.diagnostic().clone());
        })?;
        let Some(cache) = self.layer_caches.get(id) else {
            return self.fail("skia.cache.missing", "layer cache entry does not exist");
        };
        if !cache.valid {
            return self.fail(
                "skia.cache.invalid",
                "layer cache entry has been invalidated",
            );
        }
        let image = cache.image.clone();
        self.with_active_surface(|surface| {
            let mut paint = Paint::default();
            paint.set_anti_alias(true);
            let dst = rect(destination);
            surface.canvas().draw_image_rect(image, None, dst, &paint);
        })?;
        self.commands.push(format!(
            "draw-cache:{id}:{},{},{},{}",
            destination.x, destination.y, destination.width, destination.height
        ));
        Ok(())
    }

    /// Invalidates a cached layer entry by ID.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the cache entry is missing.
    pub fn invalidate_cache(&mut self, id: &str) -> Result<(), BackendError> {
        let Some(cache) = self.layer_caches.get_mut(id) else {
            return self.fail("skia.cache.missing", "layer cache entry does not exist");
        };
        cache.invalidate();
        self.cache_invalidation_keys.push(id.to_string());
        self.commands.push(format!("invalidate-cache:{id}"));
        Ok(())
    }

    /// Draws a compiled vector asset by stable asset ID.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when there is no active frame.
    pub fn draw_vector(&mut self, vector: &str) -> Result<(), BackendError> {
        self.require_active_frame()?;
        let Some(asset) = self.vector_assets.get(vector).cloned() else {
            return self.fail(
                "skia.vector.missing",
                "compiled vector asset is not registered with the renderer",
            );
        };
        self.with_active_surface(|surface| {
            for record in &asset.paths {
                let mut paint = paint(record.fill, PaintStyle::Fill);
                paint.set_anti_alias(true);
                surface.canvas().draw_path(&record.path, &paint);
            }
        })?;
        self.commands.push(format!("vector:{vector}"));
        Ok(())
    }

    /// Executes a custom draw surface hook into the active Skia frame.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when there is no active frame, the surface request is invalid, or
    /// the destination geometry cannot be drawn safely.
    pub fn draw_custom_surface(
        &mut self,
        request: &CustomSurfaceDrawRequest,
    ) -> Result<(), BackendError> {
        self.require_active_frame()?;
        let surface = request.surface();
        surface.validate().map_err(|error| {
            BackendError::new(error.rule().to_string(), error.message().to_string())
        })?;
        let geometry = surface.reserved_layout();
        validate_geometry("skia.custom-surface.invalid-geometry", geometry).inspect_err(
            |error| {
                self.diagnostics.push(error.diagnostic().clone());
            },
        )?;
        if !surface.is_frame_due(request.context().frame_index()) {
            self.commands.push(format!(
                "custom-surface-skip:{}:{}:frame={}",
                surface.id(),
                surface.category().stable_key(),
                request.context().frame_index()
            ));
            return Ok(());
        }
        if surface.invalidated() {
            self.mark_dirty(geometry)?;
        }

        self.with_active_surface(|surface_frame| {
            draw_custom_surface(
                surface_frame.canvas(),
                surface.category(),
                geometry,
                request.data().samples(),
            );
        })?;
        self.commands.push(format!(
            "custom-surface:{}:{}:frame={}:samples={}",
            surface.id(),
            surface.category().stable_key(),
            request.context().frame_index(),
            request.data().samples().len()
        ));
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

    fn render_text_at(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        font_size: f32,
        color: Color,
    ) -> Result<(), BackendError> {
        self.require_active_frame()?;
        if !self.capabilities.text {
            return self.fail(
                "backend.capability.text.missing",
                "backend does not support text rendering",
            );
        }
        validate_text_placement(x, y, font_size).inspect_err(|error| {
            self.diagnostics.push(error.diagnostic().clone());
        })?;
        let Some(typeface) = self.default_typeface.clone() else {
            return self.fail(
                "skia.text.typeface-unavailable",
                "system font manager did not provide a default typeface",
            );
        };
        let font = Font::new(typeface, font_size);
        self.with_active_surface(|surface| {
            let mut paint = paint(color, PaintStyle::Fill);
            paint.set_anti_alias(true);
            surface.canvas().draw_str(text, (x, y), &font, &paint);
        })?;
        Ok(())
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
        let snapshot = match capture_frame_snapshot(surface) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.diagnostics.push(error.diagnostic().clone());
                return Err(error);
            }
        };
        surface.frame_active = false;
        surface.presented_frames = surface.presented_frames.saturating_add(1);
        surface.last_presented_frame = Some(snapshot);
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
        validate_geometry("skia.geometry.invalid", geometry).inspect_err(|error| {
            self.diagnostics.push(error.diagnostic().clone());
        })?;
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
        validate_geometry("skia.geometry.invalid", geometry).inspect_err(|error| {
            self.diagnostics.push(error.diagnostic().clone());
        })?;
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
        let style = self.default_text_style;
        self.render_text_at(
            text,
            style.x,
            style.baseline_y,
            style.font_size,
            style.color,
        )?;
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
        validate_geometry("skia.geometry.invalid", geometry).inspect_err(|error| {
            self.diagnostics.push(error.diagnostic().clone());
        })?;
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
        validate_transform(transform).inspect_err(|error| {
            self.diagnostics.push(error.diagnostic().clone());
        })?;
        self.with_active_surface(|surface| {
            surface.canvas().concat(&Matrix::new_all(
                transform.scale_x,
                transform.skew_x,
                transform.translate_x,
                transform.skew_y,
                transform.scale_y,
                transform.translate_y,
                0.0,
                0.0,
                1.0,
            ));
        })?;
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
        match parse_layer_effect(effect).inspect_err(|error| {
            self.diagnostics.push(error.diagnostic().clone());
        })? {
            ParsedLayerEffect::ShadowRect {
                geometry,
                offset_x,
                offset_y,
                blur_radius,
                color,
            } => {
                self.draw_shadow_rect(geometry, offset_x, offset_y, blur_radius, color)?;
                self.commands.pop();
                self.commands.push(format!("effect:{effect}"));
                Ok(())
            }
            ParsedLayerEffect::GlowRect {
                geometry,
                blur_radius,
                color,
            } => {
                self.draw_glow_rect(geometry, blur_radius, color)?;
                self.commands.pop();
                self.commands.push(format!("effect:{effect}"));
                Ok(())
            }
        }
    }

    fn begin_opacity_group(&mut self, opacity: f32) -> Result<(), BackendError> {
        self.require_active_frame()?;
        validate_opacity(opacity).inspect_err(|error| {
            self.diagnostics.push(error.diagnostic().clone());
        })?;
        self.with_active_surface(|surface| {
            surface.canvas().save_layer_alpha_f(None, opacity);
        })?;
        self.opacity_group_depth = self.opacity_group_depth.saturating_add(1);
        self.commands.push(format!("begin-opacity-group:{opacity}"));
        Ok(())
    }

    fn end_opacity_group(&mut self) -> Result<(), BackendError> {
        self.require_active_frame()?;
        if self.opacity_group_depth == 0 {
            return self.fail(
                "skia.opacity-group.unbalanced",
                "cannot end opacity group because none is active",
            );
        }
        self.with_active_surface(|surface| {
            surface.canvas().restore();
        })?;
        self.opacity_group_depth -= 1;
        self.commands.push("end-opacity-group".to_string());
        Ok(())
    }

    fn create_cache_handle(&mut self, id: &str) -> Result<BackendCacheHandle, BackendError> {
        self.require_active_frame()?;
        self.commands.push(format!("cache:{id}"));
        Ok(BackendCacheHandle::new(id))
    }

    fn mark_dirty(&mut self, geometry: Geometry) -> Result<(), BackendError> {
        validate_geometry("skia.geometry.invalid", geometry).inspect_err(|error| {
            self.diagnostics.push(error.diagnostic().clone());
        })?;
        self.with_active_surface(|surface| {
            surface.dirty_regions.push(geometry);
        })?;
        self.invalidate_caches_intersecting(geometry);
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

impl SkiaRendererBackend {
    fn invalidate_caches_intersecting(&mut self, geometry: Geometry) {
        for cache in self.layer_caches.values_mut() {
            if cache.valid && geometry_intersects(cache.source, geometry) {
                cache.invalidate();
            }
        }
    }
}

impl RendererCacheInvalidator for SkiaRendererBackend {
    fn invalidate_backend_cache(&mut self, id: &str) -> Result<(), BackendError> {
        self.invalidate_cache(id)
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

fn validate_asset_id(id: &str) -> Result<(), BackendError> {
    if id.trim().is_empty() {
        Err(BackendError::new(
            "skia.asset.invalid-id",
            "asset ID must not be empty",
        ))
    } else {
        Ok(())
    }
}

fn extract_svg_path_data(svg: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for segment in svg.split("<path").skip(1) {
        if let Some(value) = extract_svg_attribute(segment, "d") {
            paths.push(value);
        }
    }
    paths
}

fn extract_svg_attribute(segment: &str, attribute: &str) -> Option<String> {
    let prefix = format!("{attribute}=");
    let start = segment.find(&prefix)? + prefix.len();
    let quote = segment[start..].chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let value_start = start + quote.len_utf8();
    let value_end = segment[value_start..].find(quote)? + value_start;
    Some(segment[value_start..value_end].to_string())
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

fn validate_geometry(rule: &'static str, geometry: Geometry) -> Result<(), BackendError> {
    if geometry.x.is_finite()
        && geometry.y.is_finite()
        && geometry.width.is_finite()
        && geometry.height.is_finite()
        && geometry.width > 0.0
        && geometry.height > 0.0
    {
        Ok(())
    } else {
        Err(BackendError::new(
            rule,
            "geometry coordinates and dimensions must be finite and dimensions must be greater than zero",
        ))
    }
}

fn validate_text_placement(x: f32, y: f32, font_size: f32) -> Result<(), BackendError> {
    if x.is_finite() && y.is_finite() && font_size.is_finite() && font_size > 0.0 {
        Ok(())
    } else {
        Err(BackendError::new(
            "skia.text.invalid-placement",
            "text position and font size must be finite and font size must be greater than zero",
        ))
    }
}

fn validate_blur(rule: &'static str, blur_radius: f32) -> Result<(), BackendError> {
    if blur_radius.is_finite() && blur_radius > 0.0 {
        Ok(())
    } else {
        Err(BackendError::new(
            rule,
            "blur radius must be finite and greater than zero",
        ))
    }
}

fn validate_opacity(opacity: f32) -> Result<(), BackendError> {
    if opacity.is_finite() && (0.0..=1.0).contains(&opacity) {
        Ok(())
    } else {
        Err(BackendError::new(
            "skia.opacity-group.invalid",
            "opacity group alpha must be finite and within 0.0..=1.0",
        ))
    }
}

enum ParsedLayerEffect {
    ShadowRect {
        geometry: Geometry,
        offset_x: f32,
        offset_y: f32,
        blur_radius: f32,
        color: Color,
    },
    GlowRect {
        geometry: Geometry,
        blur_radius: f32,
        color: Color,
    },
}

fn parse_layer_effect(effect: &str) -> Result<ParsedLayerEffect, BackendError> {
    let fields: Vec<_> = effect.split(':').collect();
    match fields.as_slice() {
        ["shadow-rect", geometry, offset, blur_radius, color] => {
            let geometry = parse_geometry(geometry)?;
            let (offset_x, offset_y) = parse_pair(offset)?;
            let blur_radius = parse_f32(blur_radius)?;
            let color = parse_color(color)?;
            Ok(ParsedLayerEffect::ShadowRect {
                geometry,
                offset_x,
                offset_y,
                blur_radius,
                color,
            })
        }
        ["glow-rect", geometry, blur_radius, color] => {
            let geometry = parse_geometry(geometry)?;
            let blur_radius = parse_f32(blur_radius)?;
            let color = parse_color(color)?;
            Ok(ParsedLayerEffect::GlowRect {
                geometry,
                blur_radius,
                color,
            })
        }
        _ => Err(BackendError::new(
            "skia.effect.unsupported",
            "effect must be shadow-rect:x,y,w,h:dx,dy:blur:r,g,b,a or glow-rect:x,y,w,h:blur:r,g,b,a",
        )),
    }
}

fn parse_geometry(value: &str) -> Result<Geometry, BackendError> {
    let values = parse_f32_list(value, 4)?;
    Ok(Geometry::new(values[0], values[1], values[2], values[3]))
}

fn parse_pair(value: &str) -> Result<(f32, f32), BackendError> {
    let values = parse_f32_list(value, 2)?;
    Ok((values[0], values[1]))
}

fn parse_color(value: &str) -> Result<Color, BackendError> {
    let values: Vec<_> = value.split(',').map(parse_u8).collect::<Result<_, _>>()?;
    if values.len() == 4 {
        Ok(Color::rgba(values[0], values[1], values[2], values[3]))
    } else {
        Err(BackendError::new(
            "skia.effect.invalid-color",
            "effect color must have four u8 channels",
        ))
    }
}

fn parse_f32_list(value: &str, expected_len: usize) -> Result<Vec<f32>, BackendError> {
    let values: Vec<_> = value.split(',').map(parse_f32).collect::<Result<_, _>>()?;
    if values.len() == expected_len {
        Ok(values)
    } else {
        Err(BackendError::new(
            "skia.effect.invalid-number-list",
            "effect numeric list has the wrong number of entries",
        ))
    }
}

fn parse_f32(value: &str) -> Result<f32, BackendError> {
    value.parse::<f32>().map_err(|_| {
        BackendError::new(
            "skia.effect.invalid-number",
            "effect numeric values must be valid f32 values",
        )
    })
}

fn parse_u8(value: &str) -> Result<u8, BackendError> {
    value.parse::<u8>().map_err(|_| {
        BackendError::new(
            "skia.effect.invalid-color",
            "effect color channels must be valid u8 values",
        )
    })
}

fn validate_transform(transform: Transform) -> Result<(), BackendError> {
    if transform.is_finite() {
        Ok(())
    } else {
        Err(BackendError::new(
            "skia.transform.invalid",
            "transform coordinates must be finite",
        ))
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn geometry_to_irect(geometry: Geometry) -> Result<IRect, BackendError> {
    let left = geometry.x.floor();
    let top = geometry.y.floor();
    let right = (geometry.x + geometry.width).ceil();
    let bottom = (geometry.y + geometry.height).ceil();
    if left < i32::MIN as f32
        || top < i32::MIN as f32
        || right > i32::MAX as f32
        || bottom > i32::MAX as f32
    {
        return Err(BackendError::new(
            "skia.geometry.integer-overflow",
            "geometry exceeds Skia integer rectangle limits",
        ));
    }
    Ok(IRect::from_ltrb(
        left as i32,
        top as i32,
        right as i32,
        bottom as i32,
    ))
}

fn geometry_intersects(left: Geometry, right: Geometry) -> bool {
    left.x < right.x + right.width
        && left.x + left.width > right.x
        && left.y < right.y + right.height
        && left.y + left.height > right.y
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

fn decode_raster_image(encoded_bytes: &[u8]) -> Option<Image> {
    let decoded = image::load_from_memory(encoded_bytes).ok()?.to_rgba8();
    let width = i32::try_from(decoded.width()).ok()?;
    let height = i32::try_from(decoded.height()).ok()?;
    let row_bytes = usize::try_from(u64::from(decoded.width()) * 4).ok()?;
    let info = ImageInfo::new(
        (width, height),
        ColorType::RGBA8888,
        AlphaType::Premul,
        None,
    );
    images::raster_from_data(&info, Data::new_copy(decoded.as_raw()), row_bytes)
}

fn capture_frame_snapshot(surface: &mut SkiaSurface) -> Result<SkiaFrameSnapshot, BackendError> {
    let width = surface.pixel_width();
    let height = surface.pixel_height();
    let skia_width = i32::try_from(width).map_err(|_| {
        BackendError::new(
            "skia.snapshot.size-overflow",
            "frame snapshot width exceeds Skia raster limits",
        )
    })?;
    let skia_height = i32::try_from(height).map_err(|_| {
        BackendError::new(
            "skia.snapshot.size-overflow",
            "frame snapshot height exceeds Skia raster limits",
        )
    })?;
    let row_bytes = usize::try_from(u64::from(width) * 4).map_err(|_| {
        BackendError::new(
            "skia.snapshot.row-bytes-overflow",
            "frame snapshot row byte count exceeds addressable memory",
        )
    })?;
    let byte_len = row_bytes
        .checked_mul(usize::try_from(height).map_err(|_| {
            BackendError::new(
                "skia.snapshot.byte-count-overflow",
                "frame snapshot byte count exceeds addressable memory",
            )
        })?)
        .ok_or_else(|| {
            BackendError::new(
                "skia.snapshot.byte-count-overflow",
                "frame snapshot byte count exceeds addressable memory",
            )
        })?;
    let image_info = ImageInfo::new(
        (skia_width, skia_height),
        ColorType::RGBA8888,
        AlphaType::Premul,
        None,
    );
    let mut rgba = vec![0_u8; byte_len];
    if !surface
        .raster_surface
        .read_pixels(&image_info, &mut rgba, row_bytes, (0, 0))
    {
        return Err(BackendError::new(
            "skia.snapshot.readback-failed",
            "failed to read presented Skia frame pixels",
        ));
    }
    let pixels = rgba
        .chunks_exact(4)
        .map(|chunk| (u32::from(chunk[0]) << 16) | (u32::from(chunk[1]) << 8) | u32::from(chunk[2]))
        .collect();
    SkiaFrameSnapshot::new(width, height, pixels)
}

fn paint(color: Color, style: PaintStyle) -> Paint {
    let mut paint = Paint::default();
    paint.set_style(style);
    paint.set_color(to_skia_color(color));
    paint
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn draw_custom_surface(
    canvas: &Canvas,
    category: CustomSurfaceCategory,
    geometry: Geometry,
    samples: &[f32],
) {
    let mut background = paint(Color::rgba(12, 15, 22, 220), PaintStyle::Fill);
    background.set_anti_alias(true);
    canvas.draw_rect(rect(geometry), &background);

    let mut outline = paint(Color::rgba(62, 72, 92, 255), PaintStyle::Stroke);
    outline.set_anti_alias(true);
    outline.set_stroke_width(1.0);
    canvas.draw_rect(rect(geometry), &outline);

    match category {
        CustomSurfaceCategory::Meter
        | CustomSurfaceCategory::Slider
        | CustomSurfaceCategory::Knob => {
            draw_meter_surface(canvas, geometry, samples);
        }
        CustomSurfaceCategory::Scope
        | CustomSurfaceCategory::Analyzer
        | CustomSurfaceCategory::EqCurve
        | CustomSurfaceCategory::Modulation
        | CustomSurfaceCategory::Timeline
        | CustomSurfaceCategory::GraphEditor
        | CustomSurfaceCategory::InspectorPanel => {
            draw_curve_surface(canvas, geometry, samples, category);
        }
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn draw_meter_surface(canvas: &Canvas, geometry: Geometry, samples: &[f32]) {
    let values = if samples.is_empty() {
        &[0.0][..]
    } else {
        samples
    };
    let bar_count = values.len().max(1);
    let gap = 2.0_f32.min(geometry.width / bar_count as f32 / 3.0);
    let bar_width =
        ((geometry.width - gap * (bar_count.saturating_sub(1) as f32)) / bar_count as f32).max(1.0);
    let mut fill = paint(Color::rgba(70, 222, 142, 255), PaintStyle::Fill);
    fill.set_anti_alias(true);
    for (index, sample) in values.iter().enumerate() {
        let normalized = sample.clamp(0.0, 1.0);
        let height = (geometry.height * normalized).max(1.0);
        let x = geometry.x + index as f32 * (bar_width + gap);
        let y = geometry.y + geometry.height - height;
        canvas.draw_rect(Rect::from_xywh(x, y, bar_width, height), &fill);
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn draw_curve_surface(
    canvas: &Canvas,
    geometry: Geometry,
    samples: &[f32],
    category: CustomSurfaceCategory,
) {
    let accent = match category {
        CustomSurfaceCategory::Analyzer => Color::rgba(80, 180, 255, 255),
        CustomSurfaceCategory::EqCurve => Color::rgba(255, 198, 74, 255),
        CustomSurfaceCategory::Timeline => Color::rgba(255, 118, 96, 255),
        CustomSurfaceCategory::GraphEditor => Color::rgba(188, 132, 255, 255),
        _ => Color::rgba(70, 222, 142, 255),
    };
    let mut stroke = paint(accent, PaintStyle::Stroke);
    stroke.set_anti_alias(true);
    stroke.set_stroke_width(2.0);
    if samples.len() < 2 {
        let y = geometry.y + geometry.height / 2.0;
        canvas.draw_line((geometry.x, y), (geometry.x + geometry.width, y), &stroke);
        return;
    }
    let step = geometry.width / (samples.len() - 1) as f32;
    let mut path = PathBuilder::new();
    for (index, sample) in samples.iter().enumerate() {
        let x = geometry.x + index as f32 * step;
        let y = geometry.y + geometry.height - geometry.height * sample.clamp(0.0, 1.0);
        if index == 0 {
            path.move_to((x, y));
        } else {
            path.line_to((x, y));
        }
    }
    canvas.draw_path(&path.detach(), &stroke);
}

fn rect(geometry: Geometry) -> Rect {
    Rect::from_xywh(geometry.x, geometry.y, geometry.width, geometry.height)
}

fn to_skia_color(color: Color) -> SkiaColor {
    SkiaColor::from_argb(color.a, color.r, color.g, color.b)
}

fn to_skia_color4f(color: Color) -> Color4f {
    Color4f::new(
        f32::from(color.r) / 255.0,
        f32::from(color.g) / 255.0,
        f32::from(color.b) / 255.0,
        f32::from(color.a) / 255.0,
    )
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

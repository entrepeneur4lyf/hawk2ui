//! Skia-backed software frame generation for native host presentation.

use hawk2ui_assets::{AssetKind, AssetRecord};
use hawk2ui_host::SurfaceMetrics;
use hawk2ui_render::{BackendError, Color, Geometry, RendererBackend, Transform};
use hawk2ui_render_skia::{
    RuntimeSceneAssetFallback, RuntimeSceneReplayOptions, SkiaRendererBackend, SkiaSurfaceConfig,
};
use hawk2ui_runtime::RuntimeSceneFrame;
use skia_safe::{
    AlphaType, Color as SkiaColor, ColorType, Font, ImageInfo, Paint, PaintStyle, Rect, surfaces,
};

use crate::WinitHostError;

const SOFTWARE_FRAME_SURFACE_ID: &str = "software-frame";

/// Fully-rendered software frame in the `softbuffer` presentation format.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SoftwareFrame {
    width: u32,
    height: u32,
    pixels: Vec<u32>,
}

impl SoftwareFrame {
    /// Creates a software frame from validated pixel data.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when dimensions are zero or the pixel count is invalid.
    pub fn new(width: u32, height: u32, pixels: Vec<u32>) -> Result<Self, WinitHostError> {
        validate_pixel_size(width, height)?;
        let expected_len = usize::try_from(u64::from(width) * u64::from(height)).map_err(|_| {
            WinitHostError::new(
                "desktop.frame.pixel-count-overflow",
                "frame pixel count exceeds addressable memory",
            )
        })?;
        if pixels.len() != expected_len {
            return Err(WinitHostError::new(
                "desktop.frame.pixel-count-mismatch",
                "frame pixel buffer length does not match frame dimensions",
            ));
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    /// Returns the frame width in physical pixels.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Returns the frame height in physical pixels.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Returns pixels in `0x00RRGGBB` presentation format.
    #[must_use]
    pub fn pixels(&self) -> &[u32] {
        &self.pixels
    }
}

/// Skia-backed frame renderer for the first production desktop host vertical.
#[derive(Clone, Debug, Default)]
pub struct SoftwareFrameRenderer {
    assets: Vec<AssetRecord>,
    error_overlay: Option<DesktopErrorOverlay>,
}

/// Visual diagnostic overlay rendered inside a development desktop surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopErrorOverlay {
    rule: String,
    message: String,
    source_path: Option<String>,
}

impl DesktopErrorOverlay {
    /// Creates an error overlay from a stable diagnostic rule and user-facing message.
    #[must_use]
    pub fn new(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            rule: rule.into(),
            message: message.into(),
            source_path: None,
        }
    }

    /// Adds a source path to display in the overlay.
    #[must_use]
    pub fn with_source_path(mut self, source_path: impl Into<String>) -> Self {
        self.source_path = Some(source_path.into());
        self
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

    /// Returns the optional source path.
    #[must_use]
    pub fn source_path(&self) -> Option<&str> {
        self.source_path.as_deref()
    }
}

impl SoftwareFrameRenderer {
    /// Creates a software frame renderer without pre-registered runtime assets.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            assets: Vec::new(),
            error_overlay: None,
        }
    }

    /// Adds compiled runtime assets that scene image/vector commands may draw.
    #[must_use]
    pub fn with_assets(mut self, assets: impl IntoIterator<Item = AssetRecord>) -> Self {
        self.assets = assets.into_iter().collect();
        self
    }

    /// Adds a development error overlay to composite over rendered frames.
    #[must_use]
    pub fn with_error_overlay(mut self, overlay: DesktopErrorOverlay) -> Self {
        self.error_overlay = Some(overlay);
        self
    }

    /// Returns the compiled runtime assets registered with this renderer.
    #[must_use]
    pub fn assets(&self) -> &[AssetRecord] {
        &self.assets
    }

    /// Returns the configured development error overlay.
    #[must_use]
    pub fn error_overlay(&self) -> Option<&DesktopErrorOverlay> {
        self.error_overlay.as_ref()
    }

    /// Renders a full-surface frame through Skia.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when dimensions are invalid, Skia cannot allocate a raster
    /// surface, or pixels cannot be read back for host presentation.
    pub fn render_frame(
        &self,
        title: &str,
        width: u32,
        height: u32,
        scale_factor: f64,
    ) -> Result<SoftwareFrame, WinitHostError> {
        validate_pixel_size(width, height)?;
        let skia_width = i32::try_from(width).map_err(|_| {
            WinitHostError::new(
                "desktop.frame.size-overflow",
                "frame width exceeds Skia raster limits",
            )
        })?;
        let skia_height = i32::try_from(height).map_err(|_| {
            WinitHostError::new(
                "desktop.frame.size-overflow",
                "frame height exceeds Skia raster limits",
            )
        })?;
        let mut surface =
            surfaces::raster_n32_premul((skia_width, skia_height)).ok_or_else(|| {
                WinitHostError::new(
                    "desktop.frame.skia-allocation-failed",
                    "failed to allocate Skia CPU raster surface",
                )
            })?;

        draw_default_scene(surface.canvas(), title, width, height, scale_factor);
        if let Some(overlay) = &self.error_overlay {
            draw_error_overlay_canvas(surface.canvas(), width, height, scale_factor, overlay);
        }

        surface_to_frame(surface, width, height, skia_width, skia_height)
    }

    /// Renders a prepared runtime scene frame through Skia.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when dimensions are invalid, Skia cannot allocate a raster
    /// surface, draw commands are invalid, or pixels cannot be read back for host presentation.
    pub fn render_scene_frame(
        &self,
        scene: &RuntimeSceneFrame,
        width: u32,
        height: u32,
        scale_factor: f64,
    ) -> Result<SoftwareFrame, WinitHostError> {
        validate_pixel_size(width, height)?;
        let scale = scale_factor_to_f32(scale_factor)?;
        let mut backend = SkiaRendererBackend::new();
        backend
            .create_surface_with_config(SkiaSurfaceConfig::cpu_raster(
                SOFTWARE_FRAME_SURFACE_ID,
                width,
                height,
            ))
            .map_err(|error| map_backend_error(&error))?;
        register_runtime_assets(&mut backend, &self.assets)?;
        backend
            .begin_frame(SOFTWARE_FRAME_SURFACE_ID)
            .map_err(|error| map_backend_error(&error))?;
        backend
            .clear(Color::rgba(0, 0, 0, 0))
            .map_err(|error| map_backend_error(&error))?;
        backend
            .push_transform(Transform::affine(scale, 0.0, 0.0, scale, 0.0, 0.0))
            .map_err(|error| map_backend_error(&error))?;
        backend
            .draw_runtime_scene_frame_with_options(
                scene,
                RuntimeSceneReplayOptions::new(0, scale)
                    .with_missing_asset_fallback(RuntimeSceneAssetFallback::Placeholder),
            )
            .map_err(|error| map_backend_error(&error))?;
        if let Some(overlay) = &self.error_overlay {
            draw_error_overlay_backend(&mut backend, width, height, scale, overlay)
                .map_err(|error| map_backend_error(&error))?;
        }

        backend
            .end_frame(SOFTWARE_FRAME_SURFACE_ID)
            .map_err(|error| map_backend_error(&error))?;
        let snapshot = backend
            .frame_snapshot(SOFTWARE_FRAME_SURFACE_ID)
            .map_err(|error| map_backend_error(&error))?;
        SoftwareFrame::new(
            snapshot.width(),
            snapshot.height(),
            snapshot.pixels().to_vec(),
        )
    }
}

fn register_runtime_assets(
    backend: &mut SkiaRendererBackend,
    assets: &[AssetRecord],
) -> Result<(), WinitHostError> {
    for asset in assets {
        if matches!(asset.kind(), AssetKind::Image | AssetKind::Vector) {
            backend
                .register_compiled_asset(asset)
                .map_err(|error| map_backend_error(&error))?;
        }
    }
    Ok(())
}

/// Converts host metrics to non-zero physical frame dimensions.
///
/// # Errors
///
/// Returns [`WinitHostError`] when the converted physical size is empty.
pub fn physical_frame_size(metrics: SurfaceMetrics) -> Result<(u32, u32), WinitHostError> {
    let (width, height) = metrics.physical_size();
    validate_pixel_size(width, height)?;
    Ok((width, height))
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn draw_default_scene(
    canvas: &skia_safe::Canvas,
    title: &str,
    width: u32,
    height: u32,
    scale_factor: f64,
) {
    canvas.clear(SkiaColor::from_argb(255, 11, 12, 18));

    let mut panel = Paint::default();
    panel.set_style(PaintStyle::Fill);
    panel.set_anti_alias(true);
    panel.set_color(SkiaColor::from_argb(255, 20, 22, 31));
    canvas.draw_rect(
        Rect::from_xywh(0.0, 0.0, width as f32, height as f32),
        &panel,
    );

    let accent_width = (width as f32).max(1.0);
    let mut accent = Paint::default();
    accent.set_style(PaintStyle::Fill);
    accent.set_anti_alias(true);
    accent.set_color(SkiaColor::from_argb(255, 42, 128, 255));
    canvas.draw_rect(Rect::from_xywh(0.0, 0.0, accent_width, 4.0), &accent);

    let mut text = Paint::default();
    text.set_style(PaintStyle::Fill);
    text.set_anti_alias(true);
    text.set_color(SkiaColor::from_argb(255, 241, 245, 249));
    let mut font = Font::default();
    font.set_size((18.0 * scale_factor.max(1.0)) as f32);
    canvas.draw_str(title, (24.0, 48.0), &font, &text);

    let mut secondary = Paint::default();
    secondary.set_style(PaintStyle::Fill);
    secondary.set_anti_alias(true);
    secondary.set_color(SkiaColor::from_argb(255, 166, 173, 186));
    let mut secondary_font = Font::default();
    secondary_font.set_size((13.0 * scale_factor.max(1.0)) as f32);
    canvas.draw_str(
        "Native desktop runtime: winit + Skia + software presentation",
        (24.0, 78.0),
        &secondary_font,
        &secondary,
    );
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn draw_error_overlay_canvas(
    canvas: &skia_safe::Canvas,
    width: u32,
    height: u32,
    scale_factor: f64,
    overlay: &DesktopErrorOverlay,
) {
    let scale = scale_factor.max(1.0) as f32;
    let margin = 18.0 * scale;
    let overlay_width = ((width as f32) - margin * 2.0).min(560.0 * scale).max(1.0);
    let overlay_height = 128.0 * scale;
    let x = margin;
    let y = ((height as f32) - overlay_height - margin).max(margin);

    let mut shadow = Paint::default();
    shadow.set_style(PaintStyle::Fill);
    shadow.set_anti_alias(true);
    shadow.set_color(SkiaColor::from_argb(170, 0, 0, 0));
    canvas.draw_rect(
        Rect::from_xywh(
            x + 6.0 * scale,
            y + 8.0 * scale,
            overlay_width,
            overlay_height,
        ),
        &shadow,
    );

    let mut panel = Paint::default();
    panel.set_style(PaintStyle::Fill);
    panel.set_anti_alias(true);
    panel.set_color(SkiaColor::from_argb(245, 24, 18, 26));
    canvas.draw_rect(Rect::from_xywh(x, y, overlay_width, overlay_height), &panel);

    let mut accent = Paint::default();
    accent.set_style(PaintStyle::Fill);
    accent.set_anti_alias(true);
    accent.set_color(SkiaColor::from_argb(255, 248, 81, 73));
    canvas.draw_rect(Rect::from_xywh(x, y, 6.0 * scale, overlay_height), &accent);

    let mut heading = Paint::default();
    heading.set_style(PaintStyle::Fill);
    heading.set_anti_alias(true);
    heading.set_color(SkiaColor::from_argb(255, 255, 245, 245));
    let mut heading_font = Font::default();
    heading_font.set_size(18.0 * scale);
    canvas.draw_str(
        format!("Build error: {}", overlay.rule()),
        (x + 22.0 * scale, y + 34.0 * scale),
        &heading_font,
        &heading,
    );

    let mut body = Paint::default();
    body.set_style(PaintStyle::Fill);
    body.set_anti_alias(true);
    body.set_color(SkiaColor::from_argb(255, 253, 186, 186));
    let mut body_font = Font::default();
    body_font.set_size(14.0 * scale);
    canvas.draw_str(
        overlay.message(),
        (x + 22.0 * scale, y + 66.0 * scale),
        &body_font,
        &body,
    );
    if let Some(source_path) = overlay.source_path() {
        canvas.draw_str(
            source_path,
            (x + 22.0 * scale, y + 96.0 * scale),
            &body_font,
            &body,
        );
    }
}

#[allow(clippy::cast_precision_loss)]
fn draw_error_overlay_backend(
    backend: &mut SkiaRendererBackend,
    width: u32,
    height: u32,
    scale: f32,
    overlay: &DesktopErrorOverlay,
) -> Result<(), BackendError> {
    let logical_width = width as f32 / scale;
    let logical_height = height as f32 / scale;
    let overlay_width = (logical_width - 36.0).clamp(1.0, 560.0);
    let overlay_height = 128.0;
    let x = 18.0;
    let y = (logical_height - overlay_height - 18.0).max(18.0);

    backend.draw_shadow_rect(
        Geometry::new(x, y, overlay_width, overlay_height),
        8.0,
        10.0,
        12.0,
        Color::rgba(0, 0, 0, 170),
    )?;
    backend.draw_rounded_rect(
        Geometry::new(x, y, overlay_width, overlay_height),
        16.0,
        Color::rgba(24, 18, 26, 245),
    )?;
    backend.draw_rounded_rect(
        Geometry::new(x, y, 8.0, overlay_height),
        4.0,
        Color::rgba(248, 81, 73, 255),
    )?;
    backend.draw_text_at(
        &format!("Build error: {}", overlay.rule()),
        18.0,
        x + 24.0,
        y + 36.0,
        Color::rgba(255, 245, 245, 255),
    )?;
    backend.draw_text_at(
        overlay.message(),
        14.0,
        x + 24.0,
        y + 68.0,
        Color::rgba(253, 186, 186, 255),
    )?;
    if let Some(source_path) = overlay.source_path() {
        backend.draw_text_at(
            source_path,
            14.0,
            x + 24.0,
            y + 98.0,
            Color::rgba(253, 186, 186, 255),
        )?;
    }
    Ok(())
}

fn surface_to_frame(
    mut surface: skia_safe::Surface,
    width: u32,
    height: u32,
    skia_width: i32,
    skia_height: i32,
) -> Result<SoftwareFrame, WinitHostError> {
    let row_bytes = usize::try_from(u64::from(width) * 4).map_err(|_| {
        WinitHostError::new(
            "desktop.frame.row-bytes-overflow",
            "frame row byte count exceeds addressable memory",
        )
    })?;
    let byte_len = row_bytes
        .checked_mul(usize::try_from(height).map_err(|_| {
            WinitHostError::new(
                "desktop.frame.byte-count-overflow",
                "frame byte count exceeds addressable memory",
            )
        })?)
        .ok_or_else(|| {
            WinitHostError::new(
                "desktop.frame.byte-count-overflow",
                "frame byte count exceeds addressable memory",
            )
        })?;
    let image_info = ImageInfo::new(
        (skia_width, skia_height),
        ColorType::RGBA8888,
        AlphaType::Premul,
        None,
    );
    let mut rgba = vec![0_u8; byte_len];
    if !surface.read_pixels(&image_info, &mut rgba, row_bytes, (0, 0)) {
        return Err(WinitHostError::new(
            "desktop.frame.readback-failed",
            "failed to read Skia raster pixels",
        ));
    }

    let pixels = rgba
        .chunks_exact(4)
        .map(|chunk| (u32::from(chunk[0]) << 16) | (u32::from(chunk[1]) << 8) | u32::from(chunk[2]))
        .collect();
    SoftwareFrame::new(width, height, pixels)
}

#[allow(clippy::cast_possible_truncation)]
fn scale_factor_to_f32(scale_factor: f64) -> Result<f32, WinitHostError> {
    if !scale_factor.is_finite() || scale_factor <= 0.0 {
        return Err(WinitHostError::new(
            "desktop.frame.invalid-scale",
            "scale factor must be finite and greater than zero",
        ));
    }
    let scale = scale_factor as f32;
    if scale.is_finite() && scale > 0.0 {
        Ok(scale)
    } else {
        Err(WinitHostError::new(
            "desktop.frame.invalid-scale",
            "scale factor must be finite and greater than zero",
        ))
    }
}

fn validate_pixel_size(width: u32, height: u32) -> Result<(), WinitHostError> {
    if width == 0 || height == 0 {
        Err(WinitHostError::new(
            "desktop.frame.invalid-size",
            "frame dimensions must be greater than zero",
        ))
    } else {
        Ok(())
    }
}

fn map_backend_error(error: &BackendError) -> WinitHostError {
    WinitHostError::new(
        error.diagnostic().rule(),
        error.diagnostic().message().to_string(),
    )
}

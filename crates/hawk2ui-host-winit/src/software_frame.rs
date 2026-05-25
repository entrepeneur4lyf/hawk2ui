//! Skia-backed software frame generation for native host presentation.

use hawk2ui_host::SurfaceMetrics;
use skia_safe::{AlphaType, Color, ColorType, Font, ImageInfo, Paint, PaintStyle, Rect, surfaces};

use crate::WinitHostError;

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
pub struct SoftwareFrameRenderer;

impl SoftwareFrameRenderer {
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
            .map(|chunk| {
                (u32::from(chunk[0]) << 16) | (u32::from(chunk[1]) << 8) | u32::from(chunk[2])
            })
            .collect();
        SoftwareFrame::new(width, height, pixels)
    }
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
    canvas.clear(Color::from_argb(255, 11, 12, 18));

    let mut panel = Paint::default();
    panel.set_style(PaintStyle::Fill);
    panel.set_anti_alias(true);
    panel.set_color(Color::from_argb(255, 20, 22, 31));
    canvas.draw_rect(
        Rect::from_xywh(0.0, 0.0, width as f32, height as f32),
        &panel,
    );

    let accent_width = (width as f32).max(1.0);
    let mut accent = Paint::default();
    accent.set_style(PaintStyle::Fill);
    accent.set_anti_alias(true);
    accent.set_color(Color::from_argb(255, 42, 128, 255));
    canvas.draw_rect(Rect::from_xywh(0.0, 0.0, accent_width, 4.0), &accent);

    let mut text = Paint::default();
    text.set_style(PaintStyle::Fill);
    text.set_anti_alias(true);
    text.set_color(Color::from_argb(255, 241, 245, 249));
    let mut font = Font::default();
    font.set_size((18.0 * scale_factor.max(1.0)) as f32);
    canvas.draw_str(title, (24.0, 48.0), &font, &text);

    let mut secondary = Paint::default();
    secondary.set_style(PaintStyle::Fill);
    secondary.set_anti_alias(true);
    secondary.set_color(Color::from_argb(255, 166, 173, 186));
    let mut secondary_font = Font::default();
    secondary_font.set_size((13.0 * scale_factor.max(1.0)) as f32);
    canvas.draw_str(
        "Native desktop runtime: winit + Skia + software presentation",
        (24.0, 78.0),
        &secondary_font,
        &secondary,
    );
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

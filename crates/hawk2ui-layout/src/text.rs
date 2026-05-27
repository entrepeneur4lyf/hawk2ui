//! Text measurement bridge.

use hawk2ui_text::{LineBreakMode, TextBackend, TextLayoutInput, TruncationMode};

/// Text measurement mode.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextMeasureMode {
    /// Measure intrinsic single-line size.
    Intrinsic,
    /// Measure wrapped size with a maximum width.
    Wrap {
        /// Maximum line width.
        max_width: f32,
    },
    /// Measure truncated size with a maximum width.
    Truncate {
        /// Maximum line width.
        max_width: f32,
    },
}

/// Text measurement input.
#[derive(Clone, Debug, PartialEq)]
pub struct TextMeasureInput {
    text: String,
    font_family: String,
    font_size_px: f32,
    dpi_scale: f32,
    mode: TextMeasureMode,
}

impl TextMeasureInput {
    /// Creates text measurement input.
    #[must_use]
    pub fn new(
        text: impl Into<String>,
        font_family: impl Into<String>,
        font_size_px: f32,
        mode: TextMeasureMode,
    ) -> Self {
        Self {
            text: text.into(),
            font_family: font_family.into(),
            font_size_px,
            dpi_scale: 1.0,
            mode,
        }
    }

    /// Returns the text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the font family.
    #[must_use]
    pub fn font_family(&self) -> &str {
        &self.font_family
    }

    /// Returns the font size in pixels.
    #[must_use]
    pub const fn font_size_px(&self) -> f32 {
        self.font_size_px
    }

    /// Sets the DPI scale used by production text measurement.
    #[must_use]
    pub const fn with_dpi_scale(mut self, dpi_scale: f32) -> Self {
        self.dpi_scale = dpi_scale;
        self
    }

    /// Returns the DPI scale used by production text measurement.
    #[must_use]
    pub const fn dpi_scale(&self) -> f32 {
        self.dpi_scale
    }

    /// Returns the measurement mode.
    #[must_use]
    pub const fn mode(&self) -> TextMeasureMode {
        self.mode
    }
}

/// Stable text measurement cache key.
#[derive(Clone, Debug, PartialEq)]
pub struct TextMeasureKey {
    input: TextMeasureInput,
}

impl TextMeasureKey {
    /// Creates a text measurement cache key.
    #[must_use]
    pub fn new(
        text: impl Into<String>,
        font_family: impl Into<String>,
        font_size_px: f32,
        mode: TextMeasureMode,
    ) -> Self {
        Self {
            input: TextMeasureInput::new(text, font_family, font_size_px, mode),
        }
    }
}

/// Text measurement result.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextMeasureResult {
    /// Measured width.
    pub width: f32,
    /// Measured height.
    pub height: f32,
    /// Line count.
    pub line_count: u32,
    /// Whether output was truncated.
    pub truncated: bool,
}

impl TextMeasureResult {
    /// Creates a text measurement result.
    #[must_use]
    pub const fn new(width: f32, height: f32, line_count: u32, truncated: bool) -> Self {
        Self {
            width,
            height,
            line_count,
            truncated,
        }
    }
}

/// Text measurement error reported while feeding text sizing into layout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextMeasureError {
    rule: String,
    message: String,
}

impl TextMeasureError {
    /// Creates a text measurement error.
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

    /// Returns the human-readable diagnostic message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Text measurement provider used by layout.
pub trait LayoutTextMeasurer {
    /// Measures text for Taffy leaf sizing.
    ///
    /// # Errors
    ///
    /// Returns [`TextMeasureError`] when the measurement backend rejects the input.
    fn measure(&self, input: &TextMeasureInput) -> Result<TextMeasureResult, TextMeasureError>;
}

/// Production adapter from `hawk2ui-text` into layout measurement.
pub struct HawkTextMeasurer {
    backend: TextBackend,
}

impl HawkTextMeasurer {
    /// Creates a production text measurer from a text backend.
    #[must_use]
    pub const fn new(backend: TextBackend) -> Self {
        Self { backend }
    }

    /// Returns the underlying production text backend.
    #[must_use]
    pub const fn backend(&self) -> &TextBackend {
        &self.backend
    }
}

impl LayoutTextMeasurer for HawkTextMeasurer {
    fn measure(&self, input: &TextMeasureInput) -> Result<TextMeasureResult, TextMeasureError> {
        let layout_input = text_layout_input(input);
        let layout = self.backend.layout(&layout_input).map_err(|error| {
            TextMeasureError::new(
                error.diagnostic().rule(),
                error.diagnostic().message().to_string(),
            )
        })?;
        Ok(TextMeasureResult::new(
            layout.width_px(),
            layout.height_px(),
            layout.line_count(),
            layout.truncated(),
        ))
    }
}

/// Deterministic text measurer used by layout tests and fixtures.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TestTextMeasurer {
    average_glyph_width: f32,
    line_height_multiplier: f32,
}

impl TestTextMeasurer {
    /// Creates a deterministic text measurer.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            average_glyph_width: 8.0,
            line_height_multiplier: 1.2,
        }
    }

    /// Sets average glyph width in pixels.
    #[must_use]
    pub const fn with_average_glyph_width(mut self, average_glyph_width: f32) -> Self {
        self.average_glyph_width = average_glyph_width;
        self
    }

    /// Measures text deterministically.
    #[must_use]
    pub fn measure(&self, input: &TextMeasureInput) -> TextMeasureResult {
        let glyph_count = input.text.chars().fold(0.0_f32, |count, _| count + 1.0);
        let intrinsic_width = glyph_count * self.average_glyph_width;
        let line_height = input.font_size_px * self.line_height_multiplier;
        match input.mode {
            TextMeasureMode::Intrinsic => {
                TextMeasureResult::new(intrinsic_width, line_height, 1, false)
            }
            TextMeasureMode::Wrap { max_width } => {
                let lines = wrapped_line_count(intrinsic_width, max_width);
                let height = (0..lines).fold(0.0_f32, |height, _| height + line_height);
                TextMeasureResult::new(max_width.min(intrinsic_width), height, lines, false)
            }
            TextMeasureMode::Truncate { max_width } => TextMeasureResult::new(
                max_width.min(intrinsic_width),
                line_height,
                1,
                intrinsic_width > max_width,
            ),
        }
    }
}

impl LayoutTextMeasurer for TestTextMeasurer {
    fn measure(&self, input: &TextMeasureInput) -> Result<TextMeasureResult, TextMeasureError> {
        Ok(Self::measure(self, input))
    }
}

impl Default for TestTextMeasurer {
    fn default() -> Self {
        Self::new()
    }
}

fn text_layout_input(input: &TextMeasureInput) -> TextLayoutInput {
    let mut layout_input =
        TextLayoutInput::new(input.text(), input.font_family(), input.font_size_px())
            .with_dpi_scale(input.dpi_scale())
            .with_bidi(true);
    match input.mode() {
        TextMeasureMode::Intrinsic => {}
        TextMeasureMode::Wrap { max_width } => {
            layout_input = layout_input.with_line_break(LineBreakMode::Wrap {
                max_width_px: max_width,
            });
        }
        TextMeasureMode::Truncate { max_width } => {
            layout_input = layout_input.with_truncation(TruncationMode::EndEllipsis {
                max_width_px: max_width,
            });
        }
    }
    layout_input
}

fn wrapped_line_count(intrinsic_width: f32, max_width: f32) -> u32 {
    if max_width <= 0.0 {
        return 1;
    }
    let mut remaining_width = intrinsic_width;
    let mut lines = 1_u32;
    while remaining_width > max_width {
        remaining_width -= max_width;
        lines = lines.saturating_add(1);
    }
    lines
}

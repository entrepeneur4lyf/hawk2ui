//! Text rendering contracts.

/// Font registry for discovered and app-provided fonts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FontRegistry {
    system_fonts: Vec<String>,
    app_fonts: Vec<(String, String)>,
}

impl FontRegistry {
    /// Creates an empty font registry.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            system_fonts: Vec::new(),
            app_fonts: Vec::new(),
        }
    }

    /// Adds a discovered system font.
    #[must_use]
    pub fn with_system_font(mut self, family: impl Into<String>) -> Self {
        self.system_fonts.push(family.into());
        self
    }

    /// Adds an application font.
    #[must_use]
    pub fn with_app_font(mut self, family: impl Into<String>, path: impl Into<String>) -> Self {
        self.app_fonts.push((family.into(), path.into()));
        self
    }

    fn contains_family(&self, family: &str) -> bool {
        self.system_fonts.iter().any(|font| font == family)
            || self.app_fonts.iter().any(|(font, _)| font == family)
    }
}

impl Default for FontRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Line breaking mode.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineBreakMode {
    /// No line breaking.
    None,
    /// Wrap to a maximum width.
    Wrap {
        /// Maximum width.
        max_width: f32,
    },
}

/// Text render measurement input.
#[derive(Clone, Debug, PartialEq)]
pub struct TextRenderInput {
    text: String,
    font_family: String,
    size_px: f32,
    dpi_scale: f32,
    line_break: LineBreakMode,
    bidi: bool,
}

impl TextRenderInput {
    /// Creates text render input.
    #[must_use]
    pub fn new(text: impl Into<String>, font_family: impl Into<String>, size_px: f32) -> Self {
        Self {
            text: text.into(),
            font_family: font_family.into(),
            size_px,
            dpi_scale: 1.0,
            line_break: LineBreakMode::None,
            bidi: false,
        }
    }

    /// Sets DPI scale.
    #[must_use]
    pub const fn with_dpi_scale(mut self, dpi_scale: f32) -> Self {
        self.dpi_scale = dpi_scale;
        self
    }

    /// Sets line breaking mode.
    #[must_use]
    pub const fn with_line_break(mut self, line_break: LineBreakMode) -> Self {
        self.line_break = line_break;
        self
    }

    /// Enables or disables bidi resolution.
    #[must_use]
    pub const fn with_bidi(mut self, bidi: bool) -> Self {
        self.bidi = bidi;
        self
    }
}

/// Text measurement output.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextMeasureOutput {
    /// Measured width.
    pub width: f32,
    /// Measured height.
    pub height: f32,
    /// Baseline in device pixels.
    pub baseline: f32,
    /// Line count.
    pub line_count: u32,
    /// Whether shaping occurred.
    pub shaped: bool,
    /// Whether bidi resolution occurred.
    pub bidi_resolved: bool,
}

/// Text rendering error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextRenderTextError {
    rule: String,
    message: String,
}

impl TextRenderTextError {
    /// Creates a text rendering error.
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

    /// Returns the diagnostic message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Deterministic text measurer.
#[derive(Clone, Debug, PartialEq)]
pub struct DeterministicTextMeasurer {
    registry: FontRegistry,
    average_glyph_width: f32,
}

impl DeterministicTextMeasurer {
    /// Creates a deterministic text measurer.
    #[must_use]
    pub const fn new(registry: FontRegistry) -> Self {
        Self {
            registry,
            average_glyph_width: 8.0,
        }
    }

    /// Sets average glyph width.
    #[must_use]
    pub const fn with_average_glyph_width(mut self, average_glyph_width: f32) -> Self {
        self.average_glyph_width = average_glyph_width;
        self
    }

    /// Measures text with deterministic shaping and line breaking.
    ///
    /// # Errors
    ///
    /// Returns [`TextRenderTextError`] when the requested font family is unavailable.
    pub fn measure(
        &self,
        input: &TextRenderInput,
    ) -> Result<TextMeasureOutput, TextRenderTextError> {
        validate_text_metrics(input, self.average_glyph_width)?;
        if !self.registry.contains_family(&input.font_family) {
            return Err(TextRenderTextError::new(
                "text.font.unavailable",
                format!("font family '{}' is not registered", input.font_family),
            ));
        }
        let glyph_count = input.text.chars().fold(0.0_f32, |count, _| count + 1.0);
        let intrinsic_width = glyph_count * self.average_glyph_width;
        let line_count = match input.line_break {
            LineBreakMode::None => 1,
            LineBreakMode::Wrap { max_width } => wrapped_line_count(intrinsic_width, max_width),
        };
        let width = match input.line_break {
            LineBreakMode::None => intrinsic_width,
            LineBreakMode::Wrap { max_width } => max_width.min(intrinsic_width),
        };
        let line_height = input.size_px * input.dpi_scale * 1.2;
        let height = (0..line_count).fold(0.0_f32, |height, _| height + line_height);
        Ok(TextMeasureOutput {
            width: round_tenth(width),
            height: round_tenth(height),
            baseline: round_tenth(input.size_px * input.dpi_scale * 0.8),
            line_count,
            shaped: true,
            bidi_resolved: input.bidi,
        })
    }
}

fn validate_text_metrics(
    input: &TextRenderInput,
    average_glyph_width: f32,
) -> Result<(), TextRenderTextError> {
    if input.font_family.trim().is_empty() {
        return Err(TextRenderTextError::new(
            "text.font-family.invalid",
            "font family must not be empty",
        ));
    }
    if !input.size_px.is_finite() || input.size_px <= 0.0 {
        return Err(TextRenderTextError::new(
            "text.size.invalid",
            "font size must be finite and greater than zero",
        ));
    }
    if !input.dpi_scale.is_finite() || input.dpi_scale <= 0.0 {
        return Err(TextRenderTextError::new(
            "text.dpi.invalid",
            "DPI scale must be finite and greater than zero",
        ));
    }
    if !average_glyph_width.is_finite() || average_glyph_width <= 0.0 {
        return Err(TextRenderTextError::new(
            "text.average-glyph-width.invalid",
            "average glyph width must be finite and greater than zero",
        ));
    }
    match input.line_break {
        LineBreakMode::None => Ok(()),
        LineBreakMode::Wrap { max_width } if max_width.is_finite() && max_width > 0.0 => Ok(()),
        LineBreakMode::Wrap { .. } => Err(TextRenderTextError::new(
            "text.wrap-width.invalid",
            "wrap width must be finite and greater than zero",
        )),
    }
}

/// Glyph cache key.
#[derive(Clone, Debug, PartialEq)]
pub struct GlyphCacheKey {
    text: String,
    font_family: String,
    size_px: f32,
    dpi_scale: f32,
    bidi: bool,
}

impl GlyphCacheKey {
    /// Creates a glyph cache key.
    #[must_use]
    pub fn new(
        text: impl Into<String>,
        font_family: impl Into<String>,
        size_px: f32,
        dpi_scale: f32,
        bidi: bool,
    ) -> Self {
        Self {
            text: text.into(),
            font_family: font_family.into(),
            size_px,
            dpi_scale,
            bidi,
        }
    }

    /// Returns the stable cache key.
    #[must_use]
    pub fn stable_key(&self) -> String {
        format!(
            "text={}|font={}|size={}|dpi={}|bidi={}",
            self.text, self.font_family, self.size_px, self.dpi_scale, self.bidi
        )
    }
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

fn round_tenth(value: f32) -> f32 {
    (value * 10.0).round() / 10.0
}

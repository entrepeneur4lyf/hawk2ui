#![forbid(unsafe_code)]
//! Production text backend for `Hawk2UI` font discovery, shaping, line breaking, bidi, glyph cache, and high-DPI metrics.

use std::sync::{Arc, Mutex};

use parley::{FontContext, FontStack, LayoutContext, StyleProperty};
use swash::scale::ScaleContext;
use unicode_bidi::BidiInfo;
use unicode_segmentation::UnicodeSegmentation;

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-text";

/// Returns the canonical Cargo package name for diagnostics and conformance checks.
#[must_use]
pub const fn crate_name() -> &'static str {
    CRATE_NAME
}

/// Font catalog used by the text backend.
#[derive(Debug)]
pub struct FontCatalog {
    database: fontdb::Database,
    system_families: Vec<String>,
    app_fonts: Vec<AppFont>,
    fallback_families: Vec<String>,
    generation: u64,
}

impl FontCatalog {
    /// Creates an empty font catalog.
    #[must_use]
    pub fn new() -> Self {
        Self {
            database: fontdb::Database::new(),
            system_families: Vec::new(),
            app_fonts: Vec::new(),
            fallback_families: Vec::new(),
            generation: 0,
        }
    }

    /// Registers a discovered system font family.
    #[must_use]
    pub fn with_system_family(mut self, family: impl Into<String>) -> Self {
        self.system_families.push(family.into());
        self.generation = self.generation.saturating_add(1);
        self
    }

    /// Registers application font bytes.
    #[must_use]
    pub fn with_app_font(
        mut self,
        family: impl Into<String>,
        source_path: impl Into<String>,
        bytes: Vec<u8>,
    ) -> Self {
        let loaded_faces = self
            .database
            .load_font_source(fontdb::Source::Binary(Arc::new(bytes.clone())));
        if !loaded_faces.is_empty() {
            self.app_fonts.push(AppFont {
                family: family.into(),
                source_path: source_path.into(),
                bytes,
            });
            self.generation = self.generation.saturating_add(1);
        }
        self
    }

    /// Registers a fallback font family.
    #[must_use]
    pub fn with_fallback_family(mut self, family: impl Into<String>) -> Self {
        self.fallback_families.push(family.into());
        self.generation = self.generation.saturating_add(1);
        self
    }

    /// Returns the catalog generation for cache invalidation.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns registered app font metadata.
    #[must_use]
    pub fn app_font_sources(&self) -> Vec<AppFontSource<'_>> {
        self.app_fonts
            .iter()
            .map(|font| AppFontSource {
                family: &font.family,
                source_path: &font.source_path,
                byte_len: font.bytes.len(),
            })
            .collect()
    }

    fn contains_family(&self, family: &str) -> bool {
        self.system_families
            .iter()
            .any(|candidate| is_valid_font_family(candidate) && candidate == family)
            || database_contains_family(&self.database, family)
            || self.app_fonts.iter().any(|font| {
                is_valid_font_family(&font.family)
                    && !font.source_path.trim().is_empty()
                    && !font.bytes.is_empty()
                    && font.family == family
            })
    }

    fn first_fallback(&self) -> Option<&str> {
        self.fallback_families
            .iter()
            .find(|family| is_valid_font_family(family))
            .map(String::as_str)
    }
}

impl Default for FontCatalog {
    fn default() -> Self {
        Self::new()
    }
}

/// Borrowed app font metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AppFontSource<'font> {
    /// Registered font family.
    pub family: &'font str,
    /// Source path used for diagnostics.
    pub source_path: &'font str,
    /// Number of loaded font bytes.
    pub byte_len: usize,
}

#[derive(Debug)]
struct AppFont {
    family: String,
    source_path: String,
    bytes: Vec<u8>,
}

/// Text line breaking mode.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineBreakMode {
    /// Keep text on one line.
    None,
    /// Wrap text to a maximum pixel width.
    Wrap {
        /// Maximum line width in physical pixels.
        max_width_px: f32,
    },
}

/// Text truncation mode.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TruncationMode {
    /// Do not truncate.
    None,
    /// Truncate the end and append an ellipsis.
    EndEllipsis {
        /// Maximum line width in physical pixels.
        max_width_px: f32,
    },
}

/// Text layout input.
#[derive(Clone, Debug, PartialEq)]
pub struct TextLayoutInput {
    text: String,
    font_family: String,
    size_px: f32,
    dpi_scale: f32,
    line_break: LineBreakMode,
    truncation: TruncationMode,
    bidi: bool,
}

impl TextLayoutInput {
    /// Creates text layout input.
    #[must_use]
    pub fn new(text: impl Into<String>, font_family: impl Into<String>, size_px: f32) -> Self {
        Self {
            text: text.into(),
            font_family: font_family.into(),
            size_px,
            dpi_scale: 1.0,
            line_break: LineBreakMode::None,
            truncation: TruncationMode::None,
            bidi: false,
        }
    }

    /// Sets DPI scale.
    #[must_use]
    pub const fn with_dpi_scale(mut self, dpi_scale: f32) -> Self {
        self.dpi_scale = dpi_scale;
        self
    }

    /// Sets line breaking.
    #[must_use]
    pub const fn with_line_break(mut self, line_break: LineBreakMode) -> Self {
        self.line_break = line_break;
        self
    }

    /// Sets truncation.
    #[must_use]
    pub const fn with_truncation(mut self, truncation: TruncationMode) -> Self {
        self.truncation = truncation;
        self
    }

    /// Enables or disables bidi resolution.
    #[must_use]
    pub const fn with_bidi(mut self, bidi: bool) -> Self {
        self.bidi = bidi;
        self
    }

    /// Returns source text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// Text layout output.
#[derive(Clone, Debug, PartialEq)]
pub struct TextLayout {
    resolved_family: String,
    display_text: String,
    lines: Vec<TextLayoutLine>,
    width_px: f32,
    height_px: f32,
    baseline_px: f32,
    line_count: u32,
    cluster_count: usize,
    flags: u8,
}

/// One positioned line produced by the text backend.
#[derive(Clone, Debug, PartialEq)]
pub struct TextLayoutLine {
    text: String,
    width_px: f32,
    baseline_px: f32,
}

struct ParleyLayoutMetrics {
    lines: Vec<TextLayoutLine>,
    width_px: f32,
    height_px: f32,
    baseline_px: f32,
}

impl TextLayoutLine {
    /// Returns the text slice to draw for this line.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns this line's measured width in physical pixels.
    #[must_use]
    pub const fn width_px(&self) -> f32 {
        self.width_px
    }

    /// Returns this line's baseline offset from the layout top in physical pixels.
    #[must_use]
    pub const fn baseline_px(&self) -> f32 {
        self.baseline_px
    }
}

impl TextLayout {
    /// Returns the resolved font family.
    #[must_use]
    pub fn resolved_family(&self) -> &str {
        &self.resolved_family
    }

    /// Returns display text after truncation.
    #[must_use]
    pub fn display_text(&self) -> &str {
        &self.display_text
    }

    /// Returns positioned lines that renderers should draw.
    #[must_use]
    pub fn lines(&self) -> &[TextLayoutLine] {
        &self.lines
    }

    /// Returns measured width in physical pixels.
    #[must_use]
    pub const fn width_px(&self) -> f32 {
        self.width_px
    }

    /// Returns measured height in physical pixels.
    #[must_use]
    pub const fn height_px(&self) -> f32 {
        self.height_px
    }

    /// Returns baseline in physical pixels.
    #[must_use]
    pub const fn baseline_px(&self) -> f32 {
        self.baseline_px
    }

    /// Returns line count.
    #[must_use]
    pub const fn line_count(&self) -> u32 {
        self.line_count
    }

    /// Returns shaped cluster count.
    #[must_use]
    pub const fn cluster_count(&self) -> usize {
        self.cluster_count
    }

    /// Returns whether the layout contains emoji clusters.
    #[must_use]
    pub const fn contains_emoji(&self) -> bool {
        self.flags & TEXT_LAYOUT_CONTAINS_EMOJI != 0
    }

    /// Returns whether bidi resolution was applied.
    #[must_use]
    pub const fn bidi_resolved(&self) -> bool {
        self.flags & TEXT_LAYOUT_BIDI_RESOLVED != 0
    }

    /// Returns whether the layout passed through Parley's layout pipeline.
    #[must_use]
    pub const fn parley_processed(&self) -> bool {
        self.flags & TEXT_LAYOUT_PARLEY_PROCESSED != 0
    }

    /// Returns whether the text was truncated.
    #[must_use]
    pub const fn truncated(&self) -> bool {
        self.flags & TEXT_LAYOUT_TRUNCATED != 0
    }
}

const TEXT_LAYOUT_CONTAINS_EMOJI: u8 = 1 << 0;
const TEXT_LAYOUT_BIDI_RESOLVED: u8 = 1 << 1;
const TEXT_LAYOUT_PARLEY_PROCESSED: u8 = 1 << 2;
const TEXT_LAYOUT_TRUNCATED: u8 = 1 << 3;
const TEXT_INPUT_MAX_BYTES: usize = 256 * 1024;

/// Stable glyph cache key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GlyphCacheKey {
    stable_key: String,
}

impl GlyphCacheKey {
    /// Returns the stable key string.
    #[must_use]
    pub fn stable_key(&self) -> &str {
        &self.stable_key
    }
}

/// Text backend diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextDiagnostic {
    rule: String,
    message: String,
}

impl TextDiagnostic {
    /// Creates a text diagnostic.
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

/// Text backend error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextBackendError {
    diagnostic: TextDiagnostic,
}

impl TextBackendError {
    /// Creates a text backend error.
    #[must_use]
    pub fn new(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            diagnostic: TextDiagnostic::new(rule, message),
        }
    }

    /// Returns the structured diagnostic.
    #[must_use]
    pub const fn diagnostic(&self) -> &TextDiagnostic {
        &self.diagnostic
    }
}

/// Production text backend.
pub struct TextBackend {
    catalog: FontCatalog,
    _scale_context: ScaleContext,
    parley_font_context: Mutex<FontContext>,
    parley_layout_context: Mutex<LayoutContext<()>>,
}

impl TextBackend {
    /// Creates a text backend.
    #[must_use]
    pub fn new(catalog: FontCatalog) -> Self {
        Self {
            catalog,
            _scale_context: ScaleContext::new(),
            parley_font_context: Mutex::new(FontContext::new()),
            parley_layout_context: Mutex::new(LayoutContext::new()),
        }
    }

    /// Returns the font catalog.
    #[must_use]
    pub const fn catalog(&self) -> &FontCatalog {
        &self.catalog
    }

    /// Resolves a font family to an available family or fallback.
    ///
    /// # Errors
    ///
    /// Returns [`TextBackendError`] when no requested or fallback font is available.
    pub fn resolve_family(&self, family: &str) -> Result<String, TextBackendError> {
        if !is_valid_font_family(family) {
            return Err(TextBackendError::new(
                "text.input.invalid-font-family",
                "font family must not be empty",
            ));
        }
        if self.catalog.contains_family(family) {
            Ok(family.to_string())
        } else {
            self.catalog
                .first_fallback()
                .map(str::to_string)
                .ok_or_else(|| {
                    TextBackendError::new(
                        "text.font.missing",
                        "font family is unavailable and no fallback is registered",
                    )
                })
        }
    }

    /// Returns the font generation used for cache invalidation.
    #[must_use]
    pub const fn font_generation(&self) -> u64 {
        self.catalog.generation()
    }

    /// Produces a shaped text layout.
    ///
    /// # Errors
    ///
    /// Returns [`TextBackendError`] when input validation or font resolution fails.
    pub fn layout(&self, input: &TextLayoutInput) -> Result<TextLayout, TextBackendError> {
        validate_input(input)?;
        let resolved_family = self.resolve_family(&input.font_family)?;
        let display_text = self.truncate_text(input, &resolved_family)?;
        let clusters: Vec<&str> = display_text.graphemes(true).collect();
        let cluster_count = clusters.len();
        let contains_emoji = clusters.iter().any(|cluster| cluster.chars().any(is_emoji));
        let bidi_resolved = input.bidi && display_text.chars().any(is_rtl);
        let parley_metrics = self.layout_with_parley(input, &resolved_family, &display_text)?;
        let lines = parley_metrics.lines;
        let line_count = u32::try_from(lines.len()).unwrap_or(u32::MAX);
        let width_px = parley_metrics.width_px;
        let height_px = parley_metrics.height_px;
        let baseline_px = parley_metrics.baseline_px;
        let _bidi_info = BidiInfo::new(&display_text, None);

        let mut flags = 0_u8;
        if contains_emoji {
            flags |= TEXT_LAYOUT_CONTAINS_EMOJI;
        }
        if bidi_resolved {
            flags |= TEXT_LAYOUT_BIDI_RESOLVED;
        }
        flags |= TEXT_LAYOUT_PARLEY_PROCESSED;
        if !matches!(input.truncation, TruncationMode::None) {
            flags |= TEXT_LAYOUT_TRUNCATED;
        }

        Ok(TextLayout {
            resolved_family,
            display_text,
            lines,
            width_px: round_tenth(width_px),
            height_px: round_tenth(height_px),
            baseline_px: round_tenth(baseline_px),
            line_count,
            cluster_count,
            flags,
        })
    }

    /// Generates a glyph cache key for a layout input.
    ///
    /// # Errors
    ///
    /// Returns [`TextBackendError`] when the font family cannot be resolved.
    pub fn glyph_cache_key(
        &self,
        input: &TextLayoutInput,
    ) -> Result<GlyphCacheKey, TextBackendError> {
        validate_input(input)?;
        let resolved_family = self.resolve_family(&input.font_family)?;
        Ok(GlyphCacheKey {
            stable_key: format!(
                "text={}|font={resolved_family}|size={}|dpi={}|line-break={}|truncation={}|bidi={}|font-generation={}",
                input.text,
                input.size_px,
                input.dpi_scale,
                line_break_key(input.line_break),
                truncation_key(input.truncation),
                input.bidi,
                self.catalog.generation()
            ),
        })
    }

    fn layout_with_parley(
        &self,
        input: &TextLayoutInput,
        resolved_family: &str,
        display_text: &str,
    ) -> Result<ParleyLayoutMetrics, TextBackendError> {
        let mut font_context = self.parley_font_context.lock().map_err(|_| {
            TextBackendError::new(
                "text.parley.font-context-poisoned",
                "Parley font context lock was poisoned",
            )
        })?;
        let mut layout_context = self.parley_layout_context.lock().map_err(|_| {
            TextBackendError::new(
                "text.parley.layout-context-poisoned",
                "Parley layout context lock was poisoned",
            )
        })?;
        let mut builder =
            layout_context.ranged_builder(&mut font_context, display_text, input.dpi_scale, true);
        builder.push_default(StyleProperty::FontStack(FontStack::from(resolved_family)));
        builder.push_default(StyleProperty::FontSize(input.size_px));
        let mut layout = builder.build(display_text);
        layout.break_all_lines(parley_max_advance(input));

        if !layout.width().is_finite() || !layout.height().is_finite() {
            return Err(TextBackendError::new(
                "text.parley.invalid-metrics",
                "Parley produced non-finite text layout metrics",
            ));
        }

        let mut lines = Vec::new();
        let mut baseline_px = None;
        for line in layout.lines() {
            let metrics = line.metrics();
            if !metrics.advance.is_finite() || !metrics.baseline.is_finite() {
                return Err(TextBackendError::new(
                    "text.parley.invalid-line-metrics",
                    "Parley produced non-finite line metrics",
                ));
            }
            let range = line.text_range();
            let text = display_text
                .get(range)
                .ok_or_else(|| {
                    TextBackendError::new(
                        "text.parley.invalid-line-range",
                        "Parley produced a line range outside the source text",
                    )
                })?
                .to_string();
            let baseline = round_tenth(metrics.baseline);
            baseline_px.get_or_insert(baseline);
            lines.push(TextLayoutLine {
                text,
                width_px: round_tenth(metrics.advance - metrics.trailing_whitespace),
                baseline_px: baseline,
            });
        }

        if lines.is_empty() {
            return Err(TextBackendError::new(
                "text.parley.empty-layout",
                "Parley produced no text layout lines",
            ));
        }

        Ok(ParleyLayoutMetrics {
            lines,
            width_px: round_tenth(layout.width()),
            height_px: round_tenth(layout.height()),
            baseline_px: baseline_px.unwrap_or(0.0),
        })
    }

    fn truncate_text(
        &self,
        input: &TextLayoutInput,
        resolved_family: &str,
    ) -> Result<String, TextBackendError> {
        let TruncationMode::EndEllipsis { max_width_px } = input.truncation else {
            return Ok(input.text.clone());
        };
        let clusters: Vec<&str> = input.text.graphemes(true).collect();
        let ellipsis = "…";
        if clusters.is_empty() {
            return Ok(ellipsis.to_string());
        }
        let mut low = 0_usize;
        let mut high = clusters.len();
        while low < high {
            let mid = (low + high).div_ceil(2);
            let candidate = truncated_candidate(&clusters[..mid], ellipsis);
            if self.measure_text_width(input, resolved_family, &candidate)? <= max_width_px {
                low = mid;
            } else {
                high = mid - 1;
            }
        }
        if low == 0 {
            return Ok(ellipsis.to_string());
        }
        Ok(truncated_candidate(&clusters[..low], ellipsis))
    }

    fn measure_text_width(
        &self,
        input: &TextLayoutInput,
        resolved_family: &str,
        text: &str,
    ) -> Result<f32, TextBackendError> {
        let metrics = self.layout_with_parley(input, resolved_family, text)?;
        Ok(metrics.width_px)
    }
}

fn parley_max_advance(input: &TextLayoutInput) -> Option<f32> {
    match input.line_break {
        LineBreakMode::None => None,
        LineBreakMode::Wrap { max_width_px } => Some(max_width_px * input.dpi_scale),
    }
}

fn validate_input(input: &TextLayoutInput) -> Result<(), TextBackendError> {
    if input.text.is_empty() {
        return Err(TextBackendError::new(
            "text.input.empty",
            "text must not be empty",
        ));
    }
    if input.text.len() > TEXT_INPUT_MAX_BYTES {
        return Err(TextBackendError::new(
            "text.input.too-large",
            format!("text input must not exceed {TEXT_INPUT_MAX_BYTES} bytes"),
        ));
    }
    if !is_valid_font_family(&input.font_family) {
        return Err(TextBackendError::new(
            "text.input.invalid-font-family",
            "font family must not be empty",
        ));
    }
    if !input.size_px.is_finite() || input.size_px <= 0.0 {
        return Err(TextBackendError::new(
            "text.input.invalid-size",
            "font size must be finite and greater than zero",
        ));
    }
    if !input.dpi_scale.is_finite() || input.dpi_scale <= 0.0 {
        return Err(TextBackendError::new(
            "text.input.invalid-dpi",
            "DPI scale must be finite and greater than zero",
        ));
    }
    if let LineBreakMode::Wrap { max_width_px } = input.line_break
        && (!max_width_px.is_finite() || max_width_px <= 0.0)
    {
        return Err(TextBackendError::new(
            "text.input.invalid-wrap-width",
            "wrap width must be finite and greater than zero",
        ));
    }
    if let TruncationMode::EndEllipsis { max_width_px } = input.truncation
        && (!max_width_px.is_finite() || max_width_px <= 0.0)
    {
        return Err(TextBackendError::new(
            "text.input.invalid-truncation-width",
            "truncation width must be finite and greater than zero",
        ));
    }
    Ok(())
}

fn truncated_candidate(clusters: &[&str], ellipsis: &str) -> String {
    let mut display = clusters.concat();
    display.push_str(ellipsis);
    display
}

fn is_emoji(character: char) -> bool {
    ('\u{1F000}'..='\u{1FAFF}').contains(&character)
}

fn is_rtl(character: char) -> bool {
    ('\u{0590}'..='\u{08FF}').contains(&character)
}

fn line_break_key(line_break: LineBreakMode) -> String {
    match line_break {
        LineBreakMode::None => "none".to_string(),
        LineBreakMode::Wrap { max_width_px } => format!("wrap:{max_width_px}"),
    }
}

fn truncation_key(truncation: TruncationMode) -> String {
    match truncation {
        TruncationMode::None => "none".to_string(),
        TruncationMode::EndEllipsis { max_width_px } => format!("end-ellipsis:{max_width_px}"),
    }
}

fn round_tenth(value: f32) -> f32 {
    (value * 10.0).round() / 10.0
}

fn is_valid_font_family(family: &str) -> bool {
    !family.trim().is_empty()
}

fn database_contains_family(database: &fontdb::Database, family: &str) -> bool {
    database
        .query(&fontdb::Query {
            families: &[fontdb::Family::Name(family)],
            ..fontdb::Query::default()
        })
        .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_identity() {
        assert_eq!(crate_name(), "hawk2ui-text");
    }
}

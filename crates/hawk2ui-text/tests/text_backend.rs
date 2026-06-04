use hawk2ui_text::{FontCatalog, LineBreakMode, TextBackend, TextLayoutInput, TruncationMode};

const TUFFY_TTF: &[u8] = include_bytes!("fixtures/Tuffy.ttf");

#[test]
fn text_backend_discovers_app_fonts_and_chooses_fallbacks() {
    let catalog = FontCatalog::new()
        .with_system_family("Atkinson")
        .with_fallback_family("EmojiFallback");
    let backend = TextBackend::new(catalog);

    assert_eq!(backend.resolve_family("Atkinson").unwrap(), "Atkinson");
    assert_eq!(backend.resolve_family("Missing").unwrap(), "EmojiFallback");
    assert!(backend.font_generation() > 0);
}

#[test]
fn text_backend_rejects_app_font_metadata_when_fontdb_loads_no_faces() {
    let backend = TextBackend::new(
        FontCatalog::new()
            .with_app_font(
                "Display",
                "assets/fonts/display.ttf",
                b"not-a-font".to_vec(),
            )
            .with_fallback_family("EmojiFallback"),
    );

    assert!(backend.catalog().app_font_sources().is_empty());
    assert_eq!(backend.resolve_family("Display").unwrap(), "EmojiFallback");
}

#[test]
fn text_backend_shapes_with_registered_app_font_bytes() {
    let backend = TextBackend::new(
        FontCatalog::new()
            .with_app_font("AppDisplay", "assets/fonts/Tuffy.ttf", TUFFY_TTF.to_vec())
            .with_system_family("sans-serif"),
    );
    let app_font_input = TextLayoutInput::new("mmmmmmmm", "AppDisplay", 22.0);
    let system_font_input = TextLayoutInput::new("mmmmmmmm", "sans-serif", 22.0);

    let app_font_layout = backend
        .layout(&app_font_input)
        .expect("app font layout succeeds");
    let system_font_layout = backend
        .layout(&system_font_input)
        .expect("system font layout succeeds");

    assert_eq!(app_font_layout.resolved_family(), "AppDisplay");
    assert!(
        (app_font_layout.width_px() - system_font_layout.width_px()).abs() > 0.1,
        "registered app font bytes must affect Parley shaping metrics"
    );
}

#[test]
fn text_backend_shapes_latin_emoji_combining_and_bidi_text() {
    let backend = TextBackend::new(
        FontCatalog::new()
            .with_system_family("Display")
            .with_fallback_family("EmojiFallback"),
    );
    let input = TextLayoutInput::new("Cafe\u{301} 🚀 שלום", "Display", 18.0)
        .with_dpi_scale(2.0)
        .with_bidi(true);

    let layout = backend.layout(&input).expect("layout succeeds");

    assert_eq!(layout.resolved_family(), "Display");
    assert!(layout.cluster_count() < input.text().chars().count());
    assert!(layout.contains_emoji());
    assert!(layout.bidi_resolved());
    assert!(layout.parley_processed());
    assert_eq!(layout.line_count(), 1);
    assert!(layout.baseline_px() > 0.0);
}

#[test]
fn text_backend_reports_bidi_for_presentation_form_characters() {
    let backend = TextBackend::new(FontCatalog::new().with_system_family("Display"));
    let input = TextLayoutInput::new("Gain \u{FE91}", "Display", 18.0).with_bidi(true);

    let layout = backend.layout(&input).expect("layout succeeds");

    assert!(
        layout.bidi_resolved(),
        "Arabic presentation-form characters must be reported as bidi text"
    );
}

#[test]
fn text_backend_reports_common_bmp_emoji_clusters() {
    let backend = TextBackend::new(FontCatalog::new().with_system_family("Display"));
    let input = TextLayoutInput::new("Favorite \u{2764}", "Display", 18.0);

    let layout = backend.layout(&input).expect("layout succeeds");

    assert!(
        layout.contains_emoji(),
        "BMP dingbat emoji must be reported as emoji clusters"
    );
}

#[test]
fn text_backend_wraps_truncates_and_scales_high_dpi_metrics() {
    let backend = TextBackend::new(FontCatalog::new().with_system_family("Display"));
    let wrapped = TextLayoutInput::new("Gain reduction meter readout", "Display", 16.0)
        .with_dpi_scale(1.0)
        .with_line_break(LineBreakMode::Wrap { max_width_px: 96.0 });
    let high_dpi = wrapped.clone().with_dpi_scale(2.0);
    let truncated = wrapped
        .clone()
        .with_truncation(TruncationMode::EndEllipsis { max_width_px: 72.0 });

    let wrapped_layout = backend.layout(&wrapped).expect("wrapped layout succeeds");
    let high_dpi_layout = backend.layout(&high_dpi).expect("high dpi layout succeeds");
    let truncated_layout = backend
        .layout(&truncated)
        .expect("truncated layout succeeds");

    assert!(wrapped_layout.line_count() > 1);
    assert_eq!(
        wrapped_layout.lines().len(),
        usize::try_from(wrapped_layout.line_count()).unwrap()
    );
    assert_close(
        wrapped_layout.lines()[0].baseline_px(),
        wrapped_layout.baseline_px(),
    );
    assert!(wrapped_layout.lines()[1].baseline_px() > wrapped_layout.lines()[0].baseline_px());
    assert!(
        wrapped_layout
            .lines()
            .iter()
            .all(|line| line.width_px() <= 96.0)
    );
    assert!(high_dpi_layout.width_px() > wrapped_layout.width_px());
    assert!(high_dpi_layout.baseline_px() > wrapped_layout.baseline_px());
    assert!(truncated_layout.truncated());
    assert!(truncated_layout.display_text().ends_with('…'));
    assert!(truncated_layout.width_px() <= 72.0);
    assert_eq!(
        truncated_layout.lines()[0].text(),
        truncated_layout.display_text()
    );
}

fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < f32::EPSILON,
        "expected {actual} to equal {expected}"
    );
}

#[test]
fn text_backend_generates_stable_glyph_cache_invalidation_keys() {
    let backend = TextBackend::new(FontCatalog::new().with_system_family("Display"));
    let input = TextLayoutInput::new("Amount", "Display", 18.0).with_dpi_scale(2.0);
    let different_dpi = input.clone().with_dpi_scale(1.0);

    let key = backend.glyph_cache_key(&input).expect("cache key succeeds");
    let dpi_key = backend
        .glyph_cache_key(&different_dpi)
        .expect("dpi cache key succeeds");
    let wrapped_key = backend
        .glyph_cache_key(&input.clone().with_line_break(LineBreakMode::Wrap {
            max_width_px: 120.0,
        }))
        .expect("wrapped cache key succeeds");
    let truncated_key = backend
        .glyph_cache_key(&input.clone().with_truncation(TruncationMode::EndEllipsis {
            max_width_px: 120.0,
        }))
        .expect("truncated cache key succeeds");

    assert!(key.stable_key().contains("font=Display"));
    assert!(key.stable_key().contains("dpi=2"));
    assert!(wrapped_key.stable_key().contains("line-break=wrap:120"));
    assert!(
        truncated_key
            .stable_key()
            .contains("truncation=end-ellipsis:120")
    );
    assert_ne!(key, dpi_key);
    assert_ne!(key, wrapped_key);
    assert_ne!(key, truncated_key);
}

#[test]
fn text_backend_rejects_invalid_layout_and_cache_inputs() {
    let backend = TextBackend::new(FontCatalog::new().with_system_family("Display"));

    let error = backend
        .layout(&TextLayoutInput::new("Amount", "", 18.0))
        .expect_err("empty font families must fail");
    assert_eq!(error.diagnostic().rule(), "text.input.invalid-font-family");

    let error = backend
        .layout(
            &TextLayoutInput::new("Amount", "Display", 18.0)
                .with_line_break(LineBreakMode::Wrap { max_width_px: 0.0 }),
        )
        .expect_err("non-positive wrap widths must fail");
    assert_eq!(error.diagnostic().rule(), "text.input.invalid-wrap-width");

    let error = backend
        .layout(
            &TextLayoutInput::new("Amount", "Display", 18.0).with_truncation(
                TruncationMode::EndEllipsis {
                    max_width_px: f32::NAN,
                },
            ),
        )
        .expect_err("non-finite truncation widths must fail");
    assert_eq!(
        error.diagnostic().rule(),
        "text.input.invalid-truncation-width"
    );

    let error = backend
        .glyph_cache_key(&TextLayoutInput::new("Amount", "Display", 18.0).with_dpi_scale(0.0))
        .expect_err("cache keys must reject invalid DPI scales");
    assert_eq!(error.diagnostic().rule(), "text.input.invalid-dpi");

    let error = backend
        .layout(&TextLayoutInput::new(
            "x".repeat(256 * 1024 + 1),
            "Display",
            18.0,
        ))
        .expect_err("oversized text input must fail before shaping");
    assert_eq!(error.diagnostic().rule(), "text.input.too-large");
}

#[test]
fn text_backend_rejects_invalid_font_catalog_entries() {
    let backend = TextBackend::new(
        FontCatalog::new()
            .with_system_family("")
            .with_app_font("Display", "", Vec::new())
            .with_fallback_family(""),
    );

    let error = backend
        .resolve_family("Missing")
        .expect_err("invalid catalog entries must not resolve");
    assert_eq!(error.diagnostic().rule(), "text.font.missing");
}

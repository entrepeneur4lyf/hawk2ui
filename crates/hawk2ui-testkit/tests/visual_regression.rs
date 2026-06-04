use hawk2ui_layout::{FlexDirection, LayoutSizing, LayoutStyle, Viewport};
use hawk2ui_render::{
    Color, CustomDrawSurface, CustomSurfaceCapability, CustomSurfaceCategory,
    CustomSurfaceDataSnapshot, CustomSurfaceDrawRequest, CustomSurfaceFrameContext, Geometry,
    RendererBackend,
};
use hawk2ui_render_skia::SkiaRendererBackend;
use hawk2ui_runtime::{
    RuntimeSceneBridge, RuntimeViewId, RuntimeViewNode, RuntimeViewTree, RuntimeVisual,
};
use hawk2ui_testkit::{
    ImageComparisonMetadata, ImagePixelFormat, VisualFixtureKind, VisualFixtureSet,
    VisualImageSnapshot, VisualRegressionError, VisualRegressionSuite,
};
use std::fs;

#[test]
fn visual_regression_covers_required_fixture_families() {
    let fixtures = VisualFixtureSet::production_baseline();

    for kind in [
        VisualFixtureKind::Text,
        VisualFixtureKind::Shape,
        VisualFixtureKind::Gradient,
        VisualFixtureKind::ImageLayer,
        VisualFixtureKind::VectorAsset,
        VisualFixtureKind::CustomControl,
        VisualFixtureKind::GraphSurface,
        VisualFixtureKind::DpiScaling,
        VisualFixtureKind::PremiumDesktopTemplate,
        VisualFixtureKind::PremiumPluginTemplate,
    ] {
        assert!(fixtures.fixture(kind).is_some(), "missing {kind:?}");
    }
}

#[test]
fn visual_regression_records_snapshot_and_image_comparison_metadata() {
    let baseline = VisualImageSnapshot::from_pixels("card", 2, 1, vec![0x0010_2030, 0x0040_5060])
        .expect("baseline validates");
    let candidate = baseline.clone();
    let comparison = ImageComparisonMetadata::strict_rgba8();
    let suite = VisualRegressionSuite::new().with_image_case("card-baseline", baseline, candidate);
    let report = suite.evaluate_images(&comparison);

    assert!(report.accepted());
    assert!(comparison.accepts(0, 0));
    assert!(!comparison.accepts(1, 0));
    assert_eq!(comparison.pixel_format(), ImagePixelFormat::Rgb8Srgb);
}

#[test]
fn visual_regression_rejects_invalid_image_snapshot_dimensions() {
    assert_eq!(
        VisualImageSnapshot::from_pixels("bad", 2, 2, vec![0x0000_0000]),
        Err(VisualRegressionError::PixelCountMismatch {
            expected: 4,
            actual: 1,
        })
    );
}

#[test]
fn visual_regression_renders_runtime_scene_with_skia_and_reports_pixel_diff_artifacts() {
    let tree = RuntimeViewTree::new(RuntimeViewNode::new(
        RuntimeViewId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(96.0, 64.0)),
        RuntimeVisual::Fill(Color::rgba(8, 10, 14, 255)),
    ))
    .with_child(
        &RuntimeViewId::new("root"),
        RuntimeViewNode::new(
            RuntimeViewId::new("accent"),
            LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(32.0, 16.0)),
            RuntimeVisual::Fill(Color::rgba(30, 144, 255, 255)),
        ),
    )
    .expect("accent child attaches");
    let frame = RuntimeSceneBridge::new(Viewport::new(96.0, 64.0))
        .build(&tree)
        .expect("runtime scene frame builds");

    let mut backend = SkiaRendererBackend::new();
    backend
        .create_surface("main", 96, 64)
        .expect("surface creates");
    backend.begin_frame("main").expect("frame begins");
    backend
        .draw_runtime_scene_frame(&frame, 0, 1.0)
        .expect("runtime scene renders");
    backend.end_frame("main").expect("frame presents");

    let snapshot = backend.frame_snapshot("main").expect("snapshot exists");
    let baseline = VisualImageSnapshot::from_pixels(
        "runtime-card",
        snapshot.width(),
        snapshot.height(),
        snapshot.pixels().to_vec(),
    )
    .expect("baseline snapshot validates");
    let mut candidate_pixels = baseline.pixels().to_vec();
    candidate_pixels[0] = 0x0000_ff00;
    let candidate = VisualImageSnapshot::from_pixels(
        "runtime-card",
        snapshot.width(),
        snapshot.height(),
        candidate_pixels,
    )
    .expect("candidate snapshot validates");

    let comparison = ImageComparisonMetadata::strict_rgba8();
    let report = comparison.compare(&baseline, &candidate);
    assert_eq!(report.changed_pixels(), 1);
    assert_eq!(report.max_pixel_delta(), 255);
    assert!(!report.accepted());
    assert!(
        report
            .artifact_payload()
            .contains("case = \"runtime-card\"")
    );
    assert!(report.artifact_payload().contains("changed_pixels = 1"));

    let suite =
        VisualRegressionSuite::new().with_image_case("runtime-card", baseline.clone(), candidate);
    let suite_report = suite.evaluate_images(&comparison);
    assert_eq!(suite_report.case_count(), 1);
    assert_eq!(suite_report.failed_count(), 1);
    assert!(!suite_report.accepted());
    assert!(suite_report.artifact_payload().contains("[[cases]]"));

    let matching_suite = VisualRegressionSuite::new().with_image_case(
        "runtime-card-match",
        suite.image_cases()[0].baseline().clone(),
        suite.image_cases()[0].baseline().clone(),
    );
    assert!(matching_suite.evaluate_images(&comparison).accepted());

    assert!(
        baseline.count_changed_pixels(0x0008_0a0e, Geometry::new(0.0, 0.0, 96.0, 64.0)) > 0,
        "runtime scene must render visible pixels against the expected background"
    );
}

#[test]
fn visual_regression_renders_premium_desktop_and_plugin_surface_primitives() {
    let mut desktop = SkiaRendererBackend::new();
    desktop
        .create_surface("desktop-premium", 1280, 720)
        .expect("desktop premium surface creates");
    register_premium_assets(&mut desktop);
    desktop
        .begin_frame("desktop-premium")
        .expect("desktop premium frame begins");
    draw_premium_desktop_template(&mut desktop).expect("desktop premium template renders");
    desktop
        .end_frame("desktop-premium")
        .expect("desktop premium frame presents");

    let desktop_snapshot = desktop
        .frame_snapshot("desktop-premium")
        .expect("desktop premium snapshot exists");
    let desktop_baseline = VisualImageSnapshot::from_pixels(
        "premium-desktop-template",
        desktop_snapshot.width(),
        desktop_snapshot.height(),
        desktop_snapshot.pixels().to_vec(),
    )
    .expect("desktop premium baseline validates");

    assert!(
        desktop_baseline.count_changed_pixels(0x0007_090d, Geometry::new(0.0, 0.0, 1280.0, 720.0))
            > 240_000,
        "desktop premium template must fill the surface with visible layered composition"
    );
    assert!(
        desktop_baseline.count_changed_pixels(0x000f_1722, Geometry::new(72.0, 82.0, 520.0, 312.0))
            > 12_000,
        "desktop premium hero card must include text, image/vector, shadow, glow, and gradient pixels"
    );

    let mut plugin = SkiaRendererBackend::new();
    plugin
        .create_surface("plugin-premium", 960, 540)
        .expect("plugin premium surface creates");
    plugin
        .begin_frame("plugin-premium")
        .expect("plugin premium frame begins");
    draw_premium_plugin_template(&mut plugin).expect("plugin premium template renders");
    plugin
        .end_frame("plugin-premium")
        .expect("plugin premium frame presents");

    let plugin_snapshot = plugin
        .frame_snapshot("plugin-premium")
        .expect("plugin premium snapshot exists");
    let plugin_baseline = VisualImageSnapshot::from_pixels(
        "premium-plugin-template",
        plugin_snapshot.width(),
        plugin_snapshot.height(),
        plugin_snapshot.pixels().to_vec(),
    )
    .expect("plugin premium baseline validates");

    assert!(
        plugin_baseline.count_changed_pixels(0x0006_070a, Geometry::new(0.0, 0.0, 960.0, 540.0))
            > 120_000,
        "plugin premium template must render a full editor surface"
    );
    assert!(
        plugin_baseline.count_changed_pixels(0x0011_1827, Geometry::new(420.0, 96.0, 420.0, 250.0))
            > 8_000,
        "plugin premium template must render analyzers, meters, knobs, and dense control panels"
    );
}

fn register_premium_assets(backend: &mut SkiaRendererBackend) {
    backend
        .register_image_asset("premium-hero", ONE_BY_ONE_PNG)
        .expect("premium image asset registers");
    backend
        .register_vector_paths(
            "premium-mark",
            [
                "M8 36 L24 8 L40 36 L52 18 L64 36",
                "M12 44 L60 44",
                "M32 20 L40 20 L40 28 L32 28 Z",
            ],
        )
        .expect("premium vector asset registers");
}

fn draw_premium_desktop_template(
    backend: &mut SkiaRendererBackend,
) -> Result<(), hawk2ui_render::BackendError> {
    backend.clear(Color::rgba(7, 9, 13, 255))?;
    backend.draw_linear_gradient(
        Geometry::new(0.0, 0.0, 1280.0, 720.0),
        Color::rgba(8, 12, 18, 255),
        Color::rgba(13, 35, 48, 255),
    )?;
    backend.draw_glow_rect(
        Geometry::new(64.0, 72.0, 560.0, 344.0),
        28.0,
        Color::rgba(70, 200, 255, 92),
    )?;
    backend.draw_shadow_rect(
        Geometry::new(72.0, 82.0, 520.0, 312.0),
        18.0,
        24.0,
        22.0,
        Color::rgba(0, 0, 0, 180),
    )?;
    backend.draw_rounded_rect(
        Geometry::new(72.0, 82.0, 520.0, 312.0),
        34.0,
        Color::rgba(15, 23, 34, 255),
    )?;
    backend.draw_linear_gradient(
        Geometry::new(72.0, 82.0, 520.0, 312.0),
        Color::rgba(20, 35, 56, 255),
        Color::rgba(5, 117, 130, 255),
    )?;
    backend.draw_image_rect("premium-hero", Geometry::new(116.0, 126.0, 150.0, 150.0))?;
    backend.draw_vector_rect("premium-mark", Geometry::new(302.0, 132.0, 156.0, 104.0))?;
    backend.draw_text_at(
        "Premium native dashboard",
        30.0,
        116.0,
        334.0,
        Color::rgba(248, 250, 252, 255),
    )?;
    backend.draw_text_at(
        "Layered Skia rendering without a webview",
        18.0,
        116.0,
        366.0,
        Color::rgba(214, 226, 240, 255),
    )?;

    for (index, x) in [680.0, 820.0, 960.0, 1100.0].into_iter().enumerate() {
        backend.draw_shadow_rect(
            Geometry::new(x, 128.0, 96.0, 160.0),
            10.0,
            14.0,
            14.0,
            Color::rgba(0, 0, 0, 150),
        )?;
        backend.draw_rounded_rect(
            Geometry::new(x, 128.0, 96.0, 160.0),
            22.0,
            Color::rgba(17, 24, 39, 255),
        )?;
        backend.draw_text_at(
            ["Gain", "Tone", "Drive", "Mix"][index],
            16.0,
            x + 18.0,
            264.0,
            Color::rgba(226, 232, 240, 255),
        )?;
    }

    draw_custom_surface(
        backend,
        "desktop-meter",
        CustomSurfaceCategory::Meter,
        Geometry::new(680.0, 342.0, 420.0, 52.0),
        [0.1, 0.35, 0.62, 0.92],
    )?;
    draw_custom_surface(
        backend,
        "desktop-curve",
        CustomSurfaceCategory::EqCurve,
        Geometry::new(680.0, 432.0, 440.0, 160.0),
        [0.15, 0.72, 0.38, 0.86, 0.44, 0.64],
    )
}

fn draw_premium_plugin_template(
    backend: &mut SkiaRendererBackend,
) -> Result<(), hawk2ui_render::BackendError> {
    backend.clear(Color::rgba(6, 7, 10, 255))?;
    backend.draw_linear_gradient(
        Geometry::new(0.0, 0.0, 960.0, 540.0),
        Color::rgba(6, 7, 10, 255),
        Color::rgba(30, 24, 48, 255),
    )?;
    backend.draw_glow_rect(
        Geometry::new(72.0, 76.0, 256.0, 256.0),
        34.0,
        Color::rgba(255, 138, 76, 90),
    )?;
    backend.draw_rounded_rect(
        Geometry::new(56.0, 60.0, 300.0, 312.0),
        38.0,
        Color::rgba(19, 20, 26, 255),
    )?;
    backend.draw_text_at(
        "Nebula-class editor",
        28.0,
        84.0,
        116.0,
        Color::rgba(255, 247, 237, 255),
    )?;
    backend.draw_text_at(
        "Host-safe CLAP UI surface",
        17.0,
        86.0,
        150.0,
        Color::rgba(252, 211, 177, 255),
    )?;

    for (index, (x, value)) in [(88.0, 0.24), (184.0, 0.46), (280.0, 0.68)]
        .into_iter()
        .enumerate()
    {
        draw_custom_surface(
            backend,
            ["knob-time", "knob-feedback", "knob-width"][index],
            CustomSurfaceCategory::Knob,
            Geometry::new(x, 204.0, 60.0, 60.0),
            [value, 0.9],
        )?;
    }

    backend.draw_shadow_rect(
        Geometry::new(420.0, 96.0, 420.0, 250.0),
        18.0,
        22.0,
        20.0,
        Color::rgba(0, 0, 0, 170),
    )?;
    backend.draw_rounded_rect(
        Geometry::new(420.0, 96.0, 420.0, 250.0),
        28.0,
        Color::rgba(17, 24, 39, 255),
    )?;
    draw_custom_surface(
        backend,
        "plugin-analyzer",
        CustomSurfaceCategory::Analyzer,
        Geometry::new(452.0, 130.0, 356.0, 96.0),
        [0.12, 0.44, 0.7, 0.95, 0.58, 0.32, 0.74],
    )?;
    draw_custom_surface(
        backend,
        "plugin-scope",
        CustomSurfaceCategory::Scope,
        Geometry::new(452.0, 246.0, 356.0, 70.0),
        [-0.6, -0.1, 0.35, 0.75, 0.2, -0.25, -0.55],
    )?;
    backend.draw_text_at(
        "Realtime visuals",
        18.0,
        452.0,
        334.0,
        Color::rgba(226, 232, 240, 255),
    )
}

fn draw_custom_surface<const N: usize>(
    backend: &mut SkiaRendererBackend,
    id: &str,
    category: CustomSurfaceCategory,
    geometry: Geometry,
    samples: [f32; N],
) -> Result<(), hawk2ui_render::BackendError> {
    let surface = CustomDrawSurface::new(id, category, geometry)
        .with_capability(CustomSurfaceCapability::RealtimeData)
        .invalidate();
    let context = CustomSurfaceFrameContext::new(1, 1.0).expect("custom frame context is valid");
    let data = CustomSurfaceDataSnapshot::new(samples).expect("custom surface samples are valid");
    let request = CustomSurfaceDrawRequest::new(surface, context, data)
        .expect("custom draw request is valid");
    backend.draw_custom_surface(&request)
}

#[test]
fn visual_regression_writes_report_baseline_candidate_and_diff_artifacts() {
    let root =
        std::env::temp_dir().join(format!("hawk2ui-visual-artifacts-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);

    let baseline =
        VisualImageSnapshot::from_pixels("bad/name", 2, 1, vec![0x0000_0000, 0x0011_2233])
            .expect("baseline snapshot validates");
    let candidate =
        VisualImageSnapshot::from_pixels("bad/name", 2, 1, vec![0x0000_0000, 0x0033_2211])
            .expect("candidate snapshot validates");
    let suite = VisualRegressionSuite::new().with_image_case("bad/name", baseline, candidate);

    let artifacts = suite
        .write_image_artifacts(&ImageComparisonMetadata::strict_rgba8(), &root)
        .expect("visual artifacts write");

    assert_eq!(artifacts.root(), root.as_path());
    assert!(artifacts.contains_file_name("visual-report.toml"));
    assert!(artifacts.contains_file_name("bad-name-baseline.ppm"));
    assert!(artifacts.contains_file_name("bad-name-candidate.ppm"));
    assert!(artifacts.contains_file_name("bad-name-diff.ppm"));

    let report = fs::read_to_string(root.join("visual-report.toml")).expect("visual report reads");
    assert!(report.contains("failed_count = 1"));
    assert!(report.contains("changed_pixels = 1"));

    let diff = fs::read_to_string(root.join("bad-name-diff.ppm")).expect("diff artifact reads");
    assert!(diff.starts_with("P3\n2 1\n255\n"));
    assert!(
        diff.lines().any(|line| line != "0 0 0"),
        "diff artifact must contain a visible changed pixel"
    );

    fs::remove_dir_all(root).expect("visual artifact temp directory cleans up");
}

const ONE_BY_ONE_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 15, 4, 0, 9, 251, 3,
    253, 167, 175, 213, 63, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

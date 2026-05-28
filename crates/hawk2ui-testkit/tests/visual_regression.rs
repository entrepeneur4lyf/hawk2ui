use hawk2ui_layout::{FlexDirection, LayoutSizing, LayoutStyle, Viewport};
use hawk2ui_render::{Color, Geometry, RendererBackend};
use hawk2ui_render_skia::SkiaRendererBackend;
use hawk2ui_runtime::{
    RuntimeSceneBridge, RuntimeViewId, RuntimeViewNode, RuntimeViewTree, RuntimeVisual,
};
use hawk2ui_testkit::{
    ImageComparisonMetadata, VisualFixtureKind, VisualFixtureSet, VisualImageSnapshot,
    VisualRegressionCase, VisualRegressionSuite, VisualSnapshot,
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
    ] {
        assert!(fixtures.fixture(kind).is_some(), "missing {kind:?}");
    }
}

#[test]
fn visual_regression_records_snapshot_and_image_comparison_metadata() {
    let baseline = VisualSnapshot::new("card", 640, 360)
        .with_command("draw-rounded-rect:background:12")
        .with_command("draw-text:title:Amount");
    let candidate = baseline.clone();
    let comparison = ImageComparisonMetadata::strict_rgba8();
    let suite = VisualRegressionSuite::new().with_case(VisualRegressionCase::new(
        "card-baseline",
        baseline,
        candidate,
    ));

    assert!(suite.all_match());
    assert!(comparison.accepts(0, 0));
    assert!(!comparison.accepts(1, 0));
    assert_eq!(comparison.color_space(), "rgba8-srgb");
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
    candidate_pixels[0] = 0x00ff00;
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
        baseline.count_changed_pixels(0x080a0e, Geometry::new(0.0, 0.0, 96.0, 64.0)) > 0,
        "runtime scene must render visible pixels against the expected background"
    );
}

#[test]
fn visual_regression_writes_report_baseline_candidate_and_diff_artifacts() {
    let root =
        std::env::temp_dir().join(format!("hawk2ui-visual-artifacts-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);

    let baseline = VisualImageSnapshot::from_pixels("bad/name", 2, 1, vec![0x000000, 0x112233])
        .expect("baseline snapshot validates");
    let candidate = VisualImageSnapshot::from_pixels("bad/name", 2, 1, vec![0x000000, 0x332211])
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

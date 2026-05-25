use hawk2ui_testkit::{
    ImageComparisonMetadata, VisualFixtureKind, VisualFixtureSet, VisualRegressionCase,
    VisualRegressionSuite, VisualSnapshot,
};

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

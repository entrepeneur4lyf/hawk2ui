#![allow(clippy::float_cmp, clippy::similar_names)]

use hawk2ui_layout::{
    AnalyzerRegion, BoxEdges, FlexDirection, GeneratedParameterLayout, GraphRegion,
    HawkTextMeasurer, LayoutAlignItems, LayoutGridAutoFlow, LayoutGridLine, LayoutGridStyle,
    LayoutGridTrack, LayoutJustifyContent, LayoutNode, LayoutNodeId, LayoutSizing, LayoutStyle,
    LayoutTextMeasurer, LayoutTree, LayoutValue, PluginEditorConstraints, PluginEditorSize,
    SceneGeometryAttachment, TestTextMeasurer, TextMeasureInput, TextMeasureKey, TextMeasureMode,
    TextMeasureResult, Viewport,
};
use hawk2ui_text::{FontCatalog, TextBackend};

#[test]
fn layout_tree_preserves_parent_child_relationships() {
    let tree = LayoutTree::new(LayoutNode::new(
        LayoutNodeId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Row),
    ))
    .with_child(
        LayoutNodeId::new("root"),
        LayoutNode::new(
            LayoutNodeId::new("panel"),
            LayoutStyle::scroll_container().with_gap(LayoutValue::px(12.0)),
        ),
    )
    .expect("child insertion should succeed")
    .with_child(
        LayoutNodeId::new("panel"),
        LayoutNode::new(
            LayoutNodeId::new("meter"),
            LayoutStyle::custom_measured().with_min_size(LayoutSizing::fixed(96.0, 48.0)),
        ),
    )
    .expect("nested child insertion should succeed");

    assert_eq!(
        tree.parent_of(&LayoutNodeId::new("panel"))
            .unwrap()
            .as_str(),
        "root"
    );
    assert_eq!(
        tree.parent_of(&LayoutNodeId::new("meter"))
            .unwrap()
            .as_str(),
        "panel"
    );
    assert_eq!(
        tree.children_of(&LayoutNodeId::new("root"))[0].as_str(),
        "panel"
    );
}

#[test]
fn layout_tree_converts_style_values_to_layout_records() {
    let style = LayoutStyle::absolute_region()
        .with_size(LayoutSizing::percent(50.0, 25.0))
        .with_min_size(LayoutSizing::fixed(120.0, 80.0))
        .with_max_size(LayoutSizing::fixed(640.0, 360.0))
        .with_margin(BoxEdges::all(LayoutValue::px(8.0)))
        .with_padding(BoxEdges::axis(LayoutValue::px(16.0), LayoutValue::px(12.0)))
        .with_gap(LayoutValue::px(10.0));

    assert_eq!(style.size().width(), LayoutValue::Percent(50.0));
    assert_eq!(style.size().height(), LayoutValue::Percent(25.0));
    assert_eq!(style.min_size().width(), LayoutValue::Px(120.0));
    assert_eq!(style.max_size().height(), LayoutValue::Px(360.0));
    assert_eq!(style.margin().left, LayoutValue::Px(8.0));
    assert_eq!(style.padding().top, LayoutValue::Px(12.0));
    assert_eq!(style.gap(), LayoutValue::Px(10.0));
    assert!(style.absolute());
}

#[test]
fn layout_tree_rejects_invalid_ids_styles_and_viewports() {
    let error = LayoutTree::new(LayoutNode::new(
        LayoutNodeId::new(""),
        LayoutStyle::flex_container(FlexDirection::Row),
    ))
    .validate()
    .expect_err("empty root layout IDs must fail validation");
    assert_eq!(
        error,
        hawk2ui_layout::LayoutTreeError::InvalidNodeId(String::new())
    );

    let error = LayoutTree::new(LayoutNode::new(
        LayoutNodeId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Row),
    ))
    .with_child(
        LayoutNodeId::new("root"),
        LayoutNode::new(
            LayoutNodeId::new(""),
            LayoutStyle::flex_container(FlexDirection::Column),
        ),
    )
    .expect_err("empty child layout IDs must fail insertion");
    assert_eq!(
        error,
        hawk2ui_layout::LayoutTreeError::InvalidNodeId(String::new())
    );

    let tree = LayoutTree::new(LayoutNode::new(
        LayoutNodeId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Row)
            .with_size(LayoutSizing::fixed(f32::NAN, 100.0)),
    ));
    let error = tree
        .try_compute_layout(Viewport::new(300.0, 100.0))
        .expect_err("invalid style values must fail before layout");
    assert_eq!(
        error,
        hawk2ui_layout::LayoutTreeError::InvalidStyle("root".to_string())
    );

    let tree = LayoutTree::new(LayoutNode::new(
        LayoutNodeId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Row),
    ));
    let error = tree
        .try_compute_layout(Viewport::new(0.0, 100.0))
        .expect_err("non-positive viewport sizes must fail");
    assert_eq!(error, hawk2ui_layout::LayoutTreeError::InvalidViewport);

    let tree = LayoutTree::new(LayoutNode::new(
        LayoutNodeId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Row).with_flex_grow(f32::NAN),
    ));
    let error = tree
        .try_compute_layout(Viewport::new(300.0, 100.0))
        .expect_err("invalid flex factors must fail before layout");
    assert_eq!(
        error,
        hawk2ui_layout::LayoutTreeError::InvalidStyle("root".to_string())
    );
}

#[test]
fn flex_scroll_layout_calculates_row_column_gaps_and_padding() {
    let tree = LayoutTree::new(LayoutNode::new(
        LayoutNodeId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Row)
            .with_size(LayoutSizing::fixed(300.0, 100.0))
            .with_padding(BoxEdges::all(LayoutValue::px(10.0)))
            .with_gap(LayoutValue::px(5.0)),
    ))
    .with_child(
        LayoutNodeId::new("root"),
        LayoutNode::new(
            LayoutNodeId::new("left"),
            LayoutStyle::flex_container(FlexDirection::Column)
                .with_size(LayoutSizing::fixed(100.0, 80.0)),
        ),
    )
    .expect("left child insertion should succeed")
    .with_child(
        LayoutNodeId::new("root"),
        LayoutNode::new(
            LayoutNodeId::new("right"),
            LayoutStyle::flex_container(FlexDirection::Column)
                .with_size(LayoutSizing::fixed(100.0, 80.0)),
        ),
    )
    .expect("right child insertion should succeed");

    let output = tree.compute_layout(Viewport::new(300.0, 100.0));

    assert_eq!(output.geometry(&LayoutNodeId::new("left")).unwrap().x, 10.0);
    assert_eq!(
        output.geometry(&LayoutNodeId::new("right")).unwrap().x,
        115.0
    );
    assert_eq!(
        output.geometry(&LayoutNodeId::new("right")).unwrap().height,
        80.0
    );
}

#[test]
fn flex_scroll_layout_tracks_scroll_clip_and_absolute_regions() {
    let tree = LayoutTree::new(LayoutNode::new(
        LayoutNodeId::new("root"),
        LayoutStyle::scroll_container().with_size(LayoutSizing::fixed(200.0, 120.0)),
    ))
    .with_child(
        LayoutNodeId::new("root"),
        LayoutNode::new(
            LayoutNodeId::new("content"),
            LayoutStyle::flex_container(FlexDirection::Column)
                .with_size(LayoutSizing::fixed(200.0, 300.0)),
        ),
    )
    .expect("content child insertion should succeed")
    .with_child(
        LayoutNodeId::new("root"),
        LayoutNode::new(
            LayoutNodeId::new("overlay"),
            LayoutStyle::absolute_region().with_size(LayoutSizing::fixed(40.0, 30.0)),
        ),
    )
    .expect("absolute child insertion should succeed");

    let output = tree.compute_layout(Viewport::new(200.0, 120.0));

    assert_eq!(
        output.geometry(&LayoutNodeId::new("root")).unwrap().width,
        200.0
    );
    assert_eq!(
        output.clip(&LayoutNodeId::new("root")).unwrap().height,
        120.0
    );
    assert_eq!(
        output
            .geometry(&LayoutNodeId::new("content"))
            .unwrap()
            .height,
        300.0
    );
    assert_eq!(
        output.geometry(&LayoutNodeId::new("overlay")).unwrap().x,
        0.0
    );
    assert!(
        output
            .geometry(&LayoutNodeId::new("overlay"))
            .unwrap()
            .absolute
    );
}

#[test]
fn layout_compute_applies_min_and_max_constraints() {
    let tree = LayoutTree::new(LayoutNode::new(
        LayoutNodeId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Row)
            .with_size(LayoutSizing::fixed(500.0, 100.0)),
    ))
    .with_child(
        LayoutNodeId::new("root"),
        LayoutNode::new(
            LayoutNodeId::new("min-bound"),
            LayoutStyle::flex_container(FlexDirection::Column)
                .with_size(LayoutSizing::percent(10.0, 100.0))
                .with_min_size(LayoutSizing::fixed(120.0, 40.0)),
        ),
    )
    .expect("min-bound child insertion should succeed")
    .with_child(
        LayoutNodeId::new("root"),
        LayoutNode::new(
            LayoutNodeId::new("max-bound"),
            LayoutStyle::flex_container(FlexDirection::Column)
                .with_size(LayoutSizing::percent(100.0, 100.0))
                .with_max_size(LayoutSizing::fixed(200.0, 80.0)),
        ),
    )
    .expect("max-bound child insertion should succeed");

    let output = tree.compute_layout(Viewport::new(500.0, 100.0));

    assert_eq!(
        output
            .geometry(&LayoutNodeId::new("min-bound"))
            .unwrap()
            .width,
        120.0
    );
    assert_eq!(
        output
            .geometry(&LayoutNodeId::new("max-bound"))
            .unwrap()
            .width,
        200.0
    );
    assert_eq!(
        output
            .geometry(&LayoutNodeId::new("max-bound"))
            .unwrap()
            .height,
        80.0
    );
}

#[test]
fn layout_compute_supports_flex_growth_alignment_basis_and_absolute_insets() {
    let tree = LayoutTree::new(LayoutNode::new(
        LayoutNodeId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Row)
            .with_size(LayoutSizing::fixed(600.0, 200.0))
            .with_align_items(LayoutAlignItems::Center)
            .with_justify_content(LayoutJustifyContent::Start),
    ))
    .with_child(
        LayoutNodeId::new("root"),
        LayoutNode::new(
            LayoutNodeId::new("left"),
            LayoutStyle::flex_container(FlexDirection::Column)
                .with_size(LayoutSizing::fixed(0.0, 40.0))
                .with_flex_basis(LayoutValue::px(100.0))
                .with_flex_grow(1.0),
        ),
    )
    .expect("left child insertion should succeed")
    .with_child(
        LayoutNodeId::new("root"),
        LayoutNode::new(
            LayoutNodeId::new("right"),
            LayoutStyle::flex_container(FlexDirection::Column)
                .with_size(LayoutSizing::fixed(0.0, 80.0))
                .with_flex_basis(LayoutValue::px(100.0))
                .with_flex_grow(2.0),
        ),
    )
    .expect("right child insertion should succeed")
    .with_child(
        LayoutNodeId::new("root"),
        LayoutNode::new(
            LayoutNodeId::new("overlay"),
            LayoutStyle::absolute_region()
                .with_size(LayoutSizing::fixed(50.0, 40.0))
                .with_inset(BoxEdges {
                    left: LayoutValue::px(24.0),
                    right: LayoutValue::Auto,
                    top: LayoutValue::px(32.0),
                    bottom: LayoutValue::Auto,
                }),
        ),
    )
    .expect("absolute child insertion should succeed");

    let output = tree.compute_layout(Viewport::new(600.0, 200.0));
    let left = output.geometry(&LayoutNodeId::new("left")).unwrap();
    let right = output.geometry(&LayoutNodeId::new("right")).unwrap();
    let overlay = output.geometry(&LayoutNodeId::new("overlay")).unwrap();

    assert_eq!(left.width, 233.0);
    assert_eq!(right.width, 367.0);
    assert_eq!(left.y, 80.0);
    assert_eq!(right.y, 60.0);
    assert_eq!(overlay.x, 24.0);
    assert_eq!(overlay.y, 32.0);

    let centered_tree = LayoutTree::new(LayoutNode::new(
        LayoutNodeId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Row)
            .with_size(LayoutSizing::fixed(300.0, 100.0))
            .with_justify_content(LayoutJustifyContent::Center),
    ))
    .with_child(
        LayoutNodeId::new("root"),
        LayoutNode::new(
            LayoutNodeId::new("centered"),
            LayoutStyle::flex_container(FlexDirection::Column)
                .with_size(LayoutSizing::fixed(100.0, 20.0)),
        ),
    )
    .expect("centered child insertion should succeed");
    let centered = centered_tree.compute_layout(Viewport::new(300.0, 100.0));
    assert_eq!(
        centered.geometry(&LayoutNodeId::new("centered")).unwrap().x,
        100.0
    );
}

#[test]
fn grid_layout_computes_template_tracks_gaps_spans_and_auto_flow() {
    let tree = LayoutTree::new(LayoutNode::new(
        LayoutNodeId::new("root"),
        LayoutStyle::grid_container(
            vec![
                LayoutGridTrack::px(80.0),
                LayoutGridTrack::fr(1.0),
                LayoutGridTrack::px(40.0),
            ],
            vec![LayoutGridTrack::px(30.0), LayoutGridTrack::fr(1.0)],
        )
        .with_grid(
            LayoutGridStyle::new(
                vec![
                    LayoutGridTrack::px(80.0),
                    LayoutGridTrack::fr(1.0),
                    LayoutGridTrack::px(40.0),
                ],
                vec![LayoutGridTrack::px(30.0), LayoutGridTrack::fr(1.0)],
            )
            .with_auto_columns(vec![LayoutGridTrack::MaxContent])
            .with_auto_rows(vec![LayoutGridTrack::MinContent])
            .with_auto_flow(LayoutGridAutoFlow::RowDense),
        )
        .with_size(LayoutSizing::fixed(240.0, 120.0))
        .with_gap(LayoutValue::px(10.0)),
    ))
    .with_child(
        LayoutNodeId::new("root"),
        LayoutNode::new(
            LayoutNodeId::new("header"),
            LayoutStyle::custom_measured()
                .with_grid_column(LayoutGridLine::span(3))
                .with_grid_row(LayoutGridLine::line(1)),
        ),
    )
    .expect("header child insertion should succeed")
    .with_child(
        LayoutNodeId::new("root"),
        LayoutNode::new(
            LayoutNodeId::new("content"),
            LayoutStyle::custom_measured()
                .with_grid_column(LayoutGridLine::line(2))
                .with_grid_row(LayoutGridLine::line(2)),
        ),
    )
    .expect("content child insertion should succeed");

    let output = tree
        .try_compute_layout(Viewport::new(240.0, 120.0))
        .expect("grid layout should compute");
    let header = output.geometry(&LayoutNodeId::new("header")).unwrap();
    let content = output.geometry(&LayoutNodeId::new("content")).unwrap();

    assert_eq!(header.x, 0.0);
    assert_eq!(header.y, 0.0);
    assert_eq!(header.width, 240.0);
    assert_eq!(header.height, 30.0);
    assert_eq!(content.x, 90.0);
    assert_eq!(content.y, 40.0);
    assert_eq!(content.width, 100.0);
    assert_eq!(content.height, 80.0);
}

#[test]
fn text_measurement_reports_intrinsic_wrapped_and_truncated_sizes() {
    let measurer = TestTextMeasurer::new().with_average_glyph_width(8.0);
    let intrinsic = measurer.measure(&TextMeasureInput::new(
        "Hello Hawk",
        "Atkinson",
        16.0,
        TextMeasureMode::Intrinsic,
    ));
    let wrapped = measurer.measure(&TextMeasureInput::new(
        "Hello Hawk",
        "Atkinson",
        16.0,
        TextMeasureMode::Wrap { max_width: 40.0 },
    ));
    let truncated = measurer.measure(&TextMeasureInput::new(
        "Hello Hawk",
        "Atkinson",
        16.0,
        TextMeasureMode::Truncate { max_width: 32.0 },
    ));

    assert_eq!(intrinsic, TextMeasureResult::new(80.0, 19.2, 1, false));
    assert_eq!(wrapped, TextMeasureResult::new(40.0, 38.4, 2, false));
    assert_eq!(truncated, TextMeasureResult::new(32.0, 19.2, 1, true));
}

#[test]
fn text_measurement_keys_invalidate_by_text_and_font_metrics() {
    let base = TextMeasureKey::new("Gain", "Atkinson", 16.0, TextMeasureMode::Intrinsic);
    let text_changed = TextMeasureKey::new("Mix", "Atkinson", 16.0, TextMeasureMode::Intrinsic);
    let font_changed = TextMeasureKey::new("Gain", "Commit Mono", 16.0, TextMeasureMode::Intrinsic);
    let size_changed = TextMeasureKey::new("Gain", "Atkinson", 18.0, TextMeasureMode::Intrinsic);

    assert_ne!(base, text_changed);
    assert_ne!(base, font_changed);
    assert_ne!(base, size_changed);
}

#[test]
fn hawk_text_measurer_uses_production_text_backend_metrics() {
    let backend = TextBackend::new(
        FontCatalog::new()
            .with_system_family("Atkinson")
            .with_fallback_family("Fallback Sans"),
    );
    let measurer = HawkTextMeasurer::new(backend);

    let measured = measurer
        .measure(
            &TextMeasureInput::new(
                "Hawk שלום 🦅",
                "Atkinson",
                16.0,
                TextMeasureMode::Wrap { max_width: 80.0 },
            )
            .with_dpi_scale(2.0),
        )
        .expect("production text backend measures layout input");

    assert!(measured.width <= 160.0);
    assert!(measured.height >= 38.0);
    assert!(measured.line_count >= 1);
}

#[test]
fn layout_tree_feeds_text_measurements_into_taffy() {
    let text_id = LayoutNodeId::new("title");
    let tree = LayoutTree::new(LayoutNode::new(
        LayoutNodeId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(160.0, 96.0)),
    ))
    .with_child(
        LayoutNodeId::new("root"),
        LayoutNode::new(text_id.clone(), LayoutStyle::custom_measured()).with_text_measurement(
            TextMeasureInput::new("Measured", "Atkinson", 16.0, TextMeasureMode::Intrinsic),
        ),
    )
    .expect("text node attaches");

    let output = tree
        .try_compute_layout_with_text_measurer(
            Viewport::new(160.0, 96.0),
            &TestTextMeasurer::new().with_average_glyph_width(8.0),
        )
        .expect("layout computes with text measure function");
    let geometry = output.geometry(&text_id).expect("text geometry exists");

    assert_eq!(geometry.width, 160.0);
    assert_eq!(geometry.height, 19.0);
}

#[test]
fn plugin_constraints_clamp_host_sizes() {
    let constraints = PluginEditorConstraints::new(PluginEditorSize::new(960.0, 540.0))
        .with_min_size(PluginEditorSize::new(640.0, 360.0))
        .with_max_size(PluginEditorSize::new(1440.0, 900.0));

    assert_eq!(
        constraints
            .try_clamp_host_size(PluginEditorSize::new(320.0, 240.0))
            .expect("valid host size clamps"),
        PluginEditorSize::new(640.0, 360.0)
    );
    assert_eq!(
        constraints
            .try_clamp_host_size(PluginEditorSize::new(1920.0, 1200.0))
            .expect("valid host size clamps"),
        PluginEditorSize::new(1440.0, 900.0)
    );
    assert_eq!(
        constraints
            .try_clamp_host_size(PluginEditorSize::new(1000.0, 700.0))
            .expect("valid host size clamps"),
        PluginEditorSize::new(1000.0, 700.0)
    );
}

#[test]
fn plugin_constraints_negotiate_host_resize_with_clamp_and_dpi_metrics() {
    let constraints = PluginEditorConstraints::new(PluginEditorSize::new(960.0, 540.0))
        .with_min_size(PluginEditorSize::new(640.0, 360.0))
        .with_max_size(PluginEditorSize::new(1440.0, 900.0));

    let negotiation = constraints
        .try_negotiate_host_resize(PluginEditorSize::new(1920.0, 1200.0), 1.5)
        .expect("valid plugin host resize negotiates");

    assert_eq!(
        negotiation.requested_size(),
        PluginEditorSize::new(1920.0, 1200.0)
    );
    assert_eq!(
        negotiation.accepted_size(),
        PluginEditorSize::new(1440.0, 900.0)
    );
    assert!(negotiation.was_clamped());
    assert_eq!(negotiation.surface_metrics().logical_width, 1440.0);
    assert_eq!(negotiation.surface_metrics().logical_height, 900.0);
    assert_eq!(negotiation.surface_metrics().scale_factor, 1.5);
    assert_eq!(negotiation.surface_metrics().physical_size(), (2160, 1350));
}

#[test]
fn plugin_constraints_reject_invalid_sizes_regions_and_parameters() {
    let error = PluginEditorConstraints::new(PluginEditorSize::new(f32::NAN, 540.0))
        .validate()
        .expect_err("non-finite default sizes must fail");
    assert_eq!(error.rule(), "plugin-layout.size.invalid");

    let error = PluginEditorConstraints::new(PluginEditorSize::new(960.0, 540.0))
        .with_min_size(PluginEditorSize::new(1440.0, 900.0))
        .with_max_size(PluginEditorSize::new(640.0, 360.0))
        .try_clamp_host_size(PluginEditorSize::new(1000.0, 700.0))
        .expect_err("inverted min/max ranges must fail");
    assert_eq!(error.rule(), "plugin-layout.range.invalid");

    let error = PluginEditorConstraints::new(PluginEditorSize::new(960.0, 540.0))
        .with_graph_region(GraphRegion::new("", 0.0, 0.0, 10.0, 10.0))
        .validate()
        .expect_err("empty graph region IDs must fail");
    assert_eq!(error.rule(), "plugin-layout.region.invalid");

    let error = PluginEditorConstraints::new(PluginEditorSize::new(960.0, 540.0))
        .with_generated_parameters(GeneratedParameterLayout::dense_panel("parameters", [""], 2))
        .validate()
        .expect_err("empty generated parameter IDs must fail");
    assert_eq!(error.rule(), "plugin-layout.parameters.invalid");
}

#[test]
fn plugin_constraints_preserve_graph_regions_and_generate_dense_panels() {
    let constraints = PluginEditorConstraints::new(PluginEditorSize::new(960.0, 540.0))
        .with_graph_region(GraphRegion::new("spectrum", 24.0, 96.0, 600.0, 240.0))
        .with_analyzer_region(AnalyzerRegion::new(
            "gain-reduction",
            648.0,
            96.0,
            288.0,
            120.0,
        ))
        .with_generated_parameters(GeneratedParameterLayout::dense_panel(
            "parameters",
            ["gain", "mix", "attack", "release"],
            2,
        ));

    assert_eq!(constraints.graph_region("spectrum").unwrap().width, 600.0);
    assert_eq!(
        constraints
            .analyzer_region("gain-reduction")
            .unwrap()
            .height,
        120.0
    );
    assert_eq!(
        constraints
            .generated_parameters("parameters")
            .unwrap()
            .rows(),
        2
    );
}

#[test]
fn scene_geometry_attaches_render_hit_accessibility_clip_and_surface_records() {
    let tree = LayoutTree::new(LayoutNode::new(
        LayoutNodeId::new("root"),
        LayoutStyle::scroll_container().with_size(LayoutSizing::fixed(200.0, 120.0)),
    ))
    .with_child(
        LayoutNodeId::new("root"),
        LayoutNode::new(
            LayoutNodeId::new("surface"),
            LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(100.0, 80.0)),
        ),
    )
    .expect("surface child insertion should succeed");

    let output = tree.compute_layout(Viewport::new(200.0, 120.0));
    let scene = SceneGeometryAttachment::from_layout(&output)
        .with_accessibility_node("surface", "Oscilloscope")
        .with_custom_surface("surface");

    let geometry = scene.geometry("surface").unwrap();

    assert_eq!(geometry.render.width, 100.0);
    assert_eq!(geometry.hit_test.height, 80.0);
    assert_eq!(
        geometry.accessibility_label.as_deref(),
        Some("Oscilloscope")
    );
    assert!(geometry.custom_surface);
    assert_eq!(scene.clip("root").unwrap().height, 120.0);
}

#[test]
fn scene_geometry_updates_after_layout_recomputation() {
    let tree = LayoutTree::new(LayoutNode::new(
        LayoutNodeId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Row)
            .with_size(LayoutSizing::percent(100.0, 100.0)),
    ))
    .with_child(
        LayoutNodeId::new("root"),
        LayoutNode::new(
            LayoutNodeId::new("child"),
            LayoutStyle::flex_container(FlexDirection::Column)
                .with_size(LayoutSizing::percent(50.0, 100.0)),
        ),
    )
    .expect("child insertion should succeed");

    let first =
        SceneGeometryAttachment::from_layout(&tree.compute_layout(Viewport::new(200.0, 100.0)));
    let second =
        SceneGeometryAttachment::from_layout(&tree.compute_layout(Viewport::new(400.0, 100.0)));

    assert_eq!(first.geometry("child").unwrap().render.width, 100.0);
    assert_eq!(second.geometry("child").unwrap().render.width, 200.0);
    assert_ne!(
        first.geometry("child").unwrap(),
        second.geometry("child").unwrap()
    );
}

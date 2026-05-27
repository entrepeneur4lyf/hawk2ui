use hawk2ui_render::RendererBackend;
use hawk2ui_render::{
    AccessibilityRef, Geometry, HitTestGeometry, InvalidationReason, SceneGraph, SceneNode,
    SceneNodeId, Transform,
};

#[test]
fn scene_graph_preserves_parent_child_mutation_and_z_order() {
    let graph = SceneGraph::new(SceneNode::new(SceneNodeId::new("root")))
        .with_child(
            SceneNodeId::new("root"),
            SceneNode::new(SceneNodeId::new("back")).with_z_order(10),
        )
        .expect("back child insertion succeeds")
        .with_child(
            SceneNodeId::new("root"),
            SceneNode::new(SceneNodeId::new("front")).with_z_order(30),
        )
        .expect("front child insertion succeeds")
        .with_child(
            SceneNodeId::new("root"),
            SceneNode::new(SceneNodeId::new("middle")).with_z_order(20),
        )
        .expect("middle child insertion succeeds");

    let ordered: Vec<_> = graph
        .children_sorted_by_z(&SceneNodeId::new("root"))
        .iter()
        .map(|node| node.id().as_str())
        .collect();

    assert_eq!(ordered, ["back", "middle", "front"]);
    assert_eq!(
        graph
            .parent_of(&SceneNodeId::new("front"))
            .expect("front parent")
            .as_str(),
        "root"
    );
}

#[test]
fn scene_graph_attaches_geometry_and_propagates_invalidation() {
    let graph = SceneGraph::new(SceneNode::new(SceneNodeId::new("root")))
        .with_child(
            SceneNodeId::new("root"),
            SceneNode::new(SceneNodeId::new("meter"))
                .with_layout(Geometry::new(10.0, 20.0, 120.0, 48.0))
                .with_clip(Geometry::new(0.0, 0.0, 200.0, 120.0))
                .with_transform(Transform::translate(4.0, 8.0))
                .with_opacity(0.75)
                .with_hit_test(HitTestGeometry::new(8.0, 18.0, 124.0, 52.0))
                .with_accessibility_ref(AccessibilityRef::new("meter-a11y")),
        )
        .expect("meter child insertion succeeds")
        .invalidate(&SceneNodeId::new("meter"), InvalidationReason::Geometry)
        .expect("invalidation succeeds");

    let meter = graph.node(&SceneNodeId::new("meter")).expect("meter node");
    let root = graph.node(&SceneNodeId::new("root")).expect("root node");

    assert_eq!(meter.layout().unwrap().width, 120.0);
    assert_eq!(meter.clip().unwrap().height, 120.0);
    assert_eq!(meter.transform(), Transform::translate(4.0, 8.0));
    assert_eq!(meter.opacity(), 0.75);
    assert_eq!(meter.hit_test().unwrap().height, 52.0);
    assert_eq!(meter.accessibility_ref().unwrap().as_str(), "meter-a11y");
    assert!(meter.invalidated());
    assert!(
        root.invalidated(),
        "child invalidation must propagate to root"
    );
}

#[test]
fn scene_graph_rejects_invalid_node_records() {
    let error = SceneGraph::new(SceneNode::new(SceneNodeId::new("")))
        .validate()
        .expect_err("empty root scene IDs must fail validation");
    assert_eq!(
        error,
        hawk2ui_render::SceneGraphError::InvalidNodeId(String::new())
    );

    let error = SceneGraph::new(SceneNode::new(SceneNodeId::new("root")))
        .with_child(
            SceneNodeId::new("root"),
            SceneNode::new(SceneNodeId::new("")),
        )
        .expect_err("empty child scene IDs must fail insertion");
    assert_eq!(
        error,
        hawk2ui_render::SceneGraphError::InvalidNodeId(String::new())
    );

    let error = SceneGraph::new(
        SceneNode::new(SceneNodeId::new("root")).with_layout(Geometry::new(
            0.0,
            0.0,
            f32::NAN,
            10.0,
        )),
    )
    .validate()
    .expect_err("non-finite scene geometry must fail validation");
    assert_eq!(
        error,
        hawk2ui_render::SceneGraphError::InvalidGeometry("root".to_string())
    );

    let error = SceneGraph::new(SceneNode::new(SceneNodeId::new("root")).with_opacity(1.5))
        .validate()
        .expect_err("opacity outside the renderable range must fail validation");
    assert_eq!(
        error,
        hawk2ui_render::SceneGraphError::InvalidOpacity("root".to_string())
    );
}

#[test]
fn layer_records_sort_deterministically_and_serialize_stably() {
    let stack = hawk2ui_render::LayerStack::new()
        .with_layer(hawk2ui_render::PaintLayer::new(
            "text",
            30,
            hawk2ui_render::LayerKind::Text(hawk2ui_render::TextLayer::new("Hello")),
        ))
        .with_layer(hawk2ui_render::PaintLayer::new(
            "fill",
            10,
            hawk2ui_render::LayerKind::Fill(hawk2ui_render::Color::rgba(20, 30, 40, 255)),
        ))
        .with_layer(hawk2ui_render::PaintLayer::new(
            "custom",
            20,
            hawk2ui_render::LayerKind::CustomSurface("scope".to_string()),
        ));

    assert_eq!(
        stack.ordered_keys(),
        [
            "10:fill:fill(20,30,40,255)",
            "20:custom:custom-surface(scope)",
            "30:text:text(Hello)",
        ]
    );
    assert_eq!(
        stack.serialize_stable(),
        "10:fill:fill(20,30,40,255)|20:custom:custom-surface(scope)|30:text:text(Hello)"
    );
}

#[test]
fn layer_records_cover_required_layer_families() {
    let layers = [
        hawk2ui_render::LayerKind::Stroke(hawk2ui_render::Stroke::new(2.0)),
        hawk2ui_render::LayerKind::RoundedRect(hawk2ui_render::RoundedRect::new(8.0)),
        hawk2ui_render::LayerKind::Path(hawk2ui_render::PathLayer::new("M0 0L10 10")),
        hawk2ui_render::LayerKind::Gradient(hawk2ui_render::GradientLayer::linear()),
        hawk2ui_render::LayerKind::Shadow(hawk2ui_render::ShadowLayer::new(12.0)),
        hawk2ui_render::LayerKind::Glow(hawk2ui_render::GlowLayer::new(6.0)),
        hawk2ui_render::LayerKind::OpacityGroup(0.5),
        hawk2ui_render::LayerKind::Clip(Geometry::new(0.0, 0.0, 100.0, 100.0)),
        hawk2ui_render::LayerKind::Transform(Transform::translate(2.0, 4.0)),
        hawk2ui_render::LayerKind::Image("hero".to_string()),
        hawk2ui_render::LayerKind::Vector("logo".to_string()),
        hawk2ui_render::LayerKind::Control("button".to_string()),
        hawk2ui_render::LayerKind::StaticCache("card-cache".to_string()),
        hawk2ui_render::LayerKind::LiveLayer("meter".to_string()),
    ];

    let keys: Vec<_> = layers
        .iter()
        .map(hawk2ui_render::LayerKind::stable_key)
        .collect();

    assert_eq!(keys.len(), 14);
    assert!(keys.contains(&"gradient(linear)".to_string()));
    assert!(keys.contains(&"static-cache(card-cache)".to_string()));
    assert!(keys.contains(&"live-layer(meter)".to_string()));
}

#[test]
fn backend_boundary_records_surface_lifecycle_and_frame_commands() {
    let mut backend = hawk2ui_render::RecordingBackend::new(
        hawk2ui_render::BackendCapabilities::new()
            .with_gpu(true)
            .with_text(true)
            .with_images(true),
    );

    backend.create_surface("main", 800, 600).unwrap();
    backend.resize_surface("main", 1024, 768, 2.0).unwrap();
    backend.begin_frame("main").unwrap();
    backend
        .clear(hawk2ui_render::Color::rgba(0, 0, 0, 255))
        .unwrap();
    backend
        .fill(
            Geometry::new(0.0, 0.0, 100.0, 50.0),
            hawk2ui_render::Color::rgba(255, 0, 0, 255),
        )
        .unwrap();
    backend
        .stroke(
            Geometry::new(0.0, 0.0, 100.0, 50.0),
            hawk2ui_render::Stroke::new(2.0),
        )
        .unwrap();
    backend.draw_path("M0 0L10 10").unwrap();
    backend.draw_text("Hello").unwrap();
    backend.draw_image("hero").unwrap();
    backend
        .push_clip(Geometry::new(0.0, 0.0, 80.0, 40.0))
        .unwrap();
    backend
        .push_transform(Transform::translate(4.0, 8.0))
        .unwrap();
    backend.apply_layer_effect("shadow").unwrap();
    let cache = backend.create_cache_handle("card").unwrap();
    backend
        .mark_dirty(Geometry::new(0.0, 0.0, 100.0, 50.0))
        .unwrap();
    backend.end_frame("main").unwrap();
    backend.teardown_surface("main").unwrap();

    assert_eq!(cache.as_str(), "card");
    assert!(backend.capabilities().gpu);
    assert_eq!(backend.dirty_regions().len(), 1);
    assert_eq!(
        backend.command_keys(),
        [
            "create-surface:main:800x600",
            "resize-surface:main:1024x768@2",
            "begin-frame:main",
            "clear:0,0,0,255",
            "fill:0,0,100,50:255,0,0,255",
            "stroke:0,0,100,50:2",
            "path:M0 0L10 10",
            "text:Hello",
            "image:hero",
            "clip:0,0,80,40",
            "transform:4,8",
            "effect:shadow",
            "cache:card",
            "dirty:0,0,100,50",
            "end-frame:main",
            "teardown-surface:main",
        ]
    );
}

#[test]
fn backend_boundary_reports_diagnostics_for_missing_capabilities() {
    let mut backend =
        hawk2ui_render::RecordingBackend::new(hawk2ui_render::BackendCapabilities::new());

    let error = backend
        .draw_text("Hello")
        .expect_err("missing text capability must fail");

    assert_eq!(error.diagnostic().rule(), "backend.capability.text.missing");
}

#[test]
fn backend_boundary_rejects_invalid_surface_and_geometry_inputs() {
    let mut backend =
        hawk2ui_render::RecordingBackend::new(hawk2ui_render::BackendCapabilities::new());

    let error = backend
        .create_surface("", 800, 600)
        .expect_err("empty surface IDs must fail");
    assert_eq!(error.diagnostic().rule(), "backend.surface.id.invalid");

    let error = backend
        .create_surface("main", 0, 600)
        .expect_err("zero surface dimensions must fail");
    assert_eq!(error.diagnostic().rule(), "backend.surface.size.invalid");

    let error = backend
        .resize_surface("main", 800, 600, f32::NAN)
        .expect_err("invalid DPI scales must fail");
    assert_eq!(error.diagnostic().rule(), "backend.surface.dpi.invalid");

    let error = backend
        .mark_dirty(Geometry::new(0.0, 0.0, -1.0, 20.0))
        .expect_err("invalid dirty geometry must fail");
    assert_eq!(error.diagnostic().rule(), "backend.geometry.invalid");

    assert!(
        backend.command_keys().is_empty(),
        "invalid backend inputs must not be recorded as commands"
    );
}

#[test]
fn render_export_produces_stable_paint_commands() {
    let stack = hawk2ui_render::LayerStack::new()
        .with_layer(hawk2ui_render::PaintLayer::new(
            "shape",
            10,
            hawk2ui_render::LayerKind::RoundedRect(hawk2ui_render::RoundedRect::new(12.0)),
        ))
        .with_layer(hawk2ui_render::PaintLayer::new(
            "gradient",
            20,
            hawk2ui_render::LayerKind::Gradient(hawk2ui_render::GradientLayer::linear()),
        ))
        .with_layer(hawk2ui_render::PaintLayer::new(
            "text",
            30,
            hawk2ui_render::LayerKind::Text(hawk2ui_render::TextLayer::new("Amount")),
        ))
        .with_layer(hawk2ui_render::PaintLayer::new(
            "image",
            40,
            hawk2ui_render::LayerKind::Image("hero".to_string()),
        ))
        .with_layer(hawk2ui_render::PaintLayer::new(
            "vector",
            50,
            hawk2ui_render::LayerKind::Vector("logo".to_string()),
        ))
        .with_layer(hawk2ui_render::PaintLayer::new(
            "surface",
            60,
            hawk2ui_render::LayerKind::CustomSurface("scope".to_string()),
        ));

    let commands = hawk2ui_render::export_paint_commands(&stack).expect("layer stack is valid");

    assert_eq!(
        commands.serialize_stable(),
        "draw-rounded-rect:shape:12|draw-gradient:gradient:linear|draw-text:text:Amount|draw-image:image:hero|draw-vector:vector:logo|draw-custom-surface:surface:scope"
    );
}

#[test]
fn paint_export_rejects_invalid_layer_records() {
    let error = hawk2ui_render::export_paint_commands(
        &hawk2ui_render::LayerStack::new().with_layer(hawk2ui_render::PaintLayer::new(
            "",
            0,
            hawk2ui_render::LayerKind::Fill(hawk2ui_render::Color::rgba(0, 0, 0, 255)),
        )),
    )
    .expect_err("empty layer keys must fail");
    assert_eq!(error.rule(), "layer.key.invalid");

    let error = hawk2ui_render::export_paint_commands(
        &hawk2ui_render::LayerStack::new().with_layer(hawk2ui_render::PaintLayer::new(
            "stroke",
            0,
            hawk2ui_render::LayerKind::Stroke(hawk2ui_render::Stroke::new(f32::NAN)),
        )),
    )
    .expect_err("non-finite stroke widths must fail");
    assert_eq!(error.rule(), "layer.stroke.invalid");

    let error = hawk2ui_render::export_paint_commands(
        &hawk2ui_render::LayerStack::new().with_layer(hawk2ui_render::PaintLayer::new(
            "image",
            0,
            hawk2ui_render::LayerKind::Image(String::new()),
        )),
    )
    .expect_err("empty image asset IDs must fail");
    assert_eq!(error.rule(), "layer.reference.invalid");
}

#[test]
fn text_contracts_measure_shape_linebreak_bidi_and_baseline() {
    let registry = hawk2ui_render::FontRegistry::new()
        .with_system_font("Atkinson")
        .with_app_font("Display", "assets/fonts/display.ttf");
    let measurer =
        hawk2ui_render::DeterministicTextMeasurer::new(registry).with_average_glyph_width(7.0);
    let input = hawk2ui_render::TextRenderInput::new("Gain שלום", "Display", 18.0)
        .with_dpi_scale(2.0)
        .with_line_break(hawk2ui_render::LineBreakMode::Wrap { max_width: 42.0 })
        .with_bidi(true);

    let output = measurer.measure(&input).expect("text measurement succeeds");

    assert_eq!(output.width, 42.0);
    assert_eq!(output.line_count, 2);
    assert_eq!(output.baseline, 28.8);
    assert!(output.shaped);
    assert!(output.bidi_resolved);
}

#[test]
fn text_contracts_reject_invalid_measurement_inputs() {
    let registry = hawk2ui_render::FontRegistry::new().with_system_font("Atkinson");
    let measurer =
        hawk2ui_render::DeterministicTextMeasurer::new(registry).with_average_glyph_width(7.0);

    let error = measurer
        .measure(&hawk2ui_render::TextRenderInput::new(
            "Gain",
            "Atkinson",
            f32::NAN,
        ))
        .expect_err("non-finite font sizes must fail");
    assert_eq!(error.rule(), "text.size.invalid");

    let error = measurer
        .measure(
            &hawk2ui_render::TextRenderInput::new("Gain", "Atkinson", 18.0).with_dpi_scale(0.0),
        )
        .expect_err("non-positive DPI scales must fail");
    assert_eq!(error.rule(), "text.dpi.invalid");

    let error = measurer
        .measure(
            &hawk2ui_render::TextRenderInput::new("Gain", "Atkinson", 18.0)
                .with_line_break(hawk2ui_render::LineBreakMode::Wrap { max_width: -1.0 }),
        )
        .expect_err("non-positive wrap widths must fail");
    assert_eq!(error.rule(), "text.wrap-width.invalid");
}

#[test]
fn text_contracts_generate_stable_glyph_cache_and_invalidation_keys() {
    let key = hawk2ui_render::GlyphCacheKey::new("Gain", "Display", 18.0, 2.0, true);
    let font_changed = hawk2ui_render::GlyphCacheKey::new("Gain", "Atkinson", 18.0, 2.0, true);
    let dpi_changed = hawk2ui_render::GlyphCacheKey::new("Gain", "Display", 18.0, 1.0, true);

    assert_eq!(
        key.stable_key(),
        "text=Gain|font=Display|size=18|dpi=2|bidi=true"
    );
    assert_ne!(key, font_changed);
    assert_ne!(key, dpi_changed);
}

#[test]
fn asset_records_require_compiled_image_vector_and_font_records() {
    let image =
        hawk2ui_render::CompiledAsset::image("hero", "assets/hero.png", "sha256:hero", 1024, 512)
            .with_sanitized(true)
            .with_backend_requirement(hawk2ui_render::BackendRequirement::Images)
            .with_package_path("pkg/assets/hero.png")
            .with_cache_generation(3);
    let vector =
        hawk2ui_render::CompiledAsset::vector("logo", "assets/logo.svg", "sha256:logo", 200, 80);
    let font = hawk2ui_render::CompiledAsset::font("display", "assets/display.ttf", "sha256:font");

    assert_eq!(
        image.stable_key(),
        "image:hero:sha256:hero:1024x512:sanitized=true:cache=3"
    );
    assert_eq!(vector.kind(), hawk2ui_render::AssetKind::Vector);
    assert_eq!(font.kind(), hawk2ui_render::AssetKind::Font);
    assert_eq!(image.package_path(), Some("pkg/assets/hero.png"));
}

#[test]
fn asset_records_reject_raw_draw_references() {
    let error = hawk2ui_render::AssetDrawRecord::from_raw_path("assets/hero.png")
        .expect_err("raw asset paths must not render directly");
    let asset =
        hawk2ui_render::CompiledAsset::image("hero", "assets/hero.png", "sha256:hero", 1024, 512);
    let draw =
        hawk2ui_render::AssetDrawRecord::from_compiled(&asset).expect("compiled asset is valid");

    assert_eq!(error.diagnostic().rule(), "asset.raw-reference.rejected");
    assert_eq!(draw.asset_id(), "hero");
}

#[test]
fn asset_records_reject_invalid_compiled_assets() {
    let error = hawk2ui_render::AssetDrawRecord::from_compiled(
        &hawk2ui_render::CompiledAsset::image("", "assets/hero.png", "sha256:hero", 1024, 512),
    )
    .expect_err("empty asset IDs must fail");
    assert_eq!(error.diagnostic().rule(), "asset.id.invalid");

    let error = hawk2ui_render::AssetDrawRecord::from_compiled(
        &hawk2ui_render::CompiledAsset::image("hero", "", "sha256:hero", 1024, 512),
    )
    .expect_err("empty asset source paths must fail");
    assert_eq!(error.diagnostic().rule(), "asset.source.invalid");

    let error = hawk2ui_render::AssetDrawRecord::from_compiled(
        &hawk2ui_render::CompiledAsset::image("hero", "assets/hero.png", "", 1024, 512),
    )
    .expect_err("empty asset hashes must fail");
    assert_eq!(error.diagnostic().rule(), "asset.hash.invalid");

    let error = hawk2ui_render::AssetDrawRecord::from_compiled(
        &hawk2ui_render::CompiledAsset::image("hero", "assets/hero.png", "sha256:hero", 0, 512),
    )
    .expect_err("zero image dimensions must fail");
    assert_eq!(error.diagnostic().rule(), "asset.dimensions.invalid");
}

#[test]
fn custom_surface_records_cover_categories_capabilities_and_frame_scheduling() {
    let surfaces = [
        hawk2ui_render::CustomSurfaceCategory::Knob,
        hawk2ui_render::CustomSurfaceCategory::Slider,
        hawk2ui_render::CustomSurfaceCategory::Meter,
        hawk2ui_render::CustomSurfaceCategory::Scope,
        hawk2ui_render::CustomSurfaceCategory::Analyzer,
        hawk2ui_render::CustomSurfaceCategory::EqCurve,
        hawk2ui_render::CustomSurfaceCategory::Modulation,
        hawk2ui_render::CustomSurfaceCategory::Timeline,
        hawk2ui_render::CustomSurfaceCategory::GraphEditor,
        hawk2ui_render::CustomSurfaceCategory::InspectorPanel,
    ];

    let keys: Vec<_> = surfaces
        .iter()
        .map(hawk2ui_render::CustomSurfaceCategory::stable_key)
        .collect();

    assert_eq!(keys.len(), 10);
    assert!(keys.contains(&"eq-curve"));

    let surface = hawk2ui_render::CustomDrawSurface::new(
        "scope",
        hawk2ui_render::CustomSurfaceCategory::Scope,
        Geometry::new(20.0, 30.0, 320.0, 180.0),
    )
    .with_capability(hawk2ui_render::CustomSurfaceCapability::RealtimeData)
    .with_capability(hawk2ui_render::CustomSurfaceCapability::GpuPreferred)
    .invalidate()
    .schedule_frame(42);

    assert!(surface.hit_test(40.0, 40.0));
    assert_eq!(surface.reserved_layout().width, 320.0);
    assert!(surface.invalidated());
    assert_eq!(surface.next_frame(), Some(42));
    assert!(surface.reports_capability(hawk2ui_render::CustomSurfaceCapability::RealtimeData));
}

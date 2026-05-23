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

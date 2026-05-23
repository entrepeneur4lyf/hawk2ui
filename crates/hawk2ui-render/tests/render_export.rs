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

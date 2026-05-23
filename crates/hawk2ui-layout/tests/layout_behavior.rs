use hawk2ui_layout::{
    BoxEdges, FlexDirection, LayoutNode, LayoutNodeId, LayoutSizing, LayoutStyle, LayoutTree,
    LayoutValue,
};

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

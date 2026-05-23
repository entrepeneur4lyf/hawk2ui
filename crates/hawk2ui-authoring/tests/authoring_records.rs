use hawk2ui_authoring::{ChildList, ElementId, ElementKind, ElementNode, KeyedChild, PropValue};

#[test]
fn element_records_preserve_stable_node_identity() {
    let node = ElementNode::new(ElementId::new("root"), ElementKind::View)
        .with_prop("role", PropValue::String("main".to_string()));

    assert_eq!(node.id().as_str(), "root");
    assert_eq!(node.kind(), ElementKind::View);
    assert_eq!(node.prop("role"), Some(&PropValue::String("main".into())));
}

#[test]
fn element_records_preserve_child_order() {
    let children = ChildList::ordered([
        ElementNode::new(ElementId::new("title"), ElementKind::Text),
        ElementNode::new(ElementId::new("button"), ElementKind::Button),
        ElementNode::new(ElementId::new("meter"), ElementKind::View),
    ])
    .expect("ordered children should be accepted");

    let ids: Vec<_> = children.iter().map(|node| node.id().as_str()).collect();

    assert_eq!(ids, ["title", "button", "meter"]);
}

#[test]
fn element_records_reject_duplicate_keyed_children() {
    let error = ChildList::keyed([
        KeyedChild::new(
            "gain",
            ElementNode::new(ElementId::new("knob-a"), ElementKind::View),
        ),
        KeyedChild::new(
            "gain",
            ElementNode::new(ElementId::new("knob-b"), ElementKind::View),
        ),
    ])
    .expect_err("duplicate keyed children must be rejected");

    assert_eq!(error.duplicate_key(), "gain");
}

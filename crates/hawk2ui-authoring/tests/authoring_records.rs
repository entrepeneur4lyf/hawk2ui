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

#[test]
fn component_records_preserve_props_references_and_slots() {
    let slot = ChildList::ordered([ElementNode::new(ElementId::new("label"), ElementKind::Text)])
        .expect("slot children should be accepted");

    let component = hawk2ui_authoring::ComponentInstance::new(
        hawk2ui_authoring::ComponentId::new("gain-knob"),
        "PremiumKnob",
    )
    .with_prop("value", PropValue::Number(0.75))
    .with_reference("parameter", "gain")
    .with_slot("label", slot);

    assert_eq!(component.id().as_str(), "gain-knob");
    assert_eq!(component.component_name(), "PremiumKnob");
    assert_eq!(component.prop("value"), Some(&PropValue::Number(0.75)));
    assert_eq!(component.reference("parameter"), Some("gain"));
    assert_eq!(component.slot("label").unwrap().iter().count(), 1);
}

#[test]
fn component_records_keep_custom_controls_and_surfaces_distinct() {
    let control = hawk2ui_authoring::ComponentInstance::new(
        hawk2ui_authoring::ComponentId::new("scope-control"),
        "ScopeControl",
    );
    let surface = hawk2ui_authoring::CustomSurfaceDeclaration::new(
        hawk2ui_authoring::SurfaceId::new("scope-surface"),
        hawk2ui_authoring::SurfacePurpose::CustomDraw,
    )
    .with_reference("feed", "oscilloscope");

    assert_ne!(control.id().as_str(), surface.id().as_str());
    assert_eq!(
        surface.purpose(),
        hawk2ui_authoring::SurfacePurpose::CustomDraw
    );
    assert_eq!(surface.reference("feed"), Some("oscilloscope"));
}

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

#[test]
fn event_records_cover_native_event_domains() {
    let events = [
        hawk2ui_authoring::EventKind::Pointer(hawk2ui_authoring::PointerEventKind::Press),
        hawk2ui_authoring::EventKind::Keyboard(hawk2ui_authoring::KeyboardEventKind::KeyDown),
        hawk2ui_authoring::EventKind::Focus(hawk2ui_authoring::FocusEventKind::FocusIn),
        hawk2ui_authoring::EventKind::Input(hawk2ui_authoring::InputEventKind::ValueChanged),
        hawk2ui_authoring::EventKind::Resize,
        hawk2ui_authoring::EventKind::Lifecycle(hawk2ui_authoring::LifecycleEventKind::Mounted),
        hawk2ui_authoring::EventKind::CustomComponent("knob.drag".to_string()),
        hawk2ui_authoring::EventKind::PluginParameter("gain".to_string()),
    ];

    let keys: Vec<_> = events
        .iter()
        .map(hawk2ui_authoring::EventKind::stable_key)
        .collect();

    assert_eq!(
        keys,
        [
            "pointer.press",
            "keyboard.key-down",
            "focus.focus-in",
            "input.value-changed",
            "resize",
            "lifecycle.mounted",
            "component.knob.drag",
            "plugin-parameter.gain",
        ]
    );
}

#[test]
fn event_records_do_not_depend_on_browser_event_object_names() {
    let binding = hawk2ui_authoring::EventBinding::new(
        ElementId::new("gain-knob"),
        hawk2ui_authoring::EventKind::Pointer(hawk2ui_authoring::PointerEventKind::Drag),
        hawk2ui_authoring::HandlerRef::new("update_gain_from_pointer"),
    )
    .with_payload(hawk2ui_authoring::EventPayloadField::Position)
    .with_payload(hawk2ui_authoring::EventPayloadField::Delta);

    assert_eq!(binding.target().as_str(), "gain-knob");
    assert_eq!(binding.event().stable_key(), "pointer.drag");
    assert_eq!(binding.handler().as_str(), "update_gain_from_pointer");
    assert_eq!(
        binding.payload_fields(),
        &[
            hawk2ui_authoring::EventPayloadField::Position,
            hawk2ui_authoring::EventPayloadField::Delta,
        ]
    );
}

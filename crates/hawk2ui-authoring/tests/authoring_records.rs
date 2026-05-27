use hawk2ui_authoring::{
    AssetRef, ChildList, ElementId, ElementKind, ElementNode, EventKind, EventPayloadField,
    FrameworkNativeNode, FrameworkNativeProgram, HandlerRef, KeyedChild, NativeLifecycleEvent,
    NativeRef, PointerEventKind, PropValue, StyleRef,
};

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
fn framework_native_program_records_explicit_compiler_boundary_without_source_scanning() {
    let program = FrameworkNativeProgram::new(
        FrameworkNativeNode::new("root", ElementKind::View)
            .with_ref(NativeRef::new("root_ref"))
            .with_style(StyleRef::new("surface.card"))
            .with_asset(AssetRef::new("hawk.logo", "assets/logo.svg"))
            .with_event(
                EventKind::Pointer(PointerEventKind::Press),
                HandlerRef::new("handlePress"),
                [EventPayloadField::Position],
            )
            .with_lifecycle(NativeLifecycleEvent::Mounted, HandlerRef::new("onMount"))
            .with_lifecycle(
                NativeLifecycleEvent::Unmounted,
                HandlerRef::new("onDestroy"),
            )
            .with_child(
                "title",
                FrameworkNativeNode::new("title", ElementKind::Text)
                    .with_prop("text", PropValue::String("Boundary Title".to_string()))
                    .with_key("title"),
            ),
    );

    assert_eq!(program.root().id().as_str(), "root");
    assert_eq!(program.keyed_child_order(), ["title"]);
    assert_eq!(
        program.custom_renderer_operation_keys("svelte").unwrap(),
        [
            "create-node:root:view",
            "set-style:root:surface.card",
            "set-asset:root:assets/logo.svg",
            "set-ref:root:root_ref",
            "bind-event:root:pointer.press",
            "bind-lifecycle:root:mounted:onMount",
            "bind-lifecycle:root:unmounted:onDestroy",
            "create-node:title:text",
            "set-prop:title:text",
            "append-child:root:title:key:title",
            "commit:root"
        ]
    );
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

#[test]
fn state_records_group_batched_updates_by_scope() {
    let batch = hawk2ui_authoring::BatchedUpdate::new("startup")
        .with_update(hawk2ui_authoring::StateUpdate::new(
            hawk2ui_authoring::StateId::new("app.theme"),
            hawk2ui_authoring::StateScope::App,
            PropValue::String("dark".to_string()),
        ))
        .with_update(hawk2ui_authoring::StateUpdate::new(
            hawk2ui_authoring::StateId::new("component.gain.value"),
            hawk2ui_authoring::StateScope::Component(ElementId::new("gain-knob")),
            PropValue::Number(0.5),
        ))
        .with_update(hawk2ui_authoring::StateUpdate::new(
            hawk2ui_authoring::StateId::new("plugin.gain"),
            hawk2ui_authoring::StateScope::PluginBinding("gain".to_string()),
            PropValue::Number(0.5),
        ));

    assert_eq!(batch.name(), "startup");
    assert_eq!(
        batch
            .updates_for_scope(hawk2ui_authoring::StateScopeKind::App)
            .len(),
        1
    );
    assert_eq!(
        batch
            .updates_for_scope(hawk2ui_authoring::StateScopeKind::Component)
            .len(),
        1
    );
    assert_eq!(
        batch
            .updates_for_scope(hawk2ui_authoring::StateScopeKind::PluginBinding)
            .len(),
        1
    );
}

#[test]
fn state_records_preserve_deterministic_teardown_ordering() {
    let subscription = hawk2ui_authoring::StateSubscription::new(
        hawk2ui_authoring::SubscriptionId::new("gain-subscription"),
        hawk2ui_authoring::StateId::new("plugin.gain"),
        hawk2ui_authoring::HandlerRef::new("sync_gain"),
    );

    let teardown = hawk2ui_authoring::TeardownPlan::new()
        .with_step(hawk2ui_authoring::TeardownStep::ReleaseSubscription(
            subscription.id().clone(),
        ))
        .with_step(hawk2ui_authoring::TeardownStep::DetachPluginBinding(
            "gain".to_string(),
        ))
        .with_step(hawk2ui_authoring::TeardownStep::ClearComponentState(
            ElementId::new("gain-knob"),
        ));

    assert_eq!(subscription.state().as_str(), "plugin.gain");
    assert_eq!(
        teardown.step_keys(),
        [
            "release-subscription:gain-subscription",
            "detach-plugin-binding:gain",
            "clear-component-state:gain-knob",
        ]
    );
}

#[test]
fn compile_basic_fixture_emits_component_text_children_and_click_event() {
    let input = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("fixtures/authoring/basic_component.hawk"),
    )
    .expect("basic authoring fixture must be readable");
    let mut diagnostics = Vec::new();

    let artifact = hawk2ui_authoring::compile_authoring_source(&input, &mut diagnostics);

    assert!(diagnostics.is_empty());
    assert_eq!(artifact.components().len(), 1);
    assert_eq!(artifact.events().len(), 1);

    let component = &artifact.components()[0];
    assert_eq!(component.id().as_str(), "counter-card");
    assert_eq!(component.component_name(), "CounterCard");
    assert_eq!(
        component.slot("default").unwrap().iter().count(),
        2,
        "fixture must compile two text children"
    );
    assert_eq!(artifact.events()[0].event().stable_key(), "pointer.press");
    assert_eq!(artifact.events()[0].handler().as_str(), "increment_counter");
}

#[test]
fn adapter_contract_records_equivalent_operations_for_framework_labels() {
    for framework in ["native", "svelte", "react", "vue", "solid"] {
        let mut adapter = hawk2ui_authoring::RecordingNativeRendererAdapter::new(framework);
        adapter
            .apply(hawk2ui_authoring::NodeOperation::MountElement(
                ElementNode::new(ElementId::new("root"), ElementKind::View),
            ))
            .expect("recording adapter accepts mount operation");
        adapter
            .apply(hawk2ui_authoring::NodeOperation::BindEvent(
                hawk2ui_authoring::EventBinding::new(
                    ElementId::new("root"),
                    hawk2ui_authoring::EventKind::Pointer(
                        hawk2ui_authoring::PointerEventKind::Press,
                    ),
                    hawk2ui_authoring::HandlerRef::new("handle_press"),
                ),
            ))
            .expect("recording adapter accepts event operation");

        assert_eq!(adapter.framework_label(), framework);
        assert_eq!(
            adapter.operation_keys(),
            ["mount-element:root", "bind-event:root:pointer.press"]
        );
    }
}

#[test]
fn custom_renderer_protocol_records_full_node_lifecycle_surface() {
    let mut protocol = hawk2ui_authoring::CustomRendererProtocol::new("react");
    let root = ElementId::new("root");

    protocol
        .apply(hawk2ui_authoring::CustomRendererOperation::CreateNode {
            id: root.clone(),
            kind: ElementKind::View,
        })
        .expect("create node should be accepted");
    protocol
        .apply(hawk2ui_authoring::CustomRendererOperation::SetProp {
            id: root.clone(),
            name: "role".to_string(),
            value: PropValue::String("main".to_string()),
        })
        .expect("set prop should be accepted");
    protocol
        .apply(hawk2ui_authoring::CustomRendererOperation::SetStyleRef {
            id: root.clone(),
            style_ref: hawk2ui_authoring::StyleRef::new("surface.card"),
        })
        .expect("style ref should be accepted");
    protocol
        .apply(hawk2ui_authoring::CustomRendererOperation::SetAssetRef {
            id: root.clone(),
            asset_ref: hawk2ui_authoring::AssetRef::new("logo", "assets/logo.svg"),
        })
        .expect("asset ref should be accepted");
    protocol
        .apply(hawk2ui_authoring::CustomRendererOperation::SetRef {
            id: root.clone(),
            reference: hawk2ui_authoring::NativeRef::new("root_ref"),
        })
        .expect("native ref should be accepted");
    protocol
        .apply(hawk2ui_authoring::CustomRendererOperation::BindEvent {
            binding: hawk2ui_authoring::EventBinding::new(
                root.clone(),
                hawk2ui_authoring::EventKind::Pointer(hawk2ui_authoring::PointerEventKind::Press),
                hawk2ui_authoring::HandlerRef::new("handle_press"),
            )
            .with_payload(hawk2ui_authoring::EventPayloadField::Position),
        })
        .expect("event binding should be accepted");
    protocol
        .apply(hawk2ui_authoring::CustomRendererOperation::BindLifecycle {
            id: root.clone(),
            event: hawk2ui_authoring::NativeLifecycleEvent::Mounted,
            handler: hawk2ui_authoring::HandlerRef::new("on_mount"),
        })
        .expect("lifecycle binding should be accepted");
    protocol
        .apply(hawk2ui_authoring::CustomRendererOperation::CreateNode {
            id: ElementId::new("title"),
            kind: ElementKind::Text,
        })
        .expect("child create should be accepted");
    protocol
        .apply(hawk2ui_authoring::CustomRendererOperation::AppendChild {
            parent: root.clone(),
            child: ElementId::new("title"),
            key: Some("title".to_string()),
        })
        .expect("append child should be accepted");
    protocol
        .apply(
            hawk2ui_authoring::CustomRendererOperation::EnterErrorBoundary {
                id: root.clone(),
                handler: hawk2ui_authoring::HandlerRef::new("handle_error"),
            },
        )
        .expect("error boundary should be accepted");
    protocol
        .apply(hawk2ui_authoring::CustomRendererOperation::Commit { root: root.clone() })
        .expect("commit should be accepted");
    protocol
        .apply(hawk2ui_authoring::CustomRendererOperation::RemoveNode {
            id: ElementId::new("title"),
        })
        .expect("remove node should be accepted");

    assert_eq!(protocol.framework_label(), "react");
    assert_eq!(
        protocol.operation_keys(),
        [
            "create-node:root:view",
            "set-prop:root:role",
            "set-style:root:surface.card",
            "set-asset:root:assets/logo.svg",
            "set-ref:root:root_ref",
            "bind-event:root:pointer.press",
            "bind-lifecycle:root:mounted:on_mount",
            "create-node:title:text",
            "append-child:root:title:key:title",
            "error-boundary:root:handle_error",
            "commit:root",
            "remove-node:title",
        ]
    );
}

#[test]
fn custom_renderer_protocol_rejects_duplicate_nodes_and_missing_children() {
    let mut protocol = hawk2ui_authoring::CustomRendererProtocol::new("vue");
    protocol
        .apply(hawk2ui_authoring::CustomRendererOperation::CreateNode {
            id: ElementId::new("root"),
            kind: ElementKind::View,
        })
        .expect("first create should be accepted");

    let duplicate = protocol
        .apply(hawk2ui_authoring::CustomRendererOperation::CreateNode {
            id: ElementId::new("root"),
            kind: ElementKind::Text,
        })
        .expect_err("duplicate node IDs must be rejected");
    assert_eq!(duplicate.rule(), "custom-renderer.node.duplicate");

    let missing = protocol
        .apply(hawk2ui_authoring::CustomRendererOperation::AppendChild {
            parent: ElementId::new("root"),
            child: ElementId::new("missing"),
            key: None,
        })
        .expect_err("missing child IDs must be rejected");
    assert_eq!(missing.rule(), "custom-renderer.node.missing");
}

use hawk2ui_a11y::{
    A11Y_ACTION_EVENT_HISTORY_LIMIT, A11Y_MAX_TREE_DEPTH, A11Y_MAX_TREE_NODES, A11yAction,
    A11yActionDispatcher, A11yActionEvent, A11yBounds, A11yNode, A11yNumericValue, A11yRole,
    A11yTree, CheckedState,
};
use hawk2ui_api::{Diagnostic, DiagnosticSeverity};

#[test]
fn tree_records_preserve_shape_identity_bounds_and_hierarchy() {
    let tree = A11yTree::new(
        A11yNode::new("root", A11yRole::Window)
            .name("Main Window")
            .bounds(A11yBounds::new(0.0, 0.0, 800.0, 600.0))
            .child(
                A11yNode::new("gain", A11yRole::Slider)
                    .name("Gain")
                    .description("Output gain")
                    .value("-6 dB")
                    .focused(true)
                    .bounds(A11yBounds::new(20.0, 20.0, 120.0, 32.0))
                    .action(A11yAction::Increment)
                    .action(A11yAction::Decrement),
            )
            .child(
                A11yNode::new("enabled", A11yRole::Checkbox)
                    .name("Enabled")
                    .checked(CheckedState::Checked)
                    .disabled(false),
            ),
    );

    assert_eq!(tree.root.id, "root");
    assert_eq!(tree.root.children[0].id, "gain");
    assert!((tree.root.children[0].bounds.unwrap().width - 120.0).abs() < f64::EPSILON);
    assert_eq!(
        tree.find("enabled").unwrap().checked,
        Some(CheckedState::Checked)
    );
}

#[test]
fn tree_records_are_serializable_contracts() {
    let tree = A11yTree::new(
        A11yNode::new("root", A11yRole::Window).child(
            A11yNode::new("gain", A11yRole::Slider)
                .name("Gain")
                .value("0.5")
                .numeric_value(A11yNumericValue::new(0.5).min(0.0).max(1.0).step(0.1))
                .size_of_set(4)
                .position_in_set(1)
                .action(A11yAction::SetValue(String::new())),
        ),
    );

    let encoded = serde_json::to_string(&tree).expect("a11y tree serializes");
    let decoded: A11yTree = serde_json::from_str(&encoded).expect("a11y tree deserializes");

    assert_eq!(decoded, tree);
}

use hawk2ui_a11y::{ComponentKind, ComponentSemantics, VisualStyleSemantics};

#[test]
fn component_semantics_exist_independently_of_visual_styles() {
    let styled = VisualStyleSemantics::new("primary", "#ffcc00");
    let button = ComponentSemantics::button("bypass", "Bypass").with_style(styled.clone());
    let slider = ComponentSemantics::slider("gain", "Gain", "-6 dB");
    let input = ComponentSemantics::text_input("name", "Preset Name", "Init");
    let checkbox = ComponentSemantics::checkbox("enabled", "Enabled", true);
    let list = ComponentSemantics::list("presets", "Presets", 4);
    let panel = ComponentSemantics::panel("main", "Main Panel");
    let custom = ComponentSemantics::custom("scope", "Oscilloscope", A11yRole::Custom);

    assert_eq!(button.kind, ComponentKind::Button);
    assert_eq!(button.accessible.role, A11yRole::Button);
    assert_eq!(button.style, Some(styled));
    assert_eq!(slider.accessible.value.as_deref(), Some("-6 dB"));
    assert_eq!(input.accessible.role, A11yRole::TextInput);
    assert_eq!(checkbox.accessible.checked, Some(CheckedState::Checked));
    assert_eq!(list.item_count, Some(4));
    assert_eq!(panel.accessible.role, A11yRole::Panel);
    assert_eq!(custom.accessible.role, A11yRole::Custom);
}

#[test]
fn actions_values_dispatch_focus_press_increment_decrement_set_value_and_custom() {
    let tree = A11yTree::new(
        A11yNode::new("root", A11yRole::Window)
            .child(A11yNode::new("other", A11yRole::Button).focused(true))
            .child(
                A11yNode::new("gain", A11yRole::Slider)
                    .name("Gain")
                    .value("0.5")
                    .numeric_value(A11yNumericValue::new(0.5).min(0.0).max(1.0).step(0.25))
                    .action(A11yAction::Focus)
                    .action(A11yAction::Increment)
                    .action(A11yAction::Decrement)
                    .action(A11yAction::SetValue(String::new()))
                    .action(A11yAction::Custom("reset".into())),
            )
            .child(
                A11yNode::new("enabled", A11yRole::Checkbox)
                    .checked(CheckedState::Unchecked)
                    .action(A11yAction::Press),
            ),
    );
    let mut dispatcher = A11yActionDispatcher::new(tree);

    dispatcher.dispatch(A11yActionEvent::focus("gain")).unwrap();
    dispatcher
        .dispatch(A11yActionEvent::increment("gain"))
        .unwrap();
    dispatcher
        .dispatch(A11yActionEvent::decrement("gain"))
        .unwrap();
    dispatcher
        .dispatch(A11yActionEvent::set_value("gain", "0.75"))
        .unwrap();
    dispatcher
        .dispatch(A11yActionEvent::custom("gain", "reset"))
        .unwrap();
    dispatcher
        .dispatch(A11yActionEvent::press("enabled"))
        .unwrap();

    assert!(dispatcher.tree().find("gain").unwrap().focused);
    assert!(!dispatcher.tree().find("other").unwrap().focused);
    assert_eq!(
        dispatcher.tree().find("gain").unwrap().value.as_deref(),
        Some("0.75")
    );
    assert!(
        (dispatcher
            .tree()
            .find("gain")
            .unwrap()
            .numeric_value
            .unwrap()
            .value
            - 0.75)
            .abs()
            < f64::EPSILON
    );
    assert_eq!(
        dispatcher.tree().find("enabled").unwrap().checked,
        Some(CheckedState::Checked)
    );
    assert_eq!(dispatcher.events().len(), 6);
}

#[test]
fn action_dispatch_set_value_clamps_numeric_text_and_preserves_suffix() {
    let tree = A11yTree::new(
        A11yNode::new("gain", A11yRole::Slider)
            .value("0.25 dB")
            .numeric_value(A11yNumericValue::new(0.25).min(0.0).max(1.0).step(0.25))
            .action(A11yAction::SetValue(String::new())),
    );
    let mut dispatcher = A11yActionDispatcher::new(tree);

    dispatcher
        .dispatch(A11yActionEvent::set_value("gain", "2 dB"))
        .expect("set value parses numeric text with suffix");

    let node = dispatcher.tree().find("gain").expect("gain node exists");
    assert_eq!(node.value.as_deref(), Some("1 dB"));
    assert!((node.numeric_value.unwrap().value - 1.0).abs() < f64::EPSILON);
}

#[test]
fn action_dispatch_rejects_unsupported_disabled_and_invalid_numeric_actions() {
    let tree = A11yTree::new(
        A11yNode::new("root", A11yRole::Window)
            .child(A11yNode::new("button", A11yRole::Button).action(A11yAction::Press))
            .child(
                A11yNode::new("disabled", A11yRole::Button)
                    .disabled(true)
                    .action(A11yAction::Press),
            )
            .child(
                A11yNode::new("bad-slider", A11yRole::Slider)
                    .value("loud")
                    .action(A11yAction::Increment),
            ),
    );
    let mut dispatcher = A11yActionDispatcher::new(tree);

    let unsupported = dispatcher
        .dispatch(A11yActionEvent::focus("button"))
        .expect_err("unsupported focus action must be rejected");
    let disabled = dispatcher
        .dispatch(A11yActionEvent::press("disabled"))
        .expect_err("disabled action target must be rejected");
    let invalid_value = dispatcher
        .dispatch(A11yActionEvent::increment("bad-slider"))
        .expect_err("invalid numeric value must be rejected");

    assert_eq!(unsupported.code, "a11y.action-unsupported");
    assert_eq!(disabled.code, "a11y.action-target-disabled");
    assert_eq!(invalid_value.code, "a11y.action-invalid-value");
}

#[test]
fn action_dispatch_bounds_event_history() {
    let tree = A11yTree::new(A11yNode::new("button", A11yRole::Button).action(A11yAction::Press));
    let mut dispatcher = A11yActionDispatcher::new(tree);

    for _ in 0..(A11Y_ACTION_EVENT_HISTORY_LIMIT + 4) {
        dispatcher
            .dispatch(A11yActionEvent::press("button"))
            .unwrap();
    }

    assert_eq!(dispatcher.events().len(), A11Y_ACTION_EVENT_HISTORY_LIMIT);
}

#[test]
fn action_dispatch_rebounds_deserialized_event_history_above_limit() {
    let events = (0..(A11Y_ACTION_EVENT_HISTORY_LIMIT + 4))
        .map(|_| A11yActionEvent::press("button"))
        .collect::<Vec<_>>();
    let value = serde_json::json!({
        "tree": {
            "root": A11yNode::new("button", A11yRole::Button).action(A11yAction::Press),
        },
        "events": events,
    });
    let mut dispatcher: A11yActionDispatcher =
        serde_json::from_value(value).expect("dispatcher deserializes with over-cap history");

    dispatcher
        .dispatch(A11yActionEvent::press("button"))
        .expect("valid event dispatches");

    assert_eq!(dispatcher.events().len(), A11Y_ACTION_EVENT_HISTORY_LIMIT);
}

#[test]
fn a11y_action_dispatch_error_converts_to_shared_diagnostic() {
    let tree = A11yTree::new(A11yNode::new("root", A11yRole::Window));
    let mut dispatcher = A11yActionDispatcher::new(tree);
    let error = dispatcher
        .dispatch(A11yActionEvent::focus("missing"))
        .expect_err("missing action target is rejected");
    let diagnostic = Diagnostic::from(error);

    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(diagnostic.rule.as_str(), "a11y.action-target-missing");
    assert!(diagnostic.message.contains("missing"));
}

use hawk2ui_a11y::{A11yHostExporter, A11yHostSurfaceKind, LayoutGeometryUpdate};

#[test]
fn host_export_updates_bounds_from_layout_geometry() {
    let tree = A11yTree::new(A11yNode::new("button", A11yRole::Button).name("Render"));
    let mut exporter = A11yHostExporter::desktop(tree);

    exporter
        .apply_geometry(&LayoutGeometryUpdate::new(
            "button",
            A11yBounds::new(10.0, 12.0, 80.0, 24.0),
        ))
        .unwrap();

    assert_eq!(exporter.surface_kind, A11yHostSurfaceKind::Desktop);
    let button_width = exporter
        .tree()
        .find("button")
        .unwrap()
        .bounds
        .unwrap()
        .width;
    assert!((button_width - 80.0).abs() < f64::EPSILON);
    assert!(exporter.export_snapshot().platform_services_enabled);
}

#[test]
fn host_export_errors_convert_to_shared_diagnostics() {
    let tree = A11yTree::new(A11yNode::new("button", A11yRole::Button).name("Render"));
    let mut exporter = A11yHostExporter::desktop(tree);

    let missing_geometry = exporter
        .apply_geometry(&LayoutGeometryUpdate::new(
            "missing",
            A11yBounds::new(10.0, 12.0, 80.0, 24.0),
        ))
        .expect_err("missing accessibility geometry target must fail");
    let diagnostic = Diagnostic::from(missing_geometry);

    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(diagnostic.rule.as_str(), "a11y.geometry-node-missing");
    assert!(diagnostic.message.contains("missing"));

    let invalid_bounds = A11yHostExporter::desktop(A11yTree::new(
        A11yNode::new("root", A11yRole::Window).bounds(A11yBounds::new(0.0, 0.0, -1.0, 1.0)),
    ))
    .export_accesskit_update()
    .expect_err("negative accessibility bounds must fail");
    let diagnostic = Diagnostic::from(invalid_bounds);

    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(diagnostic.rule.as_str(), "a11y.accesskit.invalid-bounds");
}

#[test]
fn host_export_records_plugin_editor_accessibility_availability() {
    let tree = A11yTree::new(A11yNode::new("editor", A11yRole::Panel).name("Plugin Editor"));
    let exporter = A11yHostExporter::plugin_editor(tree, true);

    let snapshot = exporter.export_snapshot();
    assert_eq!(snapshot.surface_kind, A11yHostSurfaceKind::PluginEditor);
    assert!(snapshot.plugin_accessibility_available);
    assert!(!snapshot.platform_services_enabled);
}

#[test]
fn host_export_builds_accesskit_tree_update() {
    let tree = A11yTree::new(
        A11yNode::new("root", A11yRole::Window)
            .name("Main Window")
            .bounds(A11yBounds::new(0.0, 0.0, 800.0, 600.0))
            .child(
                A11yNode::new("gain", A11yRole::Slider)
                    .name("Gain")
                    .value("-6 dB")
                    .numeric_value(A11yNumericValue::new(-6.0).min(-60.0).max(12.0).step(0.5))
                    .focused(true)
                    .bounds(A11yBounds::new(20.0, 20.0, 120.0, 32.0))
                    .action(A11yAction::Increment)
                    .action(A11yAction::Decrement),
            )
            .child(
                A11yNode::new("presets", A11yRole::List)
                    .name("Presets")
                    .size_of_set(2)
                    .child(
                        A11yNode::new("preset-a", A11yRole::ListItem)
                            .name("A")
                            .position_in_set(1)
                            .size_of_set(2),
                    ),
            ),
    );
    let export = A11yHostExporter::desktop(tree)
        .export_accesskit_update()
        .expect("accesskit tree export succeeds");
    let root_id = export.node_id("root").expect("root id exported");
    let gain_id = export.node_id("gain").expect("gain id exported");
    let root = export
        .update
        .nodes
        .iter()
        .find(|(id, _)| *id == root_id)
        .map(|(_, node)| node)
        .expect("root node exported");
    let gain = export
        .update
        .nodes
        .iter()
        .find(|(id, _)| *id == gain_id)
        .map(|(_, node)| node)
        .expect("gain node exported");
    let presets = export
        .update
        .nodes
        .iter()
        .find(|(id, _)| *id == export.node_id("presets").unwrap())
        .map(|(_, node)| node)
        .expect("presets node exported");
    let preset_a = export
        .update
        .nodes
        .iter()
        .find(|(id, _)| *id == export.node_id("preset-a").unwrap())
        .map(|(_, node)| node)
        .expect("preset node exported");

    assert_eq!(export.update.focus, gain_id);
    assert_eq!(root.role(), accesskit::Role::Window);
    assert_eq!(
        root.children(),
        &[gain_id, export.node_id("presets").unwrap()]
    );
    assert_eq!(gain.role(), accesskit::Role::Slider);
    assert_eq!(gain.label(), Some("Gain"));
    assert_eq!(gain.value(), Some("-6 dB"));
    assert_eq!(gain.numeric_value(), Some(-6.0));
    assert_eq!(gain.min_numeric_value(), Some(-60.0));
    assert_eq!(gain.max_numeric_value(), Some(12.0));
    assert_eq!(gain.numeric_value_step(), Some(0.5));
    assert!(gain.supports_action(accesskit::Action::Increment));
    assert!(gain.supports_action(accesskit::Action::Decrement));
    assert_eq!(presets.size_of_set(), Some(2));
    assert_eq!(preset_a.position_in_set(), Some(1));
    assert_eq!(preset_a.size_of_set(), Some(2));
    assert_eq!(
        gain.bounds().expect("bounds exported"),
        accesskit::Rect::new(20.0, 20.0, 140.0, 52.0)
    );
}

#[test]
fn host_export_rejects_invalid_accesskit_ids_and_excessive_depth() {
    let empty_id = A11yHostExporter::desktop(A11yTree::new(A11yNode::new("", A11yRole::Window)))
        .export_accesskit_update()
        .expect_err("empty accessibility identifiers must fail");
    let duplicate_id = A11yHostExporter::desktop(A11yTree::new(
        A11yNode::new("root", A11yRole::Window)
            .child(A11yNode::new("duplicate", A11yRole::Button))
            .child(A11yNode::new("duplicate", A11yRole::Button)),
    ))
    .export_accesskit_update()
    .expect_err("duplicate accessibility identifiers must fail");
    let mut root = A11yNode::new("node-0", A11yRole::Window);
    for depth in (1..=(A11Y_MAX_TREE_DEPTH + 1)).rev() {
        root = A11yNode::new(format!("node-{depth}"), A11yRole::Panel).child(root);
    }
    let too_deep = A11yHostExporter::desktop(A11yTree::new(root))
        .export_accesskit_update()
        .expect_err("excessively deep accessibility trees must fail");
    let mut wide_root = A11yNode::new("wide-root", A11yRole::Window);
    for index in 0..A11Y_MAX_TREE_NODES {
        wide_root = wide_root.child(A11yNode::new(
            format!("wide-child-{index}"),
            A11yRole::Panel,
        ));
    }
    let too_wide = A11yHostExporter::desktop(A11yTree::new(wide_root))
        .export_accesskit_update()
        .expect_err("excessively wide accessibility trees must fail");

    assert_eq!(empty_id.rule, "a11y.accesskit.invalid-id");
    assert_eq!(duplicate_id.rule, "a11y.accesskit.duplicate-id");
    assert_eq!(too_deep.rule, "a11y.accesskit.max-depth");
    assert_eq!(too_wide.rule, "a11y.accesskit.max-nodes");
}

#[test]
fn host_export_rejects_multiple_focused_accesskit_nodes() {
    let tree = A11yTree::new(
        A11yNode::new("root", A11yRole::Window)
            .child(A11yNode::new("gain", A11yRole::Slider).focused(true))
            .child(A11yNode::new("mix", A11yRole::Slider).focused(true)),
    );

    let error = A11yHostExporter::desktop(tree)
        .export_accesskit_update()
        .expect_err("AccessKit export must not silently choose between focused nodes");

    assert_eq!(error.rule, "a11y.accesskit.multiple-focused-nodes");
}

use hawk2ui_a11y::{A11yPluginGuard, A11yPluginOperation, A11yThreadContext};

#[test]
fn plugin_accessibility_safety_denies_audio_thread_and_unstable_host_calls() {
    let guard = A11yPluginGuard;

    let audio = guard
        .ensure_allowed(
            A11yThreadContext::AudioThread,
            A11yPluginOperation::TreeUpdate,
        )
        .expect_err("audio thread must not update accessibility");
    let unstable = guard
        .ensure_allowed(
            A11yThreadContext::UiThread,
            A11yPluginOperation::UnstableHostCall,
        )
        .expect_err("unstable host calls must be denied");

    assert_eq!(audio.code, "a11y.plugin-audio-thread-denied");
    assert_eq!(unstable.code, "a11y.plugin-unstable-host-call-denied");
}

#[test]
fn plugin_accessibility_safety_allows_safe_editor_updates() {
    let guard = A11yPluginGuard;

    guard
        .ensure_allowed(A11yThreadContext::UiThread, A11yPluginOperation::TreeUpdate)
        .expect("UI thread tree updates should be allowed");
    guard
        .ensure_allowed(
            A11yThreadContext::UiThread,
            A11yPluginOperation::FocusUpdate,
        )
        .expect("UI thread focus updates should be allowed");
}

use hawk2ui_a11y::{
    A11yAction, A11yActionDispatcher, A11yActionEvent, A11yBounds, A11yNode, A11yRole, A11yTree,
    CheckedState,
};
use hawk2ui_api::{Diagnostic, DiagnosticSeverity};
use serde::{Serialize, de::DeserializeOwned};

fn assert_serde<T: Serialize + DeserializeOwned>() {}

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
    assert_serde::<A11yTree>();
    assert_serde::<A11yNode>();
    assert_serde::<A11yBounds>();
    assert_serde::<A11yAction>();
    assert_serde::<A11yRole>();
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
        A11yNode::new("gain", A11yRole::Slider)
            .name("Gain")
            .value("0.5")
            .action(A11yAction::Focus)
            .action(A11yAction::Press)
            .action(A11yAction::Increment)
            .action(A11yAction::Decrement)
            .action(A11yAction::SetValue("0.75".into()))
            .action(A11yAction::Custom("reset".into())),
    );
    let mut dispatcher = A11yActionDispatcher::new(tree);

    dispatcher.dispatch(A11yActionEvent::focus("gain")).unwrap();
    dispatcher.dispatch(A11yActionEvent::press("gain")).unwrap();
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

    assert!(dispatcher.tree().find("gain").unwrap().focused);
    assert_eq!(
        dispatcher.tree().find("gain").unwrap().value.as_deref(),
        Some("0.75")
    );
    assert_eq!(dispatcher.events().len(), 6);
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
        .apply_geometry(LayoutGeometryUpdate::new(
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
        .apply_geometry(LayoutGeometryUpdate::new(
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
                    .focused(true)
                    .bounds(A11yBounds::new(20.0, 20.0, 120.0, 32.0))
                    .action(A11yAction::Increment)
                    .action(A11yAction::Decrement),
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

    assert_eq!(export.update.focus, gain_id);
    assert_eq!(root.role(), accesskit::Role::Window);
    assert_eq!(root.children(), &[gain_id]);
    assert_eq!(gain.role(), accesskit::Role::Slider);
    assert_eq!(gain.label(), Some("Gain"));
    assert_eq!(gain.value(), Some("-6 dB"));
    assert!(gain.supports_action(accesskit::Action::Increment));
    assert!(gain.supports_action(accesskit::Action::Decrement));
    assert_eq!(
        gain.bounds().expect("bounds exported"),
        accesskit::Rect::new(20.0, 20.0, 140.0, 52.0)
    );
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

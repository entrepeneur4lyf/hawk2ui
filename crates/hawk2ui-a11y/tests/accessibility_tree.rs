use hawk2ui_a11y::{A11yAction, A11yBounds, A11yNode, A11yRole, A11yTree, CheckedState};
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
    assert_eq!(tree.root.children[0].bounds.unwrap().width, 120.0);
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

use hawk2ui_a11y::{A11yActionDispatcher, A11yActionEvent};

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
    assert_eq!(
        exporter
            .tree()
            .find("button")
            .unwrap()
            .bounds
            .unwrap()
            .width,
        80.0
    );
    assert!(exporter.export_snapshot().platform_services_enabled);
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

use std::{fs, path::PathBuf};

use hawk2ui_build::{HawkManifest, PackageTarget};
use hawk2ui_plugin::{AutomationEvent, AutomationEventKind, AutomationOrigin, AutomationSequence};
use hawk2ui_render::{
    GradientLayer, LayerKind, LayerStack, PaintLayer, RoundedRect, SceneGraph, SceneNode,
    SceneNodeId, TextLayer, export_paint_commands,
};
use hawk2ui_runtime::{RuntimeEvent, RuntimeScheduler, TimerJob};

fn workspace_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

#[test]
fn source_to_render_compiles_manifest_authoring_scene_runtime_and_plugin_paths() {
    let manifest_source = fs::read_to_string(workspace_path("examples/plugin-basic/hawk.json"))
        .expect("plugin manifest fixture");
    let manifest = HawkManifest::parse(&manifest_source).expect("plugin manifest parses");

    assert!(manifest.has_target(PackageTarget::Plugin));
    assert_eq!(manifest.parameters.len(), 2);

    let authoring_source =
        fs::read_to_string(workspace_path("fixtures/authoring/basic_component.hawk"))
            .expect("authoring fixture");
    let mut diagnostics = Vec::new();
    let authoring =
        hawk2ui_authoring::compile_authoring_source(&authoring_source, &mut diagnostics);

    assert!(diagnostics.is_empty());
    assert_eq!(authoring.components().len(), 1);

    let component = &authoring.components()[0];
    let graph = SceneGraph::new(SceneNode::new(SceneNodeId::new("root")))
        .with_child(
            SceneNodeId::new("root"),
            SceneNode::new(SceneNodeId::new(component.id().as_str())),
        )
        .expect("component scene child");

    assert!(
        graph
            .node(&SceneNodeId::new(component.id().as_str()))
            .is_some()
    );

    let layers = LayerStack::new()
        .with_layer(PaintLayer::new(
            "background",
            10,
            LayerKind::RoundedRect(RoundedRect::new(12.0)),
        ))
        .with_layer(PaintLayer::new(
            "accent",
            20,
            LayerKind::Gradient(GradientLayer::linear()),
        ))
        .with_layer(PaintLayer::new(
            "title",
            30,
            LayerKind::Text(TextLayer::new(component.component_name())),
        ));
    let paint_commands = export_paint_commands(&layers).expect("layer stack is valid");

    assert_eq!(
        paint_commands.serialize_stable(),
        "draw-rounded-rect:background:12|draw-gradient:accent:linear|draw-text:title:CounterCard"
    );

    let mut scheduler = RuntimeScheduler::default();
    scheduler.schedule_ui_event(RuntimeEvent::ui("root", "click"));
    scheduler.invalidate_render("root");
    scheduler.schedule_timer(TimerJob::new("debounce", 32));
    let batch = scheduler.drain_batch().expect("runtime batch");

    assert_eq!(batch.ui_events[0].name, "click");
    assert_eq!(batch.render_invalidations, vec!["root"]);
    assert_eq!(batch.timers[0].id, "debounce");

    let mut automation = AutomationSequence::default();
    automation
        .push(AutomationEvent::begin_gesture("gain", AutomationOrigin::Ui))
        .expect("begin automation");
    automation
        .push(AutomationEvent::value_change(
            "gain",
            AutomationOrigin::Ui,
            0.75,
        ))
        .expect("change automation");
    automation
        .push(AutomationEvent::end_gesture("gain", AutomationOrigin::Ui))
        .expect("end automation");

    assert_eq!(
        automation.events()[0].kind,
        AutomationEventKind::BeginGesture
    );
    assert_eq!(automation.events()[1].normalized_value, Some(0.75));
}

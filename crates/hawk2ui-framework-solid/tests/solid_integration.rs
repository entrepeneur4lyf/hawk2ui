use hawk2ui_authoring::{
    AssetRef, ElementKind, EventKind, EventPayloadField, FrameworkDynamicBinding,
    FrameworkNativeNode, FrameworkNativeProgram, FrameworkReactiveBinding, HandlerRef,
    NativeLifecycleEvent, NativeRef, PointerEventKind, PropValue, StyleRef,
};
use hawk2ui_framework_solid::{SolidComponentSource, SolidIntegration};
use hawk2ui_layout::Viewport;
use hawk2ui_render::Color;
use hawk2ui_runtime::{RuntimeDrawCommand, RuntimeSceneBridge, RuntimeViewId};
use hawk2ui_style::{TokenSet, compile_style_source};

#[test]
fn solid_renderer_rejects_raw_source_without_compiler_artifact() {
    let error = SolidIntegration::new()
        .render(SolidComponentSource::new(
            "src/App.tsx",
            r#"<hawk-view id="root"><hawk-text id="title">Title</hawk-text></hawk-view>"#,
        ))
        .expect_err("raw Solid source must not be substring-scanned in production");

    assert_eq!(
        error.diagnostics()[0].rule.as_str(),
        "solid.compiler-artifact.required"
    );
}

#[test]
fn solid_renderer_accepts_versioned_compiler_json_artifact() {
    let source = SolidComponentSource::from_compiler_json(
        "src/App.tsx",
        r#"
{
  "schema_version": 1,
  "root": {
    "id": "root",
    "kind": "view",
    "refs": ["root_ref"],
    "style_refs": ["surface.card"],
    "asset_refs": [{ "name": "solid.asset", "path": "assets/logo.svg" }],
    "events": [{
      "kind": "pointer.press",
      "handler": "handlePress",
      "payload_fields": ["position"]
    }],
    "lifecycle": [
      { "event": "mounted", "handler": "onMount" },
      { "event": "unmounted", "handler": "onCleanup" }
    ],
    "children": [{
      "key": "title",
      "node": {
        "id": "title",
        "kind": "text",
        "key": "title",
        "props": [
          { "name": "text", "value": { "type": "string", "value": "Compiled Title" } },
          { "name": "font_size", "value": { "type": "number", "value": 18.0 } }
        ]
      }
    }]
  },
  "dynamic_bindings": [{
    "node_id": "title",
    "target": { "type": "prop", "name": "text" },
    "expression": "label()",
    "dependencies": ["label"]
  }],
  "reactivity": [
    { "kind": "signal", "name": "params" },
    { "kind": "keyed-for-each", "name": "params" },
    { "kind": "effect", "name": "meter-paint" }
  ]
}
"#,
    )
    .expect("Solid compiler JSON should parse");

    let artifact = SolidIntegration::new()
        .render_to_runtime(source)
        .expect("Solid compiler JSON should render through runtime");

    assert_eq!(artifact.rendered().keyed_children(), ["title"]);
    assert_eq!(
        artifact
            .dynamic_bindings()
            .iter()
            .map(FrameworkDynamicBinding::stable_key)
            .collect::<Vec<_>>(),
        ["title:prop:text=label()"]
    );
    assert_eq!(artifact.metadata_for("root").unwrap().refs(), ["root_ref"]);
    assert_eq!(
        artifact.rendered().fine_grained_updates(),
        [
            "signal:params",
            "for-each:keyed:params",
            "effect:meter-paint"
        ]
    );
    assert!(
        artifact
            .operation_keys()
            .contains(&"mount-element:root".into())
    );
}

#[test]
fn solid_renderer_maps_fine_grained_updates_lifecycle_keyed_children_events_refs_styles_assets_and_source_maps()
 {
    let source = SolidComponentSource::from_native_program(
        "examples/frameworks/solid-basic/src/App.tsx",
        framework_native_program_with_children(
            "solid.asset",
            "onMount",
            "onCleanup",
            &["surface.card", "intent.primary"],
            &["root_ref"],
            &[("title", "title"), ("cta", "cta"), ("meter", "meter")],
            false,
        )
        .with_reactive_binding(FrameworkReactiveBinding::signal("items"))
        .with_reactive_binding(FrameworkReactiveBinding::keyed_for_each("items"))
        .with_reactive_binding(FrameworkReactiveBinding::effect("root-props")),
    );

    let artifact = SolidIntegration::new()
        .render(source)
        .expect("valid Solid source should render");

    assert_eq!(artifact.framework(), "solid");
    assert_eq!(artifact.framework_version_requirement(), ">=1");
    assert_eq!(artifact.root().id().as_str(), "root");
    assert_eq!(artifact.root().kind(), ElementKind::View);
    assert_eq!(artifact.keyed_children(), ["title", "cta", "meter"]);
    assert_eq!(artifact.refs(), ["root_ref"]);
    assert_eq!(artifact.style_refs(), ["surface.card", "intent.primary"]);
    assert_eq!(artifact.asset_refs()[0].path(), "assets/logo.svg");
    assert_eq!(artifact.events()[0].event().stable_key(), "pointer.press");
    assert_eq!(
        artifact.events()[0].payload_fields(),
        &[EventPayloadField::Position]
    );
    assert_eq!(
        artifact.lifecycle_handlers(),
        ["mounted:onMount", "unmounted:onCleanup"]
    );
    assert_eq!(
        artifact.fine_grained_updates(),
        ["signal:items", "for-each:keyed:items", "effect:root-props"]
    );
    assert_eq!(
        artifact.source_map().author_file(),
        "examples/frameworks/solid-basic/src/App.tsx"
    );
}

#[test]
fn solid_renderer_accepts_explicit_native_compiler_boundary_without_source_scanning() {
    let source = SolidComponentSource::from_native_program(
        "examples/frameworks/solid-basic/src/App.tsx",
        framework_native_program("solid.asset", "onCleanup"),
    );

    let artifact = SolidIntegration::new()
        .render(source.clone())
        .expect("explicit Solid native compiler output should render");
    let runtime = SolidIntegration::new()
        .render_to_runtime(source)
        .expect("explicit Solid native compiler output should bridge");

    assert_eq!(artifact.root().id().as_str(), "root");
    assert_eq!(artifact.keyed_children(), ["title"]);
    assert_eq!(artifact.refs(), ["root_ref"]);
    assert_eq!(artifact.style_refs(), ["surface.card"]);
    assert_eq!(artifact.asset_refs()[0].path(), "assets/logo.svg");
    assert_eq!(artifact.events()[0].event().stable_key(), "pointer.press");
    assert_eq!(
        artifact.lifecycle_handlers(),
        ["mounted:onMount", "unmounted:onCleanup"]
    );
    assert_eq!(
        artifact.fine_grained_updates(),
        [
            "signal:params",
            "for-each:keyed:params",
            "effect:meter-paint"
        ]
    );
    assert!(
        runtime
            .operation_keys()
            .contains(&"mount-element:root".into())
    );
}

#[test]
fn solid_renderer_reports_author_source_diagnostics() {
    let source = SolidComponentSource::new(
        "src/Broken.tsx",
        "<hawk-view data-asset=\"https://example.invalid/logo.svg\"><Missing /></hawk-view>",
    );

    let error = SolidIntegration::new()
        .render(source)
        .expect_err("raw Solid source should fail before source-level diagnostics");
    let rules: Vec<_> = error
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.rule.as_str())
        .collect();

    assert_eq!(rules, ["solid.compiler-artifact.required"]);
    assert_eq!(error.source_map().author_file(), "src/Broken.tsx");
}

#[test]
fn solid_renderer_rejects_duplicate_static_child_keys() {
    let source = SolidComponentSource::from_native_program(
        "src/DuplicateKeys.tsx",
        duplicate_child_key_program(),
    );

    let error = SolidIntegration::new()
        .render(source)
        .expect_err("duplicate keyed compiler output should fail");

    assert!(error.diagnostics().iter().any(|diagnostic| {
        diagnostic.rule.as_str() == "solid.custom-renderer.failed"
            && diagnostic
                .message
                .contains("custom-renderer.child-key.duplicate")
    }));
}

#[test]
fn solid_renderer_bridges_to_runtime_tree() {
    let source = SolidComponentSource::from_native_program(
        "examples/frameworks/solid-basic/src/App.tsx",
        framework_native_program_with_children(
            "solid.asset",
            "onMount",
            "onCleanup",
            &["surface.card", "intent.primary"],
            &["root_ref"],
            &[("title", "title"), ("cta", "cta"), ("meter", "meter")],
            true,
        ),
    );

    let artifact = SolidIntegration::new()
        .render_to_runtime(source)
        .expect("valid Solid source should bridge to runtime");

    assert_eq!(artifact.rendered().framework(), "solid");
    assert_eq!(artifact.runtime_tree().root_id().as_str(), "root");
    assert_eq!(
        artifact
            .runtime_tree()
            .children_of(&RuntimeViewId::new("root"))
            .iter()
            .map(RuntimeViewId::as_str)
            .collect::<Vec<_>>(),
        vec!["title", "cta", "meter"]
    );
    assert_eq!(artifact.metadata_for("root").unwrap().refs(), ["root_ref"]);
    assert_eq!(
        artifact.metadata_for("root").unwrap().style_refs(),
        ["surface.card", "intent.primary"]
    );
    assert_eq!(
        artifact.metadata_for("root").unwrap().asset_paths(),
        ["assets/logo.svg"]
    );
    assert!(
        artifact
            .operation_keys()
            .contains(&"bind-event:root:pointer.press".to_string())
    );
}

#[test]
fn solid_renderer_preserves_static_text_children() {
    let source = SolidComponentSource::from_native_program(
        "examples/frameworks/solid-basic/src/App.tsx",
        static_framework_native_program(&[], &[], &[("title", "Static Title")], &[]),
    );

    let artifact = SolidIntegration::new()
        .render_to_runtime(source)
        .expect("valid Solid source should bridge to runtime");

    assert_eq!(artifact.rendered().keyed_children(), ["title"]);
    assert_eq!(
        artifact
            .runtime_tree()
            .children_of(&RuntimeViewId::new("root"))
            .iter()
            .map(RuntimeViewId::as_str)
            .collect::<Vec<_>>(),
        vec!["title"]
    );
    let frame = RuntimeSceneBridge::new(Viewport::new(160.0, 80.0))
        .build(artifact.runtime_tree())
        .expect("runtime scene builds");
    assert!(frame.draw_commands().iter().any(|command| {
        matches!(
            command,
            RuntimeDrawCommand::Text {
                id,
                text,
                ..
            } if id.as_str() == "title" && text == "Static Title"
        )
    }));
}

#[test]
fn solid_render_to_runtime_with_styles_applies_compiled_root_background() {
    let source = SolidComponentSource::from_native_program(
        "examples/frameworks/solid-basic/src/App.tsx",
        static_framework_native_program(&["surface"], &[], &[], &[]),
    );
    let sheet = compile_style_source(".surface { background-color: token(color.surface); }")
        .expect("style source compiles");
    let tokens = TokenSet::production().with_color("color.surface", 240, 88, 40, 255);

    let artifact = SolidIntegration::new()
        .render_to_runtime_with_styles(source, &sheet, &tokens)
        .expect("valid Solid source should bridge with styles");
    let frame = RuntimeSceneBridge::new(Viewport::new(160.0, 80.0))
        .build(artifact.runtime_tree())
        .expect("runtime scene builds");

    assert!(frame.draw_commands().iter().any(|command| {
        matches!(
            command,
            RuntimeDrawCommand::Fill {
                id,
                color,
                ..
            } if id.as_str() == "root" && *color == Color::rgba(240, 88, 40, 255)
        )
    }));
}

#[test]
fn solid_render_to_runtime_with_theme_applies_theme_background() {
    let source = SolidComponentSource::from_native_program(
        "examples/frameworks/solid-basic/src/App.tsx",
        static_framework_native_program(&["surface"], &[], &[], &[]),
    );
    let sheet = compile_style_source(".surface { background-color: token(color.surface); }")
        .expect("style source compiles");
    let tokens = TokenSet::production()
        .with_color("color.surface", 8, 10, 14, 255)
        .with_theme(hawk2ui_style::ThemeVariant::new("light").with_token(
            "color.surface",
            hawk2ui_style::TokenValue::ColorRgba(245, 243, 238, 255),
        ));

    let artifact = SolidIntegration::new()
        .render_to_runtime_with_theme(source, &sheet, &tokens, "light")
        .expect("valid Solid source should bridge with themed styles");
    let frame = RuntimeSceneBridge::new(Viewport::new(160.0, 80.0))
        .build(artifact.runtime_tree())
        .expect("runtime scene builds");

    assert!(frame.draw_commands().iter().any(|command| {
        matches!(
            command,
            RuntimeDrawCommand::Fill {
                id,
                color,
                ..
            } if id.as_str() == "root" && *color == Color::rgba(245, 243, 238, 255)
        )
    }));
}

#[test]
fn solid_smoke_app_declares_public_package_entrypoint() {
    let package_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("packages/hawk2ui-solid");
    let example_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("examples/frameworks/solid-basic");

    let package_json = std::fs::read_to_string(package_root.join("package.json"))
        .expect("Solid package manifest should exist");
    let index_ts = std::fs::read_to_string(package_root.join("src/index.ts"))
        .expect("Solid package entrypoint should exist");
    let app = std::fs::read_to_string(example_root.join("src/App.tsx"))
        .expect("Solid smoke app should exist");

    assert!(package_json.contains("@hawk2ui/solid"));
    assert!(index_ts.contains("compileHawkSolid"));
    assert!(app.contains("<For each={items()}"));
    assert!(app.contains("assets/logo.svg"));
}

#[test]
fn solid_source_path_lifecycle_handlers_track_declared_hooks() {
    let none = SolidIntegration::new()
        .render(SolidComponentSource::from_native_program(
            "src/NoLifecycle.tsx",
            static_framework_native_program(&[], &[], &[], &[]),
        ))
        .expect("compiler output without lifecycle hooks should render");
    assert!(none.lifecycle_handlers().is_empty());
    assert!(
        !none
            .events()
            .iter()
            .any(|event| matches!(event.event(), EventKind::Lifecycle(_)))
    );

    let mount_only = SolidIntegration::new()
        .render(SolidComponentSource::from_native_program(
            "src/MountOnly.tsx",
            static_framework_native_program(&[], &[], &[], &[("mounted", "onMount")]),
        ))
        .expect("compiler output with a single lifecycle hook should render");
    assert_eq!(mount_only.lifecycle_handlers(), ["mounted:onMount"]);
}

#[test]
fn solid_reports_unsupported_event() {
    let error = SolidIntegration::new()
        .render(SolidComponentSource::new(
            "src/Unsupported.tsx",
            r#"<hawk-view id="root" onClick={handleClick}></hawk-view>"#,
        ))
        .expect_err("raw Solid source should require a compiler artifact");
    assert!(
        error
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.rule.as_str() == "solid.compiler-artifact.required")
    );
}

fn framework_native_program(asset_name: &str, unmounted: &str) -> FrameworkNativeProgram {
    framework_native_program_with_children(
        asset_name,
        "onMount",
        unmounted,
        &["surface.card"],
        &["root_ref"],
        &[("title", "Boundary Title")],
        true,
    )
}

fn framework_native_program_with_children(
    asset_name: &str,
    mounted: &str,
    unmounted: &str,
    styles: &[&str],
    refs: &[&str],
    children: &[(&str, &str)],
    include_reactivity: bool,
) -> FrameworkNativeProgram {
    let mut root = FrameworkNativeNode::new("root", ElementKind::View)
        .with_asset(AssetRef::new(asset_name, "assets/logo.svg"))
        .with_event(
            EventKind::Pointer(PointerEventKind::Press),
            HandlerRef::new("handlePress"),
            [EventPayloadField::Position],
        )
        .with_lifecycle(NativeLifecycleEvent::Mounted, HandlerRef::new(mounted))
        .with_lifecycle(NativeLifecycleEvent::Unmounted, HandlerRef::new(unmounted));
    for style in styles {
        root = root.with_style(StyleRef::new(*style));
    }
    for reference in refs {
        root = root.with_ref(NativeRef::new(*reference));
    }
    for (child_id, text) in children {
        root = root.with_child(
            *child_id,
            FrameworkNativeNode::new(*child_id, ElementKind::Text)
                .with_key(*child_id)
                .with_prop("text", PropValue::String((*text).to_string()))
                .with_prop("font_size", PropValue::Number(18.0)),
        );
    }
    let program = FrameworkNativeProgram::new(root);
    if include_reactivity {
        program
            .with_reactive_binding(FrameworkReactiveBinding::signal("params"))
            .with_reactive_binding(FrameworkReactiveBinding::keyed_for_each("params"))
            .with_reactive_binding(FrameworkReactiveBinding::effect("meter-paint"))
    } else {
        program
    }
}

fn static_framework_native_program(
    styles: &[&str],
    refs: &[&str],
    children: &[(&str, &str)],
    lifecycle: &[(&str, &str)],
) -> FrameworkNativeProgram {
    let mut root = FrameworkNativeNode::new("root", ElementKind::View);
    for style in styles {
        root = root.with_style(StyleRef::new(*style));
    }
    for reference in refs {
        root = root.with_ref(NativeRef::new(*reference));
    }
    for (event, handler) in lifecycle {
        let lifecycle_event = match *event {
            "mounted" => NativeLifecycleEvent::Mounted,
            "unmounted" => NativeLifecycleEvent::Unmounted,
            other => panic!("unsupported test lifecycle event `{other}`"),
        };
        root = root.with_lifecycle(lifecycle_event, HandlerRef::new(*handler));
    }
    for (child_id, text) in children {
        root = root.with_child(
            *child_id,
            FrameworkNativeNode::new(*child_id, ElementKind::Text)
                .with_key(*child_id)
                .with_prop("text", PropValue::String((*text).to_string()))
                .with_prop("font_size", PropValue::Number(18.0)),
        );
    }
    FrameworkNativeProgram::new(root)
}

fn duplicate_child_key_program() -> FrameworkNativeProgram {
    FrameworkNativeProgram::new(
        FrameworkNativeNode::new("root", ElementKind::View)
            .with_child(
                "title",
                FrameworkNativeNode::new("title", ElementKind::Text)
                    .with_key("title")
                    .with_prop("text", PropValue::String("A".to_string()))
                    .with_prop("font_size", PropValue::Number(18.0)),
            )
            .with_child(
                "title",
                FrameworkNativeNode::new("duplicate-title", ElementKind::Text)
                    .with_key("title")
                    .with_prop("text", PropValue::String("B".to_string()))
                    .with_prop("font_size", PropValue::Number(18.0)),
            ),
    )
}

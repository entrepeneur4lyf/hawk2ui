use hawk2ui_authoring::{
    AssetRef, ElementKind, EventKind, EventPayloadField, FrameworkDynamicBinding,
    FrameworkNativeNode, FrameworkNativeProgram, HandlerRef, NativeLifecycleEvent, NativeRef,
    PointerEventKind, PropValue, StyleRef,
};
use hawk2ui_framework_react::{ReactElementTree, ReactIntegration};
use hawk2ui_layout::Viewport;
use hawk2ui_render::Color;
use hawk2ui_runtime::{RuntimeDrawCommand, RuntimeSceneBridge, RuntimeViewId};
use hawk2ui_style::{TokenSet, compile_style_source};

#[test]
fn react_19_renderer_rejects_raw_source_without_compiler_artifact() {
    let error = ReactIntegration::new()
        .render(ReactElementTree::new(
            "src/App.tsx",
            r#"<hawk-view id="root"><hawk-text id="title">Title</hawk-text></hawk-view>"#,
        ))
        .expect_err("raw React TSX must not be substring-scanned in production");

    assert_eq!(
        error.diagnostics()[0].rule.as_str(),
        "react.compiler-artifact.required"
    );
}

#[test]
fn react_19_renderer_accepts_versioned_compiler_json_artifact() {
    let tree = ReactElementTree::from_compiler_json(
        "src/App.tsx",
        r#"
{
  "schema_version": 1,
  "root": {
    "id": "root",
    "kind": "view",
    "refs": ["root_ref"],
    "style_refs": ["surface.card"],
    "asset_refs": [{ "name": "react.asset", "path": "assets/logo.svg" }],
    "events": [{
      "kind": "pointer.press",
      "handler": "handlePress",
      "payload_fields": ["position"]
    }],
    "lifecycle": [
      { "event": "mounted", "handler": "onMount" },
      { "event": "unmounted", "handler": "onUnmount" }
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
    "expression": "label",
    "dependencies": ["label"]
  }]
}
"#,
    )
    .expect("React compiler JSON should parse");

    let artifact = ReactIntegration::new()
        .render_to_runtime(tree)
        .expect("React compiler JSON should render through runtime");

    assert_eq!(artifact.rendered().keyed_children(), ["title"]);
    assert_eq!(
        artifact
            .dynamic_bindings()
            .iter()
            .map(FrameworkDynamicBinding::stable_key)
            .collect::<Vec<_>>(),
        ["title:prop:text=label"]
    );
    assert_eq!(artifact.metadata_for("root").unwrap().refs(), ["root_ref"]);
    assert_eq!(
        artifact.metadata_for("root").unwrap().asset_paths(),
        ["assets/logo.svg"]
    );
    assert!(
        artifact
            .operation_keys()
            .contains(&"mount-element:root".into())
    );
}

#[test]
fn react_19_renderer_maps_reconciler_lifecycle_keyed_children_events_refs_styles_assets_and_source_maps()
 {
    let source = ReactElementTree::from_native_program(
        "examples/frameworks/react-basic/src/App.tsx",
        framework_native_program_with_children(
            "react.asset",
            "onMount",
            "onUnmount",
            "root",
            &["surface.card", "intent.primary"],
            &["root_ref"],
            &[("title", "title"), ("cta", "cta"), ("meter", "meter")],
        ),
    );

    let artifact = ReactIntegration::new()
        .render(source)
        .expect("valid React source should render");

    assert_eq!(artifact.framework(), "react");
    assert_eq!(artifact.framework_version_requirement(), ">=19");
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
        ["mounted:onMount", "unmounted:onUnmount"]
    );
    assert_eq!(
        artifact.source_map().author_file(),
        "examples/frameworks/react-basic/src/App.tsx"
    );
    assert_eq!(
        artifact.reconciler_operations(),
        [
            "create-node:root:view",
            "set-style:root:surface.card",
            "set-style:root:intent.primary",
            "set-asset:root:assets/logo.svg",
            "set-ref:root:root_ref",
            "bind-event:root:pointer.press",
            "bind-lifecycle:root:mounted:onMount",
            "bind-lifecycle:root:unmounted:onUnmount",
            "create-node:title:text",
            "set-prop:title:text",
            "set-prop:title:font_size",
            "append:title",
            "create-node:cta:text",
            "set-prop:cta:text",
            "set-prop:cta:font_size",
            "append:cta",
            "create-node:meter:text",
            "set-prop:meter:text",
            "set-prop:meter:font_size",
            "append:meter",
            "commit:root"
        ]
    );
}

#[test]
fn react_19_renderer_accepts_explicit_native_compiler_boundary_without_source_scanning() {
    let tree = ReactElementTree::from_native_program(
        "examples/frameworks/react-basic/src/App.tsx",
        framework_native_program("react.asset", "onUnmount"),
    );

    let artifact = ReactIntegration::new()
        .render(tree.clone())
        .expect("explicit React native compiler output should render");
    let runtime = ReactIntegration::new()
        .render_to_runtime(tree)
        .expect("explicit React native compiler output should bridge");

    assert_eq!(artifact.root().id().as_str(), "root");
    assert_eq!(artifact.keyed_children(), ["title"]);
    assert_eq!(artifact.refs(), ["root_ref"]);
    assert_eq!(artifact.style_refs(), ["surface.card"]);
    assert_eq!(artifact.asset_refs()[0].path(), "assets/logo.svg");
    assert_eq!(artifact.events()[0].event().stable_key(), "pointer.press");
    assert_eq!(
        artifact.lifecycle_handlers(),
        ["mounted:onMount", "unmounted:onUnmount"]
    );
    assert_eq!(
        artifact.reconciler_operations(),
        [
            "create-node:root:view",
            "set-style:root:surface.card",
            "set-asset:root:assets/logo.svg",
            "set-ref:root:root_ref",
            "bind-event:root:pointer.press",
            "bind-lifecycle:root:mounted:onMount",
            "bind-lifecycle:root:unmounted:onUnmount",
            "create-node:title:text",
            "set-prop:title:text",
            "set-prop:title:font_size",
            "append:title",
            "commit:root"
        ]
    );
    assert!(
        runtime
            .operation_keys()
            .contains(&"mount-element:root".into())
    );
}

#[test]
fn react_19_renderer_reports_author_source_diagnostics() {
    let source = ReactElementTree::new(
        "src/Broken.tsx",
        "<hawk-view data-asset=\"https://example.invalid/logo.svg\"><Missing /></hawk-view>",
    );

    let error = ReactIntegration::new()
        .render(source)
        .expect_err("raw React source should fail before source-level diagnostics");
    let rules: Vec<_> = error
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.rule.as_str())
        .collect();

    assert_eq!(rules, ["react.compiler-artifact.required"]);
    assert_eq!(error.source_map().author_file(), "src/Broken.tsx");
}

#[test]
fn react_19_renderer_rejects_duplicate_static_child_keys() {
    let source = ReactElementTree::from_native_program(
        "src/DuplicateKeys.tsx",
        duplicate_child_key_program(),
    );

    let error = ReactIntegration::new()
        .render(source)
        .expect_err("duplicate keyed compiler output should fail");

    assert!(error.diagnostics().iter().any(|diagnostic| {
        diagnostic.rule.as_str() == "react.custom-renderer.failed"
            && diagnostic
                .message
                .contains("custom-renderer.child-key.duplicate")
    }));
}

#[test]
fn react_19_renderer_bridges_to_runtime_tree() {
    let source = ReactElementTree::from_native_program(
        "examples/frameworks/react-basic/src/App.tsx",
        framework_native_program_with_children(
            "react.asset",
            "onMount",
            "onUnmount",
            "root",
            &["surface.card", "intent.primary"],
            &["root_ref"],
            &[("title", "title"), ("cta", "cta"), ("meter", "meter")],
        ),
    );

    let artifact = ReactIntegration::new()
        .render_to_runtime(source)
        .expect("valid React source should bridge to runtime");

    assert_eq!(artifact.rendered().framework(), "react");
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
fn react_19_renderer_preserves_static_text_children() {
    let source = ReactElementTree::from_native_program(
        "examples/frameworks/react-basic/src/App.tsx",
        static_framework_native_program("root", &[], &[], &[("title", "Static Title")], &[]),
    );

    let artifact = ReactIntegration::new()
        .render_to_runtime(source)
        .expect("valid React source should bridge to runtime");

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
fn react_19_render_to_runtime_with_styles_applies_compiled_root_background() {
    let source = ReactElementTree::from_native_program(
        "examples/frameworks/react-basic/src/App.tsx",
        static_framework_native_program("root", &["surface"], &[], &[], &[]),
    );
    let sheet = compile_style_source(".surface { background-color: token(color.surface); }")
        .expect("style source compiles");
    let tokens = TokenSet::production().with_color("color.surface", 240, 88, 40, 255);

    let artifact = ReactIntegration::new()
        .render_to_runtime_with_styles(source, &sheet, &tokens)
        .expect("valid React source should bridge with styles");
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
fn react_19_render_to_runtime_with_theme_applies_theme_background() {
    let source = ReactElementTree::from_native_program(
        "examples/frameworks/react-basic/src/App.tsx",
        static_framework_native_program("root", &["surface"], &[], &[], &[]),
    );
    let sheet = compile_style_source(".surface { background-color: token(color.surface); }")
        .expect("style source compiles");
    let tokens = TokenSet::production()
        .with_color("color.surface", 8, 10, 14, 255)
        .with_theme(hawk2ui_style::ThemeVariant::new("light").with_token(
            "color.surface",
            hawk2ui_style::TokenValue::ColorRgba(245, 243, 238, 255),
        ));

    let artifact = ReactIntegration::new()
        .render_to_runtime_with_theme(source, &sheet, &tokens, "light")
        .expect("valid React source should bridge with themed styles");
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
fn react_smoke_app_declares_public_package_entrypoint() {
    let package_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("packages/hawk2ui-react");
    let example_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("examples/frameworks/react-basic");

    let package_json = std::fs::read_to_string(package_root.join("package.json"))
        .expect("React package manifest should exist");
    let index_ts = std::fs::read_to_string(package_root.join("src/index.ts"))
        .expect("React package entrypoint should exist");
    let app = std::fs::read_to_string(example_root.join("src/App.tsx"))
        .expect("React smoke app should exist");

    assert!(package_json.contains("@hawk2ui/react"));
    assert!(index_ts.contains("createRoot"));
    assert!(app.contains("items.map"));
    assert!(app.contains("assets/logo.svg"));
}

#[test]
fn react_source_path_lifecycle_handlers_track_declared_hooks() {
    let none = ReactIntegration::new()
        .render(ReactElementTree::from_native_program(
            "src/NoLifecycle.tsx",
            static_framework_native_program("root", &[], &[], &[], &[]),
        ))
        .expect("compiler output without lifecycle hooks should render");
    assert!(none.lifecycle_handlers().is_empty());
    assert!(
        !none
            .events()
            .iter()
            .any(|event| matches!(event.event(), EventKind::Lifecycle(_)))
    );

    let mount_only = ReactIntegration::new()
        .render(ReactElementTree::from_native_program(
            "src/MountOnly.tsx",
            static_framework_native_program("root", &[], &[], &[], &[("mounted", "onMount")]),
        ))
        .expect("compiler output with a single lifecycle hook should render");
    assert_eq!(mount_only.lifecycle_handlers(), ["mounted:onMount"]);
}

#[test]
fn react_public_operation_keys_use_actual_root_id() {
    // A non-`"root"` root id must still lower append operations to the public `append:{child}`
    // form rather than leaking the raw internal `append-child:{root}:…` key.
    let artifact = ReactIntegration::new()
        .render(ReactElementTree::from_native_program(
            "src/App.tsx",
            static_framework_native_program("app", &[], &[], &[("title", "Hi")], &[]),
        ))
        .expect("custom-root-id compiler output should render");

    assert_eq!(artifact.root().id().as_str(), "app");
    assert!(
        artifact
            .reconciler_operations()
            .contains(&"append:title".to_string())
    );
    assert!(
        !artifact
            .reconciler_operations()
            .iter()
            .any(|key| key.starts_with("append-child:")),
        "raw internal append-child keys must not leak into reconciler_operations: {:?}",
        artifact.reconciler_operations()
    );
}

#[test]
fn react_reports_unsupported_event() {
    let error = ReactIntegration::new()
        .render(ReactElementTree::new(
            "src/Unsupported.tsx",
            r#"<hawk-view id="root" onClick={handleClick}></hawk-view>"#,
        ))
        .expect_err("raw React source should require a compiler artifact");
    assert!(
        error
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.rule.as_str() == "react.compiler-artifact.required")
    );
}

fn framework_native_program(asset_name: &str, unmounted: &str) -> FrameworkNativeProgram {
    framework_native_program_with_children(
        asset_name,
        "onMount",
        unmounted,
        "root",
        &["surface.card"],
        &["root_ref"],
        &[("title", "Boundary Title")],
    )
}

fn framework_native_program_with_children(
    asset_name: &str,
    mounted: &str,
    unmounted: &str,
    root_id: &str,
    styles: &[&str],
    refs: &[&str],
    children: &[(&str, &str)],
) -> FrameworkNativeProgram {
    let mut root = FrameworkNativeNode::new(root_id, ElementKind::View)
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
    FrameworkNativeProgram::new(root)
}

fn static_framework_native_program(
    root_id: &str,
    styles: &[&str],
    refs: &[&str],
    children: &[(&str, &str)],
    lifecycle: &[(&str, &str)],
) -> FrameworkNativeProgram {
    let mut root = FrameworkNativeNode::new(root_id, ElementKind::View);
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

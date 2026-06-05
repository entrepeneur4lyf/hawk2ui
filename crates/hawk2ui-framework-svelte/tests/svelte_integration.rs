use hawk2ui_authoring::{
    AssetRef, ElementKind, EventKind, EventPayloadField, FrameworkNativeNode,
    FrameworkNativeProgram, HandlerRef, NativeLifecycleEvent, NativeRef, PointerEventKind,
    PropValue, StyleRef,
};
use hawk2ui_framework_svelte::{SvelteComponentSource, SvelteIntegration};
use hawk2ui_layout::Viewport;
use hawk2ui_render::{Color, Geometry, RendererBackend};
use hawk2ui_render_skia::{SkiaFrameSnapshot, SkiaRendererBackend};
use hawk2ui_runtime::{RuntimeDrawCommand, RuntimeSceneBridge, RuntimeSceneFrame, RuntimeViewId};
use hawk2ui_style::{TokenSet, compile_style_source};

#[test]
fn svelte_5_compile_rejects_raw_source_without_compiler_artifact() {
    let error = SvelteIntegration::new()
        .compile(SvelteComponentSource::new(
            "src/App.svelte",
            r#"<hawk-view id="root"><hawk-text id="title">Title</hawk-text></hawk-view>"#,
        ))
        .expect_err("raw Svelte source must not be substring-scanned in production");

    assert_eq!(
        error.diagnostics()[0].rule.as_str(),
        "svelte.compiler-artifact.required"
    );
}

#[test]
fn svelte_5_compile_accepts_versioned_compiler_json_artifact() {
    let source = SvelteComponentSource::from_compiler_json(
        "src/App.svelte",
        r#"
{
  "schema_version": 1,
  "root": {
    "id": "root",
    "kind": "view",
    "refs": ["root_ref"],
    "style_refs": ["surface.card"],
    "asset_refs": [{ "name": "svelte.asset", "path": "assets/logo.svg" }],
    "events": [{
      "kind": "pointer.press",
      "handler": "handlePress",
      "payload_fields": ["position"]
    }],
    "lifecycle": [
      { "event": "mounted", "handler": "onMount" },
      { "event": "unmounted", "handler": "onDestroy" }
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
  }
}
"#,
    )
    .expect("Svelte compiler JSON should parse");

    let artifact = SvelteIntegration::new()
        .compile_to_runtime(source)
        .expect("Svelte compiler JSON should render through runtime");

    assert_eq!(artifact.compiled().keyed_children(), ["title"]);
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
fn svelte_5_compile_maps_lifecycle_keyed_children_events_refs_styles_assets_and_source_maps() {
    let source = SvelteComponentSource::from_native_program(
        "examples/frameworks/svelte-basic/src/App.svelte",
        framework_native_program_with_children(
            "svelte.asset",
            "onDestroy",
            &["surface.card", "intent.primary"],
            &["root_ref"],
            &[("title", "title"), ("cta", "cta"), ("meter", "meter")],
        ),
    );

    let artifact = SvelteIntegration::new()
        .compile(source)
        .expect("valid Svelte source should compile");

    assert_eq!(artifact.framework(), "svelte");
    assert_eq!(artifact.framework_version_requirement(), ">=5");
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
        ["mounted:onMount", "unmounted:onDestroy"]
    );
    assert_eq!(
        artifact.renderer_operations(),
        [
            "create-node:root:view",
            "set-style:root:surface.card",
            "set-style:root:intent.primary",
            "set-asset:root:assets/logo.svg",
            "set-ref:root:root_ref",
            "bind-event:root:pointer.press",
            "bind-lifecycle:root:mounted:onMount",
            "bind-lifecycle:root:unmounted:onDestroy",
            "create-node:title:text",
            "set-prop:title:text",
            "set-prop:title:font_size",
            "append-child:root:title:key:title",
            "create-node:cta:text",
            "set-prop:cta:text",
            "set-prop:cta:font_size",
            "append-child:root:cta:key:cta",
            "create-node:meter:text",
            "set-prop:meter:text",
            "set-prop:meter:font_size",
            "append-child:root:meter:key:meter",
            "commit:root",
        ]
    );
    assert_eq!(
        artifact.source_map().author_file(),
        "examples/frameworks/svelte-basic/src/App.svelte"
    );
    assert_eq!(artifact.diagnostics(), []);
}

#[test]
fn svelte_5_compile_accepts_explicit_native_compiler_boundary_without_source_scanning() {
    let source = SvelteComponentSource::from_native_program(
        "examples/frameworks/svelte-basic/src/App.svelte",
        framework_native_program("svelte.asset", "onDestroy"),
    );

    let artifact = SvelteIntegration::new()
        .compile(source.clone())
        .expect("explicit Svelte native compiler output should compile");
    let runtime = SvelteIntegration::new()
        .compile_to_runtime(source)
        .expect("explicit Svelte native compiler output should bridge");

    assert_eq!(artifact.root().id().as_str(), "root");
    assert_eq!(artifact.keyed_children(), ["title"]);
    assert_eq!(artifact.refs(), ["root_ref"]);
    assert_eq!(artifact.style_refs(), ["surface.card"]);
    assert_eq!(artifact.asset_refs()[0].path(), "assets/logo.svg");
    assert_eq!(artifact.events()[0].event().stable_key(), "pointer.press");
    assert_eq!(
        artifact.lifecycle_handlers(),
        ["mounted:onMount", "unmounted:onDestroy"]
    );
    assert_eq!(
        artifact.renderer_operations(),
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
            "set-prop:title:font_size",
            "append-child:root:title:key:title",
            "commit:root",
        ]
    );
    assert!(
        runtime
            .operation_keys()
            .contains(&"mount-element:root".into())
    );
}

#[test]
fn svelte_5_compile_gates_lifecycle_handlers_and_collects_all_refs() {
    let source = SvelteComponentSource::from_native_program(
        "src/Static.svelte",
        static_framework_native_program(&[], &["root_ref", "panel_ref"], &[("title", "Title")]),
    );

    let artifact = SvelteIntegration::new()
        .compile(source)
        .expect("recognized Hawk source should compile");

    assert_eq!(artifact.refs(), ["root_ref", "panel_ref"]);
    assert!(artifact.events().is_empty());
    assert!(artifact.lifecycle_handlers().is_empty());
    assert!(
        !artifact
            .renderer_operations()
            .iter()
            .any(|operation| operation.contains("bind-lifecycle")),
        "renderer operations must not report lifecycle hooks absent from the source"
    );
}

#[test]
fn svelte_5_compile_rejects_non_hawk_source_and_all_invalid_asset_paths() {
    let error = SvelteIntegration::new()
        .compile(SvelteComponentSource::new(
            "src/Invalid.svelte",
            r#"<div data-asset="assets/logo.svg"></div><hawk-view data-asset="assets/logo.svg" data-asset="..%2Fsecret.svg" data-asset="icons\logo.svg"></hawk-view>"#,
        ))
        .expect_err("raw Svelte source should fail before Rust-side source scanning");
    let rules: Vec<_> = error
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.rule.as_str())
        .collect();

    assert_eq!(rules, ["svelte.compiler-artifact.required"]);

    let no_root = SvelteIntegration::new()
        .compile(SvelteComponentSource::new(
            "src/NoRoot.svelte",
            "<script></script>",
        ))
        .expect_err("raw Svelte source should require a compiler artifact");

    assert_eq!(
        no_root.diagnostics()[0].rule.as_str(),
        "svelte.compiler-artifact.required"
    );
}

#[test]
fn svelte_5_compile_reports_author_source_diagnostics() {
    let source = SvelteComponentSource::new(
        "src/Broken.svelte",
        "<hawk-view data-asset=\"https://example.invalid/logo.svg\"><Broken /></hawk-view>",
    );

    let error = SvelteIntegration::new()
        .compile(source)
        .expect_err("raw Svelte source should fail before source-level diagnostics");
    let rules: Vec<_> = error
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.rule.as_str())
        .collect();

    assert_eq!(rules, ["svelte.compiler-artifact.required"]);
    assert_eq!(error.source_map().author_file(), "src/Broken.svelte");
}

#[test]
fn svelte_5_compile_rejects_duplicate_static_child_keys() {
    let source = SvelteComponentSource::from_native_program(
        "src/DuplicateKeys.svelte",
        duplicate_child_key_program(),
    );

    let error = SvelteIntegration::new()
        .compile(source)
        .expect_err("duplicate keyed compiler output should fail");

    assert!(error.diagnostics().iter().any(|diagnostic| {
        diagnostic.rule.as_str() == "svelte.custom-renderer.failed"
            && diagnostic
                .message
                .contains("custom-renderer.child-key.duplicate")
    }));
}

#[test]
fn svelte_smoke_app_declares_public_package_entrypoint() {
    let package_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("packages/hawk2ui-svelte");
    let example_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("examples/frameworks/svelte-basic");

    let package_json = std::fs::read_to_string(package_root.join("package.json"))
        .expect("Svelte package manifest should exist");
    let index_ts = std::fs::read_to_string(package_root.join("src/index.ts"))
        .expect("Svelte package entrypoint should exist");
    let app = std::fs::read_to_string(example_root.join("src/App.svelte"))
        .expect("Svelte smoke app should exist");

    assert!(package_json.contains("@hawk2ui/svelte"));
    assert!(index_ts.contains("compileHawkSvelte"));
    assert!(app.contains("{#each items as item (item.id)}"));
    assert!(app.contains("assets/logo.svg"));
}

#[test]
fn svelte_5_compile_to_runtime_uses_native_bridge_contract() {
    let source = SvelteComponentSource::from_native_program(
        "examples/frameworks/svelte-basic/src/App.svelte",
        framework_native_program_with_children(
            "svelte.asset",
            "onDestroy",
            &["surface.card", "intent.primary"],
            &["root_ref"],
            &[("title", "title"), ("cta", "cta"), ("meter", "meter")],
        ),
    );

    let artifact = SvelteIntegration::new()
        .compile_to_runtime(source)
        .expect("valid Svelte source should bridge to runtime");

    assert_eq!(artifact.compiled().framework(), "svelte");
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
            .contains(&"mount-element:root".to_string())
    );
    assert!(
        artifact
            .operation_keys()
            .contains(&"bind-event:root:pointer.press".to_string())
    );
}

#[test]
fn svelte_5_compile_to_runtime_preserves_static_text_children() {
    let source = SvelteComponentSource::from_native_program(
        "examples/frameworks/svelte-basic/src/App.svelte",
        static_framework_native_program(&[], &[], &[("title", "Static Title")]),
    );

    let artifact = SvelteIntegration::new()
        .compile_to_runtime(source)
        .expect("valid Svelte source should bridge to runtime");

    assert_eq!(artifact.compiled().keyed_children(), ["title"]);
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
fn svelte_5_runtime_bridge_renders_visible_skia_pixels() {
    let source = SvelteComponentSource::from_native_program(
        "examples/frameworks/svelte-basic/src/App.svelte",
        framework_native_program_with_children(
            "svelte.asset",
            "onDestroy",
            &["surface.card", "intent.primary"],
            &["root_ref"],
            &[("title", "title"), ("cta", "cta"), ("meter", "meter")],
        ),
    );
    let artifact = SvelteIntegration::new()
        .compile_to_runtime(source)
        .expect("valid Svelte source should bridge to runtime");
    let frame = RuntimeSceneBridge::new(Viewport::new(220.0, 120.0))
        .build(artifact.runtime_tree())
        .expect("runtime scene builds");

    let mut backend = SkiaRendererBackend::default();
    backend
        .create_surface("main", 220, 120)
        .expect("surface creates");
    backend.begin_frame("main").expect("frame begins");
    backend
        .clear(Color::rgba(0, 0, 0, 255))
        .expect("surface clears");
    render_runtime_frame_with_skia(&frame, &mut backend);
    backend.end_frame("main").expect("frame ends");

    let snapshot = backend.frame_snapshot("main").expect("snapshot exists");
    assert!(snapshot.pixels().contains(&0x0008_0a0e));
    assert!(
        count_changed_pixels(
            snapshot,
            0x0008_0a0e,
            frame.geometry_for(&RuntimeViewId::new("title")).unwrap()
        ) > 0
    );
}

#[test]
fn svelte_5_compile_to_runtime_with_styles_applies_compiled_root_background() {
    let source = SvelteComponentSource::from_native_program(
        "examples/frameworks/svelte-basic/src/App.svelte",
        static_framework_native_program(&["surface"], &[], &[]),
    );
    let sheet = compile_style_source(".surface { background-color: token(color.surface); }")
        .expect("style source compiles");
    let tokens = TokenSet::production().with_color("color.surface", 240, 88, 40, 255);

    let artifact = SvelteIntegration::new()
        .compile_to_runtime_with_styles(source, &sheet, &tokens)
        .expect("valid Svelte source should bridge with styles");
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
fn svelte_5_compile_to_runtime_with_theme_applies_theme_background() {
    let source = SvelteComponentSource::from_native_program(
        "examples/frameworks/svelte-basic/src/App.svelte",
        static_framework_native_program(&["surface"], &[], &[]),
    );
    let sheet = compile_style_source(".surface { background-color: token(color.surface); }")
        .expect("style source compiles");
    let tokens = TokenSet::production()
        .with_color("color.surface", 8, 10, 14, 255)
        .with_theme(hawk2ui_style::ThemeVariant::new("light").with_token(
            "color.surface",
            hawk2ui_style::TokenValue::ColorRgba(245, 243, 238, 255),
        ));

    let artifact = SvelteIntegration::new()
        .compile_to_runtime_with_theme(source, &sheet, &tokens, "light")
        .expect("valid Svelte source should bridge with themed styles");
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

fn render_runtime_frame_with_skia(frame: &RuntimeSceneFrame, backend: &mut SkiaRendererBackend) {
    backend
        .draw_runtime_scene_frame(frame, 0, 1.0)
        .expect("runtime scene frame renders");
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn count_changed_pixels(
    snapshot: &SkiaFrameSnapshot,
    background: u32,
    geometry: Geometry,
) -> usize {
    let start_x = geometry.x.max(0.0).floor() as u32;
    let start_y = geometry.y.max(0.0).floor() as u32;
    let end_x = (geometry.x + geometry.width)
        .ceil()
        .min(snapshot.width() as f32) as u32;
    let end_y = (geometry.y + geometry.height)
        .ceil()
        .min(snapshot.height() as f32) as u32;
    let mut changed = 0;
    for y in start_y..end_y {
        for x in start_x..end_x {
            if snapshot
                .pixel_at(x, y)
                .is_some_and(|pixel| pixel != background)
            {
                changed += 1;
            }
        }
    }
    changed
}

fn framework_native_program(asset_name: &str, unmounted: &str) -> FrameworkNativeProgram {
    framework_native_program_with_children(
        asset_name,
        unmounted,
        &["surface.card"],
        &["root_ref"],
        &[("title", "Boundary Title")],
    )
}

fn framework_native_program_with_children(
    asset_name: &str,
    unmounted: &str,
    styles: &[&str],
    refs: &[&str],
    children: &[(&str, &str)],
) -> FrameworkNativeProgram {
    let mut root = FrameworkNativeNode::new("root", ElementKind::View)
        .with_asset(AssetRef::new(asset_name, "assets/logo.svg"))
        .with_event(
            EventKind::Pointer(PointerEventKind::Press),
            HandlerRef::new("handlePress"),
            [EventPayloadField::Position],
        )
        .with_lifecycle(NativeLifecycleEvent::Mounted, HandlerRef::new("onMount"))
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
    styles: &[&str],
    refs: &[&str],
    children: &[(&str, &str)],
) -> FrameworkNativeProgram {
    let mut root = FrameworkNativeNode::new("root", ElementKind::View);
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

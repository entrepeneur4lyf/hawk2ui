use hawk2ui_authoring::{
    AssetRef, ElementKind, EventKind, EventPayloadField, NativeAuthoringElement,
    NativeAuthoringRuntime, NativeChild, NativeLifecycleEvent, NativeRef, NativeRuntimeBridge,
    PointerEventKind, PropValue, StyleRef,
};
use hawk2ui_layout::Viewport;
use hawk2ui_render::{Color, Geometry, RendererBackend};
use hawk2ui_render_skia::{SkiaFrameSnapshot, SkiaRendererBackend};
use hawk2ui_runtime::RuntimeViewId;
use hawk2ui_runtime::{RuntimeDrawCommand, RuntimeSceneBridge, RuntimeSceneFrame};
use hawk2ui_style::{TokenSet, compile_style_source};

#[test]
fn native_authoring_runtime_emits_typed_operations_for_lifecycle_children_refs_events_styles_assets_and_diagnostics()
 {
    let mut runtime = NativeAuthoringRuntime::new("native-smoke");

    runtime.mount(
        NativeAuthoringElement::new("root", ElementKind::View)
            .with_prop("role", PropValue::String("main".into()))
            .with_style(StyleRef::new("surface.card"))
            .with_asset(AssetRef::new("hawk.logo", "assets/logo.svg"))
            .with_ref(NativeRef::new("root_ref"))
            .with_child(NativeChild::keyed(
                "title",
                NativeAuthoringElement::new("title", ElementKind::Text)
                    .with_prop("text", PropValue::String("Native Hawk2UI".into())),
            ))
            .with_child(NativeChild::keyed(
                "cta",
                NativeAuthoringElement::new("cta", ElementKind::Button),
            ))
            .with_event(
                EventKind::Pointer(PointerEventKind::Press),
                "handle_press",
                [EventPayloadField::Position],
            )
            .with_lifecycle(NativeLifecycleEvent::Mounted, "on_mount")
            .with_lifecycle(NativeLifecycleEvent::Unmounted, "on_unmount"),
    );

    let artifact = runtime.finish().expect("native authoring should compile");

    assert_eq!(artifact.name(), "native-smoke");
    assert_eq!(artifact.root().id().as_str(), "root");
    assert_eq!(artifact.root().style_refs()[0].name(), "surface.card");
    assert_eq!(artifact.root().asset_refs()[0].path(), "assets/logo.svg");
    assert_eq!(artifact.root().refs()[0].name(), "root_ref");
    assert_eq!(artifact.root().keyed_child_order(), vec!["title", "cta"]);
    assert_eq!(artifact.events().len(), 3);
    assert_eq!(artifact.events()[0].event().stable_key(), "pointer.press");
    assert_eq!(artifact.diagnostics(), []);
    assert_eq!(
        artifact.operation_keys(),
        [
            "lifecycle:mounted:root:on_mount",
            "mount-element:root",
            "mount-element:title",
            "mount-element:cta",
            "bind-event:root:pointer.press",
            "lifecycle:unmounted:root:on_unmount",
        ]
    );
}

#[test]
fn native_authoring_runtime_reports_source_diagnostics_for_duplicate_keys_and_invalid_asset_paths()
{
    let mut runtime = NativeAuthoringRuntime::new("native-invalid");

    runtime.mount(
        NativeAuthoringElement::new("root", ElementKind::View)
            .with_asset(AssetRef::new("remote", "https://example.invalid/logo.svg"))
            .with_child(NativeChild::keyed(
                "duplicate",
                NativeAuthoringElement::new("first", ElementKind::Text),
            ))
            .with_child(NativeChild::keyed(
                "duplicate",
                NativeAuthoringElement::new("second", ElementKind::Text),
            )),
    );

    let error = runtime.finish().expect_err("invalid authoring should fail");
    let rules: Vec<_> = error
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.rule.as_str())
        .collect();

    assert_eq!(
        rules,
        ["native.child-key.duplicate", "native.asset.path-invalid",]
    );
}

#[test]
fn native_authoring_package_smoke_fixture_uses_public_toolchain_shape() {
    let package_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("packages/hawk2ui-native");
    let example_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("examples/frameworks/native-basic");

    let package_json = std::fs::read_to_string(package_root.join("package.json"))
        .expect("native package manifest should exist");
    let index_ts = std::fs::read_to_string(package_root.join("src/index.ts"))
        .expect("native package entrypoint should exist");
    let app_ts = std::fs::read_to_string(example_root.join("src/app.ts"))
        .expect("native framework smoke source should exist");

    assert!(package_json.contains("@hawk2ui/native"));
    assert!(index_ts.contains("createHawkApp"));
    assert!(app_ts.contains("createHawkApp"));
    assert!(app_ts.contains("assets/logo.svg"));
}

#[test]
fn native_authoring_elements_expose_read_only_tree_for_runtime_bridge() {
    let root =
        NativeAuthoringElement::new("root", ElementKind::View).with_child(NativeChild::keyed(
            "title",
            NativeAuthoringElement::new("title", ElementKind::Text)
                .with_prop("text", PropValue::String("Hello".into())),
        ));

    assert_eq!(root.node().id().as_str(), "root");
    assert_eq!(root.children().len(), 1);
    assert_eq!(root.children()[0].key(), Some("title"));
    assert_eq!(root.children()[0].element().node().id().as_str(), "title");
}

#[test]
fn native_runtime_bridge_converts_authoring_artifact_to_runtime_view_tree() {
    let mut runtime = NativeAuthoringRuntime::new("native-runtime");
    runtime.mount(
        NativeAuthoringElement::new("root", ElementKind::View)
            .with_prop("background", PropValue::String("#080a0e".into()))
            .with_style(StyleRef::new("surface.card"))
            .with_asset(AssetRef::new("hawk.logo", "assets/logo.svg"))
            .with_ref(NativeRef::new("root_ref"))
            .with_child(NativeChild::keyed(
                "title",
                NativeAuthoringElement::new("title", ElementKind::Text)
                    .with_prop("text", PropValue::String("Native Runtime".into()))
                    .with_prop("font_size", PropValue::Number(18.0))
                    .with_prop("color", PropValue::String("#f0f5ff".into()))
                    .with_prop("width", PropValue::Number(160.0))
                    .with_prop("height", PropValue::Number(32.0)),
            )),
    );
    let artifact = runtime.finish().expect("authoring finalizes");

    let bridged = NativeRuntimeBridge::new()
        .bridge_artifact(&artifact)
        .expect("authoring artifact bridges to runtime");

    assert_eq!(bridged.runtime_tree().root_id().as_str(), "root");
    assert_eq!(
        bridged
            .runtime_tree()
            .children_of(&RuntimeViewId::new("root"))
            .iter()
            .map(RuntimeViewId::as_str)
            .collect::<Vec<_>>(),
        vec!["title"]
    );
    assert_eq!(
        bridged.metadata_for("root").unwrap().style_refs(),
        ["surface.card"]
    );
    assert_eq!(
        bridged.metadata_for("root").unwrap().asset_paths(),
        ["assets/logo.svg"]
    );
    assert_eq!(bridged.metadata_for("root").unwrap().refs(), ["root_ref"]);
    assert_eq!(bridged.operation_keys(), artifact.operation_keys());
}

#[test]
fn native_runtime_bridge_applies_compiled_style_refs_to_runtime_visuals() {
    let sheet = compile_style_source(
        r#"
.surface {
  background-color: token(color.surface);
}
.headline {
  font-size: 22px;
}
"#,
    )
    .expect("style source compiles");
    let tokens = TokenSet::production().with_color("color.surface", 8, 10, 14, 255);
    let mut runtime = NativeAuthoringRuntime::new("native-styled-runtime");
    runtime.mount(
        NativeAuthoringElement::new("root", ElementKind::View)
            .with_style(StyleRef::new("surface"))
            .with_child(NativeChild::keyed(
                "title",
                NativeAuthoringElement::new("title", ElementKind::Text)
                    .with_style(StyleRef::new("headline"))
                    .with_prop("text", PropValue::String("Styled Runtime".into()))
                    .with_prop("width", PropValue::Number(180.0))
                    .with_prop("height", PropValue::Number(36.0)),
            )),
    );
    let artifact = runtime.finish().expect("authoring finalizes");

    let bridged = NativeRuntimeBridge::new()
        .bridge_artifact_with_styles(&artifact, &sheet, &tokens)
        .expect("styled artifact bridges");
    let frame = RuntimeSceneBridge::new(Viewport::new(200.0, 100.0))
        .build(bridged.runtime_tree())
        .expect("runtime scene builds");

    assert!(frame.draw_commands().iter().any(|command| {
        matches!(
            command,
            RuntimeDrawCommand::Fill {
                id,
                color: Color { .. },
                ..
            } if id.as_str() == "root"
        )
    }));
    assert!(frame.draw_commands().iter().any(|command| {
        matches!(
            command,
            RuntimeDrawCommand::Fill {
                id,
                color,
                ..
            } if id.as_str() == "root" && *color == Color::rgba(8, 10, 14, 255)
        )
    }));
    assert!(frame.draw_commands().iter().any(|command| {
        matches!(
            command,
            RuntimeDrawCommand::Text {
                id,
                font_size,
                ..
            } if id.as_str() == "title" && *font_size == 22.0
        )
    }));
}

#[test]
fn native_runtime_bridge_rejects_invalid_styled_font_size() {
    let sheet = compile_style_source(".headline { font-size: 0px; }")
        .expect("style source compiles before runtime validation");
    let element = NativeAuthoringElement::new("title", ElementKind::Text)
        .with_style(StyleRef::new("headline"))
        .with_prop("text", PropValue::String("Invalid styled type".into()));

    let error = NativeRuntimeBridge::new()
        .bridge_element_with_styles(&element, &sheet, &TokenSet::production())
        .expect_err("zero styled font-size must fail before rendering");

    assert_eq!(error.rule(), "native-runtime.layout.invalid-number");
    assert!(error.message().contains("font-size"));
}

#[test]
fn native_runtime_bridge_rejects_invalid_layout_numbers_with_structured_error() {
    let element = NativeAuthoringElement::new("bad", ElementKind::View)
        .with_prop("width", PropValue::Number(f64::NAN));

    let error = NativeRuntimeBridge::new()
        .bridge_element(&element)
        .expect_err("non-finite width must fail");

    assert_eq!(error.rule(), "native-runtime.layout.invalid-number");
    assert!(error.message().contains("width"));
}

#[test]
fn native_runtime_bridge_rejects_zero_font_size_before_rendering() {
    let element = NativeAuthoringElement::new("bad-text", ElementKind::Text)
        .with_prop("text", PropValue::String("Bad".into()))
        .with_prop("font_size", PropValue::Number(0.0));

    let error = NativeRuntimeBridge::new()
        .bridge_element(&element)
        .expect_err("zero font size must fail before Skia rendering");

    assert_eq!(error.rule(), "native-runtime.layout.invalid-number");
    assert!(error.message().contains("font_size"));
}

#[test]
fn native_runtime_bridge_renders_authoring_artifact_to_visible_skia_pixels() {
    let mut runtime = NativeAuthoringRuntime::new("native-pixels");
    runtime.mount(
        NativeAuthoringElement::new("root", ElementKind::View)
            .with_prop("background", PropValue::String("#080a0e".into()))
            .with_child(NativeChild::keyed(
                "title",
                NativeAuthoringElement::new("title", ElementKind::Text)
                    .with_prop("text", PropValue::String("Native Pixels".into()))
                    .with_prop("font_size", PropValue::Number(18.0))
                    .with_prop("color", PropValue::String("#ffffff".into()))
                    .with_prop("width", PropValue::Number(160.0))
                    .with_prop("height", PropValue::Number(32.0)),
            ))
            .with_child(NativeChild::keyed(
                "bar",
                NativeAuthoringElement::new("bar", ElementKind::View)
                    .with_prop("background", PropValue::String("#f05828".into()))
                    .with_prop("width", PropValue::Number(96.0))
                    .with_prop("height", PropValue::Number(24.0)),
            )),
    );
    let artifact = runtime.finish().expect("authoring finalizes");
    let bridged = NativeRuntimeBridge::new()
        .bridge_artifact(&artifact)
        .expect("artifact bridges");
    let frame = RuntimeSceneBridge::new(Viewport::new(180.0, 96.0))
        .build(bridged.runtime_tree())
        .expect("runtime scene builds");

    let mut backend = SkiaRendererBackend::default();
    backend
        .create_surface("main", 180, 96)
        .expect("surface creates");
    backend.begin_frame("main").expect("frame begins");
    backend
        .clear(Color::rgba(0, 0, 0, 255))
        .expect("surface clears");
    render_runtime_frame_with_skia(&frame, &mut backend);
    backend.end_frame("main").expect("frame ends");

    let snapshot = backend.frame_snapshot("main").expect("snapshot exists");
    assert!(snapshot.pixels().iter().any(|pixel| *pixel == 0xf05828));
    assert!(
        count_changed_pixels(
            snapshot,
            0x080a0e,
            frame.geometry_for(&RuntimeViewId::new("title")).unwrap()
        ) > 0
    );
}

fn render_runtime_frame_with_skia(frame: &RuntimeSceneFrame, backend: &mut SkiaRendererBackend) {
    for command in frame.draw_commands() {
        match command {
            RuntimeDrawCommand::Fill {
                geometry, color, ..
            } => backend
                .fill(*geometry, *color)
                .expect("fill command renders"),
            RuntimeDrawCommand::Text {
                geometry,
                text,
                font_size,
                color,
                ..
            } => backend
                .draw_text_at(
                    text,
                    geometry.x,
                    geometry.y + geometry.height,
                    *font_size,
                    *color,
                )
                .expect("text command renders"),
        }
    }
}

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

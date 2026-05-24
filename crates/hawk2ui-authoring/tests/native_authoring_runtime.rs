use hawk2ui_authoring::{
    AssetRef, ElementKind, EventKind, EventPayloadField, NativeAuthoringElement,
    NativeAuthoringRuntime, NativeChild, NativeLifecycleEvent, NativeRef, PointerEventKind,
    PropValue, StyleRef,
};

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

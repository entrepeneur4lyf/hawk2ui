use hawk2ui_authoring::{ElementKind, EventPayloadField};
use hawk2ui_framework_solid::{SolidComponentSource, SolidIntegration};

#[test]
fn solid_renderer_maps_fine_grained_updates_lifecycle_keyed_children_events_refs_styles_assets_and_source_maps()
 {
    let source = SolidComponentSource::new(
        "examples/frameworks/solid-basic/src/App.tsx",
        r#"
export function App() {
  const [items] = createSignal([{ id: 'title' }, { id: 'cta' }]);
  return <hawk-view id="root" ref={root_ref} class="surface.card" data-asset="assets/logo.svg" onPointerDown={handlePress} onMount={onMount} onCleanup={onCleanup}>
    <For each={items()}>{(item) => <hawk-text id={item.id}>{item.id}</hawk-text>}</For>
  </hawk-view>;
}
"#,
    );

    let artifact = SolidIntegration::new()
        .render(source)
        .expect("valid Solid source should render");

    assert_eq!(artifact.framework(), "solid");
    assert_eq!(artifact.framework_version_requirement(), ">=1");
    assert_eq!(artifact.root().id().as_str(), "root");
    assert_eq!(artifact.root().kind(), ElementKind::View);
    assert_eq!(artifact.keyed_children(), ["title", "cta"]);
    assert_eq!(artifact.refs(), ["root_ref"]);
    assert_eq!(artifact.style_refs(), ["surface.card"]);
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
        ["signal:items", "for-each:keyed", "effect:root-props"]
    );
    assert_eq!(
        artifact.source_map().author_file(),
        "examples/frameworks/solid-basic/src/App.tsx"
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
        .expect_err("invalid Solid source should fail");
    let rules: Vec<_> = error
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.rule.as_str())
        .collect();

    assert_eq!(
        rules,
        [
            "solid.asset.path-invalid",
            "solid.renderer.unresolved-component"
        ]
    );
    assert_eq!(error.source_map().author_file(), "src/Broken.tsx");
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
    assert!(index_ts.contains("renderHawkSolid"));
    assert!(app.contains("<For each={items()}"));
    assert!(app.contains("assets/logo.svg"));
}

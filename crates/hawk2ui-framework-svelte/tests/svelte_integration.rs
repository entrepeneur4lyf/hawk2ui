use hawk2ui_authoring::{ElementKind, EventPayloadField};
use hawk2ui_framework_svelte::{SvelteComponentSource, SvelteIntegration};

#[test]
fn svelte_5_compile_maps_lifecycle_keyed_children_events_refs_styles_assets_and_source_maps() {
    let source = SvelteComponentSource::new(
        "examples/frameworks/svelte-basic/src/App.svelte",
        r#"
<script>
  import logo from '../assets/logo.svg';
  let items = [{ id: 'title' }, { id: 'cta' }];
</script>

<hawk-view id="root" use:ref="root_ref" class="surface.card" data-asset="assets/logo.svg" on:press={handlePress} on:mount={onMount} on:destroy={onDestroy}>
  {#each items as item (item.id)}
    <hawk-text id={item.id}>{item.id}</hawk-text>
  {/each}
</hawk-view>
"#,
    );

    let artifact = SvelteIntegration::new()
        .compile(source)
        .expect("valid Svelte source should compile");

    assert_eq!(artifact.framework(), "svelte");
    assert_eq!(artifact.framework_version_requirement(), ">=5");
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
        ["mounted:onMount", "unmounted:onDestroy"]
    );
    assert_eq!(
        artifact.source_map().author_file(),
        "examples/frameworks/svelte-basic/src/App.svelte"
    );
    assert_eq!(artifact.diagnostics(), []);
}

#[test]
fn svelte_5_compile_reports_author_source_diagnostics() {
    let source = SvelteComponentSource::new(
        "src/Broken.svelte",
        "<hawk-view data-asset=\"https://example.invalid/logo.svg\"><Broken /></hawk-view>",
    );

    let error = SvelteIntegration::new()
        .compile(source)
        .expect_err("invalid Svelte source should fail");
    let rules: Vec<_> = error
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.rule.as_str())
        .collect();

    assert_eq!(
        rules,
        [
            "svelte.asset.path-invalid",
            "svelte.compile.unresolved-component"
        ]
    );
    assert_eq!(error.source_map().author_file(), "src/Broken.svelte");
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

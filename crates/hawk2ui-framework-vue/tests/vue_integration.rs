use hawk2ui_authoring::{ElementKind, EventPayloadField};
use hawk2ui_framework_vue::{VueIntegration, VueSingleFileComponent};

#[test]
fn vue_35_renderer_maps_lifecycle_keyed_children_events_refs_styles_assets_and_source_maps() {
    let source = VueSingleFileComponent::new(
        "examples/frameworks/vue-basic/src/App.vue",
        r#"
<script setup>
const items = [{ id: 'title' }, { id: 'cta' }];
</script>

<template>
  <hawk-view id="root" ref="root_ref" class="surface.card" data-asset="assets/logo.svg" @pointerdown="handlePress" @mounted="onMounted" @unmounted="onUnmounted">
    <hawk-text v-for="item in items" :id="item.id" :key="item.id">{{ item.id }}</hawk-text>
  </hawk-view>
</template>
"#,
    );

    let artifact = VueIntegration::new()
        .render(source)
        .expect("valid Vue source should render");

    assert_eq!(artifact.framework(), "vue");
    assert_eq!(artifact.framework_version_requirement(), ">=3.5");
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
        ["mounted:onMounted", "unmounted:onUnmounted"]
    );
    assert_eq!(
        artifact.source_map().author_file(),
        "examples/frameworks/vue-basic/src/App.vue"
    );
    assert_eq!(
        artifact.renderer_operations(),
        [
            "create:root",
            "insert:title",
            "insert:cta",
            "patch-props:root"
        ]
    );
}

#[test]
fn vue_35_renderer_reports_author_source_diagnostics() {
    let source = VueSingleFileComponent::new(
        "src/Broken.vue",
        "<template><hawk-view data-asset=\"https://example.invalid/logo.svg\"><Missing /></hawk-view></template>",
    );

    let error = VueIntegration::new()
        .render(source)
        .expect_err("invalid Vue source should fail");
    let rules: Vec<_> = error
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.rule.as_str())
        .collect();

    assert_eq!(
        rules,
        [
            "vue.asset.path-invalid",
            "vue.renderer.unresolved-component"
        ]
    );
    assert_eq!(error.source_map().author_file(), "src/Broken.vue");
}

#[test]
fn vue_smoke_app_declares_public_package_entrypoint() {
    let package_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("packages/hawk2ui-vue");
    let example_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("examples/frameworks/vue-basic");

    let package_json = std::fs::read_to_string(package_root.join("package.json"))
        .expect("Vue package manifest should exist");
    let index_ts = std::fs::read_to_string(package_root.join("src/index.ts"))
        .expect("Vue package entrypoint should exist");
    let app = std::fs::read_to_string(example_root.join("src/App.vue"))
        .expect("Vue smoke app should exist");

    assert!(package_json.contains("@hawk2ui/vue"));
    assert!(index_ts.contains("createHawkVueRenderer"));
    assert!(app.contains("v-for=\"item in items\""));
    assert!(app.contains("assets/logo.svg"));
}

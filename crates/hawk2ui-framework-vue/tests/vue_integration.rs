use hawk2ui_authoring::{
    AssetRef, ElementKind, EventKind, EventPayloadField, FrameworkNativeNode,
    FrameworkNativeProgram, HandlerRef, NativeLifecycleEvent, NativeRef, PointerEventKind,
    PropValue, StyleRef,
};
use hawk2ui_framework_vue::{VueIntegration, VueSingleFileComponent};
use hawk2ui_layout::Viewport;
use hawk2ui_render::Color;
use hawk2ui_runtime::{RuntimeDrawCommand, RuntimeSceneBridge, RuntimeViewId};
use hawk2ui_style::{TokenSet, compile_style_source};

#[test]
fn vue_35_renderer_maps_lifecycle_keyed_children_events_refs_styles_assets_and_source_maps() {
    let source = VueSingleFileComponent::new(
        "examples/frameworks/vue-basic/src/App.vue",
        r#"
<script setup>
const items = [{ id: 'title' }, { id: 'cta' }, { id: 'meter' }];
</script>

<template>
  <hawk-view id="root" ref="root_ref" class="surface.card intent.primary" data-asset="assets/logo.svg" @pointerdown="handlePress" @mounted="onMounted" @unmounted="onUnmounted">
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
        ["mounted:onMounted", "unmounted:onUnmounted"]
    );
    assert_eq!(
        artifact.source_map().author_file(),
        "examples/frameworks/vue-basic/src/App.vue"
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
            "bind-lifecycle:root:mounted:onMounted",
            "bind-lifecycle:root:unmounted:onUnmounted",
            "create-node:title:text",
            "append-child:root:title:key:title",
            "create-node:cta:text",
            "append-child:root:cta:key:cta",
            "create-node:meter:text",
            "append-child:root:meter:key:meter",
            "commit:root",
        ]
    );
}

#[test]
fn vue_35_renderer_accepts_explicit_native_compiler_boundary_without_source_scanning() {
    let source = VueSingleFileComponent::from_native_program(
        "examples/frameworks/vue-basic/src/App.vue",
        framework_native_program("vue.asset", "onUnmounted"),
    );

    let artifact = VueIntegration::new()
        .render(source.clone())
        .expect("explicit Vue native compiler output should render");
    let runtime = VueIntegration::new()
        .render_to_runtime(source)
        .expect("explicit Vue native compiler output should bridge");

    assert_eq!(artifact.root().id().as_str(), "root");
    assert_eq!(artifact.keyed_children(), ["title"]);
    assert_eq!(artifact.refs(), ["root_ref"]);
    assert_eq!(artifact.style_refs(), ["surface.card"]);
    assert_eq!(artifact.asset_refs()[0].path(), "assets/logo.svg");
    assert_eq!(artifact.events()[0].event().stable_key(), "pointer.press");
    assert_eq!(
        artifact.lifecycle_handlers(),
        ["mounted:onMount", "unmounted:onUnmounted"]
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
            "bind-lifecycle:root:unmounted:onUnmounted",
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
fn vue_35_renderer_gates_lifecycle_handlers_and_collects_all_refs() {
    let source = VueSingleFileComponent::new(
        "src/Static.vue",
        r#"<template><hawk-view id="root" ref="root_ref" ref="panel_ref"><hawk-text id="title">Title</hawk-text></hawk-view></template>"#,
    );

    let artifact = VueIntegration::new()
        .render(source)
        .expect("recognized Hawk source should render");

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
fn vue_35_renderer_rejects_non_hawk_source_and_all_invalid_asset_paths() {
    let error = VueIntegration::new()
        .render(VueSingleFileComponent::new(
            "src/Invalid.vue",
            r#"<template><hawk-view data-asset="assets/logo.svg" data-asset="%2e%2e/secret.svg" data-asset="icons\logo.svg"></hawk-view></template>"#,
        ))
        .expect_err("invalid raw-source bridge input should fail");
    let rules: Vec<_> = error
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.rule.as_str())
        .collect();

    assert_eq!(rules, ["vue.asset.path-invalid", "vue.asset.path-invalid"]);

    let no_root = VueIntegration::new()
        .render(VueSingleFileComponent::new(
            "src/NoRoot.vue",
            "<script setup></script>",
        ))
        .expect_err("raw-source bridge should reject non-Hawk source");

    assert_eq!(
        no_root.diagnostics()[0].rule.as_str(),
        "vue.renderer.no-root"
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
fn vue_35_renderer_bridges_to_runtime_tree() {
    let source = VueSingleFileComponent::new(
        "examples/frameworks/vue-basic/src/App.vue",
        r#"<script setup>const items = [{ id: 'title' }, { id: 'cta' }, { id: 'meter' }];</script><template><hawk-view id="root" ref="root_ref" class="surface.card intent.primary" data-asset="assets/logo.svg" @pointerdown="handlePress" @mounted="onMounted" @unmounted="onUnmounted"><hawk-text v-for="item in items" :id="item.id" :key="item.id">{{ item.id }}</hawk-text></hawk-view></template>"#,
    );

    let artifact = VueIntegration::new()
        .render_to_runtime(source)
        .expect("valid Vue source should bridge to runtime");

    assert_eq!(artifact.rendered().framework(), "vue");
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
fn vue_35_renderer_preserves_static_text_children() {
    let source = VueSingleFileComponent::new(
        "examples/frameworks/vue-basic/src/App.vue",
        r#"<template><hawk-view id="root"><hawk-text id="title">Static Title</hawk-text></hawk-view></template>"#,
    );

    let artifact = VueIntegration::new()
        .render_to_runtime(source)
        .expect("valid Vue source should bridge to runtime");

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
fn vue_35_render_to_runtime_with_styles_applies_compiled_root_background() {
    let source = VueSingleFileComponent::new(
        "examples/frameworks/vue-basic/src/App.vue",
        r#"<template><hawk-view id="root" class="surface"></hawk-view></template>"#,
    );
    let sheet = compile_style_source(".surface { background-color: token(color.surface); }")
        .expect("style source compiles");
    let tokens = TokenSet::production().with_color("color.surface", 240, 88, 40, 255);

    let artifact = VueIntegration::new()
        .render_to_runtime_with_styles(source, &sheet, &tokens)
        .expect("valid Vue source should bridge with styles");
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
fn vue_35_render_to_runtime_with_theme_applies_theme_background() {
    let source = VueSingleFileComponent::new(
        "examples/frameworks/vue-basic/src/App.vue",
        r#"<template><hawk-view id="root" class="surface"></hawk-view></template>"#,
    );
    let sheet = compile_style_source(".surface { background-color: token(color.surface); }")
        .expect("style source compiles");
    let tokens = TokenSet::production()
        .with_color("color.surface", 8, 10, 14, 255)
        .with_theme(hawk2ui_style::ThemeVariant::new("light").with_token(
            "color.surface",
            hawk2ui_style::TokenValue::ColorRgba(245, 243, 238, 255),
        ));

    let artifact = VueIntegration::new()
        .render_to_runtime_with_theme(source, &sheet, &tokens, "light")
        .expect("valid Vue source should bridge with themed styles");
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

fn framework_native_program(asset_name: &str, unmounted: &str) -> FrameworkNativeProgram {
    FrameworkNativeProgram::new(
        FrameworkNativeNode::new("root", ElementKind::View)
            .with_ref(NativeRef::new("root_ref"))
            .with_style(StyleRef::new("surface.card"))
            .with_asset(AssetRef::new(asset_name, "assets/logo.svg"))
            .with_event(
                EventKind::Pointer(PointerEventKind::Press),
                HandlerRef::new("handlePress"),
                [EventPayloadField::Position],
            )
            .with_lifecycle(NativeLifecycleEvent::Mounted, HandlerRef::new("onMount"))
            .with_lifecycle(NativeLifecycleEvent::Unmounted, HandlerRef::new(unmounted))
            .with_child(
                "title",
                FrameworkNativeNode::new("title", ElementKind::Text)
                    .with_key("title")
                    .with_prop("text", PropValue::String("Boundary Title".to_string()))
                    .with_prop("font_size", PropValue::Number(18.0)),
            ),
    )
}

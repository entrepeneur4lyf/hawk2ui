use hawk2ui_authoring::{ElementKind, EventPayloadField};
use hawk2ui_framework_svelte::{SvelteComponentSource, SvelteIntegration};
use hawk2ui_layout::Viewport;
use hawk2ui_render::{Color, Geometry, RendererBackend};
use hawk2ui_render_skia::{SkiaFrameSnapshot, SkiaRendererBackend};
use hawk2ui_runtime::{RuntimeDrawCommand, RuntimeSceneBridge, RuntimeSceneFrame, RuntimeViewId};

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

#[test]
fn svelte_5_compile_to_runtime_uses_native_bridge_contract() {
    let source = SvelteComponentSource::new(
        "examples/frameworks/svelte-basic/src/App.svelte",
        r#"
<script>
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
        vec!["title", "cta"]
    );
    assert_eq!(artifact.metadata_for("root").unwrap().refs(), ["root_ref"]);
    assert_eq!(
        artifact.metadata_for("root").unwrap().style_refs(),
        ["surface.card"]
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
fn svelte_5_runtime_bridge_renders_visible_skia_pixels() {
    let source = SvelteComponentSource::new(
        "examples/frameworks/svelte-basic/src/App.svelte",
        r#"<hawk-view id="root" use:ref="root_ref" class="surface.card" data-asset="assets/logo.svg" on:press={handlePress} on:mount={onMount} on:destroy={onDestroy}>{#each items as item (item.id)}<hawk-text id={item.id}>{item.id}</hawk-text>{/each}</hawk-view>"#,
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
    assert!(snapshot.pixels().iter().any(|pixel| *pixel == 0x080a0e));
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

use hawk2ui_authoring::{ElementKind, EventPayloadField};
use hawk2ui_framework_react::{ReactElementTree, ReactIntegration};
use hawk2ui_layout::Viewport;
use hawk2ui_render::Color;
use hawk2ui_runtime::{RuntimeDrawCommand, RuntimeSceneBridge, RuntimeViewId};
use hawk2ui_style::{TokenSet, compile_style_source};

#[test]
fn react_19_renderer_maps_reconciler_lifecycle_keyed_children_events_refs_styles_assets_and_source_maps()
 {
    let source = ReactElementTree::new(
        "examples/frameworks/react-basic/src/App.tsx",
        r#"
export function App() {
  const items = [{ id: 'title' }, { id: 'cta' }, { id: 'meter' }];
  return <hawk-view id="root" ref="root_ref" className="surface.card intent.primary" data-asset="assets/logo.svg" onPointerDown={handlePress} onMount={onMount} onUnmount={onUnmount}>
    {items.map((item) => <hawk-text id={item.id} key={item.id}>{item.id}</hawk-text>)}
  </hawk-view>;
}
"#,
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
            "create:root",
            "append:title",
            "append:cta",
            "append:meter",
            "commit:root"
        ]
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
        .expect_err("invalid React source should fail");
    let rules: Vec<_> = error
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.rule.as_str())
        .collect();

    assert_eq!(
        rules,
        [
            "react.asset.path-invalid",
            "react.reconciler.unresolved-component"
        ]
    );
    assert_eq!(error.source_map().author_file(), "src/Broken.tsx");
}

#[test]
fn react_19_renderer_bridges_to_runtime_tree() {
    let source = ReactElementTree::new(
        "examples/frameworks/react-basic/src/App.tsx",
        r#"const items = [{ id: 'title' }, { id: 'cta' }, { id: 'meter' }];<hawk-view id="root" ref="root_ref" className="surface.card intent.primary" data-asset="assets/logo.svg" onPointerDown={handlePress} onMount={onMount} onUnmount={onUnmount}>{items.map((item) => <hawk-text id={item.id} key={item.id}>{item.id}</hawk-text>)}</hawk-view>"#,
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
fn react_19_render_to_runtime_with_styles_applies_compiled_root_background() {
    let source = ReactElementTree::new(
        "examples/frameworks/react-basic/src/App.tsx",
        r#"<hawk-view id="root" className="surface"></hawk-view>"#,
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
    assert!(index_ts.contains("createHawkReactRoot"));
    assert!(app.contains("items.map"));
    assert!(app.contains("assets/logo.svg"));
}

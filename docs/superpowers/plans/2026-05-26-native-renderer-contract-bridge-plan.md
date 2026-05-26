# Native Renderer Contract Bridge Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert finalized native authoring artifacts into runtime view trees that flow through layout, scene, paint command export, and Skia pixel rendering.

**Architecture:** Add a focused `hawk2ui_authoring::runtime_bridge` module that depends on `hawk2ui-runtime`, `hawk2ui-layout`, and `hawk2ui-render`. The bridge preserves authoring metadata beside the renderable runtime tree while keeping framework-specific compiler code out of this slice.

**Tech Stack:** Rust, `hawk2ui-authoring`, `hawk2ui-runtime`, `hawk2ui-layout`, `hawk2ui-render`, `hawk2ui-render-skia` for integration tests.

---

## Files

- Modify: `crates/hawk2ui-authoring/Cargo.toml` to add runtime/layout/render dependencies and Skia as a dev-dependency.
- Modify: `crates/hawk2ui-authoring/src/lib.rs` to export the bridge API.
- Modify: `crates/hawk2ui-authoring/src/native.rs` to expose read-only node and child traversal accessors.
- Create: `crates/hawk2ui-authoring/src/runtime_bridge.rs` for the bridge, metadata records, and structured errors.
- Modify: `crates/hawk2ui-authoring/tests/native_authoring_runtime.rs` for red-green coverage.

## Task 1: Read-Only Native Tree Traversal

**Files:**
- Modify: `crates/hawk2ui-authoring/tests/native_authoring_runtime.rs`
- Modify: `crates/hawk2ui-authoring/src/native.rs`

- [ ] **Step 1: Write the failing test**

Add a test proving native elements expose immutable traversal:

```rust
#[test]
fn native_authoring_elements_expose_read_only_tree_for_runtime_bridge() {
    let root = NativeAuthoringElement::new("root", ElementKind::View)
        .with_child(NativeChild::keyed(
            "title",
            NativeAuthoringElement::new("title", ElementKind::Text)
                .with_prop("text", PropValue::String("Hello".into())),
        ));

    assert_eq!(root.node().id().as_str(), "root");
    assert_eq!(root.children().len(), 1);
    assert_eq!(root.children()[0].key(), Some("title"));
    assert_eq!(root.children()[0].element().node().id().as_str(), "title");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p hawk2ui-authoring native_authoring_elements_expose_read_only_tree_for_runtime_bridge
```

Expected: compile failure for missing `node`, `children`, `key`, and `element` accessors.

- [ ] **Step 3: Implement accessors**

Add `NativeChild::key`, `NativeChild::element`, `NativeAuthoringElement::node`, and `NativeAuthoringElement::children` as read-only methods.

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p hawk2ui-authoring native_authoring_elements_expose_read_only_tree_for_runtime_bridge
```

Expected: one passing test.

- [ ] **Step 5: Review Check**

Ask and answer: “As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability?” If revision is needed, take corrective action before continuing.

## Task 2: Native Artifact to Runtime View Tree

**Files:**
- Modify: `crates/hawk2ui-authoring/Cargo.toml`
- Modify: `crates/hawk2ui-authoring/src/lib.rs`
- Create: `crates/hawk2ui-authoring/src/runtime_bridge.rs`
- Modify: `crates/hawk2ui-authoring/tests/native_authoring_runtime.rs`

- [ ] **Step 1: Write the failing test**

Add a test proving bridge conversion preserves tree structure, visual mapping, and metadata:

```rust
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
    assert_eq!(bridged.metadata_for("root").unwrap().style_refs(), ["surface.card"]);
    assert_eq!(bridged.metadata_for("root").unwrap().asset_paths(), ["assets/logo.svg"]);
    assert_eq!(bridged.metadata_for("root").unwrap().refs(), ["root_ref"]);
    assert_eq!(bridged.operation_keys(), artifact.operation_keys());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p hawk2ui-authoring native_runtime_bridge_converts_authoring_artifact_to_runtime_view_tree
```

Expected: compile failure for missing bridge API and dependencies.

- [ ] **Step 3: Implement bridge conversion**

Implement `NativeRuntimeBridge`, `NativeRuntimeBridgeArtifact`, `NativeRuntimeNodeMetadata`, and `NativeRuntimeBridgeError`. Add dependencies on `hawk2ui-layout`, `hawk2ui-render`, and `hawk2ui-runtime`. Map background/color hex strings, text props, font size, width, and height.

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p hawk2ui-authoring native_runtime_bridge_converts_authoring_artifact_to_runtime_view_tree
```

Expected: one passing test.

- [ ] **Step 5: Review Check**

Ask and answer: “As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability?” If revision is needed, take corrective action before continuing.

## Task 3: Bridge Error Contract

**Files:**
- Modify: `crates/hawk2ui-authoring/tests/native_authoring_runtime.rs`
- Modify: `crates/hawk2ui-authoring/src/runtime_bridge.rs`

- [ ] **Step 1: Write the failing test**

Add a test proving invalid numeric layout props fail with a structured rule:

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p hawk2ui-authoring native_runtime_bridge_rejects_invalid_layout_numbers_with_structured_error
```

Expected: failure until `bridge_element` and invalid number handling exist.

- [ ] **Step 3: Implement structured errors**

Ensure bridge errors expose `rule()` and `message()`. Reject non-finite or negative `width`, `height`, and `font_size` props.

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p hawk2ui-authoring native_runtime_bridge_rejects_invalid_layout_numbers_with_structured_error
```

Expected: one passing test.

- [ ] **Step 5: Review Check**

Ask and answer: “As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability?” If revision is needed, take corrective action before continuing.

## Task 4: End-to-End Skia Pixel Proof

**Files:**
- Modify: `crates/hawk2ui-authoring/Cargo.toml`
- Modify: `crates/hawk2ui-authoring/tests/native_authoring_runtime.rs`

- [ ] **Step 1: Write the failing test**

Add a test proving native authoring output renders visible pixels:

```rust
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
    backend.create_surface("main", 180, 96).expect("surface creates");
    backend.begin_frame("main").expect("frame begins");
    backend.clear(Color::rgba(0, 0, 0, 255)).expect("surface clears");
    render_runtime_frame_with_skia(&frame, &mut backend);
    backend.end_frame("main").expect("frame ends");

    let snapshot = backend.frame_snapshot("main").expect("snapshot exists");
    assert!(snapshot.pixels().iter().any(|pixel| *pixel == 0xf05828));
    assert!(count_changed_pixels(snapshot, 0x080a0e, frame.geometry_for(&RuntimeViewId::new("title")).unwrap()) > 0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p hawk2ui-authoring native_runtime_bridge_renders_authoring_artifact_to_visible_skia_pixels
```

Expected: compile failure until Skia dev-dependency and test helpers are present.

- [ ] **Step 3: Implement test helper and dev dependency**

Add `hawk2ui-render-skia` as a dev-dependency and test helper functions that execute `RuntimeDrawCommand` through `SkiaRendererBackend`.

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p hawk2ui-authoring native_runtime_bridge_renders_authoring_artifact_to_visible_skia_pixels
```

Expected: one passing test with visible fill and text pixels.

- [ ] **Step 5: Review Check**

Ask and answer: “As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability?” If revision is needed, take corrective action before continuing.

## Task 5: Verification and Commit

**Files:**
- All changed files from prior tasks.

- [ ] **Step 1: Format check**

Run:

```bash
cargo fmt --all --check
```

Expected: exit code 0.

- [ ] **Step 2: Package tests**

Run:

```bash
cargo test -p hawk2ui-authoring
cargo test -p hawk2ui-runtime
cargo test -p hawk2ui-render-skia
```

Expected: exit code 0 for each command.

- [ ] **Step 3: Workspace gates**

Run:

```bash
cargo check --workspace
cargo clippy --workspace -- -D warnings
```

Expected: exit code 0 for both commands.

- [ ] **Step 4: GitNexus scope check**

Run GitNexus detect changes for all uncommitted changes.

Expected: affected scope matches the native renderer bridge vertical.

- [ ] **Step 5: Review Check**

Ask and answer: “As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability?” If revision is needed, take corrective action before committing.

- [ ] **Step 6: Commit**

Run:

```bash
git add Cargo.lock crates/hawk2ui-authoring/Cargo.toml crates/hawk2ui-authoring/src/lib.rs crates/hawk2ui-authoring/src/native.rs crates/hawk2ui-authoring/src/runtime_bridge.rs crates/hawk2ui-authoring/tests/native_authoring_runtime.rs
git commit -m "Add native renderer contract bridge"
```

Expected: commit succeeds.

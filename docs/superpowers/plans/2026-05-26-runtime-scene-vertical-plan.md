# Runtime Scene Vertical Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the runtime bridge that turns a retained runtime view tree into layout geometry, scene graph nodes, paint commands, and Skia-renderable draw commands.

**Architecture:** Add a focused `hawk2ui_runtime::view` module that owns runtime view records and the bridge from runtime data to layout/render crates. The bridge produces deterministic `LayoutOutput`, `SceneGraph`, `LayerStack`, stable `PaintCommandList`, and geometry-rich `RuntimeDrawCommand` records that host adapters or renderers can execute.

**Tech Stack:** Rust, `hawk2ui-runtime`, `hawk2ui-layout`, `hawk2ui-render`, `hawk2ui-render-skia`, Skia CPU raster test snapshots.

---

## Files

- Modify: `crates/hawk2ui-runtime/Cargo.toml` to depend on `hawk2ui-layout`, `hawk2ui-render`, and dev-depend on `hawk2ui-render-skia`.
- Modify: `crates/hawk2ui-runtime/src/lib.rs` to export the new runtime view API.
- Create: `crates/hawk2ui-runtime/src/view.rs` for retained runtime view records, bridge output, errors, and conversion logic.
- Modify: `crates/hawk2ui-runtime/tests/runtime_behavior.rs` for red-green behavior coverage.

## Task 1: Runtime View Tree Contract

**Files:**
- Modify: `crates/hawk2ui-runtime/tests/runtime_behavior.rs`
- Create: `crates/hawk2ui-runtime/src/view.rs`
- Modify: `crates/hawk2ui-runtime/src/lib.rs`
- Modify: `crates/hawk2ui-runtime/Cargo.toml`

- [ ] **Step 1: Write the failing test**

Append this test to `crates/hawk2ui-runtime/tests/runtime_behavior.rs`:

```rust
#[test]
fn runtime_view_tree_preserves_parent_child_order_and_rejects_duplicates() {
    let root = RuntimeViewNode::new(
        RuntimeViewId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Column),
        RuntimeVisual::Fill(Color::rgba(8, 10, 14, 255)),
    );
    let header = RuntimeViewNode::new(
        RuntimeViewId::new("header"),
        LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(300.0, 32.0)),
        RuntimeVisual::Text(RuntimeTextVisual::new("Hello Hawk2UI", 18.0, Color::rgba(240, 244, 255, 255))),
    );
    let meter = RuntimeViewNode::new(
        RuntimeViewId::new("meter"),
        LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(300.0, 48.0)),
        RuntimeVisual::Fill(Color::rgba(30, 144, 255, 255)),
    );

    let tree = RuntimeViewTree::new(root)
        .with_child(&RuntimeViewId::new("root"), header)
        .expect("header attaches to root")
        .with_child(&RuntimeViewId::new("root"), meter)
        .expect("meter attaches to root");

    assert_eq!(tree.root_id().as_str(), "root");
    assert_eq!(tree.children_of(&RuntimeViewId::new("root")).iter().map(RuntimeViewId::as_str).collect::<Vec<_>>(), vec!["header", "meter"]);
    assert!(tree.node(&RuntimeViewId::new("header")).is_some());

    let duplicate = RuntimeViewNode::new(
        RuntimeViewId::new("meter"),
        LayoutStyle::custom_measured(),
        RuntimeVisual::None,
    );
    let error = tree
        .with_child(&RuntimeViewId::new("root"), duplicate)
        .expect_err("duplicate view IDs must be rejected");

    assert_eq!(error, RuntimeSceneError::DuplicateNode("meter".into()));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p hawk2ui-runtime runtime_view_tree_preserves_parent_child_order_and_rejects_duplicates
```

Expected: compile failure because `RuntimeViewNode`, `RuntimeViewId`, `RuntimeViewTree`, `RuntimeVisual`, `RuntimeTextVisual`, and render/layout imports are not available.

- [ ] **Step 3: Implement the minimal tree API**

Add `hawk2ui-layout` and `hawk2ui-render` dependencies, export `view`, and define the runtime view tree types with duplicate and missing-parent validation.

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p hawk2ui-runtime runtime_view_tree_preserves_parent_child_order_and_rejects_duplicates
```

Expected: one passing test.

- [ ] **Step 5: Review Check**

Ask and answer: “As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability?” If revision is needed, take corrective action before continuing.

## Task 2: Layout, Scene, and Paint Bridge

**Files:**
- Modify: `crates/hawk2ui-runtime/tests/runtime_behavior.rs`
- Modify: `crates/hawk2ui-runtime/src/view.rs`

- [ ] **Step 1: Write the failing test**

Append this test:

```rust
#[test]
fn runtime_scene_bridge_computes_layout_scene_and_paint_commands() {
    let tree = RuntimeViewTree::new(RuntimeViewNode::new(
        RuntimeViewId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(320.0, 200.0))
            .with_padding(BoxEdges::all(LayoutValue::px(8.0)))
            .with_gap(LayoutValue::px(4.0)),
        RuntimeVisual::Fill(Color::rgba(12, 14, 18, 255)),
    ))
    .with_child(
        &RuntimeViewId::new("root"),
        RuntimeViewNode::new(
            RuntimeViewId::new("title"),
            LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(200.0, 32.0)),
            RuntimeVisual::Text(RuntimeTextVisual::new("Runtime Scene", 16.0, Color::rgba(255, 255, 255, 255))),
        ),
    )
    .expect("title attaches")
    .with_child(
        &RuntimeViewId::new("root"),
        RuntimeViewNode::new(
            RuntimeViewId::new("accent"),
            LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(120.0, 24.0)),
            RuntimeVisual::Fill(Color::rgba(255, 80, 48, 255)),
        ),
    )
    .expect("accent attaches");

    let frame = RuntimeSceneBridge::new(Viewport::new(320.0, 200.0))
        .build(&tree)
        .expect("runtime view tree bridges into render data");

    assert_eq!(frame.geometry_for(&RuntimeViewId::new("root")).unwrap(), Geometry::new(0.0, 0.0, 320.0, 200.0));
    assert_eq!(frame.geometry_for(&RuntimeViewId::new("title")).unwrap(), Geometry::new(8.0, 8.0, 200.0, 32.0));
    assert_eq!(frame.geometry_for(&RuntimeViewId::new("accent")).unwrap(), Geometry::new(8.0, 44.0, 120.0, 24.0));
    assert!(frame.scene().node(&SceneNodeId::new("title")).unwrap().hit_test().is_some());
    assert_eq!(frame.draw_commands().len(), 3);
    assert_eq!(frame.paint_commands().commands().len(), 3);
    assert!(frame.paint_commands().serialize_stable().contains("draw-text:title:Runtime Scene"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p hawk2ui-runtime runtime_scene_bridge_computes_layout_scene_and_paint_commands
```

Expected: compile failure because `RuntimeSceneBridge` and `RuntimeSceneFrame` are not implemented.

- [ ] **Step 3: Implement bridge output**

Implement `RuntimeSceneBridge`, `RuntimeSceneFrame`, `RuntimeDrawCommand`, conversion to `LayoutTree`, conversion to `SceneGraph`, deterministic `LayerStack`, and stable paint command export.

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p hawk2ui-runtime runtime_scene_bridge_computes_layout_scene_and_paint_commands
```

Expected: one passing test.

- [ ] **Step 5: Review Check**

Ask and answer: “As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability?” If revision is needed, take corrective action before continuing.

## Task 3: Invalidation Propagation

**Files:**
- Modify: `crates/hawk2ui-runtime/tests/runtime_behavior.rs`
- Modify: `crates/hawk2ui-runtime/src/view.rs`

- [ ] **Step 1: Write the failing test**

Append this test:

```rust
#[test]
fn runtime_scene_bridge_marks_invalidated_nodes_and_ancestors() {
    let tree = RuntimeViewTree::new(RuntimeViewNode::new(
        RuntimeViewId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Column),
        RuntimeVisual::Fill(Color::rgba(0, 0, 0, 255)),
    ))
    .with_child(
        &RuntimeViewId::new("root"),
        RuntimeViewNode::new(
            RuntimeViewId::new("meter"),
            LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(80.0, 20.0)),
            RuntimeVisual::Fill(Color::rgba(0, 200, 120, 255)),
        ),
    )
    .expect("meter attaches")
    .invalidate(&RuntimeViewId::new("meter"))
    .expect("meter invalidates");

    let frame = RuntimeSceneBridge::new(Viewport::new(100.0, 100.0))
        .build(&tree)
        .expect("invalidated tree bridges");

    assert_eq!(frame.invalidated_view_ids().iter().map(RuntimeViewId::as_str).collect::<Vec<_>>(), vec!["meter"]);
    assert!(frame.scene().node(&SceneNodeId::new("meter")).unwrap().invalidated());
    assert!(frame.scene().node(&SceneNodeId::new("root")).unwrap().invalidated());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p hawk2ui-runtime runtime_scene_bridge_marks_invalidated_nodes_and_ancestors
```

Expected: failure because invalidation is not bridged yet.

- [ ] **Step 3: Implement invalidation**

Add immutable `RuntimeViewTree::invalidate`, expose sorted invalidated view IDs on `RuntimeSceneFrame`, and call `SceneGraph::invalidate` for invalidated runtime nodes.

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p hawk2ui-runtime runtime_scene_bridge_marks_invalidated_nodes_and_ancestors
```

Expected: one passing test.

- [ ] **Step 5: Review Check**

Ask and answer: “As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability?” If revision is needed, take corrective action before continuing.

## Task 4: Skia Pixel Integration

**Files:**
- Modify: `crates/hawk2ui-runtime/Cargo.toml`
- Modify: `crates/hawk2ui-runtime/tests/runtime_behavior.rs`
- Modify: `crates/hawk2ui-runtime/src/view.rs`

- [ ] **Step 1: Write the failing test**

Append this test:

```rust
#[test]
fn runtime_scene_bridge_output_renders_visible_pixels_with_skia() {
    let tree = RuntimeViewTree::new(RuntimeViewNode::new(
        RuntimeViewId::new("root"),
        LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(180.0, 96.0))
            .with_padding(BoxEdges::all(LayoutValue::px(8.0)))
            .with_gap(LayoutValue::px(8.0)),
        RuntimeVisual::Fill(Color::rgba(8, 8, 12, 255)),
    ))
    .with_child(
        &RuntimeViewId::new("root"),
        RuntimeViewNode::new(
            RuntimeViewId::new("label"),
            LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(140.0, 28.0)),
            RuntimeVisual::Text(RuntimeTextVisual::new("Pixels", 18.0, Color::rgba(255, 255, 255, 255))),
        ),
    )
    .expect("label attaches")
    .with_child(
        &RuntimeViewId::new("root"),
        RuntimeViewNode::new(
            RuntimeViewId::new("bar"),
            LayoutStyle::custom_measured().with_size(LayoutSizing::fixed(96.0, 24.0)),
            RuntimeVisual::Fill(Color::rgba(240, 88, 40, 255)),
        ),
    )
    .expect("bar attaches");

    let frame = RuntimeSceneBridge::new(Viewport::new(180.0, 96.0))
        .build(&tree)
        .expect("runtime scene frame builds");
    let mut backend = SkiaRendererBackend::default();
    backend.create_surface("main", 180, 96).expect("surface creates");
    backend.begin_frame("main").expect("frame begins");
    backend.clear(Color::rgba(0, 0, 0, 255)).expect("surface clears");
    frame.render_with_skia(&mut backend).expect("draw commands render");
    backend.end_frame("main").expect("frame ends");

    let snapshot = backend.frame_snapshot("main").expect("snapshot exists");
    assert!(snapshot.pixels().chunks_exact(4).any(|rgba| rgba == [240, 88, 40, 255]));
    assert!(snapshot.pixels().chunks_exact(4).any(|rgba| rgba == [255, 255, 255, 255]));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p hawk2ui-runtime runtime_scene_bridge_output_renders_visible_pixels_with_skia
```

Expected: compile failure because the runtime crate lacks the dev dependency and `render_with_skia` helper.

- [ ] **Step 3: Implement Skia execution helper**

Add the `hawk2ui-render-skia` dev-dependency and an integration-test helper path that executes `RuntimeDrawCommand` records through the Skia backend. Keep production runtime independent of Skia by placing direct Skia calls in the test helper, unless a generic renderer execution helper is added in runtime against the `RendererBackend` trait.

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p hawk2ui-runtime runtime_scene_bridge_output_renders_visible_pixels_with_skia
```

Expected: one passing test and a frame snapshot containing visible fill and text pixels.

- [ ] **Step 5: Review Check**

Ask and answer: “As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability?” If revision is needed, take corrective action before continuing.

## Task 5: Full Verification and Commit

**Files:**
- All modified files from prior tasks.

- [ ] **Step 1: Format check**

Run:

```bash
cargo fmt --all --check
```

Expected: exit code 0.

- [ ] **Step 2: Runtime tests**

Run:

```bash
cargo test -p hawk2ui-runtime
```

Expected: exit code 0.

- [ ] **Step 3: Adjacent crate tests**

Run:

```bash
cargo test -p hawk2ui-layout
cargo test -p hawk2ui-render
cargo test -p hawk2ui-render-skia
```

Expected: exit code 0 for each command.

- [ ] **Step 4: Workspace compile and lint gates**

Run:

```bash
cargo check --workspace
cargo clippy --workspace -- -D warnings
```

Expected: exit code 0 for both commands.

- [ ] **Step 5: GitNexus scope check**

Run GitNexus detect changes for all uncommitted changes.

Expected: affected scope matches the runtime scene vertical.

- [ ] **Step 6: Review Check**

Ask and answer: “As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability?” If revision is needed, take corrective action before committing.

- [ ] **Step 7: Commit**

Run:

```bash
git add crates/hawk2ui-runtime/Cargo.toml crates/hawk2ui-runtime/src/lib.rs crates/hawk2ui-runtime/src/view.rs crates/hawk2ui-runtime/tests/runtime_behavior.rs
git commit -m "Add runtime scene bridge vertical"
```

Expected: commit succeeds.

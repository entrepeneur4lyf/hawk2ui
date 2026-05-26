# Svelte Runtime Bridge Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Upgrade Svelte 5 integration so supported Hawk Svelte source compiles into native runtime bridge output and Skia-visible pixels.

**Architecture:** Keep the existing `compile` API stable, add `compile_to_runtime` that internally builds a native authoring tree and invokes `NativeRuntimeBridge`. Update the public TypeScript package to emit deterministic native record strings instead of an empty record list.

**Tech Stack:** Rust, `hawk2ui-framework-svelte`, `hawk2ui-authoring`, `hawk2ui-runtime`, `hawk2ui-render-skia`, TypeScript package source, Bun/TypeScript syntax checks where available.

---

## Files

- Modify: `crates/hawk2ui-framework-svelte/Cargo.toml` to depend on runtime/layout/render crates and Skia as a dev-dependency.
- Modify: `crates/hawk2ui-framework-svelte/src/lib.rs` to add runtime artifact API and native authoring conversion.
- Modify: `crates/hawk2ui-framework-svelte/tests/svelte_integration.rs` for runtime bridge and pixel tests.
- Modify: `packages/hawk2ui-svelte/src/index.ts` to emit deterministic records.

## Task 1: Rust Runtime Artifact API

**Files:**
- Modify: `crates/hawk2ui-framework-svelte/Cargo.toml`
- Modify: `crates/hawk2ui-framework-svelte/src/lib.rs`
- Modify: `crates/hawk2ui-framework-svelte/tests/svelte_integration.rs`

- [ ] **Step 1: Write the failing test**

Add this test to `crates/hawk2ui-framework-svelte/tests/svelte_integration.rs`:

```rust
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
    assert_eq!(artifact.metadata_for("root").unwrap().style_refs(), ["surface.card"]);
    assert_eq!(artifact.metadata_for("root").unwrap().asset_paths(), ["assets/logo.svg"]);
    assert!(artifact.operation_keys().contains(&"mount-element:root".to_string()));
    assert!(artifact.operation_keys().contains(&"bind-event:root:pointer.press".to_string()));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p hawk2ui-framework-svelte svelte_5_compile_to_runtime_uses_native_bridge_contract
```

Expected: compile failure for missing `compile_to_runtime`, `SvelteRuntimeArtifact`, and runtime imports.

- [ ] **Step 3: Implement runtime artifact API**

Add dependencies on `hawk2ui-runtime`, `hawk2ui-layout`, and `hawk2ui-render`. Add `SvelteRuntimeArtifact` with `compiled`, `runtime_tree`, `metadata_for`, and `operation_keys` accessors. Implement `compile_to_runtime` by converting supported Svelte source into `NativeAuthoringElement`, finalizing it with `NativeAuthoringRuntime`, and bridging with `NativeRuntimeBridge`.

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p hawk2ui-framework-svelte svelte_5_compile_to_runtime_uses_native_bridge_contract
```

Expected: one passing test.

- [ ] **Step 5: Review Check**

Ask and answer: “As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability?” If revision is needed, take corrective action before continuing.

## Task 2: Rust Skia Pixel Proof

**Files:**
- Modify: `crates/hawk2ui-framework-svelte/Cargo.toml`
- Modify: `crates/hawk2ui-framework-svelte/tests/svelte_integration.rs`

- [ ] **Step 1: Write the failing test**

Add this test to `crates/hawk2ui-framework-svelte/tests/svelte_integration.rs`:

```rust
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
    backend.create_surface("main", 220, 120).expect("surface creates");
    backend.begin_frame("main").expect("frame begins");
    backend.clear(Color::rgba(0, 0, 0, 255)).expect("surface clears");
    render_runtime_frame_with_skia(&frame, &mut backend);
    backend.end_frame("main").expect("frame ends");

    let snapshot = backend.frame_snapshot("main").expect("snapshot exists");
    assert!(snapshot.pixels().iter().any(|pixel| *pixel == 0x080a0e));
    assert!(count_changed_pixels(snapshot, 0x080a0e, frame.geometry_for(&RuntimeViewId::new("title")).unwrap()) > 0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p hawk2ui-framework-svelte svelte_5_runtime_bridge_renders_visible_skia_pixels
```

Expected: compile failure until test helpers and Skia dev-dependency are present.

- [ ] **Step 3: Implement test helpers and dev dependency**

Add `hawk2ui-render-skia` as a dev-dependency. Add local test helpers to execute `RuntimeDrawCommand` through `SkiaRendererBackend` and count changed pixels.

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p hawk2ui-framework-svelte svelte_5_runtime_bridge_renders_visible_skia_pixels
```

Expected: one passing test.

- [ ] **Step 5: Review Check**

Ask and answer: “As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability?” If revision is needed, take corrective action before continuing.

## Task 3: TypeScript Package Records

**Files:**
- Modify: `packages/hawk2ui-svelte/src/index.ts`
- Optionally create: `packages/hawk2ui-svelte/src/index.test.ts` only if Bun can run package tests without adding a package manager scaffold.

- [ ] **Step 1: Write the failing verification**

Run this command to prove the current package returns empty records:

```bash
bun --eval 'import { compileHawkSvelte } from "./packages/hawk2ui-svelte/src/index.ts"; const out = compileHawkSvelte({ filename: "App.svelte", source: `<hawk-view id="root" use:ref="root_ref" class="surface.card" data-asset="assets/logo.svg" on:press={handlePress} on:mount={onMount} on:destroy={onDestroy}>{#each items as item (item.id)}<hawk-text id={item.id}>{item.id}</hawk-text>{/each}</hawk-view>` }); if (out.records.length === 0) throw new Error("records are empty"); console.log(out.records.join("\n"));'
```

Expected: failure with `records are empty`.

- [ ] **Step 2: Implement deterministic records**

Update `compileHawkSvelte` to parse the supported attributes and fixture markers. Emit stable records including `mount-element:root`, `ref:root:root_ref`, `style:root:surface.card`, `asset:root:assets/logo.svg`, `bind-event:root:pointer.press`, lifecycle records, and keyed child mounts.

- [ ] **Step 3: Run verification to prove records are non-empty**

Run the same `bun --eval` command.

Expected: output contains the deterministic record strings and exits 0.

- [ ] **Step 4: Verify unsafe asset rejection**

Run:

```bash
bun --eval 'import { compileHawkSvelte } from "./packages/hawk2ui-svelte/src/index.ts"; try { compileHawkSvelte({ filename: "Broken.svelte", source: `<hawk-view data-asset="https://example.invalid/logo.svg" />` }); throw new Error("unsafe asset accepted"); } catch (error) { if (!String(error).includes("svelte.asset.path-invalid")) throw error; console.log("unsafe asset rejected"); }'
```

Expected: `unsafe asset rejected`.

- [ ] **Step 5: Review Check**

Ask and answer: “As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability?” If revision is needed, take corrective action before continuing.

## Task 4: Full Verification and Commit

**Files:**
- All changed files from prior tasks.

- [ ] **Step 1: Format and package tests**

Run:

```bash
cargo fmt --all --check
cargo test -p hawk2ui-framework-svelte
cargo test -p hawk2ui-authoring
cargo test -p hawk2ui-runtime
```

Expected: exit code 0 for each command.

- [ ] **Step 2: Workspace gates**

Run:

```bash
cargo check --workspace
cargo clippy --workspace -- -D warnings
```

Expected: exit code 0 for both commands.

- [ ] **Step 3: TypeScript package verification**

Run the two `bun --eval` commands from Task 3.

Expected: non-empty records and unsafe asset rejection.

- [ ] **Step 4: GitNexus scope check**

Run GitNexus detect changes for all uncommitted changes.

Expected: affected scope matches the Svelte runtime bridge vertical.

- [ ] **Step 5: Review Check**

Ask and answer: “As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability?” If revision is needed, take corrective action before committing.

- [ ] **Step 6: Commit**

Run:

```bash
git add Cargo.lock crates/hawk2ui-framework-svelte/Cargo.toml crates/hawk2ui-framework-svelte/src/lib.rs crates/hawk2ui-framework-svelte/tests/svelte_integration.rs packages/hawk2ui-svelte/src/index.ts
git commit -m "Add Svelte runtime bridge"
```

Expected: commit succeeds.

# Authoring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the typed authoring model that compiles declarative UI source, component output, event bindings, and state references into runtime-ready records.

**Architecture:** Keep author source parsing separate from runtime records. Framework adapters emit the same typed element tree so no integration owns host lifecycle or rendering.

**Tech Stack:** Rust, serde, rowan or ungrammar for source parsing, TypeScript fixture files, cargo test.

---

## File Structure

- Create: `crates/hawk2ui-authoring/src/lib.rs` public authoring API.
- Create: `crates/hawk2ui-authoring/src/element.rs` element/component records.
- Create: `crates/hawk2ui-authoring/src/events.rs` native event binding records.
- Create: `crates/hawk2ui-authoring/src/state.rs` state and subscription records.
- Create: `crates/hawk2ui-authoring/src/compile.rs` source-to-record compilation entrypoint.
- Create: `crates/hawk2ui-authoring/tests/authoring_records.rs` behavior tests.
- Create: `fixtures/authoring/basic_component.hawk` source fixture.

## Tasks

### Task 1: Element Records

- [ ] Define `ElementId`, `ElementKind`, `ElementNode`, `PropValue`, `ChildList`, and keyed child records.
- [ ] Write tests for stable node identity, child order preservation, and keyed child uniqueness.
- [ ] Run: `rtk cargo test -p hawk2ui-authoring element_records`.
- [ ] Commit: `Add typed authoring element records`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 2: Component And Custom Surface Records

- [ ] Add component instance records with props, references, child slots, and custom draw surface declarations.
- [ ] Test that custom controls and custom surfaces compile to distinct typed records.
- [ ] Run: `rtk cargo test -p hawk2ui-authoring component_records`.
- [ ] Commit: `Add component authoring records`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 3: Event Binding Records

- [ ] Add native event kinds for pointer, keyboard, focus, input, resize, lifecycle, custom component, and plugin parameter events.
- [ ] Add tests proving event records do not depend on browser event object names or shapes.
- [ ] Run: `rtk cargo test -p hawk2ui-authoring event_records`.
- [ ] Commit: `Add native event binding records`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 4: State Records

- [ ] Add component state, app state, UI preference, plugin binding, subscription, batched update, and teardown records.
- [ ] Add tests for deterministic teardown ordering and batched update grouping.
- [ ] Run: `rtk cargo test -p hawk2ui-authoring state_records`.
- [ ] Commit: `Add authoring state records`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 5: Source Compiler Entrypoint

- [ ] Implement `compile_authoring_source(input, diagnostics) -> AuthoringArtifact` with structured diagnostics.
- [ ] Add a basic fixture that compiles into one component with two text children and one click event.
- [ ] Run: `rtk cargo test -p hawk2ui-authoring compile_basic_fixture`.
- [ ] Commit: `Compile authoring source to typed records`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 6: Framework Adapter Contract

- [ ] Add `NativeRendererAdapter` trait that accepts typed node operations and event bindings.
- [ ] Add contract tests using a recording adapter for React, Vue, Svelte, and Solid labels without importing those frameworks.
- [ ] Run: `rtk cargo test -p hawk2ui-authoring adapter_contract`.
- [ ] Commit: `Add framework adapter contract`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

## Verification

- [ ] Run: `rtk cargo fmt --all -- --check`.
- [ ] Run: `rtk cargo test -p hawk2ui-authoring`.
- [ ] Run: `rtk cargo test --workspace authoring`.

# Production Performance Gates Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Define measurable performance budgets and benchmark gates for startup, layout, rendering, text, runtime scheduling, artifact loading, memory, and realtime plugin safety.

**Architecture:** Benchmarks use deterministic fixtures and fail release gates when budgets regress beyond configured thresholds. Audio-thread checks use allocation and blocking guards rather than timing alone.

**Tech Stack:** Rust, criterion, iai-callgrind, dhat or heaptrack instructions, cargo bench, custom realtime guards.

---

## File Structure

- Create: `performance/budgets.toml` production budgets.
- Create: `crates/hawk2ui-perf/src/lib.rs` benchmark helpers.
- Create: `benches/startup.rs` startup benchmarks.
- Create: `benches/layout.rs` layout benchmarks.
- Create: `benches/render.rs` renderer benchmarks.
- Create: `benches/runtime.rs` runtime scheduling benchmarks.
- Create: `benches/plugin_realtime.rs` realtime plugin safety benchmarks.
- Create: `docs/development/performance.md` performance policy.

## Tasks

### Task 1: Performance Budgets

- [ ] Define budgets for cold start, artifact load, first frame, layout pass, scene export, frame render, text measurement, runtime event dispatch, memory, and package size.
- [ ] Add tests that every budget has unit, target, threshold, fixture, and release gate fields.
- [ ] Run: `rtk cargo test -p hawk2ui-perf budget_validation`.
- [ ] Commit: `Add production performance budgets`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 2: Startup And Artifact Benchmarks

- [ ] Add benchmarks for manifest load, sealed artifact load, runtime initialization, first scene construction, and first frame request.
- [ ] Add fixtures for small app, dense dashboard, and plugin editor.
- [ ] Run: `rtk cargo bench --bench startup`.
- [ ] Commit: `Add startup performance benchmarks`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 3: Layout And Text Benchmarks

- [ ] Add benchmarks for flex layout, scroll layout, dense parameter panel layout, text measurement, wrapping, and font invalidation.
- [ ] Add regression thresholds connected to `performance/budgets.toml`.
- [ ] Run: `rtk cargo bench --bench layout`.
- [ ] Commit: `Add layout and text performance benchmarks`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 4: Rendering Benchmarks

- [ ] Add benchmarks for scene export, layer sorting, dirty-region submission, image layers, vector layers, gradients, shadows, custom controls, and graph surfaces.
- [ ] Add deterministic renderer recording output for benchmark validation.
- [ ] Run: `rtk cargo bench --bench render`.
- [ ] Commit: `Add rendering performance benchmarks`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### Task 5: Runtime And Plugin Realtime Gates

- [ ] Add benchmarks for event dispatch, batched state updates, timer scheduling, animation ticks, host callbacks, and shutdown cancellation.
- [ ] Add realtime guards that detect allocation, blocking waits, filesystem, network, script, and rendering calls from audio-thread contexts.
- [ ] Run: `rtk cargo bench --bench runtime`.
- [ ] Run: `rtk cargo test --workspace realtime_guard`.
- [ ] Commit: `Add runtime and realtime performance gates`.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

## Verification

- [ ] Run: `rtk cargo bench --workspace`.
- [ ] Run: `rtk cargo test --workspace performance`.

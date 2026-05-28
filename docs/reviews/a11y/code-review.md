# Code Review — hawk2ui-a11y

**Reviewed at:** `5a042c51` · 2026-05-28 · working tree clean at review time
**Scope:** `src/{lib,tree,host,component,actions,plugin}.rs` + `tests/accessibility_tree.rs` (~7 files, ~470 LoC src / ~300 LoC tests)
**Purpose:** Accessibility tree records, headless component semantics, AccessKit host export, and plugin-thread a11y safety guard.
**Note:** Codex is concurrently committing hardening/remediation work; HEAD drifts. Findings reflect the SHA above and may overlap with in-progress fixes.

## Summary

This crate is **genuinely implemented, not a stub**: `A11yHostExporter::export_accesskit_update` performs real AccessKit `TreeUpdate` construction — role/action mapping, stable `NodeId` assignment, bounds validation, and single-focus enforcement. Documentation is thorough and `#![forbid(unsafe_code)]` holds. The substantive weaknesses are (1) an action dispatcher that records intent but is **inert** for `Press`/`Increment`/`Decrement`/`Custom`, (2) **unbounded recursion** over `Deserialize`-able trees (stack-overflow DoS), and (3) several AccessKit semantics that are silently dropped (numeric slider values, list item counts, custom action names). No correctness blocker on the desktop happy path; real a11y-fidelity and robustness gaps.

## Completeness & implementation gaps

1. **Inert action handlers** (`actions.rs::dispatch`). Only `Focus` and `SetValue` mutate the tree. `Press`, `Increment`, `Decrement`, and `Custom(_)` fall through to `=> {}` — the event is pushed to history but produces **no state change**. `Increment`/`Decrement` are no-ops because there is no numeric value model (value is a free-form `String`, e.g. `"-6 dB"`). The test asserts only `events().len() == 6`, so the no-op is invisible. → Implement value stepping or explicitly document these as record-only.
2. **No "supported action" gate.** `dispatch` will action any node regardless of whether the node's advertised `actions` list contains it (e.g. `Increment` on a `Button`).
3. **Focus dispatch never clears prior focus** (`actions.rs`). `Focus` sets `focused = true` on the target without clearing others. Dispatching focus twice yields **two focused nodes** — a state `export_accesskit_update` then *rejects* (`a11y.accesskit.multiple-focused-nodes`). The dispatcher can drive the tree into a state the exporter refuses (cross-file inconsistency: `actions.rs` vs `host.rs`).
4. **List item counts are dropped** (`component.rs` + `host.rs`). `ComponentSemantics::list` stores `item_count`, but the exported node has no children/items and the export never sets AccessKit `set_size`/`set_position_in_set`. Lists announce no count to assistive tech.
5. **Numeric slider semantics missing.** Slider `value` is a `String`; AccessKit `set_numeric_value`/min/max/step are never populated, so screen readers get no numeric value or range.
6. **Custom action names lost** (`host.rs::action_to_accesskit`). `A11yAction::Custom(name)` → `Action::CustomAction` with `name` discarded; multiple custom actions are indistinguishable to AT.
7. **`Panel` and `Custom` both map to `Role::Pane`** (`host.rs::role_to_accesskit`). Custom controls carry no distinct role; combined with (6), custom controls are weakly described.

## Code quality & smells

- **`LayoutGeometryUpdate.node_id: &'static str`** (`host.rs`). Node IDs are `String` everywhere else (runtime/dynamic). Forcing geometry updates to `&'static str` means layout-driven geometry for dynamically-named nodes can't be expressed without leaking strings. Should be `&str`/`String`.
- **`assert_serde` is a compile-time bound check, not a round-trip** (tests). `tree_records_are_serializable_contracts` calls `assert_serde::<T>()` (empty body) — it proves `T: Serialize + DeserializeOwned` compiles, not that real data round-trips. The test name overstates the guarantee.
- **`A11yActionDispatcher` derives `Deserialize` with private fields** including `events` history — it can be reconstituted with fabricated history. Minor; likely for snapshot tests.
- **`A11yActionDispatcher.events` grows unbounded** — no cap/drain (see Security).

## Documentation

- Strong: crate-level `//!`, every public type/fn documented, `# Errors` sections on fallible fns. No undocumented public items found.
- Minor: the recursive `find`/`find_mut` carry no note about depth/stack expectations (relevant given the DoS below).

## Testing

- Good breadth: tree shape, component semantics, action happy path, host geometry + error→`Diagnostic`, full AccessKit export, multi-focus rejection, plugin guard allow/deny.
- Gaps (no coverage): `a11y.accesskit.invalid-id` (empty id), `a11y.accesskit.duplicate-id`, id-overflow, and — most importantly — **no test that `Increment`/`Decrement`/`Press` change anything** (which masks finding #1), and no test for the focus-dispatch → multi-focus → export-reject inconsistency (#3).

## Cross-cutting conventions (workspace-wide; noted once, not a per-crate defect)

- `crate_name()` plus a duplicate `*_workspace_filter_marker` test that re-asserts it — a marker for the `domain_test_templates`/`api_contract` filter gates; recurs in every crate.
- `#![forbid(unsafe_code)]` at crate root; domain errors convert into `hawk2ui_api::Diagnostic` via `From`. Consistent and good.

## Security

- **Unbounded recursion → stack-overflow DoS** (`host.rs`, `tree.rs`). `A11yNode::find/find_mut`, `assign_accesskit_ids`, `collect_accesskit_nodes`, and `collect_focused_node_ids` recurse over `children` with no depth limit. `A11yTree`/`A11yNode` derive `Deserialize`, so a deeply nested tree from untrusted input (a deserialized snapshot, or a plugin/script-provided tree) can overflow the stack and crash the host/editor process. **Highest-severity item in this crate.** Mitigation: depth-bounded or iterative traversal with a cap, and/or a max-depth check when deserializing.
- **Unbounded `events` Vec** (`actions.rs`). Action history grows without limit; a runaway or hostile action stream is a slow memory-exhaustion vector. Bound it or make retention opt-in.
- **Verbatim string pass-through to AccessKit.** `name`/`description`/`value` are forwarded unbounded; AccessKit tolerates arbitrary text (not an injection vector), but there is no length bound — pairs with the DoS theme.
- **Good input validation at the export boundary:** empty-id, duplicate-id, id-overflow, and non-finite/negative bounds are all rejected before export.
- No `unsafe`. The plugin guard correctly denies a11y work on the audio (realtime) thread and denies unstable host calls.

### Severity-ranked findings

| # | Severity | Finding | Location |
|---|----------|---------|----------|
| 1 | High | Unbounded recursion over `Deserialize`-able tree → stack-overflow DoS | `host.rs`, `tree.rs` |
| 2 | Medium | Inert `Press`/`Increment`/`Decrement`/`Custom` dispatch (records, no effect) | `actions.rs::dispatch` |
| 3 | Medium | Focus dispatch can create multi-focus state the exporter rejects | `actions.rs` vs `host.rs` |
| 4 | Medium | Numeric slider value + list item count never exported to AccessKit | `component.rs`, `host.rs` |
| 5 | Low | Custom action names dropped; `Panel`/`Custom` both → `Role::Pane` | `host.rs` |
| 6 | Low | `LayoutGeometryUpdate.node_id: &'static str` inconsistent with `String` ids | `host.rs` |
| 7 | Low | Unbounded `events` history (memory growth) | `actions.rs` |
| 8 | Low | `assert_serde` checks bounds only, not round-trip; missing error-path tests | `tests/accessibility_tree.rs` |

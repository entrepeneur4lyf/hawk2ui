# Task List 0009: Truce Editor Parameter & Meter Seam (C3)

## Purpose

Track implementation of the editor-JS parameter/meter seam on truce: read projection, gesture-bracketed write-back, the non-advancing-read invariant enforcement, and the projection cadence. Builds on C2b (`fc85aa8f`, the editor renders its scene from the entry script).

## Sources

- Decision (local ADR — `docs/decisions/` is gitignored, so this lives on disk and in memory, not in the repo, like 0001/0002): `docs/decisions/0003-c3-editor-param-projection-protocol.md` (the contract this list executes; must be *Accepted* before any task here).
- Decision: `docs/decisions/0002-stable-architecture-baseline.md` §17 (stable string IDs).
- Spec: `specs/0009-plugin.md`.
- truce ground truth: `reference/truce-0.49.14/` (`truce-core/src/editor.rs`, `truce-params/src/{types,info}.rs`, `truce-derive/src/lib.rs`).
- boa execution model: `crates/hawk2ui-script/src/lib.rs` (`entry_mount_bootstrap:102-118`, `execute_module:454-467` — fresh, stateless, recompiled per call).

## Gate

**`tasks/0010` (parameter-id stability) is the prerequisite brick and must land first.** It is a separate task list with a different crate/blast-radius, sequenced ahead of this one. Every task here depends on `tasks/0010` complete **and** Decision 0003 *Accepted*.

## Tasks

### 0009.1 Read Projection (params + meters → typed snapshot)

Split into two units verified **independently** — the script-side JS surface (0009.1a) and the editor/model wiring (0009.1b). **Both project the parameter model's declared _defaults_, not live host state.** Reading the live truce `EditorBridge` (`get_param`/`get_param_plain`/`format_param`/`get_meter`) and re-projecting is task 0009.4: the bridge only exists at `Editor::open`, while C2b builds the scene at construction, so defaults-first is the honest staging, not a shortcut.

#### 0009.1a Script-side host projection — ✅ DONE (`cbd24dad`)

- [x] Deliverable: `entry_mount_bootstrap_with_host(source, &HostSnapshot)` regenerates the entry bootstrap with a `__hawk2ui_host` carrying a frozen JSON-literal snapshot (no host-call capability; `deny_all` preserved). String-keyed accessors `host.params`/`host.param(key)` + `host.meters`/`host.meter(key)` — two separate read channels; `host.meter(key)` returns a bare level (meters get no view). `value` typed by kind (float→number, int→integer, bool→boolean, enum→variant index), never flattened to f64. Unknown key throws. Types `HostSnapshot`/`HostParam`/`HostMeter`/`HostParamKind`/`HostParamValue` live in `hawk2ui-script`.
- [x] Verify: `cargo test -p hawk2ui-script projects_params_and_meters_into_the_entry_host` · `… an_unknown_param_or_meter_key_throws`.
- [x] Review check: satisfied — pure data projection, deny_all-preserving; gated green.

#### 0009.1b Editor + model→snapshot wiring — ✅ DONE

- [x] Deliverable: `host_snapshot_from_model(&ParameterModel) -> HostSnapshot` in `hawk2ui-build` (the one crate with both `ParameterModel` and `HostSnapshot`; `hawk2ui-plugin-truce` stays decoupled, knowing only the projected type) maps each parameter's _default_ to its typed `HostParam` (kind/value/normalized/text/variants) and meters to their floor `0.0`. Enum-default normalized is computed as `index/(count-1)` — the `Choice` case `ParameterRecord::normalize` rejects, so a last-variant default reads back `1.0` rather than a swallowed `0.0`. `build_editor_scene` and `Hawk2uiTruceEditor::{from_entry_script,try_from_entry_script}` thread a `&HostSnapshot` into `entry_mount_bootstrap_with_host`; the projection set is re-exported from `hawk2ui-plugin-truce`.
- [x] Dependencies: `0009.1a`, `tasks/0010` complete, Decision 0003 *Accepted*.
- [x] Verify: `cargo test -p hawk2ui-build editor_host` (kind-typed projection · enum-on-last-variant→normalized 1.0 · meters at floor · **round-trip through the real bootstrap** so the mapping ↔ JS-surface halves meet, not just pass in isolation) · `cargo test -p hawk2ui-plugin-truce projects_the_host_snapshot` (projection reaches the built scene).
- [x] Review check: satisfied — gated `check-fast` + clippy pedantic clean. The construction-time `&HostSnapshot` arg is expected to migrate to `open()` when 0009.4 moves scene-build there.

#### 0009.1c Snapshot `id` correction — ✅ DONE

- [x] **Honest correction:** 0009.1a/b shipped the projected snapshot **missing the `id` field**, which Decision 0003 D2 (line 68 `"id": 3`), D1 (line 55 "carries `{key, id}`"), and Lock 2 (meters carry their `METER_ID_BASE + index` u32) all require. The omission was caught during 0009.2 design, before any write-back consumer shipped — the snapshot was incomplete vs the signed-off read shape, not wrong in what it did project.
- [x] Deliverable: `HostParam.id` and `HostMeter.id` (`u32`) added; `host_snapshot_from_model` populates params from `resolved_param_ids()` (truce `ParamId`, Lock 1) and meters as `METER_ID_BASE + declaration_index` (Lock 2). The id projects into JS (`host.param(key).id`) and is the host-side key→u32 routing 0009.2 write-back consumes.
- [x] Verify: `cargo test -p hawk2ui-build editor_host` (param ids 0/1/2/3 in declaration order; meter ids at `METER_ID_BASE + index` on a 2-meter model — silent-drift guard) · `cargo test -p hawk2ui-script projects_params_and_meters_into_the_entry_host` (asserts `id` reaches JS).
- [x] Review check: satisfied — completes the contract read shape; gated green.

### 0009.2 Write-Back (return-carried edit list, gesture-bracketed, host-threaded state)

Split into the script-side protocol surface (0009.2a) and the host-side replay (0009.2b). Built outward from the **locked wire shape** (`execution.value()` = `JSON.stringify({tree, edits, ui})`, Decision 0003 D3): the shape and the parse are stable, the author-facing verb API is additive on top.

#### 0009.2a Script-side write surface + envelope + parse — ✅ DONE

- [x] Deliverable (all in `hawk2ui-script`): `entry_mount_bootstrap_with_host` gains write verbs `host.beginEdit/endEdit/setParam/setParamPlain/automate` that record an **ordered** edit list (each validates the key against the snapshot — a write to an unknown or meter key throws, so meters stay structurally unwritable at the JS layer too), and threads UI state — `incoming_ui` exposed as `host.ui`, `host.setUi(value)` sets the outgoing blob (defaulting to incoming when untouched), kept distinct from truce's plugin `set_state`. The entry returns the C2b tree; the bootstrap assembles `JSON.stringify({tree: mount(host), edits, ui})`. New types `HostEdit` (`begin`/`set`/`setPlain`/`automate`/`end`), `EntryEnvelope`, `EnvelopeError`, and `parse_entry_envelope` (parse lives with the emitter, so `hawk2ui-plugin-truce` needs no JSON dep — `build_editor_scene` now consumes the envelope's `tree_json`). **Reading note (faithful, not a deviation):** D3 shows `{tree,edits,ui}` "returned by mount"; since the verbs are also contracted (D1/D3) and re-returning the edits would be redundant, the only non-contradictory reading is *verbs record → mount returns the tree → bootstrap assembles the envelope*.
- [x] Dependencies: `0009.1` (a/b/c), Decision 0003 *Accepted*.
- [x] Verify: `cargo test -p hawk2ui-script` (ordered edit list across all five verbs; UI threads in→out and defaults when untouched; unknown write key throws; `parse_entry_envelope` splits/defaults/rejects) · `cargo test -p hawk2ui-build editor_host` (round-trip still green through the envelope). No new `HostCallPolicy` capability — `deny_all` holds.
- [x] Review check: satisfied — wire shape matches the contract; verbs validate; ui ≠ set_state; gated `check-fast` + clippy pedantic green.

#### 0009.2b Host-side replay + range routing + cross-frame gestures — ⏳ NEXT

- [ ] Deliverable: `hawk2ui-build` emits a range-carrying `EditRouting` (key → {id, kind, min, max, variant_count}) from the model; `hawk2ui-plugin-truce` gains a pure `replay_edits(bridge, edits, routing, &mut open_gestures)` that maps key → u32, normalizes `setPlain` via the range (host-normalizes, Decision 0003 D3 line 110), replays `begin_edit`/`set_param`/`end_edit`/`automate` on the bridge, and tracks open gestures **across invocations** (a begin on one frame, end on a later one, replays as one bracket). A bare `set` with no begin/end is valid (not auto-bracketed); double-begin / unmatched-end / unknown key are skip-and-record, never panic. Tested with a **recording bridge** (truce's `for_test_params` bridge is all no-ops), no window needed.
- [ ] Dependencies: `0009.2a`.
- [ ] Verify: `cargo test -p hawk2ui-plugin-truce` (replay order, cross-frame gesture via the open-gesture set, plain→normalized via routing, meter-write impossible, double-begin/unmatched-end skipped) · `cargo test -p hawk2ui-build` (routing from model).
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0009.3 Non-Advancing-Read Invariant Enforcement

- [ ] Deliverable: comment at the `Hawk2uiTruceEditor::bridge` field stating the "read only through the bridge; never capture `context.params()`" invariant and its audio-thread reason; a source-pattern conformance gate (xtask, same enforcement class as `source_hygiene`) asserting `crates/hawk2ui-plugin-truce/src/` stores no truce param-store type (`Arc<dyn Params>` / `PluginContext` field / captured `context.params()`); the gate fails CI if the invariant is broken.
- [ ] Dependencies: `0009.1`.
- [ ] Verify: `cargo run -p xtask -- check-fast` includes the new gate · a deliberately-violating fixture fails it.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0009.4 Projection Cadence / Render Loop

- [ ] Deliverable: re-project + re-invoke the single per-frame entry function on (a) user input, (b) host-driven param changes, (c) per presented frame for meters (vsync cadence, never per audio block); Rust-side retention via `RuntimeSceneBridge` (JS describes the current frame, host diffs it); per-run cost bounded by `ScriptExecutionLimits::DEFAULT`; flag (do not necessarily adopt) the parsed-program cache that drops the per-frame re-parse while staying stateless and `deny_all`-safe.
- [ ] Dependencies: `0009.1`, `0009.2`.
- [ ] Verify: `cargo test -p hawk2ui-plugin-truce cadence` · `cargo test -p hawk2ui-smoke --test smoke_apps` (plugin fixture presents from-script frames reflecting a param change).
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

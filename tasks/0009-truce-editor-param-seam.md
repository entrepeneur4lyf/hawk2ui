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

#### 0009.2b Host-side replay + range routing + cross-frame gestures — ✅ DONE

- [x] Deliverable: `hawk2ui-script` defines `ParamRoute`/`EditRouting` (lives here so both producer and consumer can name it w/o coupling) with `ParamRoute::normalize_plain` (host-side plain→normalized, Decision 0003 D3 line 110). `hawk2ui-build`'s `edit_routing_from_model` builds it from the model (key → {id, kind, min, max, variant_count}; meters excluded — read-only). `hawk2ui-plugin-truce`'s pure `replay_edits(bridge, edits, routing, &mut open_gestures)` resolves key → u32, normalizes `setPlain` via the route, replays `begin_edit`/`set_param`/`end_edit`/`automate` on the bridge, and tracks open gestures **across invocations** (begin one frame, end a later one → one bracket). A bare `set` is valid (not auto-bracketed); double-begin / unmatched-end / unknown key (incl. any meter key) are **skip-and-record** (`EditReplayDiagnostic`), never panic. The editor's field/loop wiring (holding the routing + open-gesture set, calling replay per invocation) is 0009.4 — the replay itself is pure and tested standalone.
- [x] Dependencies: `0009.2a`.
- [x] Verify: `cargo test -p hawk2ui-plugin-truce` (9 replay tests via a **recording bridge** — truce's `for_test_params` bridge no-ops writes, so it can't witness one: in-order bracket, bare set, clamp, plain→normalized, automate one-shot, cross-frame bracket, double-begin/unmatched-end/unknown-key skip+record) · `cargo test -p hawk2ui-script` (`normalize_plain` by kind, routing lookup) · `cargo test -p hawk2ui-build` (`edit_routing_from_model` id/kind/range + meter-miss).
- [x] Review check: satisfied — pure replay, never panics, host-normalizes per the contract, meters structurally unwritable; gated `check-fast` + clippy pedantic green.

### 0009.3 Non-Advancing-Read Invariant Enforcement — ✅ DONE

- [x] Deliverable: an INVARIANT comment at the `Hawk2uiTruceEditor::bridge` field stating "bridge only, never the typed param store" and its audio-thread reason (a captured store exposes a `FloatParam` whose advancing `read()` could perturb the audio thread from a GUI repaint; reading only through the bridge makes that read **unreachable**). Enforcement is a **conformance test** (not xtask — `source_hygiene` is itself a conformance test, so this joins it in `crates/hawk2ui-conformance/tests/source_hygiene.rs`, the same enforcement class): `truce_editor_crate_never_captures_a_param_store` scans `crates/hawk2ui-plugin-truce/src/` (test modules stripped, so the gated smoke and future src tests may still construct params) for the two patterns that reach the store — `.params()` (the capture call; the context's `params` field is private, so this is the *only* way to it) and `dyn Params` (the store trait-object field). The field comment is phrased to avoid those literals so it does not trip its own gate.
- [x] Dependencies: `0009.1`.
- [x] Verify: `cargo test -p hawk2ui-conformance param_store` (the invariant holds for the real crate today + a positive-control test proving the gate detects a capture in production source and strips a test-module one — non-vacuous) · runs inside `check-fast`.
- [x] Review check: satisfied — sound reachability argument (no `FloatParam` without `.params()`); gated `check-fast` + clippy pedantic green.

### 0009.4 Projection Cadence / Render Loop

Split into the testable render-cycle logic (0009.4a) and the live windowing wiring (0009.4b). Both **DONE**: 4a builds and unit-tests the per-frame cycle; 4b drives it from the Baseview frame loop, validated under Xvfb. The editor now reflects live host param/meter state every frame (cadence drivers b + c). **One driver is consciously deferred:** (a) user-input-driven edits await the entry-script input API (`host.on` is still a stub) — see 0009.4b's honest scope note. The write/replay path those edits will use is built and tested (0009.2).

#### 0009.4a Render-cycle logic (live bridge read → invoke → replay → persist) — ✅ DONE

- [x] Deliverable (all in `hawk2ui-plugin-truce`, fully unit-tested against a fake bridge, no window): `build_editor_scene` refactored to expose `build_editor_frame -> (scene, edits, ui)` (construction path drops edits/ui via a thin wrapper, signature unchanged so consumers are safe). `refresh_snapshot_from_bridge(template, bridge)` produces the **live** snapshot — keeps each entry's static fields (key/id/kind/variants) from the construction-time template, refreshes value/normalized/text/meter from the non-advancing bridge reads (`get_param_plain`/`get_param`/`format_param`/`get_meter`, D2), value typed by kind (`host_param_value`, Bool via `> f64::EPSILON`). `EditorRenderState::render(bridge)` runs one cycle: refresh → `build_editor_frame` (threading the persisted `ui_json`) → `replay_edits` (tracking `open_gestures` across cycles) → persist `ui_json` → `RenderOutcome { scene, diagnostics }`, returning `Result` (the caller degrades — wired in 4b — never panics into the host UI thread). The cycle's items carry a scoped `#[allow(dead_code)]` until 4b drives them.
- [x] Dependencies: `0009.1`, `0009.2`.
- [x] Verify: `cargo test -p hawk2ui-plugin-truce` (refresh keeps static + refreshes dynamic typed-by-kind; `host_param_value` per kind; a full cycle: live value reaches the scene (`v1200`), edits replay onto the bridge keyed through the routing, and the ui frame counter threads across two cycles (`frame1`→`frame2`) — all observed through the public scene/bridge).
- [x] Review check: satisfied — pure, tested, no-window; `EditorRenderState` is the composable unit 4b holds; gated `check-fast` + clippy pedantic green.

#### 0009.4b Live windowing wiring + render loop — ✅ DONE (validated under Xvfb)

- [x] Deliverable: `BaseviewGlSkiaFrameHandler` gained a `scene_producer: Option<EditorSceneProducer>` (`Box<dyn FnMut() -> RuntimeSceneFrame + Send>`); `on_frame` calls it each frame to refresh the presented scene (`with_scene_producer` builder; `open_gpu_editor_window` threads it). The editor retains an `EditorRenderState` (from `from_entry_script`/`try_from_entry_script`, now taking `&EditRouting`) and at `Editor::open` — where the bridge exists — builds the producer (`build_scene_producer`): one `EditorRenderState::render(bridge)` per frame. **Failure policy:** the producer degrades — on a failed cycle it keeps the last-good scene and records the error into `last_error` (so `has_error` reflects it), never panicking into the host UI thread. **Send/'static:** the closure owns the render state + `Arc<dyn EditorBridge>` + `last_error` + last-good scene, so the handler stays `Send` for `WindowHandler` (confirmed: the GL handler compiles + the smoke runs). The 4a/scene `dead_code` allowances came off (the cycle is now driven); only `RenderOutcome::diagnostics` keeps one (the producer drops them for now).
- [x] Dependencies: `0009.4a`.
- [x] Verify: **`HAWK2UI_NATIVE_BASEVIEW_SMOKE=1 xvfb-run -a cargo test -p hawk2ui-plugin-truce --test native_truce_editor_smoke`** — passes under Xvfb (real X11 window, the **producer-driven** loop presents ≥1 frame with no error, bridge captured; a failed cycle would set `has_error`, so green proves a clean live cycle). Plus a no-window unit test that only a script-built editor retains a live render state. `check-fast` + clippy pedantic green.
- [x] Review check: satisfied — see the honest scope note below.

**Honest scope of the live loop (not rounded up):** the per-frame loop drives cadence drivers **(b) host param changes** and **(c) per-frame meters** — the editor now reflects live host/automation state every frame and replays any edits the entry emits, persisting UI state across frames. Driver **(a) user input** is **not yet wired**: the entry-script input API (`host.on`) is still a no-op stub and no path feeds Baseview input events into the entry, so the editor is *live-reflecting* but not yet *interactive* (a knob drag can't emit an edit yet). Wiring input → entry (the editor event-dispatch feature) is the C3 follow-on; the write/replay machinery it will drive is already built and tested (0009.2). The parsed-program-cache optimization is also still flagged-not-adopted (per-frame recompile stands).

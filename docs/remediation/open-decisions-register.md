# Hawk2UI Open Decisions Register

## Purpose

This register holds every gap from the per-crate code reviews that I was about to **defer** rather
than resolve, so they become a **live, actioned queue** instead of "Surfaced / deferred (NOT fixed)"
prose buried in an archived review. That prose is documenting-away; the mandate is *fix real
weaknesses rather than document them away*. Two honest resolutions exist for anything I cannot just
edit in place:

1. **Fix it now** — it is a real defect or unwired false-assurance with an obvious correct action and
   no product fork. These need a go-ahead and sequencing, not a decision. → **Tier 1** below.
2. **Decide it** — there is a genuine product/architecture fork. → **Tier 2** below, each with options
   + a recommendation + a `Decision:` slot.

Re-documenting a decorative surface as "intentional" is **not** an option for either tier.

Companion docs:

- `docs/remediation/production-remediation-register.md` — `REM-<AREA>-NNN` tracker (what must be done
  + status). DEC items cross-reference REM items; line 16 there is binding: *items may be sequenced,
  but are not deferred out of scope.*
- `docs/reviews/<crate>/code-review.md` — the source finding + per-crate Remediation section for each
  contained fix already landed.

Reviews are stamped `5a042c51`; HEAD has drifted, so each item below was re-verified against current
source. Items that drift closed are recorded under **Verified closed**, not asked about.

---

## Tier 1 — Fix now (no decision needed)

Real defects and unwired false-assurance. Recommendation is **fix/remove**; the only open question is
*when*. Proposed: start immediately, severity-first, one commit per crate (same loop as the landed
contained fixes).

| # | Crate | Class | Action |
|---|-------|-------|--------|
| DEC-05a | plugin-adapters | Bug | Fix CLAP multi-instance state corruption (`plugin_data:null` → process-global statics); make the inert `realtime_guard` real or delete its enforcement claim |
| ~~DEC-06~~ | vst3 | Bug | ✅ **DONE** (`411380ba`): Serde `try_from` validation, `c_char` ARM fix, byte-length limits, nil-GUID, byte-order doc; +6 tests |
| DEC-07a | text | Bug | Push resolved family to Parley (`text` #1, 2-line); add input-length cap (DoS); fix truncation width vs `max_width_px` |
| DEC-08 | security | Remove | Remove decorative policy verdicts + tautological tests (zero consumers); keep+harden `SecretValue` redaction (add `Zeroize`) |
| DEC-15 | testkit | Remove | Remove string-`VisualSnapshot` proxy + tautological tests + 6 dangling `fixtures/security/*` helpers; keep strict pixel-diff core |
| DEC-17 | render, render-skia | Bug | Fix compiled-vector opaque-white color loss; fix text re-shape divergence; fix scene-graph O(N²) per-frame path |
| DEC-18 | ~~runtime~~, conformance | Bug | runtime recursion ✅ **already closed** at HEAD (`RUNTIME_SCENE_MAX_DEPTH`, `e7918972`); remaining: complete the conformance anti-panic gate (misses `unreachable!`/`assert!`/index/overflow/recursion) |
| DEC-19 | conformance | Remove | Security fixtures asserted-as-*mentioned*, never validated → remove the false-assurance or wire real validation |

## Tier 2 — Decisions (genuine forks, your call)

| # | Title | Recommendation | Source |
|---|-------|----------------|--------|
| DEC-01 | Framework source-scan fidelity (react/solid/svelte/vue) | **A**: make `from_native_program` the sole honest path; strip fabrications | `framework-*`, REM-AUTH-001 |
| DEC-02 | Effects pipeline end-to-end (gradient/shadow/glow) | Wire style→runtime→render→skia for one vertical, expand by spec | `render`, `render-skia`, REM-RENDER-003/004 |
| DEC-03 | Performance measurement infrastructure | Gate the deterministic subset (artifact bytes, alloc); wall-clock advisory | `perf`, REM-CRATE-006 |
| DEC-04 | Plugin parameter model fidelity (Choice/Bool) | Define parameter-type + state-format model, then wire round-trip | `plugin`, `host-baseview` |
| DEC-05b | Plugin format scope (VST3 / AU / standalone) | Build VST3 `IPluginFactory`/COM first (committed format); keep AU/standalone tracked | `plugin-adapters`, REM-CRATE-006 |
| DEC-07b | Text font-pipeline scope (glyph cache, app fonts) | Scope as a font-pipeline milestone after DEC-07a | `text`, REM-RENDER-005 |
| DEC-09 | `security-model` enforcement wiring | Wire trust validation into build/seal/verify; test forged-input rejection | `security-model` |
| DEC-11 | Style accepted-subset enforcement | Enumerate accepted keyword values; define box-shadow/transform grammar | `style`, REM-STYLE-001 |
| DEC-12 | Host surface trait unification (`HostSurface`/`FramePresenter`) | Route real adapters through it, or mark explicitly test-only | `host` |
| DEC-13 | Platform backends beyond policy/record layer | Decide secret-store + real fs/net/clipboard backend ownership | `platform` |
| DEC-14 | Smoke-test realness | Add build/style/script/host-winit deps so smokes run real paths | `smoke` |
| DEC-16 | Release-evidence: validate docs vs reality | Registry of "production" crates checked by release-check | `xtask` |

**Verified closed (re-verified at HEAD, no action):** DEC-10 (runtime script-engine limits) — the
script crate now has `ScriptExecutionLimits` (`DEFAULT_MAX_LOOP_ITERATIONS=10_000_000`,
`DEFAULT_MAX_NESTING_DEPTH=256` parse-time guard "mirrors `A11Y_MAX_TREE_DEPTH`", source/compiled byte
caps), so the review's `u64::MAX`/unguarded-parser DoS finding is remediated. Confirm enforcement is
exercised by a test; otherwise no decision.

---

# Tier 1 detail

## DEC-05a — CLAP multi-instance state corruption + inert RT guard

**Bug.** `docs/reviews/plugin-adapters/code-review.md`. The CLAP ABI stores all editor + parameter
state in process-global `static`s with `plugin_data: null`; **two instances in one host alias and
corrupt each other's state** — a real correctness bug on the only functional plugin path, masked by
single-instance tests. Separately, `realtime_guard_allows` is a per-variant constant, so the
`plugin_process` gate can never reject (decorative; the process body is in fact allocation-free).
**Action:** thread per-instance state through the CLAP `plugin_data` pointer; make the RT guard
actually evaluate (or delete the enforcement claim + correct the docs). Fixable now regardless of
DEC-05b. **Cross-ref:** REM-CRATE-006.

## DEC-06 — `hawk2ui-vst3` cross-platform & Serde hardening

**Bug.** `docs/reviews/vst3/code-review.md`. All contained, fixable now independent of whether the COM
layer ships (DEC-05b): (a) `as_tuid` produces `[i8;16]` against `TUID=[c_char;16]` → **fails to
compile where `c_char==u8`** (ARM/AArch64 Linux); (b) length limits count Unicode scalars but VST3
fields are byte-bounded `char8[N]` → ASCII 65–127 / multibyte ≤63 **overflows `char8[64]`**; (c) all
public types derive plain `Deserialize` → **Serde bypasses every validating constructor** (add
`#[serde(try_from)]`); (d) `from_u32s` is always big-endian, diverging from the SDK Windows byte
order the doc invites copying. The ARM compile break should not wait on the vertical decision.

## DEC-07a — Text shaping correctness

**Bug.** `docs/reviews/text/code-review.md`; REM-RENDER-005. (a) Resolved font family is **never
pushed to Parley** (only `FontSize`) → metrics are font-independent; `text` #1 is the cheap 2-line
`FontStack` push. (b) **No input-length cap** → unbounded grapheme/bidi/shape work; `truncate_text` is
O(n²) under wide truncation → CPU/memory DoS. (c) Truncation cut points use fake `cluster_width_factor`
constants while reported width is real Parley → `width_px` can exceed/undershoot `max_width_px`. Fix
all three now; the font-discovery/glyph-cache build-out is DEC-07b.

## DEC-08 — `hawk2ui-security` crate: remove decorative verdicts

**Remove-or-wire → remove.** `docs/reviews/security/code-review.md`. The **entire crate is unwired**
(zero production consumers; `security-model` does not depend on it). Its policies *assert* but do not
*enforce*: script-sandbox `deny()` formats a caller-chosen verdict (never touches Boa);
source-validation `reject()` parses nothing; asset-security computes no hash/size/vector. The real
gates live in `hawk2ui-build`/`hawk2ui-script`. False-confidence tests assert "denies all operations"
but check only diagnostic-string formatting. **Action:** delete the decorative verdicts + their
tautological tests; **keep and harden** the one genuine piece — `SecretValue` redaction (add
`Zeroize`/scrub-on-drop; broaden the verbatim-substring leak detector). Re-introduce real policy only
when a consumer exists. **Cross-ref:** REM-GDOC-003.

## DEC-15 — `hawk2ui-testkit`: remove string-snapshot + dangling fixtures

**Remove-or-wire → remove.** `docs/reviews/testkit/code-review.md`. Zero workspace consumers. The
`VisualSnapshot`/`matches_baseline` compares command **strings**, not pixels (cannot catch a render
regression; self-tests are tautological `candidate = baseline.clone()`); security helpers reference
six `fixtures/security/*` paths that **do not exist**. **Action:** remove the string-snapshot proxy +
tautological tests + dangling-fixture helpers; **keep** the genuine strict pixel-diff engine
(`VisualImageSnapshot`/`compare`, release-gated). If no crate will consume the rest, retire the crate
to its pixel-diff core.

## DEC-17 — Renderer correctness & performance (render / render-skia)

**Bug.** `docs/reviews/render/code-review.md` #3, `docs/reviews/render-skia/code-review.md` #2/#3.
(a) **Compiled-vector rendering forces every path opaque white** and drops all usvg fill/stroke/
gradient color — registered SVG vectors render wrong (`render-skia` #2). (b) `render_text_layout`
**re-shapes already-shaped text** via system-font lookup at a reconstructed size (`baseline/0.8`),
losing bidi/cluster fidelity (`render-skia` #3; ties DEC-07). (c) The scene graph is **O(N²) on the
per-frame path** (linear `index_of`/`node` lookups inside the paint-order sort comparator) — a real
per-frame cost on large trees (`render` #3). All three are correctness/performance bugs, not forks.

## DEC-18 — Untrusted-payload recursion / panic hardening

**Bug.** `docs/reviews/runtime/code-review.md`, `docs/reviews/conformance/code-review.md`.
(a) **Unbounded recursion over a `Deserialize`-able untrusted scene payload** (`children` self-ref) in
runtime → native stack overflow / DoS on hostile input, the same class the script crate already bounds
(`DEFAULT_MAX_NESTING_DEPTH`) and a11y bounds (`A11Y_MAX_TREE_DEPTH`). Add the equivalent depth bound.
(b) The conformance **anti-panic gate misses `unreachable!`/`assert!`/index/overflow/recursion**, so it
does not actually catch the a11y stack-overflow it claims to. Complete the gate. **Cross-ref:** a11y's
own `Deserialize`-driven recursion instance is in Codex's territory (see Scope-out); this DEC covers
the runtime + conformance instances (mine).

## DEC-19 — Conformance crate: false-security fixtures

**Remove-or-wire.** `docs/reviews/conformance/code-review.md`. Security fixtures are asserted only as
*mentioned in docs* — never validated or denied — so the conformance "security" coverage is
false-assurance (sibling to smoke DEC-14 and testkit DEC-15). **Action:** either drive the fixtures
through the real validation path or remove the security-coverage claim. (If DEC-14 wires smoke's real
validation, this can share it.)

---

# Tier 2 detail

## DEC-01 — Framework source-scan fidelity (react / solid / svelte / vue)

**Fork.** REM-AUTH-001; `docs/reviews/framework-{react,solid,svelte,vue}/code-review.md`.

Each adapter has a raw-source `render()`/`compile()` substring scanner and a `from_native_program(...)`
boundary. The scanners **fabricate records**: hardcoded handler identifiers
(`onPointerDown`→`handlePress` regardless of the authored handler — a wrong runtime binding), Solid's
canned `fine_grained_updates`, Svelte's non-Svelte `use:ref` convention + no reactivity surface, Vue's
canned `renderer_operations` on one path and raw-key leak on the other. No real per-framework compiler
exists anywhere in the repo. The four crates are ~400 LoC of near-verbatim copy-paste that has already
silently diverged.

**Load-bearing fact:** the raw-source path has **no build-pipeline/CLI caller** — every call site is
in `framework-conformance` (which uses `from_native_program`) or each crate's own tests. Production
fidelity already flows through `from_native_program`.

- **(A, recommended)** Make `from_native_program` the sole honest path; strip the heuristic
  fabrications + their false-confidence tests; reduce the scanners to honestly-derivable records or
  remove them; extract a shared `hawk2ui-framework-core` to kill the copy-paste. Matches reality
  (not-wired); removes the false-assurance; collapses duplication.
- **(B)** Build real TSX/Svelte-5/Vue-SFC/Solid compilers + reactivity models (+ a
  `FrameworkNativeProgram` reactivity extension). Large; polishes an unused path.

Folded-in Lows (resolve with the chosen branch): magic visual defaults (`#080a0e`/`#ffffff`/160×32/
font 18) baked into each adapter instead of style/tokens; the substring asset-path denylist (prefer a
workspace-relative allowlist). The clean lifecycle-handler gating (a separate false-assurance fix) has
already landed for React (`e9d37042`) and Solid (`17041488`) and is correct under either branch;
Svelte/Vue gating lands with the chosen branch.

**Decision:** _Pending._

## DEC-02 — Effects pipeline end-to-end (gradient / shadow / glow)

**Fork. RE-VERIFIED: real gap.** REM-RENDER-003/004; `docs/reviews/render/code-review.md` #2,
`docs/reviews/render-skia/code-review.md` #1/#5. `apply_layer_effect` has **4 callers, all tests** —
**zero production callers**; the wired runtime scene path (`RuntimeVisual`/`RuntimeDrawCommand`) cannot
express shadow/glow/gradient/rounded-rect, and the Skia effect grammar (`shadow-rect`/`glow-rect`)
does not match what `export_paint_commands` would emit (`draw-shadow`/…) — gradient/rounded-rect are
absent entirely. So REM-RENDER-004's "shadow/glow through Skia" is a working *method* that production
never reaches. **Options:** wire the smallest complete vertical (style property → runtime visual →
render record → Skia paint) for one gradient + one shadow and expand by spec (ties DEC-11), or
down-scope the records and remove the dead grammar. **Recommendation:** wire one vertical.

**Decision:** _Pending._

## DEC-03 — Performance measurement infrastructure

**Fork.** REM-CRATE-006; `docs/reviews/perf/code-review.md`. The comparator works but **no measurement
source exists**: every observed value is a hardcoded literal, the stability gate and RT-allocation
budget are self-reported constants, `iterations` is unused — so the budget gate **can never catch a
regression** (contradicts `docs/development/performance.md`). Deterministically measurable now:
artifact/package **size** (bytes), plugin audio-thread **allocation** (counting allocator). Judgment
call (wall-clock/RSS): cold-start, frame-render, js-evaluate, memory working-set. **Recommendation:**
gate the deterministic subset; keep wall-clock advisory until a flakiness policy is set. Fold the **RT
audio-thread deadline / process-time monitoring** (REM-CRATE-006) here — the timing probe must be
RT-safe (lock-free clock read). **Hold the `performance.md` correction until the gating subset is
chosen.**

**Decision:** _Pending._

## DEC-04 — Plugin parameter model fidelity (Choice / Bool)

**Fork.** `docs/reviews/plugin/code-review.md`, `docs/reviews/host-baseview/code-review.md` #3.
`Choice` is half-wired (no `choice()` constructor, no `StateValue::Choice`, no label model);
`Choice`/`Bool` normalize/denormalize do not round-trip; the baseview adapter coerces Choice→Float.
Making `Choice` round-trip touches the **parameter-type model** and **state-format compatibility**
(persisted state/presets, baseview adapter) — a schema/compat decision. **Recommendation:** define the
model (does `StateValue` gain `Choice`? how are labels + automation-normalized values carried?),
version the state format, then wire. Sequence after DEC-05b.

**Decision:** _Pending._

## DEC-05b — Plugin format scope (VST3 / AU / standalone)

**Fork.** REM-CRATE-006; `docs/reviews/plugin-adapters/code-review.md`. VST3 `GetPluginFactory`
returns `null` (no `IPluginFactory`/COM layer); AU/standalone/desktop emit a plain-text descriptor in
the executable slot (no binary/entry point/codegen). Only CLAP is functional, yet docs advertise
"Production" for all (correct under REM-GDOC-003). VST3 is a committed first-class format (market /
licensing / Linux), so it is *in* — but the COM implementation is real work, and the licensing model
("via-truce" permissive + commercial rider) applies. **Recommendation:** build VST3 `IPluginFactory`/
COM first; keep AU/standalone tracked; correct the crate docs now to match what is functional. (The
CLAP bug + inert guard are DEC-05a, fix-now.)

**Decision:** _Pending._

## DEC-07b — Text font-pipeline scope

**Fork.** `docs/reviews/text/code-review.md`; REM-RENDER-005. Beyond the DEC-07a bugs: `fontdb::Database`
is loaded but never queried; app-font bytes never reach the shaper; there is no glyph cache behind
`GlyphCacheKey` (keys produced, nothing rasterizes/caches/evicts; the key omits
`line_break`/`truncation` → stale-cache risk). How much real font pipeline ships now vs a later
milestone is a scope decision (interacts with asset font compilation + renderer). **Recommendation:**
scope as a font-pipeline milestone after DEC-07a lands.

**Decision:** _Pending._

## DEC-09 — `security-model` enforcement wiring

**Fork.** `docs/reviews/security-model/code-review.md` #4. The trust-layer **logic** is fixed (#1 stale
registry, #2 redactor, #3 forgeable `Verified` all remediated in `29600c6`), but **nothing wires it to
enforcement**: no crate consumes `PackageTrustValidator`/`RuntimeAuthorityPolicy`/the registries.
**Recommendation:** wire trust validation into the build/seal/verify path (likely `hawk2ui-build`
artifact verification) + a test proving a forged/invalid trust input is rejected there; decide whether
`RuntimeAuthorityPolicy` gates the script runtime. Architecture decision (which boundary owns
enforcement).

**Decision:** _Pending._

## DEC-11 — Style accepted-subset enforcement

**Fork. RE-VERIFIED: partially closed.** REM-STYLE-001; `docs/reviews/style/code-review.md`. A
`PropertySpec`/`ValueType` system now exists (`property.rs`: `overflow` is `ValueType::Keyword`,
default `visible`). Residual gap: does it **enumerate accepted keyword values** (rejecting
`display:flexx`), or does `ValueType::Keyword` still accept any identifier? And `box-shadow`/`transform`
remain render-critical properties with no grammar. Defining the accepted keyword set + effect grammar
is the subset decision (REM-STYLE-001; ties DEC-02 for shadow). **Recommendation:** enumerate keyword
values with structured rejection; define box-shadow/transform grammar (or mark passthrough-with-
validation).

**Decision:** _Pending._

## DEC-12 — Host surface trait unification (`HostSurface` / `FramePresenter`)

**Fork (low urgency).** `docs/reviews/host/code-review.md` #1. The unifying traits — the crate's stated
reason to exist — have **no production implementor** (only in-crate `Recording*` doubles);
`HostSurface::teardown(impl Into<String>)` is non-`dyn`-compatible; the real adapters implement the
specific `DesktopHostAdapter`/`PluginHostAdapter` traits. **Options:** (A) route the real adapters
through `HostSurface` (drop `impl Into<String>` so it stays object-safe); (B) demote to an explicitly
test-only conformance contract. Nothing breaks today. **Recommendation:** schedule with the next
host-adapter change.

**Decision:** _Pending._

## DEC-13 — Platform backends beyond the policy/record layer

**Fork.** REM-GDOC-003; `docs/reviews/platform/code-review.md`. `hawk2ui-platform` is a policy/record
layer (capability-scoped fs/net/clipboard/secrets/db), not a syscall layer; the secret-store,
capability-context, and real backends that perform IO are not owned here (filesystem TOCTOU +
grant-as-capability are documented host obligations after `ea06b4e2`). **Recommendation:** decide
backend ownership (platform vs host adapters vs new backend crate); at minimum specify the secret-store
backend (OS keychain) + the capability-context that threads manifest grants → runtime.

**Decision:** _Pending._

## DEC-14 — Smoke-test realness

**Fork.** `docs/reviews/smoke/code-review.md`. The dep set omits `build`/`style`/`script`/`host-winit`,
so the security, desktop, and "visual" smokes are **answer-key string-matches** (read canned
artifacts, return hardcoded structs; no build/layout/style/render runs; the only real Skia raster is
the plugin `baseview_visible_pixel` path). Realtime non-blocking asserts are vacuous (inert counters).
Making them real requires adding the omitted deps (a dep-layering decision) + restructuring fixtures.
**Recommendation:** add the deps and drive the real paths; rename tests to match what they prove;
replace inert counters with the structural `!needs_drop` proof already used in `plugin`. (The
dashboard/style-gallery `layout_nodes`/`style_rules` sub-parts are contained once restructured.)

**Decision:** _Pending._

## DEC-16 — Release-evidence: validate docs vs reality

**Fork (low priority).** `docs/reviews/xtask/code-review.md` #2. `release-check` validates document
*structure* (changelog sections, version anchoring — both hardened in `45456028`) but not that doc
*claims* match source (a crate `//!` advertising "Production" for an unimplemented format —
REM-GDOC-003 / DEC-05b). **Recommendation:** decide whether release-check asserts a maintained registry
of "production-ready" crates so maturity cannot be silently overstated.

**Decision:** _Pending._

---

## Scope-out — Codex's active territory (not duplicated here)

`hawk2ui-{a11y, api, assets, authoring}` are under Codex's concurrent review refresh (the
`M docs/reviews/{a11y,api,assets,authoring}` working-tree changes). Their open items are tracked in
Codex's stream, not this register, to avoid two owners. Two cross-cutting flags so they are not lost:

- **a11y** carries the originating `Deserialize`-driven recursion-DoS (the foil referenced across the
  framework/runtime reviews). The runtime + conformance instances are **DEC-18** (mine); a11y's own
  instance is Codex's to close, but the depth-bound pattern should be consistent
  (`A11Y_MAX_TREE_DEPTH` / script's `DEFAULT_MAX_NESTING_DEPTH`).
- **authoring** `parse_hex_color` does a byte-length check then byte-index slicing → a 6-byte string
  with a multibyte char (`"#€abc"`) **panics on a char boundary** instead of returning
  `native-runtime.color.invalid` — reachable on the real `bridge_element`/`bridge_artifact` render
  path; the only panic in a `forbid(unsafe_code)` crate (`authoring/runtime_bridge.rs:645`). Surfaced
  for Codex; trivially fixable (char-boundary-safe slicing).

## Already resolved as contained (not decisions — landed this sweep)

False-assurance / latent-bug findings fixed in place, listed so this register is not mistaken for the
full finding set:

- Framework `lifecycle_handlers` source-path gating: react `e9d37042`, solid `17041488` (svelte/vue
  land with DEC-01's branch) — a public accessor that disagreed with the gated `events`.
- React `react_public_operation_key` raw-key leak for non-`"root"` ids (`e9d37042`).
- Prior takeover: plugin RT-safety heap-free packets (`de431c3f`), host/host-baseview/host-winit
  dead-state + tautology removals, platform fail-closed scope root (`ea06b4e2`), xtask version-anchor
  + changelog-section gates (`45456028`), render dead fake-measurer removal (`7f5db8d3`), render-skia
  surface-dimension cap (`b82f1685`), security-model trust-logic fixes (`29600c6`).
- **DEC-10 verified closed at HEAD:** script `ScriptExecutionLimits` (loop/nesting/byte bounds) already
  remediates the `u64::MAX`/unguarded-parser DoS finding.

See each crate's `docs/reviews/<crate>/code-review.md` Remediation section for detail.

# Task List 0010: Parameter ID Stability (pre-C3 brick)

## Purpose

Straighten the load-bearing brick under the editor parameter seam: make a parameter's truce `ParamId` u32 **stable across releases**, owned by the author, instead of a side effect of manifest declaration order. This is the prerequisite for C3 (`tasks/0009`) and a correction that restores Decision 0002 §17's "stable string IDs" promise at the layer where it actually breaks.

This is a **separate list on purpose**: it is a codegen/manifest change in `hawk2ui-build` / `hawk2ui-plugin` / `hawk2ui-cli` with a different blast radius from the C3 editor work, and burying it inside the C3 contract is how it would get skipped.

## The bug

`emit_truce_params_struct` mints param ids `0,1,2,…` over `model.parameters` in declaration order (`crates/hawk2ui-build/src/param_codegen.rs:60-67`) and emits them as explicit `#[param(id = N)]`, which truce honors verbatim as the `ParamId` discriminant (`reference/truce-0.49.14/crates/truce-derive/src/lib.rs:939-953`, `:1418-1465`). The u32 is therefore **positional**, while the author's stable handle is the *string* id. Insert or reorder one `[[parameter]]` and every subsequent u32 shifts, silently re-pointing every saved automation lane, preset, and state blob in every user's DAW project. truce's contract is "id stable across releases, never change it," and Hawk2UI cannot honor it on the author's behalf today because the author has no handle on the u32.

## Decision

Pin parameter ids **explicitly** (Decision 0003, "Prerequisite gate"):

- Rejected: append-only manifest validation — invisible discipline that detonates the day someone alphabetizes the manifest without knowing they re-pointed every saved project.
- Chosen: an explicit author-owned numeric id per parameter that flows into `#[param(id = N)]` (truce's own nested-struct escape hatch), softened by auto-assign-and-write-back on first build so ids pin from then on.
- Scope: params only (persistence-critical); meters are ephemeral, no saved-state dependency.

## Sources

- Decision (local ADR — `docs/decisions/` is gitignored, so this lives on disk and in memory, not in the repo, like 0001/0002): `docs/decisions/0003-c3-editor-param-projection-protocol.md` (Lock 1 + Prerequisite gate).
- Decision: `docs/decisions/0002-stable-architecture-baseline.md` §17.
- Spec: `specs/0009-plugin.md`.

## Tasks

### 0010.1 Explicit, Stable Parameter IDs

- [ ] Pre-edit: run `gitnexus_impact({target: "ParameterRecord"})` and `…("parameter_model")` before editing — `ParameterModel`/`ParameterRecord` are widely consumed; report blast radius and warn on HIGH/CRITICAL (per `CLAUDE.md`).
- [ ] Deliverable: optional explicit numeric `id` per `[[parameter]]` in `manifest.hawk.toml`, carried through `ParameterRecord`/`ParameterModel` and emitted verbatim as `#[param(id = N)]` (replacing the positional `next_id` loop); manifest validation that ids are unique and `< 2²⁴` (`METER_ID_BASE`); a reorder-stability test that permutes manifest parameter order and asserts every emitted u32 is unchanged.
- [ ] Dependencies: none (this is the first brick).
- [ ] Verify: `cargo test -p hawk2ui-build param_codegen` · `cargo test -p hawk2ui-plugin parameter_model` · new reorder-stability test green.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

### 0010.2 Auto-Assign and Write-Back on First Build

- [ ] Deliverable: on build, any parameter lacking an explicit `id` is assigned the next free id and the id is **written back into `manifest.hawk.toml`** (preserving formatting/comments as far as the TOML writer allows), so subsequent builds are pinned; a diagnostic notes which ids were assigned; idempotent on re-run (no churn once pinned).
- [ ] Dependencies: `0010.1`.
- [ ] Verify: `cargo test -p hawk2ui-build` (write-back assigns gaps, is idempotent, never renumbers an already-pinned id) · `cargo test -p hawk2ui-cli` build path round-trip.
- [ ] Review check: As you are delivering this product yourself, are you satisfied with the implementation or should there be revisions to ensure production ready stability? If revision is needed, take corrective action before continuing so the task meets the standard of production ready stability.

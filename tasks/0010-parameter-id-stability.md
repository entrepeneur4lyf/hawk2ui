# Task List 0010: Parameter ID Stability (pre-C3 brick)

## Purpose

Straighten the load-bearing brick under the editor parameter seam: make a parameter's truce `ParamId` u32 **stable across releases**, owned by the author, instead of a side effect of manifest declaration order. This is the prerequisite for C3 (`tasks/0009`) and a correction that restores Decision 0002 §17's "stable string IDs" promise at the layer where it actually breaks.

This is a **separate list on purpose**: it is a codegen/manifest change in `hawk2ui-build` / `hawk2ui-plugin` / `hawk2ui-cli` with a different blast radius from the C3 editor work, and burying it inside the C3 contract is how it would get skipped.

## The bug

`emit_truce_params_struct` mints param ids `0,1,2,…` over `model.parameters` in declaration order (`crates/hawk2ui-build/src/param_codegen.rs:60-67`) and emits them as explicit `#[param(id = N)]`, which truce honors verbatim as the `ParamId` discriminant (`reference/truce-0.49.14/crates/truce-derive/src/lib.rs:939-953`, `:1418-1465`). The u32 is therefore **positional**, while the author's stable handle is the *string* id. Insert or reorder one `[[parameter]]` and every subsequent u32 shifts, silently re-pointing every saved automation lane, preset, and state blob in every user's DAW project. truce's contract is "id stable across releases, never change it," and Hawk2UI cannot honor it on the author's behalf today because the author has no handle on the u32.

## Decision

Pin parameter ids **explicitly** (Decision 0003, "Prerequisite gate"):

- Rejected: append-only manifest validation — invisible discipline that detonates the day someone alphabetizes the manifest without knowing they re-pointed every saved project.
- Chosen: an explicit author-owned numeric id per parameter that flows into `#[param(id = N)]` (truce's own nested-struct escape hatch), softened by an explicit `hawk2ui pin-ids` command (opt-in, format-preserving write-back) plus a `validate`-time warning on unpinned ids — **no silent build-time mutation** of the author's manifest (cleaner for CI/git), and the `new` scaffold ships pinned ids.
- Scope: params only (persistence-critical); meters are ephemeral, no saved-state dependency.

## Sources

- Decision (local ADR — `docs/decisions/` is gitignored, so this lives on disk and in memory, not in the repo, like 0001/0002): `docs/decisions/0003-c3-editor-param-projection-protocol.md` (Lock 1 + Prerequisite gate).
- Decision: `docs/decisions/0002-stable-architecture-baseline.md` §17.
- Spec: `specs/0009-plugin.md`.

## Tasks

### 0010.1 Explicit, Stable Parameter IDs — ✅ DONE (`ce4bc580`)

- [x] Pre-edit: ran `gitnexus_impact` on `ParameterRecord`/`parameter_model` — gitnexus was stale (mis-resolved + line drift); fell back to tilth (production consumers: CLI `package_plugin`/`export_params`; rest tests). Risk LOW–MEDIUM, additive.
- [x] Deliverable: optional explicit numeric `param_id` per `[[parameter]]` in `manifest.hawk.toml`, carried through `ParameterRecord`/`ParameterModel` and emitted verbatim as `#[param(id = N)]` (the positional `next_id` loop is replaced by `ParameterModel::resolved_param_ids`); manifest validation rejects ids that are duplicate or `>= 2²⁴` (`METER_ID_BASE`); a reorder-stability test permutes parameter order and asserts every emitted u32 is unchanged.
- [x] Dependencies: none (this is the first brick).
- [x] Verify: `cargo test -p hawk2ui-build param_codegen` · `cargo test -p hawk2ui-plugin parameter_model` · reorder-stability test green.
- [x] Review check: satisfied — backward-compatible (un-pinned manifests emit byte-identical source); gated `check-fast` + clippy pedantic clean.

### 0010.2 Explicit `pin-ids` Command + `validate` Warning — ✅ DONE

Revised from "auto-write-back on build" (owner call, 2026-05-30): a build silently mutating a source file dirties git trees and surprises CI, so write-back is an **opt-in command**, not a build side effect.

- [x] Deliverable: `hawk2ui pin-ids` reads the manifest, assigns the resolved id to every unpinned `[[parameter]]`, and writes it back **preserving comments/formatting** (`toml_edit`); idempotent (no-op once all pinned); prints the assignments. `validate` emits a **non-fatal warning** naming any unpinned parameter (suggesting `hawk2ui pin-ids`), exit code unchanged. The `new` scaffold ships pinned ids so a fresh project is warning-free.
- [x] Dependencies: `0010.1`.
- [x] Verify: `cargo test -p hawk2ui-build pin_param_ids` (assign + comment-preservation + idempotency) · `cargo test -p hawk2ui-cli` (pin-ids command round-trip + idempotency; validate warning names only unpinned params).
- [x] Review check: satisfied — explicit command + warning is cleaner than silent build mutation; gated `check-fast` + clippy pedantic + `cargo deny` green.

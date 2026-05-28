# Code Review — hawk2ui-api

**Reviewed at:** `5a042c51` · 2026-05-28 · working tree clean for this crate at review time
**Scope:** `src/{lib,diagnostic,artifact,plugin,runtime,surface,inventory}.rs` + `tests/{api_inventory,api_stability_docs,artifact_contract,diagnostic_contract,plugin_contract,surface_runtime_contracts}.rs` (8 src files ~1.8k LoC, 6 test files ~430 LoC). No `build.rs`/`benches/`.
**Purpose:** The foundational crate. Defines the public API contracts shared across all `Hawk2UI` crates — `Diagnostic`/`SourceSpan`, versioned artifact manifest records, plugin parameter/state/editor/realtime contracts, runtime lifecycle/job/binding records, host surface metrics/input/repaint records, and a self-describing API inventory used by stability tests.
**Note:** Codex is concurrently committing hardening/remediation work; HEAD drifts. Findings reflect the SHA above and may overlap with in-progress fixes.

## Summary

This crate is **almost entirely real, not a stub**: every public contract type is a concrete record with working constructors, builder methods, and pure accessors, and the non-trivial logic that exists is correct — `ArtifactSchemaVersion::ensure_can_read` performs a real compatibility check, `Diagnostic::to_cli_string` formats a real deterministic line, `PluginStateContract::with_entry` does real dedupe-and-sort, `KeyModifiers` is a real bitflag implementation, and `InputEvent::{surface_position,requires_focus}` are real exhaustive matches. Serde round-trips are genuinely exercised. The one substantive defect is in `inventory.rs`: the production API inventory lists **three types that do not exist anywhere in the repository** (`ExperimentalScriptEngineContract`, `ArtifactBuilderInternals`, `SurfaceCompileFixture`), and these phantom entries are the *only* `FeatureGated`/`Internal`/`TestOnly` entries — so two inventory tests pass **solely** because of fabricated data (false confidence). Secondary gaps: `CompiledAssetRecord` can only construct `Image` (the `Vector`/`Font` variants are unreachable via the public builder), and `Diagnostic::redacted()` is a self-asserted flag that scrubs nothing. **Unlike peer crates (e.g. hawk2ui-a11y), this crate has no recursion, no `unsafe`, and no panics on fallible paths — there is no High-severity DoS or memory-safety surface here.** Honest severity ceiling: **Medium**.

## Completeness & implementation gaps

1. **Phantom inventory entries (records intent that does not exist)** (`inventory.rs::ApiInventory::production_baseline`). Three entries name types that are defined and exported **nowhere** — confirmed by a repo-wide search, they appear only as string literals here:
   - `ExperimentalScriptEngineContract` (`FeatureGated`, Runtime) — line 195
   - `ArtifactBuilderInternals` (`Internal`, Build) — line 199
   - `SurfaceCompileFixture` (`TestOnly`, Test) — line 200
   The inventory is a `&'static str` table, so nothing forces these names to correspond to real items. This is the crate's "record-only" pattern: the inventory asserts an API shape that the crate does not actually have. **The sharper problem is that all three non-`Public` classifications are fabricated** (see Testing #1). → Either add the real types, or remove the entries and adjust the tests to not require a `FeatureGated`/`TestOnly` member that doesn't exist.
   *Balancing note (anti-false-positive): the other ~49 entries are all `Public` and map 1:1 to genuinely exported types — that part of the inventory is real and `api_contract_inventory_includes_all_*` is a real coverage check. Only the three non-`Public` rows are fabricated.*

2. **`CompiledAssetRecord` can only ever be an `Image`** (`artifact.rs::CompiledAssetRecord`). The sole public constructor is `image()`, which hardcodes `kind: CompiledAssetKind::Image` and always sets `width`/`height` to `Some`. There is no `vector(...)`/`font(...)` constructor. Consequently:
   - `CompiledAssetKind::{Vector, Font}` are unreachable through the builder API (only via `Deserialize`), yet the inventory advertises `CompiledAssetKind` as a public contract and the framework targets vector/font assets.
   - The `None => "unbounded"` branch of `stable_key` (`width.zip(height)` failing) is dead via the public API — `image()` never produces `None` dimensions.
   → Add `vector(...)`/`font(...)` constructors (font/vector assets legitimately have no pixel dimensions, which is exactly what the `"unbounded"` branch is for).

3. **`Diagnostic::redacted()` redacts nothing** (`diagnostic.rs::Diagnostic::redacted`). It flips `self.redacted = true` and returns `self`; `message`, `related`, and `source` are left verbatim. The flag is self-asserted by the caller and the crate enforces no scrubbing. Within this crate that is record-only behavior (see Security). (The identically-named `Secret::redacted()` in hawk2ui-security/-platform is an unrelated type that *does* return a masked string — not this method.)

No `todo!()`/`unimplemented!()`/`unreachable!()`/placeholder `panic!()` and no TODO/FIXME/HACK/XXX anywhere in the crate.

## Code quality & smells

- **`stable_key` delimiter collides with hash contents** (`artifact.rs::CompiledAssetRecord::stable_key`). The key is `"{kind}:{id}:{hash}:{dimensions}"` joined on `:`, but `ArtifactHash` values are themselves colon-bearing (`"sha256:hero"` in the tests). The resulting key (`image:hero:sha256:hero:1024x512`) is unambiguous for equality/hashing but is **not field-parseable** and would alias if an id also contained `:`. Fine as an opaque dedupe key; misleading if anything ever splits it. → Use a non-colliding separator or length-prefix the fields.
- **`ensure_can_read` has no cross-major back-compat and the reverse-major path is asymmetric** (`artifact.rs::ArtifactSchemaVersion::ensure_can_read`). `self.major != artifact.major` rejects in *both* directions, so a newer runtime cannot read an older-major artifact. The doc matches the code, so this is a deliberate semantic, not a bug — but it is a strong policy worth calling out for the foundational compatibility primitive, and the runtime-newer-major case is untested (see Testing).
- **No `f32` validation on numeric contracts** (`plugin.rs`, `surface.rs`). `PluginParameterContract::{with_normalized_range,accepts_normalized}`, `default_normalized`, and `SurfaceMetrics::new` accept `NaN`/`Inf`/`min > max` silently. `accepts_normalized(NaN)` is `false`, `with_normalized_range(1.0, 0.0)` yields an empty-accepting range, and a `NaN` scale factor flows straight through. As public contracts these are the canonical place to reject non-finite input; none do.
- **Builder fragility in `PluginParameterContract`** (`plugin.rs`). `with_normalized_range` sets the bounds but `new` already defaulted `default_normalized` with no clamp/validation against those bounds, so `accepts_normalized(self.default_normalized)` can be `false` for a freshly-built parameter. Minor consistency gap.
- **`SurfaceMetrics` derives `PartialEq` (not `Eq`)** because of the `f32` fields — correct, but means metrics-bearing records (`HostSurfaceContract`, `InputEvent::Resized`) are also `PartialEq`-only. Expected, noted for completeness; no action.

## Documentation

- Strong overall: crate-level `//!`, a `## Stability` section in every public module (`api_stability_docs.rs` enforces this), and every public type/field/fn carries a doc comment. No undocumented public items found.
- **Copy-paste doc bug** (`surface.rs::KeyModifiers::with_shift`): doc reads *"Creates keyboard modifier state."* while every sibling (`with_control`/`with_alt`/`with_meta`) correctly reads *"Adds the X modifier."* `with_shift` adds Shift to an existing value; the doc is wrong.
- A handful of public fields are documented but their *invariants* are not: `PluginParameterContract::{normalized_min,normalized_max}` don't state "expected finite, min ≤ max"; `SurfaceMetrics::scale_factor` doesn't state "expected > 0". Given there is no validation (above), the docs are the only contract and they're silent on it.
- `# Errors` coverage is correct: the only fallible public fn is `ensure_can_read`, and it has a proper `# Errors` section.

## Testing

- Good breadth across all six modules: diagnostic serialize + CLI-string snapshot + deserialize round-trip; artifact builder + version gate + serialize; plugin parameter/state/preset/realtime + exact JSON snapshot; surface/runtime events, bitflag modifiers, jobs, bindings + exact JSON snapshots. Serde is genuinely round-tripped, not bound-checked.
1. **Two inventory tests pass only because of phantom data** (`api_inventory.rs::api_inventory_classifies_public_internal_feature_gated_and_test_only_types`). The test asserts `types().any(status == FeatureGated)` and `types().any(status == TestOnly)`. The *only* `FeatureGated` entry is `ExperimentalScriptEngineContract` and the *only* `TestOnly` entry is `SurfaceCompileFixture` — both fabricated (Completeness #1). So these assertions are satisfied **exclusively** by data that corresponds to no real API. This is false confidence: the test claims the inventory classifies feature-gated/test-only types, but there are no real such types to classify.
2. **`stable_key`'s `"unbounded"` branch is untested and unreachable via the builder** (Completeness #2) — `artifact_contract.rs` only ever builds an `Image` with finite dimensions. The `Vector`/`Font` `kind_key` arms are likewise never exercised.
3. **Reverse-major version check untested** — `artifact_contract.rs::artifact_contract_allows_older_minor_and_rejects_newer_minor_or_major` tests artifact-newer (`2.0.0` rejected) but never runtime-newer-major (e.g. runtime `2.x` reading artifact `1.x`), which `ensure_can_read` also rejects.
4. **No tests for `f32` edge cases** (`NaN`/`Inf`/`min > max`) on parameter ranges or surface metrics — consistent with the absence of validation, so the gap is invisible.
5. Minor: `redacted()` is exercised (`diagnostic_contract.rs`) only to assert the bool serializes as `true` — there is (correctly) no assertion that anything is scrubbed, but that also means nothing documents the no-op semantics for downstream readers.

## Cross-cutting conventions (workspace-wide; expected deviations for the foundational crate)

- This crate **defines** `Diagnostic`/`RuleId`/`SourceSpan`; it therefore (correctly) contains **no `From<X> for Diagnostic`** impls — those live in the domain crates that convert *into* this contract. Not a defect.
- This crate has **no `crate_name()` and no `*_workspace_filter_marker` test** — the marker convention seen elsewhere is absent here. As the contracts crate it has no domain-error surface to filter, so the absence is expected; noted only because the convention is workspace-wide.
- `#![forbid(unsafe_code)]` is present at the crate root (`lib.rs:1`) and the crate is `unsafe`-free. Consistent with the workspace lint (`unsafe_code = "forbid"`).

## Security

This is a pure data-contract crate with a very small attack surface, and it is clean on the dimensions that bite peer crates:

- **No `unsafe`** anywhere; `#![forbid(unsafe_code)]` holds.
- **No recursion.** None of the contract types are self-referential (no nested-tree shape), so there is **no stack-overflow DoS vector** — in explicit contrast to recursion-bearing peer crates.
- **No panics on fallible paths.** The only `expect(...)` in the crate is inside a `#[cfg(test)]` test in `lib.rs`; production code paths do not `unwrap`/`expect`/`panic`.
- **`Diagnostic::redacted()` is advisory, not a redaction boundary** (`diagnostic.rs`). `redacted == true` is set by the caller and the crate scrubs nothing — `message`/`related`/`source` are emitted verbatim by `to_cli_string` and serde. Downstream code (and operators reading logs) must **not** treat the flag as a guarantee that secrets were removed. Low severity, but worth an explicit doc warning since this is the shared diagnostic emitted by every crate.
- **Unbounded `Vec`/`String` on `Deserialize`-able records** (`Diagnostic.{fixes,related}`, `ArtifactManifestSnapshot.{capabilities,assets,styles,scripts,targets}`, `PluginStateContract.entries`, `PluginParameterContract.name`, ids/hashes). All derive `Deserialize` with no length caps, so a hostile/oversized payload (a deserialized artifact manifest or plugin state from an untrusted package) is a memory-pressure vector. There is no nesting, so it is allocation-pressure only, not stack overflow — Low. Bound sizes at the deserialization boundary in the consuming crates if these records are ever read from untrusted input.
- **No non-finite `f32` rejection** (see Code quality). A `NaN` scale factor or normalized value deserialized from an untrusted contract propagates unchecked into layout/automation math downstream — the contract layer is the right place to reject it.
- No crypto, no signature/secret handling (hashes here are opaque strings, not verified), and no filesystem/path handling — no path-traversal surface in this crate.

### Severity-ranked findings

| # | Severity | Finding | Location |
|---|----------|---------|----------|
| 1 | Medium | Inventory lists 3 phantom types; they are the *only* non-`Public` entries, so two inventory tests pass solely on fabricated data (false confidence) | `inventory.rs::production_baseline`; `tests/api_inventory.rs` |
| 2 | Low | `CompiledAssetRecord` has only an `image()` ctor — `Vector`/`Font` variants + the `"unbounded"` dimension branch unreachable via the public API and untested | `artifact.rs::CompiledAssetRecord` |
| 3 | Low | `Diagnostic::redacted()` sets a self-asserted flag but scrubs nothing — must not be trusted as a redaction boundary | `diagnostic.rs::Diagnostic::redacted` |
| 4 | Low | No `f32` validation (NaN/Inf/min>max) on parameter ranges or surface metrics — public contracts accept invalid numerics silently | `plugin.rs`, `surface.rs` |
| 5 | Low | Unbounded `Vec`/`String` on `Deserialize`-able records → memory-pressure DoS from untrusted payloads (no nesting → no stack overflow) | `diagnostic.rs`, `artifact.rs`, `plugin.rs` |
| 6 | Low | `stable_key` joins on `:` while `ArtifactHash` contains `:` → non-parseable / potentially aliasing key | `artifact.rs::stable_key` |
| 7 | Low | `ensure_can_read` rejects runtime-newer-major too; reverse-major path untested | `artifact.rs::ensure_can_read`; `tests/artifact_contract.rs` |
| 8 | Low | `KeyModifiers::with_shift` doc is a copy-paste of `empty()` ("Creates…" instead of "Adds Shift") | `surface.rs::KeyModifiers::with_shift` |

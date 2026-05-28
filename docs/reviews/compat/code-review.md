# Code Review — hawk2ui-compat

**Reviewed at:** `5a042c51` · 2026-05-28
**Scope:** `src/{lib,matrix}.rs` + `tests/matrix_validation.rs` + `Cargo.toml` (3 src/test files, ~455 src LoC / ~169 test LoC) and the four checked-in data files in `compatibility/*.toml`.
**Purpose:** Machine-readable compatibility matrices (OS targets, graphics backends, plugin hosts, package outputs) and unsupported-target diagnostics for Hawk2UI.
**Note:** Codex is concurrently committing hardening/remediation work; HEAD drifts. Findings reflect the SHA above and may overlap with in-progress fixes.

## Summary

This crate is **genuinely implemented, not a stub**. All four matrices parse real TOML via `serde`/`toml`, build their typed row structs, deduplicate by stable key (`BTreeSet`), and validate that every string field is non-empty (`matrix.rs::*::validate`, `require_field`). The lookup accessors (`contains_target`, `host`, `package`, `supports_feature`) do real work over the parsed data. Documentation is clean and `#![forbid(unsafe_code)]` holds.

The substantive weakness is that the half of the crate named in its own purpose statement — **diagnostics** — is incomplete and partly *incorrect*: the one accept/reject method, `CompatibilityMatrix::unsupported_target_diagnostic`, keys only on the row `name` and **never consults `release` status**. A target authored as `release = "blocked"` or `release = "unsupported"` is silently treated as accepted (returns `None`), and a blocked target is even listed inside the "Supported targets:" string. Consequently `ReleaseStatus::{Blocked, Unsupported}` are authored enum variants that **no code branches on**, and the per-row `diagnostic` strings carried by the graphics matrix (and `CoverageStatus::Missing` on hosts) are validated/stored but **never emitted**. There is **no genuine High** here — input is trusted checked-in TOML, the data structures are flat `Vec`s, unknown targets fail closed, and nothing wires these accessors to a runtime gate yet (the only consumers are this crate's tests and `hawk2ui-conformance`). The mis-accept is **Medium** today and would be **High** the moment a runtime path calls `unsupported_target_diagnostic`.

## Completeness & implementation gaps

1. **`release` status is never consulted → wrong-accept of declared-unsupported targets** (`matrix.rs::CompatibilityMatrix::unsupported_target_diagnostic`, `::contains_target`). `contains_target` matches on `name` only. `unsupported_target_diagnostic` returns `None` (= no diagnostic, accepted) for *any* present row, regardless of `release`. The data model explicitly supports `release = "blocked"` / `"unsupported"` (`ReleaseStatus`), so a row authored as unsupported is silently accepted by the method whose entire job is to flag unsupported targets. This is a correctness defect, not merely an unused branch (see Security).
2. **The "Supported targets:" list includes non-supported rows** (`matrix.rs::unsupported_target_diagnostic`). The suggestion string is built from `self.targets.iter().map(|row| row.name.as_str())` with no `release` filter, so a `Blocked`/`Unsupported` row would be advertised to the user as supported.
3. **`ReleaseStatus::Blocked` and `ReleaseStatus::Unsupported` are inert variants.** They are deserialized and round-tripped but no method in the crate branches on them (verified: no `.release` / `ReleaseStatus::` match arm exists in `src/`). The only `release`-aware logic lives in a *test* (`tests/matrix_validation.rs`: supported ⇒ `ci_coverage`), not in the library.
4. **Graphics diagnostics are validated but never emitted** (`matrix.rs::GraphicsCompatibilityMatrix`). `GraphicsBackendCompatibility.diagnostic` is required non-empty by `validate`, but the only query, `supports_feature`, returns a bare `bool`. An unsupported feature yields `false` with no diagnostic surfaced — so the stored `diagnostic` string (e.g. `"backend.capability.gpu-unavailable"`) is dead data.
5. **`CoverageStatus::Missing` has no diagnostic/aggregate consumer** (`matrix.rs`). `is_covered()` exists, but there is no method that, given a `PluginHostCompatibility`, reports *which* behaviors are missing or emits a diagnostic for a `Missing` cell. Coverage is asserted entirely in tests. Compounding the rot risk: `compatibility/hosts.toml` currently declares **every** cell `covered` for all four formats (clap/vst3/au/standalone), and the only test (`tests/matrix_validation.rs::plugin_host_matrix_declares_editor_lifecycle_state_and_realtime_coverage`) asserts exactly that all cells are covered. So the host matrix is a purely aspirational declaration with no drift detection — if a "covered" claim silently becomes false in the real plugin code, nothing in this crate would surface it.
6. **Asymmetric diagnostic API across the four matrices.** Only `CompatibilityMatrix` ships a diagnostic generator (`unsupported_target_diagnostic`). Graphics, host, and package matrices offer lookup only (`supports_feature` / `host` / `package`) with no diagnostic counterpart, despite the crate purpose being "matrices **and** unsupported-target diagnostics." The diagnostic half is implemented for one of four matrices.
7. **Query API is real but not yet wired to any runtime consumer.** `unsupported_target_diagnostic`, `supports_feature`, `host`, and `package` have **zero call sites** in the workspace outside this crate's own unit tests; the matrices are otherwise consumed only as parsed-and-asserted data by `hawk2ui-conformance`. This is not a stub — the logic is real — but the crate currently functions as a *test-checked declarative spec*, not a live gate. Frame any productionization around finding #1 before these accessors are called for real.

## Code quality & smells

- **`MatrixError::DuplicateTarget` is reused for non-target rows** (`matrix.rs`: `GraphicsCompatibilityMatrix::validate`, `HostCompatibilityMatrix::validate`, `PackageCompatibilityMatrix::validate`). A duplicate backend / host format / package output is reported as `DuplicateTarget("skia-cpu-raster")` etc. The variant name is misleading for three of the four call sites; a `DuplicateKey { kind, key }` (or per-matrix variants) would be accurate.
- **`unsupported_target_diagnostic` calls `contains_target`, which re-scans `targets`, then scans again to build the list.** Minor; matrices are tiny so this is not a perf concern, but it duplicates the iteration. Fine to leave.
- **Per-matrix `parse`/`validate` are near-identical boilerplate** (four `RawX → X → validate` blocks). Acceptable given Rust's lack of cheap generics over named TOML tables, but it is copy-paste that will drift; the `DuplicateTarget` misuse above is a symptom.
- **No accessor returns owned/iterable supported sets.** Callers wanting "all supported OS targets" must reach into the public `targets: Vec<…>` field and filter on `release` themselves — which is exactly the filter the library forgot to apply internally (#1).

## Documentation

- Strong and complete: crate-level `//!` on both `lib.rs` and `matrix.rs`; every public type, enum, variant, and field is documented; all four `parse` fns and `CoverageStatus::is_covered` carry doc comments; all four fallible `parse` fns have `# Errors` sections. **No undocumented public items found.**
- Minor: the doc comment on `unsupported_target_diagnostic` ("Returns an unsupported-target diagnostic for missing targets") is accurate to the code but understates the gap — it silently does not handle present-but-`Blocked`/`Unsupported` rows. The doc reinforces the #1 mismatch rather than flagging it.

## Testing

- Reasonable breadth on the **happy path**: real-data parse for all four matrices, the supported⇒`ci_coverage` invariant, presence of expected targets/features/hosts/outputs, and an end-to-end `unsupported_target_diagnostic("plan9-desktop")` for a *missing* target.
- The one error path tested is `CompatibilityMatrix` `DuplicateTarget` (`lib.rs::tests::rejects_duplicate_target_names`).
- **Gaps (no coverage):**
  - `MatrixError::MissingRequiredField` — never triggered for any matrix.
  - `MatrixError::Parse` — malformed TOML never tested.
  - Graphics empty-`features` rejection (`GraphicsCompatibilityMatrix::validate`).
  - Duplicate backend / host format / package output dedup paths (and thus the `DuplicateTarget`-name misuse goes unnoticed).
  - `ReleaseStatus::{Blocked, Unsupported}` and `CoverageStatus::Missing` — no test exercises a non-positive row, which is precisely why the mis-accept (#1/#2) is invisible. **A test with a `release = "blocked"` row asserting `unsupported_target_diagnostic` returns a diagnostic would fail today** — there is no such test.
  - The `None` (present-target) return branch of `unsupported_target_diagnostic` is never asserted.
  - `supports_feature(false)` for an unsupported feature, and `host`/`package` miss (`None`) paths.

## Cross-cutting conventions (deviations noted per the rules)

- **No `crate_name()` function and no `*_workspace_filter_marker` test** in this crate (verified: zero matches). The workspace convention that pairs these is **absent here** — if the `domain_test_templates`/`api_contract` filter gates expect them, this crate is not registered the same way as its siblings.
- **No `hawk2ui_api` dependency and no `From<MatrixError> for hawk2ui_api::Diagnostic`** (Cargo.toml deps are only `serde` + `toml`). The crate's "diagnostic" is a plain `String` produced by `unsupported_target_diagnostic`, not the workspace `Diagnostic` type. This **deviates** from the workspace convention and makes the word "diagnostic" in the API misleading — a caller cannot route these through the standard diagnostic pipeline without re-wrapping.
- `#![forbid(unsafe_code)]` is present at the crate root and holds.

## Security

This crate has a benign runtime profile but one correctness-as-security finding that matters for the matrices' purpose.

- **Stale/mis-keyed data can wrongly ACCEPT an unsupported target — and it does so by construction, not just by data rot** (`matrix.rs::unsupported_target_diagnostic`). Because the method ignores `release`, the *only* way a present target is rejected is by being entirely absent. A target deliberately recorded as `release = "unsupported"` or `"blocked"` is accepted with no diagnostic, and is even listed as supported in the suggestion text (#1, #2). If/when this method is wired to a release gate or a runtime guard (it is not today — no callers exist outside tests), this is a direct "ship to an unsupported target" hole. **Highest-severity item in this crate, rated Medium because it is not yet on a live path; it becomes High the moment it is.** Mitigation: filter `contains_target`/the suggestion list on `release == Supported`, and emit a diagnostic for present-but-non-supported rows.
- **No `unsafe`.** `#![forbid(unsafe_code)]` enforced.
- **Input is `Deserialize`-able, but the trust boundary is favorable.** All four row types derive `Deserialize`. In practice the input is the workspace's own checked-in `compatibility/*.toml`, parsed via `include_str!`/`fs::read_to_string`. There is no recursion and no nesting (all matrices are flat `Vec<Row>`), so there is **no stack-overflow / unbounded-recursion vector** here. Allocation is bounded by input size; `toml` does the parsing. A hostile multi-megabyte TOML is a generic large-input concern, not specific to this crate.
- **Panics-as-DoS:** none in `src/` (the `panic!`/`unwrap`/`expect` calls all live in `tests/` and `lib.rs::tests`, i.e. test-failure messages, which is correct). Library code returns `Result`/`Option` throughout.
- **Validation is real but string-only and key-only.** `require_field` rejects empty/whitespace string fields; `surface`, `release`, `ci_coverage` (and the graphics/host `bool`/`CoverageStatus` cells) are non-`Option` enums/bools, so `serde` enforces their presence at parse time — that omission from `require_field` is **not** a defect. What is *not* validated is semantic consistency (e.g. a `Supported` target with `ci_coverage = false` parses fine; only a *test* catches it, not the library), and `release`/`diagnostic`/`Missing` values are never acted upon (#1, #3–#5).

### Severity-ranked findings

| # | Severity | Finding | Location |
|---|----------|---------|----------|
| 1 | Medium | `unsupported_target_diagnostic`/`contains_target` ignore `release`; declared `Blocked`/`Unsupported` targets are wrongly accepted (High if wired to a runtime gate) | `matrix.rs::CompatibilityMatrix` |
| 2 | Medium | "Supported targets:" suggestion list is built from all rows, advertising non-supported targets as supported | `matrix.rs::unsupported_target_diagnostic` |
| 3 | Low | `ReleaseStatus::{Blocked, Unsupported}` inert — deserialized but no code branches on them | `matrix.rs` |
| 4 | Low | Graphics `diagnostic` strings and `CoverageStatus::Missing` validated/stored but never emitted | `matrix.rs::GraphicsCompatibilityMatrix`, `CoverageStatus` |
| 5 | Low | Diagnostic API asymmetric: only `CompatibilityMatrix` has a diagnostic generator (purpose claims "matrices and diagnostics") | `matrix.rs` (graphics/host/package) |
| 6 | Low | `MatrixError::DuplicateTarget` reused for duplicate backends/hosts/packages → misleading error text | `matrix.rs::{Graphics,Host,Package}CompatibilityMatrix::validate` |
| 7 | Low | Error/negative-path tests missing: `Parse`, `MissingRequiredField`, non-OS dedup, `Blocked`/`Unsupported`/`Missing` rows, `None` branch of the diagnostic | `tests/matrix_validation.rs`, `lib.rs::tests` |
| 8 | Low (convention deviation) | No `hawk2ui_api` dep / no `From<MatrixError> for Diagnostic`; "diagnostic" is a plain `String`. No `crate_name()`/`*_workspace_filter_marker` present | `Cargo.toml`, `matrix.rs` |

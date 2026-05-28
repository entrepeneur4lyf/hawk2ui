# Code Review — hawk2ui-core

**Reviewed at:** `5a042c51` · 2026-05-28
**Scope:** `src/lib.rs` + `Cargo.toml` (1 source file, ~49 LoC; no `tests/` dir)
**Purpose:** Thin public facade re-exporting `Hawk2UI` product-model records (`ProductModel`, `HostTarget`, `SurfaceKind`, `ProductCapability`, `ProductModelError`) from `hawk2ui-schema` and the diagnostic contract (`Diagnostic`, `DiagnosticSeverity`, `RuleId`, `SourceSpan`, `SuggestedFix`, `RelatedContext`) from `hawk2ui-api`, under one crate name.
**Note:** Codex is concurrently committing hardening/remediation work; HEAD drifts. Findings reflect the SHA above and may overlap with in-progress fixes.

## Summary

This crate is **correctly a facade, not a stub** — and that is by design, not a defect. There is no business logic to implement: `lib.rs` is two `pub use` blocks plus the standard `CRATE_NAME`/`crate_name()` identity pair and three smoke tests. `#![forbid(unsafe_code)]` holds, there is no `unsafe`, no `todo!()`/`unimplemented!()`/placeholder `panic!()`, and no TODO/FIXME/HACK markers. **Thinness is intentional and is not reported as a defect here.** Applying the only test that matters for a facade — *does every re-exported item have a self-contained public signature, i.e. does it ever name a type that was not also re-exported?* — the crate **passes**: every method on `ProductModel` mentions only `HostTarget`/`SurfaceKind`/`ProductCapability`/`ProductModelError` (all present), and `ProductModelError → Diagnostic` lands inside the fully re-exported diagnostic module. So the surface that exists is coherent and correct.

The one genuine defect is honesty of the crate-level doc: the `//!` advertises **"runtime entry points"** that the facade does not re-export and that do not exist in this crate at all (no runtime types, no entry-point function). Beyond that, the only observation is one of **scope clarity, not coherence**: core surfaces the `ProductModel` *record* but not the schema operations (`product_model_json_schema`, `validate_product_model_json`, `schema_catalog*`) that act on it — forcing a consumer to reach past the facade to `hawk2ui-schema` — and core forwards only six of the ~50 `Public` types `hawk2ui-api` declares in its own `production_baseline`. Whether that narrow scope is *intended* cannot be confirmed because no rationale is documented; if it is intended, the doc and name should say so. No correctness bug anywhere.

## Completeness & implementation gaps

1. **Crate doc promises "runtime entry points" that are absent** (`lib.rs:2`, `Cargo.toml:10`). The `//!` reads *"Core public facade for `Hawk2UI` product records **and runtime entry points**."* and the package `description` repeats it. The facade re-exports **zero** runtime types and **zero** entry-point functions: nothing from `hawk2ui_api::runtime` (`RuntimeJob`, `RuntimePhase`, `HostBindingContract`, `CapabilityKey`, …) and no callable `run`/`launch`/init function. For a facade, the doc *is* the contract; this one describes a surface the crate does not provide. This is the one self-contained defect. → Either re-export the runtime entry surface or drop the "runtime entry points" clause.
2. **Record re-exported without its operations** (`lib.rs:7-9`). The facade surfaces the `ProductModel` record and its `ProductModelError`, but not the schema operations defined alongside them: `product_model_json_schema`, `validate_product_model_json`, `schema_catalog`, `schema_catalog_json`, the `SchemaCatalog`/`SchemaCatalogEntry` types, `SCHEMA_CATALOG_VERSION`, and the error those four functions return, `SchemaValidationError` (`hawk2ui-schema/src/lib.rs:14-151`, `hawk2ui-schema/src/product.rs:141-175`). A consumer that wants to validate or serialize a `ProductModel` it obtained through the facade must reach past it to `hawk2ui-schema` directly. Note this is *coherent* — `SchemaValidationError` never appears in any re-exported signature, so dropping the functions and their error type together leaves no dangling reference — but it makes the facade less useful than its `ProductModel` re-export implies. Severity is bounded by finding #5 (intent is undocumented, so "should be re-exported" cannot be asserted, only flagged).
3. **The `hawk2ui-api` non-diagnostic surface is not forwarded** (`lib.rs:4-6`). `ApiInventory::production_baseline` (`hawk2ui-api/src/inventory.rs:134-203`) enumerates the api crate's own public baseline: ~50 `Public` types across `Artifact`, `Diagnostic`, `Plugin`, `Runtime`, `Surface`. The facade forwards only the six `Diagnostic`-module types. This is **a scope question, not a coherence defect** — and re-exporting *all* of `hawk2ui-api` would make `hawk2ui-core` a near-redundant alias of `hawk2ui-api` (build/plugin/platform crates already consume those contracts directly, per their manifests). The legitimate ask is only that the curated scope be **stated**: a crate named `*-core` and documented as "the core public facade" implies a broader surface than product-records + diagnostics, so either the surface or the doc should be reconciled. Recorded as a clarity note, not a missing-feature defect.
4. **Facade has zero internal consumers** (workspace-wide grep). No `Cargo.toml` in the workspace depends on `hawk2ui-core`; it is referenced only by its own manifest. It is therefore a purely outward-facing entry point with no in-tree usage exercising the chosen surface — which is why doc/scope drift like (1)–(3) goes unnoticed (see Testing).

## Code quality & smells

- **No real logic to critique** (`lib.rs`). The body is `pub use` re-exports plus the `CRATE_NAME` const and `crate_name()` const-fn. This is appropriate for a facade; flagging "thinness" would be a false positive. The only smells are the surface-coherence items above.
- **Re-export grouping is clean and alphabetized** (`lib.rs:4-9`); the two `pub use` blocks are readable and the import paths are correct. No duplication, no dead code, no leaky internals (everything forwarded is already `pub` upstream).
- **Selection is undocumented at the boundary.** There is no `//!` note explaining *why* only product + diagnostic types are surfaced and the rest of `hawk2ui-api` is not. A one-line rationale would turn findings (2)–(3) from "looks incomplete" into "intentional minimal surface."

## Documentation

- Per-item docs are adequate: `CRATE_NAME` (`lib.rs:11`) and `crate_name()` (`lib.rs:14`) each carry a doc comment; the re-exported items inherit their (thorough) upstream docs, so there are **no undocumented public items**.
- The **crate-level `//!` is the one misleading doc** (finding #1): it advertises "runtime entry points" that are not present. This is the highest-value doc fix.

## Testing

- Three `#[cfg(test)]` smoke tests (`lib.rs:20-49`): `exposes_crate_identity` (asserts `crate_name() == "hawk2ui-core"`), `product_model_is_available_from_core_facade` (builds a `ProductModel`, checks `supports_surface`, and asserts two enum self-equalities), and `diagnostic_contract_is_available_from_core_facade` (constructs a `Diagnostic` and reads `.rule`).
- **These are compile-availability checks, not behavior tests.** `assert_eq!(HostTarget::PluginHost, HostTarget::PluginHost)` and the `ProductCapability` self-equality (`lib.rs:36-40`) assert a reflexive constant — they prove the symbol resolves through the facade, nothing more. That is a legitimate facade smoke test, but the suite asserts *only* that the eleven chosen symbols compile.
- **Coverage gap that masks the doc/scope findings:** no test asserts the facade surface matches its documented intent. Nothing checks that "runtime entry points" are reachable (they are not — #1), that the schema operations are reachable (#2), or pins the curated scope (#3). A facade is exactly the place for a "surface contract" test (cf. `hawk2ui-api/tests/api_inventory.rs`, which pins the upstream baseline); its absence here is why the re-export can drift from the documented intent undetected.

## Cross-cutting conventions (workspace-wide; noted once, not a per-crate defect)

- `CRATE_NAME` + `crate_name()` + the `exposes_crate_identity` test (`lib.rs:11-27`) are the standard identity marker present in every crate. **This crate does *not* carry a duplicate `*_workspace_filter_marker` test** — so on this convention it is actually leaner than `hawk2ui-a11y`; not a defect.
- `#![forbid(unsafe_code)]` is at the crate root (`lib.rs:1`) via the inherited workspace lint. Domain errors (`ProductModelError`, `SchemaValidationError`) convert into `hawk2ui_api::Diagnostic` via `From` **upstream**; the facade neither adds nor breaks that convention. Consistent.

## Security

- **No `unsafe`; trust boundary is not widened.** The facade only re-publishes types that are already `pub` in `hawk2ui-api`/`hawk2ui-schema`; it exposes no internals that were intended to stay private (no `pub(crate)`/`#[doc(hidden)]` leakage), introduces no `unsafe`, and adds no new `Deserialize` impls.
- **`Deserialize` surface is fully inherited, not introduced.** `ProductModel`/`HostTarget`/`SurfaceKind`/`ProductCapability` derive `Deserialize` with `#[serde(deny_unknown_fields)]` on the struct (`hawk2ui-schema/src/product.rs:8-54`); the facade changes none of that. The diagnostic types carry their own upstream serde behavior. There is no recursive/`Vec`-nested untrusted-input traversal *defined in this crate* (the DoS class flagged in `hawk2ui-a11y` does not arise here — `ProductModel`'s `Vec` fields are flat).
- **Net: security surface is minimal and inherited.** The only security-relevant observation is the same as the functional one — the facade is *narrower* than upstream, so it cannot over-expose; the risk is under-exposure (the documented surface is incomplete), not leakage.

### Severity-ranked findings

| # | Severity | Finding | Location |
|---|----------|---------|----------|
| 1 | Medium | Crate doc advertises "runtime entry points" the facade neither re-exports nor contains (misleading contract) | `lib.rs:2`, `Cargo.toml:10` |
| 2 | Low | `ProductModel` record re-exported but not its schema operations (`product_model_json_schema`, `validate_product_model_json`, `schema_catalog*`) or their `SchemaValidationError` | `lib.rs:7-9` vs `hawk2ui-schema/src/lib.rs:14-151` |
| 3 | Low | Curated scope undocumented: only 6 of ~50 `Public` api types forwarded; clarify scope vs. name/doc (not a missing-feature defect) | `lib.rs:4-6` vs `hawk2ui-api/src/inventory.rs:134-203` |
| 4 | Low | No surface-contract test; suite asserts only that 11 chosen symbols compile, so doc/scope drift is undetected | `lib.rs:20-49` |
| 5 | Low | No `//!` rationale for the minimal/curated surface; selection intent undocumented | `lib.rs:2` |

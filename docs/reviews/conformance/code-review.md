# Code Review — hawk2ui-conformance

**Reviewed at:** `5a042c51` · 2026-05-28
**Scope:** `src/lib.rs` + `tests/{product_scope,source_to_render,source_hygiene,manual_entrypoint,manual_source_truth,verification_gates}.rs` (1 src file ~21 LoC / 6 test files ~700 LoC) + `Cargo.toml`.
**Purpose:** "Product conformance checks that keep `Hawk2UI` requirements represented in code and examples." Encodes product requirements as executable tests that cross-check manifests, manuals, scripts, CI, and compatibility matrices against the real workspace.
**Note:** Codex is concurrently committing hardening/remediation work; HEAD drifts. Findings reflect the SHA above and may overlap with in-progress fixes.

## Summary

The conformance surface lives almost entirely in `tests/` — `src/lib.rs` is the standard `crate_name()` + marker only. **That placement is correct by design and is not a defect**: a conformance crate legitimately encodes requirements as integration tests against the other workspace crates (declared as `dev-dependencies`). The tests split sharply into two tiers, and the split is the whole review:

- **Genuinely behavioral (real confidence).** `source_to_render.rs` builds a real `SceneGraph` / `LayerStack` / `RuntimeScheduler` / `AutomationSequence` and asserts on *computed* output (`export_paint_commands(...).serialize_stable() == "draw-rounded-rect:background:12|…"`). `product_scope.rs` parses real `HawkManifest`s and asserts capabilities/targets/parameter counts. The CLI and registry cross-refs in `manual_source_truth.rs` call real `CommandCatalog::parse`, `PropertyRegistry::production()`, and `ApiInventory::production_baseline()`. I confirmed every referenced production fn is a real implementation (`export.rs:122`, `scheduler.rs:343`, `manifest.rs:176`, `inventory.rs:134`, `property.rs:254`) — **not** stubs.
- **Presence-of-strings (false confidence — the central risk).** `verification_gates.rs` and `manual_entrypoint.rs`, plus the doc half of `manual_source_truth.rs`, assert only that a file *contains a substring*. "`scripts/check.sh` contains `cargo deny check`" is not "the dependency policy passes"; "`security.md` mentions `fixtures/security/unsafe-vector.svg`" is not "the validator rejects that SVG." **No test in the crate feeds a security fixture through a validator and asserts denial.** The security-conformance story here is purely documentational.
- **A weak gate that advertises more than it verifies.** `source_hygiene.rs` does real workspace-wide work but its forbidden-token list catches only 5 panic spellings (`.expect(`, `.unwrap(`, `panic!(`, `todo!(`, `unimplemented!(`) — missing `unreachable!(`, `assert!`/`debug_assert!`, slice-index and arithmetic-overflow panics, and recursion-driven stack overflow. It claims "production code cannot panic" but the sibling `hawk2ui-a11y` crate's own top finding (unbounded-recursion stack-overflow DoS) would pass this gate untouched.

No correctness blocker; the crate compiles and the real-tier assertions are sound. The defect is *what a subset of tests assert*, not where they live.

## Completeness & implementation gaps

1. **No behavioral security conformance** (`manual_source_truth.rs::manual_runtime_security_and_packaging_match_machine_readable_gates`). The seven `fixtures/security/*` files (`unsafe-vector.svg`, `unsupported-script.ts`, `hash-mismatch.manifest`, …) are only asserted to be *mentioned* in `security.md`. They are never parsed/validated and no denial diagnostic is asserted. A fixture could be silently neutered (e.g. its malicious content edited away) and the suite stays green so long as the path string survives in the manual. This is the single largest "false assurance" in the crate. → Run each fixture through the real validator (`HawkManifest::parse`, the style/asset compilers) and assert the expected rejection.
2. **Verification gates verify text, not behavior** (`verification_gates.rs`, all four tests). The crate asserts `check.sh` / `check-fast.sh` / `ci.yml` / `verification.md` *contain* command substrings. It never executes a single gate. The four required gate substrings in `check.sh` (`cargo clippy … -D warnings`, `cargo deny check`, `cargo doc`, `git diff --check`) are matched as strings; a commented-out or reordered/broken command still passes. The test name `..._full_script_runs_release_blocking_checks` overstates: nothing *runs*. Presence-checking is legitimately the right tool for *drift detection* (you would not run `cargo deny` inside a unit test), so the fix is **additive**, not deletion: keep the substring checks but add an execution gate (invoke the scripts in CI and gate on exit code) and rename so the "presence-only" guarantee is honest.
3. **`source_to_render` test name oversells, but the body under-delivers vs. its own claim too.** Name: `..._compiles_manifest_authoring_scene_runtime_and_plugin_paths`. The body is actually *stronger* than "compiles" (it asserts real output) — good — but it exercises exactly one happy-path component and one fixed paint string. No malformed-input, no error path, no second component. It is a smoke test mislabeled as broad conformance.
4. **`manual_entrypoint.rs` is a heading spell-check** (`manual_entrypoint_covers_required_product_domains`). Asserts `manual/README.md` contains 7 `##` headings. A heading with zero body content passes. Purely structural.
5. **Doc cross-refs prove mention, not accuracy** (`manual_source_truth.rs`). The strong half enumerates real `PropertyRegistry::production()` entries, `inventory.types()` with `ApiTypeStatus::Public`, parsed compatibility matrices, and asserts each *name* appears in the relevant manual. This catches "doc forgot a public type" (valuable) but not "doc describes the type incorrectly." The guarantee is one-directional (code → doc mention), which the test names ("…document implemented…", "…match machine-readable gates") imply is bidirectional/semantic.

## Code quality & smells

- **`unwrap_or_else(|| panic!)` everywhere in tests is fine, but `source_to_render.rs` uses bare `.expect(...)`/`.unwrap_or_else` while the other five test files standardize on a `read_workspace_file` helper.** `source_to_render.rs` inlines `fs::read_to_string(...).expect(...)` instead. Minor inconsistency; harmless in tests.
- **Five near-identical `workspace_path` / `read_workspace_file` helper pairs** are copy-pasted across `product_scope.rs`, `source_to_render.rs`, `manual_entrypoint.rs`, `manual_source_truth.rs`, `verification_gates.rs`. `source_hygiene.rs` independently reimplements the root walk via `ancestors().nth(2)`. No shared test-support module. Duplication; a `tests/common/mod.rs` would consolidate.
- **`source_hygiene.rs::production_source` truncates at the *first* `\n#[cfg(test)]`** (`source_hygiene.rs:57`). Any production code *after* the first test module in a file is silently dropped from the scan — a real (if narrow) false-negative hole in the gate.
- **`source_hygiene.rs` substring-scans the pre-test region including doc comments and string literals.** A doc comment containing the literal text `panic!(` or a string `".unwrap("` would trip a false *positive*. The gate is textual, not AST-aware. (Currently passing, so no live false positive — but the gate is fragile to benign text.)
- **`#[lib] name`/`path` are redundant.** `Cargo.toml:12-14` restates the default `src/lib.rs` path and the derived lib name. Cosmetic.

## Documentation

- Crate-level `//!` present and accurate (`lib.rs:2`). `CRATE_NAME` and `crate_name()` are both documented (`lib.rs:4,7`). No undocumented public items.
- No fallible public functions, so `# Errors` sections are N/A — correctly absent. Documentation for the (tiny) public surface is complete.
- Test functions themselves carry no doc/comment explaining the *intent vs. limitation* of each gate (e.g. that `verification_gates` is presence-only). Given the false-confidence risk, a one-line comment per presence-only test clarifying "asserts presence, not execution" would prevent future readers from over-trusting them.

## Testing

The crate *is* the test suite; the meta-question is whether its assertions exercise real behavior. Per the gold-standard rubric, here is the per-file verdict:

- **Real behavior (keep):** `source_to_render.rs` (asserts computed paint/runtime/automation output), `product_scope.rs` (parses manifests, asserts parsed fields), and the API/CLI/registry/matrix *parsing* in `manual_source_truth.rs`.
- **Presence-only (false confidence):** `verification_gates.rs` (all 4 tests), `manual_entrypoint.rs`, and every `manual.contains(...)` assertion in `manual_source_truth.rs`. These assert on fabricated/static text rather than runtime behavior — exactly the failure mode this review is meant to flag.
- **Coverage gaps.** No error-path/negative tests anywhere: no malformed manifest, no failing gate, no rejected security fixture. The `source_hygiene` anti-panic gate has a **forbidden-list completeness gap** (see Security): it would green-light `unreachable!`, `assert!`, index/arithmetic panics, and recursive stack overflow in production code — i.e. it cannot catch the very class of DoS the a11y review flagged. No test asserts the gate would *fail* on a planted panic (the gate is never self-tested against a known-bad input).

## Cross-cutting conventions (workspace-wide; noted once, not a per-crate defect)

- `CRATE_NAME` / `crate_name()` plus the `exposes_crate_identity` test — the standard package-identity marker; recurs in every crate. (This crate uses `exposes_crate_identity` rather than a `*_workspace_filter_marker` name, and has no `api_contract`/`domain_test_templates` filter-marker test of its own — consistent enough; not a defect.)
- `#![forbid(unsafe_code)]` at crate root (`lib.rs:1`). No domain error types here, so no `From<…> for Diagnostic` conversions to assess. Consistent.

## Security

The crate holds no `unsafe`, takes no runtime input, performs no allocation/recursion of its own, and only reads workspace files under `CARGO_MANIFEST_DIR` — so it is not itself an attack surface. The security findings are about **the assurance it falsely projects onto the rest of the system**:

- **Documentational-only security conformance (false assurance).** The seven `fixtures/security/*` adversarial inputs are asserted to be *named in `security.md`*, never executed against a validator (`manual_source_truth.rs`). There is **no test that an unsafe SVG, an oversized asset, a hash mismatch, or a malformed manifest is actually *rejected***. A regression that stops rejecting any of these would not be caught here. This is the highest-impact item: the crate's name implies it guards security properties, but it only guards that the manual talks about them.
- **Anti-panic gate is incomplete → false "no DoS" assurance** (`source_hygiene.rs::production_source_does_not_use_panic_style_fallible_assumptions`). The forbidden list `[".expect(", ".unwrap(", "panic!(", "todo!(", "unimplemented!("]` omits `unreachable!(`, `assert!`/`debug_assert!`, slice-index panics (`v[i]`), integer-overflow panics, and `.unwrap_err(`. **Unbounded recursion / stack-overflow DoS — the top finding in the sibling `hawk2ui-a11y` review — passes this gate untouched**, as does any `assert!`-as-DoS. The gate advertises "production code cannot panic" but verifies only 5 spellings; this is a concrete false assurance of a correctness/availability property that is not actually verified.
- **Substring gate can be evaded / spuriously fire.** Because both `source_hygiene.rs` and `verification_gates.rs` reason over raw text (not AST / not exit codes), the security/quality gates they enforce are bypassable by phrasing (panic token inside a macro alias, a comment, or a reordered script) and brittle to benign text. Presence-of-strings is standing in for behavioral verification throughout.
- **No malicious-input handling needed in-crate, but no negative tests for the validators either.** Since the crate never invokes the validators on bad input, the rest of the workspace's input-validation behavior is entirely unverified *from here*.

### Severity-ranked findings

| # | Severity | Finding | Location |
|---|----------|---------|----------|
| 1 | High | Security fixtures only asserted as *mentioned* in docs; never validated/denied → false security assurance | `manual_source_truth.rs::manual_runtime_security_and_packaging_match_machine_readable_gates` |
| 2 | High | Anti-panic gate misses `unreachable!`/`assert!`/index/overflow/recursion → a11y stack-overflow DoS passes; false "no-DoS" assurance | `source_hygiene.rs::production_source_does_not_use_panic_style_fallible_assumptions` |
| 3 | Medium | Verification gates assert command *substrings*, never execute scripts; name says "runs … checks" | `verification_gates.rs` (all 4 tests) |
| 4 | Medium | Doc cross-refs prove *mention*, not accuracy; one-directional; names imply semantic/bidirectional sync | `manual_source_truth.rs` (`manual.contains(...)` assertions) |
| 5 | Low | `source_to_render` exercises one happy-path component/string; no error or negative path | `source_to_render.rs::source_to_render_compiles_manifest_authoring_scene_runtime_and_plugin_paths` |
| 6 | Low | `production_source` truncates at first `#[cfg(test)]` (drops later prod code); textual gate trips on comments/strings | `source_hygiene.rs:57` |
| 7 | Low | `manual_entrypoint` is a heading spell-check; empty sections pass | `manual_entrypoint.rs::manual_entrypoint_covers_required_product_domains` |
| 8 | Low | Five duplicated `workspace_path`/`read_workspace_file` helpers; no shared test-support module | all `tests/*.rs` |

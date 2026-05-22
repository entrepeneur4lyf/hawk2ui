# Hawk2UI Implementation Plan Index

This index maps each final spec and production-readiness gate to its local execution plan. These files are implementation work material and do not redefine the specs.

## Implementation Standard

Hawk2UI implementation plans target complete production behavior, not reduced-scope delivery.

Every plan must produce:

- complete public contracts for its domain,
- production implementations for every required behavior,
- deterministic fixtures for normal, edge, and failure paths,
- automated tests that prove required behavior,
- diagnostics that expose failures clearly,
- release-gate evidence where the domain affects shipping quality.

Partial implementations may be committed only when they are internally coherent, tested, and blocked from being presented as finished product behavior.

## Supplemental Gate Plans

These plans support production readiness but are not final spec numbers.

- Workspace And CI Gates: `docs/superpowers/plans/2026-05-22-0000-workspace-and-ci-gates-plan.md`
- Security Threat Model: `docs/superpowers/plans/2026-05-22-0018-security-threat-model-plan.md`

## Final Spec Plans

Final spec numbers are authoritative. Some plan filenames retain creation-order numbers; use this mapping when selecting implementation work.

- Spec 0001 Product And Scope: `docs/superpowers/plans/2026-05-22-0001-product-and-scope-plan.md`
- Spec 0002 Authoring: `docs/superpowers/plans/2026-05-22-0002-authoring-plan.md`
- Spec 0003 Rendering: `docs/superpowers/plans/2026-05-22-0003-rendering-plan.md`
- Spec 0004 Host And Windowing: `docs/superpowers/plans/2026-05-22-0004-host-windowing-plan.md`
- Spec 0005 Style: `docs/superpowers/plans/2026-05-22-0005-style-plan.md`
- Spec 0006 Layout: `docs/superpowers/plans/2026-05-22-0006-layout-plan.md`
- Spec 0007 Runtime: `docs/superpowers/plans/2026-05-22-0007-runtime-plan.md`
- Spec 0008 Build And Artifacts: `docs/superpowers/plans/2026-05-22-0008-build-artifacts-plan.md`
- Spec 0009 Plugin: `docs/superpowers/plans/2026-05-22-0009-plugin-plan.md`
- Spec 0010 Platform APIs: `docs/superpowers/plans/2026-05-22-0010-platform-apis-plan.md`
- Spec 0011 Security: `docs/superpowers/plans/2026-05-22-0011-security-plan.md`
- Spec 0012 Accessibility: `docs/superpowers/plans/2026-05-22-0012-accessibility-plan.md`
- Spec 0013 Developer Experience: `docs/superpowers/plans/2026-05-22-0013-developer-experience-plan.md`
- Spec 0014 Testing: `docs/superpowers/plans/2026-05-22-0014-testing-plan.md`
- Spec 0015 API Contracts: `docs/superpowers/plans/2026-05-22-0015-api-contracts-plan.md`
- Spec 0016 Compatibility Matrix: `docs/superpowers/plans/2026-05-22-0016-compatibility-matrix-plan.md`
- Spec 0017 Performance And Stability: `docs/superpowers/plans/2026-05-22-0017-production-performance-gates-plan.md`
- Spec 0018 Smoke Apps And Fixtures: `docs/superpowers/plans/2026-05-22-0019-smoke-apps-and-fixtures-plan.md`
- Spec 0019 Release Readiness: `docs/superpowers/plans/2026-05-22-0020-release-readiness-plan.md`
- Spec 0020 Manual Completion: `docs/superpowers/plans/2026-05-22-0021-manual-completion-plan.md`
- Spec 0021 Framework Integrations: `docs/superpowers/plans/2026-05-22-0022-framework-integrations-plan.md`
- Spec 0022 Native Backends And Adapters: `docs/superpowers/plans/2026-05-22-0023-native-backends-and-adapters-plan.md`

## Execution Order

1. Run 0000 to establish workspace, local checks, CI, and dependency hygiene.
2. Run 0015 to lock public API contracts before domain crates drift.
3. Run 0014 early enough to provide testkit support for all domains.
4. Run 0016, 0017, and Security Threat Model to install compatibility, performance, and security gates before broad implementation.
5. Run 0001 to establish product conformance records and example manifests.
6. Run 0002, 0005, 0006, and 0003 to establish source-to-scene basics.
7. Run 0008, 0011, and 0010 to establish artifact and capability boundaries.
8. Run 0007 to connect runtime scheduling to scene updates.
9. Run 0004 to open desktop and embedded surfaces.
10. Run 0009 to wire plugin-specific editor, parameter, state, automation, and realtime data behavior.
11. Run 0018 continuously to prove real smoke applications and plugin fixtures work.
12. Run 0012 and 0013 across the implementation so accessibility and developer experience do not lag behind core runtime work.
13. Run 0021 to complete framework integrations for Svelte, React, Vue, Solid, and direct native authoring.
14. Run 0022 to complete production renderer, text, asset, host, script, plugin, and package adapters.
15. Run 0020 continuously so manuals track implemented behavior.
16. Run 0019 before each release candidate.

## Size Rule

Each plan must stay below 750 lines. If a plan approaches that size, split it by milestone before implementation begins.

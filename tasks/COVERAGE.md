# Spec To Task Coverage Matrix

This matrix records the current alignment from final specs to implementation task lists and implementation plans.

## Coverage

| Spec | Task Lists | Implementation Plans |
| --- | --- | --- |
| `specs/0001-product-and-scope.md` | `tasks/0001-core-contracts.md` | `docs/superpowers/plans/2026-05-22-0001-product-and-scope-plan.md` |
| `specs/0002-authoring.md` | `tasks/0002-authoring-style-layout-rendering.md`, `tasks/0007-framework-integrations.md` | `docs/superpowers/plans/2026-05-22-0002-authoring-plan.md`, `docs/superpowers/plans/2026-05-22-0022-framework-integrations-plan.md` |
| `specs/0003-rendering.md` | `tasks/0002-authoring-style-layout-rendering.md`, `tasks/0008-native-backends-and-adapters.md` | `docs/superpowers/plans/2026-05-22-0003-rendering-plan.md`, `docs/superpowers/plans/2026-05-22-0023-native-backends-and-adapters-plan.md` |
| `specs/0004-host-windowing.md` | `tasks/0004-host-windowing-plugin.md`, `tasks/0008-native-backends-and-adapters.md` | `docs/superpowers/plans/2026-05-22-0004-host-windowing-plan.md`, `docs/superpowers/plans/2026-05-22-0023-native-backends-and-adapters-plan.md` |
| `specs/0005-style.md` | `tasks/0002-authoring-style-layout-rendering.md` | `docs/superpowers/plans/2026-05-22-0005-style-plan.md` |
| `specs/0006-layout.md` | `tasks/0002-authoring-style-layout-rendering.md` | `docs/superpowers/plans/2026-05-22-0006-layout-plan.md` |
| `specs/0007-runtime.md` | `tasks/0003-build-runtime-platform-security.md`, `tasks/0007-framework-integrations.md`, `tasks/0008-native-backends-and-adapters.md` | `docs/superpowers/plans/2026-05-22-0007-runtime-plan.md`, `docs/superpowers/plans/2026-05-22-0022-framework-integrations-plan.md`, `docs/superpowers/plans/2026-05-22-0023-native-backends-and-adapters-plan.md` |
| `specs/0008-build-artifacts.md` | `tasks/0001-core-contracts.md`, `tasks/0003-build-runtime-platform-security.md`, `tasks/0008-native-backends-and-adapters.md` | `docs/superpowers/plans/2026-05-22-0008-build-artifacts-plan.md`, `docs/superpowers/plans/2026-05-22-0023-native-backends-and-adapters-plan.md` |
| `specs/0009-plugin.md` | `tasks/0004-host-windowing-plugin.md`, `tasks/0008-native-backends-and-adapters.md` | `docs/superpowers/plans/2026-05-22-0009-plugin-plan.md`, `docs/superpowers/plans/2026-05-22-0023-native-backends-and-adapters-plan.md` |
| `specs/0010-platform-apis.md` | `tasks/0003-build-runtime-platform-security.md` | `docs/superpowers/plans/2026-05-22-0010-platform-apis-plan.md` |
| `specs/0011-security.md` | `tasks/0003-build-runtime-platform-security.md` | `docs/superpowers/plans/2026-05-22-0011-security-plan.md`, `docs/superpowers/plans/2026-05-22-0018-security-threat-model-plan.md` |
| `specs/0012-accessibility.md` | `tasks/0005-accessibility-dx-manuals.md` | `docs/superpowers/plans/2026-05-22-0012-accessibility-plan.md` |
| `specs/0013-developer-experience.md` | `tasks/0005-accessibility-dx-manuals.md`, `tasks/0007-framework-integrations.md` | `docs/superpowers/plans/2026-05-22-0013-developer-experience-plan.md` |
| `specs/0014-testing.md` | `tasks/0001-core-contracts.md` | `docs/superpowers/plans/2026-05-22-0014-testing-plan.md` |
| `specs/0015-api-contracts.md` | `tasks/0000-production-gates.md`, `tasks/0001-core-contracts.md` | `docs/superpowers/plans/2026-05-22-0015-api-contracts-plan.md` |
| `specs/0016-compatibility-matrix.md` | `tasks/0000-production-gates.md`, `tasks/0004-host-windowing-plugin.md` | `docs/superpowers/plans/2026-05-22-0016-compatibility-matrix-plan.md` |
| `specs/0017-performance-and-stability.md` | `tasks/0000-production-gates.md`, `tasks/0003-build-runtime-platform-security.md`, `tasks/0004-host-windowing-plugin.md` | `docs/superpowers/plans/2026-05-22-0017-production-performance-gates-plan.md` |
| `specs/0018-smoke-apps-and-fixtures.md` | `tasks/0006-smoke-release-readiness.md` | `docs/superpowers/plans/2026-05-22-0019-smoke-apps-and-fixtures-plan.md` |
| `specs/0019-release-readiness.md` | `tasks/0000-production-gates.md`, `tasks/0006-smoke-release-readiness.md` | `docs/superpowers/plans/2026-05-22-0020-release-readiness-plan.md` |
| `specs/0020-manual-completion.md` | `tasks/0005-accessibility-dx-manuals.md`, `tasks/0006-smoke-release-readiness.md` | `docs/superpowers/plans/2026-05-22-0021-manual-completion-plan.md` |
| `specs/0021-framework-integrations.md` | `tasks/0007-framework-integrations.md` | `docs/superpowers/plans/2026-05-22-0022-framework-integrations-plan.md` |
| `specs/0022-native-backends-and-adapters.md` | `tasks/0008-native-backends-and-adapters.md` | `docs/superpowers/plans/2026-05-22-0023-native-backends-and-adapters-plan.md` |

## Audit Rule

Every final spec must map to at least one task list and at least one implementation plan. If a spec lacks either mapping, the implementation queue is incomplete.

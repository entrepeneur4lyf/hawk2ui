# Dependency Hygiene

Dependency hygiene is a production gate. New dependencies must pass the local
policy before they can be committed.

## Local Commands

- `rtk cargo deny check`
- `rtk cargo tree -d`
- `rtk cargo metadata --format-version 1`
- `rtk cargo test -p xtask repository_dependency_policy_tracks_release_blocking_dependency_risks`

## Approval Rules

- Licenses must be present in `deny.toml`.
- Yanked crates are denied.
- Unknown registries and unknown Git sources are denied.
- Duplicate versions are warnings until a dependency owner accepts or removes
  them.
- Every alpha, pre-1.0, fast-moving, native, or Git dependency that affects a
  production domain must be listed in `release/dependency-policy.toml`.
- Git dependencies are release blockers unless they are isolated from published
  crates or replaced with a crates.io release.
- Dependency upgrades must run the dependency's `upgrade_gate` command from
  `release/dependency-policy.toml` before the version is accepted.

## Tracked High-Risk Dependencies

- `boa_engine` is currently pinned to a Git revision and is a release blocker.
- The OXC compiler family is version-aligned at `0.133.0` and must be upgraded
  as one compatibility-tested set.
- `lightningcss` is accepted on the alpha line only behind Hawk2UI's typed CSS
  subset and style tests.
- `taffy` is accepted as the layout engine with release gates for flex, text
  measurement, DPI, and plugin constraints.
- `skia-safe` is accepted as the renderer binding with renderer and host tests
  required for upgrades.

## Adding A Dependency

1. Add the dependency in the narrowest crate that needs it.
2. Run `rtk cargo metadata --format-version 1`.
3. Run `rtk cargo deny check`.
4. If the dependency is alpha, pre-1.0, fast-moving, native, or from Git, add it
   to `release/dependency-policy.toml` with an owner, risk statement, release
   blocker flag, and upgrade gate.
5. If a new license is required, update `deny.toml` in the same commit and
   document why the license is acceptable.

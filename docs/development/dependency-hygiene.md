# Dependency Hygiene

Dependency hygiene is a production gate. New dependencies must pass the local
policy before they can be committed.

## Local Commands

- `rtk cargo deny check`
- `rtk cargo tree -d`
- `rtk cargo metadata --format-version 1`

## Approval Rules

- Licenses must be present in `deny.toml`.
- Yanked crates are denied.
- Unknown registries and unknown Git sources are denied.
- Duplicate versions are warnings until a dependency owner accepts or removes
  them.

## Adding A Dependency

1. Add the dependency in the narrowest crate that needs it.
2. Run `rtk cargo metadata --format-version 1`.
3. Run `rtk cargo deny check`.
4. If a new license is required, update `deny.toml` in the same commit and
   document why the license is acceptable.

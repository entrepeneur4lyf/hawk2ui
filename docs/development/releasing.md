# Releasing Hawk2UI

Hawk2UI releases are gated by executable checks. A release candidate is not ready until every command below exits successfully from the repository root.

## Command Sequence

1. `rtk bash scripts/release-check.sh --version-only`
2. `rtk bash scripts/release-check.sh --packages-only`
3. `rtk bash scripts/release-check.sh --changelog-only`
4. `rtk bash scripts/release-check.sh`

## What The Full Check Runs

The full release check validates release criteria, version policy, package target declarations, and changelog evidence before running `scripts/check.sh`.

`scripts/check.sh` runs formatting, clippy with warnings denied, all workspace tests, documentation builds, and dependency policy checks.

## Failure Policy

Do not tag, package, publish, or announce a release when any release check fails. Fix the failing gate, rerun the exact command, and keep the failure output as engineering context until the gate passes.

## Evidence

Release evidence belongs under `target/release-evidence/`. Evidence files are generated or captured by the concrete domain tasks that satisfy each release criterion.

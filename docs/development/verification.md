# Verification Gates

Hawk2UI uses local commands that mirror CI release gates.

## Fast Gate

Run before committing routine implementation work:

```bash
bash scripts/check-fast.sh
```

The fast gate runs formatting, workspace compilation, and workspace tests.

## Full Gate

Run before merging release-bound implementation work:

```bash
bash scripts/check.sh
```

The full gate runs formatting, clippy with warnings denied, tests, documentation build, and dependency policy checks.

## Xtask Entry Points

The same gates are available through `xtask`:

```bash
cargo run -p xtask -- check-fast
cargo run -p xtask -- check
```

## Blocking Policy

A failed fast gate blocks task completion.

A failed full gate blocks release readiness and any claim of production-ready stability.

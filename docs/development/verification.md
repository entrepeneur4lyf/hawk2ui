# Verification Gates

Hawk2UI uses local commands that mirror CI release gates. Run commands through
`rtk` locally so shell output and failures are captured consistently.

## Before Every Commit

Run the fast gate before committing routine implementation work:

```bash
rtk bash scripts/check-fast.sh
```

The fast gate runs formatting, workspace compilation, workspace tests, API
contract filters, domain template filters, and smoke app fixtures.

## Before Every Release

Run the full gate before release-bound implementation work:

```bash
rtk bash scripts/check.sh
```

The full gate runs formatting, clippy with warnings denied, workspace tests,
source-to-render integration, smoke fixtures, render baseline benchmark,
documentation build, dependency policy, and whitespace diff checks.

## Merge-Blocking Failures

The following failures block merge:

- Formatting failure from `rtk cargo fmt --all -- --check`.
- Compilation failure from `rtk cargo check --workspace`.
- Test failure from `rtk cargo test --workspace`.
- Zero or failing filtered contract gates such as `api_contract` and
  `domain_test_templates`.
- Smoke fixture failure from `rtk cargo test -p hawk2ui-smoke --test smoke_apps`.

## Release-Blocking Failures

The following failures block release readiness:

- Any merge-blocking failure.
- Clippy warnings from `rtk cargo clippy --workspace -- -D warnings`.
- Integration failure from `rtk cargo test --test source_to_render`.
- Performance gate failure from `rtk cargo bench --bench render_baseline -- --quick`.
- Documentation failure from `rtk cargo doc --workspace --no-deps`.
- Dependency policy failure from `rtk cargo deny check`.
- Whitespace failure from `rtk git diff --check`.

## CI Troubleshooting

Use the failing CI job name to run the equivalent local command:

- `format`: `rtk cargo fmt --all -- --check`.
- `clippy`: `rtk cargo clippy --workspace -- -D warnings`.
- `unit tests`: `rtk cargo test --workspace`.
- `integration tests`: `rtk cargo test --test source_to_render`.
- `dependency policy`: `rtk cargo deny check`.
- `docs`: `rtk cargo doc --workspace --no-deps`.
- `examples and smoke fixtures`: `rtk cargo test -p hawk2ui-smoke --test smoke_apps`.

## Xtask Entry Points

The same gates are available through `xtask`:

```bash
rtk cargo run -p xtask -- check-fast
rtk cargo run -p xtask -- check
```

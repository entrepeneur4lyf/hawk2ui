#!/usr/bin/env bash
set -euo pipefail

cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
cargo test --test source_to_render
cargo test -p hawk2ui-smoke --test smoke_apps
cargo bench --bench render_baseline -- --quick
cargo bench --bench release_gates -- --quick
bun install --frozen-lockfile
bun run test:react-package
bun run typecheck:react-package
cargo doc --workspace --no-deps
cargo deny check
git diff --check

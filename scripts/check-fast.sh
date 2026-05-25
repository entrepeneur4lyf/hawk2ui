#!/usr/bin/env bash
set -euo pipefail

cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo test --workspace api_contract
cargo test --workspace domain_test_templates
cargo test -p hawk2ui-smoke --test smoke_apps

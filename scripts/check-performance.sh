#!/usr/bin/env bash
set -euo pipefail

cargo test -p hawk2ui-perf
cargo bench -p hawk2ui-perf --bench startup -- --quick
cargo bench -p hawk2ui-perf --bench style -- --quick
cargo bench -p hawk2ui-perf --bench layout -- --quick
cargo bench -p hawk2ui-perf --bench render -- --quick
cargo bench -p hawk2ui-perf --bench script -- --quick
cargo bench -p hawk2ui-perf --bench assets -- --quick
cargo bench -p hawk2ui-perf --bench runtime -- --quick
cargo bench -p hawk2ui-perf --bench package -- --quick
cargo bench -p hawk2ui-perf --bench desktop_host -- --quick
cargo bench -p hawk2ui-perf --bench plugin_realtime -- --quick

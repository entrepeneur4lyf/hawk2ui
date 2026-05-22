#!/usr/bin/env bash
set -euo pipefail

case "${1:-}" in
  --version-only)
    cargo run -p xtask -- release-check --version-only
    ;;
  --packages-only)
    cargo run -p xtask -- release-check --packages-only
    ;;
  --changelog-only)
    cargo run -p xtask -- release-check --changelog-only
    ;;
  "")
    cargo run -p xtask -- release-check
    ;;
  *)
    echo "Usage: scripts/release-check.sh [--version-only|--packages-only|--changelog-only]" >&2
    exit 2
    ;;
esac

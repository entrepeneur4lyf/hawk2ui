#!/usr/bin/env bash
set -euo pipefail

workspace_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
manifest_path="${workspace_root}/Cargo.toml"

framework_examples=(
  "svelte-basic"
  "vue-basic"
  "solid-basic"
)

for example in "${framework_examples[@]}"; do
  example_root="${workspace_root}/examples/frameworks/${example}"
  output="$(
    cd "${example_root}"
    cargo run --manifest-path "${manifest_path}" -q -p hawk2ui-cli -- build-dev
  )"

  printf '%s\n' "${output}"

  if [[ "${output}" != *"compiled-frameworks: 1"* ]]; then
    printf 'framework example %s did not produce one compiled framework artifact\n' "${example}" >&2
    exit 1
  fi

  if [[ "${output}" != *"compiled-scripts: 0"* ]]; then
    printf 'framework example %s unexpectedly compiled a script entrypoint\n' "${example}" >&2
    exit 1
  fi

  if [[ "${output}" != *"verification-status: release-ready"* ]]; then
    printf 'framework example %s did not finish release-ready verification\n' "${example}" >&2
    exit 1
  fi
done

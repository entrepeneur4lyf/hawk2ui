use std::{fs, path::PathBuf};

fn workspace_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn read_workspace_file(path: &str) -> String {
    fs::read_to_string(workspace_path(path))
        .unwrap_or_else(|error| panic!("required gate file `{path}` must be readable: {error}"))
}

fn assert_contains(input: &str, required: &str) {
    assert!(
        input.contains(required),
        "gate file must contain `{required}`"
    );
}

#[test]
fn verification_gates_fast_script_matches_ci_fast_scope() {
    let script = read_workspace_file("scripts/check-fast.sh");

    for required in [
        "cargo fmt --all -- --check",
        "cargo check --workspace",
        "cargo test --workspace",
        "cargo test --workspace api_contract",
        "cargo test --workspace domain_test_templates",
        "cargo test -p hawk2ui-smoke --test smoke_apps",
    ] {
        assert_contains(&script, required);
    }
}

#[test]
fn verification_gates_full_script_runs_release_blocking_checks() {
    let script = read_workspace_file("scripts/check.sh");

    for required in [
        "cargo fmt --all -- --check",
        "cargo clippy --workspace -- -D warnings",
        "cargo test --workspace",
        "cargo test --test source_to_render",
        "cargo test -p hawk2ui-smoke --test smoke_apps",
        "cargo bench --bench render_baseline -- --quick",
        "cargo doc --workspace --no-deps",
        "cargo deny check",
        "git diff --check",
    ] {
        assert_contains(&script, required);
    }
}

#[test]
fn verification_gates_ci_has_named_jobs_for_each_gate_family() {
    let workflow = read_workspace_file(".github/workflows/ci.yml");

    for required in [
        "name: format",
        "name: clippy",
        "name: unit tests",
        "name: integration tests",
        "name: dependency policy",
        "name: docs",
        "name: examples and smoke fixtures",
        "ubuntu-latest",
        "windows-latest",
        "macos-latest",
    ] {
        assert_contains(&workflow, required);
    }
}

#[test]
fn verification_gates_docs_cover_commit_release_ci_troubleshooting_and_blockers() {
    let docs = read_workspace_file("docs/development/verification.md");

    for required in [
        "## Before Every Commit",
        "## Before Every Release",
        "## Merge-Blocking Failures",
        "## Release-Blocking Failures",
        "## CI Troubleshooting",
        "rtk bash scripts/check-fast.sh",
        "rtk bash scripts/check.sh",
        "rtk cargo deny check",
    ] {
        assert_contains(&docs, required);
    }
}

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
fn verification_gate_definitions_fast_script_matches_ci_fast_scope() {
    let script = read_workspace_file("scripts/check-fast.sh");

    // Conformance tests assert gate definitions stay wired; CI executes these scripts.
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
fn verification_gate_definitions_full_script_lists_release_blocking_checks() {
    let script = read_workspace_file("scripts/check.sh");

    // This is a definition drift check. Script execution remains owned by CI and release runs.
    for required in [
        "cargo fmt --all -- --check",
        "cargo clippy --workspace -- -D warnings",
        "cargo test --workspace",
        "cargo test --test source_to_render",
        "cargo test -p hawk2ui-smoke --test smoke_apps",
        "cargo bench --bench render_baseline -- --quick",
        "cargo bench --bench release_gates -- --quick",
        "bun install --frozen-lockfile",
        "bun run test:react-package",
        "bun run typecheck:react-package",
        "bun run test:vue-package",
        "bun run typecheck:vue-package",
        "cargo doc --workspace --no-deps",
        "cargo deny check",
        "git diff --check",
    ] {
        assert_contains(&script, required);
    }
}

#[test]
fn release_blocking_full_script_does_not_run_incubating_framework_examples() {
    let criteria = read_workspace_file("release/release-criteria.toml");
    let script = read_workspace_file("scripts/check.sh");

    assert_contains(&criteria, "id = \"framework-compilers\"");
    assert_contains(&criteria, "blocking = \"advisory\"");
    assert!(
        !script.contains("scripts/check-framework-examples.sh"),
        "incubating framework compiler examples must not be run by release-blocking check.sh"
    );
}

#[test]
fn release_blocking_full_script_uses_react_package_checks_not_incubating_suite() {
    let package_json = read_workspace_file("package.json");
    let script = read_workspace_file("scripts/check.sh");

    assert_contains(&package_json, "\"test:react-package\"");
    assert_contains(&package_json, "\"typecheck:react-package\"");
    assert_contains(&package_json, "\"test:vue-package\"");
    assert_contains(&package_json, "\"typecheck:vue-package\"");
    assert_contains(&script, "bun run test:react-package");
    assert_contains(&script, "bun run typecheck:react-package");
    assert_contains(&script, "bun run test:vue-package");
    assert_contains(&script, "bun run typecheck:vue-package");
    assert!(
        !script.contains("bun run test:packages"),
        "release-blocking check.sh must not run the all-framework package suite"
    );
    assert!(
        !script.contains("bun run typecheck:packages"),
        "release-blocking check.sh must not run the all-framework package typecheck"
    );
}

#[test]
fn verification_gate_definitions_ci_has_named_jobs_for_each_gate_family() {
    let workflow = read_workspace_file(".github/workflows/ci.yml");

    // This checks required CI job declarations, not the outcome of a live CI run.
    for required in [
        "name: format",
        "name: clippy",
        "name: unit tests",
        "name: integration tests",
        "name: dependency policy",
        "name: docs",
        "name: examples and smoke fixtures",
        "name: framework packages",
        "oven-sh/setup-bun@v2",
        "bun install --frozen-lockfile",
        "bun run test:packages",
        "bun run typecheck:packages",
        "generated npm package verification",
        "cargo run -p xtask -- npm-packages --verify",
        "generated npm package publish dry-run",
        "cargo run -p xtask -- npm-packages --publish-dry-run",
        "scripts/check-framework-examples.sh",
        "cargo bench --bench release_gates -- --quick",
        "ubuntu-latest",
        "windows-latest",
        "macos-latest",
    ] {
        assert_contains(&workflow, required);
    }
}

#[test]
fn performance_budget_definitions_track_react_deno_runtime_path() {
    let budgets = read_workspace_file("performance/budgets.toml");
    let script_bench = read_workspace_file("crates/hawk2ui-perf/benches/script.rs");
    let release_gate_bench = read_workspace_file("crates/hawk2ui-perf/benches/release_gates.rs");
    let performance_script = read_workspace_file("scripts/check-performance.sh");

    for required in [
        "name = \"js-evaluate\"",
        "fixture = \"examples/react-desktop-basic\"",
        "name = \"react-package-size\"",
        "release_gate = true\nfixture = \"examples/react-desktop-basic\"",
        "name = \"react-plugin-package-size\"",
        "release_gate = true\nfixture = \"examples/react-plugin-basic\"",
    ] {
        assert_contains(&budgets, required);
    }
    assert_contains(&script_bench, "\"js-evaluate\"");
    assert_contains(&script_bench, "examples/react-desktop-basic");
    assert_contains(&release_gate_bench, "\"react-package-size\"");
    assert_contains(&release_gate_bench, "react_desktop_fixture");
    assert_contains(&release_gate_bench, "\"react-plugin-package-size\"");
    assert_contains(&release_gate_bench, "react_plugin_fixture");
    assert_contains(
        &performance_script,
        "cargo bench -p hawk2ui-perf --bench release_gates -- --quick",
    );
}

#[test]
fn release_criteria_execute_capability_api_gate_tests() {
    let criteria = read_workspace_file("release/release-criteria.toml");

    for required in [
        "id = \"capability-apis\"",
        "cargo test -p hawk2ui-js-runtime capabilities -- --nocapture",
        "cargo test -p hawk2ui-platform --test platform_capabilities -- --nocapture",
        "cargo test -p hawk2ui-security -- --nocapture",
        "cargo test -p hawk2ui-smoke react_ -- --nocapture",
        "cargo test -p hawk2ui-smoke vue_ -- --nocapture",
    ] {
        assert_contains(&criteria, required);
    }
}

#[test]
fn release_criteria_execute_vue_deno_runtime_gate_tests() {
    let criteria = read_workspace_file("release/release-criteria.toml");

    for required in [
        "id = \"vue-deno-runtime\"",
        "title = \"Vue Deno runtime path\"",
        "cargo test -p hawk2ui-js-runtime vue_ -- --nocapture",
        "bun run test:vue-package",
        "bun run typecheck:vue-package",
        "cargo test -p hawk2ui-smoke vue_ -- --nocapture",
        "target/release-evidence/vue-deno-runtime.txt",
    ] {
        assert_contains(&criteria, required);
    }
}

#[test]
fn release_criteria_execute_generated_npm_package_gate_tests() {
    let criteria = read_workspace_file("release/release-criteria.toml");

    for required in [
        "id = \"generated-npm-packages\"",
        "cargo run -p xtask -- npm-packages --verify",
        "id = \"generated-npm-packages-publish-dry-run\"",
        "cargo run -p xtask -- npm-packages --publish-dry-run",
    ] {
        assert_contains(&criteria, required);
    }
}

#[test]
fn release_criteria_execute_react_and_vue_developer_experience_gate_tests() {
    let criteria = read_workspace_file("release/release-criteria.toml");

    for required in [
        "id = \"developer-experience\"",
        "title = \"React and Vue developer experience\"",
        "cargo test -p hawk2ui-cli workspace_init_ -- --nocapture",
        "cargo test -p hawk2ui-cli workspace_dev_ -- --nocapture",
        "cargo test -p hawk2ui-cli dev_loop_ -- --nocapture",
    ] {
        assert_contains(&criteria, required);
    }
}

#[test]
fn release_criteria_and_dependency_policy_execute_v8_artifact_checks() {
    let criteria = read_workspace_file("release/release-criteria.toml");
    let dependency_policy = read_workspace_file("release/dependency-policy.toml");

    for required in [
        "id = \"v8-artifact-policy\"",
        "cargo test -p hawk2ui-js-runtime --test v8_artifacts -- --nocapture",
        "cargo test -p xtask repository_package_targets_cover_required_outputs -- --nocapture",
    ] {
        assert_contains(&criteria, required);
    }
    assert_contains(
        &dependency_policy,
        "upgrade_gate = \"cargo test -p hawk2ui-js-runtime --test v8_artifacts -- --nocapture\"",
    );
}

#[test]
fn framework_compiler_examples_are_incubating_and_do_not_gate_react_release() {
    let criteria = read_workspace_file("release/release-criteria.toml");
    let script = read_workspace_file("scripts/check-framework-examples.sh");

    assert_contains(&criteria, "id = \"framework-compilers\"");
    assert_contains(&criteria, "blocking = \"advisory\"");
    assert!(
        !script.contains("\"react-basic\""),
        "React examples must not be verified through the legacy framework compiler path"
    );
    for incubating in ["\"svelte-basic\"", "\"vue-basic\"", "\"solid-basic\""] {
        assert_contains(&script, incubating);
    }
}

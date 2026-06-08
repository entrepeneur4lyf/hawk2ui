use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use hawk2ui_build::{PackageManagerKind, PackageManagerSelection};

#[test]
fn package_manager_detects_each_lockfile_and_commands() {
    for (kind, lockfile, install_args) in [
        (
            PackageManagerKind::Bun,
            "bun.lock",
            vec!["install", "--frozen-lockfile"],
        ),
        (PackageManagerKind::Npm, "package-lock.json", vec!["ci"]),
        (
            PackageManagerKind::Pnpm,
            "pnpm-lock.yaml",
            vec!["install", "--frozen-lockfile"],
        ),
        (
            PackageManagerKind::Yarn,
            "yarn.lock",
            vec!["install", "--immutable"],
        ),
    ] {
        let root = temp_project(&format!("pm-{}", kind.as_str()));
        fs::write(root.join(lockfile), format!("{lockfile}\n")).expect("lockfile writes");

        let selected =
            PackageManagerSelection::detect(&root, None).expect("package manager detects");

        assert_eq!(selected.kind(), kind);
        assert_eq!(
            selected.lockfile_path(),
            Some(root.join(lockfile).as_path())
        );
        assert_eq!(
            selected.lockfile_sha256(),
            Some(sha256_hex(format!("{lockfile}\n").as_bytes()).as_str())
        );
        assert_eq!(selected.install_command().program(), kind.as_str());
        assert_eq!(selected.install_command().args(), install_args.as_slice());
        assert_eq!(selected.build_command().args(), ["run", "build"]);
        assert_eq!(selected.version_command().args(), ["--version"]);
    }
}

#[test]
fn package_manager_rejects_ambiguous_lockfiles_without_explicit_selection() {
    let root = temp_project("pm-ambiguous");
    fs::write(root.join("bun.lock"), "bun\n").expect("bun lock writes");
    fs::write(root.join("package-lock.json"), "npm\n").expect("npm lock writes");

    let error = PackageManagerSelection::detect(&root, None)
        .expect_err("ambiguous lockfiles require explicit selection");

    assert_eq!(error.rule(), "build.package-manager.ambiguous");
    assert!(error.message().contains("bun"));
    assert!(error.message().contains("npm"));
}

#[test]
fn package_manager_ambiguous_lockfile_error_names_conflicting_files() {
    let root = temp_project("pm-ambiguous-files");
    fs::write(root.join("bun.lock"), "bun\n").expect("bun lock writes");
    fs::write(root.join("package-lock.json"), "npm\n").expect("npm lock writes");

    let error = PackageManagerSelection::detect(&root, None)
        .expect_err("ambiguous lockfiles require explicit selection");

    assert_eq!(error.rule(), "build.package-manager.ambiguous");
    assert!(error.message().contains("bun.lock"), "{}", error.message());
    assert!(
        error.message().contains("package-lock.json"),
        "{}",
        error.message()
    );
}

#[test]
fn package_manager_missing_lockfile_error_names_supported_inputs() {
    let root = temp_project("pm-missing");

    let error = PackageManagerSelection::detect(&root, None)
        .expect_err("missing lockfile must produce actionable diagnostic");

    assert_eq!(error.rule(), "build.package-manager.missing");
    for lockfile in [
        "bun.lock",
        "package-lock.json",
        "pnpm-lock.yaml",
        "yarn.lock",
    ] {
        assert!(error.message().contains(lockfile), "{}", error.message());
    }
}

#[test]
fn package_manager_explicit_selection_resolves_ambiguous_lockfiles() {
    let root = temp_project("pm-explicit");
    fs::write(root.join("bun.lock"), "bun\n").expect("bun lock writes");
    fs::write(root.join("yarn.lock"), "yarn\n").expect("yarn lock writes");

    let selected = PackageManagerSelection::detect(&root, Some(PackageManagerKind::Yarn))
        .expect("explicit yarn selection resolves ambiguity");

    assert_eq!(selected.kind(), PackageManagerKind::Yarn);
    assert_eq!(
        selected.lockfile_path(),
        Some(root.join("yarn.lock").as_path())
    );
    assert_eq!(
        selected.lockfile_sha256(),
        Some(sha256_hex(b"yarn\n").as_str())
    );
}

#[test]
fn package_manager_metadata_records_reproducible_command_specs() {
    let root = temp_project("pm-metadata-commands");
    fs::write(root.join("pnpm-lock.yaml"), "pnpm\n").expect("pnpm lock writes");

    let selected = PackageManagerSelection::detect(&root, None).expect("package manager detects");
    let metadata = selected.metadata();

    assert_eq!(metadata.kind, PackageManagerKind::Pnpm);
    assert_eq!(metadata.install_command.program(), "pnpm");
    assert_eq!(
        metadata.install_command.args(),
        ["install", "--frozen-lockfile"]
    );
    assert_eq!(metadata.build_command.program(), "pnpm");
    assert_eq!(metadata.build_command.args(), ["run", "build"]);
    assert_eq!(metadata.version_command.program(), "pnpm");
    assert_eq!(metadata.version_command.args(), ["--version"]);
}

#[test]
fn package_manager_metadata_records_resolved_version_evidence() {
    let root = temp_project("pm-metadata-version");
    fs::write(root.join("yarn.lock"), "yarn\n").expect("yarn lock writes");

    let selected = PackageManagerSelection::detect(&root, None).expect("package manager detects");
    let metadata = selected.metadata().with_package_manager_version("4.5.1\n");

    assert_eq!(metadata.kind, PackageManagerKind::Yarn);
    assert_eq!(metadata.package_manager_version.as_deref(), Some("4.5.1"));
    assert_eq!(metadata.version_command.program(), "yarn");
    assert_eq!(metadata.version_command.args(), ["--version"]);
}

fn temp_project(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock is valid")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("hawk2ui-build-{name}-{unique}"));
    fs::create_dir_all(&path).expect("temp project creates");
    path
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};

    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

#[allow(
    dead_code,
    reason = "keeps Path imported for clearer assertion diagnostics"
)]
fn _path_type_marker(_: &Path) {}

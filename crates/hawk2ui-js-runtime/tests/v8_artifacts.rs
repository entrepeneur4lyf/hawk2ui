use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use hawk2ui_js_runtime::{RustyV8ArtifactSet, sha256_file};

#[test]
fn rusty_v8_artifact_set_verifies_archive_and_binding_pair() {
    let root = temp_root("valid");
    fs::create_dir_all(&root).expect("temp root is created");
    let archive = root.join("librusty_v8_release_x86_64-unknown-linux-gnu.a.gz");
    let binding = root.join("src_binding_release_x86_64-unknown-linux-gnu.rs");
    fs::write(&archive, b"archive-bytes").expect("archive fixture is written");
    fs::write(&binding, b"binding-bytes").expect("binding fixture is written");

    let artifacts = RustyV8ArtifactSet::new(
        "x86_64-unknown-linux-gnu",
        "release",
        archive.clone(),
        sha256_file(&archive).expect("archive hash computes"),
        binding.clone(),
        sha256_file(&binding).expect("binding hash computes"),
    );

    artifacts.verify().expect("artifact pair verifies");
    remove_temp_root(&root);
}

#[test]
fn rusty_v8_artifact_set_rejects_missing_binding_pair() {
    let root = temp_root("missing-binding");
    fs::create_dir_all(&root).expect("temp root is created");
    let archive = root.join("librusty_v8_release_x86_64-unknown-linux-gnu.a.gz");
    let binding = root.join("src_binding_release_x86_64-unknown-linux-gnu.rs");
    fs::write(&archive, b"archive-bytes").expect("archive fixture is written");

    let artifacts = RustyV8ArtifactSet::new(
        "x86_64-unknown-linux-gnu",
        "release",
        archive.clone(),
        sha256_file(&archive).expect("archive hash computes"),
        binding,
        "00",
    );

    let error = artifacts
        .verify()
        .expect_err("missing binding must be rejected");
    assert_eq!(error.rule(), "js-runtime.v8-artifact.missing");
    remove_temp_root(&root);
}

#[test]
fn rusty_v8_artifact_set_rejects_wrong_target_names() {
    let root = temp_root("wrong-target");
    fs::create_dir_all(&root).expect("temp root is created");
    let archive = root.join("librusty_v8_release_aarch64-unknown-linux-gnu.a.gz");
    let binding = root.join("src_binding_release_x86_64-unknown-linux-gnu.rs");
    fs::write(&archive, b"archive-bytes").expect("archive fixture is written");
    fs::write(&binding, b"binding-bytes").expect("binding fixture is written");

    let artifacts = RustyV8ArtifactSet::new(
        "x86_64-unknown-linux-gnu",
        "release",
        archive.clone(),
        sha256_file(&archive).expect("archive hash computes"),
        binding.clone(),
        sha256_file(&binding).expect("binding hash computes"),
    );

    let error = artifacts
        .verify()
        .expect_err("wrong target archive name must be rejected");
    assert_eq!(error.rule(), "js-runtime.v8-artifact.invalid-name");
    remove_temp_root(&root);
}

#[test]
fn rusty_v8_artifact_set_rejects_hash_mismatch() {
    let root = temp_root("hash-mismatch");
    fs::create_dir_all(&root).expect("temp root is created");
    let archive = root.join("librusty_v8_release_x86_64-unknown-linux-gnu.a.gz");
    let binding = root.join("src_binding_release_x86_64-unknown-linux-gnu.rs");
    fs::write(&archive, b"archive-bytes").expect("archive fixture is written");
    fs::write(&binding, b"binding-bytes").expect("binding fixture is written");

    let artifacts = RustyV8ArtifactSet::new(
        "x86_64-unknown-linux-gnu",
        "release",
        archive,
        "00",
        binding.clone(),
        sha256_file(&binding).expect("binding hash computes"),
    );

    let error = artifacts
        .verify()
        .expect_err("hash mismatch must be rejected");
    assert_eq!(error.rule(), "js-runtime.v8-artifact.hash-mismatch");
    remove_temp_root(&root);
}

#[test]
fn rusty_v8_artifact_policy_rejects_source_build_requests() {
    let error = RustyV8ArtifactSet::reject_source_build_request(true)
        .expect_err("source-build fallback must be rejected");

    assert_eq!(
        error.rule(),
        "js-runtime.v8-artifact.source-build-unsupported"
    );
    assert!(error.message().contains("prebuilt rusty_v8 artifacts"));
    assert!(RustyV8ArtifactSet::reject_source_build_request(false).is_ok());
}

fn temp_root(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "hawk2ui-js-runtime-{label}-{}-{nonce}",
        std::process::id()
    ))
}

fn remove_temp_root(path: &Path) {
    let _ = fs::remove_dir_all(path);
}

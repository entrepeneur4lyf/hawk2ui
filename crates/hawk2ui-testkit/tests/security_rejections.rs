use hawk2ui_testkit::{
    SecurityFixtureKind, SecurityRejection, SecurityRejectionFixtureSet,
    SecurityRejectionFixtureSetError, SecurityRejectionMatrix, SecurityRejectionMatrixError,
};
use std::path::{Path, PathBuf};

#[test]
fn security_rejections_cover_required_fixture_families() {
    let fixtures = SecurityRejectionFixtureSet::production_baseline();

    for kind in [
        SecurityFixtureKind::InvalidManifest,
        SecurityFixtureKind::MissingAsset,
        SecurityFixtureKind::UnsafeAsset,
        SecurityFixtureKind::AssetHashMismatch,
        SecurityFixtureKind::OversizedAsset,
        SecurityFixtureKind::UnsupportedScript,
        SecurityFixtureKind::UnsupportedStyle,
    ] {
        assert!(fixtures.fixture(kind).is_some(), "missing {kind:?}");
    }

    fixtures
        .verify_fixture_paths(workspace_root())
        .expect("production security fixtures must exist");
}

#[test]
fn security_rejections_catalog_uses_real_validator_diagnostics() {
    let fixtures = SecurityRejectionFixtureSet::production_baseline();

    for (kind, expected_rule) in [
        (SecurityFixtureKind::InvalidManifest, "manifest.malformed"),
        (SecurityFixtureKind::MissingAsset, "asset.missing"),
        (
            SecurityFixtureKind::UnsafeAsset,
            "asset.vector.unsafe-content",
        ),
        (
            SecurityFixtureKind::AssetHashMismatch,
            "asset.hash.mismatch",
        ),
        (
            SecurityFixtureKind::OversizedAsset,
            "asset.limit.bytes-exceeded",
        ),
        (SecurityFixtureKind::UnsupportedScript, "script.eval.failed"),
        (
            SecurityFixtureKind::UnsupportedStyle,
            "style.property.unknown",
        ),
    ] {
        let fixture = fixtures.fixture(kind).expect("fixture kind exists");
        assert_eq!(fixture.diagnostic_rule(), expected_rule);
    }
}

#[test]
fn security_rejections_report_missing_fixture_paths() {
    let fixtures = SecurityRejectionFixtureSet::new([hawk2ui_testkit::SecurityFixture::new(
        SecurityFixtureKind::InvalidManifest,
        "fixtures/security/does-not-exist.toml",
        "manifest.invalid",
    )]);

    assert_eq!(
        fixtures.verify_fixture_paths(workspace_root()),
        Err(SecurityRejectionFixtureSetError::MissingFixturePath {
            path: PathBuf::from("fixtures/security/does-not-exist.toml")
        })
    );
}

#[test]
fn security_rejections_require_every_capability_boundary_to_have_a_case() {
    let matrix = SecurityRejectionMatrix::new([
        SecurityRejection::new(
            "fs.read",
            "fixtures/security/fs-read-denied.toml",
            "security.capability.denied",
        ),
        SecurityRejection::new(
            "network.fetch",
            "fixtures/security/network-fetch-denied.toml",
            "security.capability.denied",
        ),
    ]);

    matrix
        .require_capabilities(&["fs.read", "network.fetch"])
        .expect("declared capabilities covered");

    assert_eq!(
        matrix.require_capabilities(&["clipboard.write"]),
        Err(SecurityRejectionMatrixError::MissingCapability(
            "clipboard.write".to_string()
        ))
    );
}

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("testkit crate lives under crates/")
}

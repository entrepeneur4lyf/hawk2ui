use hawk2ui_testkit::{
    SecurityFixtureKind, SecurityRejection, SecurityRejectionFixtureSet, SecurityRejectionMatrix,
    SecurityRejectionMatrixError,
};

#[test]
fn security_rejections_cover_required_fixture_families() {
    let fixtures = SecurityRejectionFixtureSet::production_baseline();

    for kind in [
        SecurityFixtureKind::UndeclaredCapability,
        SecurityFixtureKind::UnsupportedSourceFeature,
        SecurityFixtureKind::UnsafeAsset,
        SecurityFixtureKind::InvalidManifest,
        SecurityFixtureKind::DeniedHostApi,
        SecurityFixtureKind::SecretLeak,
    ] {
        assert!(fixtures.fixture(kind).is_some(), "missing {kind:?}");
    }
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

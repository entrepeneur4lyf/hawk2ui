use hawk2ui_security_model::{
    AttackFixtures, AttackFixturesError, CapabilityRejections, CapabilityRejectionsError,
    CapabilityVerdict, PackageSignatureStatus, PackageTrustRecord, PackageTrustValidator,
    PackageTrustViolation, RuntimeAuthorityPolicy, RuntimeOperation, Severity, ThreatModel,
    ThreatModelError, VerificationReportStatus,
};

const THREAT_MODEL: &str = include_str!("../../../security/threat-model.toml");
const REJECTION_CASES: &str = include_str!("../../../security/rejection-cases.toml");
const ATTACK_FIXTURES: &str = include_str!("../../../security/source-asset-fixtures.toml");

#[test]
fn security_threat_registry_covers_required_domains() {
    let model = ThreatModel::parse(THREAT_MODEL).expect("threat model parses");

    for id in [
        "malicious-source",
        "malformed-artifact",
        "unsafe-asset",
        "hostile-package-input",
        "malicious-plugin-host-data",
        "untrusted-user-data",
        "secret-exposure",
        "denied-platform-authority",
    ] {
        assert!(model.contains(id), "missing threat {id}");
    }

    assert!(model.threats.iter().all(|threat| {
        !threat.affected_domain.is_empty()
            && !threat.mitigation.is_empty()
            && !threat.required_test.is_empty()
            && matches!(threat.severity, Severity::High | Severity::Critical)
    }));
}

#[test]
fn threat_registry_rejects_duplicate_ids() {
    let input = r#"
[[threats]]
id = "malicious-source"
severity = "critical"
affected_domain = "source-validation"
mitigation = "Reject unsupported syntax before runtime."
required_test = "source_asset_fixtures_reject_unsupported_style"

[[threats]]
id = "malicious-source"
severity = "high"
affected_domain = "source-validation"
mitigation = "Reject unsupported syntax before runtime."
required_test = "source_asset_fixtures_reject_unsupported_style"
"#;

    let error = ThreatModel::parse(input).expect_err("duplicate IDs must fail");

    assert_eq!(
        error,
        ThreatModelError::DuplicateThreat("malicious-source".into())
    );
}

#[test]
fn threat_registry_rejects_missing_required_test() {
    let input = r#"
[[threats]]
id = "malicious-source"
severity = "critical"
affected_domain = "source-validation"
mitigation = "Reject unsupported syntax before runtime."
required_test = ""
"#;

    let error = ThreatModel::parse(input).expect_err("missing test link must fail");

    assert_eq!(
        error,
        ThreatModelError::MissingRequiredField {
            id: "malicious-source".into(),
            field: "required_test"
        }
    );
}

#[test]
fn capability_rejections_cover_all_capabilities() {
    let cases = CapabilityRejections::parse(REJECTION_CASES).expect("rejection cases parse");

    for capability in [
        "filesystem",
        "network",
        "clipboard",
        "secrets",
        "database",
        "package-targets",
        "plugin-metadata",
        "host-apis",
        "runtime-bindings",
    ] {
        assert!(
            cases.has_allow_and_deny(capability),
            "missing allow and deny coverage for {capability}"
        );
    }
}

#[test]
fn capability_rejections_reject_missing_deny_case() {
    let input = r#"
[[cases]]
id = "filesystem-allow"
capability = "filesystem"
verdict = "allow"
diagnostic_rule = "capability.filesystem.allow"
fixture = "fixtures/security/filesystem-allow.toml"
"#;

    let error = CapabilityRejections::parse(input).expect_err("missing deny must fail");

    assert_eq!(
        error,
        CapabilityRejectionsError::MissingVerdict {
            capability: "filesystem".into(),
            verdict: CapabilityVerdict::Deny
        }
    );
}

#[test]
fn capability_rejections_reject_duplicate_case_ids() {
    let input = r#"
[[cases]]
id = "filesystem-deny"
capability = "filesystem"
verdict = "deny"
diagnostic_rule = "capability.filesystem.deny"
fixture = "fixtures/security/filesystem-deny.toml"

[[cases]]
id = "filesystem-deny"
capability = "filesystem"
verdict = "deny"
diagnostic_rule = "capability.filesystem.deny"
fixture = "fixtures/security/filesystem-deny.toml"
"#;

    let error = CapabilityRejections::parse(input).expect_err("duplicate case IDs must fail");

    assert_eq!(
        error,
        CapabilityRejectionsError::DuplicateCase("filesystem-deny".into())
    );
}

#[test]
fn source_asset_fixtures_map_each_attack_to_diagnostic() {
    let fixtures = AttackFixtures::parse(ATTACK_FIXTURES).expect("attack fixtures parse");

    for id in [
        "unsupported-style-syntax",
        "unsupported-script-syntax",
        "unsafe-vector-content",
        "oversized-asset",
        "hash-mismatch",
        "missing-asset",
        "malformed-manifest",
    ] {
        let fixture = fixtures
            .get(id)
            .unwrap_or_else(|| panic!("missing fixture {id}"));
        assert!(!fixture.path.is_empty());
        assert!(!fixture.diagnostic_rule.is_empty());
    }
}

#[test]
fn source_asset_fixtures_reject_duplicate_ids() {
    let input = r#"
[[fixtures]]
id = "unsafe-vector-content"
path = "fixtures/security/unsafe-vector.svg"
diagnostic_rule = "asset.vector.unsafe"

[[fixtures]]
id = "unsafe-vector-content"
path = "fixtures/security/unsafe-vector.svg"
diagnostic_rule = "asset.vector.unsafe"
"#;

    let error = AttackFixtures::parse(input).expect_err("duplicate fixture IDs must fail");

    assert_eq!(
        error,
        AttackFixturesError::DuplicateFixture("unsafe-vector-content".into())
    );
}

#[test]
fn runtime_authority_denies_unsafe_operations() {
    let policy = RuntimeAuthorityPolicy::sandboxed();

    for operation in [
        RuntimeOperation::StringToCode,
        RuntimeOperation::UndeclaredHostApi,
        RuntimeOperation::DirectFilesystem,
        RuntimeOperation::DirectNetwork,
        RuntimeOperation::ProcessSpawn,
        RuntimeOperation::NativeModuleLoading,
    ] {
        assert!(policy.is_denied(operation), "{operation:?} must be denied");
    }
}

#[test]
fn runtime_authority_redacts_secret_payloads() {
    let policy = RuntimeAuthorityPolicy::sandboxed();
    let diagnostic = policy.redact_diagnostic(
        "Denied token sk_live_1234567890 and source payload Function('return secrets')",
    );

    assert!(!diagnostic.contains("sk_live_1234567890"));
    assert!(!diagnostic.contains("Function('return secrets')"));
    assert!(diagnostic.contains("[redacted-secret]"));
    assert!(diagnostic.contains("[redacted-source]"));
}

#[test]
fn package_trust_accepts_complete_verified_record() {
    let record = PackageTrustRecord {
        artifact_schema_version: 1,
        manifest_snapshot_hash: "blake3:manifest".into(),
        compiled_asset_hashes: vec!["blake3:asset".into()],
        compiled_script_hashes: vec!["blake3:script".into()],
        target_metadata: "linux-wayland-desktop".into(),
        signature_status: PackageSignatureStatus::Verified,
        verification_report_status: VerificationReportStatus::Present,
    };

    assert!(PackageTrustValidator::new(1).validate(&record).is_ok());
}

#[test]
fn package_trust_rejects_tampered_artifact() {
    let record = PackageTrustRecord {
        artifact_schema_version: 2,
        manifest_snapshot_hash: "blake3:manifest".into(),
        compiled_asset_hashes: vec!["blake3:asset".into()],
        compiled_script_hashes: vec!["blake3:script".into()],
        target_metadata: "linux-wayland-desktop".into(),
        signature_status: PackageSignatureStatus::Verified,
        verification_report_status: VerificationReportStatus::Present,
    };

    let error = PackageTrustValidator::new(1)
        .validate(&record)
        .expect_err("schema mismatch must fail");

    assert_eq!(
        error,
        PackageTrustViolation::ArtifactSchemaMismatch {
            expected: 1,
            actual: 2
        }
    );
}

#[test]
fn package_trust_rejects_missing_verification_report() {
    let record = PackageTrustRecord {
        artifact_schema_version: 1,
        manifest_snapshot_hash: "blake3:manifest".into(),
        compiled_asset_hashes: vec!["blake3:asset".into()],
        compiled_script_hashes: vec!["blake3:script".into()],
        target_metadata: "linux-wayland-desktop".into(),
        signature_status: PackageSignatureStatus::Verified,
        verification_report_status: VerificationReportStatus::Missing,
    };

    let error = PackageTrustValidator::new(1)
        .validate(&record)
        .expect_err("missing report must fail");

    assert_eq!(error, PackageTrustViolation::MissingVerificationReport);
}

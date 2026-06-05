use ed25519_dalek::{Signer, SigningKey};
use hawk2ui_api::{Diagnostic, DiagnosticSeverity};
use hawk2ui_build::{
    ARTIFACT_SIGNATURE_ALGORITHM_ED25519_SHA256_V1, ArtifactHash, ArtifactSchemaVersion,
    ArtifactSignature, ArtifactSignatureVerificationKey, ArtifactSignatureVerifier,
    AssetManifestEntry, CompiledAssetRecord, CompiledScriptRecord, HawkManifest, SealedArtifact,
};
use hawk2ui_security_model::{
    AttackFixtures, AttackFixturesError, CapabilityRejections, CapabilityRejectionsError,
    CapabilityVerdict, PackageSignatureStatus, PackageTrustRecord, PackageTrustValidator,
    PackageTrustViolation, RuntimeAuthorityPolicy, RuntimeOperation, Severity, ThreatModel,
    ThreatModelError, VerificationReportStatus,
};

const THREAT_MODEL: &str = include_str!("../../../security/threat-model.toml");
const REJECTION_CASES: &str = include_str!("../../../security/rejection-cases.toml");
const ATTACK_FIXTURES: &str = include_str!("../../../security/source-asset-fixtures.toml");

fn sign_artifact(
    artifact: SealedArtifact,
    key_id: &str,
) -> (SealedArtifact, ArtifactSignatureVerifier) {
    let signing_key = SigningKey::from_bytes(&[9; 32]);
    let signature = signing_key.sign(&artifact.signature_payload_bytes());
    let verifier =
        ArtifactSignatureVerifier::new([ArtifactSignatureVerificationKey::ed25519_sha256_v1(
            key_id,
            signing_key.verifying_key().to_bytes(),
        )]);
    let signed = artifact.with_signature(ArtifactSignature::verified(
        ARTIFACT_SIGNATURE_ALGORITHM_ED25519_SHA256_V1,
        key_id,
        encode_hex(&signature.to_bytes()),
    ));
    (signed, verifier)
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

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
fn security_threat_registry_required_tests_exist() {
    // Each threat's `required_test` must name a real test fn in this file, so the registry's
    // "every threat is test-covered" promise is enforced rather than decorative. All security
    // tests live here; a cross-crate `required_test` would (correctly) fail this check.
    let source = include_str!("threat_model.rs");
    let model = ThreatModel::parse(THREAT_MODEL).expect("threat model parses");
    for threat in &model.threats {
        let definition = format!("fn {}(", threat.required_test);
        assert!(
            source.contains(&definition),
            "threat `{}` references required_test `{}`, which is not a test fn in this file",
            threat.id,
            threat.required_test
        );
    }
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
fn runtime_authority_redacts_diverse_secret_formats() {
    let policy = RuntimeAuthorityPolicy::sandboxed();

    for secret in [
        "pk_live_abcdefghij",
        "rk_test_0987654321",
        "AKIAIOSFODNN7EXAMPLE",
        "ghp_aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789",
        "xoxb-111-222-aaaaaaaaaaaa",
    ] {
        // Quoted and punctuation-adjacent occurrences must still be redacted.
        let message = format!("leaked \"{secret}\" in config={secret};");
        let redacted = policy.redact_diagnostic(&message);
        assert!(
            !redacted.contains(secret),
            "expected `{secret}` to be redacted, got `{redacted}`"
        );
        assert!(redacted.contains("[redacted-secret]"));
        assert!(
            redacted.contains("config="),
            "surrounding text must be preserved"
        );
    }

    let bearer = policy.redact_diagnostic("auth Bearer eyJhbGciOiJIUzI1NiJ9abcdefghij next");
    assert!(!bearer.contains("eyJhbGciOiJIUzI1NiJ9abcdefghij"));
    assert!(bearer.contains("[redacted-secret]"));
    assert!(bearer.contains("Bearer") && bearer.contains("next"));

    let eval = policy.redact_diagnostic("ran eval(danger) at runtime");
    assert!(!eval.contains("eval(danger"));
    assert!(eval.contains("[redacted-source]"));
    assert!(eval.contains("at runtime"));
}

#[test]
fn package_trust_accepts_complete_verified_record() {
    // `Verified` is only obtainable via the crypto path; a complete, genuinely-verified record
    // with a present verification report validates.
    let record = signed_trust_record(VerificationReportStatus::Present);
    assert!(PackageTrustValidator::new(1).validate(&record).is_ok());
}

#[test]
fn package_trust_record_is_derived_from_actual_sealed_artifact_payloads() {
    let manifest = HawkManifest::parse(
        r#"
[identity]
id = "com.hawk2ui.secure"
name = "Secure"
version = "1.0.0"

[source]
entry = "src/main.ts"

[[targets]]
kind = "desktop"
name = "linux-wayland"
"#,
    )
    .expect("manifest parses");
    let artifact = SealedArtifact::from_manifest(ArtifactSchemaVersion::new(1, 0), &manifest)
        .with_compiled_script(
            CompiledScriptRecord::new(
                "main",
                "src/main.ts",
                "scripts/main.hawk.js",
                ArtifactHash::from_bytes(b"script-source"),
            )
            .with_compiled_source("compiled script"),
        )
        .with_asset_manifest_entry(AssetManifestEntry::new(
            "hero",
            "image",
            "assets/hero.pack",
            ArtifactHash::from_bytes(b"asset-payload"),
        ))
        .with_compiled_asset(CompiledAssetRecord::new(
            "hero",
            "assets/hero.png",
            "assets/hero.pack",
            ArtifactHash::from_bytes(b"asset-source"),
        ));
    let (artifact, verifier) = sign_artifact(artifact, "release-key");

    let record = PackageTrustRecord::from_trusted_sealed_artifact(
        &artifact,
        &verifier,
        VerificationReportStatus::Present,
    );

    assert_eq!(record.artifact_schema_version, 1);
    assert_eq!(
        record.manifest_snapshot_hash,
        artifact.manifest_snapshot_hash.0
    );
    assert!(
        record
            .compiled_asset_hashes
            .contains(&ArtifactHash::from_bytes(b"asset-source").0)
    );
    assert!(
        record
            .compiled_script_hashes
            .contains(&ArtifactHash::from_bytes(b"compiled script").0)
    );
    assert_eq!(record.target_metadata, "desktop:linux-wayland");
    assert!(PackageTrustValidator::new(1).validate(&record).is_ok());

    let metadata_only =
        PackageTrustRecord::from_sealed_artifact(&artifact, VerificationReportStatus::Present);
    assert_eq!(
        metadata_only.signature_status,
        PackageSignatureStatus::Invalid
    );
    assert_eq!(
        PackageTrustValidator::new(1)
            .validate(&metadata_only)
            .expect_err("metadata-only signature status is not trusted"),
        PackageTrustViolation::InvalidSignature
    );
}

#[test]
fn package_trust_accepts_signed_artifacts_without_declared_assets() {
    let record = signed_trust_record_without_assets(VerificationReportStatus::Present);

    assert_eq!(record.compiled_asset_hashes, Vec::<String>::new());
    assert!(PackageTrustValidator::new(1).validate(&record).is_ok());
}

#[test]
fn package_trust_rejects_tampered_artifact() {
    let record = PackageTrustRecord {
        artifact_schema_version: 2,
        manifest_snapshot_hash: valid_hash("manifest"),
        compiled_asset_hashes: vec![valid_hash("asset")],
        compiled_script_hashes: vec![valid_hash("script")],
        target_metadata: "linux-wayland-desktop".into(),
        signature_status: PackageSignatureStatus::Missing,
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
    // `Verified` can only come from the crypto path, so build a genuinely-verified record and
    // confirm a missing verification report still fails closed.
    let record = signed_trust_record(VerificationReportStatus::Missing);

    let error = PackageTrustValidator::new(1)
        .validate(&record)
        .expect_err("missing report must fail even for a verified signature");

    assert_eq!(error, PackageTrustViolation::MissingVerificationReport);
}

#[test]
fn package_trust_rejects_malformed_hashes() {
    for (field, record) in [
        (
            "manifest_snapshot_hash",
            PackageTrustRecord {
                artifact_schema_version: 1,
                manifest_snapshot_hash: "blake3:manifest".into(),
                compiled_asset_hashes: vec![valid_hash("asset")],
                compiled_script_hashes: vec![valid_hash("script")],
                target_metadata: "linux-wayland-desktop".into(),
                signature_status: PackageSignatureStatus::Missing,
                verification_report_status: VerificationReportStatus::Present,
            },
        ),
        (
            "compiled_asset_hashes",
            PackageTrustRecord {
                artifact_schema_version: 1,
                manifest_snapshot_hash: valid_hash("manifest"),
                compiled_asset_hashes: vec!["sha256:not-hex".into()],
                compiled_script_hashes: vec![valid_hash("script")],
                target_metadata: "linux-wayland-desktop".into(),
                signature_status: PackageSignatureStatus::Missing,
                verification_report_status: VerificationReportStatus::Present,
            },
        ),
        (
            "compiled_script_hashes",
            PackageTrustRecord {
                artifact_schema_version: 1,
                manifest_snapshot_hash: valid_hash("manifest"),
                compiled_asset_hashes: vec![valid_hash("asset")],
                compiled_script_hashes: vec!["md5:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into()],
                target_metadata: "linux-wayland-desktop".into(),
                signature_status: PackageSignatureStatus::Missing,
                verification_report_status: VerificationReportStatus::Present,
            },
        ),
    ] {
        let error = PackageTrustValidator::new(1)
            .validate(&record)
            .expect_err("malformed hash must fail");

        assert_eq!(
            error,
            PackageTrustViolation::InvalidHash {
                field: field.into()
            }
        );
    }
}

#[test]
fn package_trust_violation_converts_to_shared_diagnostic() {
    let diagnostic = Diagnostic::from(PackageTrustViolation::InvalidHash {
        field: "compiled_script_hashes".into(),
    });

    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(diagnostic.rule.as_str(), "security.package.hash-invalid");
    assert!(diagnostic.message.contains("compiled_script_hashes"));
}

fn valid_hash(label: &str) -> String {
    let fill = match label {
        "manifest" => 'a',
        "asset" => 'b',
        "script" => 'c',
        _ => 'd',
    };
    format!("sha256:{}", fill.to_string().repeat(64))
}

/// Builds a genuinely signature-verified trust record via the crypto path (the only way to
/// obtain `PackageSignatureStatus::Verified`), with the given verification-report status.
fn signed_trust_record(report: VerificationReportStatus) -> PackageTrustRecord {
    let manifest = HawkManifest::parse(
        r#"
[identity]
id = "com.hawk2ui.secure"
name = "Secure"
version = "1.0.0"

[source]
entry = "src/main.ts"

[[targets]]
kind = "desktop"
name = "linux-wayland"
"#,
    )
    .expect("manifest parses");
    let artifact = SealedArtifact::from_manifest(ArtifactSchemaVersion::new(1, 0), &manifest)
        .with_compiled_script(
            CompiledScriptRecord::new(
                "main",
                "src/main.ts",
                "scripts/main.hawk.js",
                ArtifactHash::from_bytes(b"script-source"),
            )
            .with_compiled_source("compiled script"),
        )
        .with_asset_manifest_entry(AssetManifestEntry::new(
            "hero",
            "image",
            "assets/hero.pack",
            ArtifactHash::from_bytes(b"asset-payload"),
        ));
    let (artifact, verifier) = sign_artifact(artifact, "release-key");
    PackageTrustRecord::from_trusted_sealed_artifact(&artifact, &verifier, report)
}

fn signed_trust_record_without_assets(report: VerificationReportStatus) -> PackageTrustRecord {
    let manifest = HawkManifest::parse(
        r#"
[identity]
id = "com.hawk2ui.secure-no-assets"
name = "Secure No Assets"
version = "1.0.0"

[source]
entry = "src/main.ts"

[[targets]]
kind = "plugin"
name = "audio-plugin"
"#,
    )
    .expect("manifest parses");
    let artifact = SealedArtifact::from_manifest(ArtifactSchemaVersion::new(1, 0), &manifest)
        .with_compiled_script(
            CompiledScriptRecord::new(
                "main",
                "src/main.ts",
                "scripts/main.hawk.js",
                ArtifactHash::from_bytes(b"script-source"),
            )
            .with_compiled_source("compiled script"),
        );
    let (artifact, verifier) = sign_artifact(artifact, "release-key");
    PackageTrustRecord::from_trusted_sealed_artifact(&artifact, &verifier, report)
}

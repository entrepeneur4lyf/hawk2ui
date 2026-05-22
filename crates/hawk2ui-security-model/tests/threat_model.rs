use hawk2ui_security_model::{
    CapabilityRejections, CapabilityRejectionsError, CapabilityVerdict, Severity, ThreatModel,
    ThreatModelError,
};

const THREAT_MODEL: &str = include_str!("../../../security/threat-model.toml");
const REJECTION_CASES: &str = include_str!("../../../security/rejection-cases.toml");

#[test]
fn threat_registry_covers_required_domains() {
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

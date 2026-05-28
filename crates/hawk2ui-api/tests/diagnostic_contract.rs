use hawk2ui_api::{Diagnostic, DiagnosticSeverity, RelatedContext, SourceSpan, SuggestedFix};

#[test]
fn diagnostic_contract_serializes_cli_ready_payloads() {
    let diagnostic =
        Diagnostic::error("manifest.identity.missing", "manifest identity is required")
            .with_source(SourceSpan::new("Hawk.toml", 1, 1, 1, 10))
            .with_related(RelatedContext::new("target", "desktop"))
            .with_fix(SuggestedFix::new("Add an [identity] section."))
            .redacted();

    let json = serde_json::to_string(&diagnostic).expect("diagnostic serializes");

    assert!(json.contains("manifest.identity.missing"));
    assert!(json.contains("Hawk.toml"));
    assert!(json.contains("Add an [identity] section."));
    assert!(json.contains("\"redacted\":true"));
}

#[test]
fn diagnostic_redacted_flag_is_advisory_and_does_not_scrub_payloads() {
    let diagnostic = Diagnostic::error("secret.leaked", "token=abc123")
        .with_related(RelatedContext::new("secret", "abc123"))
        .redacted();

    assert!(diagnostic.redacted);
    assert_eq!(diagnostic.message, "token=abc123");
    assert!(diagnostic.to_cli_string().contains("abc123"));
}

#[test]
fn diagnostic_contract_formats_stable_cli_snapshot() {
    let diagnostic = Diagnostic::warning("style.unsupported", "property is not supported")
        .with_source(SourceSpan::new("styles/main.hawk.css", 12, 5, 12, 18))
        .with_related(RelatedContext::new("property", "backdrop-filter"));

    assert_eq!(
        diagnostic.to_cli_string(),
        "warning style.unsupported styles/main.hawk.css:12:5..12:18 property is not supported [property=backdrop-filter]"
    );
}

#[test]
fn diagnostic_contract_deserializes_without_losing_rule_identity() {
    let input = r#"{
        "severity":"Error",
        "rule":"runtime.host-call.denied",
        "message":"host call denied",
        "source":null,
        "fixes":[],
        "related":[],
        "redacted":false
    }"#;

    let diagnostic: Diagnostic = serde_json::from_str(input).expect("diagnostic deserializes");

    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(diagnostic.rule.as_str(), "runtime.host-call.denied");
}

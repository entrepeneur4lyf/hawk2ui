use hawk2ui_security::{TrustBoundary, TrustRecord};

#[test]
fn trust_boundaries_classify_all_security_domains() {
    let records = [
        TrustRecord::new("source", TrustBoundary::AuthorSource, "workspace"),
        TrustRecord::new("artifact", TrustBoundary::CompiledArtifact, "build"),
        TrustRecord::new("runtime", TrustBoundary::RuntimeState, "runtime"),
        TrustRecord::new("host", TrustBoundary::HostData, "host"),
        TrustRecord::new("user", TrustBoundary::UserData, "user"),
        TrustRecord::new("plugin-host", TrustBoundary::PluginHostData, "plugin-host"),
        TrustRecord::new("secret", TrustBoundary::Secret, "secret-store"),
        TrustRecord::new("asset", TrustBoundary::Asset, "asset-bundle"),
    ];

    assert_eq!(records.len(), 8);
    assert!(records.iter().all(|record| !record.id.is_empty()));
    assert_eq!(records[0].boundary.label(), "author source");
    assert_eq!(records[7].boundary.label(), "asset");
}

#[test]
fn trust_boundaries_emit_stable_diagnostic_labels() {
    let secret = TrustRecord::new("api-key", TrustBoundary::Secret, "manifest");
    let host = TrustRecord::new("clipboard", TrustBoundary::HostData, "host");

    assert_eq!(secret.diagnostic_label(), "trust.secret:api-key@manifest");
    assert_eq!(host.diagnostic_label(), "trust.host-data:clipboard@host");
}

use hawk2ui_security::{
    ScriptSandboxOperation, ScriptSandboxPolicy, SourceValidationPolicy, SourceValidationRule,
    TrustBoundary, TrustRecord,
};

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

#[test]
fn source_validation_rejects_all_build_time_security_rules() {
    let cases = [
        (
            SourceValidationRule::UnsupportedStyleSyntax,
            "style.unsupported",
            "style syntax is unsupported",
        ),
        (
            SourceValidationRule::UnsupportedScriptSyntax,
            "script.unsupported",
            "script syntax is unsupported",
        ),
        (
            SourceValidationRule::UnsafeVectorContent,
            "asset.vector.unsafe",
            "vector content is unsafe",
        ),
        (
            SourceValidationRule::MissingAsset,
            "asset.missing",
            "declared asset is missing",
        ),
        (
            SourceValidationRule::UndeclaredCapability,
            "capability.undeclared",
            "capability is not declared",
        ),
        (
            SourceValidationRule::MalformedManifest,
            "manifest.malformed",
            "manifest is malformed",
        ),
        (
            SourceValidationRule::InvalidPluginMetadata,
            "plugin.metadata.invalid",
            "plugin metadata is invalid",
        ),
        (
            SourceValidationRule::InvalidPackageTarget,
            "target.invalid",
            "package target is invalid",
        ),
    ];

    for (rule, expected_rule, expected_message) in cases {
        let record = SourceValidationPolicy::reject(rule, "Hawk.toml");

        assert_eq!(record.diagnostic.rule, expected_rule);
        assert_eq!(record.diagnostic.message, expected_message);
        assert_eq!(
            record.diagnostic_label(),
            format!("source.{expected_rule}:Hawk.toml")
        );
    }
}

#[test]
fn script_sandbox_denies_all_direct_privileged_operations() {
    let cases = [
        (
            ScriptSandboxOperation::StringToCode,
            "script.string-to-code.denied",
            "string-to-code execution is denied",
        ),
        (
            ScriptSandboxOperation::UndeclaredHostApi,
            "script.host-api.undeclared",
            "host API access is undeclared",
        ),
        (
            ScriptSandboxOperation::DirectFilesystem,
            "script.filesystem.denied",
            "direct filesystem access is denied",
        ),
        (
            ScriptSandboxOperation::DirectNetwork,
            "script.network.denied",
            "direct network access is denied",
        ),
        (
            ScriptSandboxOperation::ProcessSpawning,
            "script.process.denied",
            "process spawning is denied",
        ),
        (
            ScriptSandboxOperation::NativeModuleLoading,
            "script.native-module.denied",
            "native module loading is denied",
        ),
    ];

    for (operation, expected_rule, expected_message) in cases {
        let denial = ScriptSandboxPolicy::deny(operation, "src/app.ts");

        assert_eq!(denial.diagnostic.rule, expected_rule);
        assert_eq!(denial.diagnostic.message, expected_message);
        assert_eq!(
            denial.diagnostic_label(),
            format!("sandbox.{expected_rule}:src/app.ts")
        );
    }
}

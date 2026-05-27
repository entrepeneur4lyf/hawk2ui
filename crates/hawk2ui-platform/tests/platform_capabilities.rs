use hawk2ui_platform::{
    CapabilityRecord, CapabilitySchema, CapabilityTable, ClipboardDataType, ClipboardManifest,
    ClipboardPolicy, DatabaseMigration, DatabasePolicy, FilesystemGrant, FilesystemPolicy,
    FilesystemScope, NetworkManifest, NetworkPolicy, PlatformContext, PlatformDiagnostic,
    PlatformOperation, PlatformSecretManifest, PlatformSecretPolicy, RuntimeAvailability,
};

#[test]
fn capability_table_denies_missing_capability() {
    let table = CapabilityTable::new([]);

    let error = table
        .ensure_allowed(
            "network.fetch",
            PlatformOperation::NetworkRequest,
            PlatformContext::Desktop,
        )
        .expect_err("missing capability must be denied");

    assert_eq!(
        error.diagnostic,
        PlatformDiagnostic::error(
            "capability.missing",
            "platform capability is not declared: network.fetch"
        )
    );
}

#[test]
fn capability_table_denies_plugin_incompatible_capability() {
    let table = CapabilityTable::new([CapabilityRecord::new("clipboard.write")
        .allow(PlatformOperation::ClipboardWrite)
        .schemas(CapabilitySchema::new(
            "ClipboardWriteInput",
            "()",
            "PlatformError",
        ))
        .availability(RuntimeAvailability::Runtime)
        .desktop(true)
        .plugin(false)]);

    let error = table
        .ensure_allowed(
            "clipboard.write",
            PlatformOperation::ClipboardWrite,
            PlatformContext::Plugin,
        )
        .expect_err("plugin-incompatible capability must be denied");

    assert_eq!(
        error.diagnostic,
        PlatformDiagnostic::error(
            "capability.plugin-incompatible",
            "platform capability is not available in plugin context: clipboard.write"
        )
    );
}

#[test]
fn capability_table_records_schema_availability_and_operations() {
    let record = CapabilityRecord::new("filesystem.read")
        .allow(PlatformOperation::FilesystemRead)
        .deny(PlatformOperation::FilesystemWrite)
        .schemas(CapabilitySchema::new(
            "FilesystemReadInput",
            "FilesystemReadOutput",
            "PlatformError",
        ))
        .availability(RuntimeAvailability::Runtime)
        .desktop(true)
        .plugin(true);

    assert_eq!(record.manifest_key, "filesystem.read");
    assert!(
        record
            .allowed_operations
            .contains(&PlatformOperation::FilesystemRead)
    );
    assert!(
        record
            .denied_operations
            .contains(&PlatformOperation::FilesystemWrite)
    );
    assert_eq!(record.schema.input, "FilesystemReadInput");
    assert_eq!(record.runtime_availability, RuntimeAvailability::Runtime);
    assert!(record.desktop_applicable);
    assert!(record.plugin_applicable);
}

#[test]
fn filesystem_scope_rejects_path_escaping() {
    let grant = FilesystemGrant::new(FilesystemScope::ProjectAssets, "/app/assets");

    let error = FilesystemPolicy::resolve(&grant, "../secrets.txt")
        .expect_err("path escape must be denied");

    assert_eq!(
        error.diagnostic,
        PlatformDiagnostic::error(
            "filesystem.path.escape",
            "filesystem path escapes its scope"
        )
    );
}

#[test]
fn filesystem_scope_rejects_forbidden_paths() {
    let grant = FilesystemGrant::new(FilesystemScope::Forbidden, "/");

    let error = FilesystemPolicy::resolve(&grant, "etc/passwd")
        .expect_err("forbidden path scope must be denied");

    assert_eq!(
        error.diagnostic,
        PlatformDiagnostic::error("filesystem.path.forbidden", "filesystem path is forbidden")
    );
}

#[test]
fn filesystem_scope_rejects_structurally_unsafe_roots() {
    for root in ["relative/root", "/app/../secrets", "C:\\app\\data"] {
        let grant = FilesystemGrant::new(FilesystemScope::AppData, root);

        let error = FilesystemPolicy::resolve(&grant, "settings.json")
            .expect_err("unsafe grant root must be denied");

        assert_eq!(
            error.diagnostic,
            PlatformDiagnostic::error(
                "filesystem.root.invalid",
                "filesystem grant root is invalid"
            )
        );
    }
}

#[test]
fn filesystem_scope_allows_user_selected_file_grants() {
    let grant = FilesystemPolicy::user_selected_file("/home/user/session.hawk");

    let access = FilesystemPolicy::resolve_user_selected(&grant, "/home/user/session.hawk")
        .expect("exact user-selected file grant must be allowed");

    assert_eq!(access.scope, FilesystemScope::UserSelectedFile);
    assert_eq!(access.resolved_path, "/home/user/session.hawk");
}

#[test]
fn network_capabilities_allow_declared_host() {
    let table = CapabilityTable::new([CapabilityRecord::new("network.fetch")
        .allow(PlatformOperation::NetworkRequest)
        .availability(RuntimeAvailability::Runtime)
        .desktop(true)
        .plugin(true)]);
    let manifest = NetworkManifest::new("network.fetch", ["api.hawk2ui.dev"]);

    let request = NetworkPolicy::request(
        &table,
        &manifest,
        "https://api.hawk2ui.dev/v1/status",
        PlatformContext::Desktop,
    )
    .expect("declared host must be allowed");

    assert_eq!(request.host, "api.hawk2ui.dev");
}

#[test]
fn network_capabilities_deny_undeclared_host_malformed_url_and_missing_capability() {
    let table = CapabilityTable::new([CapabilityRecord::new("network.fetch")
        .allow(PlatformOperation::NetworkRequest)
        .availability(RuntimeAvailability::Runtime)
        .desktop(true)
        .plugin(true)]);
    let manifest = NetworkManifest::new("network.fetch", ["api.hawk2ui.dev"]);

    let denied_host = NetworkPolicy::request(
        &table,
        &manifest,
        "https://evil.example/v1/status",
        PlatformContext::Desktop,
    )
    .expect_err("undeclared host must be denied");
    let malformed =
        NetworkPolicy::request(&table, &manifest, "not a url", PlatformContext::Desktop)
            .expect_err("malformed URL must be denied");
    let missing_capability = NetworkPolicy::request(
        &CapabilityTable::new([]),
        &manifest,
        "https://api.hawk2ui.dev/v1/status",
        PlatformContext::Desktop,
    )
    .expect_err("missing network capability must be denied");

    assert_eq!(denied_host.diagnostic.rule, "network.host.denied");
    assert_eq!(malformed.diagnostic.rule, "network.url.malformed");
    assert_eq!(missing_capability.diagnostic.rule, "capability.missing");
}

#[test]
fn clipboard_capabilities_allow_text() {
    let table = CapabilityTable::new([CapabilityRecord::new("clipboard.write")
        .allow(PlatformOperation::ClipboardWrite)
        .availability(RuntimeAvailability::Runtime)
        .desktop(true)
        .plugin(true)]);
    let manifest =
        ClipboardManifest::new("clipboard.write", [ClipboardDataType::Text]).plugin(true);

    let access = ClipboardPolicy::access(
        &table,
        &manifest,
        ClipboardDataType::Text,
        PlatformOperation::ClipboardWrite,
        PlatformContext::Desktop,
    )
    .expect("text clipboard write must be allowed");

    assert_eq!(access.data_type, ClipboardDataType::Text);
}

#[test]
fn clipboard_capabilities_deny_unsupported_image_missing_capability_and_plugin_context() {
    let table = CapabilityTable::new([CapabilityRecord::new("clipboard.write")
        .allow(PlatformOperation::ClipboardWrite)
        .availability(RuntimeAvailability::Runtime)
        .desktop(true)
        .plugin(true)]);
    let manifest = ClipboardManifest::new("clipboard.write", [ClipboardDataType::Text]);

    let unsupported_image = ClipboardPolicy::access(
        &table,
        &manifest,
        ClipboardDataType::Image,
        PlatformOperation::ClipboardWrite,
        PlatformContext::Desktop,
    )
    .expect_err("unsupported image clipboard type must be denied");
    let missing_capability = ClipboardPolicy::access(
        &CapabilityTable::new([]),
        &manifest,
        ClipboardDataType::Text,
        PlatformOperation::ClipboardWrite,
        PlatformContext::Desktop,
    )
    .expect_err("missing clipboard capability must be denied");
    let plugin_denied = ClipboardPolicy::access(
        &table,
        &manifest,
        ClipboardDataType::Text,
        PlatformOperation::ClipboardWrite,
        PlatformContext::Plugin,
    )
    .expect_err("plugin clipboard access must require manifest opt-in");

    assert_eq!(
        unsupported_image.diagnostic.rule,
        "clipboard.type.unsupported"
    );
    assert_eq!(missing_capability.diagnostic.rule, "capability.missing");
    assert_eq!(plugin_denied.diagnostic.rule, "clipboard.plugin.denied");
}

#[test]
fn secrets_database_redacts_secret_values_and_denies_missing_declarations() {
    let manifest = PlatformSecretManifest::new(["api-token"]);

    let handle = PlatformSecretPolicy::read(&manifest, "api-token", "super-secret-value")
        .expect("declared secret must produce a redacted handle");
    let missing = PlatformSecretPolicy::read(&manifest, "missing-token", "unused")
        .expect_err("undeclared secret must be denied");

    assert!(!format!("{handle:?}").contains("super-secret-value"));
    assert_eq!(handle.redacted(), "[REDACTED:api-token]");
    assert_eq!(missing.diagnostic.rule, "secret.declaration.missing");
}

#[test]
fn secrets_policy_denies_structurally_invalid_secret_keys() {
    let manifest = PlatformSecretManifest::new(["api-token", ""]);

    for key in ["", "api token", "api/token", "api\ntoken", "api\0token"] {
        let error = PlatformSecretPolicy::read(&manifest, key, "unused")
            .expect_err("invalid secret key must be denied");

        assert_eq!(error.diagnostic.rule, "secret.key.invalid");
    }
}

#[test]
fn secrets_database_enforces_migration_ordering_and_safe_storage_paths() {
    DatabasePolicy::validate_migrations(&[
        DatabaseMigration::new(1, "create_settings"),
        DatabaseMigration::new(2, "add_presets"),
    ])
    .expect("ordered migrations must be valid");

    let ordering_error = DatabasePolicy::validate_migrations(&[
        DatabaseMigration::new(2, "add_presets"),
        DatabaseMigration::new(1, "create_settings"),
    ])
    .expect_err("out-of-order migrations must fail");
    let storage_error = DatabasePolicy::validate_storage_path(
        &FilesystemGrant::new(FilesystemScope::Forbidden, "/"),
        "state.sqlite",
    )
    .expect_err("forbidden storage scope must fail");

    assert_eq!(ordering_error.diagnostic.rule, "database.migration.order");
    assert_eq!(storage_error.diagnostic.rule, "database.storage.unsafe");
}

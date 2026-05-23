use hawk2ui_platform::{
    CapabilityRecord, CapabilitySchema, CapabilityTable, FilesystemGrant, FilesystemPolicy,
    FilesystemScope, NetworkManifest, NetworkPolicy, PlatformContext, PlatformDiagnostic,
    PlatformOperation, RuntimeAvailability,
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

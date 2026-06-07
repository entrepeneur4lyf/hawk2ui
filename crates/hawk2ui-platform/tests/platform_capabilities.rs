use hawk2ui_platform::{
    AiManifest, AiPolicy, AudioCueBinding, AudioManifest, AudioPlaybackSink, AudioPolicy,
    CapabilityRecord, CapabilitySchema, CapabilityTable, ClipboardDataType, ClipboardManifest,
    ClipboardPolicy, DatabaseManifest, DatabaseMigration, DatabasePolicy, DialogKind,
    DialogManifest, DialogPolicy, FilesystemGrant, FilesystemLocalizationHostBackend,
    FilesystemPolicy, FilesystemScope, GlobalShortcutSink, HostCapabilityRouter,
    HttpProviderHostBackend, LocalizationManifest, LocalizationPolicy, McpManifest, McpPolicy,
    NetworkManifest, NetworkPolicy, NetworkResponsePayload, NotificationBinding,
    NotificationManifest, NotificationPolicy, NotificationSink, PlatformBackends, PlatformContext,
    PlatformDiagnostic, PlatformHostBackend, PlatformOperation, PlatformSecretManifest,
    PlatformSecretPolicy, RuntimeAvailability, ShortcutBinding, ShortcutManifest, ShortcutPolicy,
    StaticNetworkBackend,
};
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
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
fn capability_records_generate_and_validate_json_schema() {
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

    let record_schema =
        CapabilityRecord::json_schema().expect("capability record schema generates");
    let table_schema = CapabilityTable::json_schema().expect("capability table schema generates");
    let table_value = serde_json::to_value(&table).expect("capability table serializes");

    CapabilityTable::validate_json(&table_value)
        .expect("serialized capability table validates against generated schema");
    assert_eq!(record_schema["title"], "CapabilityRecord");
    assert_eq!(table_schema["title"], "CapabilityTable");
    assert!(table_schema["properties"]["records"].is_object());

    let mut invalid = table_value;
    invalid["unexpected"] = serde_json::json!(true);
    let error = CapabilityTable::validate_json(&invalid)
        .expect_err("unknown capability table fields fail schema validation");
    assert_eq!(error.rule, "capability.schema.table.invalid");
}

#[test]
fn capability_table_preserves_first_duplicate_declaration() {
    let table = CapabilityTable::new([
        CapabilityRecord::new("filesystem.read")
            .deny(PlatformOperation::FilesystemRead)
            .availability(RuntimeAvailability::Runtime)
            .desktop(true)
            .plugin(true),
        CapabilityRecord::new("filesystem.read")
            .allow(PlatformOperation::FilesystemRead)
            .availability(RuntimeAvailability::Runtime)
            .desktop(true)
            .plugin(true),
    ]);

    let error = table
        .ensure_allowed(
            "filesystem.read",
            PlatformOperation::FilesystemRead,
            PlatformContext::Desktop,
        )
        .expect_err("later duplicate capability declarations must not override the first");

    assert_eq!(error.diagnostic.rule, "capability.operation-denied");
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

#[cfg(unix)]
#[test]
fn filesystem_scope_rejects_symlink_escape_from_existing_scope() {
    let temp = temp_platform_dir("filesystem-symlink-escape");
    let assets = temp.join("assets");
    let secrets = temp.join("secrets");
    fs::create_dir_all(&assets).expect("assets directory is created");
    fs::create_dir_all(&secrets).expect("secrets directory is created");
    fs::write(secrets.join("token.txt"), "secret").expect("secret fixture is written");
    std::os::unix::fs::symlink(&secrets, assets.join("linked-secrets"))
        .expect("symlink fixture is created");
    let grant = FilesystemGrant::new(FilesystemScope::ProjectAssets, path_string(&assets));

    let error = FilesystemPolicy::resolve(&grant, "linked-secrets/token.txt")
        .expect_err("symlink escape must be denied after canonicalization");

    assert_eq!(error.diagnostic.rule, "filesystem.path.escape");
}

#[test]
fn filesystem_scope_canonicalizes_existing_roots_and_targets() {
    let temp = temp_platform_dir("filesystem-canonical-access");
    let assets = temp.join("assets");
    fs::create_dir_all(assets.join("images")).expect("asset directory is created");
    fs::write(assets.join("images/logo.svg"), "<svg />").expect("asset fixture is written");
    let grant = FilesystemGrant::new(FilesystemScope::ProjectAssets, path_string(&assets));

    let access = FilesystemPolicy::resolve(&grant, "images/logo.svg")
        .expect("existing scoped asset should resolve");

    assert_eq!(
        access.resolved_path,
        path_string(&assets.join("images/logo.svg"))
    );
}

#[test]
fn filesystem_scope_rejects_uncanonicalizable_root() {
    // A scope root that does not exist on disk cannot be canonicalized, so containment
    // (`resolved.starts_with(root_canonical)`) cannot be verified. The resolver must fail closed
    // rather than return an unchecked `root.join(..)` (the previous silent fallback).
    let temp = temp_platform_dir("filesystem-missing-root");
    let missing_root = temp.join("does-not-exist");
    let grant = FilesystemGrant::new(FilesystemScope::ProjectAssets, path_string(&missing_root));

    let error = FilesystemPolicy::resolve(&grant, "config.toml")
        .expect_err("an uncanonicalizable scope root must be denied, not resolved unchecked");

    assert_eq!(error.diagnostic.rule, "filesystem.path.escape");
}

#[test]
fn filesystem_scope_rejects_structurally_unsafe_user_selected_grants() {
    for path in [
        "relative/session.hawk",
        "/home/user/../secrets",
        "C:\\\\Users\\\\session.hawk",
    ] {
        let grant = FilesystemPolicy::user_selected_file(path);

        let error = FilesystemPolicy::resolve_user_selected(&grant, path)
            .expect_err("unsafe exact user-selected grants must be denied");

        assert_eq!(error.diagnostic.rule, "filesystem.user-grant.invalid");
    }
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
fn network_capabilities_reject_invalid_manifest_hosts() {
    let table = CapabilityTable::new([CapabilityRecord::new("network.fetch")
        .allow(PlatformOperation::NetworkRequest)
        .availability(RuntimeAvailability::Runtime)
        .desktop(true)
        .plugin(true)]);

    for allowed_hosts in [
        vec![],
        vec!["api.hawk2ui.dev", "bad host"],
        vec!["api.hawk2ui.dev", "API.HAWK2UI.DEV"],
    ] {
        let manifest = NetworkManifest::new("network.fetch", allowed_hosts);
        let error = NetworkPolicy::request(
            &table,
            &manifest,
            "https://api.hawk2ui.dev/v1/status",
            PlatformContext::Desktop,
        )
        .expect_err("invalid network manifest hosts must be rejected");

        assert_eq!(error.diagnostic.rule, "network.manifest.invalid-hosts");
    }
}

#[test]
fn network_capabilities_canonicalize_idna_hosts_and_reject_userinfo_ports_and_fragments() {
    let table = CapabilityTable::new([CapabilityRecord::new("network.fetch")
        .allow(PlatformOperation::NetworkRequest)
        .availability(RuntimeAvailability::Runtime)
        .desktop(true)
        .plugin(true)]);
    let manifest = NetworkManifest::new("network.fetch", ["bücher.example"]);

    let request = NetworkPolicy::request(
        &table,
        &manifest,
        "https://BÜCHER.example/catalog",
        PlatformContext::Desktop,
    )
    .expect("IDNA host should canonicalize consistently");
    let userinfo = NetworkPolicy::request(
        &table,
        &manifest,
        "https://user@bücher.example/catalog",
        PlatformContext::Desktop,
    )
    .expect_err("userinfo must be rejected before host matching");
    let fragment = NetworkPolicy::request(
        &table,
        &manifest,
        "https://bücher.example/catalog#token",
        PlatformContext::Desktop,
    )
    .expect_err("fragments must be rejected because they are not network request material");
    let invalid_manifest = NetworkPolicy::request(
        &table,
        &NetworkManifest::new("network.fetch", ["api.hawk2ui.dev:443"]),
        "https://api.hawk2ui.dev/v1/status",
        PlatformContext::Desktop,
    )
    .expect_err("allowed host entries must not include ports");

    assert_eq!(request.host, "xn--bcher-kva.example");
    assert_eq!(userinfo.diagnostic.rule, "network.url.malformed");
    assert_eq!(fragment.diagnostic.rule, "network.url.malformed");
    assert_eq!(
        invalid_manifest.diagnostic.rule,
        "network.manifest.invalid-hosts"
    );
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

fn temp_platform_dir(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock supports test temp naming")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "hawk2ui-platform-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("temporary platform test directory is created");
    path
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[test]
fn platform_backends_execute_policy_checked_io_network_clipboard_and_secret_store() {
    let root = temp_platform_dir("backend");
    fs::write(root.join("config.txt"), b"cached-config").expect("test config file is written");

    let mut backend = PlatformBackends::new(StaticNetworkBackend::new([(
        "https://api.hawk2ui.dev/v1/config",
        NetworkResponsePayload::text(200, "application/json", r#"{"ok":true}"#),
    )]))
    .with_secret("api-token", "super-secret-value");
    let grant = FilesystemGrant::new(FilesystemScope::AppData, path_string(&root));

    let read = backend
        .read_file(&grant, "config.txt")
        .expect("scoped filesystem read is executed");
    let write = backend
        .write_file(&grant, "cache/output.txt", b"render-cache")
        .expect("scoped filesystem write is executed");

    assert_eq!(read.bytes, b"cached-config");
    assert_eq!(write.bytes_written, b"render-cache".len());
    assert_eq!(
        fs::read(root.join("cache/output.txt")).expect("written file is readable"),
        b"render-cache"
    );

    let capabilities = CapabilityTable::new([
        CapabilityRecord::new("network.fetch")
            .allow(PlatformOperation::NetworkRequest)
            .availability(RuntimeAvailability::Runtime)
            .desktop(true)
            .plugin(false),
        CapabilityRecord::new("clipboard.text")
            .allow(PlatformOperation::ClipboardRead)
            .allow(PlatformOperation::ClipboardWrite)
            .availability(RuntimeAvailability::Runtime)
            .desktop(true)
            .plugin(true),
    ]);
    let network_manifest = NetworkManifest::new("network.fetch", ["api.hawk2ui.dev"]);
    let response = backend
        .network_get(
            &capabilities,
            &network_manifest,
            "https://api.hawk2ui.dev/v1/config",
            PlatformContext::Desktop,
        )
        .expect("declared network request is executed");

    assert_eq!(response.status, 200);
    assert_eq!(response.body, br#"{"ok":true}"#);
    assert_eq!(
        backend.network().requested_urls(),
        &["https://api.hawk2ui.dev/v1/config".to_owned()]
    );

    let denied_network = backend
        .network_get(
            &capabilities,
            &network_manifest,
            "https://evil.example/v1/config",
            PlatformContext::Desktop,
        )
        .expect_err("denied network request must not reach the transport");

    assert_eq!(denied_network.diagnostic.rule, "network.host.denied");
    assert_eq!(
        backend.network().requested_urls(),
        &["https://api.hawk2ui.dev/v1/config".to_owned()]
    );

    let clipboard_manifest =
        ClipboardManifest::new("clipboard.text", [ClipboardDataType::Text]).plugin(true);
    backend
        .write_clipboard(
            &capabilities,
            &clipboard_manifest,
            PlatformContext::Desktop,
            "copied text",
        )
        .expect("clipboard write is policy checked and stored");
    let clipboard = backend
        .read_clipboard(&capabilities, &clipboard_manifest, PlatformContext::Desktop)
        .expect("clipboard read is policy checked and returned");

    assert_eq!(clipboard.text.as_deref(), Some("copied text"));

    let secret_manifest = PlatformSecretManifest::new(["api-token"]);
    let handle = backend
        .read_secret(&secret_manifest, "api-token")
        .expect("declared secret is loaded from the secret store");

    assert_eq!(handle.redacted(), "[REDACTED:api-token]");
    assert!(!format!("{handle:?}").contains("super-secret-value"));
    assert!(handle.is_absent_from("no raw secret here"));
    assert!(!handle.is_absent_from("super-secret-value"));
}

#[test]
fn system_platform_backends_require_explicit_host_backend_for_host_capabilities() {
    let capabilities = CapabilityTable::new([CapabilityRecord::new("clipboard.text")
        .allow(PlatformOperation::ClipboardWrite)
        .availability(RuntimeAvailability::Runtime)
        .desktop(true)
        .plugin(true)]);
    let manifest = ClipboardManifest::new("clipboard.text", [ClipboardDataType::Text]).plugin(true);
    let mut backend = PlatformBackends::system();

    let error = backend
        .write_clipboard(
            &capabilities,
            &manifest,
            PlatformContext::Desktop,
            "copied text",
        )
        .expect_err("system backend must require an explicit clipboard host adapter");

    assert_eq!(error.diagnostic.rule, "platform.host-backend.unsupported");
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
fn clipboard_capabilities_reject_non_clipboard_operations() {
    let table = CapabilityTable::new([CapabilityRecord::new("clipboard.write")
        .allow(PlatformOperation::NetworkRequest)
        .availability(RuntimeAvailability::Runtime)
        .desktop(true)
        .plugin(true)]);
    let manifest = ClipboardManifest::new("clipboard.write", [ClipboardDataType::Text]);

    let error = ClipboardPolicy::access(
        &table,
        &manifest,
        ClipboardDataType::Text,
        PlatformOperation::NetworkRequest,
        PlatformContext::Desktop,
    )
    .expect_err("clipboard policy must only accept clipboard operations");

    assert_eq!(error.diagnostic.rule, "clipboard.operation.invalid");
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

#[test]
fn database_migrations_require_stable_unique_ids() {
    for migrations in [
        vec![DatabaseMigration::new(1, "")],
        vec![DatabaseMigration::new(1, "create settings")],
        vec![DatabaseMigration::new(1, "create/settings")],
        vec![
            DatabaseMigration::new(1, "create_settings"),
            DatabaseMigration::new(2, "create_settings"),
        ],
    ] {
        let error = DatabasePolicy::validate_migrations(&migrations)
            .expect_err("invalid migration IDs must fail");

        assert_eq!(error.diagnostic.rule, "database.migration.id-invalid");
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn extended_platform_domains_enforce_capabilities_and_manifest_allowlists() {
    let capabilities = CapabilityTable::new([
        CapabilityRecord::new("audio.playback")
            .allow(PlatformOperation::AudioPlayback)
            .availability(RuntimeAvailability::Runtime)
            .desktop(true)
            .plugin(true),
        CapabilityRecord::new("ai.provider")
            .allow(PlatformOperation::AiProviderRequest)
            .availability(RuntimeAvailability::Runtime)
            .desktop(true)
            .plugin(false),
        CapabilityRecord::new("mcp.call")
            .allow(PlatformOperation::McpToolCall)
            .availability(RuntimeAvailability::Runtime)
            .desktop(true)
            .plugin(false),
        CapabilityRecord::new("notifications.send")
            .allow(PlatformOperation::NotificationSend)
            .availability(RuntimeAvailability::Runtime)
            .desktop(true)
            .plugin(false),
        CapabilityRecord::new("shortcuts.register")
            .allow(PlatformOperation::GlobalShortcutRegister)
            .availability(RuntimeAvailability::Runtime)
            .desktop(true)
            .plugin(false),
        CapabilityRecord::new("localization.load")
            .allow(PlatformOperation::LocalizationRead)
            .availability(RuntimeAvailability::Runtime)
            .desktop(true)
            .plugin(true),
        CapabilityRecord::new("dialogs.open")
            .allow(PlatformOperation::DialogOpen)
            .availability(RuntimeAvailability::Runtime)
            .desktop(true)
            .plugin(false),
        CapabilityRecord::new("files.pick")
            .allow(PlatformOperation::FilePickerOpen)
            .availability(RuntimeAvailability::Runtime)
            .desktop(true)
            .plugin(false),
    ]);

    assert_eq!(
        AudioPolicy::request(
            &capabilities,
            &AudioManifest::new("audio.playback", ["meter-click"]),
            "meter-click",
            PlatformContext::Plugin,
        )
        .expect("declared plugin-safe cue should be allowed")
        .cue_id,
        "meter-click"
    );
    assert_eq!(
        AiPolicy::request(
            &capabilities,
            &AiManifest::new("ai.provider", ["openai"], ["summarize"]),
            "openai",
            "summarize",
            PlatformContext::Desktop,
        )
        .expect("declared AI provider operation should be allowed")
        .provider_id,
        "openai"
    );
    assert_eq!(
        McpPolicy::call(
            &capabilities,
            &McpManifest::new("mcp.call", ["design-server"], ["snapshot"]),
            "design-server",
            "snapshot",
            PlatformContext::Desktop,
        )
        .expect("declared MCP server tool should be allowed")
        .tool_name,
        "snapshot"
    );
    assert_eq!(
        NotificationPolicy::send(
            &capabilities,
            &NotificationManifest::new("notifications.send", ["build"]),
            "build",
            PlatformContext::Desktop,
        )
        .expect("declared notification channel should be allowed")
        .channel,
        "build"
    );
    assert_eq!(
        ShortcutPolicy::register(
            &capabilities,
            &ShortcutManifest::new("shortcuts.register", ["CommandOrControl+K"]),
            "CommandOrControl+K",
            PlatformContext::Desktop,
        )
        .expect("declared shortcut should be allowed")
        .accelerator,
        "CommandOrControl+K"
    );
    assert_eq!(
        LocalizationPolicy::load(
            &capabilities,
            &LocalizationManifest::new("localization.load", ["en-US", "fr-FR"]),
            "fr-FR",
            PlatformContext::Plugin,
        )
        .expect("declared locale should be allowed")
        .locale,
        "fr-FR"
    );
    assert_eq!(
        DialogPolicy::open(
            &capabilities,
            &DialogManifest::new("dialogs.open", [DialogKind::Message]),
            DialogKind::Message,
            PlatformContext::Desktop,
        )
        .expect("declared dialog kind should be allowed")
        .kind,
        DialogKind::Message
    );
    assert_eq!(
        DialogPolicy::file_picker(
            &capabilities,
            &DialogManifest::new("files.pick", [DialogKind::FilePicker]),
            PlatformContext::Desktop,
        )
        .expect("declared file picker should be allowed")
        .kind,
        DialogKind::FilePicker
    );

    assert_eq!(
        AudioPolicy::request(
            &capabilities,
            &AudioManifest::new("audio.playback", ["meter-click"]),
            "explosion",
            PlatformContext::Plugin,
        )
        .expect_err("undeclared audio cue must be denied")
        .diagnostic
        .rule,
        "audio.cue.denied"
    );
    assert_eq!(
        AiPolicy::request(
            &capabilities,
            &AiManifest::new("ai.provider", ["openai"], ["summarize"]),
            "evil-ai",
            "summarize",
            PlatformContext::Desktop,
        )
        .expect_err("undeclared AI provider must be denied")
        .diagnostic
        .rule,
        "ai.provider.denied"
    );
    assert_eq!(
        McpPolicy::call(
            &capabilities,
            &McpManifest::new("mcp.call", ["design-server"], ["snapshot"]),
            "design-server",
            "delete_all",
            PlatformContext::Desktop,
        )
        .expect_err("undeclared MCP tool must be denied")
        .diagnostic
        .rule,
        "mcp.tool.denied"
    );
    assert_eq!(
        NotificationPolicy::send(
            &capabilities,
            &NotificationManifest::new("notifications.send", ["build"]),
            "marketing",
            PlatformContext::Desktop,
        )
        .expect_err("undeclared notification channel must be denied")
        .diagnostic
        .rule,
        "notification.channel.denied"
    );
    assert_eq!(
        ShortcutPolicy::register(
            &capabilities,
            &ShortcutManifest::new("shortcuts.register", ["CommandOrControl+K"]),
            "CommandOrControl+Q",
            PlatformContext::Desktop,
        )
        .expect_err("undeclared shortcut accelerator must be denied")
        .diagnostic
        .rule,
        "shortcut.accelerator.denied"
    );
    assert_eq!(
        LocalizationPolicy::load(
            &capabilities,
            &LocalizationManifest::new("localization.load", ["en-US", "fr-FR"]),
            "de-DE",
            PlatformContext::Plugin,
        )
        .expect_err("undeclared locale must be denied")
        .diagnostic
        .rule,
        "localization.locale.denied"
    );
    assert_eq!(
        DialogPolicy::open(
            &capabilities,
            &DialogManifest::new("dialogs.open", [DialogKind::Message]),
            DialogKind::FilePicker,
            PlatformContext::Desktop,
        )
        .expect_err("undeclared dialog kind must be denied")
        .diagnostic
        .rule,
        "dialog.kind.denied"
    );
    assert_eq!(
        DialogPolicy::file_picker(
            &CapabilityTable::new([]),
            &DialogManifest::new("files.pick", [DialogKind::FilePicker]),
            PlatformContext::Desktop,
        )
        .expect_err("missing file picker capability must be denied")
        .diagnostic
        .rule,
        "capability.missing"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn platform_backends_execute_all_policy_checked_extended_domains() {
    let database_dir = unique_temp_dir("hawk2ui-platform-database-backend");
    fs::create_dir_all(&database_dir).expect("temporary database directory is created");
    let database_grant = FilesystemGrant::new(
        FilesystemScope::AppData,
        database_dir.to_string_lossy().into_owned(),
    );
    let capabilities = CapabilityTable::new([
        CapabilityRecord::new("audio.playback")
            .allow(PlatformOperation::AudioPlayback)
            .availability(RuntimeAvailability::Runtime)
            .desktop(true)
            .plugin(true),
        CapabilityRecord::new("ai.provider")
            .allow(PlatformOperation::AiProviderRequest)
            .availability(RuntimeAvailability::Runtime)
            .desktop(true)
            .plugin(false),
        CapabilityRecord::new("mcp.call")
            .allow(PlatformOperation::McpToolCall)
            .availability(RuntimeAvailability::Runtime)
            .desktop(true)
            .plugin(false),
        CapabilityRecord::new("notifications.send")
            .allow(PlatformOperation::NotificationSend)
            .availability(RuntimeAvailability::Runtime)
            .desktop(true)
            .plugin(false),
        CapabilityRecord::new("shortcuts.register")
            .allow(PlatformOperation::GlobalShortcutRegister)
            .availability(RuntimeAvailability::Runtime)
            .desktop(true)
            .plugin(false),
        CapabilityRecord::new("localization.load")
            .allow(PlatformOperation::LocalizationRead)
            .availability(RuntimeAvailability::Runtime)
            .desktop(true)
            .plugin(true),
        CapabilityRecord::new("dialogs.open")
            .allow(PlatformOperation::DialogOpen)
            .allow(PlatformOperation::FilePickerOpen)
            .availability(RuntimeAvailability::Runtime)
            .desktop(true)
            .plugin(false),
        CapabilityRecord::new("database.local")
            .allow(PlatformOperation::DatabaseMigration)
            .allow(PlatformOperation::DatabaseQuery)
            .availability(RuntimeAvailability::Runtime)
            .desktop(true)
            .plugin(true),
    ]);

    let mut backend = PlatformBackends::new(StaticNetworkBackend::default())
        .with_ai_text_response(
            "openai",
            "summarize",
            "application/json",
            r#"{"summary":"ok"}"#,
        )
        .with_mcp_text_response(
            "design-server",
            "snapshot",
            "application/json",
            r#"{"nodes":2}"#,
        )
        .with_localization_text_bundle("en-US", "application/json", r#"{"hello":"Hello"}"#)
        .with_dialog_response(DialogKind::Message, true, std::iter::empty::<&str>())
        .with_dialog_response(DialogKind::FilePicker, true, ["preset.hawk"]);

    let audio = backend
        .play_audio(
            &capabilities,
            &AudioManifest::new("audio.playback", ["meter-click"]),
            "meter-click",
            PlatformContext::Plugin,
        )
        .expect("audio playback executes after policy approval");
    let ai = backend
        .ai_request(
            &capabilities,
            &AiManifest::new("ai.provider", ["openai"], ["summarize"]),
            "openai",
            "summarize",
            PlatformContext::Desktop,
        )
        .expect("AI request executes after policy approval");
    let mcp = backend
        .call_mcp(
            &capabilities,
            &McpManifest::new("mcp.call", ["design-server"], ["snapshot"]),
            "design-server",
            "snapshot",
            PlatformContext::Desktop,
        )
        .expect("MCP call executes after policy approval");
    let localization = backend
        .load_localization(
            &capabilities,
            &LocalizationManifest::new("localization.load", ["en-US"]),
            "en-US",
            PlatformContext::Plugin,
        )
        .expect("localization bundle loads after policy approval");
    let dialog = backend
        .open_dialog(
            &capabilities,
            &DialogManifest::new(
                "dialogs.open",
                [DialogKind::Message, DialogKind::FilePicker],
            ),
            DialogKind::Message,
            PlatformContext::Desktop,
        )
        .expect("dialog opens after policy approval");
    let picker = backend
        .open_file_picker(
            &capabilities,
            &DialogManifest::new(
                "dialogs.open",
                [DialogKind::Message, DialogKind::FilePicker],
            ),
            PlatformContext::Desktop,
        )
        .expect("file picker opens after policy approval");
    let notification = backend
        .send_notification(
            &capabilities,
            &NotificationManifest::new("notifications.send", ["build"]),
            "build",
            PlatformContext::Desktop,
        )
        .expect("notification sends after policy approval");
    let shortcut = backend
        .register_shortcut(
            &capabilities,
            &ShortcutManifest::new("shortcuts.register", ["CommandOrControl+K"]),
            "CommandOrControl+K",
            PlatformContext::Desktop,
        )
        .expect("shortcut registers after policy approval");

    let database_manifest = DatabaseManifest::new(
        "database.local",
        database_grant.clone(),
        "state.json",
        [
            DatabaseMigration::new(1, "create_settings"),
            DatabaseMigration::new(2, "add_presets"),
        ],
    );
    let migration = backend
        .migrate_database(&capabilities, &database_manifest, PlatformContext::Plugin)
        .expect("database migrations execute after policy approval");
    let written = backend
        .put_database_value(
            &capabilities,
            &database_manifest,
            "theme",
            serde_json::json!("carbon"),
            PlatformContext::Plugin,
        )
        .expect("database writes execute after policy approval");
    let loaded = backend
        .get_database_value(
            &capabilities,
            &database_manifest,
            "theme",
            PlatformContext::Plugin,
        )
        .expect("database reads execute after policy approval");
    let transaction = backend
        .commit_database_transaction(
            &capabilities,
            &database_manifest,
            [
                ("accent".to_owned(), serde_json::json!("ember")),
                ("density".to_owned(), serde_json::json!("compact")),
            ],
            PlatformContext::Plugin,
        )
        .expect("database transactions execute atomically after policy approval");
    let invalid_transaction = backend
        .commit_database_transaction(
            &capabilities,
            &database_manifest,
            [
                ("safe".to_owned(), serde_json::json!(true)),
                ("unsafe/key".to_owned(), serde_json::json!(false)),
            ],
            PlatformContext::Plugin,
        )
        .expect_err("invalid transaction keys must reject the whole transaction");
    let safe_after_rejected_transaction = backend
        .get_database_value(
            &capabilities,
            &database_manifest,
            "safe",
            PlatformContext::Plugin,
        )
        .expect("database remains queryable after rejected transaction");

    assert_eq!(audio.request.cue_id, "meter-click");
    assert_eq!(backend.played_audio_cues(), ["meter-click"]);
    assert_eq!(ai.request.provider_id, "openai");
    assert_eq!(ai.content_type.as_deref(), Some("application/json"));
    assert_eq!(ai.body, br#"{"summary":"ok"}"#);
    assert_eq!(mcp.request.tool_name, "snapshot");
    assert_eq!(mcp.body, br#"{"nodes":2}"#);
    assert_eq!(localization.request.locale, "en-US");
    assert_eq!(localization.body, br#"{"hello":"Hello"}"#);
    assert!(dialog.accepted);
    assert_eq!(picker.selected_paths, ["preset.hawk"]);
    assert_eq!(notification.request.channel, "build");
    assert_eq!(backend.sent_notification_channels(), ["build"]);
    assert_eq!(shortcut.registration.accelerator, "CommandOrControl+K");
    assert_eq!(backend.registered_shortcuts(), ["CommandOrControl+K"]);
    assert_eq!(
        migration.applied_migrations,
        [
            DatabaseMigration::new(1, "create_settings"),
            DatabaseMigration::new(2, "add_presets"),
        ]
    );
    assert_eq!(written.value, serde_json::json!("carbon"));
    assert_eq!(loaded.value, Some(serde_json::json!("carbon")));
    assert_eq!(transaction.written_keys, ["accent", "density"]);
    assert_eq!(invalid_transaction.diagnostic.rule, "database.key.invalid");
    assert_eq!(safe_after_rejected_transaction.value, None);

    let before_denied_ai = backend.ai_requests().len();
    let denied_ai = backend
        .ai_request(
            &capabilities,
            &AiManifest::new("ai.provider", ["openai"], ["summarize"]),
            "evil-ai",
            "summarize",
            PlatformContext::Desktop,
        )
        .expect_err("denied AI requests must not reach backend execution");

    assert_eq!(denied_ai.diagnostic.rule, "ai.provider.denied");
    assert_eq!(backend.ai_requests().len(), before_denied_ai);

    fs::remove_dir_all(database_dir).expect("temporary database directory is removed");
}

#[derive(Debug, Default)]
struct ExplicitHostBackend {
    ai_calls: usize,
    dialogs: usize,
    notifications: usize,
}

impl PlatformHostBackend for ExplicitHostBackend {
    fn request_ai(
        &mut self,
        request: &hawk2ui_platform::AiProviderRequest,
    ) -> Result<hawk2ui_platform::HostDataPayload, hawk2ui_platform::PlatformBackendError> {
        self.ai_calls += 1;
        Ok(hawk2ui_platform::HostDataPayload::text(
            "application/json",
            format!(
                r#"{{"provider":"{}","operation":"{}"}}"#,
                request.provider_id, request.operation
            ),
        ))
    }

    fn open_dialog(
        &mut self,
        request: &hawk2ui_platform::DialogRequest,
    ) -> Result<hawk2ui_platform::HostDialogResponse, hawk2ui_platform::PlatformBackendError> {
        self.dialogs += 1;
        Ok(hawk2ui_platform::HostDialogResponse::accepted(
            request.kind,
            ["chosen.hawk"],
        ))
    }

    fn send_notification(
        &mut self,
        _request: &hawk2ui_platform::NotificationRequest,
    ) -> Result<(), hawk2ui_platform::PlatformBackendError> {
        self.notifications += 1;
        Ok(())
    }
}

#[test]
fn platform_backends_delegate_extended_domains_to_explicit_host_backend() {
    let capabilities = CapabilityTable::new([
        CapabilityRecord::new("ai.provider")
            .allow(PlatformOperation::AiProviderRequest)
            .availability(RuntimeAvailability::Runtime)
            .desktop(true)
            .plugin(false),
        CapabilityRecord::new("dialogs.open")
            .allow(PlatformOperation::DialogOpen)
            .availability(RuntimeAvailability::Runtime)
            .desktop(true)
            .plugin(false),
        CapabilityRecord::new("notifications.send")
            .allow(PlatformOperation::NotificationSend)
            .availability(RuntimeAvailability::Runtime)
            .desktop(true)
            .plugin(false),
    ]);
    let mut backend = PlatformBackends::with_host(
        StaticNetworkBackend::default(),
        ExplicitHostBackend::default(),
    );

    let ai = backend
        .ai_request(
            &capabilities,
            &AiManifest::new("ai.provider", ["openai"], ["summarize"]),
            "openai",
            "summarize",
            PlatformContext::Desktop,
        )
        .expect("AI request delegates to host backend after policy approval");
    let dialog = backend
        .open_dialog(
            &capabilities,
            &DialogManifest::new("dialogs.open", [DialogKind::Message]),
            DialogKind::Message,
            PlatformContext::Desktop,
        )
        .expect("dialog delegates to host backend after policy approval");
    backend
        .send_notification(
            &capabilities,
            &NotificationManifest::new("notifications.send", ["build"]),
            "build",
            PlatformContext::Desktop,
        )
        .expect("notification delegates to host backend after policy approval");

    assert_eq!(ai.body, br#"{"provider":"openai","operation":"summarize"}"#);
    assert_eq!(dialog.selected_paths, ["chosen.hawk"]);
    assert_eq!(backend.host().ai_calls, 1);
    assert_eq!(backend.host().dialogs, 1);
    assert_eq!(backend.host().notifications, 1);
}

#[test]
fn filesystem_localization_host_loads_policy_approved_locale_bundle() {
    let locale_root = unique_temp_dir("hawk2ui-platform-localization-backend");
    fs::create_dir_all(&locale_root).expect("temporary locale directory is created");
    fs::write(
        locale_root.join("en-US.json"),
        br#"{"title":"Hawk2UI","status":"ready"}"#,
    )
    .expect("locale fixture is written");
    let capabilities = CapabilityTable::new([CapabilityRecord::new("localization.load")
        .allow(PlatformOperation::LocalizationRead)
        .availability(RuntimeAvailability::Runtime)
        .desktop(true)
        .plugin(true)]);
    let manifest = LocalizationManifest::new("localization.load", ["en-US"]);
    let mut backend = PlatformBackends::with_host(
        StaticNetworkBackend::default(),
        FilesystemLocalizationHostBackend::new(&locale_root),
    );

    let result = backend
        .load_localization(&capabilities, &manifest, "en-US", PlatformContext::Desktop)
        .expect("policy-approved locale bundle loads from filesystem host backend");

    assert_eq!(result.request.locale, "en-US");
    assert_eq!(result.content_type.as_deref(), Some("application/json"));
    assert_eq!(
        result.body,
        br#"{"title":"Hawk2UI","status":"ready"}"#.to_vec()
    );
}

#[test]
fn http_provider_host_executes_policy_approved_ai_and_mcp_requests() {
    let ai_url = serve_http_once(r#"{"provider":"openai","operation":"summarize"}"#);
    let mcp_url = serve_http_once(r#"{"server":"local","tool":"ping"}"#);
    let capabilities = CapabilityTable::new([
        CapabilityRecord::new("ai.provider")
            .allow(PlatformOperation::AiProviderRequest)
            .availability(RuntimeAvailability::Runtime)
            .desktop(true)
            .plugin(false),
        CapabilityRecord::new("mcp.call")
            .allow(PlatformOperation::McpToolCall)
            .availability(RuntimeAvailability::Runtime)
            .desktop(true)
            .plugin(false),
    ]);
    let mut backend = PlatformBackends::with_host(
        StaticNetworkBackend::default(),
        HttpProviderHostBackend::new(Duration::from_secs(5))
            .with_ai_endpoint("openai", "summarize", ai_url)
            .with_mcp_endpoint("local", "ping", mcp_url),
    );

    let ai = backend
        .ai_request(
            &capabilities,
            &AiManifest::new("ai.provider", ["openai"], ["summarize"]),
            "openai",
            "summarize",
            PlatformContext::Desktop,
        )
        .expect("AI request executes through explicit HTTP endpoint");
    let mcp = backend
        .call_mcp(
            &capabilities,
            &McpManifest::new("mcp.call", ["local"], ["ping"]),
            "local",
            "ping",
            PlatformContext::Desktop,
        )
        .expect("MCP tool call executes through explicit HTTP endpoint");

    assert_eq!(ai.content_type.as_deref(), Some("application/json"));
    assert_eq!(
        ai.body,
        br#"{"provider":"openai","operation":"summarize"}"#.to_vec()
    );
    assert_eq!(mcp.content_type.as_deref(), Some("application/json"));
    assert_eq!(mcp.body, br#"{"server":"local","tool":"ping"}"#.to_vec());
}

#[derive(Debug, Default)]
struct RecordingAudioSink {
    played: Vec<AudioCueBinding>,
}

impl AudioPlaybackSink for RecordingAudioSink {
    fn play_audio_cue(
        &mut self,
        binding: &AudioCueBinding,
    ) -> Result<(), hawk2ui_platform::PlatformBackendError> {
        self.played.push(binding.clone());
        Ok(())
    }
}

#[derive(Debug, Default)]
struct RecordingNotificationSink {
    sent: Vec<NotificationBinding>,
}

impl NotificationSink for RecordingNotificationSink {
    fn send_notification(
        &mut self,
        binding: &NotificationBinding,
    ) -> Result<(), hawk2ui_platform::PlatformBackendError> {
        self.sent.push(binding.clone());
        Ok(())
    }
}

#[derive(Debug, Default)]
struct RecordingShortcutSink {
    registered: Vec<ShortcutBinding>,
}

impl GlobalShortcutSink for RecordingShortcutSink {
    fn register_shortcut(
        &mut self,
        binding: &ShortcutBinding,
    ) -> Result<(), hawk2ui_platform::PlatformBackendError> {
        self.registered.push(binding.clone());
        Ok(())
    }
}

#[test]
fn routed_host_backend_executes_mapped_audio_notification_and_shortcut_adapters() {
    let capabilities = CapabilityTable::new([
        CapabilityRecord::new("audio.playback")
            .allow(PlatformOperation::AudioPlayback)
            .availability(RuntimeAvailability::Runtime)
            .desktop(true)
            .plugin(true),
        CapabilityRecord::new("notifications.send")
            .allow(PlatformOperation::NotificationSend)
            .availability(RuntimeAvailability::Runtime)
            .desktop(true)
            .plugin(false),
        CapabilityRecord::new("shortcuts.register")
            .allow(PlatformOperation::GlobalShortcutRegister)
            .availability(RuntimeAvailability::Runtime)
            .desktop(true)
            .plugin(false),
    ]);
    let host = HostCapabilityRouter::new(
        hawk2ui_platform::UnsupportedPlatformHost,
        RecordingAudioSink::default(),
        RecordingNotificationSink::default(),
        RecordingShortcutSink::default(),
    )
    .with_audio_cue(AudioCueBinding::new(
        "meter-click",
        "asset://audio/meter-click.wav",
    ))
    .with_notification(NotificationBinding::new(
        "build",
        "Build complete",
        "The Hawk2UI package is ready.",
    ))
    .with_shortcut(ShortcutBinding::new(
        "CommandOrControl+K",
        "command-palette",
    ));
    let mut backend = PlatformBackends::with_host(StaticNetworkBackend::default(), host);

    backend
        .play_audio(
            &capabilities,
            &AudioManifest::new("audio.playback", ["meter-click"]),
            "meter-click",
            PlatformContext::Desktop,
        )
        .expect("mapped audio cue executes through the configured audio sink");
    backend
        .send_notification(
            &capabilities,
            &NotificationManifest::new("notifications.send", ["build"]),
            "build",
            PlatformContext::Desktop,
        )
        .expect("mapped notification executes through the configured notification sink");
    backend
        .register_shortcut(
            &capabilities,
            &ShortcutManifest::new("shortcuts.register", ["CommandOrControl+K"]),
            "CommandOrControl+K",
            PlatformContext::Desktop,
        )
        .expect("mapped shortcut executes through the configured shortcut sink");

    assert_eq!(
        backend.host().audio_sink().played,
        [AudioCueBinding::new(
            "meter-click",
            "asset://audio/meter-click.wav"
        )]
    );
    assert_eq!(
        backend.host().notification_sink().sent,
        [NotificationBinding::new(
            "build",
            "Build complete",
            "The Hawk2UI package is ready."
        )]
    );
    assert_eq!(
        backend.host().shortcut_sink().registered,
        [ShortcutBinding::new(
            "CommandOrControl+K",
            "command-palette"
        )]
    );
}

#[test]
fn routed_host_backend_rejects_policy_approved_requests_without_explicit_mapping() {
    let capabilities = CapabilityTable::new([CapabilityRecord::new("audio.playback")
        .allow(PlatformOperation::AudioPlayback)
        .availability(RuntimeAvailability::Runtime)
        .desktop(true)
        .plugin(true)]);
    let host = HostCapabilityRouter::new(
        hawk2ui_platform::UnsupportedPlatformHost,
        RecordingAudioSink::default(),
        RecordingNotificationSink::default(),
        RecordingShortcutSink::default(),
    );
    let mut backend = PlatformBackends::with_host(StaticNetworkBackend::default(), host);

    let error = backend
        .play_audio(
            &capabilities,
            &AudioManifest::new("audio.playback", ["missing-cue"]),
            "missing-cue",
            PlatformContext::Desktop,
        )
        .expect_err("policy approval is not enough without an explicit host audio route");

    assert_eq!(error.diagnostic.rule, "audio.backend.cue-unmapped");
    assert!(backend.host().audio_sink().played.is_empty());
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "{prefix}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ))
}

fn serve_http_once(body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("local HTTP fixture binds");
    let address = listener
        .local_addr()
        .expect("local HTTP fixture address resolves");
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("local HTTP fixture accepts");
        let mut request = [0_u8; 1024];
        let _ = stream
            .read(&mut request)
            .expect("local HTTP fixture reads request");
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .expect("local HTTP fixture writes response");
    });
    format!("http://{address}/")
}

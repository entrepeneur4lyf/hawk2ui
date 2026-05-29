use hawk2ui_platform::{
    AiManifest, AiPolicy, AudioManifest, AudioPolicy, CapabilityRecord, CapabilitySchema,
    CapabilityTable, ClipboardDataType, ClipboardManifest, ClipboardPolicy, DatabaseMigration,
    DatabasePolicy, DialogKind, DialogManifest, DialogPolicy, FilesystemGrant, FilesystemPolicy,
    FilesystemScope, LocalizationManifest, LocalizationPolicy, McpManifest, McpPolicy,
    NetworkManifest, NetworkPolicy, NotificationManifest, NotificationPolicy, PlatformContext,
    PlatformDiagnostic, PlatformOperation, PlatformSecretManifest, PlatformSecretPolicy,
    RuntimeAvailability, ShortcutManifest, ShortcutPolicy,
};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
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

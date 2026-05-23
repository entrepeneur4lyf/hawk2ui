use hawk2ui_platform::{
    CapabilityRecord, CapabilitySchema, CapabilityTable, PlatformContext, PlatformDiagnostic,
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

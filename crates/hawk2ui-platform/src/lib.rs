#![forbid(unsafe_code)]
//! Capability-scoped platform APIs for filesystem, network, clipboard, secrets, and database access in `Hawk2UI`.

pub mod capability;
pub mod clipboard;
pub mod database;
pub mod filesystem;
pub mod network;
pub mod secrets;

pub use capability::{
    CapabilityDenied, CapabilityRecord, CapabilitySchema, CapabilityTable, PlatformContext,
    PlatformDiagnostic, PlatformOperation, RuntimeAvailability,
};
pub use clipboard::{
    ClipboardAccess, ClipboardDataType, ClipboardDenied, ClipboardManifest, ClipboardPolicy,
};
pub use database::{DatabaseDenied, DatabaseMigration, DatabasePolicy};
pub use filesystem::{
    FilesystemAccess, FilesystemDenied, FilesystemGrant, FilesystemPolicy, FilesystemScope,
};
pub use network::{NetworkDenied, NetworkManifest, NetworkPolicy, NetworkRequestRecord};
pub use secrets::{
    PlatformSecretDenied, PlatformSecretHandle, PlatformSecretManifest, PlatformSecretPolicy,
};

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-platform";

/// Returns the canonical Cargo package name for diagnostics and conformance checks.
#[must_use]
pub const fn crate_name() -> &'static str {
    CRATE_NAME
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_identity() {
        assert_eq!(crate_name(), "hawk2ui-platform");
    }
}

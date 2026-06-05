#![forbid(unsafe_code)]
//! Capability-scoped platform APIs for filesystem, network, clipboard, secrets, and database access in `Hawk2UI`.

pub mod ai;
pub mod audio;
pub mod backend;
pub mod capability;
pub mod clipboard;
pub mod database;
pub mod dialogs;
pub mod filesystem;
pub mod localization;
pub mod mcp;
pub mod network;
pub mod notifications;
pub mod secrets;
pub mod shortcuts;

pub use ai::{AiDenied, AiManifest, AiPolicy, AiProviderRequest};
pub use audio::{AudioDenied, AudioManifest, AudioPlaybackRequest, AudioPolicy};
pub use backend::{
    ClipboardReadResult, FilesystemReadResult, FilesystemWriteResult, NetworkBackend,
    NetworkResponse, NetworkResponsePayload, PlatformBackendError, PlatformBackends,
    StaticNetworkBackend, UreqNetworkBackend,
};
pub use capability::{
    CapabilityDenied, CapabilityRecord, CapabilitySchema, CapabilityTable, PlatformContext,
    PlatformDiagnostic, PlatformOperation, RuntimeAvailability,
};
pub use clipboard::{
    ClipboardAccess, ClipboardDataType, ClipboardDenied, ClipboardManifest, ClipboardPolicy,
};
pub use database::{DatabaseDenied, DatabaseMigration, DatabasePolicy};
pub use dialogs::{DialogDenied, DialogKind, DialogManifest, DialogPolicy, DialogRequest};
pub use filesystem::{
    FilesystemAccess, FilesystemDenied, FilesystemGrant, FilesystemPolicy, FilesystemScope,
};
pub use localization::{
    LocalizationDenied, LocalizationManifest, LocalizationPolicy, LocalizationRequest,
};
pub use mcp::{McpDenied, McpManifest, McpPolicy, McpToolCall};
pub use network::{NetworkDenied, NetworkManifest, NetworkPolicy, NetworkRequestRecord};
pub use notifications::{
    NotificationDenied, NotificationManifest, NotificationPolicy, NotificationRequest,
};
pub use secrets::{
    PlatformSecretDenied, PlatformSecretHandle, PlatformSecretManifest, PlatformSecretPolicy,
};
pub use shortcuts::{ShortcutDenied, ShortcutManifest, ShortcutPolicy, ShortcutRegistration};

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

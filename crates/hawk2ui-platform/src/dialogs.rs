//! Capability-scoped dialog and file-picker API records.

use crate::{CapabilityTable, PlatformContext, PlatformDiagnostic, PlatformOperation};

/// Host dialog kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DialogKind {
    /// Message or alert dialog.
    Message,
    /// Open/save file picker.
    FilePicker,
}

/// Dialog manifest declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogManifest {
    /// Required capability key.
    pub capability_key: String,
    /// Allowed dialog kinds.
    pub allowed_kinds: Vec<DialogKind>,
}

impl DialogManifest {
    /// Creates a dialog manifest declaration.
    #[must_use]
    pub fn new(
        capability_key: impl Into<String>,
        allowed_kinds: impl IntoIterator<Item = DialogKind>,
    ) -> Self {
        Self {
            capability_key: capability_key.into(),
            allowed_kinds: allowed_kinds.into_iter().collect(),
        }
    }
}

/// Allowed dialog request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogRequest {
    /// Dialog kind.
    pub kind: DialogKind,
}

/// Dialog denial.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogDenied {
    /// Dialog kind.
    pub kind: DialogKind,
    /// Structured diagnostic.
    pub diagnostic: PlatformDiagnostic,
}

/// Capability-scoped dialog policy.
pub struct DialogPolicy;

impl DialogPolicy {
    /// Validates dialog opening.
    ///
    /// # Errors
    ///
    /// Returns [`DialogDenied`] when the capability or dialog kind is denied.
    pub fn open(
        capabilities: &CapabilityTable,
        manifest: &DialogManifest,
        kind: DialogKind,
        context: PlatformContext,
    ) -> Result<DialogRequest, DialogDenied> {
        validate_dialog(
            capabilities,
            manifest,
            kind,
            PlatformOperation::DialogOpen,
            context,
        )
    }

    /// Validates file picker opening.
    ///
    /// # Errors
    ///
    /// Returns [`DialogDenied`] when the capability or file picker kind is denied.
    pub fn file_picker(
        capabilities: &CapabilityTable,
        manifest: &DialogManifest,
        context: PlatformContext,
    ) -> Result<DialogRequest, DialogDenied> {
        validate_dialog(
            capabilities,
            manifest,
            DialogKind::FilePicker,
            PlatformOperation::FilePickerOpen,
            context,
        )
    }
}

fn validate_dialog(
    capabilities: &CapabilityTable,
    manifest: &DialogManifest,
    kind: DialogKind,
    operation: PlatformOperation,
    context: PlatformContext,
) -> Result<DialogRequest, DialogDenied> {
    capabilities
        .ensure_allowed(&manifest.capability_key, operation, context)
        .map_err(|denial| DialogDenied {
            kind,
            diagnostic: denial.diagnostic,
        })?;
    if !manifest.allowed_kinds.contains(&kind) {
        return Err(DialogDenied {
            kind,
            diagnostic: PlatformDiagnostic::error(
                "dialog.kind.denied",
                format!("dialog kind is not declared: {kind:?}"),
            ),
        });
    }
    Ok(DialogRequest { kind })
}

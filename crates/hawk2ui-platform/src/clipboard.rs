//! Capability-scoped clipboard API records.

use crate::{CapabilityTable, PlatformContext, PlatformDiagnostic, PlatformOperation};

/// Clipboard data type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClipboardDataType {
    /// Plain text clipboard data.
    Text,
    /// Image clipboard data.
    Image,
}

/// Clipboard manifest declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardManifest {
    /// Required capability key.
    pub capability_key: String,
    /// Supported clipboard data types.
    pub supported_types: Vec<ClipboardDataType>,
    /// Whether clipboard access is enabled in plugin contexts.
    pub plugin_allowed: bool,
}

impl ClipboardManifest {
    /// Creates a clipboard manifest declaration.
    #[must_use]
    pub fn new(
        capability_key: impl Into<String>,
        supported_types: impl IntoIterator<Item = ClipboardDataType>,
    ) -> Self {
        Self {
            capability_key: capability_key.into(),
            supported_types: supported_types.into_iter().collect(),
            plugin_allowed: false,
        }
    }

    /// Sets plugin context availability.
    #[must_use]
    pub const fn plugin(mut self, allowed: bool) -> Self {
        self.plugin_allowed = allowed;
        self
    }
}

/// Allowed clipboard access record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardAccess {
    /// Clipboard data type.
    pub data_type: ClipboardDataType,
    /// Clipboard operation.
    pub operation: PlatformOperation,
}

/// Clipboard access denial.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardDenied {
    /// Clipboard data type.
    pub data_type: ClipboardDataType,
    /// Structured diagnostic.
    pub diagnostic: PlatformDiagnostic,
}

/// Capability-scoped clipboard policy.
pub struct ClipboardPolicy;

impl ClipboardPolicy {
    /// Validates clipboard access against capabilities and manifest declarations.
    ///
    /// # Errors
    ///
    /// Returns [`ClipboardDenied`] when the capability, data type, or context is denied.
    pub fn access(
        capabilities: &CapabilityTable,
        manifest: &ClipboardManifest,
        data_type: ClipboardDataType,
        operation: PlatformOperation,
        context: PlatformContext,
    ) -> Result<ClipboardAccess, ClipboardDenied> {
        capabilities
            .ensure_allowed(&manifest.capability_key, operation, context)
            .map_err(|denial| ClipboardDenied {
                data_type,
                diagnostic: denial.diagnostic,
            })?;
        if context == PlatformContext::Plugin && !manifest.plugin_allowed {
            return Err(ClipboardDenied {
                data_type,
                diagnostic: PlatformDiagnostic::error(
                    "clipboard.plugin.denied",
                    "clipboard access is denied in plugin context",
                ),
            });
        }
        if !manifest.supported_types.contains(&data_type) {
            return Err(ClipboardDenied {
                data_type,
                diagnostic: PlatformDiagnostic::error(
                    "clipboard.type.unsupported",
                    "clipboard data type is unsupported",
                ),
            });
        }
        Ok(ClipboardAccess {
            data_type,
            operation,
        })
    }
}

//! Capability-scoped global shortcut API records.

use crate::{CapabilityTable, PlatformContext, PlatformDiagnostic, PlatformOperation};

/// Global shortcut manifest declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShortcutManifest {
    /// Required capability key.
    pub capability_key: String,
    /// Allowed accelerator strings.
    pub allowed_accelerators: Vec<String>,
}

impl ShortcutManifest {
    /// Creates a shortcut manifest declaration.
    #[must_use]
    pub fn new(
        capability_key: impl Into<String>,
        allowed_accelerators: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            capability_key: capability_key.into(),
            allowed_accelerators: allowed_accelerators.into_iter().map(Into::into).collect(),
        }
    }
}

/// Allowed shortcut registration request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShortcutRegistration {
    /// Accelerator string.
    pub accelerator: String,
}

/// Shortcut denial.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShortcutDenied {
    /// Accelerator string.
    pub accelerator: String,
    /// Structured diagnostic.
    pub diagnostic: PlatformDiagnostic,
}

/// Capability-scoped shortcut policy.
pub struct ShortcutPolicy;

impl ShortcutPolicy {
    /// Validates global shortcut registration.
    ///
    /// # Errors
    ///
    /// Returns [`ShortcutDenied`] when the capability or accelerator is denied.
    pub fn register(
        capabilities: &CapabilityTable,
        manifest: &ShortcutManifest,
        accelerator: &str,
        context: PlatformContext,
    ) -> Result<ShortcutRegistration, ShortcutDenied> {
        capabilities
            .ensure_allowed(
                &manifest.capability_key,
                PlatformOperation::GlobalShortcutRegister,
                context,
            )
            .map_err(|denial| ShortcutDenied {
                accelerator: accelerator.into(),
                diagnostic: denial.diagnostic,
            })?;
        if !is_declared_accelerator(&manifest.allowed_accelerators, accelerator) {
            return Err(ShortcutDenied {
                accelerator: accelerator.into(),
                diagnostic: PlatformDiagnostic::error(
                    "shortcut.accelerator.denied",
                    format!("global shortcut accelerator is not declared: {accelerator}"),
                ),
            });
        }
        Ok(ShortcutRegistration {
            accelerator: accelerator.into(),
        })
    }
}

fn is_declared_accelerator(allowed: &[String], value: &str) -> bool {
    is_valid_accelerator(value) && allowed.iter().any(|entry| entry == value)
}

fn is_valid_accelerator(value: &str) -> bool {
    !value.is_empty()
        && !value.contains('\0')
        && !value.contains('\n')
        && !value.contains('\r')
        && !value.chars().any(char::is_whitespace)
}

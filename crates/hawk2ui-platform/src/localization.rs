//! Capability-scoped localization API records.

use crate::{CapabilityTable, PlatformContext, PlatformDiagnostic, PlatformOperation};

/// Localization manifest declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalizationManifest {
    /// Required capability key.
    pub capability_key: String,
    /// Allowed locale tags.
    pub allowed_locales: Vec<String>,
}

impl LocalizationManifest {
    /// Creates a localization manifest declaration.
    #[must_use]
    pub fn new(
        capability_key: impl Into<String>,
        allowed_locales: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            capability_key: capability_key.into(),
            allowed_locales: allowed_locales.into_iter().map(Into::into).collect(),
        }
    }
}

/// Allowed localization load request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalizationRequest {
    /// Locale tag.
    pub locale: String,
}

/// Localization denial.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalizationDenied {
    /// Locale tag.
    pub locale: String,
    /// Structured diagnostic.
    pub diagnostic: PlatformDiagnostic,
}

/// Capability-scoped localization policy.
pub struct LocalizationPolicy;

impl LocalizationPolicy {
    /// Validates localization bundle loading.
    ///
    /// # Errors
    ///
    /// Returns [`LocalizationDenied`] when the capability or locale is denied.
    pub fn load(
        capabilities: &CapabilityTable,
        manifest: &LocalizationManifest,
        locale: &str,
        context: PlatformContext,
    ) -> Result<LocalizationRequest, LocalizationDenied> {
        capabilities
            .ensure_allowed(
                &manifest.capability_key,
                PlatformOperation::LocalizationRead,
                context,
            )
            .map_err(|denial| LocalizationDenied {
                locale: locale.into(),
                diagnostic: denial.diagnostic,
            })?;
        if !is_declared_locale(&manifest.allowed_locales, locale) {
            return Err(LocalizationDenied {
                locale: locale.into(),
                diagnostic: PlatformDiagnostic::error(
                    "localization.locale.denied",
                    format!("locale is not declared: {locale}"),
                ),
            });
        }
        Ok(LocalizationRequest {
            locale: locale.into(),
        })
    }
}

fn is_declared_locale(allowed: &[String], value: &str) -> bool {
    is_valid_locale(value) && allowed.iter().any(|entry| entry == value)
}

fn is_valid_locale(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
}

//! Capability-scoped AI provider API records.

use crate::{CapabilityTable, PlatformContext, PlatformDiagnostic, PlatformOperation};

/// AI provider manifest declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AiManifest {
    /// Required capability key.
    pub capability_key: String,
    /// Allowed provider identifiers.
    pub allowed_providers: Vec<String>,
    /// Allowed operation identifiers.
    pub allowed_operations: Vec<String>,
}

impl AiManifest {
    /// Creates an AI provider manifest declaration.
    #[must_use]
    pub fn new(
        capability_key: impl Into<String>,
        allowed_providers: impl IntoIterator<Item = impl Into<String>>,
        allowed_operations: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            capability_key: capability_key.into(),
            allowed_providers: allowed_providers.into_iter().map(Into::into).collect(),
            allowed_operations: allowed_operations.into_iter().map(Into::into).collect(),
        }
    }
}

/// Allowed AI provider request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AiProviderRequest {
    /// Provider identifier.
    pub provider_id: String,
    /// Operation identifier.
    pub operation: String,
}

/// AI provider denial.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AiDenied {
    /// Provider identifier.
    pub provider_id: String,
    /// Structured diagnostic.
    pub diagnostic: PlatformDiagnostic,
}

/// Capability-scoped AI provider policy.
pub struct AiPolicy;

impl AiPolicy {
    /// Validates an AI provider request against capabilities and provider allowlists.
    ///
    /// # Errors
    ///
    /// Returns [`AiDenied`] when the capability, provider, or operation is denied.
    pub fn request(
        capabilities: &CapabilityTable,
        manifest: &AiManifest,
        provider_id: &str,
        operation: &str,
        context: PlatformContext,
    ) -> Result<AiProviderRequest, AiDenied> {
        capabilities
            .ensure_allowed(
                &manifest.capability_key,
                PlatformOperation::AiProviderRequest,
                context,
            )
            .map_err(|denial| AiDenied {
                provider_id: provider_id.into(),
                diagnostic: denial.diagnostic,
            })?;
        if !is_declared(&manifest.allowed_providers, provider_id) {
            return Err(AiDenied {
                provider_id: provider_id.into(),
                diagnostic: PlatformDiagnostic::error(
                    "ai.provider.denied",
                    format!("AI provider is not declared: {provider_id}"),
                ),
            });
        }
        if !is_declared(&manifest.allowed_operations, operation) {
            return Err(AiDenied {
                provider_id: provider_id.into(),
                diagnostic: PlatformDiagnostic::error(
                    "ai.operation.denied",
                    format!("AI operation is not declared: {operation}"),
                ),
            });
        }
        Ok(AiProviderRequest {
            provider_id: provider_id.into(),
            operation: operation.into(),
        })
    }
}

fn is_declared(allowed: &[String], value: &str) -> bool {
    is_stable_id(value) && allowed.iter().any(|entry| entry == value)
}

fn is_stable_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
}

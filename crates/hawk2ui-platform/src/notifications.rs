//! Capability-scoped notification API records.

use crate::{CapabilityTable, PlatformContext, PlatformDiagnostic, PlatformOperation};

/// Notification manifest declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NotificationManifest {
    /// Required capability key.
    pub capability_key: String,
    /// Allowed notification channels.
    pub allowed_channels: Vec<String>,
}

impl NotificationManifest {
    /// Creates a notification manifest declaration.
    #[must_use]
    pub fn new(
        capability_key: impl Into<String>,
        allowed_channels: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            capability_key: capability_key.into(),
            allowed_channels: allowed_channels.into_iter().map(Into::into).collect(),
        }
    }
}

/// Allowed notification send request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NotificationRequest {
    /// Notification channel.
    pub channel: String,
}

/// Notification denial.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NotificationDenied {
    /// Notification channel.
    pub channel: String,
    /// Structured diagnostic.
    pub diagnostic: PlatformDiagnostic,
}

/// Capability-scoped notification policy.
pub struct NotificationPolicy;

impl NotificationPolicy {
    /// Validates notification send access.
    ///
    /// # Errors
    ///
    /// Returns [`NotificationDenied`] when the capability or channel is denied.
    pub fn send(
        capabilities: &CapabilityTable,
        manifest: &NotificationManifest,
        channel: &str,
        context: PlatformContext,
    ) -> Result<NotificationRequest, NotificationDenied> {
        capabilities
            .ensure_allowed(
                &manifest.capability_key,
                PlatformOperation::NotificationSend,
                context,
            )
            .map_err(|denial| NotificationDenied {
                channel: channel.into(),
                diagnostic: denial.diagnostic,
            })?;
        if !is_declared(&manifest.allowed_channels, channel) {
            return Err(NotificationDenied {
                channel: channel.into(),
                diagnostic: PlatformDiagnostic::error(
                    "notification.channel.denied",
                    format!("notification channel is not declared: {channel}"),
                ),
            });
        }
        Ok(NotificationRequest {
            channel: channel.into(),
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

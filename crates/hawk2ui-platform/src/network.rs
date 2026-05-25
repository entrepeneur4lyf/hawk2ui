//! Capability-scoped network API records.

use crate::{CapabilityTable, PlatformContext, PlatformDiagnostic, PlatformOperation};
use url::Url;

/// Network manifest declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkManifest {
    /// Required platform capability key.
    pub capability_key: String,
    /// Allowed host names.
    pub allowed_hosts: Vec<String>,
}

impl NetworkManifest {
    /// Creates a network manifest declaration.
    #[must_use]
    pub fn new(
        capability_key: impl Into<String>,
        allowed_hosts: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            capability_key: capability_key.into(),
            allowed_hosts: allowed_hosts.into_iter().map(Into::into).collect(),
        }
    }
}

/// Allowed network request record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkRequestRecord {
    /// Request URL.
    pub url: String,
    /// Parsed request host.
    pub host: String,
}

/// Network request denial.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkDenied {
    /// Request URL.
    pub url: String,
    /// Structured diagnostic.
    pub diagnostic: PlatformDiagnostic,
}

/// Capability-scoped network policy.
pub struct NetworkPolicy;

impl NetworkPolicy {
    /// Validates a network request against capabilities and host allowlists.
    ///
    /// # Errors
    ///
    /// Returns [`NetworkDenied`] when the capability, URL, or host is denied.
    pub fn request(
        capabilities: &CapabilityTable,
        manifest: &NetworkManifest,
        url: &str,
        context: PlatformContext,
    ) -> Result<NetworkRequestRecord, NetworkDenied> {
        capabilities
            .ensure_allowed(
                &manifest.capability_key,
                PlatformOperation::NetworkRequest,
                context,
            )
            .map_err(|denial| NetworkDenied {
                url: url.into(),
                diagnostic: denial.diagnostic,
            })?;
        let host = parse_host(url).ok_or_else(|| NetworkDenied {
            url: url.into(),
            diagnostic: PlatformDiagnostic::error(
                "network.url.malformed",
                "network URL is malformed or uses an unsupported scheme",
            ),
        })?;
        if !manifest
            .allowed_hosts
            .iter()
            .filter_map(|allowed| canonical_host(allowed))
            .any(|allowed| allowed == host)
        {
            return Err(NetworkDenied {
                url: url.into(),
                diagnostic: PlatformDiagnostic::error(
                    "network.host.denied",
                    format!("network host is not declared: {host}"),
                ),
            });
        }
        Ok(NetworkRequestRecord {
            url: url.into(),
            host,
        })
    }
}

fn parse_host(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    match parsed.scheme() {
        "http" | "https" => {}
        _ => return None,
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return None;
    }
    parsed.host_str().and_then(canonical_host)
}

fn canonical_host(host: &str) -> Option<String> {
    let trimmed = host.trim().trim_end_matches('.');
    if trimmed.is_empty()
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains('@')
        || trimmed.contains(char::is_whitespace)
    {
        return None;
    }
    Some(trimmed.to_ascii_lowercase())
}

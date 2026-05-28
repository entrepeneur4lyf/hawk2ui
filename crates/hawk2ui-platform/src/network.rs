//! Capability-scoped network API records.

use std::collections::BTreeSet;

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
        let allowed_hosts = validate_allowed_hosts(manifest).ok_or_else(|| NetworkDenied {
            url: url.into(),
            diagnostic: PlatformDiagnostic::error(
                "network.manifest.invalid-hosts",
                "network manifest hosts must be non-empty, unique, and structurally valid",
            ),
        })?;
        let host = parse_host(url).ok_or_else(|| NetworkDenied {
            url: url.into(),
            diagnostic: PlatformDiagnostic::error(
                "network.url.malformed",
                "network URL is malformed or uses an unsupported scheme",
            ),
        })?;
        if !allowed_hosts.contains(&host) {
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

fn validate_allowed_hosts(manifest: &NetworkManifest) -> Option<BTreeSet<String>> {
    if manifest.allowed_hosts.is_empty() {
        return None;
    }
    let mut hosts = BTreeSet::new();
    for allowed in &manifest.allowed_hosts {
        let host = canonical_allowed_host(allowed)?;
        if !hosts.insert(host) {
            return None;
        }
    }
    Some(hosts)
}

fn parse_host(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    match parsed.scheme() {
        "http" | "https" => {}
        _ => return None,
    }
    if !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.fragment().is_some()
        || has_non_default_port(&parsed)
    {
        return None;
    }
    parsed.host_str().and_then(canonical_parsed_host)
}

fn canonical_allowed_host(host: &str) -> Option<String> {
    let trimmed = host.trim();
    if trimmed.is_empty()
        || trimmed.contains(char::is_whitespace)
        || trimmed.contains(['/', '\\', '@', '?', '#', ':'])
    {
        return None;
    }
    let parsed = Url::parse(&format!("https://{trimmed}/")).ok()?;
    if !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || parsed.path() != "/"
    {
        return None;
    }
    parsed.host_str().and_then(canonical_parsed_host)
}

fn canonical_parsed_host(host: &str) -> Option<String> {
    let trimmed = host.trim().trim_end_matches('.');
    (!trimmed.is_empty()
        && !trimmed.contains(['/', '\\', '@'])
        && !trimmed.contains(char::is_whitespace))
    .then(|| trimmed.to_ascii_lowercase())
}

fn has_non_default_port(url: &Url) -> bool {
    matches!(
        (url.scheme(), url.port()),
        ("http", Some(port)) if port != 80
    ) || matches!(
        (url.scheme(), url.port()),
        ("https", Some(port)) if port != 443
    )
}

//! Concrete platform backends layered behind capability-scoped policies.

use std::{collections::BTreeMap, fs, path::Path, time::Duration};

use crate::{
    CapabilityTable, ClipboardAccess, ClipboardDataType, ClipboardManifest, ClipboardPolicy,
    FilesystemAccess, FilesystemDenied, FilesystemGrant, FilesystemPolicy, NetworkDenied,
    NetworkManifest, NetworkPolicy, NetworkRequestRecord, PlatformContext, PlatformDiagnostic,
    PlatformOperation, PlatformSecretHandle, PlatformSecretManifest, PlatformSecretPolicy,
};

/// Platform backend failure with the operation that was being executed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformBackendError {
    /// Platform operation associated with the failure.
    pub operation: PlatformOperation,
    /// Structured diagnostic describing the failure.
    pub diagnostic: PlatformDiagnostic,
}

impl PlatformBackendError {
    fn new(
        operation: PlatformOperation,
        rule: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            operation,
            diagnostic: PlatformDiagnostic::error(rule, message),
        }
    }

    fn filesystem(operation: PlatformOperation, denied: FilesystemDenied) -> Self {
        Self {
            operation,
            diagnostic: denied.diagnostic,
        }
    }

    fn network(denied: NetworkDenied) -> Self {
        Self {
            operation: PlatformOperation::NetworkRequest,
            diagnostic: denied.diagnostic,
        }
    }
}

/// Result of a capability-scoped filesystem read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilesystemReadResult {
    /// Filesystem access resolved by [`FilesystemPolicy`].
    pub access: FilesystemAccess,
    /// Bytes read from the resolved file.
    pub bytes: Vec<u8>,
}

/// Result of a capability-scoped filesystem write.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilesystemWriteResult {
    /// Filesystem access resolved by [`FilesystemPolicy`].
    pub access: FilesystemAccess,
    /// Number of bytes written.
    pub bytes_written: usize,
}

/// Network response payload produced by a transport backend.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkResponsePayload {
    /// HTTP status code.
    pub status: u16,
    /// Response content type when provided by the server.
    pub content_type: Option<String>,
    /// Response body bytes.
    pub body: Vec<u8>,
}

impl NetworkResponsePayload {
    /// Creates a byte response payload.
    #[must_use]
    pub fn bytes(
        status: u16,
        content_type: Option<impl Into<String>>,
        body: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            status,
            content_type: content_type.map(Into::into),
            body: body.into(),
        }
    }

    /// Creates a UTF-8 text response payload.
    #[must_use]
    pub fn text(status: u16, content_type: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            status,
            content_type: Some(content_type.into()),
            body: body.into().into_bytes(),
        }
    }
}

/// Policy-approved network response.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkResponse {
    /// Request record approved by [`NetworkPolicy`].
    pub request: NetworkRequestRecord,
    /// HTTP status code.
    pub status: u16,
    /// Response content type when provided by the server.
    pub content_type: Option<String>,
    /// Response body bytes.
    pub body: Vec<u8>,
}

impl NetworkResponse {
    fn new(request: NetworkRequestRecord, payload: NetworkResponsePayload) -> Self {
        Self {
            request,
            status: payload.status,
            content_type: payload.content_type,
            body: payload.body,
        }
    }
}

/// Transport used by [`PlatformBackends`] after [`NetworkPolicy`] approves a request.
pub trait NetworkBackend {
    /// Executes an approved HTTP GET request.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformBackendError`] when the transport cannot complete the request.
    fn get(
        &mut self,
        request: &NetworkRequestRecord,
    ) -> Result<NetworkResponsePayload, PlatformBackendError>;
}

/// Concrete HTTP(S) network backend backed by `ureq`.
#[derive(Clone, Debug)]
pub struct UreqNetworkBackend {
    agent: ureq::Agent,
}

impl UreqNetworkBackend {
    /// Creates a `ureq` backend with a bounded global request timeout.
    #[must_use]
    pub fn new(timeout: Duration) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(timeout))
            .build()
            .new_agent();
        Self { agent }
    }
}

impl Default for UreqNetworkBackend {
    fn default() -> Self {
        Self::new(Duration::from_secs(30))
    }
}

impl NetworkBackend for UreqNetworkBackend {
    fn get(
        &mut self,
        request: &NetworkRequestRecord,
    ) -> Result<NetworkResponsePayload, PlatformBackendError> {
        let mut response = self.agent.get(&request.url).call().map_err(|error| {
            PlatformBackendError::new(
                PlatformOperation::NetworkRequest,
                "network.backend.request-failed",
                format!("network request failed: {error}"),
            )
        })?;
        let status = response.status().as_u16();
        let content_type = response.body().mime_type().map(ToOwned::to_owned);
        let body = response.body_mut().read_to_vec().map_err(|error| {
            PlatformBackendError::new(
                PlatformOperation::NetworkRequest,
                "network.backend.read-failed",
                format!("network response body could not be read: {error}"),
            )
        })?;

        Ok(NetworkResponsePayload {
            status,
            content_type,
            body,
        })
    }
}

/// Deterministic network backend for offline tests and packaged fixtures.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StaticNetworkBackend {
    responses: BTreeMap<String, NetworkResponsePayload>,
    requested_urls: Vec<String>,
}

impl StaticNetworkBackend {
    /// Creates a static backend from URL-to-response entries.
    #[must_use]
    pub fn new<K: Into<String>>(
        responses: impl IntoIterator<Item = (K, NetworkResponsePayload)>,
    ) -> Self {
        Self {
            responses: responses
                .into_iter()
                .map(|(url, response)| (url.into(), response))
                .collect(),
            requested_urls: Vec::new(),
        }
    }

    /// Returns URLs that reached the transport after policy approval.
    #[must_use]
    pub fn requested_urls(&self) -> &[String] {
        &self.requested_urls
    }
}

impl NetworkBackend for StaticNetworkBackend {
    fn get(
        &mut self,
        request: &NetworkRequestRecord,
    ) -> Result<NetworkResponsePayload, PlatformBackendError> {
        self.requested_urls.push(request.url.clone());
        self.responses.get(&request.url).cloned().ok_or_else(|| {
            PlatformBackendError::new(
                PlatformOperation::NetworkRequest,
                "network.backend.static-missing",
                format!("static network response is not registered: {}", request.url),
            )
        })
    }
}

/// Text clipboard read result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardReadResult {
    /// Clipboard access approved by [`ClipboardPolicy`].
    pub access: ClipboardAccess,
    /// Stored text, if the clipboard has text content.
    pub text: Option<String>,
}

/// Concrete platform backend stack.
#[derive(Clone, Debug)]
pub struct PlatformBackends<N = UreqNetworkBackend> {
    network: N,
    clipboard_text: Option<String>,
    secrets: BTreeMap<String, String>,
}

impl<N> PlatformBackends<N> {
    /// Creates a backend stack with the provided network backend.
    #[must_use]
    pub fn new(network: N) -> Self {
        Self {
            network,
            clipboard_text: None,
            secrets: BTreeMap::new(),
        }
    }

    /// Adds or replaces a secret value in the host secret store.
    #[must_use]
    pub fn with_secret(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.secrets.insert(key.into(), value.into());
        self
    }

    /// Returns the network backend for inspection or host-specific integration.
    #[must_use]
    pub const fn network(&self) -> &N {
        &self.network
    }
}

impl PlatformBackends<UreqNetworkBackend> {
    /// Creates a production backend stack using the default `ureq` HTTP(S) transport.
    #[must_use]
    pub fn system() -> Self {
        Self::new(UreqNetworkBackend::default())
    }
}

impl<N: NetworkBackend> PlatformBackends<N> {
    /// Reads a scoped filesystem path through [`FilesystemPolicy`].
    ///
    /// # Errors
    ///
    /// Returns [`PlatformBackendError`] when policy resolution or filesystem IO fails.
    pub fn read_file(
        &self,
        grant: &FilesystemGrant,
        relative_path: &str,
    ) -> Result<FilesystemReadResult, PlatformBackendError> {
        let access = FilesystemPolicy::resolve(grant, relative_path).map_err(|denied| {
            PlatformBackendError::filesystem(PlatformOperation::FilesystemRead, denied)
        })?;
        let bytes = fs::read(&access.resolved_path).map_err(|error| {
            PlatformBackendError::new(
                PlatformOperation::FilesystemRead,
                "filesystem.backend.read-failed",
                format!("filesystem read failed: {error}"),
            )
        })?;

        Ok(FilesystemReadResult { access, bytes })
    }

    /// Writes a scoped filesystem path through [`FilesystemPolicy`].
    ///
    /// # Errors
    ///
    /// Returns [`PlatformBackendError`] when policy resolution or filesystem IO fails.
    pub fn write_file(
        &self,
        grant: &FilesystemGrant,
        relative_path: &str,
        bytes: &[u8],
    ) -> Result<FilesystemWriteResult, PlatformBackendError> {
        let access = FilesystemPolicy::resolve(grant, relative_path).map_err(|denied| {
            PlatformBackendError::filesystem(PlatformOperation::FilesystemWrite, denied)
        })?;
        if let Some(parent) = Path::new(&access.resolved_path).parent() {
            fs::create_dir_all(parent).map_err(|error| {
                PlatformBackendError::new(
                    PlatformOperation::FilesystemWrite,
                    "filesystem.backend.create-parent-failed",
                    format!("filesystem write parent directory could not be created: {error}"),
                )
            })?;
        }
        fs::write(&access.resolved_path, bytes).map_err(|error| {
            PlatformBackendError::new(
                PlatformOperation::FilesystemWrite,
                "filesystem.backend.write-failed",
                format!("filesystem write failed: {error}"),
            )
        })?;

        Ok(FilesystemWriteResult {
            access,
            bytes_written: bytes.len(),
        })
    }

    /// Executes a policy-approved HTTP GET request.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformBackendError`] when capability/host policy denies the request or the
    /// transport fails.
    pub fn network_get(
        &mut self,
        capabilities: &CapabilityTable,
        manifest: &NetworkManifest,
        url: &str,
        context: PlatformContext,
    ) -> Result<NetworkResponse, PlatformBackendError> {
        let request = NetworkPolicy::request(capabilities, manifest, url, context)
            .map_err(PlatformBackendError::network)?;
        let payload = self.network.get(&request)?;
        Ok(NetworkResponse::new(request, payload))
    }

    /// Writes text to the host clipboard after policy approval.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformBackendError`] when clipboard policy denies the write.
    pub fn write_clipboard(
        &mut self,
        capabilities: &CapabilityTable,
        manifest: &ClipboardManifest,
        context: PlatformContext,
        text: impl Into<String>,
    ) -> Result<ClipboardAccess, PlatformBackendError> {
        let access = ClipboardPolicy::access(
            capabilities,
            manifest,
            ClipboardDataType::Text,
            PlatformOperation::ClipboardWrite,
            context,
        )
        .map_err(|denied| PlatformBackendError {
            operation: PlatformOperation::ClipboardWrite,
            diagnostic: denied.diagnostic,
        })?;
        self.clipboard_text = Some(text.into());
        Ok(access)
    }

    /// Reads text from the host clipboard after policy approval.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformBackendError`] when clipboard policy denies the read.
    pub fn read_clipboard(
        &self,
        capabilities: &CapabilityTable,
        manifest: &ClipboardManifest,
        context: PlatformContext,
    ) -> Result<ClipboardReadResult, PlatformBackendError> {
        let access = ClipboardPolicy::access(
            capabilities,
            manifest,
            ClipboardDataType::Text,
            PlatformOperation::ClipboardRead,
            context,
        )
        .map_err(|denied| PlatformBackendError {
            operation: PlatformOperation::ClipboardRead,
            diagnostic: denied.diagnostic,
        })?;

        Ok(ClipboardReadResult {
            access,
            text: self.clipboard_text.clone(),
        })
    }

    /// Reads a declared secret from the host secret store.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformBackendError`] when the secret declaration is invalid/missing or the host
    /// store does not contain a value.
    pub fn read_secret(
        &self,
        manifest: &PlatformSecretManifest,
        key: &str,
    ) -> Result<PlatformSecretHandle, PlatformBackendError> {
        let Some(value) = self.secrets.get(key) else {
            PlatformSecretPolicy::read(manifest, key, "").map_err(|denied| {
                PlatformBackendError {
                    operation: PlatformOperation::SecretRead,
                    diagnostic: denied.diagnostic,
                }
            })?;
            return Err(PlatformBackendError::new(
                PlatformOperation::SecretRead,
                "secret.store.missing",
                format!("secret value is not present in the host store: {key}"),
            ));
        };
        PlatformSecretPolicy::read(manifest, key, value).map_err(|denied| PlatformBackendError {
            operation: PlatformOperation::SecretRead,
            diagnostic: denied.diagnostic,
        })
    }
}

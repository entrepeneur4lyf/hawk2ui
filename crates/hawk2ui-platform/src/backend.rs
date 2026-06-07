//! Concrete platform backends layered behind capability-scoped policies.

use std::{
    collections::BTreeMap,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    time::Duration,
};

use serde::{Deserialize, Serialize};

use crate::{
    AiDenied, AiManifest, AiPolicy, AiProviderRequest, AudioDenied, AudioManifest,
    AudioPlaybackRequest, AudioPolicy, CapabilityDenied, CapabilityTable, ClipboardAccess,
    ClipboardDataType, ClipboardManifest, ClipboardPolicy, DatabaseDenied, DatabaseManifest,
    DatabaseMigration, DatabasePolicy, DialogDenied, DialogKind, DialogManifest, DialogPolicy,
    DialogRequest, FilesystemAccess, FilesystemDenied, FilesystemGrant, FilesystemPolicy,
    LocalizationDenied, LocalizationManifest, LocalizationPolicy, LocalizationRequest, McpDenied,
    McpManifest, McpPolicy, McpToolCall, NetworkDenied, NetworkManifest, NetworkPolicy,
    NetworkRequestRecord, NotificationDenied, NotificationManifest, NotificationPolicy,
    NotificationRequest, PlatformContext, PlatformDiagnostic, PlatformOperation,
    PlatformSecretHandle, PlatformSecretManifest, PlatformSecretPolicy, ShortcutDenied,
    ShortcutManifest, ShortcutPolicy, ShortcutRegistration,
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

    fn capability(operation: PlatformOperation, denied: CapabilityDenied) -> Self {
        Self {
            operation,
            diagnostic: denied.diagnostic,
        }
    }

    fn ai(denied: AiDenied) -> Self {
        Self {
            operation: PlatformOperation::AiProviderRequest,
            diagnostic: denied.diagnostic,
        }
    }

    fn audio(denied: AudioDenied) -> Self {
        Self {
            operation: PlatformOperation::AudioPlayback,
            diagnostic: denied.diagnostic,
        }
    }

    fn database(operation: PlatformOperation, denied: DatabaseDenied) -> Self {
        Self {
            operation,
            diagnostic: denied.diagnostic,
        }
    }

    fn dialog(operation: PlatformOperation, denied: DialogDenied) -> Self {
        Self {
            operation,
            diagnostic: denied.diagnostic,
        }
    }

    fn localization(denied: LocalizationDenied) -> Self {
        Self {
            operation: PlatformOperation::LocalizationRead,
            diagnostic: denied.diagnostic,
        }
    }

    fn mcp(denied: McpDenied) -> Self {
        Self {
            operation: PlatformOperation::McpToolCall,
            diagnostic: denied.diagnostic,
        }
    }

    fn notification(denied: NotificationDenied) -> Self {
        Self {
            operation: PlatformOperation::NotificationSend,
            diagnostic: denied.diagnostic,
        }
    }

    fn shortcut(denied: ShortcutDenied) -> Self {
        Self {
            operation: PlatformOperation::GlobalShortcutRegister,
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

/// Binary/text payload returned by host/provider backends.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostDataPayload {
    /// Response content type when known.
    content_type: Option<String>,
    /// Response body bytes.
    body: Vec<u8>,
}

impl HostDataPayload {
    /// Creates a text payload.
    #[must_use]
    pub fn text(content_type: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            content_type: Some(content_type.into()),
            body: body.into().into_bytes(),
        }
    }

    /// Creates a byte payload.
    #[must_use]
    pub fn bytes(content_type: Option<impl Into<String>>, body: impl Into<Vec<u8>>) -> Self {
        Self {
            content_type: content_type.map(Into::into),
            body: body.into(),
        }
    }

    fn into_parts(self) -> (Option<String>, Vec<u8>) {
        (self.content_type, self.body)
    }
}

/// Host-resolved audio cue binding for an approved [`AudioPlaybackRequest`].
///
/// `AudioPolicy` intentionally approves only stable cue identifiers. Production hosts bind those
/// identifiers to actual assets, streams, or engine-specific cue handles through this type before
/// playback reaches a concrete audio sink.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AudioCueBinding {
    /// Stable cue identifier approved by platform policy.
    pub cue_id: String,
    /// Host-resolved audio source URI or asset handle.
    pub source_uri: String,
}

impl AudioCueBinding {
    /// Creates an audio cue binding.
    #[must_use]
    pub fn new(cue_id: impl Into<String>, source_uri: impl Into<String>) -> Self {
        Self {
            cue_id: cue_id.into(),
            source_uri: source_uri.into(),
        }
    }
}

/// Host-resolved notification binding for an approved [`NotificationRequest`].
///
/// The notification policy authorizes a stable channel. Hosts bind that channel to concrete title
/// and body content so adapters can send a real OS or host notification without inferring copy from
/// the channel name alone.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NotificationBinding {
    /// Stable notification channel approved by platform policy.
    pub channel: String,
    /// Notification title to show through the host adapter.
    pub title: String,
    /// Notification body to show through the host adapter.
    pub body: String,
}

impl NotificationBinding {
    /// Creates a notification binding.
    #[must_use]
    pub fn new(
        channel: impl Into<String>,
        title: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        Self {
            channel: channel.into(),
            title: title.into(),
            body: body.into(),
        }
    }
}

/// Host-resolved global shortcut binding for an approved [`ShortcutRegistration`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShortcutBinding {
    /// Accelerator string approved by platform policy.
    pub accelerator: String,
    /// Stable host action invoked when the shortcut fires.
    pub action_id: String,
}

impl ShortcutBinding {
    /// Creates a shortcut binding.
    #[must_use]
    pub fn new(accelerator: impl Into<String>, action_id: impl Into<String>) -> Self {
        Self {
            accelerator: accelerator.into(),
            action_id: action_id.into(),
        }
    }
}

/// Adapter boundary for concrete audio cue playback.
pub trait AudioPlaybackSink {
    /// Plays a host-resolved audio cue.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformBackendError`] when the concrete audio backend cannot play the binding.
    fn play_audio_cue(&mut self, binding: &AudioCueBinding) -> Result<(), PlatformBackendError>;
}

/// Adapter boundary for concrete notification delivery.
pub trait NotificationSink {
    /// Sends a host-resolved notification.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformBackendError`] when the concrete notification backend cannot send it.
    fn send_notification(
        &mut self,
        binding: &NotificationBinding,
    ) -> Result<(), PlatformBackendError>;
}

/// Adapter boundary for concrete global shortcut registration.
pub trait GlobalShortcutSink {
    /// Registers a host-resolved global shortcut.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformBackendError`] when the concrete shortcut backend cannot register it.
    fn register_shortcut(&mut self, binding: &ShortcutBinding) -> Result<(), PlatformBackendError>;
}

/// Filesystem-backed localization bundle host adapter.
///
/// This adapter loads approved locale bundles from a configured directory using the
/// `{locale}.json` naming convention. It performs its own path containment check after policy
/// approval so a compromised or malformed request cannot escape the bundle root through symlinks
/// or unsafe locale strings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilesystemLocalizationHostBackend {
    root_path: PathBuf,
    content_type: String,
}

impl FilesystemLocalizationHostBackend {
    /// Creates a localization host backend rooted at `root_path`.
    #[must_use]
    pub fn new(root_path: impl AsRef<Path>) -> Self {
        Self {
            root_path: root_path.as_ref().to_path_buf(),
            content_type: "application/json".to_owned(),
        }
    }

    fn bundle_path(&self, request: &LocalizationRequest) -> Result<PathBuf, PlatformBackendError> {
        if !is_safe_locale_segment(&request.locale) {
            return Err(PlatformBackendError::new(
                PlatformOperation::LocalizationRead,
                "localization.backend.locale-invalid",
                format!(
                    "localization locale is not safe for filesystem bundle resolution: {}",
                    request.locale
                ),
            ));
        }
        Ok(self.root_path.join(format!("{}.json", request.locale)))
    }
}

impl PlatformHostBackend for FilesystemLocalizationHostBackend {
    fn load_localization(
        &mut self,
        request: &LocalizationRequest,
    ) -> Result<HostDataPayload, PlatformBackendError> {
        let root = self.root_path.canonicalize().map_err(|error| {
            PlatformBackendError::new(
                PlatformOperation::LocalizationRead,
                "localization.backend.root-invalid",
                format!(
                    "localization bundle root {} cannot be canonicalized: {error}",
                    self.root_path.display()
                ),
            )
        })?;
        let bundle_path = self.bundle_path(request)?;
        let canonical_bundle = bundle_path.canonicalize().map_err(|error| {
            let rule = if error.kind() == ErrorKind::NotFound {
                "localization.backend.bundle-missing"
            } else {
                "localization.backend.bundle-invalid"
            };
            PlatformBackendError::new(
                PlatformOperation::LocalizationRead,
                rule,
                format!(
                    "localization bundle {} cannot be canonicalized: {error}",
                    bundle_path.display()
                ),
            )
        })?;
        if !canonical_bundle.starts_with(&root) {
            return Err(PlatformBackendError::new(
                PlatformOperation::LocalizationRead,
                "localization.backend.bundle-escape",
                "localization bundle path escapes the configured root",
            ));
        }
        let body = fs::read(&canonical_bundle).map_err(|error| {
            PlatformBackendError::new(
                PlatformOperation::LocalizationRead,
                "localization.backend.read-failed",
                format!(
                    "localization bundle {} could not be read: {error}",
                    canonical_bundle.display()
                ),
            )
        })?;
        Ok(HostDataPayload::bytes(
            Some(self.content_type.clone()),
            body,
        ))
    }
}

/// HTTP-backed provider adapter for approved AI and MCP calls.
///
/// The backend never discovers endpoints implicitly. Hosts must explicitly map each approved
/// `(provider, operation)` or `(server, tool)` pair to a concrete endpoint, and the normal
/// capability policy must approve the request before this backend is invoked.
#[derive(Clone, Debug)]
pub struct HttpProviderHostBackend {
    agent: ureq::Agent,
    ai_endpoints: BTreeMap<(String, String), String>,
    mcp_endpoints: BTreeMap<(String, String), String>,
}

impl HttpProviderHostBackend {
    /// Creates an HTTP provider backend with a bounded global request timeout.
    #[must_use]
    pub fn new(timeout: Duration) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(timeout))
            .build()
            .new_agent();
        Self {
            agent,
            ai_endpoints: BTreeMap::new(),
            mcp_endpoints: BTreeMap::new(),
        }
    }

    /// Registers an AI provider endpoint for one provider operation.
    #[must_use]
    pub fn with_ai_endpoint(
        mut self,
        provider_id: impl Into<String>,
        operation: impl Into<String>,
        url: impl Into<String>,
    ) -> Self {
        self.ai_endpoints
            .insert((provider_id.into(), operation.into()), url.into());
        self
    }

    /// Registers an MCP endpoint for one server/tool pair.
    #[must_use]
    pub fn with_mcp_endpoint(
        mut self,
        server_id: impl Into<String>,
        tool_name: impl Into<String>,
        url: impl Into<String>,
    ) -> Self {
        self.mcp_endpoints
            .insert((server_id.into(), tool_name.into()), url.into());
        self
    }

    fn get_endpoint(
        &mut self,
        operation: PlatformOperation,
        url: &str,
    ) -> Result<HostDataPayload, PlatformBackendError> {
        let mut response = self.agent.get(url).call().map_err(|error| {
            PlatformBackendError::new(
                operation,
                "provider.http.request-failed",
                format!("provider HTTP request failed: {error}"),
            )
        })?;
        let content_type = response.body().mime_type().map(ToOwned::to_owned);
        let body = response.body_mut().read_to_vec().map_err(|error| {
            PlatformBackendError::new(
                operation,
                "provider.http.read-failed",
                format!("provider HTTP response body could not be read: {error}"),
            )
        })?;
        Ok(HostDataPayload::bytes(content_type, body))
    }
}

impl PlatformHostBackend for HttpProviderHostBackend {
    fn request_ai(
        &mut self,
        request: &AiProviderRequest,
    ) -> Result<HostDataPayload, PlatformBackendError> {
        let key = (request.provider_id.clone(), request.operation.clone());
        let url = self.ai_endpoints.get(&key).cloned().ok_or_else(|| {
            PlatformBackendError::new(
                PlatformOperation::AiProviderRequest,
                "ai.backend.endpoint-missing",
                format!(
                    "AI provider endpoint is not registered: {}:{}",
                    request.provider_id, request.operation
                ),
            )
        })?;
        self.get_endpoint(PlatformOperation::AiProviderRequest, &url)
    }

    fn call_mcp(&mut self, request: &McpToolCall) -> Result<HostDataPayload, PlatformBackendError> {
        let key = (request.server_id.clone(), request.tool_name.clone());
        let url = self.mcp_endpoints.get(&key).cloned().ok_or_else(|| {
            PlatformBackendError::new(
                PlatformOperation::McpToolCall,
                "mcp.backend.endpoint-missing",
                format!(
                    "MCP tool endpoint is not registered: {}:{}",
                    request.server_id, request.tool_name
                ),
            )
        })?;
        self.get_endpoint(PlatformOperation::McpToolCall, &url)
    }
}

/// Host dialog result returned by desktop or plugin host adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostDialogResponse {
    /// Dialog kind that produced the response.
    pub kind: DialogKind,
    /// Whether the host accepted/confirmed the dialog.
    accepted: bool,
    /// Selected paths for file picker dialogs.
    selected_paths: Vec<String>,
}

impl HostDialogResponse {
    /// Creates an accepted dialog response.
    #[must_use]
    pub fn accepted(
        kind: DialogKind,
        selected_paths: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            kind,
            accepted: true,
            selected_paths: selected_paths.into_iter().map(Into::into).collect(),
        }
    }

    /// Creates a cancelled dialog response.
    #[must_use]
    pub const fn cancelled(kind: DialogKind) -> Self {
        Self {
            kind,
            accepted: false,
            selected_paths: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
struct DatabaseStore {
    migrations: Vec<DatabaseMigration>,
    values: BTreeMap<String, serde_json::Value>,
}

/// Result of a capability-scoped audio playback request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AudioPlaybackResult {
    /// Audio request approved by [`AudioPolicy`].
    pub request: AudioPlaybackRequest,
}

/// Result of a capability-scoped AI provider request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AiProviderResponse {
    /// AI provider request approved by [`AiPolicy`].
    pub request: AiProviderRequest,
    /// Response content type.
    pub content_type: Option<String>,
    /// Response body bytes.
    pub body: Vec<u8>,
}

/// Result of a capability-scoped MCP tool call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct McpToolResponse {
    /// MCP tool call approved by [`McpPolicy`].
    pub request: McpToolCall,
    /// Response content type.
    pub content_type: Option<String>,
    /// Response body bytes.
    pub body: Vec<u8>,
}

/// Result of a capability-scoped localization bundle load.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalizationBundleResult {
    /// Localization request approved by [`LocalizationPolicy`].
    pub request: LocalizationRequest,
    /// Bundle content type.
    pub content_type: Option<String>,
    /// Bundle body bytes.
    pub body: Vec<u8>,
}

/// Result of a capability-scoped dialog open.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogOpenResult {
    /// Dialog request approved by [`DialogPolicy`].
    pub request: DialogRequest,
    /// Whether the host accepted/confirmed the dialog.
    pub accepted: bool,
    /// Selected paths for file picker dialogs.
    pub selected_paths: Vec<String>,
}

/// Result of a capability-scoped notification send.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NotificationSendResult {
    /// Notification request approved by [`NotificationPolicy`].
    pub request: NotificationRequest,
}

/// Result of a capability-scoped shortcut registration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShortcutRegistrationResult {
    /// Shortcut registration approved by [`ShortcutPolicy`].
    pub registration: ShortcutRegistration,
}

/// Result of a capability-scoped database migration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DatabaseMigrationResult {
    /// Resolved database storage path.
    pub storage_path: String,
    /// Complete migration history after applying pending migrations.
    pub applied_migrations: Vec<DatabaseMigration>,
}

/// Result of a capability-scoped database transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DatabaseTransactionResult {
    /// Resolved database storage path.
    pub storage_path: String,
    /// Keys written by the transaction in commit order.
    pub written_keys: Vec<String>,
}

/// Result of a capability-scoped database value write.
#[derive(Clone, Debug, PartialEq)]
pub struct DatabaseWriteResult {
    /// Resolved database storage path.
    pub storage_path: String,
    /// Database key written.
    pub key: String,
    /// JSON value written.
    pub value: serde_json::Value,
}

/// Result of a capability-scoped database value read.
#[derive(Clone, Debug, PartialEq)]
pub struct DatabaseReadResult {
    /// Resolved database storage path.
    pub storage_path: String,
    /// Database key read.
    pub key: String,
    /// JSON value loaded from the store.
    pub value: Option<serde_json::Value>,
}

/// Host/provider backend boundary for platform domains that require desktop, plugin-host, or
/// external service integration.
///
/// Production hosts implement this trait with real OS/provider adapters. The default
/// [`DeterministicPlatformHost`] implementation is intended for tests, offline fixtures, and
/// deterministic examples.
pub trait PlatformHostBackend {
    /// Writes approved clipboard text.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformBackendError`] when the host clipboard cannot be written.
    fn write_clipboard_text(
        &mut self,
        _access: &ClipboardAccess,
        _text: String,
    ) -> Result<(), PlatformBackendError> {
        Err(host_backend_unsupported(
            PlatformOperation::ClipboardWrite,
            "clipboard write",
        ))
    }

    /// Reads approved clipboard text.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformBackendError`] when the host clipboard cannot be read.
    fn read_clipboard_text(
        &mut self,
        _access: &ClipboardAccess,
    ) -> Result<Option<String>, PlatformBackendError> {
        Err(host_backend_unsupported(
            PlatformOperation::ClipboardRead,
            "clipboard read",
        ))
    }

    /// Reads a declared secret value from the host secret store.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformBackendError`] when the host secret store cannot load the key.
    fn read_secret_value(&self, key: &str) -> Result<String, PlatformBackendError> {
        Err(PlatformBackendError::new(
            PlatformOperation::SecretRead,
            "secret.store.missing",
            format!("secret value is not present in the host store: {key}"),
        ))
    }

    /// Executes approved audio playback.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformBackendError`] when the host audio service cannot play the cue.
    fn play_audio(&mut self, _request: &AudioPlaybackRequest) -> Result<(), PlatformBackendError> {
        Err(host_backend_unsupported(
            PlatformOperation::AudioPlayback,
            "audio playback",
        ))
    }

    /// Executes an approved AI provider request.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformBackendError`] when the provider cannot complete the request.
    fn request_ai(
        &mut self,
        _request: &AiProviderRequest,
    ) -> Result<HostDataPayload, PlatformBackendError> {
        Err(host_backend_unsupported(
            PlatformOperation::AiProviderRequest,
            "AI provider request",
        ))
    }

    /// Executes an approved MCP tool call.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformBackendError`] when the MCP server cannot complete the call.
    fn call_mcp(
        &mut self,
        _request: &McpToolCall,
    ) -> Result<HostDataPayload, PlatformBackendError> {
        Err(host_backend_unsupported(
            PlatformOperation::McpToolCall,
            "MCP tool call",
        ))
    }

    /// Loads an approved localization bundle.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformBackendError`] when the host cannot load the locale.
    fn load_localization(
        &mut self,
        _request: &LocalizationRequest,
    ) -> Result<HostDataPayload, PlatformBackendError> {
        Err(host_backend_unsupported(
            PlatformOperation::LocalizationRead,
            "localization load",
        ))
    }

    /// Opens an approved host dialog or file picker.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformBackendError`] when the host dialog backend fails.
    fn open_dialog(
        &mut self,
        request: &DialogRequest,
    ) -> Result<HostDialogResponse, PlatformBackendError> {
        Err(host_backend_unsupported(
            dialog_operation(request.kind),
            "dialog open",
        ))
    }

    /// Sends an approved notification.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformBackendError`] when the host notification backend fails.
    fn send_notification(
        &mut self,
        _request: &NotificationRequest,
    ) -> Result<(), PlatformBackendError> {
        Err(host_backend_unsupported(
            PlatformOperation::NotificationSend,
            "notification send",
        ))
    }

    /// Registers an approved global shortcut.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformBackendError`] when the host shortcut backend fails.
    fn register_shortcut(
        &mut self,
        _registration: &ShortcutRegistration,
    ) -> Result<(), PlatformBackendError> {
        Err(host_backend_unsupported(
            PlatformOperation::GlobalShortcutRegister,
            "shortcut registration",
        ))
    }
}

fn host_backend_unsupported(operation: PlatformOperation, name: &str) -> PlatformBackendError {
    PlatformBackendError::new(
        operation,
        "platform.host-backend.unsupported",
        format!("platform host backend does not implement {name}"),
    )
}

/// Routes selected host capabilities to explicit concrete adapters while delegating every other
/// host operation to a base [`PlatformHostBackend`].
///
/// This keeps capability approval and host execution separate: policy approves stable IDs, the
/// router resolves those IDs to concrete host bindings, then sink implementations perform the real
/// OS/provider work.
#[derive(Clone, Debug)]
pub struct HostCapabilityRouter<B, A, N, S> {
    base: B,
    audio_sink: A,
    notification_sink: N,
    shortcut_sink: S,
    audio_bindings: BTreeMap<String, AudioCueBinding>,
    notification_bindings: BTreeMap<String, NotificationBinding>,
    shortcut_bindings: BTreeMap<String, ShortcutBinding>,
}

impl<B, A, N, S> HostCapabilityRouter<B, A, N, S> {
    /// Creates a host capability router around an existing base host backend.
    #[must_use]
    pub fn new(base: B, audio_sink: A, notification_sink: N, shortcut_sink: S) -> Self {
        Self {
            base,
            audio_sink,
            notification_sink,
            shortcut_sink,
            audio_bindings: BTreeMap::new(),
            notification_bindings: BTreeMap::new(),
            shortcut_bindings: BTreeMap::new(),
        }
    }

    /// Adds or replaces an audio cue route.
    #[must_use]
    pub fn with_audio_cue(mut self, binding: AudioCueBinding) -> Self {
        self.audio_bindings.insert(binding.cue_id.clone(), binding);
        self
    }

    /// Adds or replaces a notification route.
    #[must_use]
    pub fn with_notification(mut self, binding: NotificationBinding) -> Self {
        self.notification_bindings
            .insert(binding.channel.clone(), binding);
        self
    }

    /// Adds or replaces a shortcut route.
    #[must_use]
    pub fn with_shortcut(mut self, binding: ShortcutBinding) -> Self {
        self.shortcut_bindings
            .insert(binding.accelerator.clone(), binding);
        self
    }

    /// Returns the delegated base host backend.
    #[must_use]
    pub const fn base(&self) -> &B {
        &self.base
    }

    /// Returns the audio sink.
    #[must_use]
    pub const fn audio_sink(&self) -> &A {
        &self.audio_sink
    }

    /// Returns the notification sink.
    #[must_use]
    pub const fn notification_sink(&self) -> &N {
        &self.notification_sink
    }

    /// Returns the shortcut sink.
    #[must_use]
    pub const fn shortcut_sink(&self) -> &S {
        &self.shortcut_sink
    }
}

impl<B, A, N, S> PlatformHostBackend for HostCapabilityRouter<B, A, N, S>
where
    B: PlatformHostBackend,
    A: AudioPlaybackSink,
    N: NotificationSink,
    S: GlobalShortcutSink,
{
    fn write_clipboard_text(
        &mut self,
        access: &ClipboardAccess,
        text: String,
    ) -> Result<(), PlatformBackendError> {
        self.base.write_clipboard_text(access, text)
    }

    fn read_clipboard_text(
        &mut self,
        access: &ClipboardAccess,
    ) -> Result<Option<String>, PlatformBackendError> {
        self.base.read_clipboard_text(access)
    }

    fn read_secret_value(&self, key: &str) -> Result<String, PlatformBackendError> {
        self.base.read_secret_value(key)
    }

    fn play_audio(&mut self, request: &AudioPlaybackRequest) -> Result<(), PlatformBackendError> {
        let binding = self
            .audio_bindings
            .get(&request.cue_id)
            .cloned()
            .ok_or_else(|| {
                PlatformBackendError::new(
                    PlatformOperation::AudioPlayback,
                    "audio.backend.cue-unmapped",
                    format!("audio cue has no host binding: {}", request.cue_id),
                )
            })?;
        self.audio_sink.play_audio_cue(&binding)
    }

    fn request_ai(
        &mut self,
        request: &AiProviderRequest,
    ) -> Result<HostDataPayload, PlatformBackendError> {
        self.base.request_ai(request)
    }

    fn call_mcp(&mut self, request: &McpToolCall) -> Result<HostDataPayload, PlatformBackendError> {
        self.base.call_mcp(request)
    }

    fn load_localization(
        &mut self,
        request: &LocalizationRequest,
    ) -> Result<HostDataPayload, PlatformBackendError> {
        self.base.load_localization(request)
    }

    fn open_dialog(
        &mut self,
        request: &DialogRequest,
    ) -> Result<HostDialogResponse, PlatformBackendError> {
        self.base.open_dialog(request)
    }

    fn send_notification(
        &mut self,
        request: &NotificationRequest,
    ) -> Result<(), PlatformBackendError> {
        let binding = self
            .notification_bindings
            .get(&request.channel)
            .cloned()
            .ok_or_else(|| {
                PlatformBackendError::new(
                    PlatformOperation::NotificationSend,
                    "notification.backend.channel-unmapped",
                    format!(
                        "notification channel has no host binding: {}",
                        request.channel
                    ),
                )
            })?;
        self.notification_sink.send_notification(&binding)
    }

    fn register_shortcut(
        &mut self,
        registration: &ShortcutRegistration,
    ) -> Result<(), PlatformBackendError> {
        let binding = self
            .shortcut_bindings
            .get(&registration.accelerator)
            .cloned()
            .ok_or_else(|| {
                PlatformBackendError::new(
                    PlatformOperation::GlobalShortcutRegister,
                    "shortcut.backend.accelerator-unmapped",
                    format!(
                        "global shortcut accelerator has no host binding: {}",
                        registration.accelerator
                    ),
                )
            })?;
        self.shortcut_sink.register_shortcut(&binding)
    }
}

/// Deny-by-default host/provider backend for production stacks without an installed adapter.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UnsupportedPlatformHost;

impl PlatformHostBackend for UnsupportedPlatformHost {}

/// Deterministic host/provider backend for tests, examples, and offline fixtures.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeterministicPlatformHost {
    clipboard_text: Option<String>,
    secrets: BTreeMap<String, String>,
    ai_responses: BTreeMap<(String, String), HostDataPayload>,
    ai_requests: Vec<AiProviderRequest>,
    audio_playback: Vec<AudioPlaybackRequest>,
    dialog_responses: Vec<(DialogKind, HostDialogResponse)>,
    dialog_requests: Vec<DialogRequest>,
    localization_bundles: BTreeMap<String, HostDataPayload>,
    localization_requests: Vec<LocalizationRequest>,
    mcp_responses: BTreeMap<(String, String), HostDataPayload>,
    mcp_requests: Vec<McpToolCall>,
    notifications: Vec<NotificationRequest>,
    shortcuts: Vec<ShortcutRegistration>,
}

impl DeterministicPlatformHost {
    /// Adds or replaces a secret value in the host secret store.
    #[must_use]
    pub fn with_secret(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.secrets.insert(key.into(), value.into());
        self
    }

    /// Registers a deterministic AI provider response.
    #[must_use]
    pub fn with_ai_text_response(
        mut self,
        provider_id: impl Into<String>,
        operation: impl Into<String>,
        content_type: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        self.ai_responses.insert(
            (provider_id.into(), operation.into()),
            HostDataPayload::text(content_type, body),
        );
        self
    }

    /// Registers a deterministic MCP tool response.
    #[must_use]
    pub fn with_mcp_text_response(
        mut self,
        server_id: impl Into<String>,
        tool_name: impl Into<String>,
        content_type: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        self.mcp_responses.insert(
            (server_id.into(), tool_name.into()),
            HostDataPayload::text(content_type, body),
        );
        self
    }

    /// Registers a deterministic localization bundle.
    #[must_use]
    pub fn with_localization_text_bundle(
        mut self,
        locale: impl Into<String>,
        content_type: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        self.localization_bundles
            .insert(locale.into(), HostDataPayload::text(content_type, body));
        self
    }

    /// Registers a deterministic dialog response.
    #[must_use]
    pub fn with_dialog_response(
        mut self,
        kind: DialogKind,
        accepted: bool,
        selected_paths: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        let response = HostDialogResponse {
            kind,
            accepted,
            selected_paths: selected_paths.into_iter().map(Into::into).collect(),
        };
        if let Some((_, existing)) = self
            .dialog_responses
            .iter_mut()
            .find(|(candidate, _)| *candidate == kind)
        {
            *existing = response;
        } else {
            self.dialog_responses.push((kind, response));
        }
        self
    }

    /// Returns AI requests that reached backend execution after policy approval.
    #[must_use]
    pub fn ai_requests(&self) -> &[AiProviderRequest] {
        &self.ai_requests
    }

    /// Returns audio cue IDs that reached backend execution after policy approval.
    #[must_use]
    pub fn played_audio_cues(&self) -> Vec<&str> {
        self.audio_playback
            .iter()
            .map(|request| request.cue_id.as_str())
            .collect()
    }

    /// Returns notification channels that reached backend execution after policy approval.
    #[must_use]
    pub fn sent_notification_channels(&self) -> Vec<&str> {
        self.notifications
            .iter()
            .map(|request| request.channel.as_str())
            .collect()
    }

    /// Returns shortcuts that reached backend execution after policy approval.
    #[must_use]
    pub fn registered_shortcuts(&self) -> Vec<&str> {
        self.shortcuts
            .iter()
            .map(|registration| registration.accelerator.as_str())
            .collect()
    }
}

impl PlatformHostBackend for DeterministicPlatformHost {
    fn write_clipboard_text(
        &mut self,
        _access: &ClipboardAccess,
        text: String,
    ) -> Result<(), PlatformBackendError> {
        self.clipboard_text = Some(text);
        Ok(())
    }

    fn read_clipboard_text(
        &mut self,
        _access: &ClipboardAccess,
    ) -> Result<Option<String>, PlatformBackendError> {
        Ok(self.clipboard_text.clone())
    }

    fn read_secret_value(&self, key: &str) -> Result<String, PlatformBackendError> {
        self.secrets.get(key).cloned().ok_or_else(|| {
            PlatformBackendError::new(
                PlatformOperation::SecretRead,
                "secret.store.missing",
                format!("secret value is not present in the host store: {key}"),
            )
        })
    }

    fn play_audio(&mut self, request: &AudioPlaybackRequest) -> Result<(), PlatformBackendError> {
        self.audio_playback.push(request.clone());
        Ok(())
    }

    fn request_ai(
        &mut self,
        request: &AiProviderRequest,
    ) -> Result<HostDataPayload, PlatformBackendError> {
        let key = (request.provider_id.clone(), request.operation.clone());
        let payload = self.ai_responses.get(&key).cloned().ok_or_else(|| {
            PlatformBackendError::new(
                PlatformOperation::AiProviderRequest,
                "ai.backend.response-missing",
                format!(
                    "AI provider response is not registered: {}:{}",
                    request.provider_id, request.operation
                ),
            )
        })?;
        self.ai_requests.push(request.clone());
        Ok(payload)
    }

    fn call_mcp(&mut self, request: &McpToolCall) -> Result<HostDataPayload, PlatformBackendError> {
        let key = (request.server_id.clone(), request.tool_name.clone());
        let payload = self.mcp_responses.get(&key).cloned().ok_or_else(|| {
            PlatformBackendError::new(
                PlatformOperation::McpToolCall,
                "mcp.backend.response-missing",
                format!(
                    "MCP tool response is not registered: {}:{}",
                    request.server_id, request.tool_name
                ),
            )
        })?;
        self.mcp_requests.push(request.clone());
        Ok(payload)
    }

    fn load_localization(
        &mut self,
        request: &LocalizationRequest,
    ) -> Result<HostDataPayload, PlatformBackendError> {
        let payload = self
            .localization_bundles
            .get(&request.locale)
            .cloned()
            .ok_or_else(|| {
                PlatformBackendError::new(
                    PlatformOperation::LocalizationRead,
                    "localization.backend.bundle-missing",
                    format!("localization bundle is not registered: {}", request.locale),
                )
            })?;
        self.localization_requests.push(request.clone());
        Ok(payload)
    }

    fn open_dialog(
        &mut self,
        request: &DialogRequest,
    ) -> Result<HostDialogResponse, PlatformBackendError> {
        let response = self
            .dialog_responses
            .iter()
            .find(|(kind, _)| *kind == request.kind)
            .map(|(_, response)| response.clone())
            .ok_or_else(|| {
                PlatformBackendError::new(
                    dialog_operation(request.kind),
                    "dialog.backend.response-missing",
                    format!("dialog response is not registered: {:?}", request.kind),
                )
            })?;
        self.dialog_requests.push(request.clone());
        Ok(response)
    }

    fn send_notification(
        &mut self,
        request: &NotificationRequest,
    ) -> Result<(), PlatformBackendError> {
        self.notifications.push(request.clone());
        Ok(())
    }

    fn register_shortcut(
        &mut self,
        registration: &ShortcutRegistration,
    ) -> Result<(), PlatformBackendError> {
        self.shortcuts.push(registration.clone());
        Ok(())
    }
}

/// Concrete platform backend stack.
#[derive(Clone, Debug)]
pub struct PlatformBackends<N = UreqNetworkBackend, H = UnsupportedPlatformHost> {
    network: N,
    host: H,
}

impl<N> PlatformBackends<N, DeterministicPlatformHost> {
    /// Creates a deterministic backend stack with the provided network backend.
    #[must_use]
    pub fn new(network: N) -> Self {
        Self::with_host(network, DeterministicPlatformHost::default())
    }

    /// Adds or replaces a secret value in the deterministic host secret store.
    #[must_use]
    pub fn with_secret(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.host = self.host.with_secret(key, value);
        self
    }

    /// Registers a deterministic AI provider response.
    #[must_use]
    pub fn with_ai_text_response(
        mut self,
        provider_id: impl Into<String>,
        operation: impl Into<String>,
        content_type: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        self.host = self
            .host
            .with_ai_text_response(provider_id, operation, content_type, body);
        self
    }

    /// Registers a deterministic MCP tool response.
    #[must_use]
    pub fn with_mcp_text_response(
        mut self,
        server_id: impl Into<String>,
        tool_name: impl Into<String>,
        content_type: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        self.host = self
            .host
            .with_mcp_text_response(server_id, tool_name, content_type, body);
        self
    }

    /// Registers a deterministic localization bundle.
    #[must_use]
    pub fn with_localization_text_bundle(
        mut self,
        locale: impl Into<String>,
        content_type: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        self.host = self
            .host
            .with_localization_text_bundle(locale, content_type, body);
        self
    }

    /// Registers a deterministic dialog response.
    #[must_use]
    pub fn with_dialog_response(
        mut self,
        kind: DialogKind,
        accepted: bool,
        selected_paths: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.host = self
            .host
            .with_dialog_response(kind, accepted, selected_paths);
        self
    }

    /// Returns AI requests that reached deterministic host execution after policy approval.
    #[must_use]
    pub fn ai_requests(&self) -> &[AiProviderRequest] {
        self.host.ai_requests()
    }

    /// Returns audio cue IDs that reached deterministic host execution after policy approval.
    #[must_use]
    pub fn played_audio_cues(&self) -> Vec<&str> {
        self.host.played_audio_cues()
    }

    /// Returns notification channels that reached deterministic host execution after policy approval.
    #[must_use]
    pub fn sent_notification_channels(&self) -> Vec<&str> {
        self.host.sent_notification_channels()
    }

    /// Returns shortcuts that reached deterministic host execution after policy approval.
    #[must_use]
    pub fn registered_shortcuts(&self) -> Vec<&str> {
        self.host.registered_shortcuts()
    }
}

impl<N, H> PlatformBackends<N, H> {
    /// Creates a backend stack with explicit network and host/provider backends.
    #[must_use]
    pub const fn with_host(network: N, host: H) -> Self {
        Self { network, host }
    }

    /// Returns the network backend for inspection or host-specific integration.
    #[must_use]
    pub const fn network(&self) -> &N {
        &self.network
    }

    /// Returns the host/provider backend for inspection or adapter-specific integration.
    #[must_use]
    pub const fn host(&self) -> &H {
        &self.host
    }

    /// Returns the host/provider backend mutably for adapter-specific integration.
    #[must_use]
    pub const fn host_mut(&mut self) -> &mut H {
        &mut self.host
    }
}

impl PlatformBackends<UreqNetworkBackend, UnsupportedPlatformHost> {
    /// Creates a production backend stack using the default `ureq` HTTP(S) transport and a
    /// deny-by-default host/provider backend.
    ///
    /// Host-only capabilities such as clipboard, secrets, AI, MCP, dialogs, notifications,
    /// shortcuts, audio, and localization require an explicit [`PlatformHostBackend`] supplied via
    /// [`PlatformBackends::with_host`].
    #[must_use]
    pub fn system() -> Self {
        Self::with_host(UreqNetworkBackend::default(), UnsupportedPlatformHost)
    }
}

impl<N: NetworkBackend, H: PlatformHostBackend> PlatformBackends<N, H> {
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
        self.host.write_clipboard_text(&access, text.into())?;
        Ok(access)
    }

    /// Reads text from the host clipboard after policy approval.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformBackendError`] when clipboard policy denies the read.
    pub fn read_clipboard(
        &mut self,
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

        let text = self.host.read_clipboard_text(&access)?;

        Ok(ClipboardReadResult { access, text })
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
        PlatformSecretPolicy::read(manifest, key, "").map_err(|denied| PlatformBackendError {
            operation: PlatformOperation::SecretRead,
            diagnostic: denied.diagnostic,
        })?;
        let value = self.host.read_secret_value(key)?;
        PlatformSecretPolicy::read(manifest, key, &value).map_err(|denied| PlatformBackendError {
            operation: PlatformOperation::SecretRead,
            diagnostic: denied.diagnostic,
        })
    }

    /// Executes a policy-approved audio playback request.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformBackendError`] when audio policy denies the cue.
    pub fn play_audio(
        &mut self,
        capabilities: &CapabilityTable,
        manifest: &AudioManifest,
        cue_id: &str,
        context: PlatformContext,
    ) -> Result<AudioPlaybackResult, PlatformBackendError> {
        let request = AudioPolicy::request(capabilities, manifest, cue_id, context)
            .map_err(PlatformBackendError::audio)?;
        self.host.play_audio(&request)?;
        Ok(AudioPlaybackResult { request })
    }

    /// Executes a policy-approved AI provider request.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformBackendError`] when policy denies the request or the host has no response
    /// registered for the approved provider operation.
    pub fn ai_request(
        &mut self,
        capabilities: &CapabilityTable,
        manifest: &AiManifest,
        provider_id: &str,
        operation: &str,
        context: PlatformContext,
    ) -> Result<AiProviderResponse, PlatformBackendError> {
        let request = AiPolicy::request(capabilities, manifest, provider_id, operation, context)
            .map_err(PlatformBackendError::ai)?;
        let payload = self.host.request_ai(&request)?;
        let (content_type, body) = payload.into_parts();
        Ok(AiProviderResponse {
            request,
            content_type,
            body,
        })
    }

    /// Executes a policy-approved MCP tool call.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformBackendError`] when policy denies the call or the host has no response
    /// registered for the approved server/tool pair.
    pub fn call_mcp(
        &mut self,
        capabilities: &CapabilityTable,
        manifest: &McpManifest,
        server_id: &str,
        tool_name: &str,
        context: PlatformContext,
    ) -> Result<McpToolResponse, PlatformBackendError> {
        let request = McpPolicy::call(capabilities, manifest, server_id, tool_name, context)
            .map_err(PlatformBackendError::mcp)?;
        let payload = self.host.call_mcp(&request)?;
        let (content_type, body) = payload.into_parts();
        Ok(McpToolResponse {
            request,
            content_type,
            body,
        })
    }

    /// Loads a policy-approved localization bundle.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformBackendError`] when policy denies the locale or no host bundle is
    /// registered for it.
    pub fn load_localization(
        &mut self,
        capabilities: &CapabilityTable,
        manifest: &LocalizationManifest,
        locale: &str,
        context: PlatformContext,
    ) -> Result<LocalizationBundleResult, PlatformBackendError> {
        let request = LocalizationPolicy::load(capabilities, manifest, locale, context)
            .map_err(PlatformBackendError::localization)?;
        let payload = self.host.load_localization(&request)?;
        let (content_type, body) = payload.into_parts();
        Ok(LocalizationBundleResult {
            request,
            content_type,
            body,
        })
    }

    /// Opens a policy-approved host dialog.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformBackendError`] when policy denies the dialog kind or no host response is
    /// registered for it.
    pub fn open_dialog(
        &mut self,
        capabilities: &CapabilityTable,
        manifest: &DialogManifest,
        kind: DialogKind,
        context: PlatformContext,
    ) -> Result<DialogOpenResult, PlatformBackendError> {
        let request =
            DialogPolicy::open(capabilities, manifest, kind, context).map_err(|denied| {
                PlatformBackendError::dialog(PlatformOperation::DialogOpen, denied)
            })?;
        let response = self.host.open_dialog(&request)?;
        Ok(DialogOpenResult {
            request,
            accepted: response.accepted,
            selected_paths: response.selected_paths,
        })
    }

    /// Opens a policy-approved host file picker.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformBackendError`] when policy denies file picker access or no host response is
    /// registered for it.
    pub fn open_file_picker(
        &mut self,
        capabilities: &CapabilityTable,
        manifest: &DialogManifest,
        context: PlatformContext,
    ) -> Result<DialogOpenResult, PlatformBackendError> {
        let request =
            DialogPolicy::file_picker(capabilities, manifest, context).map_err(|denied| {
                PlatformBackendError::dialog(PlatformOperation::FilePickerOpen, denied)
            })?;
        let response = self.host.open_dialog(&request)?;
        Ok(DialogOpenResult {
            request,
            accepted: response.accepted,
            selected_paths: response.selected_paths,
        })
    }

    /// Sends a policy-approved notification.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformBackendError`] when notification policy denies the channel.
    pub fn send_notification(
        &mut self,
        capabilities: &CapabilityTable,
        manifest: &NotificationManifest,
        channel: &str,
        context: PlatformContext,
    ) -> Result<NotificationSendResult, PlatformBackendError> {
        let request = NotificationPolicy::send(capabilities, manifest, channel, context)
            .map_err(PlatformBackendError::notification)?;
        self.host.send_notification(&request)?;
        Ok(NotificationSendResult { request })
    }

    /// Registers a policy-approved global shortcut.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformBackendError`] when shortcut policy denies the accelerator.
    pub fn register_shortcut(
        &mut self,
        capabilities: &CapabilityTable,
        manifest: &ShortcutManifest,
        accelerator: &str,
        context: PlatformContext,
    ) -> Result<ShortcutRegistrationResult, PlatformBackendError> {
        let registration = ShortcutPolicy::register(capabilities, manifest, accelerator, context)
            .map_err(PlatformBackendError::shortcut)?;
        self.host.register_shortcut(&registration)?;
        Ok(ShortcutRegistrationResult { registration })
    }

    /// Applies pending migrations to a policy-approved database store.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformBackendError`] when capability policy, migration validation, storage
    /// resolution, or filesystem IO fails.
    pub fn migrate_database(
        &self,
        capabilities: &CapabilityTable,
        manifest: &DatabaseManifest,
        context: PlatformContext,
    ) -> Result<DatabaseMigrationResult, PlatformBackendError> {
        let access = resolve_database_access(
            capabilities,
            manifest,
            PlatformOperation::DatabaseMigration,
            context,
        )?;
        let mut store =
            read_database_store(&access.resolved_path, PlatformOperation::DatabaseMigration)?;
        apply_database_migrations(&mut store, &manifest.migrations)?;
        write_database_store(
            &access.resolved_path,
            &store,
            PlatformOperation::DatabaseMigration,
        )?;
        Ok(DatabaseMigrationResult {
            storage_path: access.resolved_path,
            applied_migrations: store.migrations,
        })
    }

    /// Writes a JSON value to a policy-approved database store.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformBackendError`] when capability policy, migration state, storage
    /// resolution, key validation, JSON serialization, or filesystem IO fails.
    pub fn put_database_value(
        &self,
        capabilities: &CapabilityTable,
        manifest: &DatabaseManifest,
        key: &str,
        value: serde_json::Value,
        context: PlatformContext,
    ) -> Result<DatabaseWriteResult, PlatformBackendError> {
        validate_database_key(key)?;
        let access = resolve_database_access(
            capabilities,
            manifest,
            PlatformOperation::DatabaseQuery,
            context,
        )?;
        let mut store =
            read_database_store(&access.resolved_path, PlatformOperation::DatabaseQuery)?;
        ensure_database_migrations_applied(&store, &manifest.migrations)?;
        store.values.insert(key.to_owned(), value.clone());
        write_database_store(
            &access.resolved_path,
            &store,
            PlatformOperation::DatabaseQuery,
        )?;
        Ok(DatabaseWriteResult {
            storage_path: access.resolved_path,
            key: key.to_owned(),
            value,
        })
    }

    /// Commits a batch of JSON value writes to a policy-approved database store.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformBackendError`] when capability policy, migration state, storage
    /// resolution, key validation, duplicate-key checks, JSON serialization, or filesystem IO
    /// fails. Validation happens before the store is read or written, so rejected transactions do
    /// not partially mutate persisted state.
    pub fn commit_database_transaction<I, K>(
        &self,
        capabilities: &CapabilityTable,
        manifest: &DatabaseManifest,
        writes: I,
        context: PlatformContext,
    ) -> Result<DatabaseTransactionResult, PlatformBackendError>
    where
        I: IntoIterator<Item = (K, serde_json::Value)>,
        K: Into<String>,
    {
        let staged = stage_database_transaction(writes)?;
        let access = resolve_database_access(
            capabilities,
            manifest,
            PlatformOperation::DatabaseQuery,
            context,
        )?;
        let mut store =
            read_database_store(&access.resolved_path, PlatformOperation::DatabaseQuery)?;
        ensure_database_migrations_applied(&store, &manifest.migrations)?;
        for (key, value) in &staged {
            store.values.insert(key.clone(), value.clone());
        }
        write_database_store(
            &access.resolved_path,
            &store,
            PlatformOperation::DatabaseQuery,
        )?;
        Ok(DatabaseTransactionResult {
            storage_path: access.resolved_path,
            written_keys: staged.into_iter().map(|(key, _)| key).collect(),
        })
    }

    /// Reads a JSON value from a policy-approved database store.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformBackendError`] when capability policy, migration state, storage
    /// resolution, key validation, JSON parsing, or filesystem IO fails.
    pub fn get_database_value(
        &self,
        capabilities: &CapabilityTable,
        manifest: &DatabaseManifest,
        key: &str,
        context: PlatformContext,
    ) -> Result<DatabaseReadResult, PlatformBackendError> {
        validate_database_key(key)?;
        let access = resolve_database_access(
            capabilities,
            manifest,
            PlatformOperation::DatabaseQuery,
            context,
        )?;
        let store = read_database_store(&access.resolved_path, PlatformOperation::DatabaseQuery)?;
        ensure_database_migrations_applied(&store, &manifest.migrations)?;
        Ok(DatabaseReadResult {
            storage_path: access.resolved_path,
            key: key.to_owned(),
            value: store.values.get(key).cloned(),
        })
    }
}

fn dialog_operation(kind: DialogKind) -> PlatformOperation {
    match kind {
        DialogKind::Message => PlatformOperation::DialogOpen,
        DialogKind::FilePicker => PlatformOperation::FilePickerOpen,
    }
}

fn is_safe_locale_segment(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
}

fn resolve_database_access(
    capabilities: &CapabilityTable,
    manifest: &DatabaseManifest,
    operation: PlatformOperation,
    context: PlatformContext,
) -> Result<FilesystemAccess, PlatformBackendError> {
    capabilities
        .ensure_allowed(&manifest.capability_key, operation, context)
        .map_err(|denied| PlatformBackendError::capability(operation, denied))?;
    DatabasePolicy::validate_migrations(&manifest.migrations)
        .map_err(|denied| PlatformBackendError::database(operation, denied))?;
    DatabasePolicy::validate_storage_path(&manifest.grant, &manifest.relative_path)
        .map_err(|denied| PlatformBackendError::database(operation, denied))?;
    FilesystemPolicy::resolve(&manifest.grant, &manifest.relative_path)
        .map_err(|denied| PlatformBackendError::filesystem(operation, denied))
}

fn read_database_store(
    storage_path: &str,
    operation: PlatformOperation,
) -> Result<DatabaseStore, PlatformBackendError> {
    match fs::read(storage_path) {
        Ok(bytes) if bytes.is_empty() => Ok(DatabaseStore::default()),
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| {
            PlatformBackendError::new(
                operation,
                "database.backend.parse-failed",
                format!("database store could not be parsed: {error}"),
            )
        }),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(DatabaseStore::default()),
        Err(error) => Err(PlatformBackendError::new(
            operation,
            "database.backend.read-failed",
            format!("database store could not be read: {error}"),
        )),
    }
}

fn write_database_store(
    storage_path: &str,
    store: &DatabaseStore,
    operation: PlatformOperation,
) -> Result<(), PlatformBackendError> {
    if let Some(parent) = Path::new(storage_path).parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PlatformBackendError::new(
                operation,
                "database.backend.create-parent-failed",
                format!("database parent directory could not be created: {error}"),
            )
        })?;
    }
    let bytes = serde_json::to_vec_pretty(store).map_err(|error| {
        PlatformBackendError::new(
            operation,
            "database.backend.serialize-failed",
            format!("database store could not be serialized: {error}"),
        )
    })?;
    fs::write(storage_path, bytes).map_err(|error| {
        PlatformBackendError::new(
            operation,
            "database.backend.write-failed",
            format!("database store could not be written: {error}"),
        )
    })
}

fn apply_database_migrations(
    store: &mut DatabaseStore,
    migrations: &[DatabaseMigration],
) -> Result<(), PlatformBackendError> {
    ensure_database_history_prefix(store, migrations)?;
    for migration in migrations.iter().skip(store.migrations.len()) {
        store.migrations.push(migration.clone());
    }
    Ok(())
}

fn ensure_database_migrations_applied(
    store: &DatabaseStore,
    migrations: &[DatabaseMigration],
) -> Result<(), PlatformBackendError> {
    ensure_database_history_prefix(store, migrations)?;
    if store.migrations.len() != migrations.len() {
        return Err(PlatformBackendError::new(
            PlatformOperation::DatabaseQuery,
            "database.backend.migrations-pending",
            "database migrations must be applied before query execution",
        ));
    }
    Ok(())
}

fn ensure_database_history_prefix(
    store: &DatabaseStore,
    migrations: &[DatabaseMigration],
) -> Result<(), PlatformBackendError> {
    if store.migrations.len() > migrations.len()
        || store
            .migrations
            .iter()
            .zip(migrations)
            .any(|(applied, expected)| applied != expected)
    {
        return Err(PlatformBackendError::new(
            PlatformOperation::DatabaseMigration,
            "database.backend.migration-history-mismatch",
            "database migration history does not match the manifest",
        ));
    }
    Ok(())
}

fn validate_database_key(key: &str) -> Result<(), PlatformBackendError> {
    if key.is_empty()
        || !key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        return Err(PlatformBackendError::new(
            PlatformOperation::DatabaseQuery,
            "database.key.invalid",
            format!("database key is not a stable identifier: {key}"),
        ));
    }
    Ok(())
}

fn stage_database_transaction<I, K>(
    writes: I,
) -> Result<Vec<(String, serde_json::Value)>, PlatformBackendError>
where
    I: IntoIterator<Item = (K, serde_json::Value)>,
    K: Into<String>,
{
    let mut seen = std::collections::BTreeSet::new();
    let mut staged = Vec::new();
    for (key, value) in writes {
        let key = key.into();
        validate_database_key(&key)?;
        if !seen.insert(key.clone()) {
            return Err(PlatformBackendError::new(
                PlatformOperation::DatabaseQuery,
                "database.transaction.duplicate-key",
                format!("database transaction contains duplicate key: {key}"),
            ));
        }
        staged.push((key, value));
    }
    Ok(staged)
}

//! Runtime permission context and explicit test backends for `Hawk2UI` capability ops.

use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt::Write as _,
    rc::Rc,
};

use serde::{Deserialize, Serialize};

/// Host context in which JavaScript is executing.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum HawkHostContext {
    /// Test-only context backed by explicit in-memory adapters.
    #[default]
    Test,
    /// Desktop UI context.
    Desktop,
    /// Plugin editor UI context.
    PluginUi,
    /// Realtime audio context. Capability ops that can block or allocate are denied.
    AudioRealtime,
}

/// Mockable network response returned by the explicit test backend.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HawkNetworkResponse {
    /// HTTP-like status code.
    pub status: u16,
    /// Response headers.
    pub headers: BTreeMap<String, String>,
    /// Text response body.
    pub body: String,
    /// Test-only response latency before the explicit backend resolves.
    #[serde(default, skip_serializing)]
    pub delay_ms: Option<u64>,
}

/// One network request observed by the explicit test backend.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HawkNetworkRequest {
    /// Request URL.
    pub url: String,
    /// HTTP method when provided.
    pub method: Option<String>,
    /// Request headers.
    pub headers: BTreeMap<String, String>,
    /// Request body when provided.
    pub body: Option<String>,
    /// Timeout in milliseconds when provided.
    pub timeout_ms: Option<u64>,
}

/// Redacted descriptor for an opaque host-side secret handle.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HawkSecretDescriptor {
    /// Declared secret name from the artifact capability manifest.
    pub name: String,
    /// Opaque handle identifier. This never contains the raw secret value.
    pub handle: String,
}

/// Desktop deep-link event delivered to JavaScript UI code.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HawkDesktopDeepLinkEvent {
    /// Declared URL scheme.
    pub scheme: String,
    /// Full deep-link URL received from the native host.
    pub url: String,
}

/// Desktop window mode command result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HawkDesktopWindowModeResult {
    /// Applied window mode.
    pub mode: String,
}

/// Desktop close command result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HawkDesktopCloseResult {
    /// Host close reason.
    pub reason: String,
}

/// One bounded audio meter frame delivered to JavaScript UI code.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HawkAudioMeterFrame {
    /// Declared meter source name.
    pub source: String,
    /// Meter values for the frame.
    pub values: Vec<f64>,
    /// Number of frames dropped before this delivery.
    pub dropped: u32,
}

/// UI-safe audio/MIDI/control input event delivered to JavaScript UI code.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HawkAudioControlEvent {
    /// Declared control source name.
    pub source: String,
    /// Stable event kind such as `note-on`, `cc`, or `pitch-bend`.
    pub kind: String,
    /// Raw numeric value from the host/control source.
    pub value: f64,
    /// Normalized value from 0.0 through 1.0 where supported.
    pub normalized_value: f64,
    /// Number of events dropped before this delivery.
    pub dropped: u32,
}

/// Result of queueing one UI-safe DSP control message.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HawkDspControlResult {
    /// Whether the control message was accepted into the bounded queue.
    pub accepted: bool,
    /// Queue depth after the operation.
    pub queue_depth: usize,
    /// Number of messages dropped by this queue.
    pub dropped: u32,
}

/// Result of applying one UI-safe DSP parameter graph update.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HawkDspParameterGraphUpdate {
    /// Monotonic host revision after the update is accepted.
    pub revision: u64,
    /// Number of graph nodes accepted by the host.
    pub node_count: usize,
    /// Number of graph edges accepted by the host.
    pub edge_count: usize,
}

/// DSP analysis job lifecycle result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HawkDspAnalysisJob {
    /// Stable host job identifier.
    pub id: String,
    /// Current job status.
    pub status: String,
}

/// DSP offline render job lifecycle result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HawkDspOfflineRenderJob {
    /// Stable host job identifier.
    pub id: String,
    /// Current job status.
    pub status: String,
}

/// DSP offline render export result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HawkDspOfflineRenderExport {
    /// Stable host job identifier.
    pub id: String,
    /// Current export status.
    pub status: String,
    /// Host-exported output path.
    pub path: String,
}

/// Result of one atomic JSON document transaction in `hawk:storage`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HawkStorageTransactionResult {
    /// Keys written by the transaction in commit order.
    pub written_keys: Vec<String>,
}

/// One file watch event delivered to JavaScript UI code.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HawkFileWatchEvent {
    /// Absolute granted file path.
    pub path: String,
    /// Stable event kind such as `modified`, `created`, or `deleted`.
    pub kind: String,
}

/// Result of importing a user-selected file into a declared app path.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HawkFileImportResult {
    /// User-selected source path.
    pub source_path: String,
    /// Declared destination path inside the app/plugin file surface.
    pub path: String,
    /// Number of bytes copied.
    pub bytes_written: usize,
}

/// Result of exporting a declared app file to a user-selected path.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HawkFileExportResult {
    /// Declared source path inside the app/plugin file surface.
    pub source_path: String,
    /// User-selected export path.
    pub path: String,
    /// Number of bytes copied.
    pub bytes_written: usize,
}

/// Plugin host transport and timeline snapshot delivered to JavaScript UI code.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HawkPluginTransportInfo {
    /// Whether the host transport is currently playing.
    pub playing: bool,
    /// Host sample rate.
    pub sample_rate: f64,
    /// Absolute sample position in the host timeline.
    pub sample_position: u64,
    /// Tempo in beats per minute.
    pub tempo_bpm: f64,
    /// Musical beat position.
    pub beat_position: f64,
    /// Time signature numerator.
    pub time_signature_numerator: u8,
    /// Time signature denominator.
    pub time_signature_denominator: u8,
}

/// Audio host transport and timeline snapshot delivered to JavaScript UI code.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HawkAudioTransportInfo {
    /// Whether the host transport is currently playing.
    pub playing: bool,
    /// Host sample rate.
    pub sample_rate: f64,
    /// Absolute sample position in the host timeline.
    pub sample_position: u64,
    /// Tempo in beats per minute.
    pub tempo_bpm: f64,
    /// Musical beat position.
    pub beat_position: f64,
    /// Time signature numerator.
    pub time_signature_numerator: u8,
    /// Time signature denominator.
    pub time_signature_denominator: u8,
}

/// Plugin editor size accepted by the host.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HawkPluginEditorSize {
    /// Editor width in physical or host-coordinated pixels.
    pub width: u32,
    /// Editor height in physical or host-coordinated pixels.
    pub height: u32,
}

/// Plugin editor focus result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HawkPluginEditorFocus {
    /// Whether focus was accepted by the host.
    pub focused: bool,
}

impl HawkNetworkResponse {
    /// Creates a JSON response for test backends.
    #[must_use]
    pub fn json(status: u16, body: impl Into<String>) -> Self {
        Self {
            status,
            headers: BTreeMap::from([("content-type".to_owned(), "application/json".to_owned())]),
            body: body.into(),
            delay_ms: None,
        }
    }

    /// Creates a text response for test backends.
    #[must_use]
    pub fn text(status: u16, body: impl Into<String>) -> Self {
        Self {
            status,
            headers: BTreeMap::from([("content-type".to_owned(), "text/plain".to_owned())]),
            body: body.into(),
            delay_ms: None,
        }
    }

    /// Adds test-only response latency for async timeout and cancellation coverage.
    #[must_use]
    pub fn with_delay_ms(mut self, delay_ms: u64) -> Self {
        self.delay_ms = Some(delay_ms);
        self
    }
}

/// Runtime capabilities and explicit in-memory backends.
#[derive(Clone, Debug, Default)]
pub struct HawkRuntimeCapabilities {
    inner: Rc<RefCell<HawkRuntimeCapabilityState>>,
}

#[derive(Debug, Default)]
struct HawkRuntimeCapabilityState {
    host_context: HawkHostContext,
    network_hosts: BTreeSet<String>,
    network_responses: BTreeMap<String, HawkNetworkResponse>,
    network_requests: Vec<HawkNetworkRequest>,
    network_body_limit_bytes: Option<usize>,
    ai_providers: BTreeSet<String>,
    ai_responses: BTreeMap<String, HawkNetworkResponse>,
    ai_stream_responses: BTreeMap<String, Vec<String>>,
    api_endpoints: BTreeSet<String>,
    api_responses: BTreeMap<String, HawkNetworkResponse>,
    secrets: BTreeSet<String>,
    secret_values: BTreeMap<String, String>,
    desktop_operations: BTreeSet<String>,
    open_dialog_results: VecDeque<Vec<String>>,
    clipboard_text: Option<String>,
    notifications: Vec<String>,
    external_urls: Vec<String>,
    shortcuts: BTreeSet<String>,
    deep_link_events: BTreeMap<String, VecDeque<String>>,
    desktop_window_mode: Option<String>,
    desktop_close_requests: Vec<String>,
    storage_namespaces: BTreeSet<String>,
    storage: BTreeMap<(String, String), String>,
    storage_documents: BTreeMap<(String, String), serde_json::Value>,
    file_paths: BTreeSet<String>,
    file_roots: BTreeSet<String>,
    files: BTreeMap<String, String>,
    file_bytes: BTreeMap<String, Vec<u8>>,
    file_pick_results: VecDeque<String>,
    file_pick_folder_results: VecDeque<String>,
    file_watch_events: BTreeMap<String, VecDeque<String>>,
    file_import_results: VecDeque<String>,
    file_export_results: VecDeque<String>,
    plugin_parameters: BTreeSet<String>,
    plugin_values: BTreeMap<String, f64>,
    plugin_automation_gestures: BTreeSet<String>,
    plugin_state_allowed: bool,
    plugin_state: Option<String>,
    plugin_presets: BTreeMap<String, String>,
    plugin_transport: Option<HawkPluginTransportInfo>,
    plugin_editor_size: Option<HawkPluginEditorSize>,
    plugin_editor_focused: bool,
    audio_transport: Option<HawkAudioTransportInfo>,
    audio_meter_streams: BTreeSet<String>,
    audio_meter_frames: BTreeMap<String, VecDeque<Vec<f64>>>,
    audio_meter_dropped: BTreeMap<String, u32>,
    audio_control_inputs: BTreeSet<String>,
    audio_control_events: BTreeMap<String, VecDeque<HawkAudioControlEvent>>,
    audio_control_dropped: BTreeMap<String, u32>,
    dsp_control_capacity: Option<usize>,
    dsp_control_queue: VecDeque<String>,
    dsp_control_dropped: u32,
    dsp_parameter_graph_allowed: bool,
    dsp_parameter_graph_revision: u64,
    dsp_parameter_graph: Option<serde_json::Value>,
    dsp_analysis_capacity: Option<usize>,
    dsp_analysis_jobs: BTreeMap<String, String>,
    dsp_analysis_next_id: u64,
    dsp_offline_render_capacity: Option<usize>,
    dsp_offline_render_jobs: BTreeMap<String, String>,
    dsp_offline_render_paths: BTreeMap<String, String>,
    dsp_offline_render_next_id: u64,
}

impl HawkRuntimeCapabilities {
    /// Creates a deny-by-default capability context.
    #[must_use]
    pub fn deny_all() -> Self {
        Self::default()
    }

    /// Creates a test capability context backed by explicit in-memory adapters.
    #[must_use]
    pub fn for_test() -> Self {
        Self::default().with_host_context(HawkHostContext::Test)
    }

    /// Sets the host context.
    #[must_use]
    pub fn with_host_context(self, host_context: HawkHostContext) -> Self {
        self.inner.borrow_mut().host_context = host_context;
        self
    }

    /// Allows requests to one network host.
    #[must_use]
    pub fn allow_network_host(self, host: impl Into<String>) -> Self {
        self.inner.borrow_mut().network_hosts.insert(host.into());
        self
    }

    /// Registers an explicit test network response for a URL.
    #[must_use]
    pub fn with_network_response(
        self,
        url: impl Into<String>,
        response: HawkNetworkResponse,
    ) -> Self {
        self.inner
            .borrow_mut()
            .network_responses
            .insert(url.into(), response);
        self
    }

    /// Returns network requests observed by the explicit test backend.
    #[must_use]
    pub fn network_requests(&self) -> Vec<HawkNetworkRequest> {
        self.inner.borrow().network_requests.clone()
    }

    /// Sets the maximum request and response body size for the explicit network backend.
    #[must_use]
    pub fn with_network_body_limit_bytes(self, limit_bytes: usize) -> Self {
        self.inner.borrow_mut().network_body_limit_bytes = Some(limit_bytes);
        self
    }

    /// Allows one AI provider.
    #[must_use]
    pub fn allow_ai_provider(self, provider: impl Into<String>) -> Self {
        self.inner.borrow_mut().ai_providers.insert(provider.into());
        self
    }

    /// Registers an explicit test response for an AI provider.
    #[must_use]
    pub fn with_ai_response(
        self,
        provider: impl Into<String>,
        response: HawkNetworkResponse,
    ) -> Self {
        self.inner
            .borrow_mut()
            .ai_responses
            .insert(provider.into(), response);
        self
    }

    /// Seeds an explicit test AI stream response for one declared provider.
    #[must_use]
    pub fn with_ai_stream_response(
        self,
        provider: impl Into<String>,
        chunks: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.inner.borrow_mut().ai_stream_responses.insert(
            provider.into(),
            chunks.into_iter().map(Into::into).collect(),
        );
        self
    }

    /// Allows one named API endpoint.
    #[must_use]
    pub fn allow_api_endpoint(self, endpoint: impl Into<String>) -> Self {
        self.inner
            .borrow_mut()
            .api_endpoints
            .insert(endpoint.into());
        self
    }

    /// Registers an explicit test response for a named API endpoint.
    #[must_use]
    pub fn with_api_response(
        self,
        endpoint: impl Into<String>,
        response: HawkNetworkResponse,
    ) -> Self {
        self.inner
            .borrow_mut()
            .api_responses
            .insert(endpoint.into(), response);
        self
    }

    /// Allows one named secret.
    #[must_use]
    pub fn allow_secret(self, name: impl Into<String>) -> Self {
        self.inner.borrow_mut().secrets.insert(name.into());
        self
    }

    /// Seeds a host-side secret value for tests. The value is never returned to JavaScript.
    #[must_use]
    pub fn with_secret_value(self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.inner
            .borrow_mut()
            .secret_values
            .insert(name.into(), value.into());
        self
    }

    /// Allows one desktop operation such as `hawk:desktop.showOpenDialog`.
    #[must_use]
    pub fn allow_desktop_operation(self, operation: impl Into<String>) -> Self {
        self.inner
            .borrow_mut()
            .desktop_operations
            .insert(operation.into());
        self
    }

    /// Seeds one file-dialog result for tests.
    #[must_use]
    pub fn with_open_dialog_result(self, paths: Vec<String>) -> Self {
        self.inner.borrow_mut().open_dialog_results.push_back(paths);
        self
    }

    /// Seeds desktop clipboard text for tests.
    #[must_use]
    pub fn with_clipboard_text(self, text: impl Into<String>) -> Self {
        self.inner.borrow_mut().clipboard_text = Some(text.into());
        self
    }

    /// Seeds one desktop deep-link event for tests.
    #[must_use]
    pub fn with_deep_link_event(self, scheme: impl Into<String>, url: impl Into<String>) -> Self {
        self.inner
            .borrow_mut()
            .deep_link_events
            .entry(scheme.into())
            .or_default()
            .push_back(url.into());
        self
    }

    /// Allows one storage namespace.
    #[must_use]
    pub fn allow_storage_namespace(self, namespace: impl Into<String>) -> Self {
        self.inner
            .borrow_mut()
            .storage_namespaces
            .insert(namespace.into());
        self
    }

    /// Seeds a JSON document for the explicit storage test backend.
    #[must_use]
    pub fn with_storage_document(
        self,
        namespace: impl Into<String>,
        key: impl Into<String>,
        value: serde_json::Value,
    ) -> Self {
        self.inner
            .borrow_mut()
            .storage_documents
            .insert((namespace.into(), key.into()), value);
        self
    }

    /// Seeds a storage value for tests.
    #[must_use]
    pub fn with_storage_value(
        self,
        namespace: impl Into<String>,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        self.inner
            .borrow_mut()
            .storage
            .insert((namespace.into(), key.into()), value.into());
        self
    }

    /// Allows access to one exact file path.
    #[must_use]
    pub fn allow_file_path(self, path: impl Into<String>) -> Self {
        self.inner.borrow_mut().file_paths.insert(path.into());
        self
    }

    /// Seeds file text for tests.
    #[must_use]
    pub fn with_file_text(self, path: impl Into<String>, text: impl Into<String>) -> Self {
        let path = path.into();
        let text = text.into();
        {
            let mut state = self.inner.borrow_mut();
            state
                .file_bytes
                .insert(path.clone(), text.as_bytes().to_vec());
            state.files.insert(path, text);
        }
        self
    }

    /// Seeds file bytes for tests.
    #[must_use]
    pub fn with_file_bytes(self, path: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        self.inner
            .borrow_mut()
            .file_bytes
            .insert(path.into(), bytes.into());
        self
    }

    /// Seeds one file picker result for tests.
    #[must_use]
    pub fn with_file_pick_result(self, path: impl Into<String>) -> Self {
        self.inner
            .borrow_mut()
            .file_pick_results
            .push_back(path.into());
        self
    }

    /// Seeds one folder picker result for tests.
    #[must_use]
    pub fn with_file_pick_folder_result(self, path: impl Into<String>) -> Self {
        self.inner
            .borrow_mut()
            .file_pick_folder_results
            .push_back(path.into());
        self
    }

    /// Seeds one file watch event for tests.
    #[must_use]
    pub fn with_file_watch_event(self, path: impl Into<String>, kind: impl Into<String>) -> Self {
        self.inner
            .borrow_mut()
            .file_watch_events
            .entry(path.into())
            .or_default()
            .push_back(kind.into());
        self
    }

    /// Seeds one user-selected file import source for tests.
    #[must_use]
    pub fn with_file_import_result(self, path: impl Into<String>) -> Self {
        self.inner
            .borrow_mut()
            .file_import_results
            .push_back(path.into());
        self
    }

    /// Seeds one user-selected file export destination for tests.
    #[must_use]
    pub fn with_file_export_result(self, path: impl Into<String>) -> Self {
        self.inner
            .borrow_mut()
            .file_export_results
            .push_back(path.into());
        self
    }

    /// Allows access to one plugin parameter.
    #[must_use]
    pub fn allow_plugin_parameter(self, parameter: impl Into<String>) -> Self {
        self.inner
            .borrow_mut()
            .plugin_parameters
            .insert(parameter.into());
        self
    }

    /// Seeds a plugin parameter value for tests.
    #[must_use]
    pub fn with_plugin_parameter(self, parameter: impl Into<String>, value: f64) -> Self {
        self.inner
            .borrow_mut()
            .plugin_values
            .insert(parameter.into(), value);
        self
    }

    /// Allows plugin state save/load operations.
    #[must_use]
    pub fn allow_plugin_state(self) -> Self {
        self.inner.borrow_mut().plugin_state_allowed = true;
        self
    }

    /// Seeds serialized plugin state for tests.
    #[must_use]
    pub fn with_plugin_state(self, state: impl Into<String>) -> Self {
        self.inner.borrow_mut().plugin_state = Some(state.into());
        self
    }

    /// Seeds a serialized plugin preset for tests.
    #[must_use]
    pub fn with_plugin_preset(
        self,
        preset_id: impl Into<String>,
        state: impl Into<String>,
    ) -> Self {
        self.inner
            .borrow_mut()
            .plugin_presets
            .insert(preset_id.into(), state.into());
        self
    }

    /// Seeds plugin host transport and timeline info for tests.
    #[must_use]
    pub fn with_plugin_transport_info(self, transport: HawkPluginTransportInfo) -> Self {
        self.inner.borrow_mut().plugin_transport = Some(transport);
        self
    }

    /// Allows one audio meter stream source.
    #[must_use]
    pub fn allow_audio_meter_stream(self, source: impl Into<String>) -> Self {
        self.inner
            .borrow_mut()
            .audio_meter_streams
            .insert(source.into());
        self
    }

    /// Seeds one audio meter frame for tests.
    #[must_use]
    pub fn with_audio_meter_frame(self, source: impl Into<String>, values: Vec<f64>) -> Self {
        self.inner
            .borrow_mut()
            .audio_meter_frames
            .entry(source.into())
            .or_default()
            .push_back(values);
        self
    }

    /// Allows one UI-safe audio/MIDI/control input source.
    #[must_use]
    pub fn allow_audio_control_input(self, source: impl Into<String>) -> Self {
        self.inner
            .borrow_mut()
            .audio_control_inputs
            .insert(source.into());
        self
    }

    /// Seeds one audio/MIDI/control input event for tests.
    #[must_use]
    pub fn with_audio_control_event(
        self,
        source: impl Into<String>,
        kind: impl Into<String>,
        value: impl Into<f64>,
        normalized_value: impl Into<f64>,
    ) -> Self {
        let source = source.into();
        self.inner
            .borrow_mut()
            .audio_control_events
            .entry(source.clone())
            .or_default()
            .push_back(HawkAudioControlEvent {
                source,
                kind: kind.into(),
                value: value.into(),
                normalized_value: normalized_value.into(),
                dropped: 0,
            });
        self
    }

    /// Seeds audio host transport and timeline info for tests.
    #[must_use]
    pub fn with_audio_transport_info(self, transport: HawkAudioTransportInfo) -> Self {
        self.inner.borrow_mut().audio_transport = Some(transport);
        self
    }

    /// Allows a bounded UI-to-DSP control queue.
    #[must_use]
    pub fn allow_dsp_control_queue(self, capacity: usize) -> Self {
        self.inner.borrow_mut().dsp_control_capacity = Some(capacity);
        self
    }

    /// Allows UI-safe DSP parameter graph updates.
    #[must_use]
    pub fn allow_dsp_parameter_graph_updates(self) -> Self {
        self.inner.borrow_mut().dsp_parameter_graph_allowed = true;
        self
    }

    /// Allows UI-safe DSP analysis jobs with a bounded in-flight capacity.
    #[must_use]
    pub fn allow_dsp_analysis_jobs(self, capacity: usize) -> Self {
        self.inner.borrow_mut().dsp_analysis_capacity = Some(capacity);
        self
    }

    /// Allows UI-safe DSP offline render jobs with a bounded in-flight capacity.
    #[must_use]
    pub fn allow_dsp_offline_render_jobs(self, capacity: usize) -> Self {
        self.inner.borrow_mut().dsp_offline_render_capacity = Some(capacity);
        self
    }

    pub(crate) fn network_request(
        &self,
        request: HawkNetworkRequest,
    ) -> Result<HawkNetworkResponse, CapabilityError> {
        self.deny_realtime("hawk:network.request")?;
        if matches!(request.timeout_ms, Some(0)) {
            return Err(CapabilityError::invalid(
                "hawk:network.request",
                "fetch timeoutMs must be greater than zero",
            ));
        }
        let host = network_host(&request.url)?;
        let mut state = self.inner.borrow_mut();
        if !state.network_hosts.contains(&host) {
            return Err(CapabilityError::denied(
                "hawk:network.request",
                format!("network host `{host}` is not declared in hawk.json capabilities"),
            ));
        }
        if let Some(limit_bytes) = state.network_body_limit_bytes {
            let body_bytes = request.body.as_deref().map(str::len).unwrap_or_default();
            if body_bytes > limit_bytes {
                return Err(CapabilityError::invalid(
                    "hawk:network.request",
                    format!(
                        "network request body exceeds configured byte limit of {limit_bytes} bytes"
                    ),
                ));
            }
        }
        let response = state
            .network_responses
            .get(&request.url)
            .cloned()
            .ok_or_else(|| {
                CapabilityError::unsupported(
                    "hawk:network.request",
                    format!(
                        "no explicit test network response is registered for `{}`",
                        request.url
                    ),
                )
            })?;
        state.network_requests.push(request);
        if let Some(limit_bytes) = state
            .network_body_limit_bytes
            .filter(|limit_bytes| response.body.len() > *limit_bytes)
        {
            return Err(CapabilityError::invalid(
                "hawk:network.request",
                format!(
                    "network response body exceeds configured byte limit of {limit_bytes} bytes"
                ),
            ));
        }
        Ok(response)
    }

    pub(crate) fn ai_call_provider(
        &self,
        provider: &str,
        secret_handles: &[String],
        budget_tokens: Option<u32>,
        timeout_ms: Option<u64>,
    ) -> Result<HawkNetworkResponse, CapabilityError> {
        self.require_ai_provider(
            "hawk:ai.callProvider",
            provider,
            secret_handles,
            budget_tokens,
            timeout_ms,
        )?;
        self.inner
            .borrow()
            .ai_responses
            .get(provider)
            .cloned()
            .ok_or_else(|| {
                CapabilityError::unsupported(
                    "hawk:ai.callProvider",
                    format!("no explicit test AI response is registered for `{provider}`"),
                )
            })
    }

    pub(crate) fn ai_stream_provider(
        &self,
        provider: &str,
        secret_handles: &[String],
        budget_tokens: Option<u32>,
        timeout_ms: Option<u64>,
    ) -> Result<Vec<String>, CapabilityError> {
        self.require_ai_provider(
            "hawk:ai.streamProvider",
            provider,
            secret_handles,
            budget_tokens,
            timeout_ms,
        )?;
        self.inner
            .borrow()
            .ai_stream_responses
            .get(provider)
            .cloned()
            .ok_or_else(|| {
                CapabilityError::unsupported(
                    "hawk:ai.streamProvider",
                    format!("no explicit test AI stream response is registered for `{provider}`"),
                )
            })
    }

    fn require_ai_provider(
        &self,
        operation: &'static str,
        provider: &str,
        secret_handles: &[String],
        budget_tokens: Option<u32>,
        timeout_ms: Option<u64>,
    ) -> Result<(), CapabilityError> {
        self.require_ui_safe_capability(operation)?;
        if provider.trim().is_empty() {
            return Err(CapabilityError::denied(
                operation,
                "AI provider name must not be empty",
            ));
        }
        if matches!(budget_tokens, Some(0)) {
            return Err(CapabilityError::denied(
                operation,
                "AI provider budgetTokens must be greater than zero when provided",
            ));
        }
        if matches!(timeout_ms, Some(0)) {
            return Err(CapabilityError::denied(
                operation,
                "AI provider timeoutMs must be greater than zero when provided",
            ));
        }
        let state = self.inner.borrow();
        if !state.ai_providers.contains(provider) {
            return Err(CapabilityError::denied(
                operation,
                format!("AI provider `{provider}` is not declared in hawk.json capabilities"),
            ));
        }
        for secret in secret_handles {
            if !state.secrets.contains(secret) {
                return Err(CapabilityError::denied(
                    operation,
                    format!(
                        "secret `{secret}` is not declared in hawk.json capabilities for AI provider `{provider}`"
                    ),
                ));
            }
            if !state.secret_values.contains_key(secret) {
                return Err(CapabilityError::unsupported(
                    operation,
                    format!("no explicit test secret value is registered for `{secret}`"),
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn api_call(
        &self,
        endpoint: &str,
        secret_handles: &[String],
    ) -> Result<HawkNetworkResponse, CapabilityError> {
        self.deny_realtime("hawk:api.call")?;
        if endpoint.trim().is_empty() {
            return Err(CapabilityError::denied(
                "hawk:api.call",
                "named API endpoint must not be empty",
            ));
        }
        let state = self.inner.borrow();
        if !state.api_endpoints.contains(endpoint) {
            return Err(CapabilityError::denied(
                "hawk:api.call",
                format!(
                    "named API endpoint `{endpoint}` is not declared in hawk.json capabilities"
                ),
            ));
        }
        for secret in secret_handles {
            if !state.secrets.contains(secret) {
                return Err(CapabilityError::denied(
                    "hawk:api.call",
                    format!(
                        "secret `{secret}` is not declared in hawk.json capabilities for named API endpoint `{endpoint}`"
                    ),
                ));
            }
            if !state.secret_values.contains_key(secret) {
                return Err(CapabilityError::unsupported(
                    "hawk:api.call",
                    format!("no explicit test secret value is registered for `{secret}`"),
                ));
            }
        }
        state.api_responses.get(endpoint).cloned().ok_or_else(|| {
            CapabilityError::unsupported(
                "hawk:api.call",
                format!("no explicit test API response is registered for `{endpoint}`"),
            )
        })
    }

    pub(crate) fn secret_read(&self, name: &str) -> Result<HawkSecretDescriptor, CapabilityError> {
        self.deny_realtime("hawk:secrets.read")?;
        if name.trim().is_empty() {
            return Err(CapabilityError::denied(
                "hawk:secrets.read",
                "secret name must not be empty",
            ));
        }
        let state = self.inner.borrow();
        if !state.secrets.contains(name) {
            return Err(CapabilityError::denied(
                "hawk:secrets.read",
                format!("secret `{name}` is not declared in hawk.json capabilities"),
            ));
        }
        if !state.secret_values.contains_key(name) {
            return Err(CapabilityError::unsupported(
                "hawk:secrets.read",
                format!("no explicit test secret value is registered for `{name}`"),
            ));
        }
        Ok(HawkSecretDescriptor {
            name: name.to_owned(),
            handle: format!("hawk-secret:{}", stable_secret_handle_suffix(name)),
        })
    }

    pub(crate) fn desktop_set_window_title(&self, title: &str) -> Result<(), CapabilityError> {
        self.require_desktop_operation("hawk:desktop.setWindowTitle")?;
        if title.trim().is_empty() {
            return Err(CapabilityError::denied(
                "hawk:desktop.setWindowTitle",
                "window title must not be empty",
            ));
        }
        Ok(())
    }

    pub(crate) fn desktop_show_open_dialog(&self) -> Result<Vec<String>, CapabilityError> {
        self.require_desktop_operation("hawk:desktop.showOpenDialog")?;
        self.inner
            .borrow_mut()
            .open_dialog_results
            .pop_front()
            .ok_or_else(|| {
                CapabilityError::unsupported(
                    "hawk:desktop.showOpenDialog",
                    "no explicit test open-dialog result is registered",
                )
            })
    }

    pub(crate) fn desktop_read_clipboard(&self) -> Result<String, CapabilityError> {
        self.require_desktop_operation("hawk:desktop.readClipboard")?;
        self.inner.borrow().clipboard_text.clone().ok_or_else(|| {
            CapabilityError::unsupported(
                "hawk:desktop.readClipboard",
                "no explicit test clipboard text is registered",
            )
        })
    }

    pub(crate) fn desktop_write_clipboard(&self, text: String) -> Result<(), CapabilityError> {
        self.require_desktop_operation("hawk:desktop.writeClipboard")?;
        self.inner.borrow_mut().clipboard_text = Some(text);
        Ok(())
    }

    pub(crate) fn desktop_notify(&self, title: &str, body: &str) -> Result<(), CapabilityError> {
        self.require_desktop_operation("hawk:desktop.notify")?;
        if title.trim().is_empty() {
            return Err(CapabilityError::denied(
                "hawk:desktop.notify",
                "notification title must not be empty",
            ));
        }
        self.inner
            .borrow_mut()
            .notifications
            .push(format!("{title}\n{body}"));
        Ok(())
    }

    pub(crate) fn desktop_register_shortcut(
        &self,
        shortcut: &str,
    ) -> Result<String, CapabilityError> {
        self.require_desktop_operation("hawk:desktop.registerShortcut")?;
        if shortcut.trim().is_empty() {
            return Err(CapabilityError::denied(
                "hawk:desktop.registerShortcut",
                "shortcut must not be empty",
            ));
        }
        self.inner
            .borrow_mut()
            .shortcuts
            .insert(shortcut.to_owned());
        Ok(format!("hawk-shortcut:{shortcut}"))
    }

    pub(crate) fn desktop_open_external(&self, url: &str) -> Result<(), CapabilityError> {
        self.require_desktop_operation("hawk:desktop.openExternal")?;
        if !url.starts_with("https://") && !url.starts_with("http://") {
            return Err(CapabilityError::denied(
                "hawk:desktop.openExternal",
                format!("external URL `{url}` must use http or https"),
            ));
        }
        self.inner.borrow_mut().external_urls.push(url.to_owned());
        Ok(())
    }

    pub(crate) fn desktop_next_deep_link(
        &self,
        scheme: &str,
    ) -> Result<HawkDesktopDeepLinkEvent, CapabilityError> {
        self.require_desktop_operation("hawk:desktop.onDeepLink")?;
        if scheme.trim().is_empty() {
            return Err(CapabilityError::invalid(
                "hawk:desktop.onDeepLink",
                "deep-link scheme must not be empty",
            ));
        }
        let mut state = self.inner.borrow_mut();
        let url = state
            .deep_link_events
            .get_mut(scheme)
            .and_then(VecDeque::pop_front)
            .ok_or_else(|| {
                CapabilityError::unsupported(
                    "hawk:desktop.onDeepLink",
                    format!("no explicit test deep-link event is registered for `{scheme}`"),
                )
            })?;
        if !url.starts_with(&format!("{scheme}:")) {
            return Err(CapabilityError::invalid(
                "hawk:desktop.onDeepLink",
                format!("deep-link URL `{url}` does not match scheme `{scheme}`"),
            ));
        }
        Ok(HawkDesktopDeepLinkEvent {
            scheme: scheme.to_owned(),
            url,
        })
    }

    pub(crate) fn desktop_set_window_mode(
        &self,
        mode: &str,
    ) -> Result<HawkDesktopWindowModeResult, CapabilityError> {
        self.require_desktop_operation("hawk:desktop.setWindowMode")?;
        let mode = mode.trim();
        match mode {
            "normal" | "minimized" | "maximized" | "fullscreen" => {}
            _ => {
                return Err(CapabilityError::invalid(
                    "hawk:desktop.setWindowMode",
                    format!(
                        "window mode `{mode}` must be normal, minimized, maximized, or fullscreen"
                    ),
                ));
            }
        }
        self.inner.borrow_mut().desktop_window_mode = Some(mode.to_owned());
        Ok(HawkDesktopWindowModeResult {
            mode: mode.to_owned(),
        })
    }

    pub(crate) fn desktop_close_window(
        &self,
        reason: &str,
    ) -> Result<HawkDesktopCloseResult, CapabilityError> {
        self.require_desktop_operation("hawk:desktop.closeWindow")?;
        let reason = reason.trim();
        if reason.is_empty() {
            return Err(CapabilityError::invalid(
                "hawk:desktop.closeWindow",
                "window close reason must not be empty",
            ));
        }
        self.inner
            .borrow_mut()
            .desktop_close_requests
            .push(reason.to_owned());
        Ok(HawkDesktopCloseResult {
            reason: reason.to_owned(),
        })
    }

    pub(crate) fn storage_get(
        &self,
        namespace: &str,
        key: &str,
    ) -> Result<Option<String>, CapabilityError> {
        self.deny_realtime("hawk:storage.getItem")?;
        let state = self.inner.borrow();
        if !state.storage_namespaces.contains(namespace) {
            return Err(CapabilityError::denied(
                "hawk:storage.getItem",
                format!(
                    "storage namespace `{namespace}` is not declared in hawk.json capabilities"
                ),
            ));
        }
        Ok(state
            .storage
            .get(&(namespace.to_owned(), key.to_owned()))
            .cloned())
    }

    pub(crate) fn storage_set(
        &self,
        namespace: &str,
        key: &str,
        value: String,
    ) -> Result<(), CapabilityError> {
        self.deny_realtime("hawk:storage.setItem")?;
        let mut state = self.inner.borrow_mut();
        if !state.storage_namespaces.contains(namespace) {
            return Err(CapabilityError::denied(
                "hawk:storage.setItem",
                format!(
                    "storage namespace `{namespace}` is not declared in hawk.json capabilities"
                ),
            ));
        }
        state
            .storage
            .insert((namespace.to_owned(), key.to_owned()), value);
        Ok(())
    }

    pub(crate) fn storage_get_document(
        &self,
        namespace: &str,
        key: &str,
    ) -> Result<serde_json::Value, CapabilityError> {
        const OPERATION: &str = "hawk:storage.getDocument";
        self.deny_realtime(OPERATION)?;
        validate_storage_document_key(OPERATION, key)?;
        let state = self.inner.borrow();
        if !state.storage_namespaces.contains(namespace) {
            return Err(CapabilityError::denied(
                OPERATION,
                format!(
                    "storage namespace `{namespace}` is not declared in hawk.json capabilities"
                ),
            ));
        }
        Ok(state
            .storage_documents
            .get(&(namespace.to_owned(), key.to_owned()))
            .cloned()
            .unwrap_or(serde_json::Value::Null))
    }

    pub(crate) fn storage_put_document(
        &self,
        namespace: &str,
        key: &str,
        value: serde_json::Value,
    ) -> Result<serde_json::Value, CapabilityError> {
        const OPERATION: &str = "hawk:storage.putDocument";
        self.deny_realtime(OPERATION)?;
        validate_storage_document_key(OPERATION, key)?;
        let mut state = self.inner.borrow_mut();
        if !state.storage_namespaces.contains(namespace) {
            return Err(CapabilityError::denied(
                OPERATION,
                format!(
                    "storage namespace `{namespace}` is not declared in hawk.json capabilities"
                ),
            ));
        }
        state
            .storage_documents
            .insert((namespace.to_owned(), key.to_owned()), value.clone());
        Ok(value)
    }

    pub(crate) fn storage_transaction<I, K>(
        &self,
        namespace: &str,
        writes: I,
    ) -> Result<HawkStorageTransactionResult, CapabilityError>
    where
        I: IntoIterator<Item = (K, serde_json::Value)>,
        K: Into<String>,
    {
        const OPERATION: &str = "hawk:storage.transaction";
        self.deny_realtime(OPERATION)?;
        let mut seen = BTreeSet::new();
        let mut staged = Vec::new();
        for (key, value) in writes {
            let key = key.into();
            validate_storage_document_key(OPERATION, &key)?;
            if !seen.insert(key.clone()) {
                return Err(CapabilityError::invalid(
                    OPERATION,
                    format!("storage transaction contains duplicate key: {key}"),
                ));
            }
            staged.push((key, value));
        }

        let mut state = self.inner.borrow_mut();
        if !state.storage_namespaces.contains(namespace) {
            return Err(CapabilityError::denied(
                OPERATION,
                format!(
                    "storage namespace `{namespace}` is not declared in hawk.json capabilities"
                ),
            ));
        }
        for (key, value) in &staged {
            state
                .storage_documents
                .insert((namespace.to_owned(), key.clone()), value.clone());
        }
        Ok(HawkStorageTransactionResult {
            written_keys: staged.into_iter().map(|(key, _)| key).collect(),
        })
    }

    pub(crate) fn file_read_text(&self, path: &str) -> Result<String, CapabilityError> {
        self.deny_realtime("hawk:files.readText")?;
        validate_file_grant_path("hawk:files.readText", path)?;
        let state = self.inner.borrow();
        if !file_path_is_granted(&state, path) {
            return Err(CapabilityError::denied(
                "hawk:files.readText",
                format!("file path `{path}` is not declared or user-granted"),
            ));
        }
        state.files.get(path).cloned().ok_or_else(|| {
            CapabilityError::unsupported(
                "hawk:files.readText",
                format!("no explicit test file text is registered for `{path}`"),
            )
        })
    }

    pub(crate) fn file_write_text(&self, path: &str, text: String) -> Result<(), CapabilityError> {
        self.deny_realtime("hawk:files.writeText")?;
        validate_file_grant_path("hawk:files.writeText", path)?;
        let mut state = self.inner.borrow_mut();
        if !file_path_is_granted(&state, path) {
            return Err(CapabilityError::denied(
                "hawk:files.writeText",
                format!("file path `{path}` is not declared or user-granted"),
            ));
        }
        state
            .file_bytes
            .insert(path.to_owned(), text.as_bytes().to_vec());
        state.files.insert(path.to_owned(), text);
        Ok(())
    }

    pub(crate) fn file_read_bytes(&self, path: &str) -> Result<Vec<u8>, CapabilityError> {
        self.deny_realtime("hawk:files.readBytes")?;
        validate_file_grant_path("hawk:files.readBytes", path)?;
        let state = self.inner.borrow();
        if !file_path_is_granted(&state, path) {
            return Err(CapabilityError::denied(
                "hawk:files.readBytes",
                format!("file path `{path}` is not declared or user-granted"),
            ));
        }
        state.file_bytes.get(path).cloned().ok_or_else(|| {
            CapabilityError::unsupported(
                "hawk:files.readBytes",
                format!("no explicit test file bytes are registered for `{path}`"),
            )
        })
    }

    pub(crate) fn file_write_bytes(
        &self,
        path: &str,
        bytes: Vec<u8>,
    ) -> Result<(), CapabilityError> {
        self.deny_realtime("hawk:files.writeBytes")?;
        validate_file_grant_path("hawk:files.writeBytes", path)?;
        let mut state = self.inner.borrow_mut();
        if !file_path_is_granted(&state, path) {
            return Err(CapabilityError::denied(
                "hawk:files.writeBytes",
                format!("file path `{path}` is not declared or user-granted"),
            ));
        }
        if let Ok(text) = String::from_utf8(bytes.clone()) {
            state.files.insert(path.to_owned(), text);
        } else {
            state.files.remove(path);
        }
        state.file_bytes.insert(path.to_owned(), bytes);
        Ok(())
    }

    pub(crate) fn file_pick(&self) -> Result<String, CapabilityError> {
        self.deny_realtime("hawk:files.pickFile")?;
        let mut state = self.inner.borrow_mut();
        let path = state.file_pick_results.pop_front().ok_or_else(|| {
            CapabilityError::unsupported(
                "hawk:files.pickFile",
                "no explicit test file picker result is registered",
            )
        })?;
        validate_file_grant_path("hawk:files.pickFile", &path)?;
        state.file_paths.insert(path.clone());
        Ok(path)
    }

    pub(crate) fn file_pick_folder(&self) -> Result<String, CapabilityError> {
        self.deny_realtime("hawk:files.pickFolder")?;
        let mut state = self.inner.borrow_mut();
        let path = state.file_pick_folder_results.pop_front().ok_or_else(|| {
            CapabilityError::unsupported(
                "hawk:files.pickFolder",
                "no explicit test folder picker result is registered",
            )
        })?;
        validate_file_grant_path("hawk:files.pickFolder", &path)?;
        let root = normalize_file_root(&path);
        state.file_roots.insert(root.clone());
        Ok(root)
    }

    pub(crate) fn file_watch(&self, path: &str) -> Result<HawkFileWatchEvent, CapabilityError> {
        self.deny_realtime("hawk:files.watch")?;
        validate_file_grant_path("hawk:files.watch", path)?;
        let mut state = self.inner.borrow_mut();
        if !file_path_is_granted(&state, path) {
            return Err(CapabilityError::denied(
                "hawk:files.watch",
                format!("file path `{path}` is not declared or user-granted for watching"),
            ));
        }
        let kind = state
            .file_watch_events
            .get_mut(path)
            .and_then(VecDeque::pop_front)
            .ok_or_else(|| {
                CapabilityError::unsupported(
                    "hawk:files.watch",
                    format!("no explicit test file watch event is registered for `{path}`"),
                )
            })?;
        Ok(HawkFileWatchEvent {
            path: path.to_owned(),
            kind,
        })
    }

    pub(crate) fn file_import(
        &self,
        destination_path: &str,
    ) -> Result<HawkFileImportResult, CapabilityError> {
        self.deny_realtime("hawk:files.importFile")?;
        validate_file_grant_path("hawk:files.importFile", destination_path)?;
        let mut state = self.inner.borrow_mut();
        if !file_path_is_granted(&state, destination_path) {
            return Err(CapabilityError::denied(
                "hawk:files.importFile",
                format!("destination path `{destination_path}` is not declared or user-granted"),
            ));
        }
        let source_path = state.file_import_results.pop_front().ok_or_else(|| {
            CapabilityError::unsupported(
                "hawk:files.importFile",
                "no explicit test file import result is registered",
            )
        })?;
        validate_file_grant_path("hawk:files.importFile", &source_path)?;
        let bytes = state.file_bytes.get(&source_path).cloned().ok_or_else(|| {
            CapabilityError::unsupported(
                "hawk:files.importFile",
                format!("no explicit test file bytes are registered for `{source_path}`"),
            )
        })?;
        state.file_paths.insert(source_path.clone());
        if let Ok(text) = String::from_utf8(bytes.clone()) {
            state.files.insert(destination_path.to_owned(), text);
        } else {
            state.files.remove(destination_path);
        }
        state
            .file_bytes
            .insert(destination_path.to_owned(), bytes.clone());
        Ok(HawkFileImportResult {
            source_path,
            path: destination_path.to_owned(),
            bytes_written: bytes.len(),
        })
    }

    pub(crate) fn file_export(
        &self,
        source_path: &str,
    ) -> Result<HawkFileExportResult, CapabilityError> {
        self.deny_realtime("hawk:files.exportFile")?;
        validate_file_grant_path("hawk:files.exportFile", source_path)?;
        let mut state = self.inner.borrow_mut();
        if !file_path_is_granted(&state, source_path) {
            return Err(CapabilityError::denied(
                "hawk:files.exportFile",
                format!("source path `{source_path}` is not declared or user-granted"),
            ));
        }
        let bytes = state.file_bytes.get(source_path).cloned().ok_or_else(|| {
            CapabilityError::unsupported(
                "hawk:files.exportFile",
                format!("no explicit test file bytes are registered for `{source_path}`"),
            )
        })?;
        let destination_path = state.file_export_results.pop_front().ok_or_else(|| {
            CapabilityError::unsupported(
                "hawk:files.exportFile",
                "no explicit test file export result is registered",
            )
        })?;
        validate_file_grant_path("hawk:files.exportFile", &destination_path)?;
        state.file_paths.insert(destination_path.clone());
        if let Ok(text) = String::from_utf8(bytes.clone()) {
            state.files.insert(destination_path.clone(), text);
        } else {
            state.files.remove(&destination_path);
        }
        state
            .file_bytes
            .insert(destination_path.clone(), bytes.clone());
        Ok(HawkFileExportResult {
            source_path: source_path.to_owned(),
            path: destination_path,
            bytes_written: bytes.len(),
        })
    }

    pub(crate) fn plugin_read_parameter(&self, parameter: &str) -> Result<f64, CapabilityError> {
        self.require_plugin_parameter("hawk:plugin.readParameter", parameter)?;
        let state = self.inner.borrow();
        Ok(*state.plugin_values.get(parameter).unwrap_or(&0.0))
    }

    pub(crate) fn plugin_write_parameter(
        &self,
        parameter: &str,
        value: f64,
    ) -> Result<(), CapabilityError> {
        self.require_plugin_parameter("hawk:plugin.writeParameter", parameter)?;
        let mut state = self.inner.borrow_mut();
        state.plugin_values.insert(parameter.to_owned(), value);
        Ok(())
    }

    pub(crate) fn plugin_begin_automation_gesture(
        &self,
        parameter: &str,
    ) -> Result<String, CapabilityError> {
        self.require_plugin_parameter("hawk:plugin.beginAutomationGesture", parameter)?;
        self.inner
            .borrow_mut()
            .plugin_automation_gestures
            .insert(parameter.to_owned());
        Ok(format!("hawk-automation:{parameter}:begin"))
    }

    pub(crate) fn plugin_end_automation_gesture(
        &self,
        parameter: &str,
    ) -> Result<String, CapabilityError> {
        self.require_plugin_parameter("hawk:plugin.endAutomationGesture", parameter)?;
        self.inner
            .borrow_mut()
            .plugin_automation_gestures
            .remove(parameter);
        Ok(format!("hawk-automation:{parameter}:end"))
    }

    pub(crate) fn plugin_load_state(&self) -> Result<String, CapabilityError> {
        self.require_plugin_state("hawk:plugin.loadState")?;
        Ok(self.inner.borrow().plugin_state.clone().unwrap_or_default())
    }

    pub(crate) fn plugin_save_state(&self, state_blob: String) -> Result<(), CapabilityError> {
        self.require_plugin_state("hawk:plugin.saveState")?;
        if state_blob.trim().is_empty() {
            return Err(CapabilityError::denied(
                "hawk:plugin.saveState",
                "plugin state blob must not be empty",
            ));
        }
        self.inner.borrow_mut().plugin_state = Some(state_blob);
        Ok(())
    }

    pub(crate) fn plugin_load_preset(&self, preset_id: &str) -> Result<String, CapabilityError> {
        self.require_plugin_preset("hawk:plugin.loadPreset", preset_id)?;
        self.inner
            .borrow()
            .plugin_presets
            .get(preset_id)
            .cloned()
            .ok_or_else(|| {
                CapabilityError::unsupported(
                    "hawk:plugin.loadPreset",
                    format!("no explicit test plugin preset is registered for `{preset_id}`"),
                )
            })
    }

    pub(crate) fn plugin_save_preset(
        &self,
        preset_id: &str,
        state_blob: String,
    ) -> Result<(), CapabilityError> {
        self.require_plugin_preset("hawk:plugin.savePreset", preset_id)?;
        if state_blob.trim().is_empty() {
            return Err(CapabilityError::denied(
                "hawk:plugin.savePreset",
                "plugin preset blob must not be empty",
            ));
        }
        self.inner
            .borrow_mut()
            .plugin_presets
            .insert(preset_id.to_owned(), state_blob);
        Ok(())
    }

    pub(crate) fn plugin_get_transport(&self) -> Result<HawkPluginTransportInfo, CapabilityError> {
        self.require_plugin_ui("hawk:plugin.getTransport")?;
        self.inner.borrow().plugin_transport.clone().ok_or_else(|| {
            CapabilityError::unsupported(
                "hawk:plugin.getTransport",
                "no explicit test plugin transport info is registered",
            )
        })
    }

    pub(crate) fn plugin_resize_editor(
        &self,
        width: u32,
        height: u32,
    ) -> Result<HawkPluginEditorSize, CapabilityError> {
        self.require_plugin_ui("hawk:plugin.resizeEditor")?;
        if width == 0 || height == 0 {
            return Err(CapabilityError::invalid(
                "hawk:plugin.resizeEditor",
                "editor width and height must be positive integers",
            ));
        }
        let size = HawkPluginEditorSize { width, height };
        self.inner.borrow_mut().plugin_editor_size = Some(size.clone());
        Ok(size)
    }

    pub(crate) fn plugin_focus_editor(&self) -> Result<HawkPluginEditorFocus, CapabilityError> {
        self.require_plugin_ui("hawk:plugin.focusEditor")?;
        self.inner.borrow_mut().plugin_editor_focused = true;
        Ok(HawkPluginEditorFocus { focused: true })
    }

    pub(crate) fn audio_subscribe_meters(
        &self,
        source: &str,
    ) -> Result<HawkAudioMeterFrame, CapabilityError> {
        self.require_ui_safe_capability("hawk:audio.subscribeMeters")?;
        if source.trim().is_empty() {
            return Err(CapabilityError::denied(
                "hawk:audio.subscribeMeters",
                "meter source must not be empty",
            ));
        }
        let mut state = self.inner.borrow_mut();
        if !state.audio_meter_streams.contains(source) {
            return Err(CapabilityError::denied(
                "hawk:audio.subscribeMeters",
                format!("audio meter stream `{source}` is not declared in hawk.json capabilities"),
            ));
        }
        let dropped = *state.audio_meter_dropped.get(source).unwrap_or(&0);
        let values = state
            .audio_meter_frames
            .get_mut(source)
            .and_then(VecDeque::pop_front)
            .ok_or_else(|| {
                CapabilityError::unsupported(
                    "hawk:audio.subscribeMeters",
                    format!("no explicit test meter frame is registered for `{source}`"),
                )
            })?;
        Ok(HawkAudioMeterFrame {
            source: source.to_owned(),
            values,
            dropped,
        })
    }

    pub(crate) fn audio_transport(&self) -> Result<HawkAudioTransportInfo, CapabilityError> {
        self.require_ui_safe_capability("hawk:audio.transport")?;
        self.inner.borrow().audio_transport.clone().ok_or_else(|| {
            CapabilityError::unsupported(
                "hawk:audio.transport",
                "no explicit test audio transport info is registered",
            )
        })
    }

    pub(crate) fn audio_next_control(
        &self,
        source: &str,
    ) -> Result<HawkAudioControlEvent, CapabilityError> {
        self.require_ui_safe_capability("hawk:audio.nextControl")?;
        if source.trim().is_empty() {
            return Err(CapabilityError::denied(
                "hawk:audio.nextControl",
                "control source must not be empty",
            ));
        }
        let mut state = self.inner.borrow_mut();
        if !state.audio_control_inputs.contains(source) {
            return Err(CapabilityError::denied(
                "hawk:audio.nextControl",
                format!("audio control input `{source}` is not declared in hawk.json capabilities"),
            ));
        }
        let dropped = *state.audio_control_dropped.get(source).unwrap_or(&0);
        let mut event = state
            .audio_control_events
            .get_mut(source)
            .and_then(VecDeque::pop_front)
            .ok_or_else(|| {
                CapabilityError::unsupported(
                    "hawk:audio.nextControl",
                    format!("no explicit test audio control event is registered for `{source}`"),
                )
            })?;
        event.dropped = dropped;
        Ok(event)
    }

    pub(crate) fn dsp_update_parameter_graph(
        &self,
        graph: &serde_json::Value,
    ) -> Result<HawkDspParameterGraphUpdate, CapabilityError> {
        self.require_ui_safe_capability("hawk:dsp.updateParameterGraph")?;
        if !self.inner.borrow().dsp_parameter_graph_allowed {
            return Err(CapabilityError::denied(
                "hawk:dsp.updateParameterGraph",
                "DSP parameter graph updates are not declared in hawk.json capabilities",
            ));
        }

        let nodes = graph
            .get("nodes")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                CapabilityError::invalid(
                    "hawk:dsp.updateParameterGraph",
                    "parameter graph must include a nodes array",
                )
            })?;
        let edges = graph
            .get("edges")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                CapabilityError::invalid(
                    "hawk:dsp.updateParameterGraph",
                    "parameter graph must include an edges array",
                )
            })?;

        let mut node_ids = BTreeSet::new();
        for node in nodes {
            let id = node
                .get("id")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    CapabilityError::invalid(
                        "hawk:dsp.updateParameterGraph",
                        "parameter graph nodes must include string ids",
                    )
                })?;
            if id.trim().is_empty() {
                return Err(CapabilityError::invalid(
                    "hawk:dsp.updateParameterGraph",
                    "parameter graph node ids must not be empty",
                ));
            }
            if !node_ids.insert(id.to_owned()) {
                return Err(CapabilityError::invalid(
                    "hawk:dsp.updateParameterGraph",
                    format!("parameter graph node id `{id}` is duplicated"),
                ));
            }
        }

        for edge in edges {
            let from = edge
                .get("from")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    CapabilityError::invalid(
                        "hawk:dsp.updateParameterGraph",
                        "parameter graph edges must include string from endpoints",
                    )
                })?;
            let to = edge
                .get("to")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    CapabilityError::invalid(
                        "hawk:dsp.updateParameterGraph",
                        "parameter graph edges must include string to endpoints",
                    )
                })?;
            if from.trim().is_empty() || to.trim().is_empty() {
                return Err(CapabilityError::invalid(
                    "hawk:dsp.updateParameterGraph",
                    "parameter graph edge endpoints must not be empty",
                ));
            }
            if !node_ids.contains(from) {
                return Err(CapabilityError::invalid(
                    "hawk:dsp.updateParameterGraph",
                    format!("parameter graph edge source `{from}` is not declared as a node"),
                ));
            }
            if !node_ids.contains(to) {
                return Err(CapabilityError::invalid(
                    "hawk:dsp.updateParameterGraph",
                    format!("parameter graph edge target `{to}` is not declared as a node"),
                ));
            }
        }

        let mut state = self.inner.borrow_mut();
        state.dsp_parameter_graph_revision = state.dsp_parameter_graph_revision.saturating_add(1);
        state.dsp_parameter_graph = Some(graph.clone());
        Ok(HawkDspParameterGraphUpdate {
            revision: state.dsp_parameter_graph_revision,
            node_count: nodes.len(),
            edge_count: edges.len(),
        })
    }

    pub(crate) fn dsp_send_control(
        &self,
        message: &serde_json::Value,
    ) -> Result<HawkDspControlResult, CapabilityError> {
        self.require_ui_safe_capability("hawk:dsp.sendControl")?;
        let mut state = self.inner.borrow_mut();
        let capacity = state.dsp_control_capacity.ok_or_else(|| {
            CapabilityError::denied(
                "hawk:dsp.sendControl",
                "DSP control queue is not declared in hawk.json capabilities",
            )
        })?;
        if state.dsp_control_queue.len() >= capacity {
            state.dsp_control_dropped = state.dsp_control_dropped.saturating_add(1);
            return Ok(HawkDspControlResult {
                accepted: false,
                queue_depth: state.dsp_control_queue.len(),
                dropped: state.dsp_control_dropped,
            });
        }
        state.dsp_control_queue.push_back(message.to_string());
        Ok(HawkDspControlResult {
            accepted: true,
            queue_depth: state.dsp_control_queue.len(),
            dropped: state.dsp_control_dropped,
        })
    }

    pub(crate) fn dsp_start_analysis_job(
        &self,
        _request: &serde_json::Value,
    ) -> Result<HawkDspAnalysisJob, CapabilityError> {
        self.require_ui_safe_capability("hawk:dsp.startAnalysisJob")?;
        let mut state = self.inner.borrow_mut();
        let capacity = state.dsp_analysis_capacity.ok_or_else(|| {
            CapabilityError::denied(
                "hawk:dsp.startAnalysisJob",
                "DSP analysis jobs are not declared in hawk.json capabilities",
            )
        })?;
        if state.dsp_analysis_jobs.len() >= capacity {
            return Err(CapabilityError::denied(
                "hawk:dsp.startAnalysisJob",
                "DSP analysis job capacity is exhausted",
            ));
        }
        state.dsp_analysis_next_id = state.dsp_analysis_next_id.saturating_add(1);
        let id = format!("hawk-dsp-analysis:{}", state.dsp_analysis_next_id);
        let status = "running".to_owned();
        state.dsp_analysis_jobs.insert(id.clone(), status.clone());
        Ok(HawkDspAnalysisJob { id, status })
    }

    pub(crate) fn dsp_cancel_analysis_job(
        &self,
        id: &str,
    ) -> Result<HawkDspAnalysisJob, CapabilityError> {
        self.require_ui_safe_capability("hawk:dsp.cancelAnalysisJob")?;
        if id.trim().is_empty() {
            return Err(CapabilityError::invalid(
                "hawk:dsp.cancelAnalysisJob",
                "analysis job id must not be empty",
            ));
        }
        let mut state = self.inner.borrow_mut();
        let status = state.dsp_analysis_jobs.get_mut(id).ok_or_else(|| {
            CapabilityError::denied(
                "hawk:dsp.cancelAnalysisJob",
                format!("DSP analysis job `{id}` is not active"),
            )
        })?;
        "cancelled".clone_into(status);
        Ok(HawkDspAnalysisJob {
            id: id.to_owned(),
            status: status.clone(),
        })
    }

    pub(crate) fn dsp_start_offline_render(
        &self,
        request: &serde_json::Value,
    ) -> Result<HawkDspOfflineRenderJob, CapabilityError> {
        self.require_ui_safe_capability("hawk:dsp.startOfflineRender")?;
        let output_path = request
            .get("outputPath")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                CapabilityError::invalid(
                    "hawk:dsp.startOfflineRender",
                    "offline render request must include outputPath",
                )
            })?;
        if output_path.trim().is_empty() {
            return Err(CapabilityError::invalid(
                "hawk:dsp.startOfflineRender",
                "offline render outputPath must not be empty",
            ));
        }
        let mut state = self.inner.borrow_mut();
        let capacity = state.dsp_offline_render_capacity.ok_or_else(|| {
            CapabilityError::denied(
                "hawk:dsp.startOfflineRender",
                "DSP offline render jobs are not declared in hawk.json capabilities",
            )
        })?;
        if state.dsp_offline_render_jobs.len() >= capacity {
            return Err(CapabilityError::denied(
                "hawk:dsp.startOfflineRender",
                "DSP offline render job capacity is exhausted",
            ));
        }
        state.dsp_offline_render_next_id = state.dsp_offline_render_next_id.saturating_add(1);
        let id = format!(
            "hawk-dsp-offline-render:{}",
            state.dsp_offline_render_next_id
        );
        let status = "running".to_owned();
        state
            .dsp_offline_render_jobs
            .insert(id.clone(), status.clone());
        state
            .dsp_offline_render_paths
            .insert(id.clone(), output_path.to_owned());
        Ok(HawkDspOfflineRenderJob { id, status })
    }

    pub(crate) fn dsp_export_offline_render(
        &self,
        id: &str,
    ) -> Result<HawkDspOfflineRenderExport, CapabilityError> {
        self.require_ui_safe_capability("hawk:dsp.exportOfflineRender")?;
        if id.trim().is_empty() {
            return Err(CapabilityError::invalid(
                "hawk:dsp.exportOfflineRender",
                "offline render job id must not be empty",
            ));
        }
        let mut state = self.inner.borrow_mut();
        let path = state
            .dsp_offline_render_paths
            .get(id)
            .cloned()
            .ok_or_else(|| {
                CapabilityError::denied(
                    "hawk:dsp.exportOfflineRender",
                    format!("DSP offline render job `{id}` is not active"),
                )
            })?;
        let status = state.dsp_offline_render_jobs.get_mut(id).ok_or_else(|| {
            CapabilityError::denied(
                "hawk:dsp.exportOfflineRender",
                format!("DSP offline render job `{id}` is not active"),
            )
        })?;
        "exported".clone_into(status);
        Ok(HawkDspOfflineRenderExport {
            id: id.to_owned(),
            status: status.clone(),
            path,
        })
    }

    fn require_plugin_ui(&self, operation: &'static str) -> Result<(), CapabilityError> {
        let host_context = self.inner.borrow().host_context;
        match host_context {
            HawkHostContext::PluginUi => Ok(()),
            HawkHostContext::AudioRealtime => Err(CapabilityError::realtime_denied(operation)),
            HawkHostContext::Test | HawkHostContext::Desktop => Err(CapabilityError::denied(
                operation,
                "plugin capability is only available in plugin UI host context",
            )),
        }
    }

    fn require_plugin_parameter(
        &self,
        operation: &'static str,
        parameter: &str,
    ) -> Result<(), CapabilityError> {
        self.require_plugin_ui(operation)?;
        let state = self.inner.borrow();
        if state.plugin_parameters.contains(parameter) {
            Ok(())
        } else {
            Err(CapabilityError::denied(
                operation,
                format!("plugin parameter `{parameter}` is not declared in hawk.json capabilities"),
            ))
        }
    }

    fn require_plugin_state(&self, operation: &'static str) -> Result<(), CapabilityError> {
        self.require_plugin_ui(operation)?;
        if self.inner.borrow().plugin_state_allowed {
            Ok(())
        } else {
            Err(CapabilityError::denied(
                operation,
                "plugin state access is not declared in hawk.json capabilities",
            ))
        }
    }

    fn require_plugin_preset(
        &self,
        operation: &'static str,
        preset_id: &str,
    ) -> Result<(), CapabilityError> {
        self.require_plugin_state(operation)?;
        if preset_id.trim().is_empty() {
            return Err(CapabilityError::invalid(
                operation,
                "plugin preset id must not be empty",
            ));
        }
        Ok(())
    }

    fn require_ui_safe_capability(&self, operation: &'static str) -> Result<(), CapabilityError> {
        let host_context = self.inner.borrow().host_context;
        match host_context {
            HawkHostContext::Test | HawkHostContext::Desktop | HawkHostContext::PluginUi => Ok(()),
            HawkHostContext::AudioRealtime => Err(CapabilityError::realtime_denied(operation)),
        }
    }

    fn deny_realtime(&self, operation: &'static str) -> Result<(), CapabilityError> {
        if self.inner.borrow().host_context == HawkHostContext::AudioRealtime {
            Err(CapabilityError::realtime_denied(operation))
        } else {
            Ok(())
        }
    }

    fn require_desktop_operation(&self, operation: &'static str) -> Result<(), CapabilityError> {
        self.deny_realtime(operation)?;
        let state = self.inner.borrow();
        match state.host_context {
            HawkHostContext::Desktop | HawkHostContext::Test => {}
            HawkHostContext::PluginUi => {
                return Err(CapabilityError::denied(
                    operation,
                    "desktop capability is only available in desktop host context",
                ));
            }
            HawkHostContext::AudioRealtime => {
                return Err(CapabilityError::denied(
                    operation,
                    "desktop capability is denied in realtime audio context",
                ));
            }
        }
        if !state.desktop_operations.contains(operation) {
            return Err(CapabilityError::denied(
                operation,
                format!(
                    "desktop operation `{operation}` is not declared in hawk.json capabilities"
                ),
            ));
        }
        Ok(())
    }
}

/// Stable capability denial diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CapabilityError {
    rule: &'static str,
    operation: &'static str,
    message: String,
}

impl CapabilityError {
    fn denied(operation: &'static str, message: impl Into<String>) -> Self {
        Self {
            rule: "js-runtime.capability.denied",
            operation,
            message: message.into(),
        }
    }

    fn invalid(operation: &'static str, message: impl Into<String>) -> Self {
        Self {
            rule: "js-runtime.capability.invalid",
            operation,
            message: message.into(),
        }
    }

    fn realtime_denied(operation: &'static str) -> Self {
        Self {
            rule: "js-runtime.capability.realtime-denied",
            operation,
            message: "capability operation is unavailable in realtime audio context".to_owned(),
        }
    }

    fn unsupported(operation: &'static str, message: impl Into<String>) -> Self {
        Self {
            rule: "js-runtime.capability.unsupported",
            operation,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for CapabilityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}: {}: {}",
            self.rule, self.operation, self.message
        )
    }
}

impl std::error::Error for CapabilityError {}

/// Built-in sealed `Hawk2UI` modules injected into every runtime module graph.
pub(crate) fn builtin_hawk_modules() -> &'static [(&'static str, &'static str)] {
    &[
        ("hawk:network", HAWK_NETWORK_MODULE),
        ("hawk:api", HAWK_API_MODULE),
        ("hawk:storage", HAWK_STORAGE_MODULE),
        ("hawk:secrets", HAWK_SECRETS_MODULE),
        ("hawk:files", HAWK_FILES_MODULE),
        ("hawk:desktop", HAWK_DESKTOP_MODULE),
        ("hawk:plugin", HAWK_PLUGIN_MODULE),
        ("hawk:audio", HAWK_AUDIO_MODULE),
        ("hawk:dsp", HAWK_DSP_MODULE),
        ("hawk:ai", HAWK_AI_MODULE),
        ("hawk:runtime", HAWK_RUNTIME_MODULE),
    ]
}

fn network_host(url: &str) -> Result<String, CapabilityError> {
    let without_scheme = url.split_once("://").ok_or_else(|| {
        CapabilityError::denied(
            "hawk:network.request",
            format!("network URL `{url}` must include an explicit scheme"),
        )
    })?;
    let host = without_scheme
        .1
        .split('/')
        .next()
        .unwrap_or_default()
        .split('@')
        .next_back()
        .unwrap_or_default()
        .split(':')
        .next()
        .unwrap_or_default();
    if host.is_empty() {
        Err(CapabilityError::denied(
            "hawk:network.request",
            format!("network URL `{url}` does not include a host"),
        ))
    } else {
        Ok(host.to_owned())
    }
}

fn validate_storage_document_key(
    operation: &'static str,
    key: &str,
) -> Result<(), CapabilityError> {
    if key.is_empty()
        || !key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        return Err(CapabilityError::invalid(
            operation,
            format!("storage document key is not a stable identifier: {key}"),
        ));
    }
    Ok(())
}

fn validate_file_grant_path(operation: &'static str, path: &str) -> Result<(), CapabilityError> {
    let has_traversal = path.split('/').any(|segment| segment == "..");
    if path.is_empty() || !path.starts_with('/') || has_traversal {
        Err(CapabilityError::denied(
            operation,
            format!("file path `{path}` is not an absolute user-grant path"),
        ))
    } else {
        Ok(())
    }
}

fn normalize_file_root(path: &str) -> String {
    if path == "/" {
        path.to_owned()
    } else {
        path.trim_end_matches('/').to_owned()
    }
}

fn file_path_is_granted(state: &HawkRuntimeCapabilityState, path: &str) -> bool {
    state.file_paths.contains(path)
        || state.file_roots.iter().any(|root| {
            if root == "/" {
                path.starts_with('/')
            } else {
                path == root
                    || path
                        .strip_prefix(root)
                        .is_some_and(|suffix| suffix.starts_with('/'))
            }
        })
}

fn stable_secret_handle_suffix(name: &str) -> String {
    let mut suffix = String::with_capacity(name.len() * 2);
    for byte in name.as_bytes() {
        let _ = write!(&mut suffix, "{byte:02x}");
    }
    suffix
}

const HAWK_NETWORK_MODULE: &str = r"
const requestOp = globalThis.__hawk2uiNetworkRequest;
export function request(url, init = {}) {
  return requestOp(String(url), init ?? {});
}
export default { request };
";

const HAWK_API_MODULE: &str = r#"
import { serializeSecretOptions } from "hawk:secrets";
const callOp = globalThis.__hawk2uiApiCall;
export function call(name, payload = undefined, options = {}) {
  return callOp(String(name), payload ?? null, serializeSecretOptions(options ?? {}));
}
export default { call };
"#;

const HAWK_STORAGE_MODULE: &str = r"
const getItemOp = globalThis.__hawk2uiStorageGetItem;
const setItemOp = globalThis.__hawk2uiStorageSetItem;
const getDocumentOp = globalThis.__hawk2uiStorageGetDocument;
const putDocumentOp = globalThis.__hawk2uiStoragePutDocument;
const transactionOp = globalThis.__hawk2uiStorageTransaction;
const STORAGE_VERSION_KEY = '__hawk2ui.storage.version';
export function getItem(namespace, key) {
  return getItemOp(String(namespace), String(key));
}
export function setItem(namespace, key, value) {
  return setItemOp(String(namespace), String(key), String(value));
}
export function getDocument(namespace, key) {
  return getDocumentOp(String(namespace), String(key));
}
export function putDocument(namespace, key, value) {
  return putDocumentOp(String(namespace), String(key), value ?? null);
}
export function transaction(namespace, writes) {
  const storageNamespace = String(namespace);
  if (!Array.isArray(writes)) {
    throw new Error('js-runtime.capability.invalid: hawk:storage.transaction: writes must be an array');
  }
  return transactionOp(storageNamespace, writes.map((write) => ({
    key: String(write?.key ?? ''),
    value: write?.value ?? null,
  })));
}
export async function migrate(namespace, migrations) {
  const storageNamespace = String(namespace);
  if (!Array.isArray(migrations)) {
    throw new Error('js-runtime.capability.invalid: hawk:storage.migrate: migrations must be an array');
  }
  const storedVersionText = await getItem(storageNamespace, STORAGE_VERSION_KEY);
  const storedVersion = storedVersionText === '' ? 0 : Number(storedVersionText);
  if (!Number.isInteger(storedVersion) || storedVersion < 0) {
    throw new Error('js-runtime.capability.invalid: hawk:storage.migrate: stored migration version is corrupt');
  }
  let expectedVersion = storedVersion + 1;
  for (const migration of migrations) {
    const version = Number(migration?.version);
    if (!Number.isInteger(version) || version !== expectedVersion) {
      throw new Error('js-runtime.capability.invalid: hawk:storage.migrate: migration versions must be contiguous and strictly increasing from the stored version');
    }
    if (typeof migration.up !== 'function') {
      throw new Error('js-runtime.capability.invalid: hawk:storage.migrate: each migration requires an up function');
    }
    expectedVersion += 1;
  }
  let currentVersion = storedVersion;
  const context = Object.freeze({
    getItem(key) {
      return getItem(storageNamespace, key);
    },
    setItem(key, value) {
      return setItem(storageNamespace, key, value);
    },
  });
  for (const migration of migrations) {
    const version = Number(migration.version);
    if (version <= currentVersion) continue;
    await migration.up(context);
    await setItem(storageNamespace, STORAGE_VERSION_KEY, String(version));
    currentVersion = version;
  }
  return currentVersion;
}
  export default { getItem, setItem, getDocument, putDocument, transaction, migrate };
  ";

const HAWK_SECRETS_MODULE: &str = r"
const readOp = globalThis.__hawk2uiSecretsRead;
const SECRET_HANDLE = Symbol.for('hawk2ui.secretHandle');

export function read(name) {
  const descriptor = readOp(String(name));
  const handle = {};
  Object.defineProperties(handle, {
    type: { value: 'hawk.secret', enumerable: true },
    name: { value: descriptor.name, enumerable: true },
    redacted: { value: true, enumerable: true },
    [SECRET_HANDLE]: { value: descriptor.handle, enumerable: false },
    toString: {
      value() {
        return `[HawkSecret ${descriptor.name} redacted]`;
      }
    },
    toJSON: {
      value() {
        return { type: 'hawk.secret', name: descriptor.name, redacted: true };
      }
    }
  });
  return Object.freeze(handle);
}

export function isSecretHandle(value) {
  return Boolean(value && typeof value === 'object' && value[SECRET_HANDLE]);
}

export function serializeSecretOptions(options = {}) {
  const serialized = { ...options };
  if (Array.isArray(serialized.secrets)) {
    serialized.secrets = serialized.secrets.map((secret) => {
      if (!isSecretHandle(secret)) {
        throw new Error('js-runtime.capability.invalid: hawk:api.call: API secret options must be values returned by hawk:secrets.read');
      }
      return secret.name;
    });
  }
  return serialized;
}

export default { read, isSecretHandle };
";

const HAWK_FILES_MODULE: &str = r"
const readTextOp = globalThis.__hawk2uiFilesReadText;
const writeTextOp = globalThis.__hawk2uiFilesWriteText;
const readBytesOp = globalThis.__hawk2uiFilesReadBytes;
const writeBytesOp = globalThis.__hawk2uiFilesWriteBytes;
const pickFileOp = globalThis.__hawk2uiFilesPick;
const pickFolderOp = globalThis.__hawk2uiFilesPickFolder;
const watchOp = globalThis.__hawk2uiFilesWatch;
const importFileOp = globalThis.__hawk2uiFilesImport;
const exportFileOp = globalThis.__hawk2uiFilesExport;
function invalidBytes(message) {
  throw new Error(`js-runtime.capability.invalid: hawk:files.writeBytes: ${message}`);
}
function normalizeBytes(bytes) {
  if (bytes instanceof Uint8Array) return Array.from(bytes);
  if (bytes instanceof ArrayBuffer) return Array.from(new Uint8Array(bytes));
  if (ArrayBuffer.isView(bytes)) {
    return Array.from(new Uint8Array(bytes.buffer, bytes.byteOffset, bytes.byteLength));
  }
  if (!Array.isArray(bytes)) {
    invalidBytes('bytes must be a Uint8Array, ArrayBuffer, typed-array view, or byte array');
  }
  return bytes.map((byte) => {
    const value = Number(byte);
    if (!Number.isInteger(value) || value < 0 || value > 255) {
      invalidBytes('byte array values must be integers from 0 through 255');
    }
    return value;
  });
}
export function readText(path) {
  return readTextOp(String(path));
}
export function writeText(path, text) {
  return writeTextOp(String(path), String(text));
}
export function readBytes(path) {
  return new Uint8Array(readBytesOp(String(path)));
}
export function writeBytes(path, bytes) {
  return writeBytesOp(String(path), normalizeBytes(bytes));
}
export function pickFile() {
  return pickFileOp();
}
export function pickFolder() {
  return pickFolderOp();
}
export function watch(path) {
  return watchOp(String(path));
}
export function importFile(optionsOrDestination = {}, destinationPath = undefined) {
  const destination = destinationPath === undefined ? optionsOrDestination : destinationPath;
  return importFileOp(String(destination));
}
export function exportFile(sourcePath, _options = {}) {
  return exportFileOp(String(sourcePath));
}
export default {
  readText,
  writeText,
  readBytes,
  writeBytes,
  pickFile,
  pickFolder,
  watch,
  importFile,
  exportFile,
};
";

const HAWK_DESKTOP_MODULE: &str = r"
const setWindowTitleOp = globalThis.__hawk2uiDesktopSetWindowTitle;
const showOpenDialogOp = globalThis.__hawk2uiDesktopShowOpenDialog;
const readClipboardOp = globalThis.__hawk2uiDesktopReadClipboard;
const writeClipboardOp = globalThis.__hawk2uiDesktopWriteClipboard;
const notifyOp = globalThis.__hawk2uiDesktopNotify;
const registerShortcutOp = globalThis.__hawk2uiDesktopRegisterShortcut;
const openExternalOp = globalThis.__hawk2uiDesktopOpenExternal;
const deepLinkOp = globalThis.__hawk2uiDesktopNextDeepLink;
const setWindowModeOp = globalThis.__hawk2uiDesktopSetWindowMode;
const closeWindowOp = globalThis.__hawk2uiDesktopCloseWindow;

export function setWindowTitle(title) {
  return setWindowTitleOp(String(title));
}
export function showOpenDialog(options = {}) {
  return showOpenDialogOp(options ?? {});
}
export function readClipboard() {
  return readClipboardOp();
}
export function writeClipboard(text) {
  return writeClipboardOp(String(text));
}
export function notify(notification) {
  return notifyOp(notification ?? {});
}
export function registerShortcut(shortcut, _handler) {
  return registerShortcutOp(String(shortcut));
}
export function openExternal(url) {
  return openExternalOp(String(url));
}
export function onDeepLink(scheme, handler) {
  if (typeof handler !== 'function') {
    throw new Error('js-runtime.capability.invalid: hawk:desktop.onDeepLink: handler must be a function');
  }
  const event = Object.freeze(deepLinkOp(String(scheme)));
  handler(event);
  return event;
}
export function setWindowMode(mode) {
  return setWindowModeOp(String(mode));
}
export function closeWindow(reason = 'requested') {
  return closeWindowOp(String(reason));
}
export default {
  setWindowTitle,
  showOpenDialog,
  readClipboard,
  writeClipboard,
  notify,
  registerShortcut,
  openExternal,
  onDeepLink,
  setWindowMode,
  closeWindow,
};
";

const HAWK_PLUGIN_MODULE: &str = r"
const readParameterOp = globalThis.__hawk2uiPluginReadParameter;
const writeParameterOp = globalThis.__hawk2uiPluginWriteParameter;
const beginAutomationGestureOp = globalThis.__hawk2uiPluginBeginAutomationGesture;
const endAutomationGestureOp = globalThis.__hawk2uiPluginEndAutomationGesture;
const loadStateOp = globalThis.__hawk2uiPluginLoadState;
const saveStateOp = globalThis.__hawk2uiPluginSaveState;
const loadPresetOp = globalThis.__hawk2uiPluginLoadPreset;
const savePresetOp = globalThis.__hawk2uiPluginSavePreset;
const getTransportOp = globalThis.__hawk2uiPluginGetTransport;
const resizeEditorOp = globalThis.__hawk2uiPluginResizeEditor;
const focusEditorOp = globalThis.__hawk2uiPluginFocusEditor;
function editorDimension(size, name) {
  const value = Number(size?.[name]);
  if (!Number.isInteger(value) || value <= 0) {
    throw new Error(`js-runtime.capability.invalid: hawk:plugin.resizeEditor: editor ${name} must be a positive integer`);
  }
  return value;
}
export function readParameter(parameter) {
  return Number(readParameterOp(String(parameter)));
}
export function writeParameter(parameter, value) {
  return writeParameterOp(String(parameter), String(Number(value)));
}
export function beginAutomationGesture(parameter) {
  return beginAutomationGestureOp(String(parameter));
}
export function endAutomationGesture(parameter) {
  return endAutomationGestureOp(String(parameter));
}
export function loadState() {
  return loadStateOp();
}
export function saveState(stateBlob) {
  return saveStateOp(String(stateBlob));
}
export function loadPreset(presetId) {
  return loadPresetOp(String(presetId));
}
export function savePreset(presetId, stateBlob) {
  return savePresetOp(String(presetId), String(stateBlob));
}
export function getTransport() {
  return getTransportOp();
}
export function resizeEditor(size) {
  return resizeEditorOp(String(editorDimension(size, 'width')), String(editorDimension(size, 'height')));
}
export function focusEditor() {
  return focusEditorOp();
}
export default {
  readParameter,
  writeParameter,
  beginAutomationGesture,
  endAutomationGesture,
  loadState,
  saveState,
  loadPreset,
  savePreset,
  getTransport,
  resizeEditor,
  focusEditor,
};
";

const HAWK_AUDIO_MODULE: &str = r"
const subscribeMetersOp = globalThis.__hawk2uiAudioSubscribeMeters;
const transportOp = globalThis.__hawk2uiAudioTransport;
const nextControlOp = globalThis.__hawk2uiAudioNextControl;
export function subscribeMeters(options = {}) {
  return subscribeMetersOp(options ?? {});
}
export function transport() {
  return transportOp();
}
export function nextControl(options = {}) {
  return nextControlOp(options ?? {});
}
export default { subscribeMeters, transport, nextControl };
";

const HAWK_DSP_MODULE: &str = r"
const sendControlOp = globalThis.__hawk2uiDspSendControl;
const updateParameterGraphOp = globalThis.__hawk2uiDspUpdateParameterGraph;
const startAnalysisJobOp = globalThis.__hawk2uiDspStartAnalysisJob;
const cancelAnalysisJobOp = globalThis.__hawk2uiDspCancelAnalysisJob;
const startOfflineRenderOp = globalThis.__hawk2uiDspStartOfflineRender;
const exportOfflineRenderOp = globalThis.__hawk2uiDspExportOfflineRender;
export function sendControl(message) {
  return sendControlOp(message ?? {});
}
export function updateParameterGraph(graph = {}) {
  return updateParameterGraphOp(graph ?? {});
}
export function startAnalysisJob(request = {}) {
  return startAnalysisJobOp(request ?? {});
}
export function cancelAnalysisJob(id) {
  return cancelAnalysisJobOp(String(id));
}
export function startOfflineRender(request = {}) {
  return startOfflineRenderOp(request ?? {});
}
export function exportOfflineRender(id) {
  return exportOfflineRenderOp(String(id));
}
export default {
  sendControl,
  updateParameterGraph,
  startAnalysisJob,
  cancelAnalysisJob,
  startOfflineRender,
  exportOfflineRender,
};
";

const HAWK_AI_MODULE: &str = r#"
import { serializeSecretOptions } from "hawk:secrets";
const callProviderOp = globalThis.__hawk2uiAiCallProvider;
const streamProviderOp = globalThis.__hawk2uiAiStreamProvider;
export function callProvider(provider, payload = undefined, options = {}) {
  return callProviderOp(String(provider), payload ?? null, serializeSecretOptions(options ?? {}));
}
export async function* streamProvider(provider, payload = undefined, options = {}) {
  const chunks = streamProviderOp(String(provider), payload ?? null, serializeSecretOptions(options ?? {}));
  for (const text of chunks) {
    yield Object.freeze({ text: String(text) });
  }
}
export default { callProvider, streamProvider };
"#;

const HAWK_RUNTIME_MODULE: &str = r#"
import * as network from "hawk:network";
import * as api from "hawk:api";
import * as storage from "hawk:storage";
import * as secrets from "hawk:secrets";
import * as files from "hawk:files";
import * as desktop from "hawk:desktop";
import * as plugin from "hawk:plugin";
import * as audio from "hawk:audio";
import * as dsp from "hawk:dsp";
import * as ai from "hawk:ai";
export { network, api, storage, secrets, files, desktop, plugin, audio, dsp, ai };
export default { network, api, storage, secrets, files, desktop, plugin, audio, dsp, ai };
"#;

//! `Hawk2UI` capability extension ops.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::Duration;

use deno_core::{Extension, OpState, op2};
use deno_error::JsErrorBox;
use serde::Deserialize;

use crate::permissions::{
    HawkAudioControlEvent, HawkAudioMeterFrame, HawkAudioTransportInfo, HawkDesktopCloseResult,
    HawkDesktopDeepLinkEvent, HawkDesktopWindowModeResult, HawkDspAnalysisJob,
    HawkDspControlResult, HawkDspOfflineRenderExport, HawkDspOfflineRenderJob,
    HawkDspParameterGraphUpdate, HawkFileExportResult, HawkFileImportResult, HawkFileWatchEvent,
    HawkNetworkRequest, HawkNetworkResponse, HawkPluginEditorFocus, HawkPluginEditorSize,
    HawkPluginTransportInfo, HawkRuntimeCapabilities, HawkSecretDescriptor,
    HawkStorageTransactionResult,
};

deno_core::extension!(
    hawk_capabilities,
      ops = [
          op_hawk_network_request,
          op_hawk_ai_call_provider,
          op_hawk_ai_stream_provider,
          op_hawk_api_call,
        op_hawk_secrets_read,
        op_hawk_desktop_set_window_title,
        op_hawk_desktop_show_open_dialog,
        op_hawk_desktop_read_clipboard,
        op_hawk_desktop_write_clipboard,
        op_hawk_desktop_notify,
          op_hawk_desktop_register_shortcut,
          op_hawk_desktop_open_external,
          op_hawk_desktop_next_deep_link,
          op_hawk_desktop_set_window_mode,
          op_hawk_desktop_close_window,
            op_hawk_storage_get_item,
            op_hawk_storage_set_item,
            op_hawk_storage_get_document,
            op_hawk_storage_put_document,
            op_hawk_storage_transaction,
            op_hawk_files_read_text,
          op_hawk_files_write_text,
          op_hawk_files_read_bytes,
          op_hawk_files_write_bytes,
            op_hawk_files_pick,
            op_hawk_files_pick_folder,
            op_hawk_files_watch,
            op_hawk_files_import,
            op_hawk_files_export,
          op_hawk_plugin_read_parameter,
        op_hawk_plugin_write_parameter,
        op_hawk_plugin_begin_automation_gesture,
            op_hawk_plugin_end_automation_gesture,
            op_hawk_plugin_load_state,
            op_hawk_plugin_save_state,
            op_hawk_plugin_load_preset,
            op_hawk_plugin_save_preset,
            op_hawk_plugin_get_transport,
            op_hawk_plugin_resize_editor,
            op_hawk_plugin_focus_editor,
              op_hawk_audio_subscribe_meters,
              op_hawk_audio_transport,
              op_hawk_audio_next_control,
            op_hawk_dsp_send_control,
            op_hawk_dsp_update_parameter_graph,
            op_hawk_dsp_start_analysis_job,
            op_hawk_dsp_cancel_analysis_job,
            op_hawk_dsp_start_offline_render,
            op_hawk_dsp_export_offline_render,
        ],
    options = {
        capabilities: HawkRuntimeCapabilities,
    },
    state = |state, options| state.put(options.capabilities)
);

/// Creates the capability extension for one runtime instance.
pub(crate) fn extension(capabilities: HawkRuntimeCapabilities) -> Extension {
    hawk_capabilities::init(capabilities)
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NetworkRequestInit {
    method: Option<String>,
    #[serde(default)]
    headers: BTreeMap<String, String>,
    body: Option<String>,
    timeout_ms: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiCallOptions {
    #[serde(default)]
    secrets: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AiCallOptions {
    #[serde(default)]
    secrets: Vec<String>,
    budget_tokens: Option<u32>,
    timeout_ms: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DesktopNotification {
    #[serde(default)]
    title: String,
    #[serde(default)]
    body: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AudioMeterOptions {
    #[serde(default)]
    source: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StorageDocumentWrite {
    key: String,
    value: serde_json::Value,
}

#[op2]
#[serde]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
async fn op_hawk_network_request(
    state: Rc<RefCell<OpState>>,
    #[string] url: String,
    #[serde] init: NetworkRequestInit,
) -> Result<HawkNetworkResponse, JsErrorBox> {
    let timeout_ms = init.timeout_ms;
    let response = capabilities(&state)
        .network_request(HawkNetworkRequest {
            url,
            method: init.method,
            headers: init.headers,
            body: init.body,
            timeout_ms,
        })
        .map_err(|error| JsErrorBox::generic(error.to_string()))?;
    drop(state);

    let delay_ms = response.delay_ms.unwrap_or_default();
    if let Some(timeout_ms) = timeout_ms.filter(|timeout_ms| delay_ms > *timeout_ms) {
        tokio::time::sleep(Duration::from_millis(timeout_ms)).await;
        return Err(JsErrorBox::generic(format!(
            "js-runtime.capability.timeout: fetch request exceeded timeoutMs {timeout_ms}"
        )));
    }
    if delay_ms > 0 {
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
    }

    Ok(response)
}

#[op2]
#[serde]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_ai_call_provider(
    state: Rc<RefCell<OpState>>,
    #[string] provider: String,
    #[serde] _payload: serde_json::Value,
    #[serde] options: AiCallOptions,
) -> Result<HawkNetworkResponse, JsErrorBox> {
    capabilities(&state)
        .ai_call_provider(
            &provider,
            &options.secrets,
            options.budget_tokens,
            options.timeout_ms,
        )
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[serde]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_ai_stream_provider(
    state: Rc<RefCell<OpState>>,
    #[string] provider: String,
    #[serde] _payload: serde_json::Value,
    #[serde] options: AiCallOptions,
) -> Result<Vec<String>, JsErrorBox> {
    capabilities(&state)
        .ai_stream_provider(
            &provider,
            &options.secrets,
            options.budget_tokens,
            options.timeout_ms,
        )
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[serde]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_api_call(
    state: Rc<RefCell<OpState>>,
    #[string] endpoint: String,
    #[serde] _payload: serde_json::Value,
    #[serde] options: ApiCallOptions,
) -> Result<HawkNetworkResponse, JsErrorBox> {
    capabilities(&state)
        .api_call(&endpoint, &options.secrets)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[serde]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_secrets_read(
    state: Rc<RefCell<OpState>>,
    #[string] name: String,
) -> Result<HawkSecretDescriptor, JsErrorBox> {
    capabilities(&state)
        .secret_read(&name)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2(fast)]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_desktop_set_window_title(
    state: Rc<RefCell<OpState>>,
    #[string] title: String,
) -> Result<(), JsErrorBox> {
    capabilities(&state)
        .desktop_set_window_title(&title)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[serde]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_desktop_show_open_dialog(
    state: Rc<RefCell<OpState>>,
    #[serde] _options: serde_json::Value,
) -> Result<Vec<String>, JsErrorBox> {
    capabilities(&state)
        .desktop_show_open_dialog()
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[string]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_desktop_read_clipboard(state: Rc<RefCell<OpState>>) -> Result<String, JsErrorBox> {
    capabilities(&state)
        .desktop_read_clipboard()
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2(fast)]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_desktop_write_clipboard(
    state: Rc<RefCell<OpState>>,
    #[string] text: String,
) -> Result<(), JsErrorBox> {
    capabilities(&state)
        .desktop_write_clipboard(text)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_desktop_notify(
    state: Rc<RefCell<OpState>>,
    #[serde] notification: DesktopNotification,
) -> Result<(), JsErrorBox> {
    capabilities(&state)
        .desktop_notify(&notification.title, &notification.body)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[string]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_desktop_register_shortcut(
    state: Rc<RefCell<OpState>>,
    #[string] shortcut: String,
) -> Result<String, JsErrorBox> {
    capabilities(&state)
        .desktop_register_shortcut(&shortcut)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2(fast)]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_desktop_open_external(
    state: Rc<RefCell<OpState>>,
    #[string] url: String,
) -> Result<(), JsErrorBox> {
    capabilities(&state)
        .desktop_open_external(&url)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[serde]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_desktop_next_deep_link(
    state: Rc<RefCell<OpState>>,
    #[string] scheme: String,
) -> Result<HawkDesktopDeepLinkEvent, JsErrorBox> {
    capabilities(&state)
        .desktop_next_deep_link(&scheme)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[serde]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_desktop_set_window_mode(
    state: Rc<RefCell<OpState>>,
    #[string] mode: String,
) -> Result<HawkDesktopWindowModeResult, JsErrorBox> {
    capabilities(&state)
        .desktop_set_window_mode(&mode)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[serde]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_desktop_close_window(
    state: Rc<RefCell<OpState>>,
    #[string] reason: String,
) -> Result<HawkDesktopCloseResult, JsErrorBox> {
    capabilities(&state)
        .desktop_close_window(&reason)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[string]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_storage_get_item(
    state: Rc<RefCell<OpState>>,
    #[string] namespace: String,
    #[string] key: String,
) -> Result<String, JsErrorBox> {
    capabilities(&state)
        .storage_get(&namespace, &key)
        .map(std::option::Option::unwrap_or_default)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2(fast)]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_storage_set_item(
    state: Rc<RefCell<OpState>>,
    #[string] namespace: String,
    #[string] key: String,
    #[string] value: String,
) -> Result<(), JsErrorBox> {
    capabilities(&state)
        .storage_set(&namespace, &key, value)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[serde]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_storage_get_document(
    state: Rc<RefCell<OpState>>,
    #[string] namespace: String,
    #[string] key: String,
) -> Result<serde_json::Value, JsErrorBox> {
    capabilities(&state)
        .storage_get_document(&namespace, &key)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[serde]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_storage_put_document(
    state: Rc<RefCell<OpState>>,
    #[string] namespace: String,
    #[string] key: String,
    #[serde] value: serde_json::Value,
) -> Result<serde_json::Value, JsErrorBox> {
    capabilities(&state)
        .storage_put_document(&namespace, &key, value)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[serde]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_storage_transaction(
    state: Rc<RefCell<OpState>>,
    #[string] namespace: String,
    #[serde] writes: Vec<StorageDocumentWrite>,
) -> Result<HawkStorageTransactionResult, JsErrorBox> {
    capabilities(&state)
        .storage_transaction(
            &namespace,
            writes.into_iter().map(|write| (write.key, write.value)),
        )
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[string]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_files_read_text(
    state: Rc<RefCell<OpState>>,
    #[string] path: String,
) -> Result<String, JsErrorBox> {
    capabilities(&state)
        .file_read_text(&path)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2(fast)]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_files_write_text(
    state: Rc<RefCell<OpState>>,
    #[string] path: String,
    #[string] text: String,
) -> Result<(), JsErrorBox> {
    capabilities(&state)
        .file_write_text(&path, text)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[serde]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_files_read_bytes(
    state: Rc<RefCell<OpState>>,
    #[string] path: String,
) -> Result<Vec<u8>, JsErrorBox> {
    capabilities(&state)
        .file_read_bytes(&path)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_files_write_bytes(
    state: Rc<RefCell<OpState>>,
    #[string] path: String,
    #[serde] bytes: Vec<u8>,
) -> Result<(), JsErrorBox> {
    capabilities(&state)
        .file_write_bytes(&path, bytes)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[string]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_files_pick(state: Rc<RefCell<OpState>>) -> Result<String, JsErrorBox> {
    capabilities(&state)
        .file_pick()
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[string]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_files_pick_folder(state: Rc<RefCell<OpState>>) -> Result<String, JsErrorBox> {
    capabilities(&state)
        .file_pick_folder()
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[serde]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_files_watch(
    state: Rc<RefCell<OpState>>,
    #[string] path: String,
) -> Result<HawkFileWatchEvent, JsErrorBox> {
    capabilities(&state)
        .file_watch(&path)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[serde]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_files_import(
    state: Rc<RefCell<OpState>>,
    #[string] destination_path: String,
) -> Result<HawkFileImportResult, JsErrorBox> {
    capabilities(&state)
        .file_import(&destination_path)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[serde]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_files_export(
    state: Rc<RefCell<OpState>>,
    #[string] source_path: String,
) -> Result<HawkFileExportResult, JsErrorBox> {
    capabilities(&state)
        .file_export(&source_path)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[string]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_plugin_read_parameter(
    state: Rc<RefCell<OpState>>,
    #[string] parameter: String,
) -> Result<String, JsErrorBox> {
    capabilities(&state)
        .plugin_read_parameter(&parameter)
        .map(|value| value.to_string())
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2(fast)]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_plugin_write_parameter(
    state: Rc<RefCell<OpState>>,
    #[string] parameter: String,
    #[string] value: String,
) -> Result<(), JsErrorBox> {
    let value = value
        .parse::<f64>()
        .map_err(|error| JsErrorBox::generic(format!("js-runtime.capability.invalid: hawk:plugin.writeParameter: plugin parameter value is not numeric: {error}")))?;
    capabilities(&state)
        .plugin_write_parameter(&parameter, value)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[string]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_plugin_begin_automation_gesture(
    state: Rc<RefCell<OpState>>,
    #[string] parameter: String,
) -> Result<String, JsErrorBox> {
    capabilities(&state)
        .plugin_begin_automation_gesture(&parameter)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[string]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_plugin_end_automation_gesture(
    state: Rc<RefCell<OpState>>,
    #[string] parameter: String,
) -> Result<String, JsErrorBox> {
    capabilities(&state)
        .plugin_end_automation_gesture(&parameter)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[string]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_plugin_load_state(state: Rc<RefCell<OpState>>) -> Result<String, JsErrorBox> {
    capabilities(&state)
        .plugin_load_state()
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2(fast)]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_plugin_save_state(
    state: Rc<RefCell<OpState>>,
    #[string] state_blob: String,
) -> Result<(), JsErrorBox> {
    capabilities(&state)
        .plugin_save_state(state_blob)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[string]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_plugin_load_preset(
    state: Rc<RefCell<OpState>>,
    #[string] preset_id: String,
) -> Result<String, JsErrorBox> {
    capabilities(&state)
        .plugin_load_preset(&preset_id)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2(fast)]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_plugin_save_preset(
    state: Rc<RefCell<OpState>>,
    #[string] preset_id: String,
    #[string] state_blob: String,
) -> Result<(), JsErrorBox> {
    capabilities(&state)
        .plugin_save_preset(&preset_id, state_blob)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[serde]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_plugin_get_transport(
    state: Rc<RefCell<OpState>>,
) -> Result<HawkPluginTransportInfo, JsErrorBox> {
    capabilities(&state)
        .plugin_get_transport()
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[serde]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_plugin_resize_editor(
    state: Rc<RefCell<OpState>>,
    #[string] width: String,
    #[string] height: String,
) -> Result<HawkPluginEditorSize, JsErrorBox> {
    let width = width.parse::<u32>().map_err(|error| {
        JsErrorBox::generic(format!(
            "js-runtime.capability.invalid: hawk:plugin.resizeEditor: editor width is invalid: {error}"
        ))
    })?;
    let height = height.parse::<u32>().map_err(|error| {
        JsErrorBox::generic(format!(
            "js-runtime.capability.invalid: hawk:plugin.resizeEditor: editor height is invalid: {error}"
        ))
    })?;
    capabilities(&state)
        .plugin_resize_editor(width, height)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[serde]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_plugin_focus_editor(
    state: Rc<RefCell<OpState>>,
) -> Result<HawkPluginEditorFocus, JsErrorBox> {
    capabilities(&state)
        .plugin_focus_editor()
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[serde]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_audio_subscribe_meters(
    state: Rc<RefCell<OpState>>,
    #[serde] options: AudioMeterOptions,
) -> Result<HawkAudioMeterFrame, JsErrorBox> {
    capabilities(&state)
        .audio_subscribe_meters(&options.source)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[serde]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_audio_transport(
    state: Rc<RefCell<OpState>>,
) -> Result<HawkAudioTransportInfo, JsErrorBox> {
    capabilities(&state)
        .audio_transport()
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[serde]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_audio_next_control(
    state: Rc<RefCell<OpState>>,
    #[serde] options: AudioMeterOptions,
) -> Result<HawkAudioControlEvent, JsErrorBox> {
    capabilities(&state)
        .audio_next_control(&options.source)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[serde]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_dsp_send_control(
    state: Rc<RefCell<OpState>>,
    #[serde] message: serde_json::Value,
) -> Result<HawkDspControlResult, JsErrorBox> {
    capabilities(&state)
        .dsp_send_control(&message)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[serde]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_dsp_update_parameter_graph(
    state: Rc<RefCell<OpState>>,
    #[serde] graph: serde_json::Value,
) -> Result<HawkDspParameterGraphUpdate, JsErrorBox> {
    capabilities(&state)
        .dsp_update_parameter_graph(&graph)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[serde]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_dsp_start_analysis_job(
    state: Rc<RefCell<OpState>>,
    #[serde] request: serde_json::Value,
) -> Result<HawkDspAnalysisJob, JsErrorBox> {
    capabilities(&state)
        .dsp_start_analysis_job(&request)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[serde]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_dsp_cancel_analysis_job(
    state: Rc<RefCell<OpState>>,
    #[string] id: String,
) -> Result<HawkDspAnalysisJob, JsErrorBox> {
    capabilities(&state)
        .dsp_cancel_analysis_job(&id)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[serde]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_dsp_start_offline_render(
    state: Rc<RefCell<OpState>>,
    #[serde] request: serde_json::Value,
) -> Result<HawkDspOfflineRenderJob, JsErrorBox> {
    capabilities(&state)
        .dsp_start_offline_render(&request)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

#[op2]
#[serde]
#[allow(
    clippy::needless_pass_by_value,
    reason = "deno_core op2 injects OpState as Rc<RefCell<OpState>> by value"
)]
fn op_hawk_dsp_export_offline_render(
    state: Rc<RefCell<OpState>>,
    #[string] id: String,
) -> Result<HawkDspOfflineRenderExport, JsErrorBox> {
    capabilities(&state)
        .dsp_export_offline_render(&id)
        .map_err(|error| JsErrorBox::generic(error.to_string()))
}

fn capabilities(state: &Rc<RefCell<OpState>>) -> HawkRuntimeCapabilities {
    state.borrow().borrow::<HawkRuntimeCapabilities>().clone()
}

use std::collections::BTreeMap;

use hawk2ui_js_runtime::{
    HawkAudioTransportInfo, HawkHostContext, HawkJsModule, HawkJsModuleGraph, HawkJsRuntime,
    HawkNetworkResponse, HawkPluginTransportInfo, HawkRuntimeCapabilities, JsRuntimeValue,
};

fn module_runtime(source: &str) -> HawkJsModuleGraph {
    HawkJsModuleGraph::new("file:///app/main.js")
        .with_module(HawkJsModule::new("file:///app/main.js", source))
}

#[test]
fn capabilities_deny_network_by_default() {
    let graph = module_runtime(
        r#"
import { request } from "hawk:network";
await request("https://api.example.test/status");
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph(graph).expect("runtime initializes");

    let error = runtime
        .execute_entrypoint_module()
        .expect_err("undeclared network access should be denied");

    assert!(
        error.message().contains("js-runtime.capability.denied"),
        "{}",
        error.message()
    );
    assert!(
        error.message().contains("hawk:network.request"),
        "{}",
        error.message()
    );
}

#[test]
fn capabilities_allow_network_with_explicit_test_backend() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .allow_network_host("api.example.test")
        .with_network_response(
            "https://api.example.test/status",
            HawkNetworkResponse::json(200, r#"{"ok":true}"#),
        );
    let graph = module_runtime(
        r#"
import { request } from "hawk:network";
const response = await request("https://api.example.test/status");
globalThis.__hawk_network_status = response.status;
globalThis.__hawk_network_body = response.body;
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("declared network access succeeds");

    assert_eq!(
        runtime
            .evaluate_script_value("read-status.js", "globalThis.__hawk_network_status")
            .expect("status is readable"),
        JsRuntimeValue::Number(200.0)
    );
    assert_eq!(
        runtime
            .evaluate_script_value("read-body.js", "globalThis.__hawk_network_body")
            .expect("body is readable"),
        JsRuntimeValue::String(r#"{"ok":true}"#.to_owned())
    );
}

#[test]
fn capabilities_fetch_uses_declared_network_backend_with_request_response_headers() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .allow_network_host("api.example.test")
        .with_network_response(
            "https://api.example.test/status",
            HawkNetworkResponse::json(201, r#"{"ok":true}"#),
        );
    let graph = module_runtime(
        r#"
const requestHeaders = new Headers([["x-client", "hawk"]]);
const request = new Request("https://api.example.test/status", {
  method: "POST",
  headers: requestHeaders,
  body: "ping"
});
const response = await fetch(request);
globalThis.__hawk_fetch_request_method = request.method;
globalThis.__hawk_fetch_request_header = request.headers.get("x-client");
globalThis.__hawk_fetch_response_constructor = response.constructor.name;
globalThis.__hawk_fetch_status = response.status;
globalThis.__hawk_fetch_ok = response.ok;
globalThis.__hawk_fetch_content_type = response.headers.get("content-type");
globalThis.__hawk_fetch_body = await response.text();
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("declared fetch access succeeds");

    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-fetch-request-method.js",
                "globalThis.__hawk_fetch_request_method"
            )
            .expect("request method is readable"),
        JsRuntimeValue::String("POST".to_owned())
    );
    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-fetch-request-header.js",
                "globalThis.__hawk_fetch_request_header"
            )
            .expect("request header is readable"),
        JsRuntimeValue::String("hawk".to_owned())
    );
    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-fetch-response-constructor.js",
                "globalThis.__hawk_fetch_response_constructor"
            )
            .expect("response constructor is readable"),
        JsRuntimeValue::String("Response".to_owned())
    );
    assert_eq!(
        runtime
            .evaluate_script_value("read-fetch-status.js", "globalThis.__hawk_fetch_status")
            .expect("status is readable"),
        JsRuntimeValue::Number(201.0)
    );
    assert_eq!(
        runtime
            .evaluate_script_value("read-fetch-ok.js", "globalThis.__hawk_fetch_ok")
            .expect("ok is readable"),
        JsRuntimeValue::Bool(true)
    );
    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-fetch-content-type.js",
                "globalThis.__hawk_fetch_content_type"
            )
            .expect("response header is readable"),
        JsRuntimeValue::String("application/json".to_owned())
    );
    assert_eq!(
        runtime
            .evaluate_script_value("read-fetch-body.js", "globalThis.__hawk_fetch_body")
            .expect("body is readable"),
        JsRuntimeValue::String(r#"{"ok":true}"#.to_owned())
    );
}

#[test]
fn capabilities_fetch_passes_method_headers_body_and_timeout_to_backend() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .allow_network_host("api.example.test")
        .with_network_response(
            "https://api.example.test/update",
            HawkNetworkResponse::text(204, ""),
        );
    let graph = module_runtime(
        r#"
await fetch("https://api.example.test/update", {
  method: "PATCH",
  headers: { "x-client": "hawk", "content-type": "text/plain" },
  body: "payload",
  timeoutMs: 750
});
"#,
    );
    let mut runtime =
        HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities.clone())
            .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("declared fetch access succeeds");

    let requests = capabilities.network_requests();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(request.url, "https://api.example.test/update");
    assert_eq!(request.method.as_deref(), Some("PATCH"));
    assert_eq!(
        request.headers.get("x-client").map(String::as_str),
        Some("hawk")
    );
    assert_eq!(
        request.headers.get("content-type").map(String::as_str),
        Some("text/plain")
    );
    assert_eq!(request.body.as_deref(), Some("payload"));
    assert_eq!(request.timeout_ms, Some(750));
}

#[test]
fn capabilities_fetch_rejects_request_bodies_over_configured_limit() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .allow_network_host("api.example.test")
        .with_network_body_limit_bytes(4)
        .with_network_response(
            "https://api.example.test/upload",
            HawkNetworkResponse::text(204, ""),
        );
    let graph = module_runtime(
        r#"
try {
  await fetch("https://api.example.test/upload", {
    method: "POST",
    body: "payload"
  });
  globalThis.__hawk_fetch_body_limit_error = "resolved";
} catch (error) {
  globalThis.__hawk_fetch_body_limit_error = error.message;
}
"#,
    );
    let mut runtime =
        HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities.clone())
            .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("request body limit is catchable");

    assert_eq!(capabilities.network_requests().len(), 0);
    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-fetch-body-limit-error.js",
                "globalThis.__hawk_fetch_body_limit_error"
            )
            .expect("body limit error is readable"),
        JsRuntimeValue::String(
            "js-runtime.capability.invalid: hawk:network.request: network request body exceeds configured byte limit of 4 bytes".to_owned()
        )
    );
}

#[test]
fn capabilities_fetch_rejects_response_bodies_over_configured_limit_after_dispatch() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .allow_network_host("api.example.test")
        .with_network_body_limit_bytes(4)
        .with_network_response(
            "https://api.example.test/download",
            HawkNetworkResponse::text(200, "payload"),
        );
    let graph = module_runtime(
        r#"
try {
  await fetch("https://api.example.test/download");
  globalThis.__hawk_fetch_response_limit_error = "resolved";
} catch (error) {
  globalThis.__hawk_fetch_response_limit_error = error.message;
}
"#,
    );
    let mut runtime =
        HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities.clone())
            .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("response body limit is catchable");

    assert_eq!(capabilities.network_requests().len(), 1);
    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-fetch-response-limit-error.js",
                "globalThis.__hawk_fetch_response_limit_error"
            )
            .expect("response limit error is readable"),
        JsRuntimeValue::String(
            "js-runtime.capability.invalid: hawk:network.request: network response body exceeds configured byte limit of 4 bytes".to_owned()
        )
    );
}

#[test]
fn capabilities_fetch_rejects_redirects_when_policy_is_error() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .allow_network_host("api.example.test")
        .with_network_response(
            "https://api.example.test/redirect",
            HawkNetworkResponse {
                status: 302,
                headers: BTreeMap::from([(
                    "location".to_owned(),
                    "https://api.example.test/next".to_owned(),
                )]),
                body: String::new(),
                delay_ms: None,
            },
        );
    let graph = module_runtime(
        r#"
try {
  await fetch("https://api.example.test/redirect", { redirect: "error" });
  globalThis.__hawk_fetch_redirect_error = "resolved";
} catch (error) {
  globalThis.__hawk_fetch_redirect_error = error.message;
}
"#,
    );
    let mut runtime =
        HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities.clone())
            .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("redirect policy is catchable");

    assert_eq!(capabilities.network_requests().len(), 1);
    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-fetch-redirect-error.js",
                "globalThis.__hawk_fetch_redirect_error"
            )
            .expect("redirect policy error is readable"),
        JsRuntimeValue::String(
            "js-runtime.capability.denied: fetch redirect policy rejected redirect response to https://api.example.test/next".to_owned()
        )
    );
}

#[test]
fn capabilities_fetch_denies_undeclared_hosts() {
    let graph = module_runtime(
        r#"
await fetch("https://api.example.test/status");
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph(graph).expect("runtime initializes");

    let error = runtime
        .execute_entrypoint_module()
        .expect_err("undeclared fetch access should be denied");

    assert!(
        error.message().contains("js-runtime.capability.denied"),
        "{}",
        error.message()
    );
    assert!(error.message().contains("hawk:network.request"));
    assert!(error.message().contains("api.example.test"));
}

#[test]
fn capabilities_fetch_honors_preaborted_signal_before_dispatch() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .allow_network_host("api.example.test")
        .with_network_response(
            "https://api.example.test/status",
            HawkNetworkResponse::text(200, "should not dispatch"),
        );
    let graph = module_runtime(
        r#"
const controller = new AbortController();
controller.abort("cancelled");
try {
  await fetch("https://api.example.test/status", { signal: controller.signal });
} catch (error) {
  globalThis.__hawk_fetch_abort_error = error.message;
}
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("pre-aborted fetch is catchable");

    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-fetch-abort-error.js",
                "globalThis.__hawk_fetch_abort_error"
            )
            .expect("abort error is readable"),
        JsRuntimeValue::String(
            "js-runtime.capability.aborted: fetch request was aborted before dispatch".to_owned()
        )
    );
}

#[test]
fn capabilities_fetch_rejects_non_positive_timeout_before_dispatch() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .allow_network_host("api.example.test")
        .with_network_response(
            "https://api.example.test/status",
            HawkNetworkResponse::text(200, "should not dispatch"),
        );
    let graph = module_runtime(
        r#"
await fetch("https://api.example.test/status", { timeoutMs: 0 });
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    let error = runtime
        .execute_entrypoint_module()
        .expect_err("non-positive timeout should fail clearly");

    assert!(
        error.message().contains("js-runtime.capability.invalid"),
        "{}",
        error.message()
    );
    assert!(
        error
            .message()
            .contains("fetch timeoutMs must be greater than zero")
    );
}

#[test]
fn capabilities_fetch_times_out_after_dispatching_delayed_network_request() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .allow_network_host("api.example.test")
        .with_network_response(
            "https://api.example.test/slow",
            HawkNetworkResponse::text(200, "late").with_delay_ms(25),
        );
    let graph = module_runtime(
        r#"
try {
  await fetch("https://api.example.test/slow", { timeoutMs: 5 });
  globalThis.__hawk_fetch_timeout_error = "resolved";
} catch (error) {
  globalThis.__hawk_fetch_timeout_error = error.message;
}
"#,
    );
    let mut runtime =
        HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities.clone())
            .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("fetch timeout is catchable");

    assert_eq!(capabilities.network_requests().len(), 1);
    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-fetch-timeout-error.js",
                "globalThis.__hawk_fetch_timeout_error"
            )
            .expect("timeout error is readable"),
        JsRuntimeValue::String(
            "js-runtime.capability.timeout: fetch request exceeded timeoutMs 5".to_owned()
        )
    );
}

#[test]
fn capabilities_fetch_honors_abort_signal_after_dispatch() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .allow_network_host("api.example.test")
        .with_network_response(
            "https://api.example.test/slow-abort",
            HawkNetworkResponse::text(200, "late").with_delay_ms(25),
        );
    let graph = module_runtime(
        r#"
const controller = new AbortController();
setTimeout(() => controller.abort("cancelled"), 5);
try {
  await fetch("https://api.example.test/slow-abort", { signal: controller.signal });
  globalThis.__hawk_fetch_abort_after_dispatch_error = "resolved";
} catch (error) {
  globalThis.__hawk_fetch_abort_after_dispatch_error = error.message;
}
"#,
    );
    let mut runtime =
        HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities.clone())
            .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("fetch abort after dispatch is catchable");

    assert_eq!(capabilities.network_requests().len(), 1);
    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-fetch-abort-after-dispatch-error.js",
                "globalThis.__hawk_fetch_abort_after_dispatch_error"
            )
            .expect("abort error is readable"),
        JsRuntimeValue::String(
            "js-runtime.capability.aborted: fetch request was aborted after dispatch".to_owned()
        )
    );
}

#[test]
fn capabilities_allow_storage_and_files_only_when_declared() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .allow_storage_namespace("settings")
        .allow_file_path("/project/config.json")
        .with_file_text("/project/config.json", r#"{"theme":"dark"}"#);
    let graph = module_runtime(
        r#"
import { getItem, setItem } from "hawk:storage";
import { readText } from "hawk:files";
await setItem("settings", "theme", "dark");
globalThis.__hawk_theme = await getItem("settings", "theme");
globalThis.__hawk_config = await readText("/project/config.json");
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("declared storage and file access succeeds");

    assert_eq!(
        runtime
            .evaluate_script_value("read-theme.js", "globalThis.__hawk_theme")
            .expect("theme is readable"),
        JsRuntimeValue::String("dark".to_owned())
    );
    assert_eq!(
        runtime
            .evaluate_script_value("read-config.js", "globalThis.__hawk_config")
            .expect("config is readable"),
        JsRuntimeValue::String(r#"{"theme":"dark"}"#.to_owned())
    );
}

#[test]
fn capabilities_storage_migrations_apply_in_order_and_record_version() {
    let capabilities = HawkRuntimeCapabilities::for_test().allow_storage_namespace("settings");
    let graph = module_runtime(
        r#"
import { getItem, migrate } from "hawk:storage";

const applied = [];
await migrate("settings", [
  {
    version: 1,
    up: async ({ setItem }) => {
      applied.push("one");
      await setItem("theme", "dark");
    },
  },
  {
    version: 2,
    up: async ({ getItem, setItem }) => {
      applied.push(await getItem("theme"));
      await setItem("mode", "wide");
    },
  },
]);

globalThis.__hawk_storage_migrations_applied = applied.join(",");
globalThis.__hawk_storage_migration_version = await getItem("settings", "__hawk2ui.storage.version");
globalThis.__hawk_storage_migration_mode = await getItem("settings", "mode");
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("declared storage migrations succeed");

    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-storage-migrations-applied.js",
                "globalThis.__hawk_storage_migrations_applied"
            )
            .expect("migration order is readable"),
        JsRuntimeValue::String("one,dark".to_owned())
    );
    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-storage-migration-version.js",
                "globalThis.__hawk_storage_migration_version"
            )
            .expect("migration version is readable"),
        JsRuntimeValue::String("2".to_owned())
    );
    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-storage-migration-mode.js",
                "globalThis.__hawk_storage_migration_mode"
            )
            .expect("migration result is readable"),
        JsRuntimeValue::String("wide".to_owned())
    );
}

#[test]
fn capabilities_storage_migrations_reject_out_of_order_versions() {
    let capabilities = HawkRuntimeCapabilities::for_test().allow_storage_namespace("settings");
    let graph = module_runtime(
        r#"
import { getItem, migrate } from "hawk:storage";

try {
  await migrate("settings", [
    { version: 2, up: async ({ setItem }) => setItem("mode", "wide") },
    { version: 1, up: async ({ setItem }) => setItem("theme", "dark") },
  ]);
  globalThis.__hawk_storage_migration_error = "resolved";
} catch (error) {
  globalThis.__hawk_storage_migration_error = error.message;
}
globalThis.__hawk_storage_migration_mode = await getItem("settings", "mode");
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("out-of-order storage migrations are catchable");

    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-storage-migration-error.js",
                "globalThis.__hawk_storage_migration_error"
            )
            .expect("migration error is readable"),
        JsRuntimeValue::String(
            "js-runtime.capability.invalid: hawk:storage.migrate: migration versions must be contiguous and strictly increasing from the stored version".to_owned()
        )
    );
    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-storage-migration-mode.js",
                "globalThis.__hawk_storage_migration_mode"
            )
            .expect("migration side effect is readable"),
        JsRuntimeValue::String(String::new())
    );
}

#[test]
fn capabilities_storage_document_database_ops_round_trip_json_and_transactions() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .allow_storage_namespace("settings")
        .with_storage_document(
            "settings",
            "preset",
            serde_json::json!({ "name": "Init", "mix": 0.5 }),
        );
    let graph = module_runtime(
        r#"
import { getDocument, putDocument, transaction } from "hawk:storage";

const preset = getDocument("settings", "preset");
const written = putDocument("settings", "theme", { mode: "dark", scale: 2 });
const tx = transaction("settings", [
  { key: "accent", value: { color: "ember" } },
  { key: "density", value: "compact" },
]);

globalThis.__hawk_storage_preset = JSON.stringify(preset);
globalThis.__hawk_storage_written = JSON.stringify(written);
globalThis.__hawk_storage_tx_keys = tx.writtenKeys.join(",");
globalThis.__hawk_storage_accent = JSON.stringify(getDocument("settings", "accent"));
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("declared storage document access succeeds");

    assert_eq!(
        runtime
            .evaluate_script_value("read-storage-preset.js", "globalThis.__hawk_storage_preset")
            .expect("preset document is readable"),
        JsRuntimeValue::String(r#"{"name":"Init","mix":0.5}"#.to_owned())
    );
    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-storage-written.js",
                "globalThis.__hawk_storage_written"
            )
            .expect("written document is readable"),
        JsRuntimeValue::String(r#"{"mode":"dark","scale":2}"#.to_owned())
    );
    assert_eq!(
        runtime
            .evaluate_script_value("read-storage-tx.js", "globalThis.__hawk_storage_tx_keys")
            .expect("transaction result is readable"),
        JsRuntimeValue::String("accent,density".to_owned())
    );
    assert_eq!(
        runtime
            .evaluate_script_value("read-storage-accent.js", "globalThis.__hawk_storage_accent")
            .expect("transaction document is readable"),
        JsRuntimeValue::String(r#"{"color":"ember"}"#.to_owned())
    );
}

#[test]
fn capabilities_storage_document_transaction_rejects_invalid_keys_without_partial_commit() {
    let capabilities = HawkRuntimeCapabilities::for_test().allow_storage_namespace("settings");
    let graph = module_runtime(
        r#"
import { getDocument, transaction } from "hawk:storage";

try {
  transaction("settings", [
    { key: "safe", value: true },
    { key: "unsafe/key", value: false },
  ]);
  globalThis.__hawk_storage_tx_error = "resolved";
} catch (error) {
  globalThis.__hawk_storage_tx_error = error.message;
}
globalThis.__hawk_storage_safe_after_error = JSON.stringify(getDocument("settings", "safe"));
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("invalid storage transaction is catchable");

    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-storage-tx-error.js",
                "globalThis.__hawk_storage_tx_error"
            )
            .expect("transaction error is readable"),
        JsRuntimeValue::String(
            "js-runtime.capability.invalid: hawk:storage.transaction: storage document key is not a stable identifier: unsafe/key".to_owned()
        )
    );
    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-storage-safe-after-error.js",
                "globalThis.__hawk_storage_safe_after_error"
            )
            .expect("transaction rollback evidence is readable"),
        JsRuntimeValue::String("null".to_owned())
    );
}

#[test]
fn capabilities_storage_document_ops_are_denied_in_realtime_context() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .with_host_context(HawkHostContext::AudioRealtime)
        .allow_storage_namespace("settings");
    let graph = module_runtime(
        r#"
import { putDocument } from "hawk:storage";

try {
  putDocument("settings", "safe", { ok: true });
  globalThis.__hawk_storage_realtime_error = "resolved";
} catch (error) {
  globalThis.__hawk_storage_realtime_error = error.message;
}
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("realtime denial is catchable");

    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-storage-realtime-error.js",
                "globalThis.__hawk_storage_realtime_error"
            )
            .expect("realtime denial is readable"),
        JsRuntimeValue::String(
            "js-runtime.capability.realtime-denied: hawk:storage.putDocument: capability operation is unavailable in realtime audio context".to_owned()
        )
    );
}

#[test]
fn capabilities_deny_undeclared_file_paths() {
    let capabilities = HawkRuntimeCapabilities::for_test().allow_file_path("/project/config.json");
    let graph = module_runtime(
        r#"
import { readText } from "hawk:files";
await readText("/project/../secret.txt");
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    let error = runtime
        .execute_entrypoint_module()
        .expect_err("undeclared file path should be denied");

    assert!(
        error.message().contains("js-runtime.capability.denied"),
        "{}",
        error.message()
    );
    assert!(error.message().contains("hawk:files.readText"));
}

#[test]
fn capabilities_file_binary_reads_and_writes_uint8_arrays_when_declared() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .allow_file_path("/project/icon.bin")
        .with_file_bytes("/project/icon.bin", vec![0, 127, 255]);
    let graph = module_runtime(
        r#"
import { readBytes, writeBytes } from "hawk:files";
const initial = await readBytes("/project/icon.bin");
await writeBytes("/project/icon.bin", new Uint8Array([1, 2, 3, 255]));
const updated = await readBytes("/project/icon.bin");
globalThis.__hawk_file_binary =
  `${initial instanceof Uint8Array}:${Array.from(initial).join(",")}|${Array.from(updated).join(",")}`;
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("declared binary file access succeeds");

    assert_eq!(
        runtime
            .evaluate_script_value("read-file-binary.js", "globalThis.__hawk_file_binary")
            .expect("binary file result is readable"),
        JsRuntimeValue::String("true:0,127,255|1,2,3,255".to_owned())
    );
}

#[test]
fn capabilities_deny_undeclared_file_binary_paths() {
    let graph = module_runtime(
        r#"
import { readBytes } from "hawk:files";
await readBytes("/project/icon.bin");
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph(graph).expect("runtime initializes");

    let error = runtime
        .execute_entrypoint_module()
        .expect_err("undeclared binary file path should be denied");

    assert!(
        error.message().contains("js-runtime.capability.denied"),
        "{}",
        error.message()
    );
    assert!(error.message().contains("hawk:files.readBytes"));
}

#[test]
fn capabilities_file_pick_grants_returned_path_for_followup_reads() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .with_file_pick_result("/project/config.json")
        .with_file_text("/project/config.json", r#"{"theme":"dark"}"#);
    let graph = module_runtime(
        r#"
import { pickFile, readText } from "hawk:files";
const path = await pickFile();
globalThis.__hawk_picked_file_path = path;
globalThis.__hawk_picked_file_text = await readText(path);
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("picked file path should become user-granted");

    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-picked-file-path.js",
                "globalThis.__hawk_picked_file_path",
            )
            .expect("picked file path is readable"),
        JsRuntimeValue::String("/project/config.json".to_owned())
    );
    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-picked-file-text.js",
                "globalThis.__hawk_picked_file_text",
            )
            .expect("picked file text is readable"),
        JsRuntimeValue::String(r#"{"theme":"dark"}"#.to_owned())
    );
}

#[test]
fn capabilities_file_pick_requires_explicit_test_adapter_result() {
    let graph = module_runtime(
        r#"
import { pickFile } from "hawk:files";
await pickFile();
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph(graph).expect("runtime initializes");

    let error = runtime
        .execute_entrypoint_module()
        .expect_err("file pick without test adapter should fail clearly");

    assert!(
        error
            .message()
            .contains("js-runtime.capability.unsupported"),
        "{}",
        error.message()
    );
    assert!(error.message().contains("hawk:files.pickFile"));
}

#[test]
fn capabilities_folder_pick_grants_contained_paths_without_exact_file_declaration() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .with_file_pick_folder_result("/project/assets")
        .with_file_text("/project/assets/icon.txt", "ok");
    let graph = module_runtime(
        r#"
import { pickFolder, readText } from "hawk:files";
const folder = await pickFolder();
globalThis.__hawk_picked_folder_path = folder;
globalThis.__hawk_picked_folder_text = await readText(`${folder}/icon.txt`);
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("picked folder path should become a user grant");

    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-picked-folder-path.js",
                "globalThis.__hawk_picked_folder_path",
            )
            .expect("picked folder path is readable"),
        JsRuntimeValue::String("/project/assets".to_owned())
    );
    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-picked-folder-text.js",
                "globalThis.__hawk_picked_folder_text",
            )
            .expect("picked folder text is readable"),
        JsRuntimeValue::String("ok".to_owned())
    );
}

#[test]
fn capabilities_folder_pick_does_not_grant_sibling_prefix_paths() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .with_file_pick_folder_result("/project/assets")
        .with_file_text("/project/assets-secret/icon.txt", "nope");
    let graph = module_runtime(
        r#"
import { pickFolder, readText } from "hawk:files";
await pickFolder();
try {
  await readText("/project/assets-secret/icon.txt");
  globalThis.__hawk_folder_sibling_error = "resolved";
} catch (error) {
  globalThis.__hawk_folder_sibling_error = error.message;
}
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("sibling denial should be catchable");

    let error = runtime
        .evaluate_script_value(
            "read-folder-sibling-error.js",
            "globalThis.__hawk_folder_sibling_error",
        )
        .expect("sibling denial error is readable");
    match error {
        JsRuntimeValue::String(message) => {
            assert!(
                message.contains("js-runtime.capability.denied"),
                "{message}"
            );
            assert!(message.contains("hawk:files.readText"), "{message}");
        }
        value => panic!("expected string error, got {value:?}"),
    }
}

#[test]
fn capabilities_deny_undeclared_file_watches() {
    let graph = module_runtime(
        r#"
import { watch } from "hawk:files";
await watch("/project/config.json");
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph(graph).expect("runtime initializes");

    let error = runtime
        .execute_entrypoint_module()
        .expect_err("undeclared file watch should be denied");

    assert!(
        error.message().contains("js-runtime.capability.denied"),
        "{}",
        error.message()
    );
    assert!(error.message().contains("hawk:files.watch"));
}

#[test]
fn capabilities_file_watch_delivers_declared_test_events() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .allow_file_path("/project/config.json")
        .with_file_watch_event("/project/config.json", "modified");
    let graph = module_runtime(
        r#"
import { watch } from "hawk:files";
const event = await watch("/project/config.json");
globalThis.__hawk_file_watch_path = event.path;
globalThis.__hawk_file_watch_kind = event.kind;
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("declared file watch succeeds");

    assert_eq!(
        runtime
            .evaluate_script_value("read-watch-path.js", "globalThis.__hawk_file_watch_path")
            .expect("watch path is readable"),
        JsRuntimeValue::String("/project/config.json".to_owned())
    );
    assert_eq!(
        runtime
            .evaluate_script_value("read-watch-kind.js", "globalThis.__hawk_file_watch_kind")
            .expect("watch kind is readable"),
        JsRuntimeValue::String("modified".to_owned())
    );
}

#[test]
fn capabilities_file_import_export_copy_bytes_through_explicit_user_grants() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .allow_file_path("/project/assets/logo.bin")
        .with_file_bytes("/downloads/logo.bin", vec![1, 2, 3, 4])
        .with_file_import_result("/downloads/logo.bin")
        .with_file_export_result("/exports/logo.bin");
    let graph = module_runtime(
        r#"
import { exportFile, importFile, readBytes } from "hawk:files";

const imported = await importFile({ title: "Import logo" }, "/project/assets/logo.bin");
const bytes = await readBytes(imported.path);
const exported = await exportFile("/project/assets/logo.bin", { suggestedName: "logo.bin" });
globalThis.__hawk_file_import = `${imported.sourcePath}->${imported.path}:${Array.from(bytes).join(",")}`;
globalThis.__hawk_file_export = `${exported.path}:${exported.bytesWritten}`;
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("declared file import/export succeeds");

    assert_eq!(
        runtime
            .evaluate_script_value("read-file-import.js", "globalThis.__hawk_file_import")
            .expect("file import result is readable"),
        JsRuntimeValue::String("/downloads/logo.bin->/project/assets/logo.bin:1,2,3,4".to_owned())
    );
    assert_eq!(
        runtime
            .evaluate_script_value("read-file-export.js", "globalThis.__hawk_file_export")
            .expect("file export result is readable"),
        JsRuntimeValue::String("/exports/logo.bin:4".to_owned())
    );
}

#[test]
fn capabilities_plugin_parameters_require_plugin_ui_context() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .with_host_context(HawkHostContext::PluginUi)
        .allow_plugin_parameter("gain")
        .with_plugin_parameter("gain", 0.25);
    let graph = module_runtime(
        r#"
import { readParameter, writeParameter } from "hawk:plugin";
globalThis.__hawk_gain_before = await readParameter("gain");
await writeParameter("gain", 0.75);
globalThis.__hawk_gain_after = await readParameter("gain");
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("declared plugin parameter access succeeds");

    assert_eq!(
        runtime
            .evaluate_script_value("read-gain-before.js", "globalThis.__hawk_gain_before")
            .expect("gain is readable"),
        JsRuntimeValue::Number(0.25)
    );
    assert_eq!(
        runtime
            .evaluate_script_value("read-gain-after.js", "globalThis.__hawk_gain_after")
            .expect("gain is readable"),
        JsRuntimeValue::Number(0.75)
    );
}

#[test]
fn capabilities_plugin_automation_gesture_wraps_parameter_write() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .with_host_context(HawkHostContext::PluginUi)
        .allow_plugin_parameter("gain")
        .with_plugin_parameter("gain", 0.25);
    let graph = module_runtime(
        r#"
import {
  beginAutomationGesture,
  readParameter,
  writeParameter,
  endAutomationGesture,
} from "hawk:plugin";

globalThis.__hawk_gesture_begin = await beginAutomationGesture("gain");
await writeParameter("gain", 0.5);
globalThis.__hawk_gesture_value = await readParameter("gain");
globalThis.__hawk_gesture_end = await endAutomationGesture("gain");
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("automation gesture succeeds in plugin UI");

    assert_eq!(
        runtime
            .evaluate_script_value("read-gesture-begin.js", "globalThis.__hawk_gesture_begin")
            .expect("begin gesture result is readable"),
        JsRuntimeValue::String("hawk-automation:gain:begin".to_owned())
    );
    assert_eq!(
        runtime
            .evaluate_script_value("read-gesture-value.js", "globalThis.__hawk_gesture_value")
            .expect("gesture value is readable"),
        JsRuntimeValue::Number(0.5)
    );
    assert_eq!(
        runtime
            .evaluate_script_value("read-gesture-end.js", "globalThis.__hawk_gesture_end")
            .expect("end gesture result is readable"),
        JsRuntimeValue::String("hawk-automation:gain:end".to_owned())
    );
}

#[test]
fn capabilities_plugin_state_round_trips_when_declared() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .with_host_context(HawkHostContext::PluginUi)
        .allow_plugin_state()
        .with_plugin_state(r#"{"preset":"Init"}"#);
    let graph = module_runtime(
        r#"
import { loadState, saveState } from "hawk:plugin";

globalThis.__hawk_plugin_state_before = await loadState();
await saveState(JSON.stringify({ preset: "Wide", version: 2 }));
globalThis.__hawk_plugin_state_after = await loadState();
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("declared plugin state access succeeds");

    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-plugin-state-before.js",
                "globalThis.__hawk_plugin_state_before"
            )
            .expect("state before is readable"),
        JsRuntimeValue::String(r#"{"preset":"Init"}"#.to_owned())
    );
    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-plugin-state-after.js",
                "globalThis.__hawk_plugin_state_after"
            )
            .expect("state after is readable"),
        JsRuntimeValue::String(r#"{"preset":"Wide","version":2}"#.to_owned())
    );
}

#[test]
fn capabilities_plugin_presets_round_trip_when_state_is_declared() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .with_host_context(HawkHostContext::PluginUi)
        .allow_plugin_state()
        .with_plugin_preset("init", r#"{"name":"Init"}"#);
    let graph = module_runtime(
        r#"
import { loadPreset, savePreset } from "hawk:plugin";

globalThis.__hawk_plugin_preset_before = await loadPreset("init");
await savePreset("init", JSON.stringify({ name: "Wide", version: 2 }));
globalThis.__hawk_plugin_preset_after = await loadPreset("init");
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("declared plugin preset access succeeds");

    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-plugin-preset-before.js",
                "globalThis.__hawk_plugin_preset_before",
            )
            .expect("preset before is readable"),
        JsRuntimeValue::String(r#"{"name":"Init"}"#.to_owned())
    );
    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-plugin-preset-after.js",
                "globalThis.__hawk_plugin_preset_after",
            )
            .expect("preset after is readable"),
        JsRuntimeValue::String(r#"{"name":"Wide","version":2}"#.to_owned())
    );
}

#[test]
fn capabilities_plugin_transport_reports_host_time_info_in_plugin_ui_context() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .with_host_context(HawkHostContext::PluginUi)
        .with_plugin_transport_info(HawkPluginTransportInfo {
            playing: true,
            sample_rate: 48_000.0,
            sample_position: 96_000,
            tempo_bpm: 128.0,
            beat_position: 12.5,
            time_signature_numerator: 7,
            time_signature_denominator: 8,
        });
    let graph = module_runtime(
        r#"
import { getTransport } from "hawk:plugin";
const transport = await getTransport();
globalThis.__hawk_plugin_transport =
  `${transport.playing}:${transport.sampleRate}:${transport.samplePosition}:${transport.tempoBpm}:${transport.beatPosition}:${transport.timeSignatureNumerator}/${transport.timeSignatureDenominator}`;
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("plugin transport access succeeds in plugin UI");

    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-plugin-transport.js",
                "globalThis.__hawk_plugin_transport",
            )
            .expect("plugin transport is readable"),
        JsRuntimeValue::String("true:48000:96000:128:12.5:7/8".to_owned())
    );
}

#[test]
fn capabilities_plugin_transport_requires_plugin_ui_context() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .with_plugin_transport_info(HawkPluginTransportInfo::default());
    let graph = module_runtime(
        r#"
import { getTransport } from "hawk:plugin";
await getTransport();
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    let error = runtime
        .execute_entrypoint_module()
        .expect_err("plugin transport should be denied outside plugin UI");

    assert!(
        error.message().contains("js-runtime.capability.denied"),
        "{}",
        error.message()
    );
    assert!(error.message().contains("hawk:plugin.getTransport"));
}

#[test]
fn capabilities_plugin_editor_resize_and_focus_route_through_plugin_ui_host() {
    let capabilities =
        HawkRuntimeCapabilities::for_test().with_host_context(HawkHostContext::PluginUi);
    let graph = module_runtime(
        r#"
import { focusEditor, resizeEditor } from "hawk:plugin";
const size = await resizeEditor({ width: 640, height: 360 });
const focus = await focusEditor();
globalThis.__hawk_plugin_editor = `${size.width}x${size.height}:${focus.focused}`;
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("plugin editor host operations succeed in plugin UI");

    assert_eq!(
        runtime
            .evaluate_script_value("read-plugin-editor.js", "globalThis.__hawk_plugin_editor")
            .expect("plugin editor result is readable"),
        JsRuntimeValue::String("640x360:true".to_owned())
    );
}

#[test]
fn capabilities_plugin_editor_resize_rejects_invalid_dimensions() {
    let capabilities =
        HawkRuntimeCapabilities::for_test().with_host_context(HawkHostContext::PluginUi);
    let graph = module_runtime(
        r#"
import { resizeEditor } from "hawk:plugin";
try {
  await resizeEditor({ width: 0, height: 360 });
  globalThis.__hawk_plugin_resize_error = "resolved";
} catch (error) {
  globalThis.__hawk_plugin_resize_error = error.message;
}
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("invalid resize should be catchable");

    let error = runtime
        .evaluate_script_value(
            "read-plugin-resize-error.js",
            "globalThis.__hawk_plugin_resize_error",
        )
        .expect("resize error is readable");
    match error {
        JsRuntimeValue::String(message) => {
            assert!(
                message.contains("js-runtime.capability.invalid"),
                "{message}"
            );
            assert!(message.contains("hawk:plugin.resizeEditor"), "{message}");
        }
        value => panic!("expected string error, got {value:?}"),
    }
}

#[test]
fn capabilities_deny_plugin_parameters_in_realtime_context() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .with_host_context(HawkHostContext::AudioRealtime)
        .allow_plugin_parameter("gain")
        .with_plugin_parameter("gain", 0.25);
    let graph = module_runtime(
        r#"
import { readParameter } from "hawk:plugin";
await readParameter("gain");
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    let error = runtime
        .execute_entrypoint_module()
        .expect_err("plugin parameter access should be denied in realtime context");

    assert!(
        error
            .message()
            .contains("js-runtime.capability.realtime-denied"),
        "{}",
        error.message()
    );
}

#[test]
fn capabilities_resolve_planned_hawk_module_surface() {
    let graph = module_runtime(
        r#"
import * as api from "hawk:api";
import * as storage from "hawk:storage";
import * as secrets from "hawk:secrets";
import * as files from "hawk:files";
import * as desktop from "hawk:desktop";
import * as audio from "hawk:audio";
import * as dsp from "hawk:dsp";
import * as ai from "hawk:ai";

globalThis.__hawk_module_surface = [
  typeof api.call,
  typeof storage.getDocument,
  typeof storage.transaction,
  typeof secrets.read,
  typeof files.importFile,
  typeof files.exportFile,
  typeof desktop.showOpenDialog,
  typeof desktop.onDeepLink,
  typeof desktop.setWindowMode,
  typeof desktop.closeWindow,
  typeof audio.subscribeMeters,
  typeof audio.nextControl,
  typeof dsp.sendControl,
  typeof dsp.updateParameterGraph,
  typeof dsp.startOfflineRender,
  typeof dsp.exportOfflineRender,
  typeof ai.callProvider,
].join(",");
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph(graph).expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("planned capability modules resolve");

    assert_eq!(
        runtime
            .evaluate_script_value("read-module-surface.js", "globalThis.__hawk_module_surface")
            .expect("surface is readable"),
        JsRuntimeValue::String(
            "function,function,function,function,function,function,function,function,function,function,function,function,function,function,function,function,function"
                .to_owned()
        )
    );
}

#[test]
fn capabilities_runtime_aggregate_module_reexports_domain_surfaces() {
    let graph = module_runtime(
        r#"
import runtime, { network, storage, plugin, audio, dsp } from "hawk:runtime";

globalThis.__hawk_runtime_aggregate_surface = [
  typeof runtime.network.request,
  typeof network.request,
  typeof runtime.storage.getDocument,
  typeof storage.transaction,
  typeof runtime.plugin.readParameter,
  typeof plugin.getTransport,
  typeof runtime.audio.subscribeMeters,
  typeof audio.nextControl,
  typeof runtime.dsp.sendControl,
  typeof dsp.startOfflineRender,
].join(",");
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph(graph).expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("hawk:runtime aggregate module resolves");

    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-runtime-aggregate-surface.js",
                "globalThis.__hawk_runtime_aggregate_surface",
            )
            .expect("runtime aggregate surface is readable"),
        JsRuntimeValue::String(
            "function,function,function,function,function,function,function,function,function,function"
                .to_owned()
        )
    );
}

#[test]
fn capabilities_planned_domain_operations_fail_with_structured_denials_when_undeclared() {
    let graph = module_runtime(
        r#"
import { streamProvider } from "hawk:ai";
for await (const _chunk of streamProvider("assistant", { prompt: "status" })) {}
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph(graph).expect("runtime initializes");

    let error = runtime
        .execute_entrypoint_module()
        .expect_err("undeclared planned domain operation should fail clearly");

    assert!(
        error.message().contains("js-runtime.capability.denied"),
        "{}",
        error.message()
    );
    assert!(error.message().contains("hawk:ai.streamProvider"));
    assert!(error.message().contains("assistant"));
}

#[test]
fn capabilities_deny_undeclared_named_api_calls() {
    let graph = module_runtime(
        r#"
import { call } from "hawk:api";
await call("customer.lookup", { id: 7 });
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph(graph).expect("runtime initializes");

    let error = runtime
        .execute_entrypoint_module()
        .expect_err("undeclared named API call should be denied");

    assert!(
        error.message().contains("js-runtime.capability.denied"),
        "{}",
        error.message()
    );
    assert!(error.message().contains("hawk:api.call"));
    assert!(error.message().contains("customer.lookup"));
}

#[test]
fn capabilities_allow_named_api_calls_with_explicit_test_backend() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .allow_api_endpoint("customer.lookup")
        .with_api_response(
            "customer.lookup",
            HawkNetworkResponse::json(200, r#"{"id":7,"name":"Ada"}"#),
        );
    let graph = module_runtime(
        r#"
import { call } from "hawk:api";
const response = await call("customer.lookup", { id: 7 });
globalThis.__hawk_api_status = response.status;
globalThis.__hawk_api_body = response.body;
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("declared named API call succeeds");

    assert_eq!(
        runtime
            .evaluate_script_value("read-api-status.js", "globalThis.__hawk_api_status")
            .expect("status is readable"),
        JsRuntimeValue::Number(200.0)
    );
    assert_eq!(
        runtime
            .evaluate_script_value("read-api-body.js", "globalThis.__hawk_api_body")
            .expect("body is readable"),
        JsRuntimeValue::String(r#"{"id":7,"name":"Ada"}"#.to_owned())
    );
}

#[test]
fn capabilities_deny_undeclared_secrets() {
    let graph = module_runtime(
        r#"
import { read } from "hawk:secrets";
await read("provider.token");
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph(graph).expect("runtime initializes");

    let error = runtime
        .execute_entrypoint_module()
        .expect_err("undeclared secret access should be denied");

    assert!(
        error.message().contains("js-runtime.capability.denied"),
        "{}",
        error.message()
    );
    assert!(error.message().contains("hawk:secrets.read"));
    assert!(error.message().contains("provider.token"));
}

#[test]
fn capabilities_secret_handles_are_redacted() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .allow_secret("provider.token")
        .with_secret_value("provider.token", "sk-test-123");
    let graph = module_runtime(
        r#"
import { read } from "hawk:secrets";
const token = await read("provider.token");
globalThis.__hawk_secret_string = String(token);
globalThis.__hawk_secret_json = JSON.stringify(token);
globalThis.__hawk_secret_value = token.value ?? "";
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("declared secret handle succeeds");

    let rendered = runtime
        .evaluate_script_value("read-secret-string.js", "globalThis.__hawk_secret_string")
        .expect("secret string is readable");
    let json = runtime
        .evaluate_script_value("read-secret-json.js", "globalThis.__hawk_secret_json")
        .expect("secret json is readable");
    let raw = runtime
        .evaluate_script_value("read-secret-value.js", "globalThis.__hawk_secret_value")
        .expect("secret raw probe is readable");

    assert_eq!(
        rendered,
        JsRuntimeValue::String("[HawkSecret provider.token redacted]".to_owned())
    );
    assert_eq!(
        json,
        JsRuntimeValue::String(
            r#"{"type":"hawk.secret","name":"provider.token","redacted":true}"#.to_owned()
        )
    );
    assert_eq!(raw, JsRuntimeValue::String(String::new()));
}

#[test]
fn capabilities_named_api_calls_accept_declared_secret_handles_without_leaking_raw_secret() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .allow_api_endpoint("provider.fetch")
        .with_api_response(
            "provider.fetch",
            HawkNetworkResponse::json(200, r#"{"ok":true}"#),
        )
        .allow_secret("provider.token")
        .with_secret_value("provider.token", "sk-test-123");
    let graph = module_runtime(
        r#"
import { call } from "hawk:api";
import { read } from "hawk:secrets";
const token = await read("provider.token");
const response = await call("provider.fetch", { query: "status" }, { secrets: [token] });
globalThis.__hawk_api_secret_status = response.status;
globalThis.__hawk_api_secret_body = response.body;
globalThis.__hawk_secret_after_api = JSON.stringify(token);
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("declared API call with secret handle succeeds");

    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-api-secret-status.js",
                "globalThis.__hawk_api_secret_status"
            )
            .expect("status is readable"),
        JsRuntimeValue::Number(200.0)
    );
    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-api-secret-body.js",
                "globalThis.__hawk_api_secret_body"
            )
            .expect("body is readable"),
        JsRuntimeValue::String(r#"{"ok":true}"#.to_owned())
    );
    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-secret-after-api.js",
                "globalThis.__hawk_secret_after_api"
            )
            .expect("secret handle is readable"),
        JsRuntimeValue::String(
            r#"{"type":"hawk.secret","name":"provider.token","redacted":true}"#.to_owned()
        )
    );
}

#[test]
fn capabilities_deny_undeclared_ai_provider_calls() {
    let graph = module_runtime(
        r#"
import { callProvider } from "hawk:ai";
await callProvider("assistant", { prompt: "status" });
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph(graph).expect("runtime initializes");

    let error = runtime
        .execute_entrypoint_module()
        .expect_err("undeclared AI provider call should be denied");

    assert!(
        error.message().contains("js-runtime.capability.denied"),
        "{}",
        error.message()
    );
    assert!(error.message().contains("hawk:ai.callProvider"));
    assert!(error.message().contains("assistant"));
}

#[test]
fn capabilities_ai_provider_calls_use_declared_provider_budget_timeout_and_secret_handles() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .allow_ai_provider("assistant")
        .with_ai_response(
            "assistant",
            HawkNetworkResponse::json(200, r#"{"message":"ready"}"#),
        )
        .allow_secret("provider.token")
        .with_secret_value("provider.token", "sk-test-123");
    let graph = module_runtime(
        r#"
import { callProvider } from "hawk:ai";
import { read } from "hawk:secrets";
const token = await read("provider.token");
const response = await callProvider(
  "assistant",
  { prompt: "status" },
  { budgetTokens: 128, timeoutMs: 250, secrets: [token] },
);
globalThis.__hawk_ai_status = response.status;
globalThis.__hawk_ai_body = response.body;
globalThis.__hawk_ai_secret_json = JSON.stringify(token);
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("declared AI provider call succeeds");

    assert_eq!(
        runtime
            .evaluate_script_value("read-ai-status.js", "globalThis.__hawk_ai_status")
            .expect("AI status is readable"),
        JsRuntimeValue::Number(200.0)
    );
    assert_eq!(
        runtime
            .evaluate_script_value("read-ai-body.js", "globalThis.__hawk_ai_body")
            .expect("AI body is readable"),
        JsRuntimeValue::String(r#"{"message":"ready"}"#.to_owned())
    );
    assert_eq!(
        runtime
            .evaluate_script_value("read-ai-secret.js", "globalThis.__hawk_ai_secret_json")
            .expect("AI secret handle is readable"),
        JsRuntimeValue::String(
            r#"{"type":"hawk.secret","name":"provider.token","redacted":true}"#.to_owned()
        )
    );
}

#[test]
fn capabilities_ai_provider_streams_declared_chunks_with_secret_handles() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .allow_ai_provider("assistant")
        .with_ai_stream_response("assistant", vec!["hel".to_owned(), "lo".to_owned()])
        .allow_secret("provider.token")
        .with_secret_value("provider.token", "sk-test-123");
    let graph = module_runtime(
        r#"
import { streamProvider } from "hawk:ai";
import { read } from "hawk:secrets";
const token = await read("provider.token");
const chunks = [];
for await (const chunk of streamProvider(
  "assistant",
  { prompt: "status" },
  { budgetTokens: 128, timeoutMs: 250, secrets: [token] },
)) {
  chunks.push(chunk.text);
}
globalThis.__hawk_ai_stream = chunks.join("");
globalThis.__hawk_ai_stream_secret_json = JSON.stringify(token);
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("declared AI provider stream succeeds");

    assert_eq!(
        runtime
            .evaluate_script_value("read-ai-stream.js", "globalThis.__hawk_ai_stream")
            .expect("AI stream is readable"),
        JsRuntimeValue::String("hello".to_owned())
    );
    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-ai-stream-secret.js",
                "globalThis.__hawk_ai_stream_secret_json"
            )
            .expect("AI stream secret handle is readable"),
        JsRuntimeValue::String(
            r#"{"type":"hawk.secret","name":"provider.token","redacted":true}"#.to_owned()
        )
    );
}

#[test]
fn capabilities_ai_provider_streams_are_denied_in_realtime_context() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .with_host_context(HawkHostContext::AudioRealtime)
        .allow_ai_provider("assistant")
        .with_ai_stream_response("assistant", vec!["hello".to_owned()]);
    let graph = module_runtime(
        r#"
import { streamProvider } from "hawk:ai";
for await (const _chunk of streamProvider("assistant", { prompt: "status" })) {
}
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    let error = runtime
        .execute_entrypoint_module()
        .expect_err("AI provider stream should be denied in realtime context");

    assert!(
        error
            .message()
            .contains("js-runtime.capability.realtime-denied"),
        "{}",
        error.message()
    );
    assert!(error.message().contains("hawk:ai.streamProvider"));
}

#[test]
fn capabilities_audio_meter_streams_require_declaration() {
    let graph = module_runtime(
        r#"
import { subscribeMeters } from "hawk:audio";
await subscribeMeters({ source: "master" });
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph(graph).expect("runtime initializes");

    let error = runtime
        .execute_entrypoint_module()
        .expect_err("undeclared audio meter stream should be denied");

    assert!(
        error.message().contains("js-runtime.capability.denied"),
        "{}",
        error.message()
    );
    assert!(error.message().contains("hawk:audio.subscribeMeters"));
    assert!(error.message().contains("master"));
}

#[test]
fn capabilities_audio_meter_streams_deliver_bounded_test_frames() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .with_host_context(HawkHostContext::PluginUi)
        .allow_audio_meter_stream("master")
        .with_audio_meter_frame("master", vec![0.125, 0.5]);
    let graph = module_runtime(
        r#"
import { subscribeMeters } from "hawk:audio";
const frame = await subscribeMeters({ source: "master" });
globalThis.__hawk_audio_source = frame.source;
globalThis.__hawk_audio_values = frame.values.join(",");
globalThis.__hawk_audio_dropped = frame.dropped;
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("declared audio meter stream succeeds");

    assert_eq!(
        runtime
            .evaluate_script_value("read-audio-source.js", "globalThis.__hawk_audio_source")
            .expect("source is readable"),
        JsRuntimeValue::String("master".to_owned())
    );
    assert_eq!(
        runtime
            .evaluate_script_value("read-audio-values.js", "globalThis.__hawk_audio_values")
            .expect("values are readable"),
        JsRuntimeValue::String("0.125,0.5".to_owned())
    );
    assert_eq!(
        runtime
            .evaluate_script_value("read-audio-dropped.js", "globalThis.__hawk_audio_dropped")
            .expect("drop count is readable"),
        JsRuntimeValue::Number(0.0)
    );
}

#[test]
fn capabilities_audio_control_input_delivers_declared_midi_events() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .with_host_context(HawkHostContext::PluginUi)
        .allow_audio_control_input("midi")
        .with_audio_control_event("midi", "note-on", 60, 0.75);
    let graph = module_runtime(
        r#"
import { nextControl } from "hawk:audio";
const event = await nextControl({ source: "midi" });
globalThis.__hawk_audio_control =
  `${event.source}:${event.kind}:${event.value}:${event.normalizedValue}:${event.dropped}`;
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("declared audio control input succeeds");

    assert_eq!(
        runtime
            .evaluate_script_value("read-audio-control.js", "globalThis.__hawk_audio_control",)
            .expect("audio control event is readable"),
        JsRuntimeValue::String("midi:note-on:60:0.75:0".to_owned())
    );
}

#[test]
fn capabilities_audio_transport_reports_tempo_meter_and_playhead() {
    let capabilities =
        HawkRuntimeCapabilities::for_test().with_audio_transport_info(HawkAudioTransportInfo {
            playing: true,
            sample_rate: 44_100.0,
            sample_position: 88_200,
            tempo_bpm: 96.0,
            beat_position: 8.25,
            time_signature_numerator: 3,
            time_signature_denominator: 4,
        });
    let graph = module_runtime(
        r#"
import { transport } from "hawk:audio";
const snapshot = await transport();
globalThis.__hawk_audio_transport =
  `${snapshot.playing}:${snapshot.sampleRate}:${snapshot.samplePosition}:${snapshot.tempoBpm}:${snapshot.beatPosition}:${snapshot.timeSignatureNumerator}/${snapshot.timeSignatureDenominator}`;
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("audio transport access succeeds");

    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-audio-transport.js",
                "globalThis.__hawk_audio_transport",
            )
            .expect("audio transport is readable"),
        JsRuntimeValue::String("true:44100:88200:96:8.25:3/4".to_owned())
    );
}

#[test]
fn capabilities_audio_transport_is_denied_in_realtime_context() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .with_host_context(HawkHostContext::AudioRealtime)
        .with_audio_transport_info(HawkAudioTransportInfo::default());
    let graph = module_runtime(
        r#"
import { transport } from "hawk:audio";
await transport();
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    let error = runtime
        .execute_entrypoint_module()
        .expect_err("audio transport should be denied in realtime context");

    assert!(
        error
            .message()
            .contains("js-runtime.capability.realtime-denied"),
        "{}",
        error.message()
    );
    assert!(error.message().contains("hawk:audio.transport"));
}

#[test]
fn capabilities_audio_control_input_is_denied_in_realtime_context() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .with_host_context(HawkHostContext::AudioRealtime)
        .allow_audio_control_input("midi")
        .with_audio_control_event("midi", "note-on", 60, 0.75);
    let graph = module_runtime(
        r#"
import { nextControl } from "hawk:audio";
await nextControl({ source: "midi" });
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    let error = runtime
        .execute_entrypoint_module()
        .expect_err("audio control input should be denied in realtime context");

    assert!(
        error
            .message()
            .contains("js-runtime.capability.realtime-denied"),
        "{}",
        error.message()
    );
    assert!(error.message().contains("hawk:audio.nextControl"));
}

#[test]
fn capabilities_dsp_control_queue_is_bounded() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .with_host_context(HawkHostContext::PluginUi)
        .allow_dsp_control_queue(1);
    let graph = module_runtime(
        r#"
import { sendControl } from "hawk:dsp";
const first = await sendControl({ type: "set-mode", value: "wide" });
const second = await sendControl({ type: "set-mode", value: "narrow" });
globalThis.__hawk_dsp_first_accepted = first.accepted;
globalThis.__hawk_dsp_first_depth = first.queueDepth;
globalThis.__hawk_dsp_second_accepted = second.accepted;
globalThis.__hawk_dsp_second_dropped = second.dropped;
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("declared DSP control queue succeeds");

    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-dsp-first-accepted.js",
                "globalThis.__hawk_dsp_first_accepted"
            )
            .expect("first accepted is readable"),
        JsRuntimeValue::Bool(true)
    );
    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-dsp-first-depth.js",
                "globalThis.__hawk_dsp_first_depth"
            )
            .expect("first depth is readable"),
        JsRuntimeValue::Number(1.0)
    );
    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-dsp-second-accepted.js",
                "globalThis.__hawk_dsp_second_accepted"
            )
            .expect("second accepted is readable"),
        JsRuntimeValue::Bool(false)
    );
    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-dsp-second-dropped.js",
                "globalThis.__hawk_dsp_second_dropped"
            )
            .expect("second drop count is readable"),
        JsRuntimeValue::Number(1.0)
    );
}

#[test]
fn capabilities_dsp_parameter_graph_updates_when_declared() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .with_host_context(HawkHostContext::PluginUi)
        .allow_dsp_parameter_graph_updates();
    let graph = module_runtime(
        r#"
import { updateParameterGraph } from "hawk:dsp";
const result = await updateParameterGraph({
  nodes: [
    { id: "gain", kind: "parameter", defaultValue: 0.5 },
    { id: "filter", kind: "processor" }
  ],
  edges: [{ from: "gain", to: "filter" }]
});
globalThis.__hawk_dsp_parameter_graph =
  `${result.revision}:${result.nodeCount}:${result.edgeCount}`;
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("declared DSP parameter graph update succeeds");

    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-dsp-parameter-graph.js",
                "globalThis.__hawk_dsp_parameter_graph",
            )
            .expect("DSP parameter graph update is readable"),
        JsRuntimeValue::String("1:2:1".to_owned())
    );
}

#[test]
fn capabilities_dsp_parameter_graph_rejects_edges_to_unknown_nodes() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .with_host_context(HawkHostContext::PluginUi)
        .allow_dsp_parameter_graph_updates();
    let graph = module_runtime(
        r#"
import { updateParameterGraph } from "hawk:dsp";
await updateParameterGraph({
  nodes: [{ id: "gain", kind: "parameter" }],
  edges: [{ from: "gain", to: "missing" }]
});
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    let error = runtime
        .execute_entrypoint_module()
        .expect_err("DSP parameter graph should reject edges to unknown nodes");

    assert!(
        error.message().contains("js-runtime.capability.invalid"),
        "{}",
        error.message()
    );
    assert!(error.message().contains("hawk:dsp.updateParameterGraph"));
    assert!(error.message().contains("missing"));
}

#[test]
fn capabilities_dsp_analysis_jobs_can_be_cancelled_when_declared() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .with_host_context(HawkHostContext::PluginUi)
        .allow_dsp_analysis_jobs(2);
    let graph = module_runtime(
        r#"
import { cancelAnalysisJob, startAnalysisJob } from "hawk:dsp";
const job = await startAnalysisJob({ type: "spectrum", source: "main" });
const cancelled = await cancelAnalysisJob(job.id);
globalThis.__hawk_dsp_analysis = `${job.id}:${job.status}:${cancelled.status}`;
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("declared DSP analysis job can be cancelled");

    assert_eq!(
        runtime
            .evaluate_script_value("read-dsp-analysis.js", "globalThis.__hawk_dsp_analysis")
            .expect("DSP analysis result is readable"),
        JsRuntimeValue::String("hawk-dsp-analysis:1:running:cancelled".to_owned())
    );
}

#[test]
fn capabilities_dsp_offline_render_exports_declared_jobs() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .with_host_context(HawkHostContext::PluginUi)
        .allow_dsp_offline_render_jobs(1);
    let graph = module_runtime(
        r#"
import { exportOfflineRender, startOfflineRender } from "hawk:dsp";
const job = await startOfflineRender({
  source: "main",
  outputPath: "/exports/main.wav",
});
const exported = await exportOfflineRender(job.id);
globalThis.__hawk_dsp_offline_render = `${job.id}:${job.status}:${exported.status}:${exported.path}`;
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("declared DSP offline render can be exported");

    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-dsp-offline-render.js",
                "globalThis.__hawk_dsp_offline_render",
            )
            .expect("DSP offline render result is readable"),
        JsRuntimeValue::String(
            "hawk-dsp-offline-render:1:running:exported:/exports/main.wav".to_owned()
        )
    );
}

#[test]
fn capabilities_dsp_parameter_graph_updates_are_denied_in_realtime_context() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .with_host_context(HawkHostContext::AudioRealtime)
        .allow_dsp_parameter_graph_updates();
    let graph = module_runtime(
        r#"
import { updateParameterGraph } from "hawk:dsp";
await updateParameterGraph({ nodes: [], edges: [] });
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    let error = runtime
        .execute_entrypoint_module()
        .expect_err("DSP parameter graph updates should be denied in realtime context");

    assert!(
        error
            .message()
            .contains("js-runtime.capability.realtime-denied"),
        "{}",
        error.message()
    );
    assert!(error.message().contains("hawk:dsp.updateParameterGraph"));
}

#[test]
fn capabilities_dsp_analysis_jobs_are_denied_in_realtime_context() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .with_host_context(HawkHostContext::AudioRealtime)
        .allow_dsp_analysis_jobs(1);
    let graph = module_runtime(
        r#"
import { startAnalysisJob } from "hawk:dsp";
await startAnalysisJob({ type: "spectrum", source: "main" });
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    let error = runtime
        .execute_entrypoint_module()
        .expect_err("DSP analysis jobs should be denied in realtime context");

    assert!(
        error
            .message()
            .contains("js-runtime.capability.realtime-denied"),
        "{}",
        error.message()
    );
    assert!(error.message().contains("hawk:dsp.startAnalysisJob"));
}

#[test]
fn capabilities_deny_desktop_operations_by_default() {
    let graph = module_runtime(
        r#"
import { setWindowTitle } from "hawk:desktop";
await setWindowTitle("Hawk2UI");
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph(graph).expect("runtime initializes");

    let error = runtime
        .execute_entrypoint_module()
        .expect_err("undeclared desktop operation should be denied");

    assert!(
        error.message().contains("js-runtime.capability.denied"),
        "{}",
        error.message()
    );
    assert!(error.message().contains("hawk:desktop.setWindowTitle"));
}

#[test]
fn capabilities_allow_desktop_operations_with_explicit_test_backend() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .with_host_context(HawkHostContext::Desktop)
        .allow_desktop_operation("hawk:desktop.setWindowTitle")
        .allow_desktop_operation("hawk:desktop.showOpenDialog")
        .allow_desktop_operation("hawk:desktop.readClipboard")
        .allow_desktop_operation("hawk:desktop.writeClipboard")
        .allow_desktop_operation("hawk:desktop.notify")
        .allow_desktop_operation("hawk:desktop.registerShortcut")
        .allow_desktop_operation("hawk:desktop.openExternal")
        .with_open_dialog_result(vec!["/project/input.wav".to_owned()])
        .with_clipboard_text("initial clipboard");
    let graph = module_runtime(
        r#"
import {
  setWindowTitle,
  showOpenDialog,
  readClipboard,
  writeClipboard,
  notify,
  registerShortcut,
  openExternal,
} from "hawk:desktop";

await setWindowTitle("Hawk2UI");
const files = await showOpenDialog({ title: "Open audio" });
globalThis.__hawk_desktop_files = files.join(",");
globalThis.__hawk_desktop_clipboard_before = await readClipboard();
await writeClipboard("rendered");
globalThis.__hawk_desktop_clipboard_after = await readClipboard();
await notify({ title: "Rendered", body: "Finished" });
globalThis.__hawk_desktop_shortcut = await registerShortcut("CmdOrCtrl+K", () => {});
await openExternal("https://example.test/docs");
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("declared desktop operations succeed");

    assert_eq!(
        runtime
            .evaluate_script_value("read-desktop-files.js", "globalThis.__hawk_desktop_files")
            .expect("dialog result is readable"),
        JsRuntimeValue::String("/project/input.wav".to_owned())
    );
    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-desktop-clipboard-before.js",
                "globalThis.__hawk_desktop_clipboard_before"
            )
            .expect("clipboard before is readable"),
        JsRuntimeValue::String("initial clipboard".to_owned())
    );
    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-desktop-clipboard-after.js",
                "globalThis.__hawk_desktop_clipboard_after"
            )
            .expect("clipboard after is readable"),
        JsRuntimeValue::String("rendered".to_owned())
    );
    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-desktop-shortcut.js",
                "globalThis.__hawk_desktop_shortcut"
            )
            .expect("shortcut handle is readable"),
        JsRuntimeValue::String("hawk-shortcut:CmdOrCtrl+K".to_owned())
    );
}

#[test]
fn capabilities_desktop_deep_link_callbacks_receive_declared_events() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .with_host_context(HawkHostContext::Desktop)
        .allow_desktop_operation("hawk:desktop.onDeepLink")
        .with_deep_link_event("hawk2ui", "hawk2ui://open?project=42");
    let graph = module_runtime(
        r#"
import { onDeepLink } from "hawk:desktop";

const event = await onDeepLink("hawk2ui", (deepLink) => {
  globalThis.__hawk_deep_link_callback = `${deepLink.scheme}:${deepLink.url}`;
  try {
    deepLink.url = "mutated";
    globalThis.__hawk_deep_link_frozen = "mutable";
  } catch (_error) {
    globalThis.__hawk_deep_link_frozen = "frozen";
  }
});
globalThis.__hawk_deep_link_return = event.url;
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("declared desktop deep-link event is delivered");

    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-deep-link-callback.js",
                "globalThis.__hawk_deep_link_callback",
            )
            .expect("deep-link callback result is readable"),
        JsRuntimeValue::String("hawk2ui:hawk2ui://open?project=42".to_owned())
    );
    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-deep-link-return.js",
                "globalThis.__hawk_deep_link_return",
            )
            .expect("deep-link return result is readable"),
        JsRuntimeValue::String("hawk2ui://open?project=42".to_owned())
    );
    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-deep-link-frozen.js",
                "globalThis.__hawk_deep_link_frozen",
            )
            .expect("deep-link frozen result is readable"),
        JsRuntimeValue::String("frozen".to_owned())
    );
}

#[test]
fn capabilities_desktop_window_commands_route_through_declared_host_operations() {
    let capabilities = HawkRuntimeCapabilities::for_test()
        .with_host_context(HawkHostContext::Desktop)
        .allow_desktop_operation("hawk:desktop.setWindowMode")
        .allow_desktop_operation("hawk:desktop.closeWindow");
    let graph = module_runtime(
        r#"
import { closeWindow, setWindowMode } from "hawk:desktop";

const mode = await setWindowMode("fullscreen");
const close = await closeWindow("render-complete");
globalThis.__hawk_window_commands = `${mode.mode}:${close.reason}`;
"#,
    );
    let mut runtime = HawkJsRuntime::from_module_graph_with_capabilities(graph, capabilities)
        .expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("declared desktop window commands succeed");

    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-window-commands.js",
                "globalThis.__hawk_window_commands",
            )
            .expect("window command result is readable"),
        JsRuntimeValue::String("fullscreen:render-complete".to_owned())
    );
}

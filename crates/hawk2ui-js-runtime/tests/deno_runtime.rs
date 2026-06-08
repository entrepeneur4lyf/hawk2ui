use hawk2ui_js_runtime::{
    HawkJsModule, HawkJsModuleGraph, HawkJsRuntime, JsRuntimeError, JsRuntimeValue,
    RuntimeSceneOpAdapter, SceneNodeKind, SceneOp, SceneOpBatch, crate_name,
};
use hawk2ui_layout::Viewport;
use hawk2ui_render::{Color, RendererBackend};
use hawk2ui_render_skia::{SkiaFrameSnapshot, SkiaRendererBackend};
use hawk2ui_runtime::{RuntimeDrawCommand, RuntimeSceneBridge, RuntimeSceneFrame};
use std::time::Duration;

#[test]
fn exposes_crate_identity() {
    assert_eq!(crate_name(), "hawk2ui-js-runtime");
}

#[test]
fn runtime_error_exposes_rule_message_and_display() {
    let error = JsRuntimeError::new("js-runtime.test", "runtime smoke failure");

    assert_eq!(error.rule(), "js-runtime.test");
    assert_eq!(error.message(), "runtime smoke failure");
    assert_eq!(error.to_string(), "js-runtime.test: runtime smoke failure");
}

#[test]
fn deno_runtime_executes_basic_javascript() {
    let mut runtime = HawkJsRuntime::new().expect("runtime initializes");

    runtime
        .execute_script("basic.js", "globalThis.__hawk_test = 1 + 2;")
        .expect("basic JavaScript executes");
}

#[test]
fn deno_runtime_reports_javascript_failures() {
    let mut runtime = HawkJsRuntime::new().expect("runtime initializes");

    let error = runtime
        .execute_script("throws.js", "throw new Error('boom');")
        .expect_err("throwing JavaScript should fail");

    assert_eq!(error.rule(), "js-runtime.execute-failed");
    assert!(
        error.message().contains("boom"),
        "error message should include thrown JavaScript detail: {}",
        error.message()
    );
}

#[test]
fn deno_runtime_reports_module_failures_with_inline_source_maps() {
    let source_map_base64 = "eyJ2ZXJzaW9uIjozLCJzb3VyY2VzIjpbImZpbGU6Ly8vdGVzdC50cyJdLCJzb3VyY2VzQ29udGVudCI6WyJmdW5jdGlvbiBncmVldChuYW1lOiBzdHJpbmcpIHtcbiAgdGhyb3cgbmV3IEVycm9yKFwiVGVzdCBlcnJvclwiKTtcbn1cblxuZ3JlZXQoXCJXb3JsZFwiKTtcbiJdLCJuYW1lcyI6W10sIm1hcHBpbmdzIjoiQUFBQTtBQUNBO0FBQ0E7QUFDQTtBQUNBIn0=";
    let graph = HawkJsModuleGraph::new("file:///app/dist/app.js").with_module(HawkJsModule::new(
        "file:///app/dist/app.js",
        format!(
            r#"function greet(name) {{
  throw new Error("Test error");
}}

greet("World");

//# sourceMappingURL=data:application/json;base64,{source_map_base64}
"#
        ),
    ));
    let mut runtime = HawkJsRuntime::from_module_graph(graph).expect("runtime initializes");

    let error = runtime
        .execute_entrypoint_module()
        .expect_err("throwing mapped module should fail");

    assert_eq!(error.rule(), "js-runtime.module.event-loop-failed");
    assert!(
        error.message().contains("Test error"),
        "error message should include thrown JavaScript detail: {}",
        error.message()
    );
    assert!(
        error.message().contains("file:///test.ts:2"),
        "error message should include original source-map location: {}",
        error.message()
    );
}

#[test]
fn deno_runtime_reports_module_failures_with_external_sealed_source_maps() {
    let source_map = r#"{
  "version": 3,
  "sources": ["file:///app/src/App.tsx"],
  "sourcesContent": ["export function App() {\n  throw new Error(\"External source map test\");\n}\nApp();\n"],
  "names": [],
  "mappings": "AAAA;AACA;AACA;AACA"
}"#;
    let graph = HawkJsModuleGraph::new("file:///app/dist/app.js")
        .with_module(HawkJsModule::new(
            "file:///app/dist/app.js",
            r#"export function App() {
  throw new Error("External source map test");
}
App();

//# sourceMappingURL=./app.js.map"#,
        ))
        .with_module(HawkJsModule::new("file:///app/dist/app.js.map", source_map));
    let mut runtime = HawkJsRuntime::from_module_graph(graph).expect("runtime initializes");

    let error = runtime
        .execute_entrypoint_module()
        .expect_err("throwing mapped module should fail");

    assert_eq!(error.rule(), "js-runtime.module.event-loop-failed");
    assert!(
        error.message().contains("External source map test"),
        "error message should include thrown JavaScript detail: {}",
        error.message()
    );
    assert!(
        error.message().contains("file:///app/src/App.tsx:2"),
        "error message should include external source-map location: {}",
        error.message()
    );
}

#[test]
fn privileged_host_apis_are_absent_from_bare_runtime() {
    let mut runtime = HawkJsRuntime::new().expect("runtime initializes");

    runtime
        .execute_script(
            "privileged-apis.js",
            r#"
const leaked = ["Deno", "hawk", "process", "require", "module", "Buffer"]
  .filter((name) => name in globalThis);
if (leaked.length > 0) throw new Error(`privileged api leaked: ${leaked.join(",")}`);
"#,
        )
        .expect("bare runtime does not expose privileged host APIs");
}

#[test]
fn unsupported_document_api_reports_structured_diagnostic() {
    let mut runtime = HawkJsRuntime::new().expect("runtime initializes");

    let error = runtime
        .execute_script(
            "unsupported-document.js",
            r#"document.createElement("div");"#,
        )
        .expect_err("unsupported document API should fail clearly");

    assert_eq!(error.rule(), "js-runtime.execute-failed");
    assert!(error.message().contains("js-runtime.web-api.unsupported"));
    assert!(error.message().contains("document.createElement"));
}

#[test]
fn unsupported_browser_cookie_api_reports_structured_diagnostic() {
    let mut runtime = HawkJsRuntime::new().expect("runtime initializes");

    let error = runtime
        .execute_script("unsupported-cookie.js", r"document.cookie;")
        .expect_err("unsupported cookie API should fail clearly");

    assert_eq!(error.rule(), "js-runtime.execute-failed");
    assert!(error.message().contains("js-runtime.web-api.unsupported"));
    assert!(error.message().contains("document.cookie"));
}

#[test]
fn unsupported_window_api_reports_structured_diagnostic() {
    let mut runtime = HawkJsRuntime::new().expect("runtime initializes");

    let error = runtime
        .execute_script("unsupported-window.js", r"window.localStorage;")
        .expect_err("unsupported window API should fail clearly");

    assert_eq!(error.rule(), "js-runtime.execute-failed");
    assert!(error.message().contains("js-runtime.web-api.unsupported"));
    assert!(error.message().contains("window.localStorage"));
}

#[test]
fn unsupported_top_level_browser_storage_reports_structured_diagnostic() {
    let mut runtime = HawkJsRuntime::new().expect("runtime initializes");

    let error = runtime
        .execute_script(
            "unsupported-local-storage.js",
            r#"localStorage.getItem("theme");"#,
        )
        .expect_err("top-level localStorage should fail clearly");

    assert_eq!(error.rule(), "js-runtime.execute-failed");
    assert!(error.message().contains("js-runtime.web-api.unsupported"));
    assert!(error.message().contains("localStorage.getItem"));
}

#[test]
fn unsupported_top_level_location_reports_structured_diagnostic() {
    let mut runtime = HawkJsRuntime::new().expect("runtime initializes");

    let error = runtime
        .execute_script("unsupported-location.js", r"location.href;")
        .expect_err("top-level location should fail clearly");

    assert_eq!(error.rule(), "js-runtime.execute-failed");
    assert!(error.message().contains("js-runtime.web-api.unsupported"));
    assert!(error.message().contains("location.href"));
}

#[test]
fn unsupported_top_level_navigator_reports_structured_diagnostic() {
    let mut runtime = HawkJsRuntime::new().expect("runtime initializes");

    let error = runtime
        .execute_script("unsupported-navigator.js", r"navigator.userAgent;")
        .expect_err("top-level navigator should fail clearly");

    assert_eq!(error.rule(), "js-runtime.execute-failed");
    assert!(error.message().contains("js-runtime.web-api.unsupported"));
    assert!(error.message().contains("navigator.userAgent"));
}

#[test]
fn unsupported_ambient_network_apis_report_structured_diagnostics() {
    for (name, source) in [
        (
            "WebSocket",
            r#"new WebSocket("wss://api.example.test/socket");"#,
        ),
        ("XMLHttpRequest", r"new XMLHttpRequest();"),
        (
            "EventSource",
            r#"new EventSource("https://api.example.test/events");"#,
        ),
    ] {
        let mut runtime = HawkJsRuntime::new().expect("runtime initializes");

        let error = runtime
            .execute_script(format!("unsupported-{name}.js"), source)
            .expect_err("ambient network APIs should fail clearly");

        assert_eq!(error.rule(), "js-runtime.execute-failed");
        assert!(error.message().contains("js-runtime.web-api.unsupported"));
        assert!(error.message().contains(name));
    }
}

#[test]
fn deno_runtime_exposes_url_and_url_search_params() {
    let mut runtime = HawkJsRuntime::new().expect("runtime initializes");

    assert_eq!(
        runtime
            .evaluate_script_value(
                "url-search-params.js",
                r#"
const url = new URL("/search?q=hawk&debug=1", "https://example.test/docs/index.html");
url.searchParams.set("page", "2");
url.searchParams.delete("debug");
url.searchParams.append("q", "runtime");
[
  url.href,
  url.origin,
  url.pathname,
  url.searchParams.get("q"),
  url.searchParams.getAll("q").join("|"),
  url.searchParams.toString()
].join("\n")
"#,
            )
            .expect("URL and URLSearchParams are available"),
        JsRuntimeValue::String(
            "https://example.test/search?q=hawk&page=2&q=runtime\nhttps://example.test\n/search\nhawk\nhawk|runtime\nq=hawk&page=2&q=runtime"
                .to_owned()
        )
    );
}

#[test]
fn deno_runtime_exposes_text_encoder_decoder_utf8() {
    let mut runtime = HawkJsRuntime::new().expect("runtime initializes");

    assert_eq!(
        runtime
            .evaluate_script_value(
                "text-encoding.js",
                r#"
const bytes = new TextEncoder().encode("Hawk\u2713");
const decoded = new TextDecoder("utf-8").decode(bytes);
`${Array.from(bytes).join(",")}|${decoded}`
"#,
            )
            .expect("TextEncoder and TextDecoder are available"),
        JsRuntimeValue::String("72,97,119,107,226,156,147|Hawk\u{2713}".to_owned())
    );
}

#[test]
fn deno_runtime_supports_zero_delay_timeout_and_clear_timeout_in_modules() {
    let graph = HawkJsModuleGraph::new("file:///app/main.js").with_module(HawkJsModule::new(
        "file:///app/main.js",
        r#"
const fired = [];
const cancelled = setTimeout(() => fired.push("cancelled"), 0);
clearTimeout(cancelled);
globalThis.__hawk_timer_result = await new Promise((resolve) => {
  setTimeout((label) => {
    fired.push(label);
    resolve(fired.join(","));
  }, 0, "fired");
});
"#,
    ));
    let mut runtime = HawkJsRuntime::from_module_graph(graph).expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("zero-delay timer module executes");

    assert_eq!(
        runtime
            .evaluate_script_value("read-timer-result.js", "globalThis.__hawk_timer_result")
            .expect("timer result is readable"),
        JsRuntimeValue::String("fired".to_owned())
    );
}

#[test]
fn deno_runtime_keeps_positive_timeout_after_microtasks() {
    let graph = HawkJsModuleGraph::new("file:///app/main.js").with_module(HawkJsModule::new(
        "file:///app/main.js",
        r#"
globalThis.__hawk_timer_result = await new Promise((resolve) => {
  const fired = [];
  setTimeout(() => fired.push("delayed"), 5);
  Promise.resolve().then(() => fired.push("microtask"));
  setTimeout(() => resolve(fired.join(",")), 10);
});
"#,
    ));
    let mut runtime = HawkJsRuntime::from_module_graph(graph).expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("positive-delay timer module executes");

    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-positive-timer-result.js",
                "globalThis.__hawk_timer_result"
            )
            .expect("timer result is readable"),
        JsRuntimeValue::String("microtask,delayed".to_owned())
    );
}

#[test]
fn deno_runtime_supports_interval_and_clear_interval_in_modules() {
    let graph = HawkJsModuleGraph::new("file:///app/main.js").with_module(HawkJsModule::new(
        "file:///app/main.js",
        r#"
globalThis.__hawk_interval_result = await new Promise((resolve) => {
  const fired = [];
  const interval = setInterval((label) => {
    fired.push(`${label}:${fired.length + 1}`);
    if (fired.length === 3) {
      clearInterval(interval);
      resolve(fired.join(","));
    }
  }, 1, "tick");
});
"#,
    ));
    let mut runtime = HawkJsRuntime::from_module_graph(graph).expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("interval timer module executes");

    assert_eq!(
        runtime
            .evaluate_script_value(
                "read-interval-result.js",
                "globalThis.__hawk_interval_result",
            )
            .expect("interval result is readable"),
        JsRuntimeValue::String("tick:1,tick:2,tick:3".to_owned())
    );
}

#[test]
fn deno_runtime_exposes_crypto_get_random_values_and_uuid() {
    let mut runtime = HawkJsRuntime::new().expect("runtime initializes");

    assert_eq!(
        runtime
            .evaluate_script_value(
                "crypto-random.js",
                r#"
const bytes = new Uint8Array(16);
const returned = crypto.getRandomValues(bytes);
const uuid = crypto.randomUUID();
[
  returned === bytes,
  bytes.length,
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(uuid)
].join(":")
"#,
            )
            .expect("crypto random APIs are available"),
        JsRuntimeValue::String("true:16:true".to_owned())
    );
}

#[test]
fn deno_runtime_exposes_crypto_subtle_sha256_digest() {
    let graph = HawkJsModuleGraph::new("file:///app/main.js").with_module(HawkJsModule::new(
        "file:///app/main.js",
        r#"
const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode("hawk"));
globalThis.__hawk_sha256 = Array.from(new Uint8Array(digest))
  .map((byte) => byte.toString(16).padStart(2, "0"))
  .join("");
"#,
    ));
    let mut runtime = HawkJsRuntime::from_module_graph(graph).expect("runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("crypto digest module executes");

    assert_eq!(
        runtime
            .evaluate_script_value("read-sha256.js", "globalThis.__hawk_sha256")
            .expect("digest is readable"),
        JsRuntimeValue::String(
            "0139bc5debaaa4b84e9341efb6ffa3e470f45a084742310e8f0b63ea83380168".to_owned()
        )
    );
}

#[test]
fn deno_runtime_evaluates_primitive_script_results() {
    let mut runtime = HawkJsRuntime::new().expect("runtime initializes");

    assert_eq!(
        runtime
            .evaluate_script_value("number.js", "1 + 2")
            .expect("number evaluates"),
        JsRuntimeValue::Number(3.0)
    );
    assert_eq!(
        runtime
            .evaluate_script_value("string.js", "'Hawk' + '2UI'")
            .expect("string evaluates"),
        JsRuntimeValue::String("Hawk2UI".to_owned())
    );
    assert_eq!(
        runtime
            .evaluate_script_value("boolean.js", "1 < 2")
            .expect("boolean evaluates"),
        JsRuntimeValue::Bool(true)
    );
    assert_eq!(
        runtime
            .evaluate_script_value("null.js", "undefined")
            .expect("undefined evaluates to null structured value"),
        JsRuntimeValue::Null
    );
}

#[test]
fn deno_runtime_rejects_unsupported_structured_results() {
    let mut runtime = HawkJsRuntime::new().expect("runtime initializes");

    let error = runtime
        .evaluate_script_value("object.js", "({ id: 'root' })")
        .expect_err("objects are not primitive structured results");

    assert_eq!(error.rule(), "js-runtime.value.unsupported");
}

#[test]
fn deno_runtime_terminates_runaway_script_evaluation() {
    let mut runtime = HawkJsRuntime::new().expect("runtime initializes");

    let error = runtime
        .evaluate_script_value_with_timeout(
            "runaway.js",
            "while (true) {}",
            Duration::from_millis(10),
        )
        .expect_err("runaway script should terminate");

    assert_eq!(error.rule(), "js-runtime.execute-failed");
    assert!(
        error.message().contains("execution terminated"),
        "{}",
        error.message()
    );
}

#[test]
fn deno_runtime_executes_sealed_module_graph_entrypoint() {
    let graph = HawkJsModuleGraph::new("file:///app/main.js")
        .with_module(HawkJsModule::new(
            "file:///app/main.js",
            r#"
import { message } from "./message.js";
globalThis.__hawk_module_result = message;
"#,
        ))
        .with_module(HawkJsModule::new(
            "file:///app/message.js",
            r#"export const message = "sealed module";"#,
        ));
    let mut runtime = HawkJsRuntime::from_module_graph(graph).expect("module runtime initializes");

    runtime
        .execute_entrypoint_module()
        .expect("entrypoint module executes");

    assert_eq!(
        runtime
            .evaluate_script_value("read-module-result.js", "globalThis.__hawk_module_result")
            .expect("module result can be read"),
        JsRuntimeValue::String("sealed module".to_owned())
    );
}

#[test]
fn unsupported_hawk_module_import_reports_structured_diagnostic() {
    let graph = HawkJsModuleGraph::new("file:///app/main.js").with_module(HawkJsModule::new(
        "file:///app/main.js",
        r#"import { readText } from "hawk:filesystem";"#,
    ));
    let mut runtime = HawkJsRuntime::from_module_graph(graph).expect("module runtime initializes");

    let error = runtime
        .execute_entrypoint_module()
        .expect_err("unsupported hawk module import should fail clearly");

    assert_eq!(error.rule(), "js-runtime.module.load-failed");
    assert!(
        error
            .message()
            .contains("js-runtime.module.unsupported-hawk-import"),
        "{}",
        error.message()
    );
    assert!(
        error.message().contains("hawk:filesystem"),
        "{}",
        error.message()
    );
}

#[test]
fn unsealed_dynamic_import_reports_structured_not_sealed_diagnostic() {
    let graph = HawkJsModuleGraph::new("file:///app/main.js").with_module(HawkJsModule::new(
        "file:///app/main.js",
        r#"await import("./missing.js");"#,
    ));
    let mut runtime = HawkJsRuntime::from_module_graph(graph).expect("module runtime initializes");

    let error = runtime
        .execute_entrypoint_module()
        .expect_err("dynamic imports outside the sealed graph must fail clearly");

    assert_eq!(error.rule(), "js-runtime.module.event-loop-failed");
    assert!(
        error.message().contains("js-runtime.module.not-sealed"),
        "{}",
        error.message()
    );
    assert!(
        error.message().contains("file:///app/missing.js"),
        "{}",
        error.message()
    );
}

#[test]
fn javascript_commits_scene_batches_to_rust() {
    let mut runtime = HawkJsRuntime::new().expect("runtime initializes");

    runtime
        .execute_script(
            "commit-scene.js",
            r#"
globalThis.__hawk2uiCommitScene({
  ops: [
    { type: "create-node", id: "root", kind: "view" },
    { type: "create-text", id: "label", text: "Hello React" },
    { type: "append-child", parent: "root", child: "label" },
    { type: "commit" }
  ]
});
"#,
        )
        .expect("scene batch commits");

    assert_eq!(
        runtime.scene_batches(),
        vec![SceneOpBatch::new([
            SceneOp::CreateNode {
                id: "root".to_owned(),
                kind: SceneNodeKind::View,
            },
            SceneOp::CreateText {
                id: "label".to_owned(),
                text: "Hello React".to_owned(),
            },
            SceneOp::AppendChild {
                parent: "root".to_owned(),
                child: "label".to_owned(),
            },
            SceneOp::Commit,
        ])]
    );
}

#[test]
fn javascript_scene_commit_rejects_invalid_batches() {
    let mut runtime = HawkJsRuntime::new().expect("runtime initializes");

    let error = runtime
        .execute_script(
            "invalid-scene.js",
            r#"
globalThis.__hawk2uiCommitScene({
  ops: [
    { type: "create-node", id: "", kind: "view" }
  ]
});
"#,
        )
        .expect_err("invalid scene batch should fail");

    assert_eq!(error.rule(), "js-runtime.execute-failed");
    assert!(
        error.message().contains("js-runtime.scene-op.invalid"),
        "scene validation error should retain rule: {}",
        error.message()
    );
    assert!(
        runtime.scene_batches().is_empty(),
        "invalid batches must not be recorded"
    );
}

#[test]
fn deno_runtime_executes_react_counter_bundle_and_second_update() {
    let mut runtime = HawkJsRuntime::new().expect("runtime initializes");

    runtime
        .execute_script(
            "react-counter-bundle.js",
            include_str!("fixtures/react_counter_bundle.js"),
        )
        .expect("react counter fixture executes");

    assert_eq!(
        runtime.scene_batches(),
        vec![SceneOpBatch::new([
            SceneOp::CreateNode {
                id: "root".to_owned(),
                kind: SceneNodeKind::View,
            },
            SceneOp::CreateNode {
                id: "count".to_owned(),
                kind: SceneNodeKind::Text,
            },
            SceneOp::SetProp {
                id: "count".to_owned(),
                name: "text".to_owned(),
                value: hawk2ui_js_runtime::SceneValue::String("0".to_owned()),
            },
            SceneOp::AppendChild {
                parent: "root".to_owned(),
                child: "count".to_owned(),
            },
            SceneOp::CreateNode {
                id: "increment".to_owned(),
                kind: SceneNodeKind::Button,
            },
            SceneOp::SetProp {
                id: "increment".to_owned(),
                name: "text".to_owned(),
                value: hawk2ui_js_runtime::SceneValue::String("Increment".to_owned()),
            },
            SceneOp::RegisterEvent {
                id: "increment".to_owned(),
                event: "pointer.press".to_owned(),
                handler: "increment".to_owned(),
            },
            SceneOp::AppendChild {
                parent: "root".to_owned(),
                child: "increment".to_owned(),
            },
            SceneOp::Commit,
        ])]
    );

    runtime
        .execute_script(
            "dispatch-counter-event.js",
            r#"globalThis.__hawk2uiDispatchEvent("increment", "pointer.press", {});"#,
        )
        .expect("synthetic event dispatch executes");

    assert_eq!(
        runtime.scene_batches()[1],
        SceneOpBatch::new([
            SceneOp::SetProp {
                id: "count".to_owned(),
                name: "text".to_owned(),
                value: hawk2ui_js_runtime::SceneValue::String("1".to_owned()),
            },
            SceneOp::Commit,
        ])
    );
}

#[test]
fn react_deno_second_frame_renders_changed_pixels() {
    let mut runtime = HawkJsRuntime::new().expect("runtime initializes");
    let mut adapter = RuntimeSceneOpAdapter::default();

    runtime
        .execute_script(
            "react-counter-bundle.js",
            include_str!("fixtures/react_counter_bundle.js"),
        )
        .expect("react counter fixture executes");
    adapter
        .apply_batch(
            runtime
                .scene_batches()
                .first()
                .expect("initial scene batch exists"),
        )
        .expect("initial scene batch applies to runtime tree");

    let scene_bridge = RuntimeSceneBridge::new(Viewport::new(320.0, 200.0));
    let initial_frame = scene_bridge
        .build(adapter.runtime_tree().expect("initial runtime tree exists"))
        .expect("initial runtime scene frame builds");
    assert_eq!(text_draw_for(&initial_frame, "count"), Some("0"));
    let initial_snapshot = render_runtime_frame(&initial_frame);

    runtime
        .execute_script(
            "dispatch-counter-event.js",
            r#"globalThis.__hawk2uiDispatchEvent("increment", "pointer.press", {});"#,
        )
        .expect("synthetic event dispatch executes");
    adapter
        .apply_batch(
            runtime
                .scene_batches()
                .get(1)
                .expect("second scene batch exists"),
        )
        .expect("second scene batch applies to runtime tree");

    let second_frame = scene_bridge
        .build(adapter.runtime_tree().expect("updated runtime tree exists"))
        .expect("updated runtime scene frame builds");
    assert_eq!(text_draw_for(&second_frame, "count"), Some("1"));
    assert!(
        second_frame
            .invalidated_view_ids()
            .iter()
            .any(|id| id.as_str() == "count"),
        "second frame should mark the React-updated text node invalidated"
    );
    let second_snapshot = render_runtime_frame(&second_frame);

    assert_ne!(
        initial_snapshot.pixels(),
        second_snapshot.pixels(),
        "React-originated state update should change rendered frame pixels"
    );
    assert!(
        second_snapshot.pixels().iter().any(|pixel| *pixel != 0),
        "second frame should contain visible non-background pixels"
    );
}

fn text_draw_for<'a>(frame: &'a RuntimeSceneFrame, node_id: &str) -> Option<&'a str> {
    frame
        .draw_commands()
        .iter()
        .find_map(|command| match command {
            RuntimeDrawCommand::Text { id, text, .. } if id.as_str() == node_id => {
                Some(text.as_str())
            }
            _ => None,
        })
}

fn render_runtime_frame(frame: &RuntimeSceneFrame) -> SkiaFrameSnapshot {
    let mut backend = SkiaRendererBackend::default();
    backend
        .create_surface("main", 320, 200)
        .expect("surface creates");
    backend.begin_frame("main").expect("frame begins");
    backend
        .clear(Color::rgba(0, 0, 0, 255))
        .expect("surface clears");
    backend
        .draw_runtime_scene_frame(frame, 0, 1.0)
        .expect("runtime scene frame renders");
    backend.end_frame("main").expect("frame ends");
    backend
        .frame_snapshot("main")
        .expect("snapshot exists")
        .clone()
}

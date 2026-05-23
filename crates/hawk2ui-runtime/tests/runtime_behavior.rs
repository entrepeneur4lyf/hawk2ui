use hawk2ui_runtime::{
    BindingExecution, BindingLifecycleAvailability, BindingSchema, HostBindingRecord,
    HostBindingRegistry, HostCallRecord, LifecycleHook, LifecyclePhase, LifecycleRegistry,
    RuntimeCapability, RuntimeError, RuntimeEvent, RuntimeEventDispatcher, RuntimeEventKind,
    RuntimeEventPayload, RuntimeEventPropagation, ScriptModuleKind, ScriptModuleRecord,
    StructuredValue,
};
use serde::{Serialize, de::DeserializeOwned};

fn assert_serde_contract<T: Serialize + DeserializeOwned>() {}

#[test]
fn runtime_records_module_identity_is_stable() {
    let module = ScriptModuleRecord::new("main", "app://main.js", ScriptModuleKind::JavaScript)
        .with_hash("sha256:abc")
        .requires(RuntimeCapability::RenderInvalidation)
        .exports("mount");

    assert_eq!(module.identity(), "main@app://main.js");
    assert_eq!(module.hash.as_deref(), Some("sha256:abc"));
    assert_eq!(module.exports, vec!["mount"]);
    assert_eq!(
        module.required_capabilities,
        vec![RuntimeCapability::RenderInvalidation]
    );
}

#[test]
fn runtime_records_report_host_call_errors() {
    let call = HostCallRecord::new(
        "main",
        "platform.clipboard.write",
        StructuredValue::object([("text", StructuredValue::string("hello"))]),
    )
    .requires(RuntimeCapability::ClipboardWrite);

    let error = RuntimeError::host_call_denied(
        call.binding_name.clone(),
        RuntimeCapability::ClipboardWrite,
        "clipboard.write is not declared",
    );

    assert_eq!(call.module_id, "main");
    assert_eq!(
        call.required_capability,
        Some(RuntimeCapability::ClipboardWrite)
    );
    assert_eq!(error.code, "runtime.host-call-denied");
    assert_eq!(error.capability, Some(RuntimeCapability::ClipboardWrite));
    assert!(error.message.contains("clipboard.write"));
}

#[test]
fn runtime_records_register_lifecycle_hooks_in_order() {
    let registry = LifecycleRegistry::new([
        LifecycleHook::new("main", LifecyclePhase::Mount, "mount"),
        LifecycleHook::new("main", LifecyclePhase::Update, "update"),
        LifecycleHook::new("main", LifecyclePhase::Teardown, "dispose"),
    ]);

    let hook_names: Vec<_> = registry
        .hooks_for(LifecyclePhase::Mount)
        .iter()
        .map(|hook| hook.export_name.as_str())
        .collect();

    assert_eq!(hook_names, vec!["mount"]);
    assert_eq!(
        registry.hooks_for(LifecyclePhase::Teardown)[0].module_id,
        "main"
    );
    assert_eq!(registry.all().len(), 3);
}

#[test]
fn runtime_records_are_serializable_contracts() {
    assert_serde_contract::<ScriptModuleRecord>();
    assert_serde_contract::<HostCallRecord>();
    assert_serde_contract::<StructuredValue>();
    assert_serde_contract::<RuntimeError>();
    assert_serde_contract::<LifecycleHook>();
    assert_serde_contract::<LifecycleRegistry>();
}

#[test]
fn host_bindings_allow_capability_scoped_calls() {
    let registry = HostBindingRegistry::new([HostBindingRecord::new(
        "platform.clipboard.write",
        BindingSchema::new("object", "null", "ClipboardError"),
    )
    .requires(RuntimeCapability::ClipboardWrite)
    .execution(BindingExecution::Synchronous)
    .available_during(BindingLifecycleAvailability::AfterMount)]);

    let call = registry
        .call(
            "platform.clipboard.write",
            StructuredValue::object([("text", StructuredValue::string("hello"))]),
            [RuntimeCapability::ClipboardWrite],
            LifecyclePhase::Update,
        )
        .expect("declared capability should allow the host call");

    assert_eq!(call.binding_name, "platform.clipboard.write");
    assert_eq!(
        call.required_capability,
        Some(RuntimeCapability::ClipboardWrite)
    );
    assert_eq!(call.output_schema, "null");
}

#[test]
fn host_bindings_deny_missing_capability() {
    let registry = HostBindingRegistry::new([HostBindingRecord::new(
        "platform.network.fetch",
        BindingSchema::new("object", "object", "NetworkError"),
    )
    .requires(RuntimeCapability::NetworkRequest)]);

    let error = registry
        .call(
            "platform.network.fetch",
            StructuredValue::object([("url", StructuredValue::string("https://example.test"))]),
            [],
            LifecyclePhase::Update,
        )
        .expect_err("missing capability should deny the host call");

    assert_eq!(error.code, "binding.capability-denied");
    assert_eq!(error.capability, Some(RuntimeCapability::NetworkRequest));
}

#[test]
fn host_bindings_reject_schema_mismatch() {
    let registry = HostBindingRegistry::new([HostBindingRecord::new(
        "platform.clipboard.write",
        BindingSchema::new("object", "null", "ClipboardError"),
    )]);

    let error = registry
        .call(
            "platform.clipboard.write",
            StructuredValue::string("not an object"),
            [],
            LifecyclePhase::Update,
        )
        .expect_err("string payload should not satisfy object schema");

    assert_eq!(error.code, "binding.schema-mismatch");
    assert!(error.message.contains("expected object"));
}

#[test]
fn host_bindings_reject_unavailable_lifecycle_phase() {
    let registry = HostBindingRegistry::new([HostBindingRecord::new(
        "host.window.close",
        BindingSchema::new("null", "null", "WindowError"),
    )
    .available_during(BindingLifecycleAvailability::MountedOnly)]);

    let error = registry
        .call(
            "host.window.close",
            StructuredValue::Null,
            [],
            LifecyclePhase::Initialize,
        )
        .expect_err("mounted-only binding should not run during initialize");

    assert_eq!(error.code, "binding.lifecycle-unavailable");
}

#[test]
fn event_dispatch_preserves_enqueue_order_across_event_kinds() {
    let mut dispatcher = RuntimeEventDispatcher::default();
    dispatcher.listen("root", RuntimeEventKind::Ui);
    dispatcher.listen("meter", RuntimeEventKind::PluginParameter);
    dispatcher.listen("host", RuntimeEventKind::HostCallback);

    dispatcher.enqueue(RuntimeEvent::ui("root", "click"));
    dispatcher.enqueue(RuntimeEvent::plugin_parameter("meter", "gain", 0.75));
    dispatcher.enqueue(RuntimeEvent::host_callback("host", "window.close"));

    let deliveries = dispatcher
        .dispatch_pending()
        .expect("dispatch should succeed");
    let names: Vec<_> = deliveries
        .iter()
        .map(|delivery| delivery.event.name.as_str())
        .collect();

    assert_eq!(names, vec!["click", "gain", "window.close"]);
}

#[test]
fn event_dispatch_bubbles_from_target_to_ancestors() {
    let mut dispatcher = RuntimeEventDispatcher::default();
    dispatcher.listen("button", RuntimeEventKind::Custom);
    dispatcher.listen("panel", RuntimeEventKind::Custom);
    dispatcher.listen("root", RuntimeEventKind::Custom);

    dispatcher.enqueue(
        RuntimeEvent::custom("button", "armed", RuntimeEventPayload::Null)
            .with_bubble_path(["panel", "root"])
            .propagation(RuntimeEventPropagation::Bubble),
    );

    let deliveries = dispatcher
        .dispatch_pending()
        .expect("dispatch should succeed");
    let targets: Vec<_> = deliveries
        .iter()
        .map(|delivery| delivery.listener_target.as_str())
        .collect();

    assert_eq!(targets, vec!["button", "panel", "root"]);
}

#[test]
fn event_dispatch_cancels_pending_events_after_teardown() {
    let mut dispatcher = RuntimeEventDispatcher::default();
    dispatcher.listen("root", RuntimeEventKind::Ui);
    dispatcher.enqueue(RuntimeEvent::ui("root", "click"));

    dispatcher.begin_teardown();
    let error = dispatcher
        .dispatch_pending()
        .expect_err("teardown should cancel pending dispatch");

    assert_eq!(error.code, "event.teardown-cancelled");
    assert!(dispatcher.is_empty());
}

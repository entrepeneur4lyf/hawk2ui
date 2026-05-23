use hawk2ui_runtime::{
    HostCallRecord, LifecycleHook, LifecyclePhase, LifecycleRegistry, RuntimeCapability,
    RuntimeError, ScriptModuleKind, ScriptModuleRecord, StructuredValue,
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

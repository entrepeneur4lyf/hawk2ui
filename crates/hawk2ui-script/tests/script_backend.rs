use hawk2ui_script::{
    HostCallPolicy, ScriptBackend, ScriptModule, ScriptModuleKind, StructuredValue, TimerPolicy,
};

#[test]
fn script_backend_executes_javascript_and_typescript_modules() {
    let mut backend = ScriptBackend::new(HostCallPolicy::deny_all(), TimerPolicy::deterministic());

    let js = backend
        .execute_module(ScriptModule::javascript("math.js", "1 + 2"))
        .expect("javascript executes");
    let ts = backend
        .execute_module(ScriptModule::typescript(
            "math.ts",
            "const value: number = 4; value + 3",
        ))
        .expect("typescript executes after compilation");

    assert_eq!(js.value(), &StructuredValue::Number(3.0));
    assert_eq!(ts.value(), &StructuredValue::Number(7.0));
    assert_eq!(
        backend.executed_modules()[0].kind(),
        ScriptModuleKind::JavaScript
    );
}

#[test]
fn script_backend_handles_promises_timers_and_structured_host_calls() {
    let mut backend = ScriptBackend::new(
        HostCallPolicy::allow(["ui.setTitle"]),
        TimerPolicy::deterministic(),
    );

    let promise = backend.create_promise("load-data");
    backend
        .resolve_promise(promise, StructuredValue::String("ready".to_string()))
        .unwrap();
    let timer = backend.schedule_timer("animation", 16).unwrap();
    let host = backend
        .call_host("ui.setTitle", StructuredValue::String("Hello".to_string()))
        .expect("allowed host call succeeds");

    assert_eq!(
        backend.promise_state(promise).unwrap().value(),
        Some(&StructuredValue::String("ready".to_string()))
    );
    assert_eq!(backend.timers()[0].id(), timer);
    assert_eq!(host.binding(), "ui.setTitle");
}

#[test]
fn script_backend_denies_host_calls_interrupts_and_tears_down_safely() {
    let mut backend = ScriptBackend::new(HostCallPolicy::deny_all(), TimerPolicy::deterministic());

    let denial = backend
        .call_host(
            "fs.read",
            StructuredValue::String("/etc/passwd".to_string()),
        )
        .expect_err("denied call fails");
    assert_eq!(denial.diagnostic().rule(), "script.host-call.denied");

    backend.interrupt("user cancelled");
    let interrupted = backend
        .execute_module(ScriptModule::javascript("after-interrupt.js", "1 + 1"))
        .expect_err("interrupted runtime rejects execution");
    assert_eq!(interrupted.diagnostic().rule(), "script.interrupted");

    backend.teardown();
    assert!(backend.torn_down());
    assert!(backend.timers().is_empty());
}

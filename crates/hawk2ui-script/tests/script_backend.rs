use hawk2ui_script::{
    HostCallPolicy, ScriptBackend, ScriptExecutionLimits, ScriptModule, ScriptModuleKind,
    StructuredValue, TimerPolicy,
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
fn script_backend_executes_real_javascript_language_features() {
    let mut backend = ScriptBackend::new(HostCallPolicy::deny_all(), TimerPolicy::deterministic());

    let execution = backend
        .execute_module(ScriptModule::javascript(
            "language.js",
            r#"
const values = [1, 2, 3];
values.map((value) => value * 2).reduce((total, value) => total + value, 0);
"#,
        ))
        .expect("boa executes standard JavaScript features");

    assert_eq!(execution.value(), &StructuredValue::Number(12.0));
}

#[test]
fn script_backend_compiles_typescript_with_interfaces_generics_and_assertions() {
    let mut backend = ScriptBackend::new(HostCallPolicy::deny_all(), TimerPolicy::deterministic());

    let execution = backend
        .execute_module(ScriptModule::typescript(
            "typed.ts",
            r#"
type Scalar = number;
interface Accumulator {
  current: Scalar;
}
function sum<T extends number>(items: T[]): Scalar {
  return items.reduce((total: number, item: T) => total + item, 0);
}
const state = { current: sum([1, 2, 3]) } as Accumulator;
state.current + 3;
"#,
        ))
        .expect("typescript source compiles through the production compiler");

    assert_eq!(execution.value(), &StructuredValue::Number(9.0));
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

#[test]
fn script_backend_rejects_modules_that_exceed_execution_limits() {
    let mut source_limited =
        ScriptBackend::new(HostCallPolicy::deny_all(), TimerPolicy::deterministic())
            .with_execution_limits(ScriptExecutionLimits::new(24, 64));

    let oversized_source = source_limited
        .execute_module(ScriptModule::javascript(
            "oversized.js",
            "const value = 'this source is too large'; value",
        ))
        .expect_err("oversized source is rejected before evaluation");
    assert_eq!(
        oversized_source.diagnostic().rule(),
        "script.source.too-large"
    );

    let mut compiled_limited =
        ScriptBackend::new(HostCallPolicy::deny_all(), TimerPolicy::deterministic())
            .with_execution_limits(ScriptExecutionLimits::new(1024, 24));
    let oversized_compiled = compiled_limited
        .execute_module(ScriptModule::typescript(
            "compiled.ts",
            "const value: string = 'compiled output is too large'; value",
        ))
        .expect_err("oversized compiled JavaScript is rejected before evaluation");
    assert_eq!(
        oversized_compiled.diagnostic().rule(),
        "script.compiled-source.too-large"
    );
    assert!(source_limited.executed_modules().is_empty());
    assert!(compiled_limited.executed_modules().is_empty());
}

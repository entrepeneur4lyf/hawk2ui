use hawk2ui_api::{Diagnostic, DiagnosticSeverity};
use hawk2ui_script::{
    HostCallPolicy, ScriptBackend, ScriptBackendError, ScriptExecutionLimits, ScriptModule,
    ScriptModuleKind, StructuredValue, TimerPolicy,
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
            r"
const values = [1, 2, 3];
values.map((value) => value * 2).reduce((total, value) => total + value, 0);
",
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
            r"
type Scalar = number;
interface Accumulator {
  current: Scalar;
}
function sum<T extends number>(items: T[]): Scalar {
  return items.reduce((total: number, item: T) => total + item, 0);
}
const state = { current: sum([1, 2, 3]) } as Accumulator;
state.current + 3;
",
        ))
        .expect("typescript source compiles through the production compiler");

    assert_eq!(execution.value(), &StructuredValue::Number(9.0));
}

#[test]
fn script_backend_compiles_module_source_without_evaluating() {
    let compiled = ScriptBackend::compile_module_source(
        &ScriptModule::typescript("build-entry.ts", "export const title: string = 'Hawk';"),
        ScriptExecutionLimits::default(),
    )
    .expect("typescript compiles for build artifact generation");

    assert!(compiled.contains("const title"));
    assert!(compiled.contains("Hawk"));
    assert!(!compiled.contains(": string"));
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
fn script_backend_projects_host_promises_and_timers_into_boa_jobs() {
    let mut backend = ScriptBackend::new(HostCallPolicy::deny_all(), TimerPolicy::deterministic());
    let promise = backend.create_promise("load-data");
    backend
        .resolve_promise(promise, StructuredValue::String("ready".to_string()))
        .expect("promise resolves");
    backend
        .schedule_timer("animation", 16)
        .expect("timer schedules");

    let execution = backend
        .execute_module_with_host_jobs(ScriptModule::javascript(
            "host-jobs.js",
            r#"
globalThis.__hawk2uiResult = "pending";
hawk2ui.promise("load-data").then((value) => {
  globalThis.__hawk2uiResult = value;
});
hawk2ui.onTimer("animation", () => {
  globalThis.__hawk2uiResult = globalThis.__hawk2uiResult + ":timer";
});
"#,
        ))
        .expect("host jobs execute through Boa");

    assert_eq!(
        execution.value(),
        &StructuredValue::String("ready:timer".to_string())
    );
}

#[test]
fn script_backend_teardown_cancels_host_promises_and_timer_jobs() {
    let mut backend = ScriptBackend::new(HostCallPolicy::deny_all(), TimerPolicy::deterministic());
    let promise = backend.create_promise("load-data");
    backend
        .schedule_timer("animation", 16)
        .expect("timer schedules");

    backend.teardown();

    assert!(backend.promise_state(promise).is_none());
    assert!(backend.timers().is_empty());
    let error = backend
        .execute_module_with_host_jobs(ScriptModule::javascript("after-teardown.js", "1 + 1"))
        .expect_err("torn down backend rejects host job execution");
    assert_eq!(error.diagnostic().rule(), "script.torn-down");
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
fn script_backend_error_converts_to_shared_diagnostic() {
    let error = ScriptBackendError::new("script.denied", "script call denied");
    let diagnostic = Diagnostic::from(error);

    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(diagnostic.rule.as_str(), "script.denied");
    assert_eq!(diagnostic.message, "script call denied");
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

#[test]
fn script_backend_bounds_runaway_loops() {
    let limits = ScriptExecutionLimits::new(1_048_576, 4_194_304).with_max_loop_iterations(1_000);
    let mut backend = ScriptBackend::new(HostCallPolicy::deny_all(), TimerPolicy::deterministic())
        .with_execution_limits(limits);

    let error = backend
        .execute_module(ScriptModule::javascript("runaway.js", "while (true) {}"))
        .expect_err("an unbounded loop terminates with an error instead of hanging the host");

    assert_eq!(error.diagnostic().rule(), "script.eval.failed");
    assert!(backend.executed_modules().is_empty());
}

#[test]
fn script_backend_rejects_deeply_nested_javascript_before_parsing() {
    let mut backend = ScriptBackend::new(HostCallPolicy::deny_all(), TimerPolicy::deterministic());
    let open = "(".repeat(300);
    let close = ")".repeat(300);
    let nested = format!("{open}1{close}");

    let error = backend
        .execute_module(ScriptModule::javascript("nested.js", nested))
        .expect_err("deeply nested source is rejected before it reaches the parser");

    assert_eq!(error.diagnostic().rule(), "script.source.too-deeply-nested");
    assert!(backend.executed_modules().is_empty());
}

#[test]
fn script_backend_rejects_deeply_nested_typescript_before_parsing() {
    let mut backend = ScriptBackend::new(HostCallPolicy::deny_all(), TimerPolicy::deterministic());
    let open = "(".repeat(300);
    let close = ")".repeat(300);
    let nested = format!("const value: number = {open}1{close};");

    let error = backend
        .execute_module(ScriptModule::typescript("nested.ts", nested))
        .expect_err("deeply nested TypeScript is rejected before the oxc parser runs");

    assert_eq!(error.diagnostic().rule(), "script.source.too-deeply-nested");
}

#[test]
fn script_backend_parses_deep_nesting_on_the_worker_thread_stack() {
    // 100-deep nesting overflows a default 2 MiB thread stack during parsing (it does abort the
    // process when parsed on the test thread directly), but parses on the dedicated worker
    // thread's larger stack (`SCRIPT_WORKER_STACK_BYTES`, 16 MiB), well inside the depth bound.
    // If this ever flakes, parser stack frames grew — raise the worker stack rather than this
    // depth.
    let mut backend = ScriptBackend::new(HostCallPolicy::deny_all(), TimerPolicy::deterministic());
    let open = "(".repeat(100);
    let close = ")".repeat(100);
    let nested = format!("{open}1 + 2{close}");

    let execution = backend
        .execute_module(ScriptModule::javascript("deep-nesting.js", nested))
        .expect("legitimate deep nesting parses on the worker thread's larger stack");

    assert_eq!(execution.value(), &StructuredValue::Number(3.0));
}

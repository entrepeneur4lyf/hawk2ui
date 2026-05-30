#![forbid(unsafe_code)]
//! Production script backend for `Hawk2UI` `JavaScript` and `TypeScript` execution.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::{self, Write as _},
    path::Path,
};

use boa_engine::{Context, JsValue, JsVariant, Source};
use hawk2ui_api::Diagnostic;
use oxc_allocator::Allocator;
use oxc_codegen::Codegen;
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;
use oxc_transformer::{HelperLoaderMode, TransformOptions, Transformer};

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-script";

/// Returns the canonical Cargo package name for diagnostics and conformance checks.
#[must_use]
pub const fn crate_name() -> &'static str {
    CRATE_NAME
}

/// Script module kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScriptModuleKind {
    /// JavaScript module.
    JavaScript,
    /// TypeScript source that has been compiled to JavaScript before execution.
    TypeScript,
}

/// Script module input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScriptModule {
    id: String,
    source: String,
    kind: ScriptModuleKind,
}

impl ScriptModule {
    /// Creates a JavaScript module.
    #[must_use]
    pub fn javascript(id: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            source: source.into(),
            kind: ScriptModuleKind::JavaScript,
        }
    }

    /// Creates a TypeScript module.
    #[must_use]
    pub fn typescript(id: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            source: source.into(),
            kind: ScriptModuleKind::TypeScript,
        }
    }

    /// Creates a module whose kind is inferred from `source_path`'s extension:
    /// `.ts`/`.tsx` produce a TypeScript module, everything else JavaScript. The
    /// path doubles as the module id.
    #[must_use]
    pub fn for_source_path(source_path: &str, source: impl Into<String>) -> Self {
        let is_typescript = std::path::Path::new(source_path)
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                extension.eq_ignore_ascii_case("ts") || extension.eq_ignore_ascii_case("tsx")
            });
        if is_typescript {
            Self::typescript(source_path, source)
        } else {
            Self::javascript(source_path, source)
        }
    }

    /// Returns the module kind.
    #[must_use]
    pub const fn kind(&self) -> ScriptModuleKind {
        self.kind
    }
}

/// Wraps a compiled entry module so its exported `mount` function is invoked
/// under a host object and its result is serialized to a JSON node-tree string
/// — the convention an entry script's `mount(host)` follows. Returns `None` when
/// the source declares no `mount` function (the caller then falls back, e.g. to
/// a visible-title probe).
///
/// Both the desktop host and the plugin editor run the same entry script, so
/// they share this convention rather than each reinventing it. The injected
/// `__hawk2ui_host` is currently a no-op stub; it is the seam where host
/// bindings (parameter reads, state) will be projected to editor JS.
#[must_use]
pub fn entry_mount_bootstrap(source: &str) -> Option<String> {
    let source = source.replacen("export function mount", "function mount", 1);
    if !source.contains("function mount") {
        return None;
    }
    Some(format!(
        r"{source}

const __hawk2ui_host = Object.freeze({{
    on(_name, _handler) {{}},
    setState(_value) {{}}
}});

JSON.stringify(mount(__hawk2ui_host));
"
    ))
}

/// Structured script value.
#[derive(Clone, Debug, PartialEq)]
pub enum StructuredValue {
    /// Null value.
    Null,
    /// Boolean value.
    Bool(bool),
    /// Numeric value.
    Number(f64),
    /// String value.
    String(String),
}

/// Script execution output.
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptExecution {
    module_id: String,
    value: StructuredValue,
}

impl ScriptExecution {
    /// Returns the execution value.
    #[must_use]
    pub const fn value(&self) -> &StructuredValue {
        &self.value
    }
}

/// Default maximum accepted source byte length (1 MiB).
const DEFAULT_MAX_SOURCE_BYTES: usize = 1_048_576;

/// Default maximum accepted compiled `JavaScript` byte length (4 MiB).
const DEFAULT_MAX_COMPILED_SOURCE_BYTES: usize = 4_194_304;

/// Default maximum loop iterations permitted before untrusted execution is aborted.
///
/// `boa` leaves its loop-iteration limit at [`u64::MAX`] (unbounded) by default, so an
/// infinite or pathological loop in untrusted script would run forever on the calling thread
/// and wedge the host (or the DAW hosting a plugin editor). This bound makes such a loop
/// terminate with a recoverable error instead.
const DEFAULT_MAX_LOOP_ITERATIONS: u64 = 10_000_000;

/// Default maximum source nesting depth permitted before parsing.
///
/// `JavaScript`/`TypeScript` are parsed by unguarded recursive descent, so deeply nested
/// source can overflow the native stack *before* any runtime limit applies — an uncatchable
/// process abort. Source is depth-bounded before it reaches either parser. Mirrors
/// `hawk2ui_a11y`'s `A11Y_MAX_TREE_DEPTH`.
const DEFAULT_MAX_NESTING_DEPTH: usize = 256;

/// Resource limits enforced on untrusted script source and execution.
///
/// Byte-length limits bound parser/codegen workload; the loop-iteration limit bounds runtime
/// CPU and the nesting-depth limit bounds parse-time native stack usage, so that untrusted
/// script cannot hang, exhaust memory, or crash the host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScriptExecutionLimits {
    source_bytes: usize,
    compiled_source_bytes: usize,
    loop_iterations: u64,
    nesting_depth: usize,
}

impl ScriptExecutionLimits {
    /// Default limits: 1 MiB source, 4 MiB compiled, 10,000,000 loop iterations, depth 256.
    pub const DEFAULT: Self = Self {
        source_bytes: DEFAULT_MAX_SOURCE_BYTES,
        compiled_source_bytes: DEFAULT_MAX_COMPILED_SOURCE_BYTES,
        loop_iterations: DEFAULT_MAX_LOOP_ITERATIONS,
        nesting_depth: DEFAULT_MAX_NESTING_DEPTH,
    };

    /// Creates source-size limits for original and compiled `JavaScript`.
    ///
    /// Runtime limits (loop iterations, nesting depth) take their default values; override
    /// them with [`Self::with_max_loop_iterations`] / [`Self::with_max_nesting_depth`].
    #[must_use]
    pub const fn new(max_source_bytes: usize, max_compiled_source_bytes: usize) -> Self {
        Self {
            source_bytes: max_source_bytes,
            compiled_source_bytes: max_compiled_source_bytes,
            loop_iterations: DEFAULT_MAX_LOOP_ITERATIONS,
            nesting_depth: DEFAULT_MAX_NESTING_DEPTH,
        }
    }

    /// Overrides the maximum loop iterations permitted during execution.
    #[must_use]
    pub const fn with_max_loop_iterations(mut self, max_loop_iterations: u64) -> Self {
        self.loop_iterations = max_loop_iterations;
        self
    }

    /// Overrides the maximum source nesting depth permitted before parsing.
    #[must_use]
    pub const fn with_max_nesting_depth(mut self, max_nesting_depth: usize) -> Self {
        self.nesting_depth = max_nesting_depth;
        self
    }

    /// Returns the maximum accepted source byte length.
    #[must_use]
    pub const fn max_source_bytes(&self) -> usize {
        self.source_bytes
    }

    /// Returns the maximum accepted compiled `JavaScript` byte length.
    #[must_use]
    pub const fn max_compiled_source_bytes(&self) -> usize {
        self.compiled_source_bytes
    }

    /// Returns the maximum loop iterations permitted during execution.
    #[must_use]
    pub const fn max_loop_iterations(&self) -> u64 {
        self.loop_iterations
    }

    /// Returns the maximum source nesting depth permitted before parsing.
    #[must_use]
    pub const fn max_nesting_depth(&self) -> usize {
        self.nesting_depth
    }
}

impl Default for ScriptExecutionLimits {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Host call policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostCallPolicy {
    allowed_bindings: BTreeSet<String>,
}

impl HostCallPolicy {
    /// Denies all host calls.
    #[must_use]
    pub const fn deny_all() -> Self {
        Self {
            allowed_bindings: BTreeSet::new(),
        }
    }

    /// Allows the provided host bindings.
    #[must_use]
    pub fn allow<const N: usize>(bindings: [&str; N]) -> Self {
        Self {
            allowed_bindings: bindings.into_iter().map(str::to_string).collect(),
        }
    }

    fn permits(&self, binding: &str) -> bool {
        self.allowed_bindings.contains(binding)
    }
}

/// Timer execution policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimerPolicy {
    deterministic: bool,
}

impl TimerPolicy {
    /// Creates deterministic timers for tests and plugin-safe scheduling.
    #[must_use]
    pub const fn deterministic() -> Self {
        Self {
            deterministic: true,
        }
    }
}

/// Promise identifier.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PromiseId(u64);

/// Timer identifier.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TimerId(u64);

/// Promise state.
#[derive(Clone, Debug, PartialEq)]
pub struct PromiseState {
    label: String,
    value: Option<StructuredValue>,
}

impl PromiseState {
    /// Returns resolved value.
    #[must_use]
    pub const fn value(&self) -> Option<&StructuredValue> {
        self.value.as_ref()
    }
}

/// Timer record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimerRecord {
    id: TimerId,
    label: String,
    delay_ms: u64,
}

impl TimerRecord {
    /// Returns timer ID.
    #[must_use]
    pub const fn id(&self) -> TimerId {
        self.id
    }
}

/// Host call record.
#[derive(Clone, Debug, PartialEq)]
pub struct HostCall {
    binding: String,
    payload: StructuredValue,
}

impl HostCall {
    /// Returns called binding name.
    #[must_use]
    pub fn binding(&self) -> &str {
        &self.binding
    }
}

/// Script diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScriptDiagnostic {
    rule: String,
    message: String,
}

impl ScriptDiagnostic {
    /// Creates a script diagnostic.
    #[must_use]
    pub fn new(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            rule: rule.into(),
            message: message.into(),
        }
    }

    /// Returns the diagnostic rule.
    #[must_use]
    pub fn rule(&self) -> &str {
        &self.rule
    }

    /// Returns the diagnostic message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Script backend error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScriptBackendError {
    diagnostic: ScriptDiagnostic,
}

impl ScriptBackendError {
    /// Creates a script backend error.
    #[must_use]
    pub fn new(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            diagnostic: ScriptDiagnostic::new(rule, message),
        }
    }

    /// Returns structured diagnostic.
    #[must_use]
    pub const fn diagnostic(&self) -> &ScriptDiagnostic {
        &self.diagnostic
    }
}

impl From<ScriptBackendError> for Diagnostic {
    fn from(error: ScriptBackendError) -> Self {
        Self::error(error.diagnostic.rule, error.diagnostic.message)
    }
}

/// Production script backend boundary.
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptBackend {
    host_policy: HostCallPolicy,
    timer_policy: TimerPolicy,
    executed_modules: Vec<ScriptModule>,
    promises: BTreeMap<PromiseId, PromiseState>,
    timers: Vec<TimerRecord>,
    host_calls: Vec<HostCall>,
    execution_limits: ScriptExecutionLimits,
    next_promise_id: u64,
    next_timer_id: u64,
    interrupted: Option<String>,
    torn_down: bool,
}

impl ScriptBackend {
    /// Creates a script backend.
    #[must_use]
    pub const fn new(host_policy: HostCallPolicy, timer_policy: TimerPolicy) -> Self {
        Self {
            host_policy,
            timer_policy,
            executed_modules: Vec::new(),
            promises: BTreeMap::new(),
            timers: Vec::new(),
            host_calls: Vec::new(),
            execution_limits: ScriptExecutionLimits::DEFAULT,
            next_promise_id: 1,
            next_timer_id: 1,
            interrupted: None,
            torn_down: false,
        }
    }

    /// Overrides deterministic execution limits.
    #[must_use]
    pub const fn with_execution_limits(mut self, execution_limits: ScriptExecutionLimits) -> Self {
        self.execution_limits = execution_limits;
        self
    }

    /// Executes a module.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptBackendError`] when execution is interrupted, torn down, or unsupported.
    pub fn execute_module(
        &mut self,
        module: ScriptModule,
    ) -> Result<ScriptExecution, ScriptBackendError> {
        self.ensure_running()?;
        let executable = Self::compile_module_source(&module, self.execution_limits)?;
        let value = evaluate_javascript(&executable, self.execution_limits)?;
        let execution = ScriptExecution {
            module_id: module.id.clone(),
            value,
        };
        self.executed_modules.push(module);
        Ok(execution)
    }

    /// Executes a module after projecting Rust-owned host promises and timers into Boa.
    ///
    /// The module can call `hawk2ui.promise(label)` to receive a real JavaScript `Promise` backed
    /// by resolved host promise records, and `hawk2ui.onTimer(label, callback)` to register a
    /// deterministic timer callback. After evaluation, Boa jobs are drained, registered timer
    /// callbacks for scheduled Rust timers are invoked, and jobs are drained again.
    ///
    /// The returned value is `globalThis.__hawk2uiResult` after host jobs settle when that global is
    /// defined; otherwise the module evaluation result is returned.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptBackendError`] when execution is interrupted, torn down, exceeds configured
    /// limits, fails JavaScript evaluation, or a projected host job fails.
    pub fn execute_module_with_host_jobs(
        &mut self,
        module: ScriptModule,
    ) -> Result<ScriptExecution, ScriptBackendError> {
        self.ensure_running()?;
        let executable = Self::compile_module_source(&module, self.execution_limits)?;
        let value = evaluate_javascript_with_host_jobs(
            &executable,
            self.execution_limits,
            &self.promises,
            &self.timers,
        )?;
        let execution = ScriptExecution {
            module_id: module.id.clone(),
            value,
        };
        self.executed_modules.push(module);
        Ok(execution)
    }

    /// Compiles a JavaScript or TypeScript module into executable JavaScript without evaluating it.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptBackendError`] when source limits are exceeded or TypeScript parsing,
    /// semantic analysis, or transformation fails.
    pub fn compile_module_source(
        module: &ScriptModule,
        execution_limits: ScriptExecutionLimits,
    ) -> Result<String, ScriptBackendError> {
        enforce_source_limit(
            "script.source.too-large",
            "script source exceeds configured execution limit",
            module.source.len(),
            execution_limits.max_source_bytes(),
        )?;
        let executable = match module.kind {
            ScriptModuleKind::JavaScript => module.source.clone(),
            ScriptModuleKind::TypeScript => compile_typescript(
                &module.id,
                &module.source,
                execution_limits.max_nesting_depth(),
            )?,
        };
        enforce_source_limit(
            "script.compiled-source.too-large",
            "compiled JavaScript exceeds configured execution limit",
            executable.len(),
            execution_limits.max_compiled_source_bytes(),
        )?;
        Ok(executable)
    }

    /// Returns executed modules.
    #[must_use]
    pub fn executed_modules(&self) -> &[ScriptModule] {
        &self.executed_modules
    }

    /// Creates a promise record.
    pub fn create_promise(&mut self, label: impl Into<String>) -> PromiseId {
        let id = PromiseId(self.next_promise_id);
        self.next_promise_id = self.next_promise_id.saturating_add(1);
        self.promises.insert(
            id,
            PromiseState {
                label: label.into(),
                value: None,
            },
        );
        id
    }

    /// Resolves a promise.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptBackendError`] when the promise is unknown.
    pub fn resolve_promise(
        &mut self,
        id: PromiseId,
        value: StructuredValue,
    ) -> Result<(), ScriptBackendError> {
        let Some(promise) = self.promises.get_mut(&id) else {
            return Err(ScriptBackendError::new(
                "script.promise.missing",
                "promise does not exist",
            ));
        };
        promise.value = Some(value);
        Ok(())
    }

    /// Returns promise state.
    #[must_use]
    pub fn promise_state(&self, id: PromiseId) -> Option<&PromiseState> {
        self.promises.get(&id)
    }

    /// Schedules a deterministic timer.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptBackendError`] when timers are unavailable or runtime is torn down.
    pub fn schedule_timer(
        &mut self,
        label: impl Into<String>,
        delay_ms: u64,
    ) -> Result<TimerId, ScriptBackendError> {
        self.ensure_running()?;
        if !self.timer_policy.deterministic {
            return Err(ScriptBackendError::new(
                "script.timer.unavailable",
                "timer policy does not allow scheduling",
            ));
        }
        let id = TimerId(self.next_timer_id);
        self.next_timer_id = self.next_timer_id.saturating_add(1);
        self.timers.push(TimerRecord {
            id,
            label: label.into(),
            delay_ms,
        });
        Ok(id)
    }

    /// Returns timers.
    #[must_use]
    pub fn timers(&self) -> &[TimerRecord] {
        &self.timers
    }

    /// Calls a typed host binding.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptBackendError`] when the binding is denied.
    pub fn call_host(
        &mut self,
        binding: impl Into<String>,
        payload: StructuredValue,
    ) -> Result<HostCall, ScriptBackendError> {
        self.ensure_running()?;
        let binding = binding.into();
        if !self.host_policy.permits(&binding) {
            return Err(ScriptBackendError::new(
                "script.host-call.denied",
                "host binding is not permitted by policy",
            ));
        }
        let call = HostCall { binding, payload };
        self.host_calls.push(call.clone());
        Ok(call)
    }

    /// Interrupts further script execution.
    pub fn interrupt(&mut self, reason: impl Into<String>) {
        self.interrupted = Some(reason.into());
    }

    /// Tears down runtime-owned state.
    pub fn teardown(&mut self) {
        self.torn_down = true;
        self.promises.clear();
        self.timers.clear();
    }

    /// Returns whether the backend is torn down.
    #[must_use]
    pub const fn torn_down(&self) -> bool {
        self.torn_down
    }

    fn ensure_running(&self) -> Result<(), ScriptBackendError> {
        if self.torn_down {
            Err(ScriptBackendError::new(
                "script.torn-down",
                "script runtime has been torn down",
            ))
        } else if self.interrupted.is_some() {
            Err(ScriptBackendError::new(
                "script.interrupted",
                "script runtime has been interrupted",
            ))
        } else {
            Ok(())
        }
    }
}

fn enforce_source_limit(
    rule: &'static str,
    message: &'static str,
    actual_bytes: usize,
    max_bytes: usize,
) -> Result<(), ScriptBackendError> {
    if actual_bytes <= max_bytes {
        return Ok(());
    }
    Err(ScriptBackendError::new(
        rule,
        format!("{message}: {actual_bytes} bytes exceeds {max_bytes} bytes"),
    ))
}

/// Rejects source whose bracket nesting depth exceeds `max_depth`.
///
/// `JavaScript`/`TypeScript` are parsed by unguarded recursive descent (neither `oxc_parser`
/// nor `boa`'s parser bounds nesting depth), so deeply nested source can overflow the native
/// thread stack *during parsing* — a `SIGSEGV`/abort that [`std::panic::catch_unwind`] cannot
/// recover. This bound therefore runs before either parser sees the source.
///
/// The scan counts `(`, `[`, `{` as openers and their closers, saturating at zero so leading
/// closers cannot mask later nesting. It tracks depth (not raw count), so balanced brackets in
/// string literals do not trip it, and because every opener counts the bound cannot be bypassed.
fn enforce_nesting_depth(source: &str, max_depth: usize) -> Result<(), ScriptBackendError> {
    let mut depth: usize = 0;
    for &byte in source.as_bytes() {
        match byte {
            b'(' | b'[' | b'{' => {
                depth += 1;
                if depth > max_depth {
                    return Err(ScriptBackendError::new(
                        "script.source.too-deeply-nested",
                        format!(
                            "script source nesting depth exceeds configured limit of {max_depth}"
                        ),
                    ));
                }
            }
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    Ok(())
}

/// Applies runtime resource limits to a freshly created context before untrusted execution.
///
/// `boa` leaves the loop-iteration limit unbounded ([`u64::MAX`]) by default; bounding it makes
/// an infinite or pathological loop terminate with a recoverable error instead of hanging the
/// calling thread. `boa`'s recursion and stack-size limits are already bounded by its own
/// defaults.
fn apply_runtime_limits(context: &mut Context, limits: ScriptExecutionLimits) {
    context
        .runtime_limits_mut()
        .set_loop_iteration_limit(limits.max_loop_iterations());
}

/// Stack size for the worker thread that parses and evaluates untrusted source.
///
/// Untrusted parsing runs on a dedicated thread with this fixed, generous stack so the
/// nesting-depth bound is calibrated against a *known* stack rather than whatever (possibly
/// small) stack the host or DAW happens to invoke us on. The worker does not *contain* a stack
/// overflow — that remains prevented by [`enforce_nesting_depth`] — but it decouples the safe
/// parse depth from the caller's thread, which matters for the embedded plugin editor.
const SCRIPT_WORKER_STACK_BYTES: usize = 16 * 1024 * 1024;

/// Runs untrusted parsing/evaluation on a dedicated worker thread, returning its result.
///
/// The worker has a known, generous stack ([`SCRIPT_WORKER_STACK_BYTES`]) so legitimately nested
/// source parses regardless of the caller's stack size. Joining the worker also converts a
/// catchable `boa`/`oxc` panic into a diagnostic instead of letting it unwind through the host. A
/// native stack overflow is not a catchable panic — that case is prevented up front by
/// [`enforce_nesting_depth`].
fn run_on_worker<T: Send>(
    panic_rule: &'static str,
    operation: impl FnOnce() -> Result<T, ScriptBackendError> + Send,
) -> Result<T, ScriptBackendError> {
    std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .stack_size(SCRIPT_WORKER_STACK_BYTES)
            .spawn_scoped(scope, operation)
            .map_err(|error| {
                ScriptBackendError::new(
                    "script.worker.spawn-failed",
                    format!("failed to spawn script worker thread: {error}"),
                )
            })?;
        worker.join().map_err(|_| {
            ScriptBackendError::new(
                panic_rule,
                "script engine panicked while processing untrusted source",
            )
        })?
    })
}

fn compile_typescript(
    module_id: &str,
    source: &str,
    max_nesting_depth: usize,
) -> Result<String, ScriptBackendError> {
    enforce_nesting_depth(source, max_nesting_depth)?;
    run_on_worker("script.typescript.panicked", || {
        compile_typescript_inner(module_id, source)
    })
}

fn compile_typescript_inner(module_id: &str, source: &str) -> Result<String, ScriptBackendError> {
    let allocator = Allocator::default();
    let source_path = Path::new(module_id);
    let source_type = SourceType::from_path(source_path).unwrap_or_else(|_| SourceType::ts());
    let parse_return = Parser::new(&allocator, source, source_type).parse();
    if !parse_return.errors.is_empty() {
        return Err(ScriptBackendError::new(
            "script.typescript.parse-failed",
            format_oxc_diagnostics("TypeScript parse failed", parse_return.errors),
        ));
    }

    let mut program = parse_return.program;
    let semantic_return = SemanticBuilder::new()
        .with_excess_capacity(2.0)
        .with_enum_eval(true)
        .build(&program);
    if !semantic_return.errors.is_empty() {
        return Err(ScriptBackendError::new(
            "script.typescript.semantic-failed",
            format_oxc_diagnostics(
                "TypeScript semantic analysis failed",
                semantic_return.errors,
            ),
        ));
    }

    let mut options = TransformOptions::default();
    options.helper_loader.mode = HelperLoaderMode::External;
    let transform_return = Transformer::new(&allocator, source_path, &options)
        .build_with_scoping(semantic_return.semantic.into_scoping(), &mut program);
    if !transform_return.errors.is_empty() {
        return Err(ScriptBackendError::new(
            "script.typescript.transform-failed",
            format_oxc_diagnostics("TypeScript transform failed", transform_return.errors),
        ));
    }

    Ok(Codegen::new().build(&program).code)
}

fn format_oxc_diagnostics<T: fmt::Debug>(prefix: &'static str, errors: Vec<T>) -> String {
    let details = errors
        .into_iter()
        .map(|error| format!("{error:?}"))
        .collect::<Vec<_>>()
        .join("; ");
    format!("{prefix}: {details}")
}

fn evaluate_javascript(
    source: &str,
    limits: ScriptExecutionLimits,
) -> Result<StructuredValue, ScriptBackendError> {
    enforce_nesting_depth(source, limits.max_nesting_depth())?;
    run_on_worker("script.eval.panicked", || {
        let mut context = Context::default();
        apply_runtime_limits(&mut context, limits);
        let value = context.eval(Source::from_bytes(source)).map_err(|error| {
            ScriptBackendError::new(
                "script.eval.failed",
                format!("JavaScript execution failed: {error}"),
            )
        })?;
        context.run_jobs().map_err(|error| {
            ScriptBackendError::new(
                "script.jobs.failed",
                format!("JavaScript job queue failed: {error}"),
            )
        })?;
        structured_value_from_js(&value)
    })
}

fn evaluate_javascript_with_host_jobs(
    source: &str,
    limits: ScriptExecutionLimits,
    promises: &BTreeMap<PromiseId, PromiseState>,
    timers: &[TimerRecord],
) -> Result<StructuredValue, ScriptBackendError> {
    enforce_nesting_depth(source, limits.max_nesting_depth())?;
    run_on_worker("script.host-jobs.panicked", || {
        evaluate_javascript_with_host_jobs_inner(source, limits, promises, timers)
    })
}

fn evaluate_javascript_with_host_jobs_inner(
    source: &str,
    limits: ScriptExecutionLimits,
    promises: &BTreeMap<PromiseId, PromiseState>,
    timers: &[TimerRecord],
) -> Result<StructuredValue, ScriptBackendError> {
    let mut context = Context::default();
    apply_runtime_limits(&mut context, limits);
    eval_js_unit(
        &mut context,
        host_job_prelude(),
        "script.host-jobs.bootstrap-failed",
    )?;
    for promise in promises.values() {
        if let Some(value) = promise.value() {
            eval_js_unit(
                &mut context,
                &format!(
                    "globalThis.__hawk2uiResolve({}, {});",
                    js_string_literal(&promise.label),
                    structured_value_js_literal(value)?
                ),
                "script.host-jobs.promise-bootstrap-failed",
            )?;
        }
    }

    let evaluation_result = context.eval(Source::from_bytes(source)).map_err(|error| {
        ScriptBackendError::new(
            "script.eval.failed",
            format!("JavaScript execution failed: {error}"),
        )
    })?;
    run_boa_jobs(&mut context)?;

    for timer in timers {
        eval_js_unit(
            &mut context,
            &format!(
                "globalThis.__hawk2uiFlushTimer({});",
                js_string_literal(&timer.label)
            ),
            "script.host-jobs.timer-failed",
        )?;
        run_boa_jobs(&mut context)?;
    }

    let settled_result = context
        .eval(Source::from_bytes(
            "typeof globalThis.__hawk2uiResult === 'undefined' ? undefined : globalThis.__hawk2uiResult",
        ))
        .map_err(|error| {
            ScriptBackendError::new(
                "script.host-jobs.result-read-failed",
                format!("JavaScript host job result read failed: {error}"),
            )
        })?;
    if matches!(settled_result.variant(), JsVariant::Undefined) {
        structured_value_from_js(&evaluation_result)
    } else {
        structured_value_from_js(&settled_result)
    }
}

fn host_job_prelude() -> &'static str {
    r#"
const __hawk2uiResolvedPromises = new Map();
const __hawk2uiTimerCallbacks = new Map();
globalThis.__hawk2uiResolve = (label, value) => {
  __hawk2uiResolvedPromises.set(label, value);
};
globalThis.__hawk2uiFlushTimer = (label) => {
  const callback = __hawk2uiTimerCallbacks.get(label);
  if (callback !== undefined) {
    callback();
  }
};
globalThis.hawk2ui = Object.freeze({
  promise(label) {
    if (!__hawk2uiResolvedPromises.has(label)) {
      return Promise.reject(new Error(`host promise is not resolved: ${label}`));
    }
    return Promise.resolve(__hawk2uiResolvedPromises.get(label));
  },
  onTimer(label, callback) {
    if (typeof callback !== "function") {
      throw new TypeError("timer callback must be a function");
    }
    __hawk2uiTimerCallbacks.set(label, callback);
    return label;
  }
});
"#
}

fn eval_js_unit(
    context: &mut Context,
    source: &str,
    rule: &'static str,
) -> Result<(), ScriptBackendError> {
    context.eval(Source::from_bytes(source)).map_err(|error| {
        ScriptBackendError::new(rule, format!("JavaScript host job setup failed: {error}"))
    })?;
    Ok(())
}

fn run_boa_jobs(context: &mut Context) -> Result<(), ScriptBackendError> {
    context.run_jobs().map_err(|error| {
        ScriptBackendError::new(
            "script.jobs.failed",
            format!("JavaScript job queue failed: {error}"),
        )
    })
}

fn structured_value_js_literal(value: &StructuredValue) -> Result<String, ScriptBackendError> {
    match value {
        StructuredValue::Null => Ok("null".to_string()),
        StructuredValue::Bool(value) => Ok(value.to_string()),
        StructuredValue::Number(value) if value.is_finite() => Ok(value.to_string()),
        StructuredValue::Number(_) => Err(ScriptBackendError::new(
            "script.value.invalid-number",
            "host promise numeric value must be finite",
        )),
        StructuredValue::String(value) => Ok(js_string_literal(value)),
    }
}

fn js_string_literal(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                let _ = write!(escaped, "\\u{:04x}", u32::from(character));
            }
            character => escaped.push(character),
        }
    }
    escaped.push('"');
    escaped
}

fn structured_value_from_js(value: &JsValue) -> Result<StructuredValue, ScriptBackendError> {
    match value.variant() {
        JsVariant::Null | JsVariant::Undefined => Ok(StructuredValue::Null),
        JsVariant::Boolean(value) => Ok(StructuredValue::Bool(value)),
        JsVariant::Float64(value) => Ok(StructuredValue::Number(value)),
        JsVariant::Integer32(value) => Ok(StructuredValue::Number(f64::from(value))),
        JsVariant::String(value) => {
            value
                .to_std_string()
                .map(StructuredValue::String)
                .map_err(|_| {
                    ScriptBackendError::new(
                        "script.value.unsupported-string",
                        "JavaScript string result cannot be represented as UTF-8",
                    )
                })
        }
        JsVariant::BigInt(_) | JsVariant::Object(_) | JsVariant::Symbol(_) => {
            Err(ScriptBackendError::new(
                "script.value.unsupported",
                "JavaScript result type cannot be represented as a structured Hawk2UI value",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_identity() {
        assert_eq!(crate_name(), "hawk2ui-script");
    }

    #[test]
    fn entry_mount_bootstrap_wraps_a_mount_function() {
        let bootstrap = entry_mount_bootstrap("export function mount(host) { return \"{}\"; }")
            .expect("a mount function bootstraps");
        // The export is rewritten to a local declaration and the host-invoked
        // result is serialized to a JSON node tree.
        assert!(bootstrap.contains("function mount(host)"));
        assert!(!bootstrap.contains("export function mount"));
        assert!(bootstrap.contains("JSON.stringify(mount(__hawk2ui_host));"));
    }

    #[test]
    fn entry_mount_bootstrap_returns_none_without_a_mount_function() {
        assert!(entry_mount_bootstrap("export function other() {}").is_none());
    }

    #[test]
    fn module_kind_is_inferred_from_source_path() {
        assert_eq!(
            ScriptModule::for_source_path("src/main.ts", "").kind(),
            ScriptModuleKind::TypeScript
        );
        assert_eq!(
            ScriptModule::for_source_path("src/main.js", "").kind(),
            ScriptModuleKind::JavaScript
        );
    }
}

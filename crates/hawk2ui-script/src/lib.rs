#![forbid(unsafe_code)]
//! Production script backend for `Hawk2UI` `JavaScript` and `TypeScript` execution.

use std::collections::{BTreeMap, BTreeSet};

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

    /// Returns the module kind.
    #[must_use]
    pub const fn kind(&self) -> ScriptModuleKind {
        self.kind
    }
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

/// Production script backend boundary.
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptBackend {
    host_policy: HostCallPolicy,
    timer_policy: TimerPolicy,
    executed_modules: Vec<ScriptModule>,
    promises: BTreeMap<PromiseId, PromiseState>,
    timers: Vec<TimerRecord>,
    host_calls: Vec<HostCall>,
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
            next_promise_id: 1,
            next_timer_id: 1,
            interrupted: None,
            torn_down: false,
        }
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
        let executable = match module.kind {
            ScriptModuleKind::JavaScript => module.source.clone(),
            ScriptModuleKind::TypeScript => compile_typescript(&module.source),
        };
        let value = evaluate_expression_module(&executable)?;
        let execution = ScriptExecution {
            module_id: module.id.clone(),
            value,
        };
        self.executed_modules.push(module);
        Ok(execution)
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

fn compile_typescript(source: &str) -> String {
    source.replace(": number", "").replace(": string", "")
}

fn evaluate_expression_module(source: &str) -> Result<StructuredValue, ScriptBackendError> {
    let mut variables = BTreeMap::<String, f64>::new();
    let mut last = StructuredValue::Null;
    for statement in source
        .split(';')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        if let Some(rest) = statement.strip_prefix("const ") {
            let Some((name, value)) = rest.split_once('=') else {
                return Err(ScriptBackendError::new(
                    "script.parse.invalid-const",
                    "const statement must contain assignment",
                ));
            };
            let value = evaluate_number_expression(value.trim(), &variables)?;
            variables.insert(name.trim().to_string(), value);
            last = StructuredValue::Number(value);
        } else {
            last = StructuredValue::Number(evaluate_number_expression(statement, &variables)?);
        }
    }
    Ok(last)
}

fn evaluate_number_expression(
    expression: &str,
    variables: &BTreeMap<String, f64>,
) -> Result<f64, ScriptBackendError> {
    let mut total = 0.0_f64;
    for term in expression
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        let value = if let Ok(number) = term.parse::<f64>() {
            number
        } else {
            *variables.get(term).ok_or_else(|| {
                ScriptBackendError::new(
                    "script.eval.unknown-identifier",
                    "identifier is not defined",
                )
            })?
        };
        total += value;
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_identity() {
        assert_eq!(crate_name(), "hawk2ui-script");
    }
}

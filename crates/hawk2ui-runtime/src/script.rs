//! Runtime script module records and structured host-call data.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::TimerJob;

/// Runtime capability required by a module, host call, or scheduler operation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum RuntimeCapability {
    /// Dispatches or receives user-interface events.
    UiEvents,
    /// Requests render invalidation.
    RenderInvalidation,
    /// Reads from the platform clipboard.
    ClipboardRead,
    /// Writes to the platform clipboard.
    ClipboardWrite,
    /// Performs network requests.
    NetworkRequest,
    /// Accesses filesystem data.
    FilesystemAccess,
    /// Reads runtime secrets.
    SecretRead,
    /// Accesses application database APIs.
    DatabaseAccess,
    /// Uses host-managed audio playback APIs.
    AudioPlayback,
    /// Calls a configured AI provider.
    AiProvider,
    /// Calls configured MCP tools or servers.
    Mcp,
    /// Opens host dialogs or file pickers.
    Dialogs,
    /// Sends host notifications.
    Notifications,
    /// Registers or handles global shortcuts.
    GlobalShortcuts,
    /// Uses plugin parameter APIs.
    PluginParameters,
}

/// Supported script module input kinds.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ScriptModuleKind {
    /// JavaScript module.
    JavaScript,
    /// Compiled TypeScript output.
    TypeScriptOutput,
}

/// Structured value exchanged between scripts and host bindings.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum StructuredValue {
    /// Null value.
    Null,
    /// Boolean value.
    Bool(bool),
    /// Number value.
    Number(f64),
    /// UTF-8 string value.
    String(String),
    /// Ordered list.
    Array(Vec<StructuredValue>),
    /// Deterministically ordered object.
    Object(BTreeMap<String, StructuredValue>),
}

impl StructuredValue {
    /// Creates a string value.
    #[must_use]
    pub fn string(value: impl Into<String>) -> Self {
        Self::String(value.into())
    }

    /// Creates an object value.
    #[must_use]
    pub fn object(entries: impl IntoIterator<Item = (impl Into<String>, StructuredValue)>) -> Self {
        Self::Object(
            entries
                .into_iter()
                .map(|(key, value)| (key.into(), value))
                .collect(),
        )
    }

    /// Returns the primitive schema label for this value.
    #[must_use]
    pub const fn schema_label(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "bool",
            Self::Number(_) => "number",
            Self::String(_) => "string",
            Self::Array(_) => "array",
            Self::Object(_) => "object",
        }
    }
}

/// Runtime script module identity and declarations.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ScriptModuleRecord {
    /// Stable module identifier.
    pub id: String,
    /// Source URL or artifact URI.
    pub source: String,
    /// Module input kind.
    pub kind: ScriptModuleKind,
    /// Optional content hash.
    pub hash: Option<String>,
    /// Capabilities required by this module.
    pub required_capabilities: Vec<RuntimeCapability>,
    /// Export names made available for lifecycle hooks or host callbacks.
    pub exports: Vec<String>,
}

impl ScriptModuleRecord {
    /// Creates a script module record.
    #[must_use]
    pub fn new(id: impl Into<String>, source: impl Into<String>, kind: ScriptModuleKind) -> Self {
        Self {
            id: id.into(),
            source: source.into(),
            kind,
            hash: None,
            required_capabilities: Vec::new(),
            exports: Vec::new(),
        }
    }

    /// Sets the content hash.
    #[must_use]
    pub fn with_hash(mut self, hash: impl Into<String>) -> Self {
        self.hash = Some(hash.into());
        self
    }

    /// Adds a required runtime capability.
    #[must_use]
    pub fn requires(mut self, capability: RuntimeCapability) -> Self {
        self.required_capabilities.push(capability);
        self
    }

    /// Adds an exported symbol.
    #[must_use]
    pub fn exports(mut self, export_name: impl Into<String>) -> Self {
        self.exports.push(export_name.into());
        self
    }

    /// Returns the stable module identity label used in diagnostics.
    #[must_use]
    pub fn identity(&self) -> String {
        format!("{}@{}", self.id, self.source)
    }
}

/// Recorded script-to-host call.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct HostCallRecord {
    /// Calling module identifier.
    pub module_id: String,
    /// Host binding name.
    pub binding_name: String,
    /// Structured call payload.
    pub payload: StructuredValue,
    /// Capability required to perform this call.
    pub required_capability: Option<RuntimeCapability>,
}

impl HostCallRecord {
    /// Creates a host call record.
    #[must_use]
    pub fn new(
        module_id: impl Into<String>,
        binding_name: impl Into<String>,
        payload: StructuredValue,
    ) -> Self {
        Self {
            module_id: module_id.into(),
            binding_name: binding_name.into(),
            payload,
            required_capability: None,
        }
    }

    /// Sets the capability required by this host call.
    #[must_use]
    pub const fn requires(mut self, capability: RuntimeCapability) -> Self {
        self.required_capability = Some(capability);
        self
    }
}

/// Structured runtime error.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeError {
    /// Stable runtime error code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
    /// Related runtime capability, when applicable.
    pub capability: Option<RuntimeCapability>,
}

impl RuntimeError {
    /// Creates a runtime error.
    #[must_use]
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            capability: None,
        }
    }

    /// Creates a denied host-call error.
    #[must_use]
    pub fn host_call_denied(
        binding_name: impl Into<String>,
        capability: RuntimeCapability,
        reason: impl Into<String>,
    ) -> Self {
        let binding_name = binding_name.into();
        Self {
            code: "runtime.host-call-denied".into(),
            message: format!("host call denied for {binding_name}: {}", reason.into()),
            capability: Some(capability),
        }
    }
}

/// Stable promise identifier used by script-engine adapters.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PromiseId {
    /// Promise identifier.
    pub id: String,
}

impl PromiseId {
    /// Creates a promise identifier.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

/// Script engine operation recorded by adapters and test engines.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum ScriptEngineOperation {
    /// Load a script module.
    LoadModule(ScriptModuleRecord),
    /// Call an exported function.
    CallExport {
        /// Module identifier.
        module_id: String,
        /// Exported function name.
        export_name: String,
        /// Structured argument.
        argument: StructuredValue,
    },
    /// Resolve a promise.
    ResolvePromise {
        /// Promise identifier.
        promise_id: PromiseId,
        /// Resolution value.
        value: StructuredValue,
    },
    /// Set a timer.
    SetTimer(TimerJob),
    /// Perform a host call.
    HostCall(HostCallRecord),
    /// Interrupt the script engine.
    Interrupt {
        /// Interrupt reason.
        reason: String,
    },
    /// Tear down the script engine.
    Teardown,
}

/// Script engine adapter error.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ScriptEngineError {
    /// Stable script-engine error code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
}

impl ScriptEngineError {
    fn teardown_complete() -> Self {
        Self {
            code: "script-engine.teardown-complete".into(),
            message: "script engine teardown is complete".into(),
        }
    }
}

/// Script engine boundary for JavaScript-compatible module runtimes.
pub trait ScriptEngine {
    /// Loads a JavaScript or compiled TypeScript module.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptEngineError`] when the engine cannot accept more work.
    fn load_module(&mut self, module: ScriptModuleRecord) -> Result<(), ScriptEngineError>;

    /// Calls an exported module function.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptEngineError`] when the engine cannot accept more work.
    fn call_export(
        &mut self,
        module_id: &str,
        export_name: &str,
        argument: StructuredValue,
    ) -> Result<(), ScriptEngineError>;

    /// Resolves a pending promise.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptEngineError`] when the engine cannot accept more work.
    fn resolve_promise(
        &mut self,
        promise_id: PromiseId,
        value: StructuredValue,
    ) -> Result<(), ScriptEngineError>;

    /// Sets a script timer.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptEngineError`] when the engine cannot accept more work.
    fn set_timer(&mut self, timer: TimerJob) -> Result<(), ScriptEngineError>;

    /// Calls a host binding from script.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptEngineError`] when the engine cannot accept more work.
    fn call_host(&mut self, call: HostCallRecord) -> Result<(), ScriptEngineError>;

    /// Interrupts script execution.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptEngineError`] when the engine cannot accept more work.
    fn interrupt(&mut self, reason: &str) -> Result<(), ScriptEngineError>;

    /// Tears down the script engine.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptEngineError`] when teardown cannot complete.
    fn teardown(&mut self) -> Result<(), ScriptEngineError>;
}

/// Recording script engine used by runtime tests and conformance fixtures.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct RecordingScriptEngine {
    operations: Vec<ScriptEngineOperation>,
    torn_down: bool,
}

impl RecordingScriptEngine {
    /// Returns recorded script engine operations in order.
    #[must_use]
    pub fn operations(&self) -> &[ScriptEngineOperation] {
        &self.operations
    }

    fn ensure_active(&self) -> Result<(), ScriptEngineError> {
        if self.torn_down {
            Err(ScriptEngineError::teardown_complete())
        } else {
            Ok(())
        }
    }
}

impl ScriptEngine for RecordingScriptEngine {
    fn load_module(&mut self, module: ScriptModuleRecord) -> Result<(), ScriptEngineError> {
        self.ensure_active()?;
        self.operations
            .push(ScriptEngineOperation::LoadModule(module));
        Ok(())
    }

    fn call_export(
        &mut self,
        module_id: &str,
        export_name: &str,
        argument: StructuredValue,
    ) -> Result<(), ScriptEngineError> {
        self.ensure_active()?;
        self.operations.push(ScriptEngineOperation::CallExport {
            module_id: module_id.into(),
            export_name: export_name.into(),
            argument,
        });
        Ok(())
    }

    fn resolve_promise(
        &mut self,
        promise_id: PromiseId,
        value: StructuredValue,
    ) -> Result<(), ScriptEngineError> {
        self.ensure_active()?;
        self.operations
            .push(ScriptEngineOperation::ResolvePromise { promise_id, value });
        Ok(())
    }

    fn set_timer(&mut self, timer: TimerJob) -> Result<(), ScriptEngineError> {
        self.ensure_active()?;
        self.operations.push(ScriptEngineOperation::SetTimer(timer));
        Ok(())
    }

    fn call_host(&mut self, call: HostCallRecord) -> Result<(), ScriptEngineError> {
        self.ensure_active()?;
        self.operations.push(ScriptEngineOperation::HostCall(call));
        Ok(())
    }

    fn interrupt(&mut self, reason: &str) -> Result<(), ScriptEngineError> {
        self.ensure_active()?;
        self.operations.push(ScriptEngineOperation::Interrupt {
            reason: reason.into(),
        });
        Ok(())
    }

    fn teardown(&mut self) -> Result<(), ScriptEngineError> {
        if !self.torn_down {
            self.operations.push(ScriptEngineOperation::Teardown);
            self.torn_down = true;
        }
        Ok(())
    }
}

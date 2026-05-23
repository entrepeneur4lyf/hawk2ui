//! Runtime script module records and structured host-call data.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

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

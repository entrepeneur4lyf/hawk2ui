//! Capability table and platform access checks.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Platform API operation.
#[derive(
    Clone, Copy, Debug, Deserialize, Eq, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
pub enum PlatformOperation {
    /// Filesystem read operation.
    FilesystemRead,
    /// Filesystem write operation.
    FilesystemWrite,
    /// Network request operation.
    NetworkRequest,
    /// Clipboard read operation.
    ClipboardRead,
    /// Clipboard write operation.
    ClipboardWrite,
    /// Secret read operation.
    SecretRead,
    /// Database query operation.
    DatabaseQuery,
    /// Database migration operation.
    DatabaseMigration,
    /// Host-managed audio playback operation.
    AudioPlayback,
    /// AI provider request operation.
    AiProviderRequest,
    /// MCP tool call operation.
    McpToolCall,
    /// Notification send operation.
    NotificationSend,
    /// Global shortcut registration operation.
    GlobalShortcutRegister,
    /// Localization bundle read operation.
    LocalizationRead,
    /// Dialog open operation.
    DialogOpen,
    /// File picker open operation.
    FilePickerOpen,
}

/// Platform execution context.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub enum PlatformContext {
    /// Desktop application context.
    Desktop,
    /// Embedded plugin editor context.
    Plugin,
}

/// Runtime availability for a platform capability.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub enum RuntimeAvailability {
    /// Available at runtime.
    Runtime,
    /// Build-only declaration.
    BuildOnly,
    /// Not available on the current platform.
    Unavailable,
}

/// Platform API schema labels.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilitySchema {
    /// Input schema label.
    pub input: String,
    /// Output schema label.
    pub output: String,
    /// Error schema label.
    pub error: String,
}

impl CapabilitySchema {
    /// Creates capability schema labels.
    #[must_use]
    pub fn new(
        input: impl Into<String>,
        output: impl Into<String>,
        error: impl Into<String>,
    ) -> Self {
        Self {
            input: input.into(),
            output: output.into(),
            error: error.into(),
        }
    }
}

impl Default for CapabilitySchema {
    fn default() -> Self {
        Self {
            input: "()".into(),
            output: "()".into(),
            error: "PlatformError".into(),
        }
    }
}

/// Platform diagnostic.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlatformDiagnostic {
    /// Stable diagnostic rule.
    pub rule: String,
    /// Human-readable message.
    pub message: String,
}

impl PlatformDiagnostic {
    /// Creates an error diagnostic.
    #[must_use]
    pub fn error(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            rule: rule.into(),
            message: message.into(),
        }
    }
}

/// Capability denial.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityDenied {
    /// Manifest capability key.
    pub manifest_key: String,
    /// Requested operation.
    pub operation: PlatformOperation,
    /// Structured diagnostic.
    pub diagnostic: PlatformDiagnostic,
}

/// Manifest-declared platform capability record.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityRecord {
    /// Manifest capability key.
    pub manifest_key: String,
    /// Allowed operations.
    pub allowed_operations: Vec<PlatformOperation>,
    /// Explicitly denied operations.
    pub denied_operations: Vec<PlatformOperation>,
    /// Input/output/error schema labels.
    pub schema: CapabilitySchema,
    /// Runtime availability.
    pub runtime_availability: RuntimeAvailability,
    /// Whether the capability is available in desktop contexts.
    pub desktop_applicable: bool,
    /// Whether the capability is available in plugin contexts.
    pub plugin_applicable: bool,
}

impl CapabilityRecord {
    /// Generates the JSON Schema for capability records.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformDiagnostic`] when the generated schema cannot be represented as JSON.
    pub fn json_schema() -> Result<serde_json::Value, PlatformDiagnostic> {
        capability_json_schema::<Self>(
            "capability.schema.record.generate-failed",
            "capability record schema",
        )
    }

    /// Validates a JSON value against the generated capability record schema.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformDiagnostic`] when schema compilation fails or the value fails validation.
    pub fn validate_json(value: &serde_json::Value) -> Result<(), PlatformDiagnostic> {
        validate_capability_json::<Self>(
            value,
            "capability.schema.record.compile-failed",
            "capability.schema.record.invalid",
            "capability record",
        )
    }

    /// Creates a capability record.
    #[must_use]
    pub fn new(manifest_key: impl Into<String>) -> Self {
        Self {
            manifest_key: manifest_key.into(),
            allowed_operations: Vec::new(),
            denied_operations: Vec::new(),
            schema: CapabilitySchema::default(),
            runtime_availability: RuntimeAvailability::Unavailable,
            desktop_applicable: false,
            plugin_applicable: false,
        }
    }

    /// Adds an allowed operation.
    #[must_use]
    pub fn allow(mut self, operation: PlatformOperation) -> Self {
        self.allowed_operations.push(operation);
        self
    }

    /// Adds an explicitly denied operation.
    #[must_use]
    pub fn deny(mut self, operation: PlatformOperation) -> Self {
        self.denied_operations.push(operation);
        self
    }

    /// Sets schema labels.
    #[must_use]
    pub fn schemas(mut self, schema: CapabilitySchema) -> Self {
        self.schema = schema;
        self
    }

    /// Sets runtime availability.
    #[must_use]
    pub const fn availability(mut self, availability: RuntimeAvailability) -> Self {
        self.runtime_availability = availability;
        self
    }

    /// Sets desktop applicability.
    #[must_use]
    pub const fn desktop(mut self, applicable: bool) -> Self {
        self.desktop_applicable = applicable;
        self
    }

    /// Sets plugin applicability.
    #[must_use]
    pub const fn plugin(mut self, applicable: bool) -> Self {
        self.plugin_applicable = applicable;
        self
    }
}

/// Platform capability table.
#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityTable {
    records: BTreeMap<String, CapabilityRecord>,
}

impl CapabilityTable {
    /// Generates the JSON Schema for capability tables.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformDiagnostic`] when the generated schema cannot be represented as JSON.
    pub fn json_schema() -> Result<serde_json::Value, PlatformDiagnostic> {
        capability_json_schema::<Self>(
            "capability.schema.table.generate-failed",
            "capability table schema",
        )
    }

    /// Validates a JSON value against the generated capability table schema.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformDiagnostic`] when schema compilation fails or the value fails validation.
    pub fn validate_json(value: &serde_json::Value) -> Result<(), PlatformDiagnostic> {
        validate_capability_json::<Self>(
            value,
            "capability.schema.table.compile-failed",
            "capability.schema.table.invalid",
            "capability table",
        )
    }

    /// Creates a capability table.
    #[must_use]
    pub fn new(records: impl IntoIterator<Item = CapabilityRecord>) -> Self {
        let mut table = BTreeMap::new();
        for record in records {
            table.entry(record.manifest_key.clone()).or_insert(record);
        }
        Self { records: table }
    }

    /// Ensures a capability permits an operation in the requested context.
    ///
    /// # Errors
    ///
    /// Returns [`CapabilityDenied`] when the capability is missing, unavailable, context-incompatible, or operation-denied.
    pub fn ensure_allowed(
        &self,
        manifest_key: &str,
        operation: PlatformOperation,
        context: PlatformContext,
    ) -> Result<(), CapabilityDenied> {
        let Some(record) = self.records.get(manifest_key) else {
            return Err(CapabilityDenied {
                manifest_key: manifest_key.into(),
                operation,
                diagnostic: PlatformDiagnostic::error(
                    "capability.missing",
                    format!("platform capability is not declared: {manifest_key}"),
                ),
            });
        };
        if context == PlatformContext::Plugin && !record.plugin_applicable {
            return Err(CapabilityDenied {
                manifest_key: manifest_key.into(),
                operation,
                diagnostic: PlatformDiagnostic::error(
                    "capability.plugin-incompatible",
                    format!(
                        "platform capability is not available in plugin context: {manifest_key}"
                    ),
                ),
            });
        }
        if context == PlatformContext::Desktop && !record.desktop_applicable {
            return Err(CapabilityDenied {
                manifest_key: manifest_key.into(),
                operation,
                diagnostic: PlatformDiagnostic::error(
                    "capability.desktop-incompatible",
                    format!(
                        "platform capability is not available in desktop context: {manifest_key}"
                    ),
                ),
            });
        }
        if record.runtime_availability != RuntimeAvailability::Runtime
            || record.denied_operations.contains(&operation)
            || !record.allowed_operations.contains(&operation)
        {
            return Err(CapabilityDenied {
                manifest_key: manifest_key.into(),
                operation,
                diagnostic: PlatformDiagnostic::error(
                    "capability.operation-denied",
                    format!("platform operation is denied by capability: {manifest_key}"),
                ),
            });
        }
        Ok(())
    }
}

fn capability_json_schema<T: JsonSchema>(
    rule: &'static str,
    label: &'static str,
) -> Result<serde_json::Value, PlatformDiagnostic> {
    serde_json::to_value(schemars::schema_for!(T)).map_err(|error| {
        PlatformDiagnostic::error(
            rule,
            format!("generated {label} could not be serialized: {error}"),
        )
    })
}

fn validate_capability_json<T: JsonSchema>(
    value: &serde_json::Value,
    compile_rule: &'static str,
    invalid_rule: &'static str,
    label: &'static str,
) -> Result<(), PlatformDiagnostic> {
    let schema = capability_json_schema::<T>(
        "capability.schema.generate-failed",
        "capability record schema",
    )?;
    let validator = jsonschema::Validator::new(&schema).map_err(|error| {
        PlatformDiagnostic::error(
            compile_rule,
            format!("generated {label} schema could not be compiled: {error}"),
        )
    })?;
    validator.validate(value).map_err(|error| {
        PlatformDiagnostic::error(
            invalid_rule,
            format!("{label} failed schema validation: {error}"),
        )
    })
}

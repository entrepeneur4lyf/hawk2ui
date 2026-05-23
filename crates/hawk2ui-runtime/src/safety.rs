//! Runtime safety guard for plugin and realtime contexts.

use serde::{Deserialize, Serialize};

/// Runtime execution context.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeExecutionContext {
    /// User-interface thread.
    UiThread,
    /// Audio callback thread.
    AudioThread,
    /// Non-realtime worker thread.
    WorkerThread,
}

/// Runtime operation guarded by context safety rules.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeGuardOperation {
    /// Script execution.
    ScriptExecution,
    /// Rendering or paint submission.
    Rendering,
    /// Filesystem access.
    Filesystem,
    /// Network access.
    Network,
    /// Blocking synchronization primitive.
    BlockingSynchronization,
    /// Plugin parameter automation.
    ParameterAutomation,
    /// Lock-free realtime data write.
    RealtimeDataWrite,
}

/// Runtime guard denial.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeGuardDenial {
    /// Stable runtime guard code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
    /// Execution context.
    pub context: RuntimeExecutionContext,
    /// Denied operation.
    pub operation: RuntimeGuardOperation,
}

/// Context-specific runtime safety guard.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeSafetyGuard {
    /// Execution context.
    pub context: RuntimeExecutionContext,
    /// Operations denied in the context.
    pub denied_operations: Vec<RuntimeGuardOperation>,
}

impl RuntimeSafetyGuard {
    /// Creates a guard for an execution context.
    #[must_use]
    pub fn for_context(context: RuntimeExecutionContext) -> Self {
        let denied_operations = if context == RuntimeExecutionContext::AudioThread {
            vec![
                RuntimeGuardOperation::ScriptExecution,
                RuntimeGuardOperation::Rendering,
                RuntimeGuardOperation::Filesystem,
                RuntimeGuardOperation::Network,
                RuntimeGuardOperation::BlockingSynchronization,
            ]
        } else {
            Vec::new()
        };
        Self {
            context,
            denied_operations,
        }
    }

    /// Ensures an operation is allowed in this guard context.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeGuardDenial`] when the operation is unsafe for the context.
    pub fn ensure_allowed(
        &self,
        operation: RuntimeGuardOperation,
    ) -> Result<(), RuntimeGuardDenial> {
        if self.denied_operations.contains(&operation) {
            Err(RuntimeGuardDenial {
                code: "runtime.audio-thread-operation-denied".into(),
                message: format!(
                    "runtime operation {operation:?} is denied in {:?}",
                    self.context
                ),
                context: self.context,
                operation,
            })
        } else {
            Ok(())
        }
    }
}

//! Test doubles for runtime adapter conformance.

use serde::{Deserialize, Serialize};

use crate::{
    TimerJob,
    script::{
        HostCallRecord, PromiseId, ScriptEngine, ScriptEngineError, ScriptEngineOperation,
        ScriptModuleRecord, StructuredValue,
    },
};

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

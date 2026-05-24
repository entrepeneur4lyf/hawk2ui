//! Plugin accessibility safety checks.

use serde::{Deserialize, Serialize};

/// Accessibility execution thread context.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum A11yThreadContext {
    /// Plugin editor UI thread.
    UiThread,
    /// Realtime audio thread.
    AudioThread,
}

/// Plugin accessibility operation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum A11yPluginOperation {
    /// Accessibility tree update.
    TreeUpdate,
    /// Accessibility focus update.
    FocusUpdate,
    /// Unstable host call.
    UnstableHostCall,
}

/// Plugin accessibility denial.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct A11yPluginDenial {
    /// Stable denial code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
    /// Thread context.
    pub context: A11yThreadContext,
    /// Denied operation.
    pub operation: A11yPluginOperation,
}

/// Plugin accessibility safety guard.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct A11yPluginGuard;

impl A11yPluginGuard {
    /// Ensures a plugin accessibility operation is safe.
    ///
    /// # Errors
    ///
    /// Returns [`A11yPluginDenial`] when the operation is unsafe for the context.
    pub fn ensure_allowed(
        &self,
        context: A11yThreadContext,
        operation: A11yPluginOperation,
    ) -> Result<(), A11yPluginDenial> {
        if context == A11yThreadContext::AudioThread {
            return Err(A11yPluginDenial {
                code: "a11y.plugin-audio-thread-denied".into(),
                message: "plugin accessibility work is denied on the audio thread".into(),
                context,
                operation,
            });
        }
        if operation == A11yPluginOperation::UnstableHostCall {
            return Err(A11yPluginDenial {
                code: "a11y.plugin-unstable-host-call-denied".into(),
                message: "unstable plugin accessibility host calls are denied".into(),
                context,
                operation,
            });
        }
        Ok(())
    }
}

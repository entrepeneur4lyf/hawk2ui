//! Runtime lifecycle hook registration.

use serde::{Deserialize, Serialize};

/// Runtime lifecycle phase.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum LifecyclePhase {
    /// Module initialization before first mount.
    Initialize,
    /// Component or application mount.
    Mount,
    /// Reactive update.
    Update,
    /// Temporary suspension that keeps state available for resume.
    Suspend,
    /// Resume after a suspension.
    Resume,
    /// Hot-reload patch application.
    HotReload,
    /// Error-boundary handling.
    ErrorBoundary,
    /// Ordered shutdown before teardown releases resources.
    Shutdown,
    /// Final teardown.
    Teardown,
}

/// Lifecycle hook exported by a script module.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LifecycleHook {
    /// Module that owns the hook.
    pub module_id: String,
    /// Phase that invokes the hook.
    pub phase: LifecyclePhase,
    /// Exported function name.
    pub export_name: String,
}

impl LifecycleHook {
    /// Creates a lifecycle hook record.
    #[must_use]
    pub fn new(
        module_id: impl Into<String>,
        phase: LifecyclePhase,
        export_name: impl Into<String>,
    ) -> Self {
        Self {
            module_id: module_id.into(),
            phase,
            export_name: export_name.into(),
        }
    }
}

/// Ordered lifecycle hook registry.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct LifecycleRegistry {
    hooks: Vec<LifecycleHook>,
}

impl LifecycleRegistry {
    /// Creates a lifecycle registry preserving insertion order.
    #[must_use]
    pub fn new(hooks: impl IntoIterator<Item = LifecycleHook>) -> Self {
        Self {
            hooks: hooks.into_iter().collect(),
        }
    }

    /// Returns all hooks for a phase in registration order.
    #[must_use]
    pub fn hooks_for(&self, phase: LifecyclePhase) -> Vec<&LifecycleHook> {
        self.hooks
            .iter()
            .filter(|hook| hook.phase == phase)
            .collect()
    }

    /// Returns all hooks in registration order.
    #[must_use]
    pub fn all(&self) -> &[LifecycleHook] {
        &self.hooks
    }
}

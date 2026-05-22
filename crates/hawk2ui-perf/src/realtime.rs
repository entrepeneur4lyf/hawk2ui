//! Realtime safety guards for plugin audio-thread contexts.

/// Runtime context being checked by a realtime guard.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RealtimeContext {
    /// Audio-thread context where blocking and allocation are denied.
    AudioThread,
    /// UI-thread context where realtime restrictions do not apply.
    UiThread,
}

/// Operation checked by a realtime guard.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RealtimeOperation {
    /// Heap allocation.
    Allocation,
    /// Blocking wait, mutex wait, sleep, or join.
    BlockingWait,
    /// Filesystem access.
    Filesystem,
    /// Network access.
    Network,
    /// Script runtime execution.
    Script,
    /// Rendering work.
    Rendering,
    /// Preallocated lock-free write.
    PreallocatedWrite,
}

/// Realtime guard configured for one runtime context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RealtimeGuard {
    context: RealtimeContext,
}

impl RealtimeGuard {
    /// Creates an audio-thread realtime guard.
    #[must_use]
    pub fn audio_thread() -> Self {
        Self {
            context: RealtimeContext::AudioThread,
        }
    }

    /// Creates a UI-thread guard.
    #[must_use]
    pub fn ui_thread() -> Self {
        Self {
            context: RealtimeContext::UiThread,
        }
    }

    /// Checks whether an operation is permitted in this guard context.
    ///
    /// # Errors
    ///
    /// Returns [`RealtimeGuardError`] when an audio-thread operation is unsafe.
    pub fn check(&self, operation: RealtimeOperation) -> Result<(), RealtimeGuardError> {
        if self.context == RealtimeContext::AudioThread && operation.denied_on_audio_thread() {
            Err(RealtimeGuardError::Denied {
                context: self.context,
                operation,
            })
        } else {
            Ok(())
        }
    }
}

impl RealtimeOperation {
    fn denied_on_audio_thread(self) -> bool {
        matches!(
            self,
            Self::Allocation
                | Self::BlockingWait
                | Self::Filesystem
                | Self::Network
                | Self::Script
                | Self::Rendering
        )
    }
}

/// Realtime guard validation error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RealtimeGuardError {
    /// Operation is denied in the current realtime context.
    Denied {
        /// Runtime context.
        context: RealtimeContext,
        /// Denied operation.
        operation: RealtimeOperation,
    },
}

//! Realtime safety guards for plugin audio-thread contexts.

use serde::{Deserialize, Serialize};

/// Runtime context being checked by a realtime guard.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RealtimeContext {
    /// Audio-thread context where blocking and allocation are denied.
    AudioThread,
    /// UI-thread context where realtime restrictions do not apply.
    UiThread,
}

/// Operation checked by a realtime guard.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
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

/// Lock policy enforced by a realtime guard.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RealtimeLockPolicy {
    /// Blocking locks, sleeps, joins, and waits are forbidden.
    NoBlockingLocks,
    /// Blocking synchronization is allowed outside realtime contexts.
    BlockingAllowedOutsideRealtime,
}

/// Realtime guard configured for one runtime context.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
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

    /// Returns the lock policy enforced by this guard.
    #[must_use]
    pub const fn lock_policy(&self) -> RealtimeLockPolicy {
        match self.context {
            RealtimeContext::AudioThread => RealtimeLockPolicy::NoBlockingLocks,
            RealtimeContext::UiThread => RealtimeLockPolicy::BlockingAllowedOutsideRealtime,
        }
    }

    /// Checks whether an operation is permitted in this guard context.
    ///
    /// # Errors
    ///
    /// Returns [`RealtimeGuardError`] when an audio-thread operation is unsafe.
    pub fn check(&self, operation: RealtimeOperation) -> Result<(), RealtimeGuardError> {
        if self.context == RealtimeContext::AudioThread && operation.is_denied_on_audio_thread() {
            Err(RealtimeGuardError::Denied {
                context: self.context,
                operation,
            })
        } else {
            Ok(())
        }
    }

    /// Audits operations and returns aggregate telemetry for release evidence.
    #[must_use]
    pub fn audit(
        &self,
        operations: impl IntoIterator<Item = RealtimeOperation>,
    ) -> RealtimeSafetyReport {
        let mut report = RealtimeSafetyReport::new(self.context);
        for operation in operations {
            report.record(operation);
        }
        report
    }
}

impl RealtimeOperation {
    /// Returns whether this operation is forbidden on the plugin audio thread.
    #[must_use]
    pub const fn is_denied_on_audio_thread(self) -> bool {
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
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RealtimeGuardError {
    /// Operation is denied in the current realtime context.
    Denied {
        /// Runtime context.
        context: RealtimeContext,
        /// Denied operation.
        operation: RealtimeOperation,
    },
}

/// Realtime safety audit telemetry.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct RealtimeSafetyTelemetry {
    /// Number of allocation attempts observed during the audit.
    pub allocation_attempts: usize,
    /// Number of blocking wait attempts observed during the audit.
    pub blocking_wait_attempts: usize,
    /// Number of denied operations observed during the audit.
    pub denied_attempts: usize,
}

/// Realtime safety audit report for release evidence.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RealtimeSafetyReport {
    /// Runtime context audited by the guard.
    pub context: RealtimeContext,
    /// Operations checked in audit order.
    pub checked_operations: Vec<RealtimeOperation>,
    /// Operations denied in audit order.
    pub denied_operations: Vec<RealtimeOperation>,
    /// Aggregated realtime safety counters.
    pub telemetry: RealtimeSafetyTelemetry,
}

impl RealtimeSafetyReport {
    fn new(context: RealtimeContext) -> Self {
        Self {
            context,
            checked_operations: Vec::new(),
            denied_operations: Vec::new(),
            telemetry: RealtimeSafetyTelemetry::default(),
        }
    }

    fn record(&mut self, operation: RealtimeOperation) {
        self.checked_operations.push(operation);
        if operation == RealtimeOperation::Allocation {
            self.telemetry.allocation_attempts += 1;
        }
        if operation == RealtimeOperation::BlockingWait {
            self.telemetry.blocking_wait_attempts += 1;
        }
        if self.context == RealtimeContext::AudioThread && operation.is_denied_on_audio_thread() {
            self.telemetry.denied_attempts += 1;
            self.denied_operations.push(operation);
        }
    }

    /// Returns the number of permitted audited operations.
    #[must_use]
    pub fn allowed_count(&self) -> usize {
        self.checked_operations
            .len()
            .saturating_sub(self.denied_operations.len())
    }

    /// Returns the number of denied audited operations.
    #[must_use]
    pub fn denied_count(&self) -> usize {
        self.denied_operations.len()
    }
}

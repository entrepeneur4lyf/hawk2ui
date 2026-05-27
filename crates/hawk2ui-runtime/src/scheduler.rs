//! Runtime work scheduler.

use std::collections::{BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::{RuntimeEvent, RuntimeSceneUpdate};

/// Timer job scheduled by the runtime.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TimerJob {
    /// Stable timer identifier.
    pub id: String,
    /// Delay in milliseconds.
    pub delay_ms: u64,
}

impl TimerJob {
    /// Creates a timer job.
    #[must_use]
    pub fn new(id: impl Into<String>, delay_ms: u64) -> Self {
        Self {
            id: id.into(),
            delay_ms,
        }
    }
}

/// Drained scheduler batch.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct RuntimeScheduleBatch {
    /// Script jobs.
    pub script_jobs: Vec<String>,
    /// Host callback names.
    pub host_callbacks: Vec<String>,
    /// UI events.
    pub ui_events: Vec<RuntimeEvent>,
    /// Coalesced render invalidation targets.
    pub render_invalidations: Vec<String>,
    /// Animation tick timestamps in milliseconds.
    pub animation_ticks: Vec<u64>,
    /// Timer jobs.
    pub timers: Vec<TimerJob>,
}

/// Scheduler error.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeScheduleError {
    /// Stable scheduler error code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
}

impl RuntimeScheduleError {
    fn shutdown_cancelled(cancelled_count: usize) -> Self {
        Self {
            code: "scheduler.shutdown-cancelled".into(),
            message: format!(
                "runtime shutdown cancelled {cancelled_count} pending scheduler item(s)"
            ),
        }
    }
}

/// Runtime scheduler with separate queues for each work class.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct RuntimeScheduler {
    script_jobs: VecDeque<String>,
    host_callbacks: VecDeque<String>,
    ui_events: VecDeque<RuntimeEvent>,
    render_invalidations: BTreeSet<String>,
    animation_ticks: VecDeque<u64>,
    timers: VecDeque<TimerJob>,
    shutting_down: bool,
}

impl RuntimeScheduler {
    /// Schedules a script job.
    pub fn schedule_script_job(&mut self, job_name: impl Into<String>) {
        self.script_jobs.push_back(job_name.into());
    }

    /// Schedules a host callback.
    pub fn schedule_host_callback(&mut self, callback_name: impl Into<String>) {
        self.host_callbacks.push_back(callback_name.into());
    }

    /// Schedules a UI event.
    pub fn schedule_ui_event(&mut self, event: RuntimeEvent) {
        self.ui_events.push_back(event);
    }

    /// Marks a render target invalid. Duplicate targets are coalesced.
    pub fn invalidate_render(&mut self, target: impl Into<String>) {
        self.render_invalidations.insert(target.into());
    }

    /// Schedules runtime work needed to present a scene update.
    pub fn schedule_scene_update(&mut self, update: &RuntimeSceneUpdate) {
        if !update.requires_repaint() {
            return;
        }
        for view_id in update.affected_view_ids() {
            self.invalidate_render(view_id.as_str());
        }
        self.schedule_host_callback("host.repaint.scene-dirty");
    }

    /// Schedules an animation tick timestamp.
    pub fn schedule_animation_tick(&mut self, timestamp_ms: u64) {
        self.animation_ticks.push_back(timestamp_ms);
    }

    /// Schedules a timer job.
    pub fn schedule_timer(&mut self, timer: TimerJob) {
        self.timers.push_back(timer);
    }

    /// Begins shutdown. Pending work is cancelled on the next drain.
    pub const fn begin_shutdown(&mut self) {
        self.shutting_down = true;
    }

    /// Returns whether all scheduler queues are empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pending_count() == 0
    }

    /// Drains pending work into a deterministic batch.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeScheduleError`] when shutdown cancels pending work.
    pub fn drain_batch(&mut self) -> Result<RuntimeScheduleBatch, RuntimeScheduleError> {
        if self.shutting_down && !self.is_empty() {
            let cancelled_count = self.pending_count();
            self.clear();
            return Err(RuntimeScheduleError::shutdown_cancelled(cancelled_count));
        }

        Ok(RuntimeScheduleBatch {
            script_jobs: self.script_jobs.drain(..).collect(),
            host_callbacks: self.host_callbacks.drain(..).collect(),
            ui_events: self.ui_events.drain(..).collect(),
            render_invalidations: std::mem::take(&mut self.render_invalidations)
                .into_iter()
                .collect(),
            animation_ticks: self.animation_ticks.drain(..).collect(),
            timers: self.timers.drain(..).collect(),
        })
    }

    fn pending_count(&self) -> usize {
        self.script_jobs.len()
            + self.host_callbacks.len()
            + self.ui_events.len()
            + self.render_invalidations.len()
            + self.animation_ticks.len()
            + self.timers.len()
    }

    fn clear(&mut self) {
        self.script_jobs.clear();
        self.host_callbacks.clear();
        self.ui_events.clear();
        self.render_invalidations.clear();
        self.animation_ticks.clear();
        self.timers.clear();
    }
}

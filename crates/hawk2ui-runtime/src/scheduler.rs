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
    /// Rich animation frame ticks with frame-rate policy metadata.
    pub animation_frames: Vec<AnimationFrameTick>,
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
    fn invalid_animation_policy(message: impl Into<String>) -> Self {
        Self {
            code: "scheduler.animation-policy.invalid".into(),
            message: message.into(),
        }
    }

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
    animation_frames: VecDeque<AnimationFrameTick>,
    timers: VecDeque<TimerJob>,
    shutting_down: bool,
}

/// Animation repaint cadence policy shared by desktop and plugin hosts.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AnimationCadencePolicy {
    max_frame_rate_hz: Option<u16>,
    reduced_motion: bool,
    reduced_rate_divisor: u16,
}

impl AnimationCadencePolicy {
    /// Creates an enabled animation policy capped to the supplied frame rate.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeScheduleError`] when the frame rate is zero.
    pub fn new(max_frame_rate_hz: u16) -> Result<Self, RuntimeScheduleError> {
        if max_frame_rate_hz == 0 {
            return Err(RuntimeScheduleError::invalid_animation_policy(
                "animation frame rate must be greater than zero",
            ));
        }
        Ok(Self {
            max_frame_rate_hz: Some(max_frame_rate_hz),
            reduced_motion: false,
            reduced_rate_divisor: 1,
        })
    }

    /// Creates a disabled animation policy.
    #[must_use]
    pub const fn disabled() -> Self {
        Self {
            max_frame_rate_hz: None,
            reduced_motion: false,
            reduced_rate_divisor: 1,
        }
    }

    /// Enables or disables automatic animation ticks for reduced-motion preferences.
    #[must_use]
    pub const fn with_reduced_motion(mut self, reduced_motion: bool) -> Self {
        self.reduced_motion = reduced_motion;
        self
    }

    /// Sets the divisor used for reduced-rate visual surfaces such as meters and analyzers.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeScheduleError`] when the divisor is zero.
    pub fn with_reduced_rate_divisor(
        mut self,
        reduced_rate_divisor: u16,
    ) -> Result<Self, RuntimeScheduleError> {
        if reduced_rate_divisor == 0 {
            return Err(RuntimeScheduleError::invalid_animation_policy(
                "reduced-rate divisor must be greater than zero",
            ));
        }
        self.reduced_rate_divisor = reduced_rate_divisor;
        Ok(self)
    }

    /// Returns the configured maximum frame rate.
    #[must_use]
    pub const fn max_frame_rate_hz(&self) -> Option<u16> {
        self.max_frame_rate_hz
    }

    /// Returns whether automatic animation ticks are suppressed by reduced motion.
    #[must_use]
    pub const fn reduced_motion(&self) -> bool {
        self.reduced_motion
    }

    /// Returns the reduced-rate divisor.
    #[must_use]
    pub const fn reduced_rate_divisor(&self) -> u16 {
        self.reduced_rate_divisor
    }

    fn primary_interval_ms(self) -> Option<u64> {
        self.max_frame_rate_hz
            .map(|rate| u64::from(1000_u16.div_ceil(rate)).max(1))
    }
}

impl Default for AnimationCadencePolicy {
    fn default() -> Self {
        Self::disabled()
    }
}

/// Deterministic animation frame tick emitted by the runtime cadence scheduler.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AnimationFrameTick {
    /// Monotonic frame index for this animation scheduler.
    pub frame_index: u64,
    /// Monotonic timestamp in milliseconds supplied by the host or headless stepper.
    pub timestamp_ms: u64,
    /// Whether reduced-rate surfaces are due on this tick.
    pub reduced_rate_due: bool,
}

impl AnimationFrameTick {
    /// Creates an animation frame tick.
    #[must_use]
    pub const fn new(frame_index: u64, timestamp_ms: u64, reduced_rate_due: bool) -> Self {
        Self {
            frame_index,
            timestamp_ms,
            reduced_rate_due,
        }
    }
}

/// Deterministic animation frame scheduler.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AnimationFrameScheduler {
    policy: AnimationCadencePolicy,
    next_frame_index: u64,
    last_primary_timestamp_ms: Option<u64>,
    last_reduced_rate_timestamp_ms: Option<u64>,
}

impl AnimationFrameScheduler {
    /// Creates an animation scheduler from a cadence policy.
    #[must_use]
    pub const fn new(policy: AnimationCadencePolicy) -> Self {
        Self {
            policy,
            next_frame_index: 0,
            last_primary_timestamp_ms: None,
            last_reduced_rate_timestamp_ms: None,
        }
    }

    /// Returns the active cadence policy.
    #[must_use]
    pub const fn policy(&self) -> AnimationCadencePolicy {
        self.policy
    }

    /// Returns whether the supplied timestamp should request a host redraw.
    #[must_use]
    pub fn should_request_frame(&self, timestamp_ms: u64) -> bool {
        self.policy
            .primary_interval_ms()
            .is_some_and(|interval| self.is_primary_due(timestamp_ms, interval))
            && !self.policy.reduced_motion
    }

    /// Advances the scheduler if the timestamp is due under the cadence policy.
    pub fn step_at(&mut self, timestamp_ms: u64) -> Option<AnimationFrameTick> {
        let interval = self.policy.primary_interval_ms()?;
        if self.policy.reduced_motion || !self.is_primary_due(timestamp_ms, interval) {
            return None;
        }
        Some(self.emit_tick(timestamp_ms, interval))
    }

    /// Emits a deterministic frame tick regardless of automatic cadence policy.
    pub fn force_step(&mut self, timestamp_ms: u64) -> AnimationFrameTick {
        let interval = self.policy.primary_interval_ms().unwrap_or(1);
        self.emit_tick(timestamp_ms, interval)
    }

    fn is_primary_due(&self, timestamp_ms: u64, interval: u64) -> bool {
        self.last_primary_timestamp_ms
            .is_none_or(|last| timestamp_ms >= last.saturating_add(interval))
    }

    fn emit_tick(&mut self, timestamp_ms: u64, interval: u64) -> AnimationFrameTick {
        let reduced_interval = interval.saturating_mul(u64::from(self.policy.reduced_rate_divisor));
        let reduced_rate_due = self
            .last_reduced_rate_timestamp_ms
            .is_none_or(|last| timestamp_ms >= last.saturating_add(reduced_interval));
        if reduced_rate_due {
            self.last_reduced_rate_timestamp_ms = Some(timestamp_ms);
        }
        let tick = AnimationFrameTick::new(self.next_frame_index, timestamp_ms, reduced_rate_due);
        self.next_frame_index = self.next_frame_index.saturating_add(1);
        self.last_primary_timestamp_ms = Some(timestamp_ms);
        tick
    }
}

impl Default for AnimationFrameScheduler {
    fn default() -> Self {
        Self::new(AnimationCadencePolicy::disabled())
    }
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

    /// Schedules an animation frame tick.
    pub fn schedule_animation_frame(&mut self, tick: AnimationFrameTick) {
        self.animation_frames.push_back(tick);
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
            animation_frames: self.animation_frames.drain(..).collect(),
            timers: self.timers.drain(..).collect(),
        })
    }

    fn pending_count(&self) -> usize {
        self.script_jobs.len()
            + self.host_callbacks.len()
            + self.ui_events.len()
            + self.render_invalidations.len()
            + self.animation_ticks.len()
            + self.animation_frames.len()
            + self.timers.len()
    }

    fn clear(&mut self) {
        self.script_jobs.clear();
        self.host_callbacks.clear();
        self.ui_events.clear();
        self.render_invalidations.clear();
        self.animation_ticks.clear();
        self.animation_frames.clear();
        self.timers.clear();
    }
}

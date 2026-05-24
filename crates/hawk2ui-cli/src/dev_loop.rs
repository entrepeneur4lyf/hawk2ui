//! Development loop orchestration records.

use serde::{Deserialize, Serialize};

use crate::CliDiagnostic;

/// Development loop event.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum DevLoopEvent {
    /// Watched file changed.
    FileChanged(String),
    /// Incremental rebuild was triggered.
    IncrementalRebuildTriggered,
    /// Validation passed.
    ValidationPassed,
    /// Validation failed.
    ValidationFailed,
    /// Native surface was reloaded.
    NativeSurfaceReloaded {
        /// Whether runtime state was preserved.
        preserve_state: bool,
    },
}

/// Recording file watcher.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RecordingWatcher {
    changed_files: Vec<String>,
}

impl RecordingWatcher {
    /// Creates a recording watcher from changed file paths.
    #[must_use]
    pub fn new(files: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            changed_files: files.into_iter().map(Into::into).collect(),
        }
    }
}

/// Recording runtime reload target.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct RecordingReloadTarget {
    reload_count: usize,
}

impl RecordingReloadTarget {
    fn reload(&mut self) {
        self.reload_count += 1;
    }
}

/// Development loop report.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DevLoopReport {
    /// Recorded events.
    pub events: Vec<DevLoopEvent>,
    /// Visible validation errors.
    pub visible_errors: Vec<CliDiagnostic>,
}

/// Recording development loop.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DevLoop {
    watcher: RecordingWatcher,
    reload_target: RecordingReloadTarget,
    preserve_state: bool,
    validation_failure_rule: Option<String>,
}

impl DevLoop {
    /// Creates a development loop.
    #[must_use]
    pub fn new(watcher: RecordingWatcher, reload_target: RecordingReloadTarget) -> Self {
        Self {
            watcher,
            reload_target,
            preserve_state: false,
            validation_failure_rule: None,
        }
    }

    /// Sets state preservation behavior.
    #[must_use]
    pub const fn preserve_state(mut self, preserve_state: bool) -> Self {
        self.preserve_state = preserve_state;
        self
    }

    /// Configures validation failure.
    #[must_use]
    pub fn validation_fails(mut self, rule: impl Into<String>) -> Self {
        self.validation_failure_rule = Some(rule.into());
        self
    }

    /// Runs one development loop iteration.
    ///
    /// # Errors
    ///
    /// Returns a string when no watched files are configured.
    pub fn run_once(&mut self) -> Result<DevLoopReport, String> {
        if self.watcher.changed_files.is_empty() {
            return Err("development loop has no watched files".into());
        }
        let mut events: Vec<_> = self
            .watcher
            .changed_files
            .iter()
            .cloned()
            .map(DevLoopEvent::FileChanged)
            .collect();
        events.push(DevLoopEvent::IncrementalRebuildTriggered);

        if let Some(rule) = &self.validation_failure_rule {
            events.push(DevLoopEvent::ValidationFailed);
            return Ok(DevLoopReport {
                events,
                visible_errors: vec![CliDiagnostic::error(rule.clone(), "validation failed")],
            });
        }

        events.push(DevLoopEvent::ValidationPassed);
        self.reload_target.reload();
        events.push(DevLoopEvent::NativeSurfaceReloaded {
            preserve_state: self.preserve_state,
        });
        Ok(DevLoopReport {
            events,
            visible_errors: Vec::new(),
        })
    }
}

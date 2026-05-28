//! Development loop orchestration records.

use std::{
    collections::{BTreeMap, BTreeSet},
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
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

/// Role of a watched project path in the development reload pipeline.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum DevWatchKind {
    /// Manifest, target, capability, or package declaration.
    Manifest,
    /// Style source that can be recompiled into a style patch.
    Style,
    /// Asset source that can invalidate renderer asset caches.
    Asset,
    /// Script/framework logic requiring script/runtime rebuild.
    Script,
    /// Runtime tree source with stable node identities.
    RuntimeTree,
}

/// One project path watched by `hawk2ui dev`.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct DevWatchedPath {
    path: String,
    kind: DevWatchKind,
}

impl DevWatchedPath {
    /// Creates a watched path classification record.
    #[must_use]
    pub fn new(path: impl Into<String>, kind: DevWatchKind) -> Self {
        Self {
            path: path.into(),
            kind,
        }
    }

    /// Returns the project-relative path.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the path's development reload role.
    #[must_use]
    pub const fn kind(&self) -> DevWatchKind {
        self.kind
    }
}

/// Development reload action selected for a changed file batch.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum DevPatchKind {
    /// Recompile styles and patch the active runtime style table.
    StylePatch,
    /// Reload assets and invalidate renderer asset caches.
    AssetPatch,
    /// Patch the retained runtime tree without restarting the process.
    RuntimeTreePatch,
    /// Rebuild scripts/runtime bindings while preserving compatible state.
    ScriptRebuild,
    /// Full rebuild is required before the active host can safely reload.
    FullRebuildRequired,
}

/// Classified development reload plan for a changed file batch.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DevPatchPlan {
    kind: DevPatchKind,
    changed_files: Vec<String>,
}

impl DevPatchPlan {
    /// Creates an empty reload plan.
    #[must_use]
    pub const fn new(kind: DevPatchKind) -> Self {
        Self {
            kind,
            changed_files: Vec::new(),
        }
    }

    /// Adds a changed file to the plan.
    #[must_use]
    pub fn with_changed_file(mut self, path: impl Into<String>) -> Self {
        self.changed_files.push(path.into());
        self
    }

    /// Returns the selected reload action.
    #[must_use]
    pub const fn kind(&self) -> DevPatchKind {
        self.kind
    }

    /// Returns the changed files in deterministic order.
    #[must_use]
    pub fn changed_files(&self) -> &[String] {
        &self.changed_files
    }
}

/// Classifies changed project files into patchable or full-rebuild dev actions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevChangeClassifier {
    watched_paths: BTreeMap<String, DevWatchKind>,
}

impl DevChangeClassifier {
    /// Creates a classifier from watched project paths.
    #[must_use]
    pub fn new(paths: impl IntoIterator<Item = DevWatchedPath>) -> Self {
        Self {
            watched_paths: paths
                .into_iter()
                .map(|path| (path.path, path.kind))
                .collect(),
        }
    }

    /// Classifies one coalesced changed file batch.
    #[must_use]
    pub fn classify(&self, files: impl IntoIterator<Item = impl Into<String>>) -> DevPatchPlan {
        let changed_files = files.into_iter().map(Into::into).collect::<BTreeSet<_>>();
        let mut selected: Option<DevPatchKind> = None;
        for file in &changed_files {
            let Some(kind) = self.watched_paths.get(file) else {
                selected = Some(DevPatchKind::FullRebuildRequired);
                break;
            };
            selected = Some(selected.map_or_else(
                || patch_kind_for_watch_kind(*kind),
                |current| select_patch_kind(current, *kind),
            ));
            if selected == Some(DevPatchKind::FullRebuildRequired) {
                break;
            }
        }
        changed_files.into_iter().fold(
            DevPatchPlan::new(selected.unwrap_or(DevPatchKind::FullRebuildRequired)),
            DevPatchPlan::with_changed_file,
        )
    }
}

/// Filesystem-backed project watcher.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileSystemWatcher {
    root: PathBuf,
    watched_files: Vec<String>,
    snapshots: BTreeMap<String, FileSnapshot>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileSnapshot {
    exists: bool,
    content_hash: u64,
}

impl FileSystemWatcher {
    /// Creates a watcher rooted at a project directory.
    #[must_use]
    pub fn new(
        root: impl Into<PathBuf>,
        files: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        let mut watched_files = files.into_iter().map(Into::into).collect::<Vec<_>>();
        watched_files.sort();
        watched_files.dedup();
        Self {
            root: root.into(),
            watched_files,
            snapshots: BTreeMap::new(),
        }
    }

    /// Returns files whose current filesystem snapshot differs from the previous poll.
    #[must_use]
    pub fn changed_files(&mut self) -> Vec<String> {
        let mut changed = Vec::new();
        for file in &self.watched_files {
            let snapshot = self.snapshot(file);
            if self.snapshots.get(file) != Some(&snapshot) {
                changed.push(file.clone());
            }
            self.snapshots.insert(file.clone(), snapshot);
        }
        changed
    }

    /// Replaces the watched project file set while preserving snapshots for unchanged files.
    pub fn replace_watched_files(&mut self, files: impl IntoIterator<Item = impl Into<String>>) {
        let mut watched_files = files.into_iter().map(Into::into).collect::<Vec<_>>();
        watched_files.sort();
        watched_files.dedup();
        self.snapshots
            .retain(|file, _snapshot| watched_files.binary_search(file).is_ok());
        self.watched_files = watched_files;
    }

    fn snapshot(&self, file: &str) -> FileSnapshot {
        let path = self.root.join(file);
        match std::fs::read(path) {
            Ok(bytes) => FileSnapshot {
                exists: true,
                content_hash: hash_bytes(&bytes),
            },
            Err(_) => FileSnapshot {
                exists: false,
                content_hash: 0,
            },
        }
    }
}

/// Error reported by the native filesystem watcher.
#[derive(Debug)]
pub struct DevWatcherError {
    message: String,
}

impl DevWatcherError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Returns the watcher error message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for DevWatcherError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for DevWatcherError {}

impl From<notify::Error> for DevWatcherError {
    fn from(error: notify::Error) -> Self {
        Self::new(format!("notify watcher event failed: {error}"))
    }
}

/// Native `notify`-backed filesystem watcher with hash validation and debounce.
pub struct NotifyFileSystemWatcher {
    root: PathBuf,
    watched_files: BTreeSet<String>,
    receiver: Receiver<notify::Result<Event>>,
    _watcher: RecommendedWatcher,
    debounce: Duration,
    snapshot_watcher: FileSystemWatcher,
}

impl NotifyFileSystemWatcher {
    /// Starts a native filesystem watcher for project-relative files.
    ///
    /// # Errors
    ///
    /// Returns [`DevWatcherError`] when the native watcher cannot start or a watched directory
    /// cannot be registered.
    pub fn new(
        root: impl Into<PathBuf>,
        files: impl IntoIterator<Item = impl Into<String>>,
        debounce: Duration,
    ) -> Result<Self, DevWatcherError> {
        let root = root.into();
        let watched_files = normalized_file_set(files);
        let (sender, receiver) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(move |event| {
            let _ = sender.send(event);
        })
        .map_err(|error| DevWatcherError::new(format!("notify watcher failed: {error}")))?;

        for directory in watched_parent_directories(&root, &watched_files) {
            watcher
                .watch(&directory, RecursiveMode::NonRecursive)
                .map_err(|error| {
                    DevWatcherError::new(format!(
                        "failed to watch {}: {error}",
                        directory.display()
                    ))
                })?;
        }

        let mut snapshot_watcher = FileSystemWatcher::new(&root, watched_files.iter().cloned());
        let _ = snapshot_watcher.changed_files();

        Ok(Self {
            root,
            watched_files,
            receiver,
            _watcher: watcher,
            debounce,
            snapshot_watcher,
        })
    }

    /// Waits for a debounced changed file batch.
    ///
    /// # Errors
    ///
    /// Returns [`DevWatcherError`] when the native watcher reports an event error.
    pub fn next_changed_files(
        &mut self,
        timeout: Duration,
    ) -> Result<Vec<String>, DevWatcherError> {
        let Ok(first_event) = self.receiver.recv_timeout(timeout) else {
            return Ok(Vec::new());
        };
        let mut changed = self.changed_files_from_event(first_event?)?;
        let debounce_deadline = Instant::now() + self.debounce;
        while let Some(remaining) = debounce_deadline.checked_duration_since(Instant::now()) {
            if remaining.is_zero() {
                break;
            }
            match self.receiver.recv_timeout(remaining) {
                Ok(event) => changed.extend(self.changed_files_from_event(event?)?),
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(DevWatcherError::new("notify watcher channel disconnected"));
                }
            }
        }

        if changed.is_empty() {
            return Ok(Vec::new());
        }
        Ok(self
            .snapshot_watcher
            .changed_files()
            .into_iter()
            .filter(|file| changed.contains(file))
            .collect())
    }

    fn changed_files_from_event(&self, event: Event) -> Result<BTreeSet<String>, DevWatcherError> {
        let mut changed = BTreeSet::new();
        for path in event.paths {
            if let Some(relative) = project_relative_path(&self.root, &path)?
                && self.watched_files.contains(&relative)
            {
                changed.insert(relative);
            }
        }
        Ok(changed)
    }
}

/// Recording runtime reload target.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct RecordingReloadTarget {
    reload_count: usize,
}

fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

fn normalized_file_set(files: impl IntoIterator<Item = impl Into<String>>) -> BTreeSet<String> {
    files
        .into_iter()
        .map(Into::into)
        .map(|file| file.replace('\\', "/"))
        .collect()
}

fn watched_parent_directories(root: &Path, files: &BTreeSet<String>) -> BTreeSet<PathBuf> {
    files
        .iter()
        .map(|file| {
            let path = root.join(file);
            path.parent()
                .map_or_else(|| root.to_path_buf(), Path::to_path_buf)
        })
        .collect()
}

fn project_relative_path(root: &Path, path: &Path) -> Result<Option<String>, DevWatcherError> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let relative = absolute.strip_prefix(root).map_err(|error| {
        DevWatcherError::new(format!(
            "notify event path {} is outside project root {}: {error}",
            absolute.display(),
            root.display()
        ))
    })?;
    let Some(relative) = relative.to_str() else {
        return Ok(None);
    };
    Ok(Some(relative.replace('\\', "/")))
}

fn select_patch_kind(current: DevPatchKind, changed: DevWatchKind) -> DevPatchKind {
    match (current, changed) {
        (DevPatchKind::FullRebuildRequired, _) | (_, DevWatchKind::Manifest) => {
            DevPatchKind::FullRebuildRequired
        }
        (DevPatchKind::ScriptRebuild, _) | (_, DevWatchKind::Script) => DevPatchKind::ScriptRebuild,
        (DevPatchKind::RuntimeTreePatch, _) | (_, DevWatchKind::RuntimeTree) => {
            DevPatchKind::RuntimeTreePatch
        }
        (DevPatchKind::AssetPatch, DevWatchKind::Style)
        | (DevPatchKind::StylePatch, DevWatchKind::Asset) => DevPatchKind::RuntimeTreePatch,
        (_, DevWatchKind::Asset) => DevPatchKind::AssetPatch,
        (_, DevWatchKind::Style) => DevPatchKind::StylePatch,
    }
}

fn patch_kind_for_watch_kind(kind: DevWatchKind) -> DevPatchKind {
    match kind {
        DevWatchKind::Manifest => DevPatchKind::FullRebuildRequired,
        DevWatchKind::Style => DevPatchKind::StylePatch,
        DevWatchKind::Asset => DevPatchKind::AssetPatch,
        DevWatchKind::Script => DevPatchKind::ScriptRebuild,
        DevWatchKind::RuntimeTree => DevPatchKind::RuntimeTreePatch,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filesystem_watcher_reports_changed_project_files_after_snapshot() {
        let root = std::env::temp_dir().join(format!("hawk2ui-dev-loop-fs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).expect("test source dir creates");
        std::fs::write(root.join("src/main.ts"), "export const value = 1;\n")
            .expect("test source writes");

        let mut watcher = FileSystemWatcher::new(&root, ["src/main.ts"]);
        assert_eq!(watcher.changed_files(), vec!["src/main.ts".to_string()]);
        assert!(watcher.changed_files().is_empty());

        std::fs::write(root.join("src/main.ts"), "export const value = 2;\n")
            .expect("test source rewrites");

        assert_eq!(watcher.changed_files(), vec!["src/main.ts".to_string()]);
        std::fs::remove_dir_all(root).expect("test source dir cleans up");
    }

    #[test]
    fn change_classifier_maps_manifest_style_asset_script_and_tree_patches() {
        let classifier = DevChangeClassifier::new([
            DevWatchedPath::new("manifest.hawk.toml", DevWatchKind::Manifest),
            DevWatchedPath::new("styles/main.hawk.css", DevWatchKind::Style),
            DevWatchedPath::new("assets/logo.svg", DevWatchKind::Asset),
            DevWatchedPath::new("src/bootstrap.ts", DevWatchKind::Script),
            DevWatchedPath::new("src/main.ts", DevWatchKind::RuntimeTree),
        ]);

        assert_eq!(
            classifier.classify(["styles/main.hawk.css"]),
            DevPatchPlan::new(DevPatchKind::StylePatch).with_changed_file("styles/main.hawk.css")
        );
        assert_eq!(
            classifier.classify(["assets/logo.svg"]),
            DevPatchPlan::new(DevPatchKind::AssetPatch).with_changed_file("assets/logo.svg")
        );
        assert_eq!(
            classifier.classify(["src/main.ts"]),
            DevPatchPlan::new(DevPatchKind::RuntimeTreePatch).with_changed_file("src/main.ts")
        );
        assert_eq!(
            classifier.classify(["manifest.hawk.toml", "styles/main.hawk.css"]),
            DevPatchPlan::new(DevPatchKind::FullRebuildRequired)
                .with_changed_file("manifest.hawk.toml")
                .with_changed_file("styles/main.hawk.css")
        );
        assert_eq!(
            classifier.classify(["src/bootstrap.ts"]),
            DevPatchPlan::new(DevPatchKind::ScriptRebuild).with_changed_file("src/bootstrap.ts")
        );
    }

    #[test]
    fn notify_watcher_reports_debounced_project_file_changes() {
        let root =
            std::env::temp_dir().join(format!("hawk2ui-dev-loop-notify-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).expect("test source dir creates");
        std::fs::write(root.join("src/main.ts"), "export const value = 1;\n")
            .expect("test source writes");

        let mut watcher = NotifyFileSystemWatcher::new(
            &root,
            ["src/main.ts"],
            std::time::Duration::from_millis(25),
        )
        .expect("notify watcher starts");
        std::fs::write(root.join("src/main.ts"), "export const value = 2;\n")
            .expect("test source rewrites");

        let changed = watcher
            .next_changed_files(std::time::Duration::from_secs(2))
            .expect("notify watcher polls");
        assert_eq!(changed, vec!["src/main.ts".to_string()]);
        std::fs::remove_dir_all(root).expect("test source dir cleans up");
    }
}

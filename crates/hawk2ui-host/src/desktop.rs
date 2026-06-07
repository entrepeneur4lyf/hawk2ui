//! Desktop host lifecycle records.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{RepaintRequest, SurfaceClipboardRequest, SurfaceMetrics};

/// Clipboard capability exposed by a desktop host.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ClipboardCapability {
    /// Clipboard unavailable.
    None,
    /// Read-only clipboard access.
    Read,
    /// Write-only clipboard access.
    Write,
    /// Read/write clipboard access.
    ReadWrite,
}

/// Desktop window mode.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum WindowMode {
    /// Normal restored window.
    Normal,
    /// Minimized window.
    Minimized,
    /// Maximized window.
    Maximized,
    /// Fullscreen window.
    Fullscreen,
}

/// Desktop window creation configuration.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DesktopWindowConfig {
    /// Window title.
    pub title: String,
    /// Initial metrics.
    pub metrics: SurfaceMetrics,
    /// Clipboard capability.
    pub clipboard: ClipboardCapability,
}

impl DesktopWindowConfig {
    /// Creates desktop window configuration.
    #[must_use]
    pub fn new(title: impl Into<String>, metrics: SurfaceMetrics) -> Self {
        Self {
            title: title.into(),
            metrics,
            clipboard: ClipboardCapability::None,
        }
    }

    /// Sets clipboard capability.
    #[must_use]
    pub const fn with_clipboard(mut self, clipboard: ClipboardCapability) -> Self {
        self.clipboard = clipboard;
        self
    }
}

/// Keyboard input record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct KeyboardInput {
    /// Physical or logical key code.
    pub key: String,
    /// Whether the key is pressed.
    pub pressed: bool,
}

impl KeyboardInput {
    /// Creates a keyboard input record.
    #[must_use]
    pub fn new(key: impl Into<String>, pressed: bool) -> Self {
        Self {
            key: key.into(),
            pressed,
        }
    }
}

/// Pointer input record.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PointerInput {
    /// Logical x coordinate.
    pub x: f64,
    /// Logical y coordinate.
    pub y: f64,
    /// Pointer button or pointer action label.
    pub button: String,
}

impl PointerInput {
    /// Creates a pointer input record.
    #[must_use]
    pub fn new(x: f64, y: f64, button: impl Into<String>) -> Self {
        Self {
            x,
            y,
            button: button.into(),
        }
    }
}

/// Severity level for a native desktop message dialog.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum DesktopDialogLevel {
    /// Informational message.
    Info,
    /// Warning message.
    Warning,
    /// Error message.
    Error,
}

/// File filter exposed to a native desktop file dialog.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DesktopDialogFileFilter {
    /// Human-readable filter name shown by the platform picker.
    pub name: String,
    /// File extensions without a leading dot.
    pub extensions: Vec<String>,
}

impl DesktopDialogFileFilter {
    /// Creates a native file-dialog filter.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        extensions: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            name: name.into(),
            extensions: extensions.into_iter().map(Into::into).collect(),
        }
    }
}

/// Native desktop dialog request.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum DesktopDialogRequest {
    /// Show a native message dialog.
    Message {
        /// Dialog title.
        title: String,
        /// Dialog body.
        message: String,
        /// Message severity.
        level: DesktopDialogLevel,
    },
    /// Open a native single-file picker.
    OpenFile {
        /// Dialog title.
        title: String,
        /// Optional starting directory.
        directory: Option<PathBuf>,
        /// File filters to expose to the platform picker.
        filters: Vec<DesktopDialogFileFilter>,
    },
    /// Open a native save-file picker.
    SaveFile {
        /// Dialog title.
        title: String,
        /// Optional starting directory.
        directory: Option<PathBuf>,
        /// Optional initial file name.
        file_name: Option<String>,
        /// File filters to expose to the platform picker.
        filters: Vec<DesktopDialogFileFilter>,
    },
}

/// Native desktop dialog response.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum DesktopDialogResponse {
    /// Message dialog was acknowledged.
    Acknowledged,
    /// User selected a file from an open-file picker.
    SelectedFile(PathBuf),
    /// User selected a file from a save-file picker.
    SavedFile(PathBuf),
    /// User cancelled the dialog.
    Cancelled,
}

/// Desktop host event.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum DesktopHostEvent {
    /// Window was created.
    WindowCreated(DesktopWindowConfig),
    /// Close was requested.
    CloseRequested(String),
    /// Window mode changed.
    ModeChanged(WindowMode),
    /// Focus changed.
    FocusChanged(bool),
    /// Keyboard input.
    KeyboardInput(KeyboardInput),
    /// Pointer input.
    PointerInput(PointerInput),
    /// Input method editor event.
    ImeInput(String),
    /// Native file drag/drop event.
    FileDragDrop(String),
    /// Native window occlusion changed.
    WindowOcclusionChanged(bool),
    /// Clipboard capability changed.
    ClipboardCapabilityChanged(ClipboardCapability),
    /// DPI scale changed.
    DpiChanged(f64),
    /// Renderer target must be recreated.
    RendererTargetRecreateRequested,
    /// Explicit repaint was requested.
    RepaintRequested(RepaintRequest),
    /// Surface metrics changed.
    Resized(SurfaceMetrics),
    /// Clipboard operation was requested.
    ClipboardRequested(SurfaceClipboardRequest),
    /// Native desktop dialog was requested.
    DialogRequested(DesktopDialogRequest),
    /// Frame was presented to the host surface.
    FramePresented {
        /// Monotonic frame identifier.
        frame_id: u64,
        /// Surface metrics used for presentation.
        metrics: SurfaceMetrics,
    },
}

/// Desktop host adapter contract.
pub trait DesktopHostAdapter {
    /// Returns desktop window configuration.
    fn config(&self) -> &DesktopWindowConfig;

    /// Returns current surface metrics.
    fn metrics(&self) -> SurfaceMetrics;

    /// Requests minimized state.
    fn request_minimize(&mut self, minimized: bool);

    /// Requests maximized state.
    fn request_maximize(&mut self, maximized: bool);

    /// Requests fullscreen state.
    fn request_fullscreen(&mut self, fullscreen: bool);

    /// Requests window close.
    fn request_close(&mut self, reason: impl Into<String>);

    /// Sets focus.
    fn set_focus(&mut self, focused: bool);

    /// Records keyboard input.
    fn keyboard_input(&mut self, input: KeyboardInput);

    /// Records pointer input.
    fn pointer_input(&mut self, input: PointerInput);

    /// Updates clipboard capability.
    fn clipboard_available(&mut self, capability: ClipboardCapability);

    /// Updates DPI scale and requests renderer target recreation.
    fn dpi_changed(&mut self, scale_factor: f64);
}

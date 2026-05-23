//! Plugin editor embedding records.

use serde::{Deserialize, Serialize};

/// Plugin editor kind.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum EditorKind {
    /// Generated editor from plugin parameter metadata.
    Generated,
    /// Custom editor supplied by the application.
    Custom,
}

/// Plugin editor size and scale.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct PluginEditorSize {
    /// Logical width in host points.
    pub logical_width: f64,
    /// Logical height in host points.
    pub logical_height: f64,
    /// Device scale factor.
    pub scale_factor: f64,
}

impl PluginEditorSize {
    /// Creates plugin editor size.
    #[must_use]
    pub const fn new(logical_width: f64, logical_height: f64, scale_factor: f64) -> Self {
        Self {
            logical_width,
            logical_height,
            scale_factor,
        }
    }

    /// Returns physical pixel size rounded and clamped.
    #[must_use]
    pub fn physical_size(&self) -> (u32, u32) {
        (
            scaled_physical_dimension(self.logical_width, self.scale_factor),
            scaled_physical_dimension(self.logical_height, self.scale_factor),
        )
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn scaled_physical_dimension(logical: f64, scale_factor: f64) -> u32 {
    let scaled = (logical.max(0.0) * scale_factor.max(0.0)).round();
    if !scaled.is_finite() {
        0
    } else if scaled >= f64::from(u32::MAX) {
        u32::MAX
    } else {
        scaled as u32
    }
}

/// Host parent handle for embedded plugin editors.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EditorParent {
    /// Opaque host parent identifier.
    pub id: String,
}

impl EditorParent {
    /// Creates an opaque editor parent handle.
    #[must_use]
    pub fn opaque(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

/// Plugin editor record.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PluginEditor {
    /// Stable editor identifier.
    pub id: String,
    /// Editor kind.
    pub kind: EditorKind,
    /// Initial editor size.
    pub initial_size: PluginEditorSize,
}

impl PluginEditor {
    /// Creates a generated editor record.
    #[must_use]
    pub fn generated(id: impl Into<String>, initial_size: PluginEditorSize) -> Self {
        Self {
            id: id.into(),
            kind: EditorKind::Generated,
            initial_size,
        }
    }

    /// Creates a custom editor record.
    #[must_use]
    pub fn custom(id: impl Into<String>, initial_size: PluginEditorSize) -> Self {
        Self {
            id: id.into(),
            kind: EditorKind::Custom,
            initial_size,
        }
    }
}

/// Plugin editor lifecycle event.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum EditorEvent {
    /// Editor was created.
    Created(PluginEditor),
    /// Host parent was attached.
    ParentAttached(EditorParent),
    /// Initial editor size was reported to the host.
    InitialSizeReported(PluginEditorSize),
    /// DPI scale changed.
    DpiChanged(f64),
    /// Host resized the embedded editor.
    HostResized(PluginEditorSize),
    /// Repaint was requested.
    RepaintRequested(String),
    /// Editor was destroyed.
    Destroyed(String),
}

/// Plugin editor lifecycle recorder.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PluginEditorLifecycle {
    editor: PluginEditor,
    parent: Option<EditorParent>,
    current_size: PluginEditorSize,
    events: Vec<EditorEvent>,
}

impl PluginEditorLifecycle {
    /// Creates an editor lifecycle and records creation.
    #[must_use]
    pub fn create(editor: PluginEditor) -> Self {
        Self {
            current_size: editor.initial_size,
            events: vec![EditorEvent::Created(editor.clone())],
            editor,
            parent: None,
        }
    }

    /// Attaches a host parent surface.
    pub fn attach_parent(&mut self, parent: EditorParent) {
        self.parent = Some(parent.clone());
        self.events.push(EditorEvent::ParentAttached(parent));
    }

    /// Reports the initial size to the host.
    pub fn report_initial_size(&mut self) {
        self.events
            .push(EditorEvent::InitialSizeReported(self.editor.initial_size));
    }

    /// Applies a DPI change.
    pub fn dpi_changed(&mut self, scale_factor: f64) {
        self.current_size.scale_factor = scale_factor;
        self.events.push(EditorEvent::DpiChanged(scale_factor));
    }

    /// Applies a host-driven resize.
    pub fn host_resize(&mut self, size: PluginEditorSize) {
        self.current_size = size;
        self.events.push(EditorEvent::HostResized(size));
    }

    /// Requests repaint.
    pub fn request_repaint(&mut self, reason: impl Into<String>) {
        self.events
            .push(EditorEvent::RepaintRequested(reason.into()));
    }

    /// Destroys the editor.
    pub fn destroy(&mut self, reason: impl Into<String>) {
        self.events.push(EditorEvent::Destroyed(reason.into()));
    }

    /// Returns current size.
    #[must_use]
    pub const fn current_size(&self) -> PluginEditorSize {
        self.current_size
    }

    /// Returns lifecycle events.
    #[must_use]
    pub fn events(&self) -> &[EditorEvent] {
        &self.events
    }

    /// Returns the attached host parent.
    #[must_use]
    pub const fn parent(&self) -> Option<&EditorParent> {
        self.parent.as_ref()
    }

    /// Plugin editors are embedded and never assume top-level window ownership.
    #[must_use]
    pub const fn assumes_top_level_window_ownership(&self) -> bool {
        false
    }
}

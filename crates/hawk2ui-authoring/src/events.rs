//! Native event binding records for authoring output.

use crate::ElementId;

/// Pointer event kind independent of browser event object names.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointerEventKind {
    /// Pointer press.
    Press,
    /// Pointer release.
    Release,
    /// Pointer move.
    Move,
    /// Pointer drag.
    Drag,
}

impl PointerEventKind {
    const fn stable_key(self) -> &'static str {
        match self {
            Self::Press => "pointer.press",
            Self::Release => "pointer.release",
            Self::Move => "pointer.move",
            Self::Drag => "pointer.drag",
        }
    }
}

/// Keyboard event kind independent of browser event object names.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyboardEventKind {
    /// Key-down transition.
    KeyDown,
    /// Key-up transition.
    KeyUp,
    /// Text input commit.
    TextInput,
}

impl KeyboardEventKind {
    const fn stable_key(self) -> &'static str {
        match self {
            Self::KeyDown => "keyboard.key-down",
            Self::KeyUp => "keyboard.key-up",
            Self::TextInput => "keyboard.text-input",
        }
    }
}

/// Focus event kind independent of browser event object names.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FocusEventKind {
    /// Focus entered target.
    FocusIn,
    /// Focus left target.
    FocusOut,
}

impl FocusEventKind {
    const fn stable_key(self) -> &'static str {
        match self {
            Self::FocusIn => "focus.focus-in",
            Self::FocusOut => "focus.focus-out",
        }
    }
}

/// Input event kind independent of browser event object names.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputEventKind {
    /// Value changed.
    ValueChanged,
    /// Value committed.
    ValueCommitted,
}

impl InputEventKind {
    const fn stable_key(self) -> &'static str {
        match self {
            Self::ValueChanged => "input.value-changed",
            Self::ValueCommitted => "input.value-committed",
        }
    }
}

/// Lifecycle event kind independent of framework lifecycle method names.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleEventKind {
    /// Node mounted.
    Mounted,
    /// Node temporarily suspended while retaining state.
    Suspended,
    /// Node resumed after suspension.
    Resumed,
    /// Node updated after a hot-reload patch.
    HotReloaded,
    /// Node entered an error boundary.
    ErrorBoundary,
    /// Node is shutting down before teardown.
    Shutdown,
    /// Node unmounted.
    Unmounted,
}

impl LifecycleEventKind {
    const fn stable_key(self) -> &'static str {
        match self {
            Self::Mounted => "lifecycle.mounted",
            Self::Suspended => "lifecycle.suspended",
            Self::Resumed => "lifecycle.resumed",
            Self::HotReloaded => "lifecycle.hot-reloaded",
            Self::ErrorBoundary => "lifecycle.error-boundary",
            Self::Shutdown => "lifecycle.shutdown",
            Self::Unmounted => "lifecycle.unmounted",
        }
    }
}

/// Native event kind emitted by authoring and framework adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EventKind {
    /// Pointer event.
    Pointer(PointerEventKind),
    /// Keyboard event.
    Keyboard(KeyboardEventKind),
    /// Focus event.
    Focus(FocusEventKind),
    /// Input event.
    Input(InputEventKind),
    /// Resize event.
    Resize,
    /// Lifecycle event.
    Lifecycle(LifecycleEventKind),
    /// Custom component event by stable author-defined event name.
    CustomComponent(String),
    /// Plugin parameter event by stable parameter identifier.
    PluginParameter(String),
}

impl EventKind {
    /// Returns the stable event key used by diagnostics, adapter contracts, and tests.
    #[must_use]
    pub fn stable_key(&self) -> String {
        match self {
            Self::Pointer(kind) => kind.stable_key().to_string(),
            Self::Keyboard(kind) => kind.stable_key().to_string(),
            Self::Focus(kind) => kind.stable_key().to_string(),
            Self::Input(kind) => kind.stable_key().to_string(),
            Self::Resize => "resize".to_string(),
            Self::Lifecycle(kind) => kind.stable_key().to_string(),
            Self::CustomComponent(name) => format!("component.{name}"),
            Self::PluginParameter(parameter) => format!("plugin-parameter.{parameter}"),
        }
    }
}

impl std::str::FromStr for EventKind {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "pointer.press" => Ok(Self::Pointer(PointerEventKind::Press)),
            "pointer.release" => Ok(Self::Pointer(PointerEventKind::Release)),
            "pointer.move" => Ok(Self::Pointer(PointerEventKind::Move)),
            "pointer.drag" => Ok(Self::Pointer(PointerEventKind::Drag)),
            "keyboard.key-down" => Ok(Self::Keyboard(KeyboardEventKind::KeyDown)),
            "keyboard.key-up" => Ok(Self::Keyboard(KeyboardEventKind::KeyUp)),
            "keyboard.text-input" => Ok(Self::Keyboard(KeyboardEventKind::TextInput)),
            "focus.focus-in" => Ok(Self::Focus(FocusEventKind::FocusIn)),
            "focus.focus-out" => Ok(Self::Focus(FocusEventKind::FocusOut)),
            "input.value-changed" => Ok(Self::Input(InputEventKind::ValueChanged)),
            "input.value-committed" => Ok(Self::Input(InputEventKind::ValueCommitted)),
            "resize" => Ok(Self::Resize),
            "lifecycle.mounted" => Ok(Self::Lifecycle(LifecycleEventKind::Mounted)),
            "lifecycle.suspended" => Ok(Self::Lifecycle(LifecycleEventKind::Suspended)),
            "lifecycle.resumed" => Ok(Self::Lifecycle(LifecycleEventKind::Resumed)),
            "lifecycle.hot-reloaded" => Ok(Self::Lifecycle(LifecycleEventKind::HotReloaded)),
            "lifecycle.error-boundary" => Ok(Self::Lifecycle(LifecycleEventKind::ErrorBoundary)),
            "lifecycle.shutdown" => Ok(Self::Lifecycle(LifecycleEventKind::Shutdown)),
            "lifecycle.unmounted" => Ok(Self::Lifecycle(LifecycleEventKind::Unmounted)),
            _ => value
                .strip_prefix("component.")
                .filter(|name| !name.is_empty())
                .map(|name| Self::CustomComponent(name.to_string()))
                .or_else(|| {
                    value
                        .strip_prefix("plugin-parameter.")
                        .filter(|name| !name.is_empty())
                        .map(|name| Self::PluginParameter(name.to_string()))
                })
                .ok_or(()),
        }
    }
}

/// Stable handler reference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandlerRef(String);

impl HandlerRef {
    /// Creates a handler reference.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the handler reference as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Payload fields requested by an event binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventPayloadField {
    /// Pointer or geometry position.
    Position,
    /// Movement delta.
    Delta,
    /// Text or control value.
    Value,
    /// Keyboard key identifier.
    Key,
}

/// Native event binding record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventBinding {
    target: ElementId,
    event: EventKind,
    handler: HandlerRef,
    payload_fields: Vec<EventPayloadField>,
}

impl EventBinding {
    /// Creates an event binding.
    #[must_use]
    pub fn new(target: ElementId, event: EventKind, handler: HandlerRef) -> Self {
        Self {
            target,
            event,
            handler,
            payload_fields: Vec::new(),
        }
    }

    /// Adds a requested payload field if it has not already been requested.
    #[must_use]
    pub fn with_payload(mut self, field: EventPayloadField) -> Self {
        if !self.payload_fields.contains(&field) {
            self.payload_fields.push(field);
        }
        self
    }

    /// Returns the target element identifier.
    #[must_use]
    pub const fn target(&self) -> &ElementId {
        &self.target
    }

    /// Returns the event kind.
    #[must_use]
    pub const fn event(&self) -> &EventKind {
        &self.event
    }

    /// Returns the handler reference.
    #[must_use]
    pub const fn handler(&self) -> &HandlerRef {
        &self.handler
    }

    /// Returns requested payload fields in author-declared order.
    #[must_use]
    pub fn payload_fields(&self) -> &[EventPayloadField] {
        &self.payload_fields
    }
}

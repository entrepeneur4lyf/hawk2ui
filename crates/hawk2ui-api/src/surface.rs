//! Host surface API contracts.
//!
//! ## Stability
//!
//! Surface records are source-compatible within a major crate version. Input
//! event, repaint, and metrics variants may be extended additively, but existing
//! event meanings and coordinate units are compatibility commitments.

use serde::{Deserialize, Serialize};

/// Kind of host surface that receives frames.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SurfaceKind {
    /// Owned native desktop window.
    Desktop,
    /// DAW-owned embedded plugin editor surface.
    Plugin,
}

/// Logical and physical metrics for a host surface.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct SurfaceMetrics {
    /// Finite non-negative logical width in UI units.
    pub logical_width: f32,
    /// Finite non-negative logical height in UI units.
    pub logical_height: f32,
    /// Physical width in pixels.
    pub physical_width: u32,
    /// Physical height in pixels.
    pub physical_height: u32,
    /// Finite positive device scale factor.
    pub scale_factor: f32,
}

#[derive(Deserialize)]
struct SurfaceMetricsWire {
    logical_width: f32,
    logical_height: f32,
    physical_width: u32,
    physical_height: u32,
    scale_factor: f32,
}

impl<'de> Deserialize<'de> for SurfaceMetrics {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = SurfaceMetricsWire::deserialize(deserializer)?;
        Ok(Self::new(
            wire.logical_width,
            wire.logical_height,
            wire.physical_width,
            wire.physical_height,
            wire.scale_factor,
        ))
    }
}

impl SurfaceMetrics {
    /// Creates surface metrics.
    #[must_use]
    pub const fn new(
        logical_width: f32,
        logical_height: f32,
        physical_width: u32,
        physical_height: u32,
        scale_factor: f32,
    ) -> Self {
        Self {
            logical_width: sanitize_non_negative(logical_width),
            logical_height: sanitize_non_negative(logical_height),
            physical_width,
            physical_height,
            scale_factor: sanitize_positive(scale_factor),
        }
    }
}

const fn sanitize_non_negative(value: f32) -> f32 {
    if value >= 0.0 && value <= f32::MAX {
        value
    } else {
        0.0
    }
}

const fn sanitize_positive(value: f32) -> f32 {
    if value > 0.0 && value <= f32::MAX {
        value
    } else {
        1.0
    }
}

/// Public host surface contract shared by desktop and plugin adapters.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct HostSurfaceContract {
    /// Surface kind.
    pub kind: SurfaceKind,
    /// Current metrics.
    pub metrics: SurfaceMetrics,
    /// Whether the surface currently has focus.
    pub focused: bool,
}

impl HostSurfaceContract {
    /// Creates a host surface contract.
    #[must_use]
    pub const fn new(kind: SurfaceKind, metrics: SurfaceMetrics, focused: bool) -> Self {
        Self {
            kind,
            metrics,
            focused,
        }
    }
}

/// Mouse or pointer button reported by a host surface.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MouseButton {
    /// Primary pointer button.
    Primary,
    /// Secondary pointer button.
    Secondary,
    /// Middle pointer button.
    Middle,
    /// Host-specific pointer button code.
    Other(u16),
}

/// Keyboard modifier state attached to key events.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct KeyModifiers(u8);

impl<'de> Deserialize<'de> for KeyModifiers {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bits = u8::deserialize(deserializer)?;
        Ok(Self::from_bits_truncate(bits))
    }
}

impl KeyModifiers {
    const SHIFT: u8 = 0b0001;
    const CONTROL: u8 = 0b0010;
    const ALT: u8 = 0b0100;
    const META: u8 = 0b1000;

    /// Creates empty keyboard modifier state.
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Creates modifier state from raw bit flags, ignoring unknown bits.
    #[must_use]
    pub const fn from_bits_truncate(bits: u8) -> Self {
        Self(bits & (Self::SHIFT | Self::CONTROL | Self::ALT | Self::META))
    }

    /// Returns raw modifier bit flags.
    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Adds the Shift modifier.
    #[must_use]
    pub const fn with_shift(self) -> Self {
        Self(self.0 | Self::SHIFT)
    }

    /// Adds the Control modifier.
    #[must_use]
    pub const fn with_control(self) -> Self {
        Self(self.0 | Self::CONTROL)
    }

    /// Adds the Alt or Option modifier.
    #[must_use]
    pub const fn with_alt(self) -> Self {
        Self(self.0 | Self::ALT)
    }

    /// Adds the platform command modifier.
    #[must_use]
    pub const fn with_meta(self) -> Self {
        Self(self.0 | Self::META)
    }

    /// Returns true when no modifiers are active.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns true when Shift is active.
    #[must_use]
    pub const fn shift(self) -> bool {
        self.0 & Self::SHIFT != 0
    }

    /// Returns true when Control is active.
    #[must_use]
    pub const fn control(self) -> bool {
        self.0 & Self::CONTROL != 0
    }

    /// Returns true when Alt or Option is active.
    #[must_use]
    pub const fn alt(self) -> bool {
        self.0 & Self::ALT != 0
    }

    /// Returns true when the platform command modifier is active.
    #[must_use]
    pub const fn meta(self) -> bool {
        self.0 & Self::META != 0
    }
}

/// Keyboard event normalized by a host adapter.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct KeyEvent {
    /// Logical key value after keyboard layout resolution.
    pub logical_key: String,
    /// Optional physical key code from the host platform.
    pub physical_key: Option<String>,
    /// Modifier state captured with the event.
    pub modifiers: KeyModifiers,
    /// Whether this event is an auto-repeat.
    pub repeat: bool,
}

impl KeyEvent {
    /// Creates a keyboard event.
    #[must_use]
    pub fn new(
        logical_key: impl Into<String>,
        physical_key: Option<impl Into<String>>,
        modifiers: KeyModifiers,
        repeat: bool,
    ) -> Self {
        Self {
            logical_key: logical_key.into(),
            physical_key: physical_key.map(Into::into),
            modifiers,
            repeat,
        }
    }
}

/// Input event delivered by a host surface.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum InputEvent {
    /// Pointer moved to a logical surface position.
    PointerMoved { x: f32, y: f32 },
    /// Pointer button was pressed at a logical surface position.
    PointerPressed {
        /// Button that was pressed.
        button: MouseButton,
        /// Logical x position.
        x: f32,
        /// Logical y position.
        y: f32,
    },
    /// Pointer button was released at a logical surface position.
    PointerReleased {
        /// Button that was released.
        button: MouseButton,
        /// Logical x position.
        x: f32,
        /// Logical y position.
        y: f32,
    },
    /// Pointer wheel or trackpad scroll delta.
    PointerScrolled { delta_x: f32, delta_y: f32 },
    /// Key was pressed.
    KeyPressed(KeyEvent),
    /// Key was released.
    KeyReleased(KeyEvent),
    /// Text input after platform composition.
    TextInput(String),
    /// Surface focus changed.
    FocusChanged(bool),
    /// Surface scale factor changed.
    ScaleFactorChanged(f32),
    /// Surface metrics changed.
    Resized(SurfaceMetrics),
}

impl InputEvent {
    /// Returns the event's logical surface position when one is available.
    #[must_use]
    pub const fn surface_position(&self) -> Option<(f32, f32)> {
        match *self {
            Self::PointerMoved { x, y }
            | Self::PointerPressed { x, y, .. }
            | Self::PointerReleased { x, y, .. } => Some((x, y)),
            Self::PointerScrolled { .. }
            | Self::KeyPressed(_)
            | Self::KeyReleased(_)
            | Self::TextInput(_)
            | Self::FocusChanged(_)
            | Self::ScaleFactorChanged(_)
            | Self::Resized(_) => None,
        }
    }

    /// Returns true when the event should only be delivered to a focused surface.
    #[must_use]
    pub const fn requires_focus(&self) -> bool {
        matches!(
            self,
            Self::KeyPressed(_) | Self::KeyReleased(_) | Self::TextInput(_)
        )
    }
}

/// Reason a surface requested another frame.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RepaintReason {
    /// First frame for a surface.
    Initial,
    /// Input changed visual state.
    Input,
    /// Animation advanced visual state.
    Animation,
    /// External data changed visual state.
    External,
    /// Surface metrics changed.
    Resize,
}

/// Frame scheduling policy for the next repaint.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum FrameSchedule {
    /// Render as soon as the host can deliver a frame.
    Immediate,
    /// Render at or after a monotonic timestamp in microseconds.
    AtMicros(u64),
    /// Render when the host is idle.
    Idle,
}

/// Request for a host surface repaint.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepaintRequest {
    /// Reason another frame is needed.
    pub reason: RepaintReason,
    /// Monotonic timestamp in microseconds when the request was created.
    pub requested_at_micros: u64,
    /// Scheduling policy for the requested frame.
    pub next_frame: FrameSchedule,
}

impl RepaintRequest {
    /// Creates a repaint request.
    #[must_use]
    pub const fn new(
        reason: RepaintReason,
        requested_at_micros: u64,
        next_frame: FrameSchedule,
    ) -> Self {
        Self {
            reason,
            requested_at_micros,
            next_frame,
        }
    }

    /// Returns true when this request should be serviced immediately.
    #[must_use]
    pub const fn is_immediate(self) -> bool {
        matches!(self.next_frame, FrameSchedule::Immediate)
    }
}

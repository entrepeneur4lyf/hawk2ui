//! Plugin API contracts.

use serde::{Deserialize, Serialize};

/// Stable plugin parameter identifier.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ParameterId(String);

impl ParameterId {
    /// Creates a parameter identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Automation gesture emitted by a plugin editor.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum AutomationGesture {
    /// Begin parameter gesture.
    Begin(ParameterId),
    /// Change parameter value in normalized 0..1 space.
    Change {
        parameter: ParameterId,
        normalized: f32,
    },
    /// End parameter gesture.
    End(ParameterId),
}

impl AutomationGesture {
    /// Returns the parameter identifier associated with this gesture.
    #[must_use]
    pub const fn parameter_id(&self) -> &ParameterId {
        match self {
            Self::Begin(parameter) | Self::Change { parameter, .. } | Self::End(parameter) => {
                parameter
            }
        }
    }
}

/// Public plugin parameter contract.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PluginParameterContract {
    /// Stable parameter identifier.
    pub id: ParameterId,
    /// Display name.
    pub name: String,
    /// Default normalized value.
    pub default_normalized: f32,
    /// Whether host automation is allowed.
    pub automatable: bool,
    /// Optional display unit.
    pub unit: Option<String>,
    /// Minimum accepted normalized value.
    pub normalized_min: f32,
    /// Maximum accepted normalized value.
    pub normalized_max: f32,
}

impl PluginParameterContract {
    /// Creates a plugin parameter contract.
    #[must_use]
    pub fn new(
        id: ParameterId,
        name: impl Into<String>,
        default_normalized: f32,
        automatable: bool,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            default_normalized,
            automatable,
            unit: None,
            normalized_min: 0.0,
            normalized_max: 1.0,
        }
    }

    /// Sets the display unit for this parameter.
    #[must_use]
    pub fn with_unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    /// Sets the accepted normalized range.
    #[must_use]
    pub const fn with_normalized_range(mut self, min: f32, max: f32) -> Self {
        self.normalized_min = min;
        self.normalized_max = max;
        self
    }

    /// Returns true when the normalized value is inside this parameter's accepted range.
    #[must_use]
    pub fn accepts_normalized(&self, normalized: f32) -> bool {
        normalized >= self.normalized_min && normalized <= self.normalized_max
    }
}

/// Plugin editor implementation kind.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum PluginEditorKind {
    /// Editor generated from framework/component metadata.
    Generated,
    /// Editor supplied by application code.
    Custom,
}

/// Public plugin editor contract.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PluginEditorContract {
    /// Default logical width.
    pub default_width: u32,
    /// Default logical height.
    pub default_height: u32,
    /// Minimum logical width.
    pub min_width: u32,
    /// Minimum logical height.
    pub min_height: u32,
    /// Editor implementation kind.
    pub kind: PluginEditorKind,
    /// Whether the host may resize the editor.
    pub resizable: bool,
}

impl PluginEditorContract {
    /// Creates a plugin editor contract.
    #[must_use]
    pub const fn new(
        default_width: u32,
        default_height: u32,
        min_width: u32,
        min_height: u32,
    ) -> Self {
        Self {
            default_width,
            default_height,
            min_width,
            min_height,
            kind: PluginEditorKind::Custom,
            resizable: false,
        }
    }

    /// Sets the plugin editor kind.
    #[must_use]
    pub const fn with_kind(mut self, kind: PluginEditorKind) -> Self {
        self.kind = kind;
        self
    }

    /// Sets whether the host may resize the editor.
    #[must_use]
    pub const fn with_resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }
}

/// Plugin state serialization format.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum PluginStateFormat {
    /// JSON state payload.
    Json,
    /// `MessagePack` state payload.
    MessagePack,
    /// Host-native binary state payload.
    Binary,
}

/// Single plugin state key/value entry.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PluginStateEntry {
    /// State key.
    pub key: String,
    /// State value.
    pub value: String,
}

/// Plugin state contract shared by hosts, presets, and generated editors.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PluginStateContract {
    /// State serialization format.
    pub format: PluginStateFormat,
    /// Media type for serialized state payloads.
    pub media_type: String,
    /// Deterministically ordered state entries.
    pub entries: Vec<PluginStateEntry>,
}

impl PluginStateContract {
    /// Creates an empty plugin state contract.
    #[must_use]
    pub fn new(format: PluginStateFormat, media_type: impl Into<String>) -> Self {
        Self {
            format,
            media_type: media_type.into(),
            entries: Vec::new(),
        }
    }

    /// Adds or replaces a state entry.
    #[must_use]
    pub fn with_entry(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let key = key.into();
        let value = value.into();
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.key == key) {
            entry.value = value;
        } else {
            self.entries.push(PluginStateEntry { key, value });
        }
        self.entries.sort_by(|left, right| left.key.cmp(&right.key));
        self
    }

    /// Returns a state entry value by key.
    #[must_use]
    pub fn entry(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|entry| entry.key == key)
            .map(|entry| entry.value.as_str())
    }
}

/// Plugin preset contract.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PluginPresetContract {
    /// Stable preset identifier.
    pub id: String,
    /// Preset display name.
    pub name: String,
    /// Optional serialized state attached to the preset.
    pub state: Option<PluginStateContract>,
}

impl PluginPresetContract {
    /// Creates a plugin preset contract.
    #[must_use]
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            state: None,
        }
    }

    /// Attaches state to the preset.
    #[must_use]
    pub fn with_state(mut self, state: PluginStateContract) -> Self {
        self.state = Some(state);
        self
    }
}

/// Realtime data payload kind.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RealtimeDataKind {
    /// `f32` frame data.
    F32Frames,
    /// MIDI 1.0 byte messages.
    MidiBytes,
    /// Metering scalar values.
    MeterValues,
}

/// Realtime data direction.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RealtimeDataDirection {
    /// Audio processor publishes data to the editor.
    ProcessorToEditor,
    /// Editor publishes data to the audio processor.
    EditorToProcessor,
}

/// Realtime data channel contract.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RealtimeDataContract {
    /// Channel name.
    pub name: String,
    /// Payload kind.
    pub kind: RealtimeDataKind,
    /// Channel direction.
    pub direction: RealtimeDataDirection,
    /// Ring capacity in frames.
    pub capacity_frames: u32,
    /// Number of interleaved or parallel channels.
    pub channel_count: u16,
}

impl RealtimeDataContract {
    /// Creates a realtime data contract.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        kind: RealtimeDataKind,
        direction: RealtimeDataDirection,
    ) -> Self {
        Self {
            name: name.into(),
            kind,
            direction,
            capacity_frames: 0,
            channel_count: 1,
        }
    }

    /// Sets the channel capacity in frames.
    #[must_use]
    pub const fn with_capacity(mut self, capacity_frames: u32) -> Self {
        self.capacity_frames = capacity_frames;
        self
    }

    /// Sets the channel count.
    #[must_use]
    pub const fn with_channel_count(mut self, channel_count: u16) -> Self {
        self.channel_count = channel_count;
        self
    }
}

//! Custom draw surface integration records.

use crate::Geometry;

/// Custom surface category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CustomSurfaceCategory {
    /// Knob surface.
    Knob,
    /// Slider surface.
    Slider,
    /// Meter surface.
    Meter,
    /// Scope surface.
    Scope,
    /// Analyzer surface.
    Analyzer,
    /// Equalizer curve surface.
    EqCurve,
    /// Modulation surface.
    Modulation,
    /// Timeline surface.
    Timeline,
    /// Graph editor surface.
    GraphEditor,
    /// Inspector panel surface.
    InspectorPanel,
}

impl CustomSurfaceCategory {
    /// Returns stable category key.
    #[must_use]
    pub const fn stable_key(&self) -> &'static str {
        match self {
            Self::Knob => "knob",
            Self::Slider => "slider",
            Self::Meter => "meter",
            Self::Scope => "scope",
            Self::Analyzer => "analyzer",
            Self::EqCurve => "eq-curve",
            Self::Modulation => "modulation",
            Self::Timeline => "timeline",
            Self::GraphEditor => "graph-editor",
            Self::InspectorPanel => "inspector-panel",
        }
    }
}

/// Custom surface capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CustomSurfaceCapability {
    /// Realtime data feed support.
    RealtimeData,
    /// GPU preferred rendering.
    GpuPreferred,
    /// Pointer interaction support.
    PointerInteraction,
}

/// Custom surface validation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomSurfaceError {
    rule: String,
    message: String,
}

impl CustomSurfaceError {
    /// Creates a custom surface error.
    #[must_use]
    pub fn new(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            rule: rule.into(),
            message: message.into(),
        }
    }

    /// Returns the stable diagnostic rule.
    #[must_use]
    pub fn rule(&self) -> &str {
        &self.rule
    }

    /// Returns the diagnostic message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Custom draw surface record.
#[derive(Clone, Debug, PartialEq)]
pub struct CustomDrawSurface {
    id: String,
    category: CustomSurfaceCategory,
    reserved_layout: Geometry,
    capabilities: Vec<CustomSurfaceCapability>,
    invalidated: bool,
    next_frame: Option<u64>,
}

impl CustomDrawSurface {
    /// Creates a custom draw surface.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        category: CustomSurfaceCategory,
        reserved_layout: Geometry,
    ) -> Self {
        Self {
            id: id.into(),
            category,
            reserved_layout,
            capabilities: Vec::new(),
            invalidated: false,
            next_frame: None,
        }
    }

    /// Adds a capability report.
    #[must_use]
    pub fn with_capability(mut self, capability: CustomSurfaceCapability) -> Self {
        if !self.capabilities.contains(&capability) {
            self.capabilities.push(capability);
        }
        self
    }

    /// Marks the surface invalidated.
    #[must_use]
    pub const fn invalidate(mut self) -> Self {
        self.invalidated = true;
        self
    }

    /// Schedules the next frame.
    #[must_use]
    pub const fn schedule_frame(mut self, frame: u64) -> Self {
        self.next_frame = Some(frame);
        self
    }

    /// Returns whether a point hits the reserved layout.
    #[must_use]
    pub fn hit_test(&self, x: f32, y: f32) -> bool {
        if !x.is_finite() || !y.is_finite() || !is_valid_geometry(self.reserved_layout) {
            return false;
        }
        x >= self.reserved_layout.x
            && y >= self.reserved_layout.y
            && x <= self.reserved_layout.x + self.reserved_layout.width
            && y <= self.reserved_layout.y + self.reserved_layout.height
    }

    /// Returns reserved layout geometry.
    #[must_use]
    pub const fn reserved_layout(&self) -> Geometry {
        self.reserved_layout
    }

    /// Returns whether this surface is invalidated.
    #[must_use]
    pub const fn invalidated(&self) -> bool {
        self.invalidated
    }

    /// Returns next scheduled frame.
    #[must_use]
    pub const fn next_frame(&self) -> Option<u64> {
        self.next_frame
    }

    /// Returns whether a capability is reported.
    #[must_use]
    pub fn reports_capability(&self, capability: CustomSurfaceCapability) -> bool {
        self.capabilities.contains(&capability)
    }

    /// Validates custom surface identity and reserved geometry.
    ///
    /// # Errors
    ///
    /// Returns [`CustomSurfaceError`] when the surface cannot be safely exported or hit-tested.
    pub fn validate(&self) -> Result<(), CustomSurfaceError> {
        if self.id.trim().is_empty() {
            return Err(CustomSurfaceError::new(
                "custom-surface.id.invalid",
                "custom surface ID must not be empty",
            ));
        }
        if !is_valid_geometry(self.reserved_layout) {
            return Err(CustomSurfaceError::new(
                "custom-surface.geometry.invalid",
                "custom surface geometry must be finite with non-negative dimensions",
            ));
        }
        Ok(())
    }

    /// Returns stable surface key.
    ///
    /// # Errors
    ///
    /// Returns [`CustomSurfaceError`] when the surface identity or geometry is invalid.
    pub fn stable_key(&self) -> Result<String, CustomSurfaceError> {
        self.validate()?;
        Ok(format!("{}:{}", self.id, self.category.stable_key()))
    }
}

fn is_valid_geometry(geometry: Geometry) -> bool {
    geometry.x.is_finite()
        && geometry.y.is_finite()
        && geometry.width.is_finite()
        && geometry.height.is_finite()
        && geometry.width >= 0.0
        && geometry.height >= 0.0
}

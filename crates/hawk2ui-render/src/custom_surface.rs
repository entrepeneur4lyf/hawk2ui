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
    frame_interval: Option<u64>,
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
            frame_interval: None,
        }
    }

    /// Returns the stable surface identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the declared surface category.
    #[must_use]
    pub const fn category(&self) -> CustomSurfaceCategory {
        self.category
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

    /// Sets the minimum host-frame interval between custom draws.
    ///
    /// A value of `1` allows drawing on every frame. Values larger than `1` are useful for
    /// meters, analyzers, scopes, timelines, and inspectors that should update independently from
    /// the host window's maximum frame rate.
    #[must_use]
    pub const fn with_frame_interval(mut self, frame_interval: u64) -> Self {
        self.frame_interval = Some(if frame_interval == 0 {
            1
        } else {
            frame_interval
        });
        self
    }

    /// Returns the minimum host-frame interval between custom draws.
    #[must_use]
    pub const fn frame_interval(&self) -> Option<u64> {
        self.frame_interval
    }

    /// Returns whether this surface is due to draw on the supplied host frame index.
    #[must_use]
    pub fn is_frame_due(&self, frame_index: u64) -> bool {
        let scheduled = self
            .next_frame
            .is_none_or(|next_frame| frame_index >= next_frame);
        let interval = self.frame_interval.unwrap_or(1);
        scheduled && frame_index.is_multiple_of(interval)
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

/// Plugin-safe realtime data snapshot for a custom draw surface.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CustomSurfaceDataSnapshot {
    samples: Vec<f32>,
}

impl CustomSurfaceDataSnapshot {
    /// Creates a bounded realtime data snapshot from finite scalar samples.
    ///
    /// # Errors
    ///
    /// Returns [`CustomSurfaceError`] when a sample is non-finite or the snapshot is too large for
    /// deterministic UI-thread drawing.
    pub fn new<I>(samples: I) -> Result<Self, CustomSurfaceError>
    where
        I: IntoIterator<Item = f32>,
    {
        let samples: Vec<f32> = samples.into_iter().collect();
        if samples.len() > 4096 || samples.iter().any(|sample| !sample.is_finite()) {
            return Err(CustomSurfaceError::new(
                "custom-surface.data.invalid",
                "custom surface data samples must be finite and contain at most 4096 values",
            ));
        }
        Ok(Self { samples })
    }

    /// Returns the immutable realtime sample payload.
    #[must_use]
    pub fn samples(&self) -> &[f32] {
        &self.samples
    }
}

/// Host frame metadata supplied to a custom draw hook.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CustomSurfaceFrameContext {
    frame_index: u64,
    dpi_scale: f32,
}

impl CustomSurfaceFrameContext {
    /// Creates custom surface frame context.
    ///
    /// # Errors
    ///
    /// Returns [`CustomSurfaceError`] when DPI scale is non-finite or not greater than zero.
    pub fn new(frame_index: u64, dpi_scale: f32) -> Result<Self, CustomSurfaceError> {
        if !dpi_scale.is_finite() || dpi_scale <= 0.0 {
            return Err(CustomSurfaceError::new(
                "custom-surface.frame-context.invalid",
                "custom surface DPI scale must be finite and greater than zero",
            ));
        }
        Ok(Self {
            frame_index,
            dpi_scale,
        })
    }

    /// Returns the host frame index.
    #[must_use]
    pub const fn frame_index(&self) -> u64 {
        self.frame_index
    }

    /// Returns the host DPI scale.
    #[must_use]
    pub const fn dpi_scale(&self) -> f32 {
        self.dpi_scale
    }
}

/// Complete input passed to a renderer-specific custom draw hook.
#[derive(Clone, Debug, PartialEq)]
pub struct CustomSurfaceDrawRequest {
    surface: CustomDrawSurface,
    context: CustomSurfaceFrameContext,
    data: CustomSurfaceDataSnapshot,
}

impl CustomSurfaceDrawRequest {
    /// Creates a custom draw request.
    ///
    /// # Errors
    ///
    /// Returns [`CustomSurfaceError`] when the surface cannot be safely rendered.
    pub fn new(
        surface: CustomDrawSurface,
        context: CustomSurfaceFrameContext,
        data: CustomSurfaceDataSnapshot,
    ) -> Result<Self, CustomSurfaceError> {
        surface.validate()?;
        Ok(Self {
            surface,
            context,
            data,
        })
    }

    /// Returns the custom surface metadata and geometry.
    #[must_use]
    pub const fn surface(&self) -> &CustomDrawSurface {
        &self.surface
    }

    /// Returns frame metadata for the draw.
    #[must_use]
    pub const fn context(&self) -> CustomSurfaceFrameContext {
        self.context
    }

    /// Returns plugin-safe realtime data for the draw.
    #[must_use]
    pub const fn data(&self) -> &CustomSurfaceDataSnapshot {
        &self.data
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

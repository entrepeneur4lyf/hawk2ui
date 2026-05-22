//! Host surface API contracts.

/// Kind of host surface that receives frames.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceKind {
    /// Owned native desktop window.
    Desktop,
    /// DAW-owned embedded plugin editor surface.
    Plugin,
}

/// Logical and physical metrics for a host surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceMetrics {
    /// Logical width in UI units.
    pub logical_width: f32,
    /// Logical height in UI units.
    pub logical_height: f32,
    /// Physical width in pixels.
    pub physical_width: u32,
    /// Physical height in pixels.
    pub physical_height: u32,
    /// Device scale factor.
    pub scale_factor: f32,
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
            logical_width,
            logical_height,
            physical_width,
            physical_height,
            scale_factor,
        }
    }
}

/// Public host surface contract shared by desktop and plugin adapters.
#[derive(Clone, Debug, PartialEq)]
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

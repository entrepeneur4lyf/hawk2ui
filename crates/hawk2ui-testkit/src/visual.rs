//! Visual regression suite helpers.

use crate::VisualSnapshot;

/// Required deterministic visual fixture family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VisualFixtureKind {
    /// Text rendering fixture.
    Text,
    /// Shape rendering fixture.
    Shape,
    /// Gradient rendering fixture.
    Gradient,
    /// Image layer fixture.
    ImageLayer,
    /// Vector asset fixture.
    VectorAsset,
    /// Custom control fixture.
    CustomControl,
    /// Graph surface fixture.
    GraphSurface,
    /// DPI scaling fixture.
    DpiScaling,
}

/// Deterministic visual fixture metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct VisualFixture {
    kind: VisualFixtureKind,
    key: String,
    width: u32,
    height: u32,
    dpi_scale: f32,
}

impl VisualFixture {
    /// Creates visual fixture metadata.
    #[must_use]
    pub fn new(
        kind: VisualFixtureKind,
        key: impl Into<String>,
        width: u32,
        height: u32,
        dpi_scale: f32,
    ) -> Self {
        Self {
            kind,
            key: key.into(),
            width,
            height,
            dpi_scale,
        }
    }

    /// Returns the fixture kind.
    #[must_use]
    pub const fn kind(&self) -> VisualFixtureKind {
        self.kind
    }

    /// Returns the fixture key.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Returns logical fixture size.
    #[must_use]
    pub const fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Returns fixture DPI scale.
    #[must_use]
    pub const fn dpi_scale(&self) -> f32 {
        self.dpi_scale
    }
}

/// Production visual fixture set.
#[derive(Clone, Debug, PartialEq)]
pub struct VisualFixtureSet {
    fixtures: Vec<VisualFixture>,
}

impl VisualFixtureSet {
    /// Creates a visual fixture set.
    #[must_use]
    pub fn new(fixtures: impl IntoIterator<Item = VisualFixture>) -> Self {
        Self {
            fixtures: fixtures.into_iter().collect(),
        }
    }

    /// Creates the production visual fixture baseline.
    #[must_use]
    pub fn production_baseline() -> Self {
        use VisualFixtureKind::{
            CustomControl, DpiScaling, Gradient, GraphSurface, ImageLayer, Shape, Text, VectorAsset,
        };

        Self::new([
            VisualFixture::new(Text, "visual::text-baseline", 640, 180, 1.0),
            VisualFixture::new(Shape, "visual::shape-stack", 640, 360, 1.0),
            VisualFixture::new(Gradient, "visual::gradient-card", 640, 360, 1.0),
            VisualFixture::new(ImageLayer, "visual::image-layer", 1024, 512, 1.0),
            VisualFixture::new(VectorAsset, "visual::vector-logo", 512, 256, 1.0),
            VisualFixture::new(CustomControl, "visual::custom-knob", 360, 360, 1.0),
            VisualFixture::new(GraphSurface, "visual::graph-surface", 960, 480, 1.0),
            VisualFixture::new(DpiScaling, "visual::dpi-scaling", 640, 360, 2.0),
        ])
    }

    /// Returns all visual fixtures.
    #[must_use]
    pub fn fixtures(&self) -> &[VisualFixture] {
        &self.fixtures
    }

    /// Returns a fixture by kind.
    #[must_use]
    pub fn fixture(&self, kind: VisualFixtureKind) -> Option<&VisualFixture> {
        self.fixtures.iter().find(|fixture| fixture.kind == kind)
    }
}

/// Image comparison metadata for deterministic snapshot checks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageComparisonMetadata {
    max_pixel_delta: u8,
    max_changed_pixels: u32,
    color_space: String,
}

impl ImageComparisonMetadata {
    /// Creates image comparison metadata.
    #[must_use]
    pub fn new(
        max_pixel_delta: u8,
        max_changed_pixels: u32,
        color_space: impl Into<String>,
    ) -> Self {
        Self {
            max_pixel_delta,
            max_changed_pixels,
            color_space: color_space.into(),
        }
    }

    /// Creates strict `RGBA8` metadata for exact snapshots.
    #[must_use]
    pub fn strict_rgba8() -> Self {
        Self::new(0, 0, "rgba8-srgb")
    }

    /// Returns true when an image diff is within the accepted threshold.
    #[must_use]
    pub const fn accepts(&self, max_pixel_delta: u8, changed_pixels: u32) -> bool {
        max_pixel_delta <= self.max_pixel_delta && changed_pixels <= self.max_changed_pixels
    }

    /// Returns the expected color space.
    #[must_use]
    pub fn color_space(&self) -> &str {
        &self.color_space
    }
}

/// Visual regression case with a baseline and candidate snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisualRegressionCase {
    name: String,
    baseline: VisualSnapshot,
    candidate: VisualSnapshot,
}

impl VisualRegressionCase {
    /// Creates a visual regression case.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        baseline: VisualSnapshot,
        candidate: VisualSnapshot,
    ) -> Self {
        Self {
            name: name.into(),
            baseline,
            candidate,
        }
    }

    /// Returns the case name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns true when baseline and candidate metadata match.
    #[must_use]
    pub fn matches_baseline(&self) -> bool {
        self.baseline == self.candidate
    }
}

/// Collection of visual regression cases.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VisualRegressionSuite {
    cases: Vec<VisualRegressionCase>,
}

impl VisualRegressionSuite {
    /// Creates an empty visual regression suite.
    #[must_use]
    pub const fn new() -> Self {
        Self { cases: Vec::new() }
    }

    /// Adds a visual regression case.
    #[must_use]
    pub fn with_case(mut self, case: VisualRegressionCase) -> Self {
        self.cases.push(case);
        self
    }

    /// Returns all visual regression cases.
    #[must_use]
    pub fn cases(&self) -> &[VisualRegressionCase] {
        &self.cases
    }

    /// Returns whether every case matches its baseline.
    #[must_use]
    pub fn all_match(&self) -> bool {
        self.cases
            .iter()
            .all(VisualRegressionCase::matches_baseline)
    }
}

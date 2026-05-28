//! Visual regression suite helpers.

use std::{
    fmt::Write as _,
    fs, io,
    path::{Path, PathBuf},
};

use hawk2ui_render::Geometry;

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
    /// Premium desktop application template fixture.
    PremiumDesktopTemplate,
    /// Premium plugin editor template fixture.
    PremiumPluginTemplate,
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
            CustomControl, DpiScaling, Gradient, GraphSurface, ImageLayer, PremiumDesktopTemplate,
            PremiumPluginTemplate, Shape, Text, VectorAsset,
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
            VisualFixture::new(
                PremiumDesktopTemplate,
                "visual::premium-desktop-template",
                1280,
                720,
                1.0,
            ),
            VisualFixture::new(
                PremiumPluginTemplate,
                "visual::premium-plugin-template",
                960,
                540,
                1.0,
            ),
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

    /// Compares two pixel snapshots and returns deterministic diff metadata.
    #[must_use]
    pub fn compare(
        &self,
        baseline: &VisualImageSnapshot,
        candidate: &VisualImageSnapshot,
    ) -> ImageComparisonReport {
        if baseline.width != candidate.width
            || baseline.height != candidate.height
            || baseline.pixels.len() != candidate.pixels.len()
        {
            return ImageComparisonReport::dimension_mismatch(
                baseline.name.clone(),
                baseline.width,
                baseline.height,
                candidate.width,
                candidate.height,
            );
        }

        let mut changed_pixels = 0_u32;
        let mut max_pixel_delta = 0_u8;
        for (left, right) in baseline.pixels.iter().zip(candidate.pixels.iter()) {
            let delta = pixel_delta(*left, *right);
            if delta > 0 {
                changed_pixels = changed_pixels.saturating_add(1);
                max_pixel_delta = max_pixel_delta.max(delta);
            }
        }
        ImageComparisonReport::new(
            baseline.name.clone(),
            max_pixel_delta,
            changed_pixels,
            self.accepts(max_pixel_delta, changed_pixels),
        )
    }

    /// Returns the expected color space.
    #[must_use]
    pub fn color_space(&self) -> &str {
        &self.color_space
    }
}

/// Visual regression helper error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VisualRegressionError {
    /// Pixel count overflowed addressable memory.
    PixelCountOverflow {
        /// Snapshot width.
        width: u32,
        /// Snapshot height.
        height: u32,
    },
    /// Pixel buffer length does not match dimensions.
    PixelCountMismatch {
        /// Expected pixel count.
        expected: usize,
        /// Actual pixel count.
        actual: usize,
    },
}

/// Files written by a visual regression artifact export.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisualArtifactSet {
    root: PathBuf,
    files: Vec<PathBuf>,
}

impl VisualArtifactSet {
    fn new(root: PathBuf, files: Vec<PathBuf>) -> Self {
        Self { root, files }
    }

    /// Returns the artifact root directory.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns the written artifact files.
    #[must_use]
    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }

    /// Returns true when an artifact with the requested file name was written.
    #[must_use]
    pub fn contains_file_name(&self, file_name: &str) -> bool {
        self.files
            .iter()
            .any(|file| file.file_name().is_some_and(|name| name == file_name))
    }
}

/// CPU-readable image snapshot used by visual regression tests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisualImageSnapshot {
    name: String,
    width: u32,
    height: u32,
    pixels: Vec<u32>,
}

impl VisualImageSnapshot {
    /// Creates an image snapshot from `0x00RRGGBB` pixels.
    ///
    /// # Errors
    ///
    /// Returns [`VisualRegressionError`] when dimensions overflow or pixel count does not match.
    pub fn from_pixels(
        name: impl Into<String>,
        width: u32,
        height: u32,
        pixels: Vec<u32>,
    ) -> Result<Self, VisualRegressionError> {
        let expected = usize::try_from(u64::from(width) * u64::from(height))
            .map_err(|_| VisualRegressionError::PixelCountOverflow { width, height })?;
        if pixels.len() != expected {
            return Err(VisualRegressionError::PixelCountMismatch {
                expected,
                actual: pixels.len(),
            });
        }
        Ok(Self {
            name: name.into(),
            width,
            height,
            pixels,
        })
    }

    /// Returns the snapshot name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the snapshot width in pixels.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Returns the snapshot height in pixels.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Returns snapshot pixels in `0x00RRGGBB` order.
    #[must_use]
    pub fn pixels(&self) -> &[u32] {
        &self.pixels
    }

    /// Writes this snapshot as an ASCII `P3` PPM file.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the destination cannot be written.
    pub fn write_ppm(&self, path: impl AsRef<Path>) -> io::Result<()> {
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, self.ppm_payload())
    }

    /// Counts pixels in a region that differ from a reference color.
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn count_changed_pixels(&self, reference_color: u32, region: Geometry) -> u32 {
        let x_start = region.x.max(0.0).floor() as u32;
        let y_start = region.y.max(0.0).floor() as u32;
        let x_end = (region.x + region.width).ceil().max(0.0) as u32;
        let y_end = (region.y + region.height).ceil().max(0.0) as u32;
        let x_end = x_end.min(self.width);
        let y_end = y_end.min(self.height);

        let mut changed = 0_u32;
        for y in y_start..y_end {
            for x in x_start..x_end {
                let Ok(index) =
                    usize::try_from(u64::from(y) * u64::from(self.width) + u64::from(x))
                else {
                    continue;
                };
                if self
                    .pixels
                    .get(index)
                    .is_some_and(|pixel| *pixel != reference_color)
                {
                    changed = changed.saturating_add(1);
                }
            }
        }
        changed
    }

    fn ppm_payload(&self) -> String {
        let mut payload = format!("P3\n{} {}\n255\n", self.width, self.height);
        for pixel in &self.pixels {
            let red = (pixel >> 16) & 0xff;
            let green = (pixel >> 8) & 0xff;
            let blue = pixel & 0xff;
            let _ = writeln!(payload, "{red} {green} {blue}");
        }
        payload
    }
}

/// Deterministic image comparison report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageComparisonReport {
    case: String,
    max_pixel_delta: u8,
    changed_pixels: u32,
    accepted: bool,
    dimension_mismatch: bool,
    baseline_size: (u32, u32),
    candidate_size: (u32, u32),
}

impl ImageComparisonReport {
    fn new(case: String, max_pixel_delta: u8, changed_pixels: u32, accepted: bool) -> Self {
        Self {
            case,
            max_pixel_delta,
            changed_pixels,
            accepted,
            dimension_mismatch: false,
            baseline_size: (0, 0),
            candidate_size: (0, 0),
        }
    }

    fn dimension_mismatch(
        case: String,
        baseline_width: u32,
        baseline_height: u32,
        candidate_width: u32,
        candidate_height: u32,
    ) -> Self {
        Self {
            case,
            max_pixel_delta: u8::MAX,
            changed_pixels: u32::MAX,
            accepted: false,
            dimension_mismatch: true,
            baseline_size: (baseline_width, baseline_height),
            candidate_size: (candidate_width, candidate_height),
        }
    }

    /// Returns the largest channel delta observed.
    #[must_use]
    pub const fn max_pixel_delta(&self) -> u8 {
        self.max_pixel_delta
    }

    /// Returns number of changed pixels.
    #[must_use]
    pub const fn changed_pixels(&self) -> u32 {
        self.changed_pixels
    }

    /// Returns whether the report satisfies its comparison metadata.
    #[must_use]
    pub const fn accepted(&self) -> bool {
        self.accepted
    }

    /// Returns deterministic report payload suitable for writing as a diff artifact.
    #[must_use]
    pub fn artifact_payload(&self) -> String {
        format!(
            "case = \"{}\"\naccepted = {}\nchanged_pixels = {}\nmax_pixel_delta = {}\ndimension_mismatch = {}\nbaseline_size = \"{}x{}\"\ncandidate_size = \"{}x{}\"\n",
            escape_toml_string(&self.case),
            self.accepted,
            self.changed_pixels,
            self.max_pixel_delta,
            self.dimension_mismatch,
            self.baseline_size.0,
            self.baseline_size.1,
            self.candidate_size.0,
            self.candidate_size.1,
        )
    }

    /// Writes the report payload as a deterministic text artifact.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the artifact cannot be written.
    pub fn write_artifact(&self, path: impl AsRef<Path>) -> io::Result<()> {
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, self.artifact_payload())
    }
}

/// Visual regression case with a baseline and candidate snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisualRegressionCase {
    name: String,
    baseline: VisualSnapshot,
    candidate: VisualSnapshot,
}

/// Visual image regression case with baseline and candidate pixels.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisualImageRegressionCase {
    name: String,
    baseline: VisualImageSnapshot,
    candidate: VisualImageSnapshot,
}

impl VisualImageRegressionCase {
    /// Creates an image regression case.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        baseline: VisualImageSnapshot,
        candidate: VisualImageSnapshot,
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

    /// Returns the baseline snapshot.
    #[must_use]
    pub const fn baseline(&self) -> &VisualImageSnapshot {
        &self.baseline
    }

    /// Returns the candidate snapshot.
    #[must_use]
    pub const fn candidate(&self) -> &VisualImageSnapshot {
        &self.candidate
    }
}

/// Aggregated visual regression report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisualRegressionReport {
    reports: Vec<ImageComparisonReport>,
}

impl VisualRegressionReport {
    fn new(reports: Vec<ImageComparisonReport>) -> Self {
        Self { reports }
    }

    /// Returns the number of evaluated cases.
    #[must_use]
    pub fn case_count(&self) -> usize {
        self.reports.len()
    }

    /// Returns the number of failed cases.
    #[must_use]
    pub fn failed_count(&self) -> usize {
        self.reports
            .iter()
            .filter(|report| !report.accepted())
            .count()
    }

    /// Returns whether all visual cases are accepted.
    #[must_use]
    pub fn accepted(&self) -> bool {
        self.failed_count() == 0
    }

    /// Returns deterministic report payload suitable for writing as suite artifact.
    #[must_use]
    pub fn artifact_payload(&self) -> String {
        let mut payload = format!(
            "case_count = {}\nfailed_count = {}\naccepted = {}\n",
            self.case_count(),
            self.failed_count(),
            self.accepted()
        );
        for report in &self.reports {
            payload.push_str("\n[[cases]]\n");
            payload.push_str(&report.artifact_payload());
        }
        payload
    }

    /// Writes the suite report payload as a deterministic text artifact.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the artifact cannot be written.
    pub fn write_artifact(&self, path: impl AsRef<Path>) -> io::Result<()> {
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, self.artifact_payload())
    }
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
    image_cases: Vec<VisualImageRegressionCase>,
}

impl VisualRegressionSuite {
    /// Creates an empty visual regression suite.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            cases: Vec::new(),
            image_cases: Vec::new(),
        }
    }

    /// Adds a visual regression case.
    #[must_use]
    pub fn with_case(mut self, case: VisualRegressionCase) -> Self {
        self.cases.push(case);
        self
    }

    /// Adds a pixel image regression case.
    #[must_use]
    pub fn with_image_case(
        mut self,
        name: impl Into<String>,
        baseline: VisualImageSnapshot,
        candidate: VisualImageSnapshot,
    ) -> Self {
        self.image_cases
            .push(VisualImageRegressionCase::new(name, baseline, candidate));
        self
    }

    /// Returns all visual regression cases.
    #[must_use]
    pub fn cases(&self) -> &[VisualRegressionCase] {
        &self.cases
    }

    /// Returns all image visual regression cases.
    #[must_use]
    pub fn image_cases(&self) -> &[VisualImageRegressionCase] {
        &self.image_cases
    }

    /// Evaluates all image regression cases with comparison metadata.
    #[must_use]
    pub fn evaluate_images(&self, comparison: &ImageComparisonMetadata) -> VisualRegressionReport {
        VisualRegressionReport::new(
            self.image_cases
                .iter()
                .map(|case| comparison.compare(case.baseline(), case.candidate()))
                .collect(),
        )
    }

    /// Writes report, baseline, candidate, and diff artifacts for all image cases.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when any artifact cannot be written.
    pub fn write_image_artifacts(
        &self,
        comparison: &ImageComparisonMetadata,
        directory: impl AsRef<Path>,
    ) -> io::Result<VisualArtifactSet> {
        let root = directory.as_ref().to_path_buf();
        fs::create_dir_all(&root)?;

        let report = self.evaluate_images(comparison);
        let report_path = root.join("visual-report.toml");
        report.write_artifact(&report_path)?;
        let mut files = vec![report_path];

        for case in &self.image_cases {
            let case_name = sanitize_artifact_name(case.name());
            let baseline_path = root.join(format!("{case_name}-baseline.ppm"));
            let candidate_path = root.join(format!("{case_name}-candidate.ppm"));
            let diff_path = root.join(format!("{case_name}-diff.ppm"));
            case.baseline().write_ppm(&baseline_path)?;
            case.candidate().write_ppm(&candidate_path)?;
            write_diff_ppm(case.baseline(), case.candidate(), &diff_path)?;
            files.extend([baseline_path, candidate_path, diff_path]);
        }

        Ok(VisualArtifactSet::new(root, files))
    }

    /// Returns whether every case matches its baseline.
    #[must_use]
    pub fn all_match(&self) -> bool {
        self.cases
            .iter()
            .all(VisualRegressionCase::matches_baseline)
    }
}

fn write_diff_ppm(
    baseline: &VisualImageSnapshot,
    candidate: &VisualImageSnapshot,
    path: impl AsRef<Path>,
) -> io::Result<()> {
    let diff_pixels = if baseline.width == candidate.width && baseline.height == candidate.height {
        baseline
            .pixels
            .iter()
            .zip(candidate.pixels.iter())
            .map(|(left, right)| diff_pixel(*left, *right))
            .collect()
    } else {
        vec![0x00ff_0000; baseline.pixels.len()]
    };

    VisualImageSnapshot::from_pixels(
        format!("{}-diff", baseline.name()),
        baseline.width,
        baseline.height,
        diff_pixels,
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, format!("{error:?}")))?
    .write_ppm(path)
}

fn pixel_delta(left: u32, right: u32) -> u8 {
    let left_r = ((left >> 16) & 0xff) as u8;
    let left_g = ((left >> 8) & 0xff) as u8;
    let left_b = (left & 0xff) as u8;
    let right_r = ((right >> 16) & 0xff) as u8;
    let right_g = ((right >> 8) & 0xff) as u8;
    let right_b = (right & 0xff) as u8;
    left_r
        .abs_diff(right_r)
        .max(left_g.abs_diff(right_g))
        .max(left_b.abs_diff(right_b))
}

fn diff_pixel(left: u32, right: u32) -> u32 {
    if left == right {
        return 0x0000_0000;
    }
    let delta = u32::from(pixel_delta(left, right));
    (delta << 16) | ((255 - delta) << 8)
}

fn escape_toml_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch => escaped.push(ch),
        }
    }
    escaped
}

fn sanitize_artifact_name(value: &str) -> String {
    let mut sanitized = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            sanitized.push(ch);
        } else {
            sanitized.push('-');
        }
    }
    if sanitized.is_empty() {
        "visual-case".to_string()
    } else {
        sanitized
    }
}

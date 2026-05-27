//! Plugin editor layout constraints.

/// Plugin editor size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PluginEditorSize {
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

impl PluginEditorSize {
    /// Creates plugin editor size.
    #[must_use]
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

/// Graph region geometry.
#[derive(Clone, Debug, PartialEq)]
pub struct GraphRegion {
    id: String,
    /// X coordinate.
    pub x: f32,
    /// Y coordinate.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

impl GraphRegion {
    /// Creates a graph region.
    #[must_use]
    pub fn new(id: impl Into<String>, x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            id: id.into(),
            x,
            y,
            width,
            height,
        }
    }

    /// Returns the graph region ID.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
}

/// Analyzer region geometry.
#[derive(Clone, Debug, PartialEq)]
pub struct AnalyzerRegion {
    id: String,
    /// X coordinate.
    pub x: f32,
    /// Y coordinate.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

impl AnalyzerRegion {
    /// Creates an analyzer region.
    #[must_use]
    pub fn new(id: impl Into<String>, x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            id: id.into(),
            x,
            y,
            width,
            height,
        }
    }

    /// Returns the analyzer region ID.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
}

/// Generated parameter layout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedParameterLayout {
    id: String,
    parameters: Vec<String>,
    columns: usize,
}

impl GeneratedParameterLayout {
    /// Creates a dense parameter panel layout.
    #[must_use]
    pub fn dense_panel(
        id: impl Into<String>,
        parameters: impl IntoIterator<Item = impl Into<String>>,
        columns: usize,
    ) -> Self {
        Self {
            id: id.into(),
            parameters: parameters.into_iter().map(Into::into).collect(),
            columns: columns.max(1),
        }
    }

    /// Returns the generated layout ID.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the generated parameter IDs.
    #[must_use]
    pub fn parameters(&self) -> &[String] {
        &self.parameters
    }

    /// Returns row count for the parameter layout.
    #[must_use]
    pub fn rows(&self) -> usize {
        self.parameters.len().div_ceil(self.columns)
    }
}

/// Plugin layout validation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginLayoutError {
    rule: String,
    message: String,
}

impl PluginLayoutError {
    /// Creates a plugin layout error.
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

/// Plugin editor constraints.
#[derive(Clone, Debug, PartialEq)]
pub struct PluginEditorConstraints {
    default_size: PluginEditorSize,
    min_size: Option<PluginEditorSize>,
    max_size: Option<PluginEditorSize>,
    graph_regions: Vec<GraphRegion>,
    analyzer_regions: Vec<AnalyzerRegion>,
    generated_parameters: Vec<GeneratedParameterLayout>,
}

impl PluginEditorConstraints {
    /// Creates plugin editor constraints with a fixed default editor size.
    #[must_use]
    pub const fn new(default_size: PluginEditorSize) -> Self {
        Self {
            default_size,
            min_size: None,
            max_size: None,
            graph_regions: Vec::new(),
            analyzer_regions: Vec::new(),
            generated_parameters: Vec::new(),
        }
    }

    /// Adds minimum host-driven resize constraints.
    #[must_use]
    pub const fn with_min_size(mut self, min_size: PluginEditorSize) -> Self {
        self.min_size = Some(min_size);
        self
    }

    /// Adds maximum host-driven resize constraints.
    #[must_use]
    pub const fn with_max_size(mut self, max_size: PluginEditorSize) -> Self {
        self.max_size = Some(max_size);
        self
    }

    /// Adds a graph region.
    #[must_use]
    pub fn with_graph_region(mut self, region: GraphRegion) -> Self {
        self.graph_regions.push(region);
        self
    }

    /// Adds an analyzer region.
    #[must_use]
    pub fn with_analyzer_region(mut self, region: AnalyzerRegion) -> Self {
        self.analyzer_regions.push(region);
        self
    }

    /// Adds a generated parameter layout.
    #[must_use]
    pub fn with_generated_parameters(mut self, layout: GeneratedParameterLayout) -> Self {
        self.generated_parameters.push(layout);
        self
    }

    /// Returns default editor size.
    #[must_use]
    pub const fn default_size(&self) -> PluginEditorSize {
        self.default_size
    }

    /// Clamps a host-provided editor size to min/max constraints.
    #[must_use]
    pub fn clamp_host_size(&self, host_size: PluginEditorSize) -> PluginEditorSize {
        self.try_clamp_host_size(host_size)
            .unwrap_or(self.default_size)
    }

    /// Clamps a host-provided editor size to min/max constraints after validation.
    ///
    /// # Errors
    ///
    /// Returns [`PluginLayoutError`] when constraints or host size are invalid.
    pub fn try_clamp_host_size(
        &self,
        host_size: PluginEditorSize,
    ) -> Result<PluginEditorSize, PluginLayoutError> {
        self.validate()?;
        validate_size("host", host_size)?;
        let min = self.min_size.unwrap_or(self.default_size);
        let max = self.max_size.unwrap_or(self.default_size);
        Ok(PluginEditorSize::new(
            host_size.width.clamp(min.width, max.width),
            host_size.height.clamp(min.height, max.height),
        ))
    }

    /// Validates all plugin editor layout constraints.
    ///
    /// # Errors
    ///
    /// Returns [`PluginLayoutError`] when size, region, or generated parameter metadata is invalid.
    pub fn validate(&self) -> Result<(), PluginLayoutError> {
        validate_size("default", self.default_size)?;
        if let Some(min_size) = self.min_size {
            validate_size("min", min_size)?;
        }
        if let Some(max_size) = self.max_size {
            validate_size("max", max_size)?;
        }
        let min = self.min_size.unwrap_or(self.default_size);
        let max = self.max_size.unwrap_or(self.default_size);
        if min.width > max.width || min.height > max.height {
            return Err(PluginLayoutError::new(
                "plugin-layout.range.invalid",
                "minimum plugin editor size must not exceed maximum size",
            ));
        }
        for region in &self.graph_regions {
            validate_region(region.id(), region.x, region.y, region.width, region.height)?;
        }
        for region in &self.analyzer_regions {
            validate_region(region.id(), region.x, region.y, region.width, region.height)?;
        }
        for layout in &self.generated_parameters {
            validate_generated_parameters(layout)?;
        }
        Ok(())
    }

    /// Returns a graph region by ID.
    #[must_use]
    pub fn graph_region(&self, id: &str) -> Option<&GraphRegion> {
        self.graph_regions.iter().find(|region| region.id == id)
    }

    /// Returns an analyzer region by ID.
    #[must_use]
    pub fn analyzer_region(&self, id: &str) -> Option<&AnalyzerRegion> {
        self.analyzer_regions.iter().find(|region| region.id == id)
    }

    /// Returns a generated parameter layout by ID.
    #[must_use]
    pub fn generated_parameters(&self, id: &str) -> Option<&GeneratedParameterLayout> {
        self.generated_parameters
            .iter()
            .find(|layout| layout.id == id)
    }
}

fn validate_size(_label: &str, size: PluginEditorSize) -> Result<(), PluginLayoutError> {
    if size.width.is_finite() && size.height.is_finite() && size.width > 0.0 && size.height > 0.0 {
        Ok(())
    } else {
        Err(PluginLayoutError::new(
            "plugin-layout.size.invalid",
            "plugin editor sizes must be finite and greater than zero",
        ))
    }
}

fn validate_region(
    id: &str,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> Result<(), PluginLayoutError> {
    if id.trim().is_empty()
        || !x.is_finite()
        || !y.is_finite()
        || !width.is_finite()
        || !height.is_finite()
        || width <= 0.0
        || height <= 0.0
    {
        Err(PluginLayoutError::new(
            "plugin-layout.region.invalid",
            "plugin layout regions require non-empty IDs and finite positive sizes",
        ))
    } else {
        Ok(())
    }
}

fn validate_generated_parameters(
    layout: &GeneratedParameterLayout,
) -> Result<(), PluginLayoutError> {
    if layout.id.trim().is_empty()
        || layout.parameters.is_empty()
        || layout
            .parameters
            .iter()
            .any(|parameter| parameter.trim().is_empty())
    {
        Err(PluginLayoutError::new(
            "plugin-layout.parameters.invalid",
            "generated parameter layouts require non-empty IDs and parameter IDs",
        ))
    } else {
        Ok(())
    }
}

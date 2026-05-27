//! Deterministic paint layer records.

use crate::{Geometry, Transform};

/// RGBA color.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Color {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
    /// Alpha channel.
    pub a: u8,
}

impl Color {
    /// Creates an RGBA color.
    #[must_use]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

/// Stroke layer data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stroke {
    /// Stroke width.
    pub width: f32,
}

impl Stroke {
    /// Creates stroke data.
    #[must_use]
    pub const fn new(width: f32) -> Self {
        Self { width }
    }
}

/// Rounded rectangle layer data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoundedRect {
    /// Corner radius.
    pub radius: f32,
}

impl RoundedRect {
    /// Creates rounded rectangle data.
    #[must_use]
    pub const fn new(radius: f32) -> Self {
        Self { radius }
    }
}

/// Path layer data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathLayer(String);

impl PathLayer {
    /// Creates a path layer.
    #[must_use]
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    /// Returns the stable path string.
    #[must_use]
    pub fn stable_value(&self) -> &str {
        &self.0
    }
}

/// Gradient layer data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GradientLayer {
    /// Linear gradient.
    Linear,
}

impl GradientLayer {
    /// Creates a linear gradient record.
    #[must_use]
    pub const fn linear() -> Self {
        Self::Linear
    }
}

/// Shadow layer data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowLayer {
    /// Blur radius.
    pub blur: f32,
}

impl ShadowLayer {
    /// Creates a shadow layer.
    #[must_use]
    pub const fn new(blur: f32) -> Self {
        Self { blur }
    }
}

/// Glow layer data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlowLayer {
    /// Glow radius.
    pub radius: f32,
}

impl GlowLayer {
    /// Creates a glow layer.
    #[must_use]
    pub const fn new(radius: f32) -> Self {
        Self { radius }
    }
}

/// Text layer data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextLayer(String);

impl TextLayer {
    /// Creates a text layer.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    /// Returns the stable text string.
    #[must_use]
    pub fn stable_value(&self) -> &str {
        &self.0
    }
}

/// Paint layer validation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LayerValidationError {
    rule: String,
    message: String,
}

impl LayerValidationError {
    /// Creates a layer validation error.
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

/// Paint layer kind.
#[derive(Clone, Debug, PartialEq)]
pub enum LayerKind {
    /// Fill layer.
    Fill(Color),
    /// Stroke layer.
    Stroke(Stroke),
    /// Rounded rectangle layer.
    RoundedRect(RoundedRect),
    /// Path layer.
    Path(PathLayer),
    /// Gradient layer.
    Gradient(GradientLayer),
    /// Shadow layer.
    Shadow(ShadowLayer),
    /// Glow layer.
    Glow(GlowLayer),
    /// Opacity group.
    OpacityGroup(f32),
    /// Clip layer.
    Clip(Geometry),
    /// Transform layer.
    Transform(Transform),
    /// Text layer.
    Text(TextLayer),
    /// Image layer by compiled asset ID.
    Image(String),
    /// Vector layer by compiled asset ID.
    Vector(String),
    /// Control layer.
    Control(String),
    /// Custom surface layer.
    CustomSurface(String),
    /// Static cache layer.
    StaticCache(String),
    /// Live layer.
    LiveLayer(String),
}

impl LayerKind {
    /// Validates layer-kind payloads before deterministic export.
    ///
    /// # Errors
    ///
    /// Returns [`LayerValidationError`] when a layer contains non-renderable data.
    pub fn validate(&self) -> Result<(), LayerValidationError> {
        match self {
            Self::Fill(_) | Self::Gradient(GradientLayer::Linear) | Self::Text(_) => Ok(()),
            Self::Stroke(stroke) if stroke.width.is_finite() && stroke.width > 0.0 => Ok(()),
            Self::Stroke(_) => Err(LayerValidationError::new(
                "layer.stroke.invalid",
                "stroke width must be finite and greater than zero",
            )),
            Self::RoundedRect(rect) if rect.radius.is_finite() && rect.radius >= 0.0 => Ok(()),
            Self::RoundedRect(_) => Err(LayerValidationError::new(
                "layer.rounded-rect.invalid",
                "rounded rectangle radius must be finite and non-negative",
            )),
            Self::Path(path) if !path.stable_value().trim().is_empty() => Ok(()),
            Self::Path(_) => Err(LayerValidationError::new(
                "layer.path.invalid",
                "path layer data must not be empty",
            )),
            Self::Shadow(shadow) if shadow.blur.is_finite() && shadow.blur >= 0.0 => Ok(()),
            Self::Shadow(_) => Err(LayerValidationError::new(
                "layer.shadow.invalid",
                "shadow blur must be finite and non-negative",
            )),
            Self::Glow(glow) if glow.radius.is_finite() && glow.radius >= 0.0 => Ok(()),
            Self::Glow(_) => Err(LayerValidationError::new(
                "layer.glow.invalid",
                "glow radius must be finite and non-negative",
            )),
            Self::OpacityGroup(opacity) if opacity.is_finite() && (0.0..=1.0).contains(opacity) => {
                Ok(())
            }
            Self::OpacityGroup(_) => Err(LayerValidationError::new(
                "layer.opacity.invalid",
                "opacity must be finite and within 0.0..=1.0",
            )),
            Self::Clip(clip) if validate_geometry(*clip) => Ok(()),
            Self::Clip(_) => Err(LayerValidationError::new(
                "layer.clip.invalid",
                "clip geometry must be finite with non-negative dimensions",
            )),
            Self::Transform(transform)
                if transform.translate_x.is_finite() && transform.translate_y.is_finite() =>
            {
                Ok(())
            }
            Self::Transform(_) => Err(LayerValidationError::new(
                "layer.transform.invalid",
                "transform coordinates must be finite",
            )),
            Self::Image(id)
            | Self::Vector(id)
            | Self::Control(id)
            | Self::CustomSurface(id)
            | Self::StaticCache(id)
            | Self::LiveLayer(id)
                if !id.trim().is_empty() =>
            {
                Ok(())
            }
            Self::Image(_)
            | Self::Vector(_)
            | Self::Control(_)
            | Self::CustomSurface(_)
            | Self::StaticCache(_)
            | Self::LiveLayer(_) => Err(LayerValidationError::new(
                "layer.reference.invalid",
                "layer references must not be empty",
            )),
        }
    }

    /// Returns a stable key for deterministic export.
    #[must_use]
    pub fn stable_key(&self) -> String {
        match self {
            Self::Fill(color) => format!("fill({},{},{},{})", color.r, color.g, color.b, color.a),
            Self::Stroke(stroke) => format!("stroke({})", stroke.width),
            Self::RoundedRect(rect) => format!("rounded-rect({})", rect.radius),
            Self::Path(path) => format!("path({})", path.0),
            Self::Gradient(GradientLayer::Linear) => "gradient(linear)".to_string(),
            Self::Shadow(shadow) => format!("shadow({})", shadow.blur),
            Self::Glow(glow) => format!("glow({})", glow.radius),
            Self::OpacityGroup(opacity) => format!("opacity-group({opacity})"),
            Self::Clip(clip) => {
                format!("clip({},{},{},{})", clip.x, clip.y, clip.width, clip.height)
            }
            Self::Transform(transform) => {
                format!(
                    "transform({},{})",
                    transform.translate_x, transform.translate_y
                )
            }
            Self::Text(text) => format!("text({})", text.0),
            Self::Image(id) => format!("image({id})"),
            Self::Vector(id) => format!("vector({id})"),
            Self::Control(id) => format!("control({id})"),
            Self::CustomSurface(id) => format!("custom-surface({id})"),
            Self::StaticCache(id) => format!("static-cache({id})"),
            Self::LiveLayer(id) => format!("live-layer({id})"),
        }
    }
}

/// Paint layer.
#[derive(Clone, Debug, PartialEq)]
pub struct PaintLayer {
    key: String,
    order: i32,
    kind: LayerKind,
}

impl PaintLayer {
    /// Creates a paint layer.
    #[must_use]
    pub fn new(key: impl Into<String>, order: i32, kind: LayerKind) -> Self {
        Self {
            key: key.into(),
            order,
            kind,
        }
    }

    /// Returns a deterministic serialized key.
    #[must_use]
    pub fn stable_key(&self) -> String {
        format!("{}:{}:{}", self.order, self.key, self.kind.stable_key())
    }

    /// Validates the paint layer before deterministic export.
    ///
    /// # Errors
    ///
    /// Returns [`LayerValidationError`] when the layer key or payload is invalid.
    pub fn validate(&self) -> Result<(), LayerValidationError> {
        if is_valid_layer_key(&self.key) {
            self.kind.validate()
        } else {
            Err(LayerValidationError::new(
                "layer.key.invalid",
                "paint layer key must be non-empty and stable",
            ))
        }
    }

    /// Returns the paint layer key.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Returns the paint layer kind.
    #[must_use]
    pub const fn kind(&self) -> &LayerKind {
        &self.kind
    }
}

/// Paint layer stack.
#[derive(Clone, Debug, PartialEq)]
pub struct LayerStack {
    layers: Vec<PaintLayer>,
}

impl LayerStack {
    /// Creates an empty layer stack.
    #[must_use]
    pub const fn new() -> Self {
        Self { layers: Vec::new() }
    }

    /// Adds a paint layer.
    #[must_use]
    pub fn with_layer(mut self, layer: PaintLayer) -> Self {
        self.layers.push(layer);
        self
    }

    /// Returns layers in deterministic order.
    #[must_use]
    pub fn ordered_layers(&self) -> Vec<&PaintLayer> {
        let mut layers: Vec<_> = self.layers.iter().collect();
        layers.sort_by(|left, right| {
            left.order
                .cmp(&right.order)
                .then_with(|| left.key.cmp(&right.key))
        });
        layers
    }

    /// Returns deterministic layer keys.
    #[must_use]
    pub fn ordered_keys(&self) -> Vec<String> {
        self.ordered_layers()
            .into_iter()
            .map(PaintLayer::stable_key)
            .collect()
    }

    /// Validates all paint layers in insertion order.
    ///
    /// # Errors
    ///
    /// Returns [`LayerValidationError`] for the first invalid layer.
    pub fn validate(&self) -> Result<(), LayerValidationError> {
        for layer in &self.layers {
            layer.validate()?;
        }
        Ok(())
    }

    /// Serializes layers deterministically.
    #[must_use]
    pub fn serialize_stable(&self) -> String {
        self.ordered_keys().join("|")
    }
}

impl Default for LayerStack {
    fn default() -> Self {
        Self::new()
    }
}

fn is_valid_layer_key(value: &str) -> bool {
    !value.trim().is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
}

fn validate_geometry(geometry: Geometry) -> bool {
    geometry.x.is_finite()
        && geometry.y.is_finite()
        && geometry.width.is_finite()
        && geometry.height.is_finite()
        && geometry.width >= 0.0
        && geometry.height >= 0.0
}

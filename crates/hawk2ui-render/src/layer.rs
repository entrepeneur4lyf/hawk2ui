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

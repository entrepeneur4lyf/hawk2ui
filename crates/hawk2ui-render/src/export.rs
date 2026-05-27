//! Scene-to-paint command export.

use crate::{
    Color, Geometry, GlowLayer, GradientLayer, LayerKind, LayerStack, LayerValidationError,
    PathLayer, RoundedRect, ShadowLayer, Stroke, TextLayer, Transform,
};

/// Typed backend-neutral paint command payload.
#[derive(Clone, Debug, PartialEq)]
pub enum PaintCommandKind {
    /// Fill command.
    Fill(Color),
    /// Stroke command.
    Stroke(Stroke),
    /// Rounded rectangle command.
    RoundedRect(RoundedRect),
    /// Path command.
    Path(PathLayer),
    /// Gradient command.
    Gradient(GradientLayer),
    /// Shadow command.
    Shadow(ShadowLayer),
    /// Glow command.
    Glow(GlowLayer),
    /// Opacity group command.
    OpacityGroup(f32),
    /// Clip command.
    Clip(Geometry),
    /// Transform command.
    Transform(Transform),
    /// Text command.
    Text(TextLayer),
    /// Image command by compiled asset ID.
    Image(String),
    /// Vector command by compiled asset ID.
    Vector(String),
    /// Native control command.
    Control(String),
    /// Custom surface command.
    CustomSurface(String),
    /// Static cache command.
    StaticCache(String),
    /// Live layer command.
    LiveLayer(String),
}

/// Stable typed paint command.
#[derive(Clone, Debug, PartialEq)]
pub struct PaintCommand {
    layer_key: String,
    kind: PaintCommandKind,
    stable_key: String,
}

impl PaintCommand {
    /// Creates a paint command from a layer key and typed payload.
    #[must_use]
    pub fn new(layer_key: impl Into<String>, kind: PaintCommandKind) -> Self {
        let layer_key = layer_key.into();
        let stable_key = command_key(&layer_key, &kind);
        Self {
            layer_key,
            kind,
            stable_key,
        }
    }

    /// Returns the source layer key.
    #[must_use]
    pub fn layer_key(&self) -> &str {
        &self.layer_key
    }

    /// Returns the typed command payload.
    #[must_use]
    pub const fn kind(&self) -> &PaintCommandKind {
        &self.kind
    }

    /// Returns the deterministic diagnostic key.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.stable_key
    }
}

/// Stable paint command list.
#[derive(Clone, Debug, PartialEq)]
pub struct PaintCommandList {
    commands: Vec<PaintCommand>,
}

impl PaintCommandList {
    /// Creates a paint command list.
    #[must_use]
    pub fn new(commands: Vec<PaintCommand>) -> Self {
        Self { commands }
    }

    /// Returns command records.
    #[must_use]
    pub fn commands(&self) -> &[PaintCommand] {
        &self.commands
    }

    /// Serializes commands deterministically.
    #[must_use]
    pub fn serialize_stable(&self) -> String {
        self.commands
            .iter()
            .map(PaintCommand::as_str)
            .collect::<Vec<_>>()
            .join("|")
    }
}

/// Exports paint commands from a prepared layer stack.
///
/// # Errors
///
/// Returns [`LayerValidationError`] when the stack contains a non-renderable layer record.
pub fn export_paint_commands(stack: &LayerStack) -> Result<PaintCommandList, LayerValidationError> {
    stack.validate()?;
    Ok(PaintCommandList::new(
        stack
            .ordered_layers()
            .into_iter()
            .map(|layer| PaintCommand::new(layer.key(), paint_kind_from_layer(layer.kind())))
            .collect(),
    ))
}

fn paint_kind_from_layer(kind: &LayerKind) -> PaintCommandKind {
    match kind {
        LayerKind::Fill(color) => PaintCommandKind::Fill(*color),
        LayerKind::Stroke(stroke) => PaintCommandKind::Stroke(*stroke),
        LayerKind::RoundedRect(rect) => PaintCommandKind::RoundedRect(*rect),
        LayerKind::Path(path) => PaintCommandKind::Path(path.clone()),
        LayerKind::Gradient(gradient) => PaintCommandKind::Gradient(*gradient),
        LayerKind::Shadow(shadow) => PaintCommandKind::Shadow(*shadow),
        LayerKind::Glow(glow) => PaintCommandKind::Glow(*glow),
        LayerKind::OpacityGroup(opacity) => PaintCommandKind::OpacityGroup(*opacity),
        LayerKind::Clip(clip) => PaintCommandKind::Clip(*clip),
        LayerKind::Transform(transform) => PaintCommandKind::Transform(*transform),
        LayerKind::Text(text) => PaintCommandKind::Text(text.clone()),
        LayerKind::Image(id) => PaintCommandKind::Image(id.clone()),
        LayerKind::Vector(id) => PaintCommandKind::Vector(id.clone()),
        LayerKind::Control(id) => PaintCommandKind::Control(id.clone()),
        LayerKind::CustomSurface(id) => PaintCommandKind::CustomSurface(id.clone()),
        LayerKind::StaticCache(id) => PaintCommandKind::StaticCache(id.clone()),
        LayerKind::LiveLayer(id) => PaintCommandKind::LiveLayer(id.clone()),
    }
}

fn command_key(layer_key: &str, kind: &PaintCommandKind) -> String {
    match kind {
        PaintCommandKind::Fill(color) => {
            format!(
                "draw-fill:{layer_key}:{},{},{},{}",
                color.r, color.g, color.b, color.a
            )
        }
        PaintCommandKind::Stroke(stroke) => format!("draw-stroke:{layer_key}:{}", stroke.width),
        PaintCommandKind::RoundedRect(rect) => {
            format!("draw-rounded-rect:{layer_key}:{}", rect.radius)
        }
        PaintCommandKind::Path(path) => format!("draw-path:{layer_key}:{}", path.stable_value()),
        PaintCommandKind::Gradient(GradientLayer::Linear) => {
            format!("draw-gradient:{layer_key}:linear")
        }
        PaintCommandKind::Shadow(shadow) => format!("draw-shadow:{layer_key}:{}", shadow.blur),
        PaintCommandKind::Glow(glow) => format!("draw-glow:{layer_key}:{}", glow.radius),
        PaintCommandKind::OpacityGroup(opacity) => {
            format!("draw-opacity-group:{layer_key}:{opacity}")
        }
        PaintCommandKind::Clip(clip) => {
            format!(
                "draw-clip:{layer_key}:{},{},{},{}",
                clip.x, clip.y, clip.width, clip.height
            )
        }
        PaintCommandKind::Transform(transform) => {
            format!(
                "draw-transform:{layer_key}:{},{},{},{},{},{}",
                transform.scale_x,
                transform.skew_x,
                transform.skew_y,
                transform.scale_y,
                transform.translate_x,
                transform.translate_y
            )
        }
        PaintCommandKind::Text(text) => format!("draw-text:{layer_key}:{}", text.stable_value()),
        PaintCommandKind::Image(id) => format!("draw-image:{layer_key}:{id}"),
        PaintCommandKind::Vector(id) => format!("draw-vector:{layer_key}:{id}"),
        PaintCommandKind::Control(id) => format!("draw-control:{layer_key}:{id}"),
        PaintCommandKind::CustomSurface(id) => format!("draw-custom-surface:{layer_key}:{id}"),
        PaintCommandKind::StaticCache(id) => format!("draw-static-cache:{layer_key}:{id}"),
        PaintCommandKind::LiveLayer(id) => format!("draw-live-layer:{layer_key}:{id}"),
    }
}

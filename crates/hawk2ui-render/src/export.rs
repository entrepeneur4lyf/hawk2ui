//! Scene-to-paint command export.

use crate::{GradientLayer, LayerKind, LayerStack};

/// Stable paint command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaintCommand(String);

impl PaintCommand {
    /// Creates a paint command from a stable key.
    #[must_use]
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    /// Returns the command key.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Stable paint command list.
#[derive(Clone, Debug, Eq, PartialEq)]
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
#[must_use]
pub fn export_paint_commands(stack: &LayerStack) -> PaintCommandList {
    PaintCommandList::new(
        stack
            .ordered_layers()
            .into_iter()
            .map(|layer| PaintCommand::new(command_key(layer.key(), layer.kind())))
            .collect(),
    )
}

fn command_key(layer_key: &str, kind: &LayerKind) -> String {
    match kind {
        LayerKind::Fill(color) => {
            format!(
                "draw-fill:{layer_key}:{},{},{},{}",
                color.r, color.g, color.b, color.a
            )
        }
        LayerKind::Stroke(stroke) => format!("draw-stroke:{layer_key}:{}", stroke.width),
        LayerKind::RoundedRect(rect) => {
            format!("draw-rounded-rect:{layer_key}:{}", rect.radius)
        }
        LayerKind::Path(path) => format!("draw-path:{layer_key}:{}", path.stable_value()),
        LayerKind::Gradient(GradientLayer::Linear) => {
            format!("draw-gradient:{layer_key}:linear")
        }
        LayerKind::Shadow(shadow) => format!("draw-shadow:{layer_key}:{}", shadow.blur),
        LayerKind::Glow(glow) => format!("draw-glow:{layer_key}:{}", glow.radius),
        LayerKind::OpacityGroup(opacity) => format!("draw-opacity-group:{layer_key}:{opacity}"),
        LayerKind::Clip(clip) => {
            format!(
                "draw-clip:{layer_key}:{},{},{},{}",
                clip.x, clip.y, clip.width, clip.height
            )
        }
        LayerKind::Transform(transform) => {
            format!(
                "draw-transform:{layer_key}:{},{}",
                transform.translate_x, transform.translate_y
            )
        }
        LayerKind::Text(text) => format!("draw-text:{layer_key}:{}", text.stable_value()),
        LayerKind::Image(id) => format!("draw-image:{layer_key}:{id}"),
        LayerKind::Vector(id) => format!("draw-vector:{layer_key}:{id}"),
        LayerKind::Control(id) => format!("draw-control:{layer_key}:{id}"),
        LayerKind::CustomSurface(id) => format!("draw-custom-surface:{layer_key}:{id}"),
        LayerKind::StaticCache(id) => format!("draw-static-cache:{layer_key}:{id}"),
        LayerKind::LiveLayer(id) => format!("draw-live-layer:{layer_key}:{id}"),
    }
}

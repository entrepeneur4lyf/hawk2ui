//! Serializable runtime scene payloads carried by sealed artifacts.

use hawk2ui_layout::{FlexDirection, LayoutSizing, LayoutStyle, Viewport};
use hawk2ui_render::Color;
use serde::{Deserialize, Serialize};

use crate::{
    RuntimeSceneBridge, RuntimeSceneError, RuntimeSceneFrame, RuntimeTextVisual, RuntimeViewId,
    RuntimeViewNode, RuntimeViewTree, RuntimeVisual,
};

/// Maximum nesting depth accepted in a runtime scene payload.
///
/// Scene payloads are deserialized from caller-supplied JSON carried by sealed artifacts, so
/// traversal is bounded to avoid stack exhaustion from adversarial input. Mirrors
/// `hawk2ui_a11y`'s `A11Y_MAX_TREE_DEPTH`.
const RUNTIME_SCENE_MAX_DEPTH: usize = 256;

/// Serializable runtime scene payload decoded from a sealed artifact.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeScenePayload {
    viewport: RuntimeSceneViewport,
    root: RuntimeScenePayloadNode,
}

impl RuntimeScenePayload {
    /// Decodes a runtime scene payload from JSON.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeScenePayloadError`] when the payload is malformed.
    pub fn from_json(value: &serde_json::Value) -> Result<Self, RuntimeScenePayloadError> {
        enforce_value_depth(value)?;
        serde_json::from_value(value.clone()).map_err(|error| {
            RuntimeScenePayloadError::new(
                "runtime-scene.payload.parse-failed",
                format!("runtime scene payload could not be parsed: {error}"),
            )
        })
    }

    /// Builds a runtime scene frame from the payload.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeScenePayloadError`] when payload metrics or runtime scene data are invalid.
    pub fn build_frame(&self) -> Result<RuntimeSceneFrame, RuntimeScenePayloadError> {
        self.validate()?;
        let tree = self.root.to_runtime_tree()?;
        RuntimeSceneBridge::new(Viewport::new(self.viewport.width, self.viewport.height))
            .build(&tree)
            .map_err(RuntimeScenePayloadError::from)
    }

    fn validate(&self) -> Result<(), RuntimeScenePayloadError> {
        validate_positive_f32("viewport.width", self.viewport.width)?;
        validate_positive_f32("viewport.height", self.viewport.height)?;
        self.root.validate_at_depth(0)
    }
}

/// Serializable viewport dimensions for a runtime scene payload.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeSceneViewport {
    /// Viewport width in logical pixels.
    pub width: f32,
    /// Viewport height in logical pixels.
    pub height: f32,
}

/// Serializable node record for a runtime scene payload.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeScenePayloadNode {
    /// Runtime node ID.
    pub id: String,
    /// Fixed node width in logical pixels.
    pub width: f32,
    /// Fixed node height in logical pixels.
    pub height: f32,
    /// Visual payload.
    pub visual: RuntimeScenePayloadVisual,
    /// Child nodes in paint/layout order.
    #[serde(default)]
    pub children: Vec<RuntimeScenePayloadNode>,
}

impl RuntimeScenePayloadNode {
    fn to_runtime_tree(&self) -> Result<RuntimeViewTree, RuntimeScenePayloadError> {
        let root_id = RuntimeViewId::new(&self.id);
        let tree = RuntimeViewTree::new(self.to_runtime_node());
        append_children(tree, &root_id, &self.children, 1)
    }

    fn to_runtime_node(&self) -> RuntimeViewNode {
        RuntimeViewNode::new(
            RuntimeViewId::new(&self.id),
            LayoutStyle::flex_container(FlexDirection::Column)
                .with_size(LayoutSizing::fixed(self.width, self.height)),
            self.visual.to_runtime_visual(),
        )
    }

    fn validate_at_depth(&self, depth: usize) -> Result<(), RuntimeScenePayloadError> {
        if depth > RUNTIME_SCENE_MAX_DEPTH {
            return Err(too_deeply_nested_error());
        }
        if self.id.trim().is_empty() {
            return Err(RuntimeScenePayloadError::new(
                "runtime-scene.node.id-invalid",
                "runtime scene node IDs must not be empty",
            ));
        }
        validate_positive_f32("node.width", self.width)?;
        validate_positive_f32("node.height", self.height)?;
        self.visual.validate()?;
        for child in &self.children {
            child.validate_at_depth(depth + 1)?;
        }
        Ok(())
    }
}

/// Serializable visual payload for a runtime scene node.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeScenePayloadVisual {
    /// Optional fill color as RGBA bytes.
    pub fill: Option<[u8; 4]>,
    /// Optional text visual.
    pub text: Option<RuntimeScenePayloadText>,
}

impl RuntimeScenePayloadVisual {
    fn to_runtime_visual(&self) -> RuntimeVisual {
        if let Some(fill) = self.fill {
            RuntimeVisual::Fill(color_from_rgba(fill))
        } else if let Some(text) = &self.text {
            RuntimeVisual::Text(RuntimeTextVisual::new(
                &text.value,
                text.font_size,
                color_from_rgba(text.color),
            ))
        } else {
            RuntimeVisual::None
        }
    }

    fn validate(&self) -> Result<(), RuntimeScenePayloadError> {
        if self.fill.is_some() && self.text.is_some() {
            return Err(RuntimeScenePayloadError::new(
                "runtime-scene.visual.ambiguous",
                "runtime scene visuals must declare only one visual kind",
            ));
        }
        if let Some(text) = &self.text {
            text.validate()?;
        }
        Ok(())
    }
}

/// Serializable text visual payload.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeScenePayloadText {
    /// Text value.
    pub value: String,
    /// Font size in logical pixels.
    pub font_size: f32,
    /// Text color as RGBA bytes.
    pub color: [u8; 4],
}

impl RuntimeScenePayloadText {
    fn validate(&self) -> Result<(), RuntimeScenePayloadError> {
        if self.value.is_empty() {
            return Err(RuntimeScenePayloadError::new(
                "runtime-scene.text.empty",
                "runtime scene text values must not be empty",
            ));
        }
        validate_positive_f32("text.font_size", self.font_size)
    }
}

/// Runtime scene payload decoding/build error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeScenePayloadError {
    rule: String,
    message: String,
}

impl RuntimeScenePayloadError {
    /// Creates a runtime scene payload error.
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

impl From<RuntimeSceneError> for RuntimeScenePayloadError {
    fn from(error: RuntimeSceneError) -> Self {
        Self::new(
            "runtime-scene.build-failed",
            format!("runtime scene payload could not build a frame: {error:?}"),
        )
    }
}

fn append_children(
    mut tree: RuntimeViewTree,
    parent_id: &RuntimeViewId,
    children: &[RuntimeScenePayloadNode],
    depth: usize,
) -> Result<RuntimeViewTree, RuntimeScenePayloadError> {
    if depth > RUNTIME_SCENE_MAX_DEPTH {
        return Err(too_deeply_nested_error());
    }
    for child in children {
        let child_id = RuntimeViewId::new(&child.id);
        tree = tree
            .with_child(parent_id, child.to_runtime_node())
            .map_err(RuntimeScenePayloadError::from)?;
        tree = append_children(tree, &child_id, &child.children, depth + 1)?;
    }
    Ok(tree)
}

/// Rejects a scene-payload `Value` whose nesting depth exceeds [`RUNTIME_SCENE_MAX_DEPTH`].
///
/// Runs before `serde_json::from_value` (and the recursive `Value::clone` it follows), using an
/// explicit work stack so the depth check itself cannot overflow the stack on adversarial input.
fn enforce_value_depth(value: &serde_json::Value) -> Result<(), RuntimeScenePayloadError> {
    let mut stack = vec![(value, 1usize)];
    while let Some((node, depth)) = stack.pop() {
        if depth > RUNTIME_SCENE_MAX_DEPTH {
            return Err(too_deeply_nested_error());
        }
        match node {
            serde_json::Value::Array(items) => {
                for item in items {
                    stack.push((item, depth + 1));
                }
            }
            serde_json::Value::Object(entries) => {
                for entry in entries.values() {
                    stack.push((entry, depth + 1));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn too_deeply_nested_error() -> RuntimeScenePayloadError {
    RuntimeScenePayloadError::new(
        "runtime-scene.payload.too-deeply-nested",
        format!(
            "runtime scene payload nesting depth exceeds the maximum of {RUNTIME_SCENE_MAX_DEPTH}"
        ),
    )
}

fn validate_positive_f32(name: &str, value: f32) -> Result<(), RuntimeScenePayloadError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(RuntimeScenePayloadError::new(
            "runtime-scene.metric.invalid",
            format!("runtime scene `{name}` must be finite and greater than zero"),
        ))
    }
}

fn color_from_rgba([r, g, b, a]: [u8; 4]) -> Color {
    Color::rgba(r, g, b, a)
}

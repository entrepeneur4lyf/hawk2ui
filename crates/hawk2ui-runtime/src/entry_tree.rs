//! Parsing and view-tree construction for an entry-script node tree.
//!
//! An author's entry/mount script returns a JSON node tree of views and text;
//! this module parses and validates that tree into an [`EntryNode`] and
//! converts it into a [`RuntimeViewTree`] for layout and rendering. It is pure
//! data transformation — no script engine — so both the desktop host (driven by
//! the CLI) and the plugin editor can share one conversion from a script's
//! serialized output to a renderable scene. Driving the script to produce that
//! JSON (a `ScriptBackend` plus the host's bootstrap convention) is the
//! consumer's concern, not this module's.

use std::collections::BTreeSet;

use hawk2ui_layout::{BoxEdges, FlexDirection, LayoutSizing, LayoutStyle, LayoutValue};
use hawk2ui_render::Color;

use crate::view::{
    RuntimeSceneError, RuntimeTextVisual, RuntimeViewId, RuntimeViewNode, RuntimeViewTree,
    RuntimeVisual,
};

/// Kind of an [`EntryNode`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntryNodeKind {
    /// A container view node.
    View,
    /// A leaf text node.
    Text,
}

/// Optional styling props parsed from an entry node's `props` object.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EntryNodeProps {
    /// Background fill color (view nodes).
    pub background_color: Option<Color>,
    /// Text color (text nodes).
    pub text_color: Option<Color>,
    /// Font size in points (text nodes).
    pub font_size: Option<f32>,
    /// Fixed width in points.
    pub width: Option<f32>,
    /// Fixed height in points.
    pub height: Option<f32>,
    /// Container padding in points (view nodes).
    pub padding: Option<f32>,
    /// Gap between children in points (view nodes).
    pub gap: Option<f32>,
}

/// A parsed, validated node from an entry-script tree.
#[derive(Clone, Debug, PartialEq)]
pub struct EntryNode {
    id: String,
    kind: EntryNodeKind,
    text: Option<String>,
    props: EntryNodeProps,
    children: Vec<Self>,
}

impl EntryNode {
    /// Creates a view node with `children`.
    #[must_use]
    pub fn view(id: impl Into<String>, children: Vec<Self>) -> Self {
        Self {
            id: id.into(),
            kind: EntryNodeKind::View,
            text: None,
            props: EntryNodeProps::default(),
            children,
        }
    }

    /// Creates a text node.
    #[must_use]
    pub fn text(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind: EntryNodeKind::Text,
            text: Some(text.into()),
            props: EntryNodeProps::default(),
            children: Vec::new(),
        }
    }

    /// Sets styling props.
    #[must_use]
    pub fn with_props(mut self, props: EntryNodeProps) -> Self {
        self.props = props;
        self
    }

    /// The node's stable id.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Parses and validates a node tree from an entry script's serialized JSON
    /// return value.
    ///
    /// # Errors
    ///
    /// Returns a message when the JSON is malformed, a node is malformed, or two
    /// nodes share an id.
    pub fn from_tree_json(value: &str) -> Result<Self, String> {
        let value: serde_json::Value = serde_json::from_str(value)
            .map_err(|error| format!("entry tree result is not valid JSON: {error}"))?;
        let root = Self::from_json(&value)?;
        root.validate_unique_ids()?;
        Ok(root)
    }

    /// Converts the tree into a [`RuntimeViewTree`] sized to `width` x `height`
    /// points, applying the default container/text styling.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeSceneError`] when the resulting tree is structurally
    /// invalid (for example, a duplicate node id surfaced during assembly).
    pub fn to_view_tree(
        &self,
        width: f32,
        height: f32,
    ) -> Result<RuntimeViewTree, RuntimeSceneError> {
        let content_width = (width - 48.0).max(1.0);
        let root_id = RuntimeViewId::new(self.id.clone());
        let root = runtime_node(self, width, height, true);
        append_children(
            RuntimeViewTree::new(root),
            &root_id,
            &self.children,
            content_width,
        )
    }

    fn from_json(value: &serde_json::Value) -> Result<Self, String> {
        let id = non_empty_json_string(value, "id")?;
        let props = EntryNodeProps::from_json(value.get("props"))?;
        let raw_kind = value
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| {
                if value.get("text").is_some() {
                    "text"
                } else {
                    "view"
                }
            });
        match raw_kind {
            "view" => {
                let children = json_children(value)?
                    .iter()
                    .map(Self::from_json)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Self::view(id, children).with_props(props))
            }
            "text" => {
                let text = non_empty_json_string(value, "text")?;
                if value.get("children").is_some() {
                    return Err(format!("text node '{id}' must not declare children"));
                }
                Ok(Self::text(id, text).with_props(props))
            }
            _ => Err(format!("node '{id}' uses unsupported type '{raw_kind}'")),
        }
    }

    fn validate_unique_ids(&self) -> Result<(), String> {
        let mut ids = BTreeSet::new();
        collect_ids(self, &mut ids)
    }
}

impl EntryNodeProps {
    fn from_json(value: Option<&serde_json::Value>) -> Result<Self, String> {
        let Some(value) = value else {
            return Ok(Self::default());
        };
        let serde_json::Value::Object(props) = value else {
            return Err("field 'props' must be an object".to_string());
        };
        let mut result = Self::default();
        for (name, value) in props {
            match name.as_str() {
                "backgroundColor" => {
                    result.background_color = Some(json_color_prop(value, name)?);
                }
                "color" => {
                    result.text_color = Some(json_color_prop(value, name)?);
                }
                "fontSize" => {
                    result.font_size = Some(json_positive_number_prop(value, name)?);
                }
                "width" => {
                    result.width = Some(json_positive_number_prop(value, name)?);
                }
                "height" => {
                    result.height = Some(json_positive_number_prop(value, name)?);
                }
                "padding" => {
                    result.padding = Some(json_non_negative_number_prop(value, name)?);
                }
                "gap" => {
                    result.gap = Some(json_non_negative_number_prop(value, name)?);
                }
                _ => return Err(format!("unsupported native node prop '{name}'")),
            }
        }
        Ok(result)
    }
}

fn runtime_node(node: &EntryNode, width: f32, height: f32, is_root: bool) -> RuntimeViewNode {
    let node_width = node.props.width.unwrap_or(width);
    let node_height = node.props.height.unwrap_or(height);
    let layout_style = match node.kind {
        EntryNodeKind::View => LayoutStyle::flex_container(FlexDirection::Column)
            .with_size(LayoutSizing::fixed(node_width, node_height))
            .with_padding(BoxEdges::all(LayoutValue::px(
                node.props
                    .padding
                    .unwrap_or(if is_root { 24.0 } else { 0.0 }),
            )))
            .with_gap(LayoutValue::px(node.props.gap.unwrap_or(12.0))),
        EntryNodeKind::Text => LayoutStyle::flex_container(FlexDirection::Row)
            .with_size(LayoutSizing::fixed(node_width, node_height)),
    };
    let visual = match node.kind {
        EntryNodeKind::View => RuntimeVisual::Fill(if is_root {
            node.props
                .background_color
                .unwrap_or(Color::rgba(11, 12, 18, 255))
        } else {
            node.props
                .background_color
                .unwrap_or(Color::rgba(20, 22, 31, 255))
        }),
        EntryNodeKind::Text => RuntimeVisual::Text(RuntimeTextVisual::new(
            node.text.clone().unwrap_or_default(),
            node.props.font_size.unwrap_or(20.0),
            node.props
                .text_color
                .unwrap_or(Color::rgba(241, 245, 249, 255)),
        )),
    };
    RuntimeViewNode::new(RuntimeViewId::new(node.id.clone()), layout_style, visual)
}

fn append_children(
    mut tree: RuntimeViewTree,
    parent_id: &RuntimeViewId,
    children: &[EntryNode],
    content_width: f32,
) -> Result<RuntimeViewTree, RuntimeSceneError> {
    for child in children {
        let child_id = RuntimeViewId::new(child.id.clone());
        let child_height = node_height(child);
        let child_node = runtime_node(child, content_width, child_height, false);
        tree = tree.with_child(parent_id, child_node)?;
        tree = append_children(tree, &child_id, &child.children, content_width)?;
    }
    Ok(tree)
}

fn node_height(node: &EntryNode) -> f32 {
    if let Some(height) = node.props.height {
        return height;
    }
    match node.kind {
        EntryNodeKind::Text => 32.0,
        EntryNodeKind::View => {
            let children_height: f32 = node.children.iter().map(node_height).sum();
            let gap_count =
                u16::try_from(node.children.len().saturating_sub(1)).unwrap_or(u16::MAX);
            let gaps = f32::from(gap_count) * node.props.gap.unwrap_or(12.0);
            (children_height + gaps).max(32.0)
        }
    }
}

fn collect_ids<'a>(node: &'a EntryNode, ids: &mut BTreeSet<&'a str>) -> Result<(), String> {
    if !ids.insert(node.id.as_str()) {
        return Err(format!("duplicate native app node id '{}'", node.id));
    }
    for child in &node.children {
        collect_ids(child, ids)?;
    }
    Ok(())
}

fn json_children(value: &serde_json::Value) -> Result<&[serde_json::Value], String> {
    match value.get("children") {
        Some(serde_json::Value::Array(children)) => Ok(children.as_slice()),
        Some(_) => Err("field 'children' must be an array".to_string()),
        None => Ok(&[]),
    }
}

fn json_color_prop(value: &serde_json::Value, name: &str) -> Result<Color, String> {
    let value = value
        .as_str()
        .ok_or_else(|| format!("prop '{name}' must be a CSS hex color string"))?;
    parse_hex_color(value)
        .ok_or_else(|| format!("prop '{name}' must use #RRGGBB or #RRGGBBAA hex color syntax"))
}

fn parse_hex_color(value: &str) -> Option<Color> {
    let hex = value.strip_prefix('#')?;
    let (r, g, b, a) = match hex.len() {
        6 => (
            u8::from_str_radix(&hex[0..2], 16).ok()?,
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..6], 16).ok()?,
            255,
        ),
        8 => (
            u8::from_str_radix(&hex[0..2], 16).ok()?,
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..6], 16).ok()?,
            u8::from_str_radix(&hex[6..8], 16).ok()?,
        ),
        _ => return None,
    };
    Some(Color::rgba(r, g, b, a))
}

fn json_positive_number_prop(value: &serde_json::Value, name: &str) -> Result<f32, String> {
    let number = json_number_prop(value, name)?;
    if number <= 0.0 {
        return Err(format!("prop '{name}' must be greater than zero"));
    }
    Ok(number)
}

fn json_non_negative_number_prop(value: &serde_json::Value, name: &str) -> Result<f32, String> {
    let number = json_number_prop(value, name)?;
    if number < 0.0 {
        return Err(format!("prop '{name}' must not be negative"));
    }
    Ok(number)
}

fn json_number_prop(value: &serde_json::Value, name: &str) -> Result<f32, String> {
    let serde_json::Value::Number(number) = value else {
        return Err(format!("prop '{name}' must be a number"));
    };
    let Some(value) = number.as_f64() else {
        return Err(format!("prop '{name}' cannot be represented as a number"));
    };
    #[allow(clippy::cast_possible_truncation)]
    let parsed = value as f32;
    if !parsed.is_finite() {
        return Err(format!("prop '{name}' must be finite"));
    }
    Ok(parsed)
}

fn non_empty_json_string(value: &serde_json::Value, key: &str) -> Result<String, String> {
    let value = value
        .get(key)
        .ok_or_else(|| format!("node is missing required '{key}' field"))?
        .as_str()
        .ok_or_else(|| format!("field '{key}' must be a string"))?
        .trim();
    if value.is_empty() {
        Err(format!("field '{key}' must not be empty"))
    } else {
        Ok(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_validates_and_converts_an_entry_tree() {
        let root = EntryNode::from_tree_json(
            r#"{
                "id": "root",
                "type": "view",
                "children": [
                    { "id": "title", "type": "text", "text": "Hello" }
                ]
            }"#,
        )
        .expect("entry tree parses");
        assert_eq!(root.id(), "root");

        // The validated tree converts to a renderable view tree without error.
        root.to_view_tree(640.0, 480.0)
            .expect("view tree builds from the parsed entry tree");
    }

    #[test]
    fn rejects_duplicate_node_ids() {
        let error = EntryNode::from_tree_json(
            r#"{
                "id": "dup",
                "type": "view",
                "children": [{ "id": "dup", "type": "text", "text": "x" }]
            }"#,
        )
        .expect_err("duplicate ids must fail");
        assert!(
            error.contains("duplicate native app node id 'dup'"),
            "{error}"
        );
    }

    #[test]
    fn rejects_text_node_with_children() {
        let error = EntryNode::from_tree_json(
            r#"{ "id": "t", "type": "text", "text": "x", "children": [] }"#,
        )
        .expect_err("text with children must fail");
        assert!(error.contains("must not declare children"), "{error}");
    }
}

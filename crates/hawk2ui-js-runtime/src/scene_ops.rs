use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::JsRuntimeError;

/// Native node kinds accepted by the JavaScript scene operation bridge.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SceneNodeKind {
    /// Generic layout container.
    View,
    /// Text rendering node.
    Text,
    /// Pressable button node.
    Button,
    /// Editable input node.
    Input,
    /// Image asset node.
    Image,
    /// Vector asset node.
    Vector,
    /// Custom drawing surface.
    CustomSurface,
    /// Scrollable container.
    ScrollView,
    /// Collection/list container.
    List,
}

/// Serializable primitive value accepted by scene property operations.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum SceneValue {
    /// Null value.
    Null,
    /// Boolean value.
    Bool(bool),
    /// Numeric value.
    Number(f64),
    /// String value.
    String(String),
}

/// Native measurement request retained until the host layout/render side answers it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SceneMeasurementRequest {
    /// Stable native node id to measure.
    pub node_id: String,
    /// Stable request id used to correlate native measurement results.
    pub request: String,
}

/// One JavaScript-originated mutation against the native scene tree.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum SceneOp {
    /// Creates a native node.
    CreateNode {
        /// Stable native node id.
        id: String,
        /// Native node kind.
        kind: SceneNodeKind,
    },
    /// Creates a text node.
    CreateText {
        /// Stable native node id.
        id: String,
        /// Text contents.
        text: String,
    },
    /// Sets a node property.
    SetProp {
        /// Stable native node id.
        id: String,
        /// Property name.
        name: String,
        /// Property value.
        value: SceneValue,
    },
    /// Sets a node style property.
    SetStyle {
        /// Stable native node id.
        id: String,
        /// Style property name.
        name: String,
        /// Style value.
        value: SceneValue,
    },
    /// Sets accessibility semantics for a node.
    SetAccessibility {
        /// Stable native node id.
        id: String,
        /// Accessibility role.
        role: Option<String>,
        /// Accessible label/name.
        label: Option<String>,
        /// Accessible description.
        description: Option<String>,
        /// Accessible value text or scalar.
        value: Option<SceneValue>,
        /// Disabled state.
        disabled: Option<bool>,
        /// Checked state.
        checked: Option<bool>,
        /// Pressed state.
        pressed: Option<bool>,
        /// Focused state.
        focused: Option<bool>,
    },
    /// Requests native focus for a node.
    FocusNode {
        /// Stable native node id.
        id: String,
    },
    /// Requests native layout measurement for a node.
    MeasureNode {
        /// Stable native node id.
        id: String,
        /// Stable measurement request id.
        request: String,
    },
    /// Appends a child node.
    AppendChild {
        /// Parent node id.
        parent: String,
        /// Child node id.
        child: String,
    },
    /// Inserts a child before an existing sibling.
    InsertBefore {
        /// Parent node id.
        parent: String,
        /// Child node id.
        child: String,
        /// Sibling node id.
        before: String,
    },
    /// Removes a child from a parent.
    RemoveChild {
        /// Parent node id.
        parent: String,
        /// Child node id.
        child: String,
    },
    /// Replaces text contents.
    ReplaceText {
        /// Stable native node id.
        id: String,
        /// Text contents.
        text: String,
    },
    /// Registers an event handler for a node.
    RegisterEvent {
        /// Stable native node id.
        id: String,
        /// Native event name.
        event: String,
        /// JavaScript handler id.
        handler: String,
    },
    /// Unregisters an event handler for a node.
    UnregisterEvent {
        /// Stable native node id.
        id: String,
        /// Native event name.
        event: String,
    },
    /// Marks the end of a transaction batch.
    Commit,
    /// Disposes a node and all descendants.
    DisposeSubtree {
        /// Stable native node id.
        id: String,
    },
}

/// Transactional JavaScript-originated scene operation batch.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct SceneOpBatch {
    /// Ordered scene operations.
    pub ops: Vec<SceneOp>,
}

impl SceneOpBatch {
    /// Creates a scene operation batch.
    #[must_use]
    pub fn new(ops: impl IntoIterator<Item = SceneOp>) -> Self {
        Self {
            ops: ops.into_iter().collect(),
        }
    }

    /// Validates structural invariants before the batch reaches native scene state.
    ///
    /// # Errors
    ///
    /// Returns [`JsRuntimeError`] when ids, names, handlers, or creation operations
    /// are structurally invalid.
    pub fn validate(&self) -> Result<(), JsRuntimeError> {
        let mut created = BTreeSet::new();

        for op in &self.ops {
            match op {
                SceneOp::CreateNode { id, .. } | SceneOp::CreateText { id, .. } => {
                    validate_id(id, "node id")?;
                    if !created.insert(id.as_str()) {
                        return Err(scene_protocol_error(format!(
                            "scene batch creates duplicate node id: {id}"
                        )));
                    }
                }
                SceneOp::SetProp { id, name, .. } | SceneOp::SetStyle { id, name, .. } => {
                    validate_id(id, "node id")?;
                    validate_name(name, "property name")?;
                }
                SceneOp::SetAccessibility {
                    id,
                    role,
                    label,
                    description,
                    ..
                } => {
                    validate_id(id, "node id")?;
                    validate_optional_name(role.as_deref(), "accessibility role")?;
                    validate_optional_name(label.as_deref(), "accessibility label")?;
                    validate_optional_name(description.as_deref(), "accessibility description")?;
                }
                SceneOp::AppendChild { parent, child } | SceneOp::RemoveChild { parent, child } => {
                    validate_id(parent, "parent id")?;
                    validate_id(child, "child id")?;
                }
                SceneOp::InsertBefore {
                    parent,
                    child,
                    before,
                } => {
                    validate_id(parent, "parent id")?;
                    validate_id(child, "child id")?;
                    validate_id(before, "sibling id")?;
                }
                SceneOp::ReplaceText { id, .. }
                | SceneOp::FocusNode { id }
                | SceneOp::DisposeSubtree { id } => {
                    validate_id(id, "node id")?;
                }
                SceneOp::MeasureNode { id, request } => {
                    validate_id(id, "node id")?;
                    validate_name(request, "measure request")?;
                }
                SceneOp::RegisterEvent { id, event, handler } => {
                    validate_id(id, "node id")?;
                    validate_name(event, "event name")?;
                    validate_name(handler, "handler id")?;
                }
                SceneOp::UnregisterEvent { id, event } => {
                    validate_id(id, "node id")?;
                    validate_name(event, "event name")?;
                }
                SceneOp::Commit => {}
            }
        }

        Ok(())
    }
}

fn validate_id(value: &str, label: &str) -> Result<(), JsRuntimeError> {
    if value.trim().is_empty() {
        return Err(scene_protocol_error(format!(
            "scene {label} must not be empty"
        )));
    }
    Ok(())
}

fn validate_name(value: &str, label: &str) -> Result<(), JsRuntimeError> {
    if value.trim().is_empty() {
        return Err(scene_protocol_error(format!(
            "scene {label} must not be empty"
        )));
    }
    Ok(())
}

fn validate_optional_name(value: Option<&str>, label: &str) -> Result<(), JsRuntimeError> {
    if let Some(value) = value {
        validate_name(value, label)?;
    }
    Ok(())
}

fn scene_protocol_error(message: impl Into<String>) -> JsRuntimeError {
    JsRuntimeError::new("js-runtime.scene-op.invalid", message)
}

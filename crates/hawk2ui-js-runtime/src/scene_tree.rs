use std::collections::{BTreeMap, BTreeSet};

use hawk2ui_layout::{FlexDirection, LayoutStyle};
use hawk2ui_render::Color;
use hawk2ui_runtime::{
    RuntimeSceneError, RuntimeTextVisual, RuntimeViewId, RuntimeViewNode, RuntimeViewTree,
    RuntimeVisual,
};

use crate::{
    JsRuntimeError, SceneMeasurementRequest, SceneNodeKind, SceneOp, SceneOpBatch, SceneValue,
};

/// Accessibility semantics retained for a JavaScript-originated scene node.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SceneAccessibilitySemantics {
    /// Accessibility role.
    pub role: Option<String>,
    /// Accessible label/name.
    pub label: Option<String>,
    /// Accessible description.
    pub description: Option<String>,
    /// Accessible value.
    pub value: Option<SceneValue>,
    /// Disabled state.
    pub disabled: Option<bool>,
    /// Checked state.
    pub checked: Option<bool>,
    /// Pressed state.
    pub pressed: Option<bool>,
    /// Focused state.
    pub focused: Option<bool>,
}

impl SceneAccessibilitySemantics {
    fn is_empty(&self) -> bool {
        self.role.is_none()
            && self.label.is_none()
            && self.description.is_none()
            && self.value.is_none()
            && self.disabled.is_none()
            && self.checked.is_none()
            && self.pressed.is_none()
            && self.focused.is_none()
    }
}

/// Applies JavaScript-originated scene operation batches to a retained runtime tree.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuntimeSceneOpAdapter {
    tree: Option<RuntimeViewTree>,
    pending_nodes: BTreeMap<String, RuntimeViewNode>,
    handlers: BTreeMap<(String, String), String>,
    accessibility: BTreeMap<String, SceneAccessibilitySemantics>,
    focused_node: Option<String>,
    measurements: Vec<SceneMeasurementRequest>,
}

impl RuntimeSceneOpAdapter {
    /// Applies one validated scene operation batch transactionally.
    ///
    /// # Errors
    ///
    /// Returns [`JsRuntimeError`] when the batch is structurally invalid, references missing
    /// nodes, duplicates existing nodes, or cannot be applied to the retained runtime tree.
    pub fn apply_batch(&mut self, batch: &SceneOpBatch) -> Result<(), JsRuntimeError> {
        batch.validate()?;
        let mut staged = self.clone();
        for op in &batch.ops {
            staged.apply_op(op)?;
        }
        *self = staged;
        Ok(())
    }

    /// Returns the retained runtime tree if the scene has a root.
    #[must_use]
    pub const fn runtime_tree(&self) -> Option<&RuntimeViewTree> {
        self.tree.as_ref()
    }

    /// Returns the JavaScript handler id registered for a node event.
    #[must_use]
    pub fn event_handler(&self, node_id: &str, event: &str) -> Option<&str> {
        self.handlers
            .get(&(node_id.to_owned(), event.to_owned()))
            .map(String::as_str)
    }

    /// Returns retained accessibility semantics for a node.
    #[must_use]
    pub fn accessibility_semantics(&self, node_id: &str) -> Option<&SceneAccessibilitySemantics> {
        self.accessibility.get(node_id)
    }

    /// Returns the node currently requested for native focus.
    #[must_use]
    pub fn focused_node(&self) -> Option<&str> {
        self.focused_node.as_deref()
    }

    /// Returns retained native measurement requests in commit order.
    #[must_use]
    pub fn measurement_requests(&self) -> &[SceneMeasurementRequest] {
        &self.measurements
    }

    fn apply_op(&mut self, op: &SceneOp) -> Result<(), JsRuntimeError> {
        match op {
            SceneOp::CreateNode { id, kind } => self.create_node(id, kind, None),
            SceneOp::CreateText { id, text } => {
                self.create_node(id, &SceneNodeKind::Text, Some(text.as_str()))
            }
            SceneOp::SetProp { id, name, value } => self.set_prop(id, name, value),
            SceneOp::SetStyle { id, .. } => self.invalidate_existing_node(id),
            SceneOp::SetAccessibility {
                id,
                role,
                label,
                description,
                value,
                disabled,
                checked,
                pressed,
                focused,
            } => self.set_accessibility(
                id,
                SceneAccessibilitySemantics {
                    role: role.clone(),
                    label: label.clone(),
                    description: description.clone(),
                    value: value.clone(),
                    disabled: *disabled,
                    checked: *checked,
                    pressed: *pressed,
                    focused: *focused,
                },
            ),
            SceneOp::FocusNode { id } => self.focus_node(id),
            SceneOp::MeasureNode { id, request } => self.measure_node(id, request),
            SceneOp::AppendChild { parent, child } => self.append_child(parent, child),
            SceneOp::InsertBefore {
                parent,
                child,
                before,
            } => self.insert_child_before(parent, child, before),
            SceneOp::RemoveChild { parent, child } => self.detach_child(parent, child),
            SceneOp::DisposeSubtree { id } => self.remove_subtree(id, true),
            SceneOp::ReplaceText { id, text } => self.update_text(id, text),
            SceneOp::RegisterEvent { id, event, handler } => {
                self.register_event(id, event, handler)
            }
            SceneOp::UnregisterEvent { id, event } => {
                self.handlers.remove(&(id.clone(), event.clone()));
                Ok(())
            }
            SceneOp::Commit => Ok(()),
        }
    }

    fn create_node(
        &mut self,
        id: &str,
        kind: &SceneNodeKind,
        text: Option<&str>,
    ) -> Result<(), JsRuntimeError> {
        self.ensure_new_node_id(id)?;
        let node = runtime_node(id, kind, text);
        if self.tree.is_none() {
            self.tree = Some(
                RuntimeViewTree::new(node)
                    .invalidate(&RuntimeViewId::new(id))
                    .map_err(scene_tree_error)?,
            );
        } else {
            self.pending_nodes.insert(id.to_owned(), node);
        }
        Ok(())
    }

    fn set_prop(&mut self, id: &str, name: &str, value: &SceneValue) -> Result<(), JsRuntimeError> {
        match (name, value) {
            ("text", SceneValue::String(text)) => self.update_text(id, text),
            ("value", value) => self.update_text(id, &scene_value_text(value)),
            ("visible", SceneValue::Bool(visible)) => self.update_visibility(id, *visible),
            _ => self.invalidate_existing_node(id),
        }
    }

    fn append_child(&mut self, parent: &str, child: &str) -> Result<(), JsRuntimeError> {
        let parent_id = RuntimeViewId::new(parent);
        let child_id = RuntimeViewId::new(child);
        let Some(child_node) = self.pending_nodes.remove(child) else {
            let tree = self
                .take_tree()?
                .append_existing_child(&parent_id, &child_id)
                .map_err(scene_tree_error)?;
            self.tree = Some(tree);
            return Ok(());
        };
        let tree = self.take_tree()?;
        let tree = tree
            .with_child(&parent_id, child_node)
            .map_err(scene_tree_error)?
            .invalidate(&parent_id)
            .map_err(scene_tree_error)?
            .invalidate(&child_id)
            .map_err(scene_tree_error)?;
        self.tree = Some(tree);
        Ok(())
    }

    fn insert_child_before(
        &mut self,
        parent: &str,
        child: &str,
        before: &str,
    ) -> Result<(), JsRuntimeError> {
        let parent_id = RuntimeViewId::new(parent);
        let child_id = RuntimeViewId::new(child);
        let before_id = RuntimeViewId::new(before);
        let Some(child_node) = self.pending_nodes.remove(child) else {
            let tree = self
                .take_tree()?
                .insert_existing_child_before(&parent_id, &child_id, &before_id)
                .map_err(scene_tree_error)?;
            self.tree = Some(tree);
            return Ok(());
        };
        let tree = self.take_tree()?;
        let tree = tree
            .insert_child_before(&parent_id, &before_id, child_node)
            .map_err(scene_tree_error)?
            .invalidate(&parent_id)
            .map_err(scene_tree_error)?
            .invalidate(&child_id)
            .map_err(scene_tree_error)?;
        self.tree = Some(tree);
        Ok(())
    }

    fn update_text(&mut self, id: &str, text: &str) -> Result<(), JsRuntimeError> {
        if let Some(node) = self.pending_nodes.get_mut(id) {
            replace_pending_visual(node, text_visual_for(node.visual(), text));
            return Ok(());
        }
        let id = RuntimeViewId::new(id);
        let current_visual = self
            .tree
            .as_ref()
            .and_then(|tree| tree.node(&id))
            .map(RuntimeViewNode::visual)
            .ok_or_else(|| missing_node_error(id.as_str()))?;
        let visual = text_visual_for(current_visual, text);
        let tree = self
            .take_tree()?
            .update_visual(&id, visual)
            .map_err(scene_tree_error)?;
        self.tree = Some(tree);
        Ok(())
    }

    fn update_visibility(&mut self, id: &str, visible: bool) -> Result<(), JsRuntimeError> {
        let id = RuntimeViewId::new(id);
        let tree = self
            .take_tree()?
            .update_visibility(&id, visible)
            .map_err(scene_tree_error)?;
        self.tree = Some(tree);
        Ok(())
    }

    fn set_accessibility(
        &mut self,
        id: &str,
        semantics: SceneAccessibilitySemantics,
    ) -> Result<(), JsRuntimeError> {
        self.ensure_node_exists(id)?;
        if semantics.is_empty() {
            self.accessibility.remove(id);
        } else {
            self.accessibility.insert(id.to_owned(), semantics);
        }
        self.invalidate_existing_node(id)
    }

    fn focus_node(&mut self, id: &str) -> Result<(), JsRuntimeError> {
        self.ensure_node_exists(id)?;
        self.focused_node = Some(id.to_owned());
        self.invalidate_existing_node(id)
    }

    fn measure_node(&mut self, id: &str, request: &str) -> Result<(), JsRuntimeError> {
        self.ensure_node_exists(id)?;
        self.measurements.push(SceneMeasurementRequest {
            node_id: id.to_owned(),
            request: request.to_owned(),
        });
        Ok(())
    }

    fn invalidate_existing_node(&mut self, id: &str) -> Result<(), JsRuntimeError> {
        if self.pending_nodes.contains_key(id) {
            return Ok(());
        }
        let id = RuntimeViewId::new(id);
        let tree = self
            .take_tree()?
            .invalidate(&id)
            .map_err(scene_tree_error)?;
        self.tree = Some(tree);
        Ok(())
    }

    fn register_event(
        &mut self,
        id: &str,
        event: &str,
        handler: &str,
    ) -> Result<(), JsRuntimeError> {
        self.ensure_node_exists(id)?;
        self.handlers
            .insert((id.to_owned(), event.to_owned()), handler.to_owned());
        Ok(())
    }

    fn detach_child(&mut self, parent: &str, child: &str) -> Result<(), JsRuntimeError> {
        let parent_id = RuntimeViewId::new(parent);
        let child_id = RuntimeViewId::new(child);
        if self.pending_nodes.contains_key(child) {
            return Err(missing_node_error(child));
        }
        let tree = self
            .take_tree()?
            .detach_child(&parent_id, &child_id)
            .map_err(scene_tree_error)?;
        self.tree = Some(tree);
        Ok(())
    }

    fn remove_subtree(&mut self, id: &str, missing_ok: bool) -> Result<(), JsRuntimeError> {
        let removed_pending = self.pending_nodes.remove(id).is_some();
        let Some(tree) = self.tree.as_ref() else {
            if !removed_pending && !missing_ok {
                return Err(missing_node_error(id));
            }
            self.unregister_handlers_for_ids([id.to_owned()]);
            self.unregister_accessibility_for_ids([id.to_owned()]);
            self.unregister_measurements_for_ids([id.to_owned()]);
            self.clear_focus_for_ids([id.to_owned()]);
            return Ok(());
        };
        let exists_in_tree = tree.node(&RuntimeViewId::new(id)).is_some();
        if !removed_pending && !exists_in_tree {
            if missing_ok {
                self.unregister_handlers_for_ids([id.to_owned()]);
                self.unregister_accessibility_for_ids([id.to_owned()]);
                self.unregister_measurements_for_ids([id.to_owned()]);
                self.clear_focus_for_ids([id.to_owned()]);
                return Ok(());
            }
            return Err(missing_node_error(id));
        }
        if tree.root_id().as_str() == id {
            let removed = self.subtree_ids(id);
            self.tree = None;
            self.unregister_handlers_for_ids(removed.clone());
            self.unregister_accessibility_for_ids(removed.clone());
            self.unregister_measurements_for_ids(removed.clone());
            self.clear_focus_for_ids(removed);
            return Ok(());
        }
        let removed = self.subtree_ids(id);
        let tree = self
            .take_tree()?
            .remove_subtrees(&[RuntimeViewId::new(id)])
            .map_err(scene_tree_error)?;
        self.tree = Some(tree);
        self.unregister_handlers_for_ids(removed.clone());
        self.unregister_accessibility_for_ids(removed.clone());
        self.unregister_measurements_for_ids(removed.clone());
        self.clear_focus_for_ids(removed);
        Ok(())
    }

    fn ensure_new_node_id(&self, id: &str) -> Result<(), JsRuntimeError> {
        let exists_in_tree = self
            .tree
            .as_ref()
            .is_some_and(|tree| tree.node(&RuntimeViewId::new(id)).is_some());
        if exists_in_tree || self.pending_nodes.contains_key(id) {
            return Err(JsRuntimeError::new(
                "js-runtime.scene-tree.apply-failed",
                format!("scene tree already contains node `{id}`"),
            ));
        }
        Ok(())
    }

    fn ensure_node_exists(&self, id: &str) -> Result<(), JsRuntimeError> {
        let exists_in_tree = self
            .tree
            .as_ref()
            .is_some_and(|tree| tree.node(&RuntimeViewId::new(id)).is_some());
        if exists_in_tree || self.pending_nodes.contains_key(id) {
            return Ok(());
        }
        Err(missing_node_error(id))
    }

    fn take_tree(&mut self) -> Result<RuntimeViewTree, JsRuntimeError> {
        self.tree.take().ok_or_else(|| missing_node_error("root"))
    }

    fn subtree_ids(&self, id: &str) -> Vec<String> {
        let mut removed = BTreeSet::new();
        self.collect_subtree_ids(id, &mut removed);
        removed.into_iter().collect()
    }

    fn collect_subtree_ids(&self, id: &str, removed: &mut BTreeSet<String>) {
        if !removed.insert(id.to_owned()) {
            return;
        }
        let Some(tree) = self.tree.as_ref() else {
            return;
        };
        for child in tree.children_of(&RuntimeViewId::new(id)) {
            self.collect_subtree_ids(child.as_str(), removed);
        }
    }

    fn unregister_handlers_for_ids(&mut self, ids: impl IntoIterator<Item = String>) {
        let removed: BTreeSet<String> = ids.into_iter().collect();
        self.handlers
            .retain(|(node_id, _), _| !removed.contains(node_id));
    }

    fn unregister_accessibility_for_ids(&mut self, ids: impl IntoIterator<Item = String>) {
        let removed: BTreeSet<String> = ids.into_iter().collect();
        self.accessibility
            .retain(|node_id, _| !removed.contains(node_id));
    }

    fn unregister_measurements_for_ids(&mut self, ids: impl IntoIterator<Item = String>) {
        let removed: BTreeSet<String> = ids.into_iter().collect();
        self.measurements
            .retain(|request| !removed.contains(&request.node_id));
    }

    fn clear_focus_for_ids(&mut self, ids: impl IntoIterator<Item = String>) {
        let removed: BTreeSet<String> = ids.into_iter().collect();
        if self
            .focused_node
            .as_ref()
            .is_some_and(|node_id| removed.contains(node_id))
        {
            self.focused_node = None;
        }
    }
}

fn runtime_node(id: &str, kind: &SceneNodeKind, text: Option<&str>) -> RuntimeViewNode {
    RuntimeViewNode::new(
        RuntimeViewId::new(id),
        layout_style(kind),
        runtime_visual(kind, text),
    )
}

fn layout_style(kind: &SceneNodeKind) -> LayoutStyle {
    match kind {
        SceneNodeKind::View | SceneNodeKind::ScrollView | SceneNodeKind::List => {
            LayoutStyle::flex_container(FlexDirection::Column)
        }
        SceneNodeKind::Text
        | SceneNodeKind::Button
        | SceneNodeKind::Input
        | SceneNodeKind::Image
        | SceneNodeKind::Vector
        | SceneNodeKind::CustomSurface => LayoutStyle::custom_measured(),
    }
}

fn runtime_visual(kind: &SceneNodeKind, text: Option<&str>) -> RuntimeVisual {
    match kind {
        SceneNodeKind::Text => RuntimeVisual::Text(default_text_visual(text.unwrap_or_default())),
        SceneNodeKind::Button | SceneNodeKind::Input => text.map_or(RuntimeVisual::None, |value| {
            RuntimeVisual::Text(default_text_visual(value))
        }),
        SceneNodeKind::View
        | SceneNodeKind::Image
        | SceneNodeKind::Vector
        | SceneNodeKind::CustomSurface
        | SceneNodeKind::ScrollView
        | SceneNodeKind::List => RuntimeVisual::None,
    }
}

fn text_visual_for(current: &RuntimeVisual, text: &str) -> RuntimeVisual {
    if let RuntimeVisual::Text(visual) = current {
        RuntimeVisual::Text(visual.clone().with_text(text))
    } else {
        RuntimeVisual::Text(default_text_visual(text))
    }
}

fn default_text_visual(text: &str) -> RuntimeTextVisual {
    RuntimeTextVisual::new(text, 16.0, Color::rgba(241, 245, 249, 255))
}

fn replace_pending_visual(node: &mut RuntimeViewNode, visual: RuntimeVisual) {
    *node = RuntimeViewNode::new(node.id().clone(), node.layout_style().clone(), visual);
}

fn scene_value_text(value: &SceneValue) -> String {
    match value {
        SceneValue::Null => String::new(),
        SceneValue::Bool(value) => value.to_string(),
        SceneValue::Number(value) => value.to_string(),
        SceneValue::String(value) => value.clone(),
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "map_err passes the owned runtime error from the fallible tree operation"
)]
fn scene_tree_error(error: RuntimeSceneError) -> JsRuntimeError {
    JsRuntimeError::new(
        "js-runtime.scene-tree.apply-failed",
        format!("scene tree operation failed: {error:?}"),
    )
}

fn missing_node_error(id: &str) -> JsRuntimeError {
    JsRuntimeError::new(
        "js-runtime.scene-tree.apply-failed",
        format!("scene tree node `{id}` is missing"),
    )
}

//! Accessibility host export hooks.

use std::collections::BTreeMap;

use accesskit::{Action, Node, NodeId, Rect, Role, Toggled, Tree, TreeId, TreeUpdate};
use serde::{Deserialize, Serialize};

use crate::{A11yAction, A11yBounds, A11yNode, A11yRole, A11yTree, CheckedState};

/// Accessibility host surface kind.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum A11yHostSurfaceKind {
    /// Desktop accessibility services.
    Desktop,
    /// Embedded plugin editor accessibility availability.
    PluginEditor,
}

/// Layout geometry update for an accessibility node.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct LayoutGeometryUpdate {
    /// Target node identifier.
    pub node_id: &'static str,
    /// Updated bounds.
    pub bounds: A11yBounds,
}

impl LayoutGeometryUpdate {
    /// Creates a layout geometry update.
    #[must_use]
    pub const fn new(node_id: &'static str, bounds: A11yBounds) -> Self {
        Self { node_id, bounds }
    }
}

/// Accessibility host export snapshot.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct A11yHostExportSnapshot {
    /// Host surface kind.
    pub surface_kind: A11yHostSurfaceKind,
    /// Whether desktop platform services are enabled.
    pub platform_services_enabled: bool,
    /// Whether plugin accessibility is available.
    pub plugin_accessibility_available: bool,
    /// Exported tree.
    pub tree: A11yTree,
}

/// AccessKit export produced from a `Hawk2UI` accessibility tree.
#[derive(Clone, Debug, PartialEq)]
pub struct AccessKitExport {
    /// AccessKit tree update for the host adapter.
    pub update: TreeUpdate,
    node_ids: BTreeMap<String, NodeId>,
}

impl AccessKitExport {
    /// Returns the AccessKit node ID for a `Hawk2UI` node identifier.
    #[must_use]
    pub fn node_id(&self, id: &str) -> Option<NodeId> {
        self.node_ids.get(id).copied()
    }
}

/// AccessKit export failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessKitExportError {
    /// Stable diagnostic rule.
    pub rule: String,
    /// Human-readable diagnostic message.
    pub message: String,
}

impl AccessKitExportError {
    fn new(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            rule: rule.into(),
            message: message.into(),
        }
    }
}

/// Accessibility host exporter.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct A11yHostExporter {
    /// Host surface kind.
    pub surface_kind: A11yHostSurfaceKind,
    tree: A11yTree,
    plugin_accessibility_available: bool,
}

impl A11yHostExporter {
    /// Creates a desktop accessibility exporter.
    #[must_use]
    pub const fn desktop(tree: A11yTree) -> Self {
        Self {
            surface_kind: A11yHostSurfaceKind::Desktop,
            tree,
            plugin_accessibility_available: false,
        }
    }

    /// Creates a plugin editor accessibility exporter.
    #[must_use]
    pub const fn plugin_editor(tree: A11yTree, available: bool) -> Self {
        Self {
            surface_kind: A11yHostSurfaceKind::PluginEditor,
            tree,
            plugin_accessibility_available: available,
        }
    }

    /// Applies a layout geometry update to the tree.
    ///
    /// # Errors
    ///
    /// Returns a message when the target node does not exist.
    pub fn apply_geometry(&mut self, update: LayoutGeometryUpdate) -> Result<(), String> {
        let Some(node) = self.tree.find_mut(update.node_id) else {
            return Err(format!("accessibility node is missing: {}", update.node_id));
        };
        node.bounds = Some(update.bounds);
        Ok(())
    }

    /// Returns exported tree.
    #[must_use]
    pub const fn tree(&self) -> &A11yTree {
        &self.tree
    }

    /// Captures an export snapshot.
    #[must_use]
    pub fn export_snapshot(&self) -> A11yHostExportSnapshot {
        A11yHostExportSnapshot {
            surface_kind: self.surface_kind,
            platform_services_enabled: self.surface_kind == A11yHostSurfaceKind::Desktop,
            plugin_accessibility_available: self.plugin_accessibility_available,
            tree: self.tree.clone(),
        }
    }

    /// Exports the current tree as an AccessKit update for native host adapters.
    ///
    /// # Errors
    ///
    /// Returns [`AccessKitExportError`] when node identifiers are empty or geometry is invalid.
    pub fn export_accesskit_update(&self) -> Result<AccessKitExport, AccessKitExportError> {
        let mut node_ids = BTreeMap::new();
        assign_accesskit_ids(&self.tree.root, &mut node_ids)?;
        let mut focused = Vec::new();
        collect_focused_node_ids(&self.tree.root, &mut focused);
        if focused.len() > 1 {
            return Err(AccessKitExportError::new(
                "a11y.accesskit.multiple-focused-nodes",
                "accessibility tree must contain at most one focused node",
            ));
        }
        let focus = focused
            .first()
            .and_then(|id| node_ids.get(*id).copied())
            .unwrap_or(NodeId(1));
        let mut nodes = Vec::new();
        collect_accesskit_nodes(&self.tree.root, &node_ids, &mut nodes)?;
        Ok(AccessKitExport {
            update: TreeUpdate {
                nodes,
                tree: Some(Tree::new(NodeId(1))),
                tree_id: TreeId::ROOT,
                focus,
            },
            node_ids,
        })
    }
}

fn assign_accesskit_ids(
    node: &A11yNode,
    node_ids: &mut BTreeMap<String, NodeId>,
) -> Result<(), AccessKitExportError> {
    if node.id.trim().is_empty() {
        return Err(AccessKitExportError::new(
            "a11y.accesskit.invalid-id",
            "accessibility node identifier must not be empty",
        ));
    }
    if node_ids.contains_key(&node.id) {
        return Err(AccessKitExportError::new(
            "a11y.accesskit.duplicate-id",
            format!("duplicate accessibility node identifier: {}", node.id),
        ));
    }
    let next = u64::try_from(node_ids.len())
        .ok()
        .and_then(|len| len.checked_add(1))
        .ok_or_else(|| {
            AccessKitExportError::new(
                "a11y.accesskit.id-overflow",
                "accessibility tree contains more nodes than AccessKit can address",
            )
        })?;
    node_ids.insert(node.id.clone(), NodeId(next));
    for child in &node.children {
        assign_accesskit_ids(child, node_ids)?;
    }
    Ok(())
}

fn collect_accesskit_nodes(
    node: &A11yNode,
    node_ids: &BTreeMap<String, NodeId>,
    nodes: &mut Vec<(NodeId, Node)>,
) -> Result<(), AccessKitExportError> {
    let Some(id) = node_ids.get(&node.id).copied() else {
        return Err(AccessKitExportError::new(
            "a11y.accesskit.missing-id",
            format!(
                "accessibility node was not assigned an AccessKit ID: {}",
                node.id
            ),
        ));
    };
    let mut accesskit_node = Node::new(role_to_accesskit(node.role));
    if let Some(name) = &node.name {
        accesskit_node.set_label(name.as_str());
    }
    if let Some(description) = &node.description {
        accesskit_node.set_description(description.as_str());
    }
    if let Some(value) = &node.value {
        accesskit_node.set_value(value.as_str());
    }
    if node.disabled {
        accesskit_node.set_disabled();
    }
    if let Some(checked) = node.checked {
        accesskit_node.set_toggled(checked_to_accesskit(checked));
    }
    if let Some(bounds) = node.bounds {
        accesskit_node.set_bounds(bounds_to_accesskit(bounds)?);
    }
    for action in &node.actions {
        accesskit_node.add_action(action_to_accesskit(action));
    }
    let children = node
        .children
        .iter()
        .filter_map(|child| node_ids.get(&child.id).copied())
        .collect::<Vec<_>>();
    accesskit_node.set_children(children);
    nodes.push((id, accesskit_node));
    for child in &node.children {
        collect_accesskit_nodes(child, node_ids, nodes)?;
    }
    Ok(())
}

fn collect_focused_node_ids<'a>(node: &'a A11yNode, focused: &mut Vec<&'a str>) {
    if node.focused {
        focused.push(&node.id);
    }
    for child in &node.children {
        collect_focused_node_ids(child, focused);
    }
}

fn role_to_accesskit(role: A11yRole) -> Role {
    match role {
        A11yRole::Window => Role::Window,
        A11yRole::Panel | A11yRole::Custom => Role::Pane,
        A11yRole::Button => Role::Button,
        A11yRole::Slider => Role::Slider,
        A11yRole::TextInput => Role::TextInput,
        A11yRole::Checkbox => Role::CheckBox,
        A11yRole::List => Role::List,
        A11yRole::ListItem => Role::ListItem,
    }
}

fn checked_to_accesskit(checked: CheckedState) -> Toggled {
    match checked {
        CheckedState::Checked => Toggled::True,
        CheckedState::Unchecked => Toggled::False,
        CheckedState::Mixed => Toggled::Mixed,
    }
}

fn action_to_accesskit(action: &A11yAction) -> Action {
    match action {
        A11yAction::Focus => Action::Focus,
        A11yAction::Press => Action::Click,
        A11yAction::Increment => Action::Increment,
        A11yAction::Decrement => Action::Decrement,
        A11yAction::SetValue(_) => Action::SetValue,
        A11yAction::Custom(_) => Action::CustomAction,
    }
}

fn bounds_to_accesskit(bounds: A11yBounds) -> Result<Rect, AccessKitExportError> {
    for value in [bounds.x, bounds.y, bounds.width, bounds.height] {
        if !value.is_finite() {
            return Err(AccessKitExportError::new(
                "a11y.accesskit.invalid-bounds",
                "accessibility bounds must be finite",
            ));
        }
    }
    if bounds.width < 0.0 || bounds.height < 0.0 {
        return Err(AccessKitExportError::new(
            "a11y.accesskit.invalid-bounds",
            "accessibility bounds width and height must not be negative",
        ));
    }
    Ok(Rect::new(
        bounds.x,
        bounds.y,
        bounds.x + bounds.width,
        bounds.y + bounds.height,
    ))
}

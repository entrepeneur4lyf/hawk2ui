//! Accessibility host export hooks.

use serde::{Deserialize, Serialize};

use crate::{A11yBounds, A11yTree};

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
}

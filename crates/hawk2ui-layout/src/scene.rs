//! Scene geometry attachment records.

use crate::{ComputedGeometry, LayoutNodeId, LayoutOutput};

/// Scene geometry for rendering, hit testing, accessibility, and custom surfaces.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneGeometry {
    /// Geometry used by rendering.
    pub render: ComputedGeometry,
    /// Geometry used by hit testing.
    pub hit_test: ComputedGeometry,
    /// Optional accessibility label.
    pub accessibility_label: Option<String>,
    /// Whether this node owns a custom draw surface.
    pub custom_surface: bool,
}

impl SceneGeometry {
    /// Creates scene geometry from computed layout geometry.
    #[must_use]
    pub const fn from_computed(geometry: ComputedGeometry) -> Self {
        Self {
            render: geometry,
            hit_test: geometry,
            accessibility_label: None,
            custom_surface: false,
        }
    }
}

/// Scene geometry attachment.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneGeometryAttachment {
    geometry: Vec<(String, SceneGeometry)>,
    clips: Vec<(String, ComputedGeometry)>,
}

impl SceneGeometryAttachment {
    /// Creates scene geometry from layout output.
    #[must_use]
    pub fn from_layout(output: &LayoutOutput) -> Self {
        Self {
            geometry: output
                .geometry_entries()
                .iter()
                .map(|(id, geometry)| {
                    (
                        id.as_str().to_string(),
                        SceneGeometry::from_computed(*geometry),
                    )
                })
                .collect(),
            clips: output
                .clip_entries()
                .iter()
                .map(|(id, clip)| (id.as_str().to_string(), *clip))
                .collect(),
        }
    }

    /// Adds or replaces an accessibility label for a node.
    #[must_use]
    pub fn with_accessibility_node(
        mut self,
        node_id: impl Into<String>,
        label: impl Into<String>,
    ) -> Self {
        let node_id = node_id.into();
        if let Some((_, geometry)) = self.geometry.iter_mut().find(|(id, _)| id == &node_id) {
            geometry.accessibility_label = Some(label.into());
        }
        self
    }

    /// Marks a node as a custom draw surface.
    #[must_use]
    pub fn with_custom_surface(mut self, node_id: impl Into<String>) -> Self {
        let node_id = node_id.into();
        if let Some((_, geometry)) = self.geometry.iter_mut().find(|(id, _)| id == &node_id) {
            geometry.custom_surface = true;
        }
        self
    }

    /// Returns scene geometry by node ID.
    #[must_use]
    pub fn geometry(&self, node_id: &str) -> Option<&SceneGeometry> {
        self.geometry
            .iter()
            .find(|(id, _)| id == node_id)
            .map(|(_, geometry)| geometry)
    }

    /// Returns scroll clip geometry by node ID.
    #[must_use]
    pub fn clip(&self, node_id: &str) -> Option<&ComputedGeometry> {
        self.clips
            .iter()
            .find(|(id, _)| id == node_id)
            .map(|(_, clip)| clip)
    }
}

impl LayoutOutput {
    /// Returns geometry entries in computation order.
    #[must_use]
    pub fn geometry_entries(&self) -> &[(LayoutNodeId, ComputedGeometry)] {
        self.geometry_entries_internal()
    }

    /// Returns clip entries in computation order.
    #[must_use]
    pub fn clip_entries(&self) -> &[(LayoutNodeId, ComputedGeometry)] {
        self.clip_entries_internal()
    }
}

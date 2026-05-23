//! Deterministic layout calculation backend.

use crate::{FlexDirection, LayoutNodeId, LayoutTree, LayoutValue};

/// Viewport size used as the root layout constraint.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    /// Viewport width.
    pub width: f32,
    /// Viewport height.
    pub height: f32,
}

impl Viewport {
    /// Creates a viewport.
    #[must_use]
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

/// Computed node geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComputedGeometry {
    /// X coordinate.
    pub x: f32,
    /// Y coordinate.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
    /// Whether the node is absolute positioned.
    pub absolute: bool,
}

impl ComputedGeometry {
    /// Creates computed geometry.
    #[must_use]
    pub const fn new(x: f32, y: f32, width: f32, height: f32, absolute: bool) -> Self {
        Self {
            x,
            y,
            width,
            height,
            absolute,
        }
    }
}

/// Layout calculation output.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutOutput {
    geometry: Vec<(LayoutNodeId, ComputedGeometry)>,
    clips: Vec<(LayoutNodeId, ComputedGeometry)>,
}

impl LayoutOutput {
    /// Creates layout output.
    #[must_use]
    pub const fn new(
        geometry: Vec<(LayoutNodeId, ComputedGeometry)>,
        clips: Vec<(LayoutNodeId, ComputedGeometry)>,
    ) -> Self {
        Self { geometry, clips }
    }

    /// Returns geometry by node ID.
    #[must_use]
    pub fn geometry(&self, node_id: &LayoutNodeId) -> Option<&ComputedGeometry> {
        self.geometry
            .iter()
            .find(|(id, _)| id.as_str() == node_id.as_str())
            .map(|(_, geometry)| geometry)
    }

    /// Returns scroll clip geometry by node ID.
    #[must_use]
    pub fn clip(&self, node_id: &LayoutNodeId) -> Option<&ComputedGeometry> {
        self.clips
            .iter()
            .find(|(id, _)| id.as_str() == node_id.as_str())
            .map(|(_, clip)| clip)
    }

    pub(crate) fn geometry_entries_internal(&self) -> &[(LayoutNodeId, ComputedGeometry)] {
        &self.geometry
    }

    pub(crate) fn clip_entries_internal(&self) -> &[(LayoutNodeId, ComputedGeometry)] {
        &self.clips
    }
}

impl LayoutTree {
    /// Computes layout geometry for this tree.
    #[must_use]
    pub fn compute_layout(&self, viewport: Viewport) -> LayoutOutput {
        let Some(root) = self.root_id() else {
            return LayoutOutput::new(Vec::new(), Vec::new());
        };
        let root_geometry = self.node(root).map_or(
            ComputedGeometry::new(0.0, 0.0, viewport.width, viewport.height, false),
            |root_node| {
                let style = root_node.style();
                ComputedGeometry::new(
                    0.0,
                    0.0,
                    resolve_value(style.size().width(), viewport.width).unwrap_or(viewport.width),
                    resolve_value(style.size().height(), viewport.height)
                        .unwrap_or(viewport.height),
                    style.absolute(),
                )
            },
        );
        let mut output = ComputedBuilder::default();
        self.compute_node(root, root_geometry, &mut output);
        LayoutOutput::new(output.geometry, output.clips)
    }

    fn compute_node(
        &self,
        node_id: &LayoutNodeId,
        parent_geometry: ComputedGeometry,
        output: &mut ComputedBuilder,
    ) {
        let Some(node) = self.node(node_id) else {
            return;
        };
        let style = node.style();
        let geometry = parent_geometry;
        output.geometry.push((node_id.clone(), parent_geometry));
        if style.is_scroll_container() {
            output.clips.push((node_id.clone(), geometry));
        }
        self.compute_children(node_id, geometry, output);
    }

    fn compute_children(
        &self,
        node_id: &LayoutNodeId,
        geometry: ComputedGeometry,
        output: &mut ComputedBuilder,
    ) {
        let Some(node) = self.node(node_id) else {
            return;
        };
        let style = node.style();
        let padding = style.padding();
        let gap = resolve_gap(style.gap());
        let mut cursor_x = geometry.x + resolve_edge(padding.left, geometry.width);
        let mut cursor_y = geometry.y + resolve_edge(padding.top, geometry.height);
        let content_width = geometry.width
            - resolve_edge(padding.left, geometry.width)
            - resolve_edge(padding.right, geometry.width);
        let content_height = geometry.height
            - resolve_edge(padding.top, geometry.height)
            - resolve_edge(padding.bottom, geometry.height);

        for child_id in self.children_of(node_id) {
            let Some(child) = self.node(child_id) else {
                continue;
            };
            let child_style = child.style();
            let child_width =
                resolve_value(child_style.size().width(), content_width).unwrap_or(content_width);
            let child_height = resolve_value(child_style.size().height(), content_height)
                .unwrap_or(content_height);
            let child_geometry = if child_style.absolute() {
                ComputedGeometry::new(geometry.x, geometry.y, child_width, child_height, true)
            } else {
                ComputedGeometry::new(cursor_x, cursor_y, child_width, child_height, false)
            };
            self.compute_node(child_id, child_geometry, output);
            if !child_style.absolute() {
                match style.flex_direction().unwrap_or(FlexDirection::Column) {
                    FlexDirection::Row => cursor_x += child_width + gap,
                    FlexDirection::Column => cursor_y += child_height + gap,
                }
            }
        }
    }
}

#[derive(Default)]
struct ComputedBuilder {
    geometry: Vec<(LayoutNodeId, ComputedGeometry)>,
    clips: Vec<(LayoutNodeId, ComputedGeometry)>,
}

fn resolve_value(value: LayoutValue, basis: f32) -> Option<f32> {
    match value {
        LayoutValue::Px(value) => Some(value),
        LayoutValue::Percent(value) => Some(basis * value / 100.0),
        LayoutValue::Auto => None,
    }
}

fn resolve_edge(value: LayoutValue, basis: f32) -> f32 {
    resolve_value(value, basis).unwrap_or(0.0)
}

fn resolve_gap(value: LayoutValue) -> f32 {
    match value {
        LayoutValue::Px(value) | LayoutValue::Percent(value) => value,
        LayoutValue::Auto => 0.0,
    }
}

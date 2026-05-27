//! Deterministic layout calculation backend.

use crate::{
    BoxEdges, FlexDirection, LayoutNodeId, LayoutStyle, LayoutTree, LayoutTreeError, LayoutValue,
};
use taffy::{
    AvailableSpace, Dimension, Display, LengthPercentage, LengthPercentageAuto, NodeId, Overflow,
    Point, Position, Rect, Size, Style, TaffyTree,
};

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
        self.try_compute_layout(viewport)
            .unwrap_or_else(|_| LayoutOutput::new(Vec::new(), Vec::new()))
    }

    /// Computes layout geometry for this tree and reports validation/backend failures.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutTreeError`] when the viewport, tree records, style records, or layout backend fail.
    pub fn try_compute_layout(&self, viewport: Viewport) -> Result<LayoutOutput, LayoutTreeError> {
        validate_viewport(viewport)?;
        self.validate()?;
        let Some(root) = self.root_id() else {
            return Err(LayoutTreeError::ComputeFailed("missing root".to_string()));
        };
        let mut taffy = TaffyTree::new();
        let mut ids = Vec::new();
        let root_node = self
            .build_taffy_node(root, viewport, true, &mut taffy, &mut ids)
            .map_err(|error| LayoutTreeError::ComputeFailed(error.to_string()))?;
        taffy
            .compute_layout(
                root_node,
                Size {
                    width: AvailableSpace::Definite(viewport.width),
                    height: AvailableSpace::Definite(viewport.height),
                },
            )
            .map_err(|error| LayoutTreeError::ComputeFailed(error.to_string()))?;

        let mut output = ComputedBuilder::default();
        for (layout_id, taffy_id) in ids {
            let Some(layout_node) = self.node(&layout_id) else {
                return Err(LayoutTreeError::ComputeFailed(
                    layout_id.as_str().to_string(),
                ));
            };
            let layout = taffy
                .layout(taffy_id)
                .map_err(|error| LayoutTreeError::ComputeFailed(error.to_string()))?;
            let geometry = ComputedGeometry::new(
                layout.location.x,
                layout.location.y,
                layout.size.width,
                layout.size.height,
                layout_node.style().absolute(),
            );
            if !is_valid_computed_geometry(geometry) {
                return Err(LayoutTreeError::ComputeFailed(
                    layout_id.as_str().to_string(),
                ));
            }
            output.geometry.push((layout_id.clone(), geometry));
            if layout_node.style().is_scroll_container() {
                output.clips.push((layout_id, geometry));
            }
        }
        Ok(LayoutOutput::new(output.geometry, output.clips))
    }

    fn build_taffy_node(
        &self,
        node_id: &LayoutNodeId,
        viewport: Viewport,
        is_root: bool,
        taffy: &mut TaffyTree,
        ids: &mut Vec<(LayoutNodeId, NodeId)>,
    ) -> Result<NodeId, taffy::TaffyError> {
        let Some(node) = self.node(node_id) else {
            return taffy.new_leaf(Style::default());
        };
        let children = self
            .children_of(node_id)
            .iter()
            .map(|child_id| self.build_taffy_node(child_id, viewport, false, taffy, ids))
            .collect::<Result<Vec<_>, _>>()?;
        let taffy_id = if children.is_empty() {
            taffy.new_leaf(taffy_style(node.style(), is_root.then_some(viewport)))?
        } else {
            taffy.new_with_children(
                taffy_style(node.style(), is_root.then_some(viewport)),
                &children,
            )?
        };
        ids.push((node_id.clone(), taffy_id));
        Ok(taffy_id)
    }
}

#[derive(Default)]
struct ComputedBuilder {
    geometry: Vec<(LayoutNodeId, ComputedGeometry)>,
    clips: Vec<(LayoutNodeId, ComputedGeometry)>,
}

fn taffy_style(style: &LayoutStyle, root_viewport: Option<Viewport>) -> Style {
    let mut taffy_style = Style {
        display: Display::Flex,
        position: if style.absolute() {
            Position::Absolute
        } else {
            Position::Relative
        },
        size: Size {
            width: dimension(style.size().width()),
            height: dimension(style.size().height()),
        },
        min_size: Size {
            width: dimension(style.min_size().width()),
            height: dimension(style.min_size().height()),
        },
        max_size: Size {
            width: dimension(style.max_size().width()),
            height: dimension(style.max_size().height()),
        },
        margin: rect_auto(style.margin()),
        padding: rect(style.padding()),
        overflow: if style.is_scroll_container() {
            Point {
                x: Overflow::Scroll,
                y: Overflow::Scroll,
            }
        } else {
            Point {
                x: Overflow::Visible,
                y: Overflow::Visible,
            }
        },
        gap: Size {
            width: length_percentage(style.gap()),
            height: length_percentage(style.gap()),
        },
        flex_direction: match style.flex_direction().unwrap_or(FlexDirection::Column) {
            FlexDirection::Row => taffy::FlexDirection::Row,
            FlexDirection::Column => taffy::FlexDirection::Column,
        },
        flex_shrink: 0.0,
        ..Style::default()
    };
    if let Some(viewport) = root_viewport {
        if matches!(style.size().width(), LayoutValue::Auto) {
            taffy_style.size.width = Dimension::length(viewport.width);
        }
        if matches!(style.size().height(), LayoutValue::Auto) {
            taffy_style.size.height = Dimension::length(viewport.height);
        }
    }
    taffy_style
}

fn dimension(value: LayoutValue) -> Dimension {
    match value {
        LayoutValue::Px(value) => Dimension::length(value),
        LayoutValue::Percent(value) => Dimension::percent(value / 100.0),
        LayoutValue::Auto => Dimension::auto(),
    }
}

fn length_percentage(value: LayoutValue) -> LengthPercentage {
    match value {
        LayoutValue::Px(value) => LengthPercentage::length(value),
        LayoutValue::Percent(value) => LengthPercentage::percent(value / 100.0),
        LayoutValue::Auto => LengthPercentage::length(0.0),
    }
}

fn length_percentage_auto(value: LayoutValue) -> LengthPercentageAuto {
    match value {
        LayoutValue::Px(value) => LengthPercentageAuto::length(value),
        LayoutValue::Percent(value) => LengthPercentageAuto::percent(value / 100.0),
        LayoutValue::Auto => LengthPercentageAuto::auto(),
    }
}

fn rect(edges: BoxEdges) -> Rect<LengthPercentage> {
    Rect {
        left: length_percentage(edges.left),
        right: length_percentage(edges.right),
        top: length_percentage(edges.top),
        bottom: length_percentage(edges.bottom),
    }
}

fn rect_auto(edges: BoxEdges) -> Rect<LengthPercentageAuto> {
    Rect {
        left: length_percentage_auto(edges.left),
        right: length_percentage_auto(edges.right),
        top: length_percentage_auto(edges.top),
        bottom: length_percentage_auto(edges.bottom),
    }
}

fn validate_viewport(viewport: Viewport) -> Result<(), LayoutTreeError> {
    if viewport.width.is_finite()
        && viewport.height.is_finite()
        && viewport.width > 0.0
        && viewport.height > 0.0
    {
        Ok(())
    } else {
        Err(LayoutTreeError::InvalidViewport)
    }
}

fn is_valid_computed_geometry(geometry: ComputedGeometry) -> bool {
    geometry.x.is_finite()
        && geometry.y.is_finite()
        && geometry.width.is_finite()
        && geometry.height.is_finite()
        && geometry.width >= 0.0
        && geometry.height >= 0.0
}

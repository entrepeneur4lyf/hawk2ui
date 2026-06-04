//! Deterministic layout calculation backend.

use crate::{
    BoxEdges, FlexDirection, LayoutAlignItems, LayoutGridAutoFlow, LayoutGridLine,
    LayoutGridPlacement, LayoutGridTrack, LayoutJustifyContent, LayoutNodeId, LayoutStyle,
    LayoutTextMeasurer, LayoutTree, LayoutTreeError, LayoutValue, TextMeasureError,
    TextMeasureInput,
};
use taffy::prelude::{auto, fr, length, line, max_content, min_content, span};
use taffy::{
    AlignContent, AlignItems, AvailableSpace, Dimension, Display,
    GridAutoFlow as TaffyGridAutoFlow, GridPlacement as TaffyGridPlacement, GridTemplateComponent,
    LengthPercentage, LengthPercentageAuto, Line, NodeId, Overflow, Point, Position, Rect, Size,
    Style, TaffyTree, TrackSizingFunction,
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
    /// X coordinate **relative to the parent node's border box** (Taffy's coordinate space) — not
    /// an absolute/viewport coordinate. `SceneGeometry` and the render/hit-test/a11y consumers copy
    /// this verbatim, so a consumer that treats it as absolute mis-places every nested node; to get
    /// absolute positions, accumulate ancestor offsets.
    pub x: f32,
    /// Y coordinate, parent-relative — see [`ComputedGeometry::x`].
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
    diagnostics: Vec<LayoutTreeError>,
}

impl LayoutOutput {
    /// Creates layout output.
    #[must_use]
    pub const fn new(
        geometry: Vec<(LayoutNodeId, ComputedGeometry)>,
        clips: Vec<(LayoutNodeId, ComputedGeometry)>,
    ) -> Self {
        Self {
            geometry,
            clips,
            diagnostics: Vec::new(),
        }
    }

    /// Creates layout output carrying a failed convenience-path diagnostic.
    #[must_use]
    pub fn diagnostic(error: LayoutTreeError) -> Self {
        Self {
            geometry: Vec::new(),
            clips: Vec::new(),
            diagnostics: vec![error],
        }
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

    /// Returns diagnostics captured by the non-fallible convenience path.
    #[must_use]
    pub fn diagnostics(&self) -> &[LayoutTreeError] {
        &self.diagnostics
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
    ///
    /// This is a convenience API for tests and trusted fixtures. Production callers should use
    /// [`Self::try_compute_layout`] so failures can be handled as structured `Result` errors. On
    /// this path, validation/backend failures return an output with no geometry and a diagnostic
    /// available through [`LayoutOutput::diagnostics`]. Text-measured leaves collapse to zero size
    /// on this path (no measurer is supplied); use
    /// [`Self::try_compute_layout_with_text_measurer`] for trees containing measured text.
    #[must_use]
    pub fn compute_layout(&self, viewport: Viewport) -> LayoutOutput {
        match self.try_compute_layout(viewport) {
            Ok(output) => output,
            Err(error) => LayoutOutput::diagnostic(error),
        }
    }

    /// Computes layout geometry for this tree and reports validation/backend failures.
    ///
    /// Text-measured leaves collapse to `Size::ZERO` on this path because no measurer is supplied;
    /// use [`Self::try_compute_layout_with_text_measurer`] for trees containing measured text.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutTreeError`] when the viewport, tree records, style records, or layout backend fail.
    pub fn try_compute_layout(&self, viewport: Viewport) -> Result<LayoutOutput, LayoutTreeError> {
        self.try_compute_layout_inner(viewport, None)
    }

    /// Computes layout geometry and feeds text measurement into Taffy leaf sizing.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutTreeError`] when validation, text measurement, or Taffy computation fails.
    pub fn try_compute_layout_with_text_measurer(
        &self,
        viewport: Viewport,
        measurer: &dyn LayoutTextMeasurer,
    ) -> Result<LayoutOutput, LayoutTreeError> {
        self.try_compute_layout_inner(viewport, Some(measurer))
    }

    fn try_compute_layout_inner(
        &self,
        viewport: Viewport,
        measurer: Option<&dyn LayoutTextMeasurer>,
    ) -> Result<LayoutOutput, LayoutTreeError> {
        validate_viewport(viewport)?;
        self.validate()?;
        let Some(root) = self.root_id() else {
            return Err(LayoutTreeError::ComputeFailed("missing root".to_string()));
        };
        let mut taffy: TaffyTree<TextMeasureInput> = TaffyTree::new();
        let mut ids = Vec::new();
        let root_node = self
            .build_taffy_node(root, viewport, true, &mut taffy, &mut ids)
            .map_err(|error| LayoutTreeError::ComputeFailed(error.to_string()))?;
        let mut measure_error = None;
        taffy
            .compute_layout_with_measure(
                root_node,
                Size {
                    width: AvailableSpace::Definite(viewport.width),
                    height: AvailableSpace::Definite(viewport.height),
                },
                |known_dimensions, available_space, _node_id, context, _style| {
                    measure_text_leaf(
                        known_dimensions,
                        available_space,
                        context,
                        measurer,
                        &mut measure_error,
                    )
                },
            )
            .map_err(|error| LayoutTreeError::ComputeFailed(error.to_string()))?;
        if let Some(error) = measure_error {
            return Err(LayoutTreeError::ComputeFailed(format!(
                "text measurement failed: {}",
                error.rule()
            )));
        }

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
        taffy: &mut TaffyTree<TextMeasureInput>,
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
        let style = taffy_style(node.style(), is_root.then_some(viewport));
        let taffy_id = if children.is_empty() {
            if let Some(input) = node.text_measurement() {
                taffy.new_leaf_with_context(style, input.clone())?
            } else {
                taffy.new_leaf(style)?
            }
        } else {
            taffy.new_with_children(style, &children)?
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

fn measure_text_leaf(
    known_dimensions: Size<Option<f32>>,
    _available_space: Size<AvailableSpace>,
    context: Option<&mut TextMeasureInput>,
    measurer: Option<&dyn LayoutTextMeasurer>,
    measure_error: &mut Option<TextMeasureError>,
) -> Size<f32> {
    let (Some(input), Some(measurer)) = (context, measurer) else {
        return Size::ZERO;
    };
    match measurer.measure(input) {
        Ok(result) => Size {
            width: known_dimensions.width.unwrap_or(result.width),
            height: known_dimensions.height.unwrap_or(result.height),
        },
        Err(error) => {
            if measure_error.is_none() {
                *measure_error = Some(error);
            }
            Size::ZERO
        }
    }
}

fn taffy_style(style: &LayoutStyle, root_viewport: Option<Viewport>) -> Style {
    let mut taffy_style = Style {
        display: if style.grid().is_some() {
            Display::Grid
        } else {
            Display::Flex
        },
        position: if style.absolute() {
            Position::Absolute
        } else {
            Position::Relative
        },
        inset: rect_auto(style.inset()),
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
        align_items: style.align_items().map(align_items),
        justify_content: style.justify_content().map(justify_content),
        flex_direction: match style.flex_direction().unwrap_or(FlexDirection::Column) {
            FlexDirection::Row => taffy::FlexDirection::Row,
            FlexDirection::Column => taffy::FlexDirection::Column,
        },
        flex_basis: dimension(style.flex_basis()),
        flex_grow: style.flex_grow(),
        flex_shrink: style.flex_shrink(),
        ..Style::default()
    };
    apply_grid_style(&mut taffy_style, style);
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

fn apply_grid_style(taffy_style: &mut Style, style: &LayoutStyle) {
    if let Some(grid) = style.grid() {
        taffy_style.grid_template_columns = grid
            .columns()
            .iter()
            .copied()
            .map(taffy_grid_template_track)
            .collect();
        taffy_style.grid_template_rows = grid
            .rows()
            .iter()
            .copied()
            .map(taffy_grid_template_track)
            .collect();
        taffy_style.grid_auto_columns = grid
            .auto_columns()
            .iter()
            .copied()
            .map(taffy_grid_track)
            .collect();
        taffy_style.grid_auto_rows = grid
            .auto_rows()
            .iter()
            .copied()
            .map(taffy_grid_track)
            .collect();
        taffy_style.grid_auto_flow = taffy_grid_auto_flow(grid.auto_flow());
    }
    taffy_style.grid_row = taffy_grid_line(style.grid_row());
    taffy_style.grid_column = taffy_grid_line(style.grid_column());
}

fn taffy_grid_template_track(track: LayoutGridTrack) -> GridTemplateComponent<String> {
    GridTemplateComponent::Single(taffy_grid_track(track))
}

fn taffy_grid_track(track: LayoutGridTrack) -> TrackSizingFunction {
    match track {
        LayoutGridTrack::Px(value) => length(value),
        LayoutGridTrack::Fr(value) => fr(value),
        LayoutGridTrack::Auto => auto(),
        LayoutGridTrack::MinContent => min_content(),
        LayoutGridTrack::MaxContent => max_content(),
    }
}

fn taffy_grid_auto_flow(value: LayoutGridAutoFlow) -> TaffyGridAutoFlow {
    match value {
        LayoutGridAutoFlow::Row => TaffyGridAutoFlow::Row,
        LayoutGridAutoFlow::Column => TaffyGridAutoFlow::Column,
        LayoutGridAutoFlow::RowDense => TaffyGridAutoFlow::RowDense,
        LayoutGridAutoFlow::ColumnDense => TaffyGridAutoFlow::ColumnDense,
    }
}

fn taffy_grid_line(value: LayoutGridLine) -> Line<TaffyGridPlacement> {
    Line {
        start: taffy_grid_placement(value.start()),
        end: taffy_grid_placement(value.end()),
    }
}

fn taffy_grid_placement(value: LayoutGridPlacement) -> TaffyGridPlacement {
    match value {
        LayoutGridPlacement::Auto => TaffyGridPlacement::Auto,
        LayoutGridPlacement::Line(value) => line(value),
        LayoutGridPlacement::Span(value) => span(value),
    }
}

fn align_items(value: LayoutAlignItems) -> AlignItems {
    match value {
        LayoutAlignItems::Start => AlignItems::Start,
        LayoutAlignItems::End => AlignItems::End,
        LayoutAlignItems::Center => AlignItems::Center,
        LayoutAlignItems::Stretch => AlignItems::Stretch,
    }
}

fn justify_content(value: LayoutJustifyContent) -> AlignContent {
    match value {
        LayoutJustifyContent::Start => AlignContent::Start,
        LayoutJustifyContent::End => AlignContent::End,
        LayoutJustifyContent::Center => AlignContent::Center,
        LayoutJustifyContent::SpaceBetween => AlignContent::SpaceBetween,
        LayoutJustifyContent::SpaceAround => AlignContent::SpaceAround,
        LayoutJustifyContent::SpaceEvenly => AlignContent::SpaceEvenly,
    }
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

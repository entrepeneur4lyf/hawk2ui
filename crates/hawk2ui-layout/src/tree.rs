//! Layout tree records.

/// Stable layout node identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LayoutNodeId(String);

impl LayoutNodeId {
    /// Creates a layout node identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Layout numeric value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LayoutValue {
    /// Pixel value.
    Px(f32),
    /// Percentage value.
    Percent(f32),
    /// Automatic value.
    Auto,
}

impl LayoutValue {
    /// Creates a pixel layout value.
    #[must_use]
    pub const fn px(value: f32) -> Self {
        Self::Px(value)
    }

    /// Creates a percentage layout value.
    #[must_use]
    pub const fn percent(value: f32) -> Self {
        Self::Percent(value)
    }
}

/// Width and height sizing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutSizing {
    width: LayoutValue,
    height: LayoutValue,
}

impl LayoutSizing {
    /// Creates sizing from explicit width and height layout values.
    #[must_use]
    pub const fn new(width: LayoutValue, height: LayoutValue) -> Self {
        Self { width, height }
    }

    /// Creates fixed pixel sizing.
    #[must_use]
    pub const fn fixed(width: f32, height: f32) -> Self {
        Self {
            width: LayoutValue::Px(width),
            height: LayoutValue::Px(height),
        }
    }

    /// Creates percentage sizing.
    #[must_use]
    pub const fn percent(width: f32, height: f32) -> Self {
        Self {
            width: LayoutValue::Percent(width),
            height: LayoutValue::Percent(height),
        }
    }

    /// Creates automatic sizing.
    #[must_use]
    pub const fn auto() -> Self {
        Self {
            width: LayoutValue::Auto,
            height: LayoutValue::Auto,
        }
    }

    /// Returns the width value.
    #[must_use]
    pub const fn width(&self) -> LayoutValue {
        self.width
    }

    /// Returns the height value.
    #[must_use]
    pub const fn height(&self) -> LayoutValue {
        self.height
    }
}

/// Box edge values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxEdges {
    /// Left edge.
    pub left: LayoutValue,
    /// Right edge.
    pub right: LayoutValue,
    /// Top edge.
    pub top: LayoutValue,
    /// Bottom edge.
    pub bottom: LayoutValue,
}

impl BoxEdges {
    /// Creates equal edge values.
    #[must_use]
    pub const fn all(value: LayoutValue) -> Self {
        Self {
            left: value,
            right: value,
            top: value,
            bottom: value,
        }
    }

    /// Creates horizontal and vertical edge values.
    #[must_use]
    pub const fn axis(horizontal: LayoutValue, vertical: LayoutValue) -> Self {
        Self {
            left: horizontal,
            right: horizontal,
            top: vertical,
            bottom: vertical,
        }
    }
}

/// Flex direction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlexDirection {
    /// Row direction.
    Row,
    /// Column direction.
    Column,
}

/// Cross-axis alignment for flex children.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutAlignItems {
    /// Pack children toward the start of the cross axis.
    Start,
    /// Pack children toward the end of the cross axis.
    End,
    /// Center children along the cross axis.
    Center,
    /// Stretch children along the cross axis.
    Stretch,
}

/// Main-axis distribution for flex children.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutJustifyContent {
    /// Pack children toward the start of the main axis.
    Start,
    /// Pack children toward the end of the main axis.
    End,
    /// Center children along the main axis.
    Center,
    /// Distribute remaining space between children.
    SpaceBetween,
    /// Distribute remaining space around children.
    SpaceAround,
    /// Distribute remaining space evenly around children.
    SpaceEvenly,
}

/// Grid track sizing function.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LayoutGridTrack {
    /// Fixed pixel track.
    Px(f32),
    /// Fractional track.
    Fr(f32),
    /// Automatic track.
    Auto,
    /// Minimum-content track.
    MinContent,
    /// Maximum-content track.
    MaxContent,
}

impl LayoutGridTrack {
    /// Creates a fixed pixel track.
    #[must_use]
    pub const fn px(value: f32) -> Self {
        Self::Px(value)
    }

    /// Creates a fractional track.
    #[must_use]
    pub const fn fr(value: f32) -> Self {
        Self::Fr(value)
    }
}

/// Grid auto-placement direction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutGridAutoFlow {
    /// Fill each row before adding rows.
    Row,
    /// Fill each column before adding columns.
    Column,
    /// Fill rows and back-fill holes.
    RowDense,
    /// Fill columns and back-fill holes.
    ColumnDense,
}

/// Grid line placement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutGridPlacement {
    /// Auto-placement.
    Auto,
    /// Explicit grid line number.
    Line(i16),
    /// Span track count.
    Span(u16),
}

impl LayoutGridPlacement {
    /// Places at a concrete grid line.
    #[must_use]
    pub const fn line(value: i16) -> Self {
        Self::Line(value)
    }

    /// Spans the provided number of grid tracks.
    #[must_use]
    pub const fn span(value: u16) -> Self {
        Self::Span(value)
    }
}

/// Grid axis placement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LayoutGridLine {
    start: LayoutGridPlacement,
    end: LayoutGridPlacement,
}

impl LayoutGridLine {
    /// Creates a grid line placement from explicit start and end placements.
    #[must_use]
    pub const fn new(start: LayoutGridPlacement, end: LayoutGridPlacement) -> Self {
        Self { start, end }
    }

    /// Creates an automatic grid line placement.
    #[must_use]
    pub const fn auto() -> Self {
        Self::new(LayoutGridPlacement::Auto, LayoutGridPlacement::Auto)
    }

    /// Creates a placement starting at a line.
    #[must_use]
    pub const fn line(start: i16) -> Self {
        Self::new(LayoutGridPlacement::Line(start), LayoutGridPlacement::Auto)
    }

    /// Creates a spanning placement.
    #[must_use]
    pub const fn span(span: u16) -> Self {
        Self::new(LayoutGridPlacement::Span(span), LayoutGridPlacement::Auto)
    }

    /// Returns the start placement.
    #[must_use]
    pub const fn start(&self) -> LayoutGridPlacement {
        self.start
    }

    /// Returns the end placement.
    #[must_use]
    pub const fn end(&self) -> LayoutGridPlacement {
        self.end
    }
}

/// Grid container style.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutGridStyle {
    columns: Vec<LayoutGridTrack>,
    rows: Vec<LayoutGridTrack>,
    auto_columns: Vec<LayoutGridTrack>,
    auto_rows: Vec<LayoutGridTrack>,
    auto_flow: LayoutGridAutoFlow,
}

impl LayoutGridStyle {
    /// Creates grid container tracks.
    #[must_use]
    pub fn new(columns: Vec<LayoutGridTrack>, rows: Vec<LayoutGridTrack>) -> Self {
        Self {
            columns,
            rows,
            auto_columns: vec![LayoutGridTrack::Auto],
            auto_rows: vec![LayoutGridTrack::Auto],
            auto_flow: LayoutGridAutoFlow::Row,
        }
    }

    /// Sets implicit column tracks.
    #[must_use]
    pub fn with_auto_columns(mut self, auto_columns: Vec<LayoutGridTrack>) -> Self {
        self.auto_columns = auto_columns;
        self
    }

    /// Sets implicit row tracks.
    #[must_use]
    pub fn with_auto_rows(mut self, auto_rows: Vec<LayoutGridTrack>) -> Self {
        self.auto_rows = auto_rows;
        self
    }

    /// Sets auto-placement flow.
    #[must_use]
    pub const fn with_auto_flow(mut self, auto_flow: LayoutGridAutoFlow) -> Self {
        self.auto_flow = auto_flow;
        self
    }

    /// Returns explicit column tracks.
    #[must_use]
    pub fn columns(&self) -> &[LayoutGridTrack] {
        &self.columns
    }

    /// Returns explicit row tracks.
    #[must_use]
    pub fn rows(&self) -> &[LayoutGridTrack] {
        &self.rows
    }

    /// Returns implicit column tracks.
    #[must_use]
    pub fn auto_columns(&self) -> &[LayoutGridTrack] {
        &self.auto_columns
    }

    /// Returns implicit row tracks.
    #[must_use]
    pub fn auto_rows(&self) -> &[LayoutGridTrack] {
        &self.auto_rows
    }

    /// Returns auto-placement flow.
    #[must_use]
    pub const fn auto_flow(&self) -> LayoutGridAutoFlow {
        self.auto_flow
    }
}

/// Layout style record.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutStyle {
    size: LayoutSizing,
    min_size: LayoutSizing,
    max_size: LayoutSizing,
    margin: BoxEdges,
    padding: BoxEdges,
    gap: LayoutValue,
    flex_direction: Option<FlexDirection>,
    flex_basis: LayoutValue,
    flex_grow: f32,
    flex_shrink: f32,
    align_items: Option<LayoutAlignItems>,
    justify_content: Option<LayoutJustifyContent>,
    grid: Option<LayoutGridStyle>,
    grid_row: LayoutGridLine,
    grid_column: LayoutGridLine,
    inset: BoxEdges,
    scroll_container: bool,
    absolute: bool,
    custom_measured: bool,
}

impl LayoutStyle {
    /// Creates a flex container style.
    #[must_use]
    pub fn flex_container(direction: FlexDirection) -> Self {
        Self {
            flex_direction: Some(direction),
            ..Self::base()
        }
    }

    /// Creates a scroll container style.
    #[must_use]
    pub fn scroll_container() -> Self {
        Self {
            scroll_container: true,
            ..Self::base()
        }
    }

    /// Creates an absolute region style.
    #[must_use]
    pub fn absolute_region() -> Self {
        Self {
            absolute: true,
            ..Self::base()
        }
    }

    /// Creates a custom measured node style.
    #[must_use]
    pub fn custom_measured() -> Self {
        Self {
            custom_measured: true,
            ..Self::base()
        }
    }

    /// Creates a grid container style.
    #[must_use]
    pub fn grid_container(columns: Vec<LayoutGridTrack>, rows: Vec<LayoutGridTrack>) -> Self {
        Self {
            grid: Some(LayoutGridStyle::new(columns, rows)),
            ..Self::base()
        }
    }

    fn base() -> Self {
        Self {
            size: LayoutSizing::auto(),
            min_size: LayoutSizing::auto(),
            max_size: LayoutSizing::auto(),
            margin: BoxEdges::all(LayoutValue::Px(0.0)),
            padding: BoxEdges::all(LayoutValue::Px(0.0)),
            gap: LayoutValue::Px(0.0),
            flex_direction: None,
            flex_basis: LayoutValue::Auto,
            flex_grow: 0.0,
            flex_shrink: 0.0,
            align_items: None,
            justify_content: None,
            grid: None,
            grid_row: LayoutGridLine::auto(),
            grid_column: LayoutGridLine::auto(),
            inset: BoxEdges::all(LayoutValue::Auto),
            scroll_container: false,
            absolute: false,
            custom_measured: false,
        }
    }

    /// Sets preferred size.
    #[must_use]
    pub const fn with_size(mut self, size: LayoutSizing) -> Self {
        self.size = size;
        self
    }

    /// Sets minimum size.
    #[must_use]
    pub const fn with_min_size(mut self, min_size: LayoutSizing) -> Self {
        self.min_size = min_size;
        self
    }

    /// Sets maximum size.
    #[must_use]
    pub const fn with_max_size(mut self, max_size: LayoutSizing) -> Self {
        self.max_size = max_size;
        self
    }

    /// Sets margin.
    #[must_use]
    pub const fn with_margin(mut self, margin: BoxEdges) -> Self {
        self.margin = margin;
        self
    }

    /// Sets padding.
    #[must_use]
    pub const fn with_padding(mut self, padding: BoxEdges) -> Self {
        self.padding = padding;
        self
    }

    /// Sets gap.
    #[must_use]
    pub const fn with_gap(mut self, gap: LayoutValue) -> Self {
        self.gap = gap;
        self
    }

    /// Sets the flex basis used by the parent flex container.
    #[must_use]
    pub const fn with_flex_basis(mut self, flex_basis: LayoutValue) -> Self {
        self.flex_basis = flex_basis;
        self
    }

    /// Sets the positive flex grow factor.
    #[must_use]
    pub const fn with_flex_grow(mut self, flex_grow: f32) -> Self {
        self.flex_grow = flex_grow;
        self
    }

    /// Sets the positive flex shrink factor.
    #[must_use]
    pub const fn with_flex_shrink(mut self, flex_shrink: f32) -> Self {
        self.flex_shrink = flex_shrink;
        self
    }

    /// Sets cross-axis alignment for this flex container's children.
    #[must_use]
    pub const fn with_align_items(mut self, align_items: LayoutAlignItems) -> Self {
        self.align_items = Some(align_items);
        self
    }

    /// Sets main-axis distribution for this flex container's children.
    #[must_use]
    pub const fn with_justify_content(mut self, justify_content: LayoutJustifyContent) -> Self {
        self.justify_content = Some(justify_content);
        self
    }

    /// Sets grid container details.
    #[must_use]
    pub fn with_grid(mut self, grid: LayoutGridStyle) -> Self {
        self.grid = Some(grid);
        self
    }

    /// Sets grid row placement for this item.
    #[must_use]
    pub const fn with_grid_row(mut self, grid_row: LayoutGridLine) -> Self {
        self.grid_row = grid_row;
        self
    }

    /// Sets grid column placement for this item.
    #[must_use]
    pub const fn with_grid_column(mut self, grid_column: LayoutGridLine) -> Self {
        self.grid_column = grid_column;
        self
    }

    /// Sets absolute/relative inset offsets.
    #[must_use]
    pub const fn with_inset(mut self, inset: BoxEdges) -> Self {
        self.inset = inset;
        self
    }

    /// Returns preferred size.
    #[must_use]
    pub const fn size(&self) -> LayoutSizing {
        self.size
    }

    /// Returns minimum size.
    #[must_use]
    pub const fn min_size(&self) -> LayoutSizing {
        self.min_size
    }

    /// Returns maximum size.
    #[must_use]
    pub const fn max_size(&self) -> LayoutSizing {
        self.max_size
    }

    /// Returns margin.
    #[must_use]
    pub const fn margin(&self) -> BoxEdges {
        self.margin
    }

    /// Returns padding.
    #[must_use]
    pub const fn padding(&self) -> BoxEdges {
        self.padding
    }

    /// Returns gap.
    #[must_use]
    pub const fn gap(&self) -> LayoutValue {
        self.gap
    }

    /// Returns flex direction if this is a flex container.
    #[must_use]
    pub const fn flex_direction(&self) -> Option<FlexDirection> {
        self.flex_direction
    }

    /// Returns the flex basis.
    #[must_use]
    pub const fn flex_basis(&self) -> LayoutValue {
        self.flex_basis
    }

    /// Returns the flex grow factor.
    #[must_use]
    pub const fn flex_grow(&self) -> f32 {
        self.flex_grow
    }

    /// Returns the flex shrink factor.
    #[must_use]
    pub const fn flex_shrink(&self) -> f32 {
        self.flex_shrink
    }

    /// Returns cross-axis child alignment.
    #[must_use]
    pub const fn align_items(&self) -> Option<LayoutAlignItems> {
        self.align_items
    }

    /// Returns main-axis child distribution.
    #[must_use]
    pub const fn justify_content(&self) -> Option<LayoutJustifyContent> {
        self.justify_content
    }

    /// Returns grid container style, if any.
    #[must_use]
    pub const fn grid(&self) -> Option<&LayoutGridStyle> {
        self.grid.as_ref()
    }

    /// Returns grid row placement.
    #[must_use]
    pub const fn grid_row(&self) -> LayoutGridLine {
        self.grid_row
    }

    /// Returns grid column placement.
    #[must_use]
    pub const fn grid_column(&self) -> LayoutGridLine {
        self.grid_column
    }

    /// Returns absolute/relative inset offsets.
    #[must_use]
    pub const fn inset(&self) -> BoxEdges {
        self.inset
    }

    /// Returns whether this node scrolls content.
    #[must_use]
    pub const fn is_scroll_container(&self) -> bool {
        self.scroll_container
    }

    /// Returns whether this node is absolute positioned.
    #[must_use]
    pub const fn absolute(&self) -> bool {
        self.absolute
    }

    /// Returns whether this node requires custom measurement.
    #[must_use]
    pub const fn is_custom_measured(&self) -> bool {
        self.custom_measured
    }
}

use crate::TextMeasureInput;

/// Layout node record.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutNode {
    id: LayoutNodeId,
    style: LayoutStyle,
    text_measurement: Option<TextMeasureInput>,
}

impl LayoutNode {
    /// Creates a layout node.
    #[must_use]
    pub const fn new(id: LayoutNodeId, style: LayoutStyle) -> Self {
        Self {
            id,
            style,
            text_measurement: None,
        }
    }

    /// Returns the node identifier.
    #[must_use]
    pub const fn id(&self) -> &LayoutNodeId {
        &self.id
    }

    /// Returns the node style.
    #[must_use]
    pub const fn style(&self) -> &LayoutStyle {
        &self.style
    }

    /// Attaches text measurement input used when this node is a measured leaf.
    #[must_use]
    pub fn with_text_measurement(mut self, input: TextMeasureInput) -> Self {
        self.text_measurement = Some(input);
        self
    }

    /// Returns text measurement input for this node, if present.
    #[must_use]
    pub const fn text_measurement(&self) -> Option<&TextMeasureInput> {
        self.text_measurement.as_ref()
    }
}

/// Layout tree error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LayoutTreeError {
    /// Parent node does not exist.
    MissingParent(String),
    /// Node already exists.
    DuplicateNode(String),
    /// Node ID is empty or contains unsupported characters.
    InvalidNodeId(String),
    /// Viewport size is not finite and positive.
    InvalidViewport,
    /// Node style contains non-renderable numeric values.
    InvalidStyle(String),
    /// Layout backend failed to compute geometry.
    ComputeFailed(String),
}

/// Layout tree record.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutTree {
    nodes: Vec<TreeEntry>,
}

impl LayoutTree {
    /// Creates a layout tree with a root node.
    #[must_use]
    pub fn new(root: LayoutNode) -> Self {
        Self {
            nodes: vec![TreeEntry {
                node: root,
                parent: None,
                children: Vec::new(),
            }],
        }
    }

    /// Adds a child node to an existing parent.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutTreeError`] when the parent is missing or the child ID already exists.
    pub fn with_child(
        mut self,
        parent_id: LayoutNodeId,
        child: LayoutNode,
    ) -> Result<Self, LayoutTreeError> {
        validate_node_id(&parent_id)?;
        validate_layout_node(&child)?;
        if self.find_index(child.id()).is_some() {
            return Err(LayoutTreeError::DuplicateNode(
                child.id().as_str().to_string(),
            ));
        }
        let Some(parent_index) = self.find_index(&parent_id) else {
            return Err(LayoutTreeError::MissingParent(
                parent_id.as_str().to_string(),
            ));
        };
        let child_id = child.id().clone();
        self.nodes[parent_index].children.push(child_id);
        self.nodes.push(TreeEntry {
            node: child,
            parent: Some(parent_id),
            children: Vec::new(),
        });
        Ok(self)
    }

    /// Validates tree identities, parent links, child links, and node styles.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutTreeError`] when any record is invalid.
    pub fn validate(&self) -> Result<(), LayoutTreeError> {
        for entry in &self.nodes {
            validate_layout_node(&entry.node)?;
            if let Some(parent_id) = entry.parent.as_ref() {
                validate_node_id(parent_id)?;
                if self.find_index(parent_id).is_none() {
                    return Err(LayoutTreeError::MissingParent(
                        parent_id.as_str().to_string(),
                    ));
                }
            }
            for child_id in &entry.children {
                validate_node_id(child_id)?;
                if self.find_index(child_id).is_none() {
                    return Err(LayoutTreeError::MissingParent(
                        child_id.as_str().to_string(),
                    ));
                }
            }
        }
        Ok(())
    }

    /// Returns the parent of a node.
    #[must_use]
    pub fn parent_of(&self, node_id: &LayoutNodeId) -> Option<&LayoutNodeId> {
        self.entry(node_id).and_then(|entry| entry.parent.as_ref())
    }

    /// Returns children of a node.
    #[must_use]
    pub fn children_of(&self, node_id: &LayoutNodeId) -> &[LayoutNodeId] {
        self.entry(node_id)
            .map_or(&[], |entry| entry.children.as_slice())
    }

    /// Returns a node by ID.
    #[must_use]
    pub fn node(&self, node_id: &LayoutNodeId) -> Option<&LayoutNode> {
        self.entry(node_id).map(|entry| &entry.node)
    }

    /// Returns the root node ID.
    #[must_use]
    pub fn root_id(&self) -> Option<&LayoutNodeId> {
        self.nodes.first().map(|entry| entry.node.id())
    }

    fn entry(&self, node_id: &LayoutNodeId) -> Option<&TreeEntry> {
        self.nodes
            .iter()
            .find(|entry| entry.node.id().as_str() == node_id.as_str())
    }

    fn find_index(&self, node_id: &LayoutNodeId) -> Option<usize> {
        self.nodes
            .iter()
            .position(|entry| entry.node.id().as_str() == node_id.as_str())
    }
}

#[derive(Clone, Debug, PartialEq)]
struct TreeEntry {
    node: LayoutNode,
    parent: Option<LayoutNodeId>,
    children: Vec<LayoutNodeId>,
}

fn validate_layout_node(node: &LayoutNode) -> Result<(), LayoutTreeError> {
    validate_node_id(node.id())?;
    validate_layout_style(node.id(), node.style())
}

fn validate_node_id(node_id: &LayoutNodeId) -> Result<(), LayoutTreeError> {
    let value = node_id.as_str();
    if !value.trim().is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
    {
        Ok(())
    } else {
        Err(LayoutTreeError::InvalidNodeId(value.to_string()))
    }
}

fn validate_layout_style(
    node_id: &LayoutNodeId,
    style: &LayoutStyle,
) -> Result<(), LayoutTreeError> {
    validate_sizing(node_id, style.size(), false)?;
    validate_sizing(node_id, style.min_size(), false)?;
    validate_sizing(node_id, style.max_size(), false)?;
    validate_edges(node_id, style.margin(), true)?;
    validate_edges(node_id, style.padding(), false)?;
    validate_edges(node_id, style.inset(), true)?;
    validate_value(node_id, style.gap(), false)?;
    validate_value(node_id, style.flex_basis(), false)?;
    if style.flex_grow().is_finite()
        && style.flex_grow() >= 0.0
        && style.flex_shrink().is_finite()
        && style.flex_shrink() >= 0.0
    {
        Ok(())
    } else {
        Err(LayoutTreeError::InvalidStyle(node_id.as_str().to_string()))
    }
}

fn validate_sizing(
    node_id: &LayoutNodeId,
    sizing: LayoutSizing,
    allow_negative: bool,
) -> Result<(), LayoutTreeError> {
    validate_value(node_id, sizing.width(), allow_negative)?;
    validate_value(node_id, sizing.height(), allow_negative)
}

fn validate_edges(
    node_id: &LayoutNodeId,
    edges: BoxEdges,
    allow_negative: bool,
) -> Result<(), LayoutTreeError> {
    validate_value(node_id, edges.left, allow_negative)?;
    validate_value(node_id, edges.right, allow_negative)?;
    validate_value(node_id, edges.top, allow_negative)?;
    validate_value(node_id, edges.bottom, allow_negative)
}

fn validate_value(
    node_id: &LayoutNodeId,
    value: LayoutValue,
    allow_negative: bool,
) -> Result<(), LayoutTreeError> {
    let is_valid = match value {
        LayoutValue::Auto => true,
        LayoutValue::Px(value) | LayoutValue::Percent(value) => {
            value.is_finite() && (allow_negative || value >= 0.0)
        }
    };
    if is_valid {
        Ok(())
    } else {
        Err(LayoutTreeError::InvalidStyle(node_id.as_str().to_string()))
    }
}

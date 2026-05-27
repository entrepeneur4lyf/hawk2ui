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
    scroll_container: bool,
    absolute: bool,
    custom_measured: bool,
}

impl LayoutStyle {
    /// Creates a flex container style.
    #[must_use]
    pub const fn flex_container(direction: FlexDirection) -> Self {
        Self {
            flex_direction: Some(direction),
            ..Self::base()
        }
    }

    /// Creates a scroll container style.
    #[must_use]
    pub const fn scroll_container() -> Self {
        Self {
            scroll_container: true,
            ..Self::base()
        }
    }

    /// Creates an absolute region style.
    #[must_use]
    pub const fn absolute_region() -> Self {
        Self {
            absolute: true,
            ..Self::base()
        }
    }

    /// Creates a custom measured node style.
    #[must_use]
    pub const fn custom_measured() -> Self {
        Self {
            custom_measured: true,
            ..Self::base()
        }
    }

    const fn base() -> Self {
        Self {
            size: LayoutSizing::auto(),
            min_size: LayoutSizing::auto(),
            max_size: LayoutSizing::auto(),
            margin: BoxEdges::all(LayoutValue::Px(0.0)),
            padding: BoxEdges::all(LayoutValue::Px(0.0)),
            gap: LayoutValue::Px(0.0),
            flex_direction: None,
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
    validate_value(node_id, style.gap(), false)
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

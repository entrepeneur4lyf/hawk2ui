//! Retained runtime view records and runtime-to-render bridge types.

use hawk2ui_layout::{
    ComputedGeometry, LayoutNode, LayoutNodeId, LayoutOutput, LayoutStyle, LayoutTree,
    LayoutTreeError, Viewport,
};
use hawk2ui_render::{
    Color, Geometry, InvalidationReason, LayerKind, LayerStack, LayerValidationError,
    PaintCommandList, PaintLayer, SceneGraph, SceneGraphError, SceneNode, SceneNodeId, TextLayer,
    export_paint_commands,
};

/// Stable runtime view identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeViewId(String);

impl RuntimeViewId {
    /// Creates a runtime view identifier.
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

/// Text visual record attached to a runtime view node.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeTextVisual {
    text: String,
    font_size: f32,
    color: Color,
}

impl RuntimeTextVisual {
    /// Creates a text visual.
    #[must_use]
    pub fn new(text: impl Into<String>, font_size: f32, color: Color) -> Self {
        Self {
            text: text.into(),
            font_size,
            color,
        }
    }

    /// Returns the text payload.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the requested font size in logical pixels.
    #[must_use]
    pub const fn font_size(&self) -> f32 {
        self.font_size
    }

    /// Returns the text color.
    #[must_use]
    pub const fn color(&self) -> Color {
        self.color
    }
}

/// Visual payload attached to a runtime view node.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeVisual {
    /// Node has no direct visual output.
    None,
    /// Solid fill visual.
    Fill(Color),
    /// Text visual.
    Text(RuntimeTextVisual),
}

/// Retained runtime view node.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeViewNode {
    id: RuntimeViewId,
    layout_style: LayoutStyle,
    visual: RuntimeVisual,
}

impl RuntimeViewNode {
    /// Creates a runtime view node.
    #[must_use]
    pub const fn new(id: RuntimeViewId, layout_style: LayoutStyle, visual: RuntimeVisual) -> Self {
        Self {
            id,
            layout_style,
            visual,
        }
    }

    /// Returns the node identifier.
    #[must_use]
    pub const fn id(&self) -> &RuntimeViewId {
        &self.id
    }

    /// Returns the node layout style.
    #[must_use]
    pub const fn layout_style(&self) -> &LayoutStyle {
        &self.layout_style
    }

    /// Returns the visual payload.
    #[must_use]
    pub const fn visual(&self) -> &RuntimeVisual {
        &self.visual
    }
}

/// Runtime scene bridge error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeSceneError {
    /// Parent node is missing.
    MissingParent(String),
    /// Node is missing.
    MissingNode(String),
    /// Node already exists.
    DuplicateNode(String),
    /// Node contains invalid runtime data.
    InvalidNode(String),
    /// Layer export failed because a paint record is invalid.
    InvalidLayer(String),
}

impl From<LayoutTreeError> for RuntimeSceneError {
    fn from(error: LayoutTreeError) -> Self {
        match error {
            LayoutTreeError::MissingParent(id) => Self::MissingParent(id),
            LayoutTreeError::DuplicateNode(id) => Self::DuplicateNode(id),
            LayoutTreeError::InvalidNodeId(id)
            | LayoutTreeError::InvalidStyle(id)
            | LayoutTreeError::ComputeFailed(id) => Self::InvalidNode(id),
            LayoutTreeError::InvalidViewport => Self::InvalidNode("viewport".to_string()),
        }
    }
}

impl From<SceneGraphError> for RuntimeSceneError {
    fn from(error: SceneGraphError) -> Self {
        match error {
            SceneGraphError::MissingParent(id) | SceneGraphError::MissingNode(id) => {
                Self::MissingNode(id)
            }
            SceneGraphError::DuplicateNode(id) => Self::DuplicateNode(id),
            SceneGraphError::InvalidNodeId(id)
            | SceneGraphError::InvalidGeometry(id)
            | SceneGraphError::InvalidTransform(id)
            | SceneGraphError::InvalidOpacity(id)
            | SceneGraphError::InvalidAccessibilityRef(id) => Self::InvalidNode(id),
        }
    }
}

impl From<LayerValidationError> for RuntimeSceneError {
    fn from(error: LayerValidationError) -> Self {
        Self::InvalidLayer(error.rule().to_string())
    }
}

/// Runtime draw command with resolved geometry.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeDrawCommand {
    /// Solid fill command.
    Fill {
        /// View node that produced this command.
        id: RuntimeViewId,
        /// Resolved layout geometry.
        geometry: Geometry,
        /// Fill color.
        color: Color,
    },
    /// Text draw command.
    Text {
        /// View node that produced this command.
        id: RuntimeViewId,
        /// Resolved layout geometry.
        geometry: Geometry,
        /// Text payload.
        text: String,
        /// Font size in logical pixels.
        font_size: f32,
        /// Text color.
        color: Color,
    },
}

impl RuntimeDrawCommand {
    /// Returns the view ID that produced this command.
    #[must_use]
    pub const fn id(&self) -> &RuntimeViewId {
        match self {
            Self::Fill { id, .. } | Self::Text { id, .. } => id,
        }
    }

    /// Returns the resolved draw geometry.
    #[must_use]
    pub const fn geometry(&self) -> Geometry {
        match self {
            Self::Fill { geometry, .. } | Self::Text { geometry, .. } => *geometry,
        }
    }
}

/// Runtime scene bridge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RuntimeSceneBridge {
    viewport: Viewport,
}

impl RuntimeSceneBridge {
    /// Creates a scene bridge for a viewport.
    #[must_use]
    pub const fn new(viewport: Viewport) -> Self {
        Self { viewport }
    }

    /// Builds a renderable runtime scene frame.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeSceneError`] when the tree cannot be converted into layout or scene data.
    pub fn build(&self, tree: &RuntimeViewTree) -> Result<RuntimeSceneFrame, RuntimeSceneError> {
        tree.validate_for_bridge()?;
        let layout_tree = tree.to_layout_tree()?;
        let layout = layout_tree.try_compute_layout(self.viewport)?;
        let geometry = collect_geometry(tree, &layout)?;
        let scene = tree.to_scene_graph(&layout)?;
        let (layers, draw_commands) = tree.to_layers_and_draw_commands(&geometry);
        let paint_commands = export_paint_commands(&layers)?;

        Ok(RuntimeSceneFrame {
            layout,
            scene,
            layers,
            paint_commands,
            draw_commands,
            geometry,
            invalidated_view_ids: tree.invalidated_view_ids(),
        })
    }
}

/// Output of a runtime scene bridge build.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeSceneFrame {
    layout: LayoutOutput,
    scene: SceneGraph,
    layers: LayerStack,
    paint_commands: PaintCommandList,
    draw_commands: Vec<RuntimeDrawCommand>,
    geometry: Vec<(RuntimeViewId, Geometry)>,
    invalidated_view_ids: Vec<RuntimeViewId>,
}

impl RuntimeSceneFrame {
    /// Returns computed layout output.
    #[must_use]
    pub const fn layout(&self) -> &LayoutOutput {
        &self.layout
    }

    /// Returns the retained scene graph.
    #[must_use]
    pub const fn scene(&self) -> &SceneGraph {
        &self.scene
    }

    /// Returns the deterministic layer stack.
    #[must_use]
    pub const fn layers(&self) -> &LayerStack {
        &self.layers
    }

    /// Returns exported stable paint commands.
    #[must_use]
    pub const fn paint_commands(&self) -> &PaintCommandList {
        &self.paint_commands
    }

    /// Returns geometry-rich draw commands.
    #[must_use]
    pub fn draw_commands(&self) -> &[RuntimeDrawCommand] {
        &self.draw_commands
    }

    /// Returns resolved render geometry by runtime view ID.
    #[must_use]
    pub fn geometry_for(&self, view_id: &RuntimeViewId) -> Option<Geometry> {
        self.geometry
            .iter()
            .find(|(id, _)| id.as_str() == view_id.as_str())
            .map(|(_, geometry)| *geometry)
    }

    /// Returns invalidated runtime view IDs.
    #[must_use]
    pub fn invalidated_view_ids(&self) -> &[RuntimeViewId] {
        &self.invalidated_view_ids
    }
}

/// Retained runtime view tree.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeViewTree {
    root_id: RuntimeViewId,
    entries: Vec<RuntimeViewEntry>,
}

impl RuntimeViewTree {
    /// Creates a runtime view tree with a root node.
    #[must_use]
    pub fn new(root: RuntimeViewNode) -> Self {
        let root_id = root.id().clone();
        Self {
            root_id,
            entries: vec![RuntimeViewEntry {
                node: root,
                parent: None,
                children: Vec::new(),
                invalidated: false,
            }],
        }
    }

    /// Adds a child view to an existing parent.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeSceneError`] when the parent is missing or the child ID already exists.
    pub fn with_child(
        mut self,
        parent_id: &RuntimeViewId,
        child: RuntimeViewNode,
    ) -> Result<Self, RuntimeSceneError> {
        if self.index_of(child.id()).is_some() {
            return Err(RuntimeSceneError::DuplicateNode(
                child.id().as_str().to_string(),
            ));
        }
        let Some(parent_index) = self.index_of(parent_id) else {
            return Err(RuntimeSceneError::MissingParent(
                parent_id.as_str().to_string(),
            ));
        };
        let child_id = child.id().clone();
        self.entries[parent_index].children.push(child_id);
        self.entries.push(RuntimeViewEntry {
            node: child,
            parent: Some(parent_id.clone()),
            children: Vec::new(),
            invalidated: false,
        });
        Ok(self)
    }

    /// Returns the root node identifier.
    #[must_use]
    pub const fn root_id(&self) -> &RuntimeViewId {
        &self.root_id
    }

    /// Returns a runtime node by identifier.
    #[must_use]
    pub fn node(&self, node_id: &RuntimeViewId) -> Option<&RuntimeViewNode> {
        self.entry(node_id).map(|entry| &entry.node)
    }

    /// Returns child identifiers in insertion order.
    #[must_use]
    pub fn children_of(&self, node_id: &RuntimeViewId) -> &[RuntimeViewId] {
        self.entry(node_id)
            .map_or(&[], |entry| entry.children.as_slice())
    }

    /// Marks a runtime view as invalidated.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeSceneError`] when the node is missing.
    pub fn invalidate(mut self, node_id: &RuntimeViewId) -> Result<Self, RuntimeSceneError> {
        let Some(index) = self.index_of(node_id) else {
            return Err(RuntimeSceneError::MissingNode(node_id.as_str().to_string()));
        };
        self.entries[index].invalidated = true;
        Ok(self)
    }

    fn invalidated_view_ids(&self) -> Vec<RuntimeViewId> {
        self.entries
            .iter()
            .filter(|entry| entry.invalidated)
            .map(|entry| entry.node.id().clone())
            .collect()
    }

    fn entry(&self, node_id: &RuntimeViewId) -> Option<&RuntimeViewEntry> {
        self.entries
            .iter()
            .find(|entry| entry.node.id().as_str() == node_id.as_str())
    }

    fn index_of(&self, node_id: &RuntimeViewId) -> Option<usize> {
        self.entries
            .iter()
            .position(|entry| entry.node.id().as_str() == node_id.as_str())
    }

    fn validate_for_bridge(&self) -> Result<(), RuntimeSceneError> {
        for entry in &self.entries {
            validate_runtime_node(&entry.node)?;
        }
        Ok(())
    }

    fn to_layout_tree(&self) -> Result<LayoutTree, RuntimeSceneError> {
        let root = self
            .entries
            .first()
            .ok_or_else(|| RuntimeSceneError::MissingNode("root".to_string()))?;
        let mut layout_tree = LayoutTree::new(LayoutNode::new(
            LayoutNodeId::new(root.node.id().as_str()),
            root.node.layout_style().clone(),
        ));
        for entry in self.entries.iter().skip(1) {
            let Some(parent_id) = entry.parent.as_ref() else {
                return Err(RuntimeSceneError::MissingParent(
                    entry.node.id().as_str().to_string(),
                ));
            };
            layout_tree = layout_tree.with_child(
                LayoutNodeId::new(parent_id.as_str()),
                LayoutNode::new(
                    LayoutNodeId::new(entry.node.id().as_str()),
                    entry.node.layout_style().clone(),
                ),
            )?;
        }
        Ok(layout_tree)
    }

    fn to_scene_graph(&self, layout: &LayoutOutput) -> Result<SceneGraph, RuntimeSceneError> {
        let root = self
            .entries
            .first()
            .ok_or_else(|| RuntimeSceneError::MissingNode("root".to_string()))?;
        let mut scene = SceneGraph::new(scene_node_for(root.node.id(), layout)?);
        for entry in self.entries.iter().skip(1) {
            let Some(parent_id) = entry.parent.as_ref() else {
                return Err(RuntimeSceneError::MissingParent(
                    entry.node.id().as_str().to_string(),
                ));
            };
            scene = scene.with_child(
                SceneNodeId::new(parent_id.as_str()),
                scene_node_for(entry.node.id(), layout)?,
            )?;
        }
        for entry in self.entries.iter().filter(|entry| entry.invalidated) {
            scene = scene.invalidate(
                &SceneNodeId::new(entry.node.id().as_str()),
                InvalidationReason::Paint,
            )?;
        }
        Ok(scene)
    }

    fn to_layers_and_draw_commands(
        &self,
        geometry: &[(RuntimeViewId, Geometry)],
    ) -> (LayerStack, Vec<RuntimeDrawCommand>) {
        let mut layers = LayerStack::new();
        let mut draw_commands = Vec::new();
        for (order, entry) in self.entries.iter().enumerate() {
            let Some(geometry) = geometry_for(geometry, entry.node.id()) else {
                continue;
            };
            match entry.node.visual() {
                RuntimeVisual::None => {}
                RuntimeVisual::Fill(color) => {
                    layers = layers.with_layer(PaintLayer::new(
                        entry.node.id().as_str(),
                        checked_order(order),
                        LayerKind::Fill(*color),
                    ));
                    draw_commands.push(RuntimeDrawCommand::Fill {
                        id: entry.node.id().clone(),
                        geometry,
                        color: *color,
                    });
                }
                RuntimeVisual::Text(text) => {
                    layers = layers.with_layer(PaintLayer::new(
                        entry.node.id().as_str(),
                        checked_order(order),
                        LayerKind::Text(TextLayer::new(text.text())),
                    ));
                    draw_commands.push(RuntimeDrawCommand::Text {
                        id: entry.node.id().clone(),
                        geometry,
                        text: text.text().to_string(),
                        font_size: text.font_size(),
                        color: text.color(),
                    });
                }
            }
        }
        (layers, draw_commands)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct RuntimeViewEntry {
    node: RuntimeViewNode,
    parent: Option<RuntimeViewId>,
    children: Vec<RuntimeViewId>,
    invalidated: bool,
}

fn collect_geometry(
    tree: &RuntimeViewTree,
    layout: &LayoutOutput,
) -> Result<Vec<(RuntimeViewId, Geometry)>, RuntimeSceneError> {
    tree.entries
        .iter()
        .map(|entry| {
            let layout_id = LayoutNodeId::new(entry.node.id().as_str());
            let geometry = layout.geometry(&layout_id).ok_or_else(|| {
                RuntimeSceneError::MissingNode(entry.node.id().as_str().to_string())
            })?;
            Ok((entry.node.id().clone(), render_geometry(*geometry)))
        })
        .collect()
}

fn scene_node_for(
    view_id: &RuntimeViewId,
    layout: &LayoutOutput,
) -> Result<SceneNode, RuntimeSceneError> {
    let layout_id = LayoutNodeId::new(view_id.as_str());
    let geometry = layout
        .geometry(&layout_id)
        .ok_or_else(|| RuntimeSceneError::MissingNode(view_id.as_str().to_string()))?;
    let node_geometry = render_geometry(*geometry);
    let mut node = SceneNode::new(SceneNodeId::new(view_id.as_str()))
        .with_layout(node_geometry)
        .with_hit_test(node_geometry);
    if let Some(clip) = layout.clip(&layout_id) {
        node = node.with_clip(render_geometry(*clip));
    }
    Ok(node)
}

fn render_geometry(geometry: ComputedGeometry) -> Geometry {
    Geometry::new(geometry.x, geometry.y, geometry.width, geometry.height)
}

fn geometry_for(
    geometry: &[(RuntimeViewId, Geometry)],
    view_id: &RuntimeViewId,
) -> Option<Geometry> {
    geometry
        .iter()
        .find(|(id, _)| id.as_str() == view_id.as_str())
        .map(|(_, geometry)| *geometry)
}

fn checked_order(order: usize) -> i32 {
    i32::try_from(order).unwrap_or(i32::MAX)
}

fn validate_runtime_node(node: &RuntimeViewNode) -> Result<(), RuntimeSceneError> {
    if !is_valid_runtime_id(node.id().as_str()) {
        return Err(RuntimeSceneError::InvalidNode(
            node.id().as_str().to_string(),
        ));
    }
    if let RuntimeVisual::Text(text) = node.visual()
        && (!text.font_size().is_finite() || text.font_size() <= 0.0)
    {
        return Err(RuntimeSceneError::InvalidNode(
            node.id().as_str().to_string(),
        ));
    }
    Ok(())
}

fn is_valid_runtime_id(value: &str) -> bool {
    !value.trim().is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
}

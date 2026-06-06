//! Retained runtime view records and runtime-to-render bridge types.

use std::collections::BTreeSet;

use hawk2ui_layout::{
    ComputedGeometry, LayoutNode, LayoutNodeId, LayoutOutput, LayoutStyle, LayoutTextMeasurer,
    LayoutTree, LayoutTreeError, TextMeasureInput, TextMeasureMode, Viewport,
};
use hawk2ui_render::{
    BackendError, Color, CustomDrawSurface, CustomSurfaceCapability, CustomSurfaceCategory,
    CustomSurfaceDataSnapshot, Geometry, GlowLayer, GradientLayer, InvalidationReason, LayerKind,
    LayerStack, LayerValidationError, PaintCommandList, PaintLayer, RendererCacheInvalidator,
    RoundedRect, SceneGraph, SceneGraphDiff, SceneGraphError, SceneNode, SceneNodeId,
    ShaderEffectChildInput, ShaderEffectUniform, ShadowLayer, TextLayer, Transform,
    export_paint_commands,
};

const MAX_RUNTIME_SHADER_EFFECT_SOURCE_BYTES: usize = 64 * 1024;

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
    font_family: String,
    font_size: f32,
    color: Color,
}

impl RuntimeTextVisual {
    /// Creates a text visual.
    #[must_use]
    pub fn new(text: impl Into<String>, font_size: f32, color: Color) -> Self {
        Self {
            text: text.into(),
            font_family: "Hawk2UI Sans".to_string(),
            font_size,
            color,
        }
    }

    /// Returns the text payload.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Replaces the text payload while preserving font and color settings.
    #[must_use]
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }

    /// Returns the requested font size in logical pixels.
    #[must_use]
    pub const fn font_size(&self) -> f32 {
        self.font_size
    }

    /// Sets the preferred font family used by text measurement.
    #[must_use]
    pub fn with_font_family(mut self, font_family: impl Into<String>) -> Self {
        self.font_family = font_family.into();
        self
    }

    /// Returns the preferred font family.
    #[must_use]
    pub fn font_family(&self) -> &str {
        &self.font_family
    }

    /// Returns the text color.
    #[must_use]
    pub const fn color(&self) -> Color {
        self.color
    }
}

/// Custom draw surface visual attached to a runtime view node.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeCustomSurfaceVisual {
    category: CustomSurfaceCategory,
    capabilities: Vec<CustomSurfaceCapability>,
    data: CustomSurfaceDataSnapshot,
    next_frame: Option<u64>,
    frame_interval: Option<u64>,
}

impl RuntimeCustomSurfaceVisual {
    /// Creates a custom surface visual for category-specific renderer hooks.
    #[must_use]
    pub fn new(category: CustomSurfaceCategory) -> Self {
        Self {
            category,
            capabilities: Vec::new(),
            data: CustomSurfaceDataSnapshot::default(),
            next_frame: None,
            frame_interval: None,
        }
    }

    /// Adds a renderer capability requirement for the custom surface.
    #[must_use]
    pub fn with_capability(mut self, capability: CustomSurfaceCapability) -> Self {
        if !self.capabilities.contains(&capability) {
            self.capabilities.push(capability);
        }
        self
    }

    /// Attaches a plugin-safe realtime data snapshot.
    #[must_use]
    pub fn with_data_snapshot(mut self, data: CustomSurfaceDataSnapshot) -> Self {
        self.data = data;
        self
    }

    /// Schedules the next host frame at which this surface may draw.
    #[must_use]
    pub const fn schedule_frame(mut self, frame: u64) -> Self {
        self.next_frame = Some(frame);
        self
    }

    /// Sets the minimum host-frame interval between draws.
    #[must_use]
    pub const fn with_frame_interval(mut self, frame_interval: u64) -> Self {
        self.frame_interval = Some(if frame_interval == 0 {
            1
        } else {
            frame_interval
        });
        self
    }

    fn to_surface(&self, id: &RuntimeViewId, geometry: Geometry) -> CustomDrawSurface {
        let mut surface = CustomDrawSurface::new(id.as_str(), self.category, geometry);
        for capability in &self.capabilities {
            surface = surface.with_capability(*capability);
        }
        if let Some(next_frame) = self.next_frame {
            surface = surface.schedule_frame(next_frame);
        }
        if let Some(frame_interval) = self.frame_interval {
            surface = surface.with_frame_interval(frame_interval);
        }
        surface
    }

    fn data(&self) -> CustomSurfaceDataSnapshot {
        self.data.clone()
    }
}

/// Linear gradient visual data for a runtime-styled box.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeLinearGradient {
    start: Color,
    end: Color,
}

impl RuntimeLinearGradient {
    /// Creates a left-to-right linear gradient.
    #[must_use]
    pub const fn new(start: Color, end: Color) -> Self {
        Self { start, end }
    }

    /// Returns the gradient start color.
    #[must_use]
    pub const fn start(self) -> Color {
        self.start
    }

    /// Returns the gradient end color.
    #[must_use]
    pub const fn end(self) -> Color {
        self.end
    }
}

/// Shadow visual data for a runtime-styled box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RuntimeShadowEffect {
    offset_x: f32,
    offset_y: f32,
    blur_radius: f32,
    color: Color,
}

impl RuntimeShadowEffect {
    /// Creates a shadow effect.
    #[must_use]
    pub const fn new(offset_x: f32, offset_y: f32, blur_radius: f32, color: Color) -> Self {
        Self {
            offset_x,
            offset_y,
            blur_radius,
            color,
        }
    }

    /// Returns the horizontal shadow offset.
    #[must_use]
    pub const fn offset_x(self) -> f32 {
        self.offset_x
    }

    /// Returns the vertical shadow offset.
    #[must_use]
    pub const fn offset_y(self) -> f32 {
        self.offset_y
    }

    /// Returns the blur radius.
    #[must_use]
    pub const fn blur_radius(self) -> f32 {
        self.blur_radius
    }

    /// Returns the shadow color.
    #[must_use]
    pub const fn color(self) -> Color {
        self.color
    }
}

/// Glow visual data for a runtime-styled box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RuntimeGlowEffect {
    blur_radius: f32,
    color: Color,
}

impl RuntimeGlowEffect {
    /// Creates a glow effect.
    #[must_use]
    pub const fn new(blur_radius: f32, color: Color) -> Self {
        Self { blur_radius, color }
    }

    /// Returns the glow blur radius.
    #[must_use]
    pub const fn blur_radius(self) -> f32 {
        self.blur_radius
    }

    /// Returns the glow color.
    #[must_use]
    pub const fn color(self) -> Color {
        self.color
    }
}

/// Renderer-ready box visual with fills, gradients, and layer effects.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeStyledBoxVisual {
    fill: Option<Color>,
    gradient: Option<RuntimeLinearGradient>,
    border_radius: f32,
    shadow: Option<RuntimeShadowEffect>,
    glow: Option<RuntimeGlowEffect>,
    opacity: f32,
    transform: Transform,
}

impl RuntimeStyledBoxVisual {
    /// Creates a styled box visual with no fill or effects.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            fill: None,
            gradient: None,
            border_radius: 0.0,
            shadow: None,
            glow: None,
            opacity: 1.0,
            transform: Transform::identity(),
        }
    }

    /// Sets the solid fill color.
    #[must_use]
    pub const fn with_fill(mut self, fill: Color) -> Self {
        self.fill = Some(fill);
        self
    }

    /// Sets the linear gradient.
    #[must_use]
    pub const fn with_gradient(mut self, gradient: RuntimeLinearGradient) -> Self {
        self.gradient = Some(gradient);
        self
    }

    /// Sets the border radius.
    #[must_use]
    pub const fn with_border_radius(mut self, border_radius: f32) -> Self {
        self.border_radius = border_radius;
        self
    }

    /// Sets the shadow effect.
    #[must_use]
    pub const fn with_shadow(mut self, shadow: RuntimeShadowEffect) -> Self {
        self.shadow = Some(shadow);
        self
    }

    /// Sets the glow effect.
    #[must_use]
    pub const fn with_glow(mut self, glow: RuntimeGlowEffect) -> Self {
        self.glow = Some(glow);
        self
    }

    /// Sets the box opacity.
    #[must_use]
    pub const fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }

    /// Sets the local transform.
    #[must_use]
    pub const fn with_transform(mut self, transform: Transform) -> Self {
        self.transform = transform;
        self
    }

    /// Returns the solid fill color.
    #[must_use]
    pub const fn fill(&self) -> Option<Color> {
        self.fill
    }

    /// Returns the linear gradient.
    #[must_use]
    pub const fn gradient(&self) -> Option<RuntimeLinearGradient> {
        self.gradient
    }

    /// Returns the border radius.
    #[must_use]
    pub const fn border_radius(&self) -> f32 {
        self.border_radius
    }

    /// Returns the shadow effect.
    #[must_use]
    pub const fn shadow(&self) -> Option<RuntimeShadowEffect> {
        self.shadow
    }

    /// Returns the glow effect.
    #[must_use]
    pub const fn glow(&self) -> Option<RuntimeGlowEffect> {
        self.glow
    }

    /// Returns the opacity.
    #[must_use]
    pub const fn opacity(&self) -> f32 {
        self.opacity
    }

    /// Returns the local transform.
    #[must_use]
    pub const fn transform(&self) -> Transform {
        self.transform
    }
}

impl Default for RuntimeStyledBoxVisual {
    fn default() -> Self {
        Self::new()
    }
}

/// Runtime shader effect visual attached to a retained view node.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeShaderEffectVisual {
    effect_id: String,
    source: String,
    uniforms: Vec<ShaderEffectUniform>,
    children: Vec<ShaderEffectChildInput>,
}

impl RuntimeShaderEffectVisual {
    /// Creates a runtime shader effect visual from a stable effect ID and backend shader source.
    #[must_use]
    pub fn new(effect_id: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            effect_id: effect_id.into(),
            source: source.into(),
            uniforms: Vec::new(),
            children: Vec::new(),
        }
    }

    /// Adds a typed uniform binding.
    #[must_use]
    pub fn with_uniform(mut self, uniform: ShaderEffectUniform) -> Self {
        self.uniforms.push(uniform);
        self
    }

    /// Adds an image child binding.
    #[must_use]
    pub fn with_child(mut self, child: ShaderEffectChildInput) -> Self {
        self.children.push(child);
        self
    }

    /// Returns the stable effect ID.
    #[must_use]
    pub fn effect_id(&self) -> &str {
        &self.effect_id
    }

    /// Returns the backend shader source.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Returns typed uniform bindings.
    #[must_use]
    pub fn uniforms(&self) -> &[ShaderEffectUniform] {
        &self.uniforms
    }

    /// Returns image child bindings.
    #[must_use]
    pub fn children(&self) -> &[ShaderEffectChildInput] {
        &self.children
    }
}

/// Visual payload attached to a runtime view node.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeVisual {
    /// Node has no direct visual output.
    None,
    /// Solid fill visual.
    Fill(Color),
    /// Box visual with renderer-ready layer effects.
    StyledBox(RuntimeStyledBoxVisual),
    /// Text visual.
    Text(RuntimeTextVisual),
    /// Compiled image asset visual.
    ImageAsset(String),
    /// Compiled vector asset visual.
    VectorAsset(String),
    /// Runtime shader effect visual.
    ShaderEffect(RuntimeShaderEffectVisual),
    /// Custom renderer-owned draw surface visual.
    CustomSurface(RuntimeCustomSurfaceVisual),
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
            | SceneGraphError::InvalidAccessibilityRef(id)
            | SceneGraphError::InvalidLayerId(id)
            | SceneGraphError::InvalidEffectRef(id)
            | SceneGraphError::InvalidOpacityGroupId(id) => Self::InvalidNode(id),
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
    /// Styled box command.
    StyledBox {
        /// View node that produced this command.
        id: RuntimeViewId,
        /// Resolved layout geometry.
        geometry: Geometry,
        /// Renderer-ready styled box visual.
        visual: RuntimeStyledBoxVisual,
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
    /// Compiled image asset draw command.
    ImageAsset {
        /// View node that produced this command.
        id: RuntimeViewId,
        /// Resolved layout geometry.
        geometry: Geometry,
        /// Compiled asset identifier.
        asset_id: String,
    },
    /// Compiled vector asset draw command.
    VectorAsset {
        /// View node that produced this command.
        id: RuntimeViewId,
        /// Resolved layout geometry.
        geometry: Geometry,
        /// Compiled asset identifier.
        asset_id: String,
    },
    /// Runtime shader effect draw command.
    ShaderEffect {
        /// View node that produced this command.
        id: RuntimeViewId,
        /// Resolved layout geometry.
        geometry: Geometry,
        /// Runtime shader effect and bindings.
        effect: RuntimeShaderEffectVisual,
    },
    /// Custom renderer-owned draw surface command.
    CustomSurface {
        /// View node that produced this command.
        id: RuntimeViewId,
        /// Resolved layout geometry.
        geometry: Geometry,
        /// Surface metadata and scheduling information.
        surface: CustomDrawSurface,
        /// Plugin-safe realtime data snapshot.
        data: CustomSurfaceDataSnapshot,
    },
}

impl RuntimeDrawCommand {
    /// Returns the view ID that produced this command.
    #[must_use]
    pub const fn id(&self) -> &RuntimeViewId {
        match self {
            Self::Fill { id, .. }
            | Self::StyledBox { id, .. }
            | Self::Text { id, .. }
            | Self::ImageAsset { id, .. }
            | Self::VectorAsset { id, .. }
            | Self::ShaderEffect { id, .. }
            | Self::CustomSurface { id, .. } => id,
        }
    }

    /// Returns the resolved draw geometry.
    #[must_use]
    pub const fn geometry(&self) -> Geometry {
        match self {
            Self::Fill { geometry, .. }
            | Self::StyledBox { geometry, .. }
            | Self::Text { geometry, .. }
            | Self::ImageAsset { geometry, .. }
            | Self::VectorAsset { geometry, .. }
            | Self::ShaderEffect { geometry, .. }
            | Self::CustomSurface { geometry, .. } => *geometry,
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
        let absolute = resolve_absolute_geometry(tree, &layout)?;
        let geometry = collect_geometry(&absolute);
        let scene = tree.to_scene_graph(&absolute)?;
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

    /// Builds a renderable runtime scene frame with text measurement feeding intrinsic layout.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeSceneError`] when the tree cannot be converted or measured for layout.
    pub fn build_with_text_measurer(
        &self,
        tree: &RuntimeViewTree,
        measurer: &dyn LayoutTextMeasurer,
    ) -> Result<RuntimeSceneFrame, RuntimeSceneError> {
        tree.validate_for_bridge()?;
        let layout_tree = tree.to_layout_tree()?;
        let layout = layout_tree.try_compute_layout_with_text_measurer(self.viewport, measurer)?;
        let absolute = resolve_absolute_geometry(tree, &layout)?;
        let geometry = collect_geometry(&absolute);
        let scene = tree.to_scene_graph(&absolute)?;
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

    /// Computes the runtime update plan needed to move from a previous frame to this frame.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeSceneError`] when either frame contains invalid retained scene data.
    pub fn diff_from(&self, previous: &Self) -> Result<RuntimeSceneUpdate, RuntimeSceneError> {
        let diff = previous.scene().diff(self.scene())?;
        Ok(RuntimeSceneUpdate::from_scene_diff(&diff))
    }
}

/// Runtime scene update plan for repaint and cache invalidation consumers.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeSceneUpdate {
    repaint_bounds: Option<Geometry>,
    affected_view_ids: Vec<RuntimeViewId>,
    cache_invalidated_view_ids: Vec<RuntimeViewId>,
}

impl RuntimeSceneUpdate {
    fn from_scene_diff(diff: &SceneGraphDiff) -> Self {
        let mut affected_view_ids = BTreeSet::new();
        for node_id in diff
            .added_node_ids()
            .iter()
            .chain(diff.removed_node_ids())
            .chain(diff.changed_node_ids())
            .chain(diff.cache_invalidated_node_ids())
        {
            affected_view_ids.insert(RuntimeViewId::new(node_id.as_str()));
        }
        Self {
            repaint_bounds: diff.repaint_bounds(),
            affected_view_ids: affected_view_ids.into_iter().collect(),
            cache_invalidated_view_ids: scene_ids_to_view_ids(diff.cache_invalidated_node_ids()),
        }
    }

    /// Returns whether the update needs a host repaint.
    #[must_use]
    pub fn requires_repaint(&self) -> bool {
        self.repaint_bounds.is_some() || !self.affected_view_ids.is_empty()
    }

    /// Returns aggregate dirty bounds for the repaint request.
    #[must_use]
    pub const fn repaint_bounds(&self) -> Option<Geometry> {
        self.repaint_bounds
    }

    /// Returns runtime view IDs affected by added, removed, changed, or invalidated scene data.
    #[must_use]
    pub fn affected_view_ids(&self) -> &[RuntimeViewId] {
        &self.affected_view_ids
    }

    /// Returns runtime view IDs whose cached render content must be evicted before replay.
    #[must_use]
    pub fn cache_invalidated_view_ids(&self) -> &[RuntimeViewId] {
        &self.cache_invalidated_view_ids
    }

    /// Applies explicit cache evictions to a renderer backend before frame replay.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the backend cannot invalidate a required cache entry.
    pub fn evict_backend_caches(
        &self,
        backend: &mut impl RendererCacheInvalidator,
    ) -> Result<(), BackendError> {
        for view_id in &self.cache_invalidated_view_ids {
            backend.invalidate_backend_cache(view_id.as_str())?;
        }
        Ok(())
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

    /// Replaces a runtime node visual and marks that node for repaint.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeSceneError`] when the node is missing.
    pub fn update_visual(
        mut self,
        node_id: &RuntimeViewId,
        visual: RuntimeVisual,
    ) -> Result<Self, RuntimeSceneError> {
        let Some(index) = self.index_of(node_id) else {
            return Err(RuntimeSceneError::MissingNode(node_id.as_str().to_string()));
        };
        self.entries[index].node.visual = visual;
        self.entries[index].invalidated = true;
        Ok(self)
    }

    /// Replaces a runtime node layout style and marks that node for relayout/repaint.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeSceneError`] when the node is missing.
    pub fn update_layout_style(
        mut self,
        node_id: &RuntimeViewId,
        layout_style: LayoutStyle,
    ) -> Result<Self, RuntimeSceneError> {
        let Some(index) = self.index_of(node_id) else {
            return Err(RuntimeSceneError::MissingNode(node_id.as_str().to_string()));
        };
        self.entries[index].node.layout_style = layout_style;
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
        let mut layout_tree = LayoutTree::new(layout_node_for(&root.node));
        for entry in self.entries.iter().skip(1) {
            let Some(parent_id) = entry.parent.as_ref() else {
                return Err(RuntimeSceneError::MissingParent(
                    entry.node.id().as_str().to_string(),
                ));
            };
            layout_tree = layout_tree.with_child(
                LayoutNodeId::new(parent_id.as_str()),
                layout_node_for(&entry.node),
            )?;
        }
        Ok(layout_tree)
    }

    fn to_scene_graph(&self, absolute: &AbsoluteGeometry) -> Result<SceneGraph, RuntimeSceneError> {
        let root = self
            .entries
            .first()
            .ok_or_else(|| RuntimeSceneError::MissingNode("root".to_string()))?;
        let mut scene = SceneGraph::new(scene_node_for(root.node.id(), absolute)?);
        for entry in self.entries.iter().skip(1) {
            let Some(parent_id) = entry.parent.as_ref() else {
                return Err(RuntimeSceneError::MissingParent(
                    entry.node.id().as_str().to_string(),
                ));
            };
            scene = scene.with_child(
                SceneNodeId::new(parent_id.as_str()),
                scene_node_for(entry.node.id(), absolute)?,
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
                RuntimeVisual::StyledBox(visual) => {
                    layers = add_styled_box_layers(
                        layers,
                        entry.node.id(),
                        checked_order_base(order),
                        visual,
                    );
                    draw_commands.push(RuntimeDrawCommand::StyledBox {
                        id: entry.node.id().clone(),
                        geometry,
                        visual: visual.clone(),
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
                RuntimeVisual::ImageAsset(asset_id) => {
                    draw_commands.push(RuntimeDrawCommand::ImageAsset {
                        id: entry.node.id().clone(),
                        geometry,
                        asset_id: asset_id.clone(),
                    });
                }
                RuntimeVisual::VectorAsset(asset_id) => {
                    draw_commands.push(RuntimeDrawCommand::VectorAsset {
                        id: entry.node.id().clone(),
                        geometry,
                        asset_id: asset_id.clone(),
                    });
                }
                RuntimeVisual::ShaderEffect(effect) => {
                    draw_commands.push(RuntimeDrawCommand::ShaderEffect {
                        id: entry.node.id().clone(),
                        geometry,
                        effect: effect.clone(),
                    });
                }
                RuntimeVisual::CustomSurface(surface) => {
                    draw_commands.push(RuntimeDrawCommand::CustomSurface {
                        id: entry.node.id().clone(),
                        geometry,
                        surface: surface.to_surface(entry.node.id(), geometry),
                        data: surface.data(),
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

/// Absolute (viewport-space) geometry for every runtime view node.
///
/// Taffy reports each node's location relative to its parent's border box, so the raw
/// [`LayoutOutput`] coordinates are parent-relative. The wired renderer blits each draw command
/// verbatim and never walks the tree to accumulate ancestor offsets, so the bridge resolves
/// absolute coordinates here — for both the flat draw-command path and the scene graph's
/// layout/hit-test rects — before handing geometry to the renderer or accessibility consumers.
struct AbsoluteGeometry {
    nodes: Vec<(RuntimeViewId, ComputedGeometry)>,
    clips: Vec<(RuntimeViewId, ComputedGeometry)>,
}

impl AbsoluteGeometry {
    fn node(&self, view_id: &RuntimeViewId) -> Option<ComputedGeometry> {
        self.nodes
            .iter()
            .find(|(id, _)| id.as_str() == view_id.as_str())
            .map(|(_, geometry)| *geometry)
    }

    fn clip(&self, view_id: &RuntimeViewId) -> Option<ComputedGeometry> {
        self.clips
            .iter()
            .find(|(id, _)| id.as_str() == view_id.as_str())
            .map(|(_, geometry)| *geometry)
    }
}

/// Resolves absolute geometry for every node by accumulating parent-relative Taffy locations down
/// the ancestor chain (`absolute(node) = location(node) + absolute(parent)`, root at the origin).
///
/// `entries` is ordered parent-before-child — [`RuntimeViewTree::with_child`] only appends a child
/// after its parent already exists — so each parent's absolute origin is resolved before its
/// children are visited. A child whose parent is not yet resolved is a malformed tree and surfaces
/// as [`RuntimeSceneError::MissingParent`].
fn resolve_absolute_geometry(
    tree: &RuntimeViewTree,
    layout: &LayoutOutput,
) -> Result<AbsoluteGeometry, RuntimeSceneError> {
    let mut nodes: Vec<(RuntimeViewId, ComputedGeometry)> = Vec::with_capacity(tree.entries.len());
    let mut clips: Vec<(RuntimeViewId, ComputedGeometry)> = Vec::new();
    for entry in &tree.entries {
        let view_id = entry.node.id();
        let layout_id = LayoutNodeId::new(view_id.as_str());
        let relative = *layout
            .geometry(&layout_id)
            .ok_or_else(|| RuntimeSceneError::MissingNode(view_id.as_str().to_string()))?;
        let (origin_x, origin_y) = match entry.parent.as_ref() {
            Some(parent_id) => {
                let parent = nodes
                    .iter()
                    .find(|(id, _)| id.as_str() == parent_id.as_str())
                    .map(|(_, geometry)| *geometry)
                    .ok_or_else(|| {
                        RuntimeSceneError::MissingParent(view_id.as_str().to_string())
                    })?;
                (parent.x, parent.y)
            }
            None => (0.0, 0.0),
        };
        if let Some(clip) = layout.clip(&layout_id) {
            clips.push((
                view_id.clone(),
                ComputedGeometry::new(
                    origin_x + clip.x,
                    origin_y + clip.y,
                    clip.width,
                    clip.height,
                    clip.absolute,
                ),
            ));
        }
        nodes.push((
            view_id.clone(),
            ComputedGeometry::new(
                origin_x + relative.x,
                origin_y + relative.y,
                relative.width,
                relative.height,
                relative.absolute,
            ),
        ));
    }
    Ok(AbsoluteGeometry { nodes, clips })
}

fn collect_geometry(absolute: &AbsoluteGeometry) -> Vec<(RuntimeViewId, Geometry)> {
    absolute
        .nodes
        .iter()
        .map(|(view_id, geometry)| (view_id.clone(), render_geometry(*geometry)))
        .collect()
}

fn scene_node_for(
    view_id: &RuntimeViewId,
    absolute: &AbsoluteGeometry,
) -> Result<SceneNode, RuntimeSceneError> {
    let geometry = absolute
        .node(view_id)
        .ok_or_else(|| RuntimeSceneError::MissingNode(view_id.as_str().to_string()))?;
    let node_geometry = render_geometry(geometry);
    let mut node = SceneNode::new(SceneNodeId::new(view_id.as_str()))
        .with_layout(node_geometry)
        .with_hit_test(node_geometry);
    if let Some(clip) = absolute.clip(view_id) {
        node = node.with_clip(render_geometry(clip));
    }
    Ok(node)
}

fn render_geometry(geometry: ComputedGeometry) -> Geometry {
    Geometry::new(geometry.x, geometry.y, geometry.width, geometry.height)
}

fn layout_node_for(node: &RuntimeViewNode) -> LayoutNode {
    let layout_node = LayoutNode::new(
        LayoutNodeId::new(node.id().as_str()),
        node.layout_style().clone(),
    );
    if let RuntimeVisual::Text(text) = node.visual() {
        layout_node.with_text_measurement(TextMeasureInput::new(
            text.text(),
            text.font_family(),
            text.font_size(),
            TextMeasureMode::Intrinsic,
        ))
    } else {
        layout_node
    }
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

fn scene_ids_to_view_ids(node_ids: &[SceneNodeId]) -> Vec<RuntimeViewId> {
    node_ids
        .iter()
        .map(|node_id| RuntimeViewId::new(node_id.as_str()))
        .collect()
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
    if let RuntimeVisual::Text(text) = node.visual()
        && text.font_family().trim().is_empty()
    {
        return Err(RuntimeSceneError::InvalidNode(
            node.id().as_str().to_string(),
        ));
    }
    match node.visual() {
        RuntimeVisual::StyledBox(visual) => {
            if !is_valid_styled_box_visual(visual) {
                return Err(RuntimeSceneError::InvalidNode(
                    node.id().as_str().to_string(),
                ));
            }
        }
        RuntimeVisual::ImageAsset(asset_id) | RuntimeVisual::VectorAsset(asset_id) => {
            if !is_valid_asset_id(asset_id) {
                return Err(RuntimeSceneError::InvalidNode(
                    node.id().as_str().to_string(),
                ));
            }
        }
        RuntimeVisual::ShaderEffect(effect) => {
            if !is_valid_shader_effect_visual(effect) {
                return Err(RuntimeSceneError::InvalidNode(
                    node.id().as_str().to_string(),
                ));
            }
        }
        RuntimeVisual::None
        | RuntimeVisual::Fill(_)
        | RuntimeVisual::Text(_)
        | RuntimeVisual::CustomSurface(_) => {}
    }
    Ok(())
}

fn add_styled_box_layers(
    mut layers: LayerStack,
    id: &RuntimeViewId,
    order_base: usize,
    visual: &RuntimeStyledBoxVisual,
) -> LayerStack {
    let mut step = 0usize;
    if visual.opacity() < 1.0 {
        layers = layers.with_layer(PaintLayer::new(
            id.as_str(),
            checked_order(order_base.saturating_add(step)),
            LayerKind::OpacityGroup(visual.opacity()),
        ));
        step = step.saturating_add(1);
    }
    if visual.transform() != Transform::identity() {
        layers = layers.with_layer(PaintLayer::new(
            id.as_str(),
            checked_order(order_base.saturating_add(step)),
            LayerKind::Transform(visual.transform()),
        ));
        step = step.saturating_add(1);
    }
    if let Some(shadow) = visual.shadow() {
        layers = layers.with_layer(PaintLayer::new(
            id.as_str(),
            checked_order(order_base.saturating_add(step)),
            LayerKind::Shadow(ShadowLayer::new(shadow.blur_radius())),
        ));
        step = step.saturating_add(1);
    }
    if visual.border_radius() > 0.0 {
        layers = layers.with_layer(PaintLayer::new(
            id.as_str(),
            checked_order(order_base.saturating_add(step)),
            LayerKind::RoundedRect(RoundedRect::new(visual.border_radius())),
        ));
        step = step.saturating_add(1);
    }
    if visual.gradient().is_some() {
        layers = layers.with_layer(PaintLayer::new(
            id.as_str(),
            checked_order(order_base.saturating_add(step)),
            LayerKind::Gradient(GradientLayer::linear()),
        ));
        step = step.saturating_add(1);
    } else if let Some(fill) = visual.fill() {
        layers = layers.with_layer(PaintLayer::new(
            id.as_str(),
            checked_order(order_base.saturating_add(step)),
            LayerKind::Fill(fill),
        ));
        step = step.saturating_add(1);
    }
    if let Some(glow) = visual.glow() {
        layers = layers.with_layer(PaintLayer::new(
            id.as_str(),
            checked_order(order_base.saturating_add(step)),
            LayerKind::Glow(GlowLayer::new(glow.blur_radius())),
        ));
    }
    layers
}

fn checked_order_base(order: usize) -> usize {
    order.saturating_mul(10)
}

fn is_valid_styled_box_visual(visual: &RuntimeStyledBoxVisual) -> bool {
    visual.border_radius().is_finite()
        && visual.border_radius() >= 0.0
        && visual.opacity().is_finite()
        && (0.0..=1.0).contains(&visual.opacity())
        && visual.transform().is_finite()
        && visual.shadow().is_none_or(|shadow| {
            shadow.offset_x().is_finite()
                && shadow.offset_y().is_finite()
                && shadow.blur_radius().is_finite()
                && shadow.blur_radius() >= 0.0
        })
        && visual
            .glow()
            .is_none_or(|glow| glow.blur_radius().is_finite() && glow.blur_radius() >= 0.0)
}

fn is_valid_shader_effect_visual(visual: &RuntimeShaderEffectVisual) -> bool {
    if !is_valid_runtime_id(visual.effect_id())
        || visual.source().trim().is_empty()
        || visual.source().len() > MAX_RUNTIME_SHADER_EFFECT_SOURCE_BYTES
    {
        return false;
    }
    let mut uniforms = BTreeSet::new();
    for uniform in visual.uniforms() {
        if !is_valid_shader_binding_id(uniform.name())
            || !uniforms.insert(uniform.name().to_string())
        {
            return false;
        }
        match uniform.value() {
            hawk2ui_render::ShaderEffectUniformValue::Float(values) => {
                if values.is_empty() || values.iter().any(|value| !value.is_finite()) {
                    return false;
                }
            }
            hawk2ui_render::ShaderEffectUniformValue::Int(values) => {
                if values.is_empty() {
                    return false;
                }
            }
        }
    }
    let mut children = BTreeSet::new();
    for child in visual.children() {
        if !is_valid_shader_binding_id(child.name())
            || !is_valid_asset_id(child.asset_id())
            || !children.insert(child.name().to_string())
        {
            return false;
        }
    }
    true
}

fn is_valid_shader_binding_id(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn is_valid_runtime_id(value: &str) -> bool {
    !value.trim().is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
}

fn is_valid_asset_id(value: &str) -> bool {
    !value.trim().is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
}

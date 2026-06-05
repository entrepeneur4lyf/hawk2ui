//! Retained scene graph records.

use std::collections::BTreeMap;

/// Stable scene node identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SceneNodeId(String);

impl SceneNodeId {
    /// Creates a scene node identifier.
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

/// Render geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geometry {
    /// X coordinate.
    pub x: f32,
    /// Y coordinate.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

impl Geometry {
    /// Creates geometry.
    #[must_use]
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Returns the union of two geometry bounds.
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        let min_x = self.x.min(other.x);
        let min_y = self.y.min(other.y);
        let max_x = (self.x + self.width).max(other.x + other.width);
        let max_y = (self.y + self.height).max(other.y + other.height);
        Self::new(min_x, min_y, max_x - min_x, max_y - min_y)
    }
}

/// Hit-test geometry.
pub type HitTestGeometry = Geometry;

/// Affine transform record.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    /// Horizontal scale.
    pub scale_x: f32,
    /// Horizontal skew.
    pub skew_x: f32,
    /// Vertical skew.
    pub skew_y: f32,
    /// Vertical scale.
    pub scale_y: f32,
    /// X translation.
    pub translate_x: f32,
    /// Y translation.
    pub translate_y: f32,
}

impl Transform {
    /// Creates identity transform.
    #[must_use]
    pub const fn identity() -> Self {
        Self {
            scale_x: 1.0,
            skew_x: 0.0,
            skew_y: 0.0,
            scale_y: 1.0,
            translate_x: 0.0,
            translate_y: 0.0,
        }
    }

    /// Creates translation transform.
    #[must_use]
    pub const fn translate(translate_x: f32, translate_y: f32) -> Self {
        Self {
            scale_x: 1.0,
            skew_x: 0.0,
            skew_y: 0.0,
            scale_y: 1.0,
            translate_x,
            translate_y,
        }
    }

    /// Creates a full 2D affine transform.
    #[must_use]
    pub const fn affine(
        scale_x: f32,
        skew_x: f32,
        skew_y: f32,
        scale_y: f32,
        translate_x: f32,
        translate_y: f32,
    ) -> Self {
        Self {
            scale_x,
            skew_x,
            skew_y,
            scale_y,
            translate_x,
            translate_y,
        }
    }

    /// Applies this affine transform to a point.
    #[must_use]
    pub fn apply_to_point(self, x: f32, y: f32) -> (f32, f32) {
        (
            self.scale_x
                .mul_add(x, self.skew_x.mul_add(y, self.translate_x)),
            self.skew_y
                .mul_add(x, self.scale_y.mul_add(y, self.translate_y)),
        )
    }

    /// Returns transform components in stable matrix order.
    #[must_use]
    pub const fn components(self) -> [f32; 6] {
        [
            self.scale_x,
            self.skew_x,
            self.skew_y,
            self.scale_y,
            self.translate_x,
            self.translate_y,
        ]
    }

    /// Returns whether all transform components are finite.
    #[must_use]
    pub fn is_finite(self) -> bool {
        self.components().iter().all(|value| value.is_finite())
    }
}

/// Accessibility geometry reference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessibilityRef(String);

impl AccessibilityRef {
    /// Creates an accessibility reference.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the reference as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Stable scene layer identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SceneLayerId(String);

impl SceneLayerId {
    /// Creates a scene layer identifier.
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

/// Stable scene effect identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SceneEffectId(String);

impl SceneEffectId {
    /// Creates a scene effect identifier.
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

/// Stable opacity group identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OpacityGroupId(String);

impl OpacityGroupId {
    /// Creates an opacity group identifier.
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

/// Offscreen compositing group opacity applied to a scene subtree.
#[derive(Clone, Debug, PartialEq)]
pub struct OpacityGroup {
    id: OpacityGroupId,
    opacity: f32,
}

impl OpacityGroup {
    /// Creates an opacity group record.
    #[must_use]
    pub const fn new(id: OpacityGroupId, opacity: f32) -> Self {
        Self { id, opacity }
    }

    /// Returns the opacity group ID.
    #[must_use]
    pub const fn id(&self) -> &OpacityGroupId {
        &self.id
    }

    /// Returns the group opacity.
    #[must_use]
    pub const fn opacity(&self) -> f32 {
        self.opacity
    }
}

/// Scene invalidation reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidationReason {
    /// Geometry changed.
    Geometry,
    /// Paint changed.
    Paint,
    /// Accessibility mapping changed.
    Accessibility,
}

/// Scene node.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneNode {
    id: SceneNodeId,
    z_order: i32,
    layout: Option<Geometry>,
    clip: Option<Geometry>,
    transform: Transform,
    opacity: f32,
    hit_test: Option<HitTestGeometry>,
    accessibility_ref: Option<AccessibilityRef>,
    layer_id: Option<SceneLayerId>,
    opacity_group: Option<OpacityGroup>,
    effect_refs: Vec<SceneEffectId>,
    invalidated: bool,
    invalidation_reasons: Vec<InvalidationReason>,
    dirty_bounds: Option<Geometry>,
    cache_invalidated: bool,
}

impl SceneNode {
    /// Creates a scene node.
    #[must_use]
    pub fn new(id: SceneNodeId) -> Self {
        Self {
            id,
            z_order: 0,
            layout: None,
            clip: None,
            transform: Transform::identity(),
            opacity: 1.0,
            hit_test: None,
            accessibility_ref: None,
            layer_id: None,
            opacity_group: None,
            effect_refs: Vec::new(),
            invalidated: false,
            invalidation_reasons: Vec::new(),
            dirty_bounds: None,
            cache_invalidated: false,
        }
    }

    /// Returns the scene node ID.
    #[must_use]
    pub const fn id(&self) -> &SceneNodeId {
        &self.id
    }

    /// Sets z-order.
    #[must_use]
    pub const fn with_z_order(mut self, z_order: i32) -> Self {
        self.z_order = z_order;
        self
    }

    /// Sets layout geometry.
    #[must_use]
    pub const fn with_layout(mut self, layout: Geometry) -> Self {
        self.layout = Some(layout);
        self
    }

    /// Sets clipping geometry.
    #[must_use]
    pub const fn with_clip(mut self, clip: Geometry) -> Self {
        self.clip = Some(clip);
        self
    }

    /// Sets transform.
    #[must_use]
    pub const fn with_transform(mut self, transform: Transform) -> Self {
        self.transform = transform;
        self
    }

    /// Sets opacity.
    #[must_use]
    pub const fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }

    /// Sets hit-test geometry.
    #[must_use]
    pub const fn with_hit_test(mut self, hit_test: HitTestGeometry) -> Self {
        self.hit_test = Some(hit_test);
        self
    }

    /// Sets accessibility reference.
    #[must_use]
    pub fn with_accessibility_ref(mut self, accessibility_ref: AccessibilityRef) -> Self {
        self.accessibility_ref = Some(accessibility_ref);
        self
    }

    /// Sets the scene layer this node contributes to.
    #[must_use]
    pub fn with_layer_id(mut self, layer_id: SceneLayerId) -> Self {
        self.layer_id = Some(layer_id);
        self
    }

    /// Sets the offscreen opacity group rooted at this node.
    #[must_use]
    pub fn with_opacity_group(mut self, opacity_group: OpacityGroup) -> Self {
        self.opacity_group = Some(opacity_group);
        self
    }

    /// Adds an effect reference applied to this node.
    #[must_use]
    pub fn with_effect_ref(mut self, effect_ref: SceneEffectId) -> Self {
        if !self.effect_refs.contains(&effect_ref) {
            self.effect_refs.push(effect_ref);
        }
        self
    }

    /// Returns z-order.
    #[must_use]
    pub const fn z_order(&self) -> i32 {
        self.z_order
    }

    /// Returns layout geometry.
    #[must_use]
    pub const fn layout(&self) -> Option<Geometry> {
        self.layout
    }

    /// Returns clip geometry.
    #[must_use]
    pub const fn clip(&self) -> Option<Geometry> {
        self.clip
    }

    /// Returns transform.
    #[must_use]
    pub const fn transform(&self) -> Transform {
        self.transform
    }

    /// Returns opacity.
    #[must_use]
    pub const fn opacity(&self) -> f32 {
        self.opacity
    }

    /// Returns hit-test geometry.
    #[must_use]
    pub const fn hit_test(&self) -> Option<HitTestGeometry> {
        self.hit_test
    }

    /// Returns accessibility reference.
    #[must_use]
    pub fn accessibility_ref(&self) -> Option<&AccessibilityRef> {
        self.accessibility_ref.as_ref()
    }

    /// Returns the scene layer this node contributes to.
    #[must_use]
    pub fn layer_id(&self) -> Option<&SceneLayerId> {
        self.layer_id.as_ref()
    }

    /// Returns the opacity group rooted at this node.
    #[must_use]
    pub fn opacity_group(&self) -> Option<&OpacityGroup> {
        self.opacity_group.as_ref()
    }

    /// Returns effect references applied to this node.
    #[must_use]
    pub fn effect_refs(&self) -> &[SceneEffectId] {
        &self.effect_refs
    }

    /// Returns whether this node is invalidated.
    #[must_use]
    pub const fn invalidated(&self) -> bool {
        self.invalidated
    }

    /// Returns invalidation reasons recorded for this node.
    #[must_use]
    pub fn invalidation_reasons(&self) -> &[InvalidationReason] {
        &self.invalidation_reasons
    }

    /// Returns dirty bounds accumulated for this node.
    #[must_use]
    pub const fn dirty_bounds(&self) -> Option<Geometry> {
        self.dirty_bounds
    }

    /// Returns whether cached layer content touching this node must be invalidated.
    #[must_use]
    pub const fn cache_invalidated(&self) -> bool {
        self.cache_invalidated
    }

    fn mark_invalidated(&mut self, reason: InvalidationReason, dirty_bounds: Option<Geometry>) {
        self.invalidated = true;
        if !self.invalidation_reasons.contains(&reason) {
            self.invalidation_reasons.push(reason);
        }
        if let Some(dirty_bounds) = dirty_bounds {
            self.dirty_bounds = Some(match self.dirty_bounds {
                Some(existing) => existing.union(dirty_bounds),
                None => dirty_bounds,
            });
        }
        if reason.invalidates_cache() {
            self.cache_invalidated = true;
        }
    }

    fn render_bounds(&self) -> Option<Geometry> {
        self.dirty_bounds.or_else(|| {
            self.layout
                .map(|layout| transform_geometry(layout, self.transform))
        })
    }

    fn has_same_render_record(&self, other: &Self) -> bool {
        let self_render_record = (
            &self.id,
            self.z_order,
            self.layout,
            self.clip,
            self.transform,
            self.opacity.to_bits(),
            self.hit_test,
            &self.accessibility_ref,
            &self.layer_id,
            &self.opacity_group,
            &self.effect_refs,
        );
        let other_render_record = (
            &other.id,
            other.z_order,
            other.layout,
            other.clip,
            other.transform,
            other.opacity.to_bits(),
            other.hit_test,
            &other.accessibility_ref,
            &other.layer_id,
            &other.opacity_group,
            &other.effect_refs,
        );
        self_render_record == other_render_record
    }
}

impl InvalidationReason {
    fn invalidates_cache(self) -> bool {
        matches!(self, Self::Geometry | Self::Paint)
    }
}

/// Scene graph error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SceneGraphError {
    /// Parent node is missing.
    MissingParent(String),
    /// Node is missing.
    MissingNode(String),
    /// Node already exists.
    DuplicateNode(String),
    /// Node ID is empty or contains unsupported characters.
    InvalidNodeId(String),
    /// Node geometry contains non-finite coordinates or negative dimensions.
    InvalidGeometry(String),
    /// Node transform contains non-finite coordinates.
    InvalidTransform(String),
    /// Node opacity is outside the renderable range.
    InvalidOpacity(String),
    /// Accessibility reference is empty.
    InvalidAccessibilityRef(String),
    /// Scene layer identifier is empty or unstable.
    InvalidLayerId(String),
    /// Scene effect reference is empty or unstable.
    InvalidEffectRef(String),
    /// Opacity group identifier is empty or unstable.
    InvalidOpacityGroupId(String),
}

/// Deterministic retained-scene diff used for repaint and cache decisions.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneGraphDiff {
    added_node_ids: Vec<SceneNodeId>,
    removed_node_ids: Vec<SceneNodeId>,
    changed_node_ids: Vec<SceneNodeId>,
    repaint_bounds: Option<Geometry>,
    cache_invalidated_node_ids: Vec<SceneNodeId>,
}

impl SceneGraphDiff {
    /// Creates an empty scene graph diff.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            added_node_ids: Vec::new(),
            removed_node_ids: Vec::new(),
            changed_node_ids: Vec::new(),
            repaint_bounds: None,
            cache_invalidated_node_ids: Vec::new(),
        }
    }

    /// Returns node IDs present only in the next graph.
    #[must_use]
    pub fn added_node_ids(&self) -> &[SceneNodeId] {
        &self.added_node_ids
    }

    /// Returns node IDs present only in the previous graph.
    #[must_use]
    pub fn removed_node_ids(&self) -> &[SceneNodeId] {
        &self.removed_node_ids
    }

    /// Returns node IDs whose scene records changed between graphs.
    #[must_use]
    pub fn changed_node_ids(&self) -> &[SceneNodeId] {
        &self.changed_node_ids
    }

    /// Returns aggregate repaint bounds covering added, removed, changed, and invalidated nodes.
    #[must_use]
    pub const fn repaint_bounds(&self) -> Option<Geometry> {
        self.repaint_bounds
    }

    /// Returns node IDs whose cached layer content must be invalidated.
    #[must_use]
    pub fn cache_invalidated_node_ids(&self) -> &[SceneNodeId] {
        &self.cache_invalidated_node_ids
    }

    fn add_repaint_bounds(&mut self, bounds: Option<Geometry>) {
        if let Some(bounds) = bounds {
            self.repaint_bounds = Some(match self.repaint_bounds {
                Some(existing) => existing.union(bounds),
                None => bounds,
            });
        }
    }
}

impl Default for SceneGraphDiff {
    fn default() -> Self {
        Self::new()
    }
}

/// Retained scene graph.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneGraph {
    entries: Vec<SceneEntry>,
    index_by_id: BTreeMap<SceneNodeId, usize>,
}

impl SceneGraph {
    /// Creates a scene graph with a root node.
    #[must_use]
    pub fn new(root: SceneNode) -> Self {
        let root_id = root.id().clone();
        Self {
            entries: vec![SceneEntry {
                node: root,
                parent: None,
                children: Vec::new(),
            }],
            index_by_id: BTreeMap::from([(root_id, 0)]),
        }
    }

    /// Adds a child node.
    ///
    /// # Errors
    ///
    /// Returns [`SceneGraphError`] when the parent is missing or the child ID already exists.
    pub fn with_child(
        mut self,
        parent_id: SceneNodeId,
        child: SceneNode,
    ) -> Result<Self, SceneGraphError> {
        validate_node_id(&parent_id)?;
        validate_scene_node(&child)?;
        if self.index_of(child.id()).is_some() {
            return Err(SceneGraphError::DuplicateNode(
                child.id().as_str().to_string(),
            ));
        }
        let Some(parent_index) = self.index_of(&parent_id) else {
            return Err(SceneGraphError::MissingParent(
                parent_id.as_str().to_string(),
            ));
        };
        let child_id = child.id().clone();
        self.entries[parent_index].children.push(child_id);
        let child_index = self.entries.len();
        self.index_by_id.insert(child.id().clone(), child_index);
        self.entries.push(SceneEntry {
            node: child,
            parent: Some(parent_id),
            children: Vec::new(),
        });
        Ok(self)
    }

    /// Validates the graph structure and renderable node records.
    ///
    /// # Errors
    ///
    /// Returns [`SceneGraphError`] when a node record, parent link, or child link is invalid.
    pub fn validate(&self) -> Result<(), SceneGraphError> {
        for entry in &self.entries {
            validate_scene_node(&entry.node)?;
            if let Some(parent_id) = entry.parent.as_ref() {
                validate_node_id(parent_id)?;
                if self.index_of(parent_id).is_none() {
                    return Err(SceneGraphError::MissingParent(
                        parent_id.as_str().to_string(),
                    ));
                }
            }
            for child_id in &entry.children {
                validate_node_id(child_id)?;
                if self.index_of(child_id).is_none() {
                    return Err(SceneGraphError::MissingNode(child_id.as_str().to_string()));
                }
            }
        }
        Ok(())
    }

    /// Invalidates a node and all ancestors.
    ///
    /// # Errors
    ///
    /// Returns [`SceneGraphError`] when the node is missing.
    pub fn invalidate(
        mut self,
        node_id: &SceneNodeId,
        reason: InvalidationReason,
    ) -> Result<Self, SceneGraphError> {
        let dirty_bounds = self.node(node_id).and_then(|node| {
            node.layout()
                .map(|layout| transform_geometry(layout, node.transform()))
        });
        let mut current = Some(node_id.clone());
        while let Some(id) = current.as_ref() {
            let Some(index) = self.index_of(id) else {
                return Err(SceneGraphError::MissingNode(id.as_str().to_string()));
            };
            self.entries[index]
                .node
                .mark_invalidated(reason, dirty_bounds);
            let parent = self.entries[index].parent.clone();
            current = parent;
        }
        Ok(self)
    }

    /// Returns a node by ID.
    #[must_use]
    pub fn node(&self, node_id: &SceneNodeId) -> Option<&SceneNode> {
        self.index_of(node_id)
            .and_then(|index| self.entries.get(index))
            .map(|entry| &entry.node)
    }

    /// Returns a node parent by ID.
    #[must_use]
    pub fn parent_of(&self, node_id: &SceneNodeId) -> Option<&SceneNodeId> {
        self.index_of(node_id)
            .and_then(|index| self.entries.get(index))
            .and_then(|entry| entry.parent.as_ref())
    }

    /// Returns children sorted by z-order.
    #[must_use]
    pub fn children_sorted_by_z(&self, node_id: &SceneNodeId) -> Vec<&SceneNode> {
        let mut children: Vec<_> = self.entry(node_id).map_or_else(Vec::new, |entry| {
            entry
                .children
                .iter()
                .filter_map(|child_id| self.node(child_id))
                .collect()
        });
        children.sort_by_key(|node| node.z_order());
        children
    }

    /// Returns all nodes in deterministic paint order.
    #[must_use]
    pub fn nodes_in_paint_order(&self) -> Vec<&SceneNode> {
        let Some(root) = self.entries.iter().find(|entry| entry.parent.is_none()) else {
            return Vec::new();
        };
        let mut nodes = Vec::new();
        self.push_paint_order(root.node.id(), &mut nodes);
        nodes
    }

    /// Resolves effective opacity for a node.
    ///
    /// Node opacity applies to the node render record. Opacity groups apply to the subtree rooted at
    /// the group node, so ancestor groups multiply into descendant opacity.
    ///
    /// # Errors
    ///
    /// Returns [`SceneGraphError::MissingNode`] when the node does not exist.
    pub fn effective_opacity(&self, node_id: &SceneNodeId) -> Result<f32, SceneGraphError> {
        let mut path = Vec::new();
        let mut current = Some(node_id.clone());
        while let Some(id) = current.as_ref() {
            let Some(entry) = self.entry(id) else {
                return Err(SceneGraphError::MissingNode(id.as_str().to_string()));
            };
            path.push(entry.node.id().clone());
            current.clone_from(&entry.parent);
        }

        let mut opacity = self.node(node_id).map_or(1.0, SceneNode::opacity);
        for id in path.iter().rev() {
            if let Some(group) = self.node(id).and_then(SceneNode::opacity_group) {
                opacity *= group.opacity();
            }
        }
        Ok(opacity)
    }

    /// Resolves render geometry for an accessibility reference.
    ///
    /// Hit-test geometry is preferred because it represents the interactive target. Layout geometry
    /// is used as the fallback for non-interactive semantic nodes.
    #[must_use]
    pub fn accessibility_geometry(&self, accessibility_ref: &AccessibilityRef) -> Option<Geometry> {
        self.entries
            .iter()
            .find(|entry| entry.node.accessibility_ref() == Some(accessibility_ref))
            .and_then(|entry| {
                entry
                    .node
                    .hit_test()
                    .or_else(|| entry.node.layout())
                    .map(|geometry| transform_geometry(geometry, entry.node.transform()))
            })
    }

    /// Computes deterministic scene changes needed for repaint and cache invalidation.
    ///
    /// # Errors
    ///
    /// Returns [`SceneGraphError`] when either graph contains invalid scene records.
    pub fn diff(&self, next: &Self) -> Result<SceneGraphDiff, SceneGraphError> {
        self.validate()?;
        next.validate()?;
        let mut diff = SceneGraphDiff::new();

        for entry in &next.entries {
            if let Some(previous) = self.node(entry.node.id()) {
                if !previous.has_same_render_record(&entry.node) {
                    diff.changed_node_ids.push(entry.node.id().clone());
                    diff.add_repaint_bounds(previous.render_bounds());
                    diff.add_repaint_bounds(entry.node.render_bounds());
                }
            } else {
                diff.added_node_ids.push(entry.node.id().clone());
                diff.add_repaint_bounds(entry.node.render_bounds());
            }
            if entry.node.cache_invalidated() {
                diff.cache_invalidated_node_ids
                    .push(entry.node.id().clone());
                diff.add_repaint_bounds(entry.node.dirty_bounds());
            }
        }

        for entry in &self.entries {
            if next.node(entry.node.id()).is_none() {
                diff.removed_node_ids.push(entry.node.id().clone());
                diff.add_repaint_bounds(entry.node.render_bounds());
            }
        }

        diff.added_node_ids.sort();
        diff.removed_node_ids.sort();
        diff.changed_node_ids.sort();
        diff.cache_invalidated_node_ids.sort();
        Ok(diff)
    }

    fn index_of(&self, node_id: &SceneNodeId) -> Option<usize> {
        self.index_by_id.get(node_id).copied()
    }

    fn entry(&self, node_id: &SceneNodeId) -> Option<&SceneEntry> {
        self.index_of(node_id)
            .and_then(|index| self.entries.get(index))
    }

    fn push_paint_order<'a>(&'a self, node_id: &SceneNodeId, nodes: &mut Vec<&'a SceneNode>) {
        let Some(entry) = self.entry(node_id) else {
            return;
        };
        nodes.push(&entry.node);
        let mut child_ids = entry.children.clone();
        child_ids.sort_by(|left, right| {
            let left_node = self.node(left);
            let right_node = self.node(right);
            match (left_node, right_node) {
                (Some(left_node), Some(right_node)) => left_node
                    .z_order()
                    .cmp(&right_node.z_order())
                    .then_with(|| left_node.id().cmp(right_node.id())),
                _ => left.cmp(right),
            }
        });
        for child_id in child_ids {
            self.push_paint_order(&child_id, nodes);
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct SceneEntry {
    node: SceneNode,
    parent: Option<SceneNodeId>,
    children: Vec<SceneNodeId>,
}

fn validate_scene_node(node: &SceneNode) -> Result<(), SceneGraphError> {
    validate_node_id(node.id())?;
    if let Some(layout) = node.layout() {
        validate_geometry(node.id(), layout)?;
    }
    if let Some(clip) = node.clip() {
        validate_geometry(node.id(), clip)?;
    }
    validate_transform(node.id(), node.transform())?;
    if !node.opacity().is_finite() || !(0.0..=1.0).contains(&node.opacity()) {
        return Err(SceneGraphError::InvalidOpacity(
            node.id().as_str().to_string(),
        ));
    }
    if let Some(hit_test) = node.hit_test() {
        validate_geometry(node.id(), hit_test)?;
    }
    if let Some(accessibility_ref) = node.accessibility_ref()
        && accessibility_ref.as_str().trim().is_empty()
    {
        return Err(SceneGraphError::InvalidAccessibilityRef(
            node.id().as_str().to_string(),
        ));
    }
    if let Some(layer_id) = node.layer_id() {
        validate_layer_id(layer_id)?;
    }
    if let Some(opacity_group) = node.opacity_group() {
        validate_opacity_group(opacity_group)?;
    }
    for effect_ref in node.effect_refs() {
        validate_effect_ref(effect_ref)?;
    }
    Ok(())
}

fn validate_node_id(node_id: &SceneNodeId) -> Result<(), SceneGraphError> {
    if is_stable_identifier(node_id.as_str()) {
        Ok(())
    } else {
        Err(SceneGraphError::InvalidNodeId(node_id.as_str().to_string()))
    }
}

fn validate_layer_id(layer_id: &SceneLayerId) -> Result<(), SceneGraphError> {
    if is_stable_identifier(layer_id.as_str()) {
        Ok(())
    } else {
        Err(SceneGraphError::InvalidLayerId(
            layer_id.as_str().to_string(),
        ))
    }
}

fn validate_effect_ref(effect_ref: &SceneEffectId) -> Result<(), SceneGraphError> {
    if is_stable_identifier(effect_ref.as_str()) {
        Ok(())
    } else {
        Err(SceneGraphError::InvalidEffectRef(
            effect_ref.as_str().to_string(),
        ))
    }
}

fn validate_opacity_group(opacity_group: &OpacityGroup) -> Result<(), SceneGraphError> {
    if !is_stable_identifier(opacity_group.id().as_str()) {
        return Err(SceneGraphError::InvalidOpacityGroupId(
            opacity_group.id().as_str().to_string(),
        ));
    }
    if opacity_group.opacity().is_finite() && (0.0..=1.0).contains(&opacity_group.opacity()) {
        Ok(())
    } else {
        Err(SceneGraphError::InvalidOpacity(
            opacity_group.id().as_str().to_string(),
        ))
    }
}

fn is_stable_identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
}

fn validate_geometry(node_id: &SceneNodeId, geometry: Geometry) -> Result<(), SceneGraphError> {
    if geometry.x.is_finite()
        && geometry.y.is_finite()
        && geometry.width.is_finite()
        && geometry.height.is_finite()
        && geometry.width >= 0.0
        && geometry.height >= 0.0
    {
        Ok(())
    } else {
        Err(SceneGraphError::InvalidGeometry(
            node_id.as_str().to_string(),
        ))
    }
}

fn validate_transform(node_id: &SceneNodeId, transform: Transform) -> Result<(), SceneGraphError> {
    if transform.is_finite() {
        Ok(())
    } else {
        Err(SceneGraphError::InvalidTransform(
            node_id.as_str().to_string(),
        ))
    }
}

fn transform_geometry(geometry: Geometry, transform: Transform) -> Geometry {
    let (x0, y0) = transform.apply_to_point(geometry.x, geometry.y);
    let (x1, y1) = transform.apply_to_point(geometry.x + geometry.width, geometry.y);
    let (x2, y2) = transform.apply_to_point(geometry.x, geometry.y + geometry.height);
    let (x3, y3) =
        transform.apply_to_point(geometry.x + geometry.width, geometry.y + geometry.height);

    let min_x = x0.min(x1).min(x2).min(x3);
    let min_y = y0.min(y1).min(y2).min(y3);
    let max_x = x0.max(x1).max(x2).max(x3);
    let max_y = y0.max(y1).max(y2).max(y3);
    Geometry::new(min_x, min_y, max_x - min_x, max_y - min_y)
}

//! Retained scene graph records.

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
}

/// Hit-test geometry.
pub type HitTestGeometry = Geometry;

/// Affine transform record.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
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
            translate_x: 0.0,
            translate_y: 0.0,
        }
    }

    /// Creates translation transform.
    #[must_use]
    pub const fn translate(translate_x: f32, translate_y: f32) -> Self {
        Self {
            translate_x,
            translate_y,
        }
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
    invalidated: bool,
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
            invalidated: false,
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

    /// Returns whether this node is invalidated.
    #[must_use]
    pub const fn invalidated(&self) -> bool {
        self.invalidated
    }

    fn mark_invalidated(&mut self) {
        self.invalidated = true;
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
}

/// Retained scene graph.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneGraph {
    entries: Vec<SceneEntry>,
}

impl SceneGraph {
    /// Creates a scene graph with a root node.
    #[must_use]
    pub fn new(root: SceneNode) -> Self {
        Self {
            entries: vec![SceneEntry {
                node: root,
                parent: None,
                children: Vec::new(),
            }],
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
        self.entries.push(SceneEntry {
            node: child,
            parent: Some(parent_id),
            children: Vec::new(),
        });
        Ok(self)
    }

    /// Invalidates a node and all ancestors.
    ///
    /// # Errors
    ///
    /// Returns [`SceneGraphError`] when the node is missing.
    pub fn invalidate(
        mut self,
        node_id: &SceneNodeId,
        _reason: InvalidationReason,
    ) -> Result<Self, SceneGraphError> {
        let mut current = Some(node_id.clone());
        while let Some(id) = current.as_ref() {
            let Some(index) = self.index_of(id) else {
                return Err(SceneGraphError::MissingNode(id.as_str().to_string()));
            };
            self.entries[index].node.mark_invalidated();
            let parent = self.entries[index].parent.clone();
            current = parent;
        }
        Ok(self)
    }

    /// Returns a node by ID.
    #[must_use]
    pub fn node(&self, node_id: &SceneNodeId) -> Option<&SceneNode> {
        self.entries
            .iter()
            .find(|entry| entry.node.id().as_str() == node_id.as_str())
            .map(|entry| &entry.node)
    }

    /// Returns a node parent by ID.
    #[must_use]
    pub fn parent_of(&self, node_id: &SceneNodeId) -> Option<&SceneNodeId> {
        self.entries
            .iter()
            .find(|entry| entry.node.id().as_str() == node_id.as_str())
            .and_then(|entry| entry.parent.as_ref())
    }

    /// Returns children sorted by z-order.
    #[must_use]
    pub fn children_sorted_by_z(&self, node_id: &SceneNodeId) -> Vec<&SceneNode> {
        let mut children: Vec<_> = self
            .entries
            .iter()
            .find(|entry| entry.node.id().as_str() == node_id.as_str())
            .map_or_else(Vec::new, |entry| {
                entry
                    .children
                    .iter()
                    .filter_map(|child_id| self.node(child_id))
                    .collect()
            });
        children.sort_by_key(|node| node.z_order());
        children
    }

    fn index_of(&self, node_id: &SceneNodeId) -> Option<usize> {
        self.entries
            .iter()
            .position(|entry| entry.node.id().as_str() == node_id.as_str())
    }
}

#[derive(Clone, Debug, PartialEq)]
struct SceneEntry {
    node: SceneNode,
    parent: Option<SceneNodeId>,
    children: Vec<SceneNodeId>,
}

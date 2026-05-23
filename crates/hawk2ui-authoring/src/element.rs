//! Typed element records emitted by authoring and framework adapters.

use std::collections::BTreeSet;

/// Stable element identifier used across authoring, diffing, and runtime records.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ElementId(String);

impl ElementId {
    /// Creates an element identifier.
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

/// Native element kind understood by the authoring layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ElementKind {
    /// Generic layout or grouping view.
    View,
    /// Text node.
    Text,
    /// Button control.
    Button,
}

/// Authoring property value.
#[derive(Clone, Debug, PartialEq)]
pub enum PropValue {
    /// String property.
    String(String),
    /// Boolean property.
    Bool(bool),
    /// Floating point number property.
    Number(f64),
}

/// Typed element node.
#[derive(Clone, Debug, PartialEq)]
pub struct ElementNode {
    id: ElementId,
    kind: ElementKind,
    props: Vec<(String, PropValue)>,
}

impl ElementNode {
    /// Creates an element node.
    #[must_use]
    pub const fn new(id: ElementId, kind: ElementKind) -> Self {
        Self {
            id,
            kind,
            props: Vec::new(),
        }
    }

    /// Adds or replaces a property value.
    #[must_use]
    pub fn with_prop(mut self, name: impl Into<String>, value: PropValue) -> Self {
        let name = name.into();
        if let Some((_, existing)) = self
            .props
            .iter_mut()
            .find(|(prop_name, _)| prop_name == &name)
        {
            *existing = value;
        } else {
            self.props.push((name, value));
        }
        self
    }

    /// Returns the stable element identifier.
    #[must_use]
    pub const fn id(&self) -> &ElementId {
        &self.id
    }

    /// Returns the element kind.
    #[must_use]
    pub const fn kind(&self) -> ElementKind {
        self.kind
    }

    /// Returns a property by name.
    #[must_use]
    pub fn prop(&self, name: &str) -> Option<&PropValue> {
        self.props
            .iter()
            .find_map(|(prop_name, value)| (prop_name == name).then_some(value))
    }
}

/// Child node with a stable author-provided key.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyedChild {
    key: String,
    node: ElementNode,
}

impl KeyedChild {
    /// Creates a keyed child.
    #[must_use]
    pub fn new(key: impl Into<String>, node: ElementNode) -> Self {
        Self {
            key: key.into(),
            node,
        }
    }

    /// Returns the child key.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Returns the child node.
    #[must_use]
    pub const fn node(&self) -> &ElementNode {
        &self.node
    }
}

/// Child list construction error for duplicate keyed children.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DuplicateChildKeyError {
    duplicate_key: String,
}

impl DuplicateChildKeyError {
    /// Creates a duplicate child key error.
    #[must_use]
    pub fn new(duplicate_key: impl Into<String>) -> Self {
        Self {
            duplicate_key: duplicate_key.into(),
        }
    }

    /// Returns the duplicate key.
    #[must_use]
    pub fn duplicate_key(&self) -> &str {
        &self.duplicate_key
    }
}

/// Ordered child list.
#[derive(Clone, Debug, PartialEq)]
pub struct ChildList {
    nodes: Vec<ElementNode>,
}

impl ChildList {
    /// Creates an ordered child list.
    ///
    /// # Errors
    ///
    /// This constructor currently cannot fail, but returns a result so ordered and keyed
    /// construction share a stable call shape.
    pub fn ordered(
        children: impl IntoIterator<Item = ElementNode>,
    ) -> Result<Self, DuplicateChildKeyError> {
        Ok(Self {
            nodes: children.into_iter().collect(),
        })
    }

    /// Creates a keyed child list and rejects duplicate keys.
    ///
    /// # Errors
    ///
    /// Returns [`DuplicateChildKeyError`] when two children use the same key.
    pub fn keyed(
        children: impl IntoIterator<Item = KeyedChild>,
    ) -> Result<Self, DuplicateChildKeyError> {
        let mut keys = BTreeSet::new();
        let mut nodes = Vec::new();
        for child in children {
            if !keys.insert(child.key.clone()) {
                return Err(DuplicateChildKeyError::new(child.key));
            }
            nodes.push(child.node);
        }
        Ok(Self { nodes })
    }

    /// Iterates over child nodes in author-declared order.
    pub fn iter(&self) -> impl Iterator<Item = &ElementNode> {
        self.nodes.iter()
    }
}

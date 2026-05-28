//! Accessibility tree records.

use serde::{Deserialize, Serialize};

/// Maximum accessibility tree depth traversed by host-facing operations.
///
/// Accessibility trees may be hydrated from package data, so traversal is
/// deliberately bounded to avoid stack exhaustion from adversarial input.
pub const A11Y_MAX_TREE_DEPTH: usize = 256;

/// Accessibility role.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum A11yRole {
    /// Window root.
    Window,
    /// Generic panel/group.
    Panel,
    /// Button.
    Button,
    /// Slider.
    Slider,
    /// Text input.
    TextInput,
    /// Checkbox.
    Checkbox,
    /// List.
    List,
    /// List item.
    ListItem,
    /// Custom control.
    Custom,
}

/// Checked state.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CheckedState {
    /// Checked.
    Checked,
    /// Unchecked.
    Unchecked,
    /// Mixed state.
    Mixed,
}

/// Accessibility action.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum A11yAction {
    /// Focus action.
    Focus,
    /// Press action.
    Press,
    /// Increment value.
    Increment,
    /// Decrement value.
    Decrement,
    /// Set value.
    SetValue(String),
    /// Custom named action.
    Custom(String),
}

/// Accessibility bounds in logical coordinates.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct A11yBounds {
    /// Logical x coordinate.
    pub x: f64,
    /// Logical y coordinate.
    pub y: f64,
    /// Logical width.
    pub width: f64,
    /// Logical height.
    pub height: f64,
}

/// Numeric accessibility value metadata.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct A11yNumericValue {
    /// Current numeric value.
    pub value: f64,
    /// Optional minimum value.
    pub min: Option<f64>,
    /// Optional maximum value.
    pub max: Option<f64>,
    /// Optional increment/decrement step.
    pub step: Option<f64>,
}

impl A11yNumericValue {
    /// Creates numeric value metadata.
    #[must_use]
    pub const fn new(value: f64) -> Self {
        Self {
            value,
            min: None,
            max: None,
            step: None,
        }
    }

    /// Sets the minimum value.
    #[must_use]
    pub const fn min(mut self, min: f64) -> Self {
        self.min = Some(min);
        self
    }

    /// Sets the maximum value.
    #[must_use]
    pub const fn max(mut self, max: f64) -> Self {
        self.max = Some(max);
        self
    }

    /// Sets the increment/decrement step.
    #[must_use]
    pub const fn step(mut self, step: f64) -> Self {
        self.step = Some(step);
        self
    }
}

impl A11yBounds {
    /// Creates bounds.
    #[must_use]
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// Accessibility node.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct A11yNode {
    /// Stable node identifier.
    pub id: String,
    /// Accessibility role.
    pub role: A11yRole,
    /// Accessible name.
    pub name: Option<String>,
    /// Accessible description.
    pub description: Option<String>,
    /// Accessible value text.
    pub value: Option<String>,
    /// Numeric value metadata for range-like controls.
    pub numeric_value: Option<A11yNumericValue>,
    /// Checked state.
    pub checked: Option<CheckedState>,
    /// Disabled state.
    pub disabled: bool,
    /// Focus state.
    pub focused: bool,
    /// Layout bounds.
    pub bounds: Option<A11yBounds>,
    /// Supported actions.
    pub actions: Vec<A11yAction>,
    /// Total item count for collection children.
    pub size_of_set: Option<usize>,
    /// One-based position in a collection.
    pub position_in_set: Option<usize>,
    /// Child nodes.
    pub children: Vec<A11yNode>,
}

impl A11yNode {
    /// Creates an accessibility node.
    #[must_use]
    pub fn new(id: impl Into<String>, role: A11yRole) -> Self {
        Self {
            id: id.into(),
            role,
            name: None,
            description: None,
            value: None,
            numeric_value: None,
            checked: None,
            disabled: false,
            focused: false,
            bounds: None,
            actions: Vec::new(),
            size_of_set: None,
            position_in_set: None,
            children: Vec::new(),
        }
    }

    /// Sets accessible name.
    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Sets accessible description.
    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets accessible value.
    #[must_use]
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Sets numeric value metadata.
    #[must_use]
    pub const fn numeric_value(mut self, value: A11yNumericValue) -> Self {
        self.numeric_value = Some(value);
        self
    }

    /// Sets checked state.
    #[must_use]
    pub const fn checked(mut self, checked: CheckedState) -> Self {
        self.checked = Some(checked);
        self
    }

    /// Sets disabled state.
    #[must_use]
    pub const fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Sets focused state.
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Sets bounds.
    #[must_use]
    pub const fn bounds(mut self, bounds: A11yBounds) -> Self {
        self.bounds = Some(bounds);
        self
    }

    /// Adds an action.
    #[must_use]
    pub fn action(mut self, action: A11yAction) -> Self {
        self.actions.push(action);
        self
    }

    /// Sets collection size metadata.
    #[must_use]
    pub const fn size_of_set(mut self, size: usize) -> Self {
        self.size_of_set = Some(size);
        self
    }

    /// Sets one-based collection position metadata.
    #[must_use]
    pub const fn position_in_set(mut self, position: usize) -> Self {
        self.position_in_set = Some(position);
        self
    }

    /// Adds a child node.
    #[must_use]
    pub fn child(mut self, child: A11yNode) -> Self {
        self.children.push(child);
        self
    }

    fn find(&self, id: &str) -> Option<&A11yNode> {
        self.find_at_depth(id, 0)
    }

    fn find_at_depth(&self, id: &str, depth: usize) -> Option<&A11yNode> {
        if depth > A11Y_MAX_TREE_DEPTH {
            return None;
        }
        if self.id == id {
            return Some(self);
        }
        self.children
            .iter()
            .find_map(|child| child.find_at_depth(id, depth + 1))
    }

    pub(crate) fn find_mut(&mut self, id: &str) -> Option<&mut A11yNode> {
        self.find_mut_at_depth(id, 0)
    }

    fn find_mut_at_depth(&mut self, id: &str, depth: usize) -> Option<&mut A11yNode> {
        if depth > A11Y_MAX_TREE_DEPTH {
            return None;
        }
        if self.id == id {
            return Some(self);
        }
        self.children
            .iter_mut()
            .find_map(|child| child.find_mut_at_depth(id, depth + 1))
    }

    pub(crate) fn clear_focus(&mut self) {
        self.clear_focus_at_depth(0);
    }

    fn clear_focus_at_depth(&mut self, depth: usize) {
        if depth > A11Y_MAX_TREE_DEPTH {
            return;
        }
        self.focused = false;
        for child in &mut self.children {
            child.clear_focus_at_depth(depth + 1);
        }
    }
}

/// Accessibility tree.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct A11yTree {
    /// Root node.
    pub root: A11yNode,
}

impl A11yTree {
    /// Creates an accessibility tree.
    #[must_use]
    pub const fn new(root: A11yNode) -> Self {
        Self { root }
    }

    /// Finds a node by identifier.
    #[must_use]
    pub fn find(&self, id: &str) -> Option<&A11yNode> {
        self.root.find(id)
    }

    pub(crate) fn find_mut(&mut self, id: &str) -> Option<&mut A11yNode> {
        self.root.find_mut(id)
    }

    pub(crate) fn clear_focus(&mut self) {
        self.root.clear_focus();
    }
}

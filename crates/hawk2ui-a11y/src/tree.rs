//! Accessibility tree records.

use serde::{Deserialize, Serialize};

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
            checked: None,
            disabled: false,
            focused: false,
            bounds: None,
            actions: Vec::new(),
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

    /// Adds a child node.
    #[must_use]
    pub fn child(mut self, child: A11yNode) -> Self {
        self.children.push(child);
        self
    }

    fn find(&self, id: &str) -> Option<&A11yNode> {
        if self.id == id {
            return Some(self);
        }
        self.children.iter().find_map(|child| child.find(id))
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
}

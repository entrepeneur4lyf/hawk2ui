//! Headless component accessibility semantics.

use serde::{Deserialize, Serialize};

use crate::{A11yNode, A11yRole, CheckedState};

/// Headless component kind.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ComponentKind {
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
    /// Panel.
    Panel,
    /// Custom control.
    Custom,
}

/// Optional visual style metadata kept separate from accessibility semantics.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct VisualStyleSemantics {
    /// Visual variant label.
    pub variant: String,
    /// Visual color token or literal.
    pub color: String,
}

impl VisualStyleSemantics {
    /// Creates visual style metadata.
    #[must_use]
    pub fn new(variant: impl Into<String>, color: impl Into<String>) -> Self {
        Self {
            variant: variant.into(),
            color: color.into(),
        }
    }
}

/// Component semantics independent of visual style.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ComponentSemantics {
    /// Component kind.
    pub kind: ComponentKind,
    /// Accessibility node semantics.
    pub accessible: A11yNode,
    /// Optional visual style metadata.
    pub style: Option<VisualStyleSemantics>,
    /// Optional list item count.
    pub item_count: Option<usize>,
}

impl ComponentSemantics {
    /// Creates button semantics.
    #[must_use]
    pub fn button(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self::new(
            ComponentKind::Button,
            A11yNode::new(id, A11yRole::Button).name(name),
        )
    }

    /// Creates slider semantics.
    #[must_use]
    pub fn slider(
        id: impl Into<String>,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        Self::new(
            ComponentKind::Slider,
            A11yNode::new(id, A11yRole::Slider).name(name).value(value),
        )
    }

    /// Creates text input semantics.
    #[must_use]
    pub fn text_input(
        id: impl Into<String>,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        Self::new(
            ComponentKind::TextInput,
            A11yNode::new(id, A11yRole::TextInput)
                .name(name)
                .value(value),
        )
    }

    /// Creates checkbox semantics.
    #[must_use]
    pub fn checkbox(id: impl Into<String>, name: impl Into<String>, checked: bool) -> Self {
        let checked = if checked {
            CheckedState::Checked
        } else {
            CheckedState::Unchecked
        };
        Self::new(
            ComponentKind::Checkbox,
            A11yNode::new(id, A11yRole::Checkbox)
                .name(name)
                .checked(checked),
        )
    }

    /// Creates list semantics.
    #[must_use]
    pub fn list(id: impl Into<String>, name: impl Into<String>, item_count: usize) -> Self {
        let mut semantics = Self::new(
            ComponentKind::List,
            A11yNode::new(id, A11yRole::List).name(name),
        );
        semantics.item_count = Some(item_count);
        semantics
    }

    /// Creates panel semantics.
    #[must_use]
    pub fn panel(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self::new(
            ComponentKind::Panel,
            A11yNode::new(id, A11yRole::Panel).name(name),
        )
    }

    /// Creates custom control semantics.
    #[must_use]
    pub fn custom(id: impl Into<String>, name: impl Into<String>, role: A11yRole) -> Self {
        Self::new(ComponentKind::Custom, A11yNode::new(id, role).name(name))
    }

    /// Adds optional visual style metadata.
    #[must_use]
    pub fn with_style(mut self, style: VisualStyleSemantics) -> Self {
        self.style = Some(style);
        self
    }

    fn new(kind: ComponentKind, accessible: A11yNode) -> Self {
        Self {
            kind,
            accessible,
            style: None,
            item_count: None,
        }
    }
}

//! Component and custom surface authoring records.

use hawk2ui_render::CustomSurfaceCategory;
use hawk2ui_runtime::{RuntimeCustomSurfaceVisual, RuntimeVisual};

use crate::{ChildList, PropValue};

/// Stable component instance identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ComponentId(String);

impl ComponentId {
    /// Creates a component identifier.
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

/// Component instance emitted by authoring or framework adapters.
#[derive(Clone, Debug, PartialEq)]
pub struct ComponentInstance {
    id: ComponentId,
    component_name: String,
    props: Vec<(String, PropValue)>,
    references: Vec<(String, String)>,
    slots: Vec<(String, ChildList)>,
}

impl ComponentInstance {
    /// Creates a component instance record.
    #[must_use]
    pub fn new(id: ComponentId, component_name: impl Into<String>) -> Self {
        Self {
            id,
            component_name: component_name.into(),
            props: Vec::new(),
            references: Vec::new(),
            slots: Vec::new(),
        }
    }

    /// Adds or replaces a component property.
    #[must_use]
    pub fn with_prop(mut self, name: impl Into<String>, value: PropValue) -> Self {
        upsert(&mut self.props, name.into(), value);
        self
    }

    /// Adds or replaces a named reference.
    #[must_use]
    pub fn with_reference(mut self, name: impl Into<String>, target: impl Into<String>) -> Self {
        upsert(&mut self.references, name.into(), target.into());
        self
    }

    /// Adds or replaces a named child slot.
    #[must_use]
    pub fn with_slot(mut self, name: impl Into<String>, children: ChildList) -> Self {
        upsert(&mut self.slots, name.into(), children);
        self
    }

    /// Returns the stable component identifier.
    #[must_use]
    pub const fn id(&self) -> &ComponentId {
        &self.id
    }

    /// Returns the component type name.
    #[must_use]
    pub fn component_name(&self) -> &str {
        &self.component_name
    }

    /// Returns a component property by name.
    #[must_use]
    pub fn prop(&self, name: &str) -> Option<&PropValue> {
        find_value(&self.props, name)
    }

    /// Returns a named reference target by name.
    #[must_use]
    pub fn reference(&self, name: &str) -> Option<&str> {
        find_value(&self.references, name).map(String::as_str)
    }

    /// Returns a named child slot.
    #[must_use]
    pub fn slot(&self, name: &str) -> Option<&ChildList> {
        find_value(&self.slots, name)
    }
}

/// Stable custom surface identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SurfaceId(String);

impl SurfaceId {
    /// Creates a surface identifier.
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

/// Custom surface purpose.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfacePurpose {
    /// Author-provided custom draw surface.
    CustomDraw,
    /// Native host-provided surface.
    NativeHost,
}

/// Custom surface declaration emitted by authoring or framework adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomSurfaceDeclaration {
    id: SurfaceId,
    purpose: SurfacePurpose,
    category: Option<CustomSurfaceCategory>,
    references: Vec<(String, String)>,
}

impl CustomSurfaceDeclaration {
    /// Creates a custom surface declaration.
    #[must_use]
    pub const fn new(id: SurfaceId, purpose: SurfacePurpose) -> Self {
        Self {
            id,
            purpose,
            category: None,
            references: Vec::new(),
        }
    }

    /// Sets the renderer category used when lowering this declaration to a runtime visual.
    #[must_use]
    pub const fn with_category(mut self, category: CustomSurfaceCategory) -> Self {
        self.category = Some(category);
        self
    }

    /// Adds or replaces a named surface reference.
    #[must_use]
    pub fn with_reference(mut self, name: impl Into<String>, target: impl Into<String>) -> Self {
        upsert(&mut self.references, name.into(), target.into());
        self
    }

    /// Returns the stable surface identifier.
    #[must_use]
    pub const fn id(&self) -> &SurfaceId {
        &self.id
    }

    /// Returns the surface purpose.
    #[must_use]
    pub const fn purpose(&self) -> SurfacePurpose {
        self.purpose
    }

    /// Returns the optional renderer category for this custom surface.
    #[must_use]
    pub const fn category(&self) -> Option<CustomSurfaceCategory> {
        self.category
    }

    /// Returns a named reference target by name.
    #[must_use]
    pub fn reference(&self, name: &str) -> Option<&str> {
        find_value(&self.references, name).map(String::as_str)
    }

    /// Lowers the declaration to a runtime custom-surface visual.
    #[must_use]
    pub fn runtime_visual(&self) -> RuntimeVisual {
        RuntimeVisual::CustomSurface(RuntimeCustomSurfaceVisual::new(self.category.unwrap_or(
            match self.purpose {
                SurfacePurpose::CustomDraw => CustomSurfaceCategory::Scope,
                SurfacePurpose::NativeHost => CustomSurfaceCategory::InspectorPanel,
            },
        )))
    }
}

fn upsert<T>(entries: &mut Vec<(String, T)>, name: String, value: T) {
    if let Some((_, existing)) = entries
        .iter_mut()
        .find(|(entry_name, _)| entry_name == &name)
    {
        *existing = value;
    } else {
        entries.push((name, value));
    }
}

fn find_value<'a, T>(entries: &'a [(String, T)], name: &str) -> Option<&'a T> {
    entries
        .iter()
        .find_map(|(entry_name, value)| (entry_name == name).then_some(value))
}

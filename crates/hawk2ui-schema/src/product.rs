//! Product model records for supported `Hawk2UI` surfaces and targets.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Supported host target.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub enum HostTarget {
    /// Owned desktop window on native Wayland Linux.
    LinuxWayland,
    /// Owned desktop window on Windows.
    WindowsDesktop,
    /// Owned desktop window on macOS.
    MacosDesktop,
    /// Embedded editor surface owned by a plugin host.
    PluginHost,
}

/// Product surface kind.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub enum SurfaceKind {
    /// Owned desktop application window.
    DesktopWindow,
    /// Embedded plugin editor surface.
    PluginEditor,
}

/// Product capability advertised by a `Hawk2UI` product model.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub enum ProductCapability {
    /// Native desktop windowing.
    NativeWindowing,
    /// Plugin editor embedding.
    PluginEditorEmbedding,
    /// Sealed artifact loading.
    SealedArtifacts,
    /// Capability-scoped platform APIs.
    CapabilityScopedPlatformApis,
}

/// Product conformance model for supported targets, surfaces, and capabilities.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProductModel {
    /// Product identifier.
    pub id: String,
    /// Supported host targets.
    pub host_targets: Vec<HostTarget>,
    /// Supported surface kinds.
    pub surface_kinds: Vec<SurfaceKind>,
    /// Supported product capabilities.
    pub capabilities: Vec<ProductCapability>,
}

impl ProductModel {
    /// Creates an empty product model.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            host_targets: Vec::new(),
            surface_kinds: Vec::new(),
            capabilities: Vec::new(),
        }
    }

    /// Adds a host target when it is not already present.
    #[must_use]
    pub fn with_host_target(mut self, target: HostTarget) -> Self {
        if !self.host_targets.contains(&target) {
            self.host_targets.push(target);
        }
        self
    }

    /// Adds a surface kind when it is not already present.
    #[must_use]
    pub fn with_surface_kind(mut self, surface: SurfaceKind) -> Self {
        if !self.surface_kinds.contains(&surface) {
            self.surface_kinds.push(surface);
        }
        self
    }

    /// Adds a capability when it is not already present.
    #[must_use]
    pub fn with_capability(mut self, capability: ProductCapability) -> Self {
        if !self.capabilities.contains(&capability) {
            self.capabilities.push(capability);
        }
        self
    }

    /// Returns true when the product supports a surface kind.
    #[must_use]
    pub fn supports_surface(&self, surface: SurfaceKind) -> bool {
        self.surface_kinds.contains(&surface)
    }

    /// Returns true when the product advertises a capability.
    #[must_use]
    pub fn has_capability(&self, capability: ProductCapability) -> bool {
        self.capabilities.contains(&capability)
    }

    /// Validates that the model includes required production surface kinds.
    ///
    /// # Errors
    ///
    /// Returns [`ProductModelError`] when a required surface is missing.
    pub fn validate_required_surfaces(&self) -> Result<(), ProductModelError> {
        for surface in [SurfaceKind::DesktopWindow, SurfaceKind::PluginEditor] {
            if !self.supports_surface(surface) {
                return Err(ProductModelError::MissingSurface(surface));
            }
        }
        Ok(())
    }
}

/// Product model validation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductModelError {
    /// Required surface is missing.
    MissingSurface(SurfaceKind),
}

/// Schema validation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaValidationError {
    rule: String,
    message: String,
}

impl SchemaValidationError {
    /// Creates a schema validation error.
    #[must_use]
    pub fn new(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            rule: rule.into(),
            message: message.into(),
        }
    }

    /// Returns the stable diagnostic rule.
    #[must_use]
    pub fn rule(&self) -> &str {
        &self.rule
    }

    /// Returns the diagnostic message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_model_requires_desktop_and_plugin_surfaces() {
        let model = ProductModel::new("hawk2ui")
            .with_host_target(HostTarget::LinuxWayland)
            .with_host_target(HostTarget::PluginHost)
            .with_surface_kind(SurfaceKind::DesktopWindow)
            .with_surface_kind(SurfaceKind::PluginEditor)
            .with_capability(ProductCapability::NativeWindowing)
            .with_capability(ProductCapability::PluginEditorEmbedding);

        assert!(model.supports_surface(SurfaceKind::DesktopWindow));
        assert!(model.supports_surface(SurfaceKind::PluginEditor));
        assert!(model.has_capability(ProductCapability::NativeWindowing));
        assert!(model.has_capability(ProductCapability::PluginEditorEmbedding));
    }

    #[test]
    fn product_model_reports_missing_required_surfaces() {
        let model = ProductModel::new("incomplete")
            .with_surface_kind(SurfaceKind::DesktopWindow)
            .with_capability(ProductCapability::NativeWindowing);

        assert_eq!(
            model.validate_required_surfaces(),
            Err(ProductModelError::MissingSurface(SurfaceKind::PluginEditor))
        );
    }

    #[test]
    fn product_model_generates_and_validates_json_schema() {
        let model = ProductModel::new("hawk2ui")
            .with_host_target(HostTarget::LinuxWayland)
            .with_surface_kind(SurfaceKind::DesktopWindow)
            .with_surface_kind(SurfaceKind::PluginEditor)
            .with_capability(ProductCapability::NativeWindowing);
        let schema = crate::product_model_json_schema().expect("product model schema generates");
        let value = serde_json::to_value(&model).expect("product model serializes");

        crate::validate_product_model_json(&value).expect("valid product model passes schema");
        assert_eq!(schema["title"], "ProductModel");
        assert!(schema["properties"]["surface_kinds"].is_object());

        let invalid = serde_json::json!({
            "id": "broken",
            "host_targets": ["LinuxWayland"],
            "surface_kinds": ["DesktopWindow"],
            "capabilities": ["NativeWindowing"],
            "unexpected": true
        });
        let error = crate::validate_product_model_json(&invalid)
            .expect_err("unknown product model fields fail schema validation");
        assert_eq!(error.rule(), "schema.product.invalid");
    }
}

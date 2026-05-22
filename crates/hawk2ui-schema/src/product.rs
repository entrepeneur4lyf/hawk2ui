//! Product model records for supported `Hawk2UI` surfaces and targets.

/// Supported host target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceKind {
    /// Owned desktop application window.
    DesktopWindow,
    /// Embedded plugin editor surface.
    PluginEditor,
}

/// Product capability advertised by a `Hawk2UI` product model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
#[derive(Clone, Debug, Eq, PartialEq)]
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
}

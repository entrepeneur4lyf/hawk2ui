//! Public API inventory and stability classification records.
//!
//! ## Stability
//!
//! Inventory records are stable test fixtures for public API coverage. Entries
//! may be added for new public contracts, but existing public entries must not be
//! removed or reclassified without the breaking-change process.

/// Public root modules exposed by `hawk2ui-api`.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ApiModule {
    /// Artifact and sealed package contracts.
    Artifact,
    /// Diagnostic contracts.
    Diagnostic,
    /// Plugin-facing contracts.
    Plugin,
    /// Runtime and host binding contracts.
    Runtime,
    /// Host surface contracts.
    Surface,
}

impl ApiModule {
    /// Returns module documentation summary.
    #[must_use]
    pub const fn documentation(self) -> &'static str {
        match self {
            Self::Artifact => "Versioned artifact, hash, manifest, and target metadata contracts.",
            Self::Diagnostic => {
                "Stable diagnostic, source span, related context, and fix contracts."
            }
            Self::Plugin => {
                "Plugin parameter, automation, state, editor, preset, and realtime contracts."
            }
            Self::Runtime => {
                "Runtime lifecycle, job, host binding, capability, and script contracts."
            }
            Self::Surface => {
                "Desktop and embedded host surface metrics, events, and repaint contracts."
            }
        }
    }
}

/// API stability status for an inventoried type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApiTypeStatus {
    /// Public stable API.
    Public,
    /// Internal implementation detail.
    Internal,
    /// Public API behind a named feature flag.
    FeatureGated,
    /// Test-only helper API.
    TestOnly,
}

/// Primary audience for an inventoried type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApiTypeAudience {
    /// Author-facing application and component code.
    Author,
    /// Build, package, and artifact tooling.
    Build,
    /// Runtime, renderer, and host integrations.
    Runtime,
    /// Plugin and audio-host integrations.
    Plugin,
    /// Test and conformance tooling.
    Test,
}

/// Single API inventory entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ApiTypeEntry {
    module: ApiModule,
    name: &'static str,
    status: ApiTypeStatus,
    audience: ApiTypeAudience,
}

impl ApiTypeEntry {
    /// Creates an API inventory entry.
    #[must_use]
    pub const fn new(
        module: ApiModule,
        name: &'static str,
        status: ApiTypeStatus,
        audience: ApiTypeAudience,
    ) -> Self {
        Self {
            module,
            name,
            status,
            audience,
        }
    }

    /// Returns the root API module.
    #[must_use]
    pub const fn module(&self) -> ApiModule {
        self.module
    }

    /// Returns the type name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Returns API stability status.
    #[must_use]
    pub const fn status(&self) -> ApiTypeStatus {
        self.status
    }

    /// Returns the primary API audience.
    #[must_use]
    pub const fn audience(&self) -> ApiTypeAudience {
        self.audience
    }
}

/// Public API inventory for production stability checks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApiInventory {
    types: Vec<ApiTypeEntry>,
}

impl ApiInventory {
    /// Creates the production baseline inventory.
    #[must_use]
    pub fn production_baseline() -> Self {
        use ApiModule::{
            Artifact as ArtifactModule, Diagnostic as DiagnosticModule, Plugin as PluginModule,
            Runtime as RuntimeModule, Surface as SurfaceModule,
        };
        use ApiTypeAudience::{Author, Build, Plugin, Runtime, Test};
        use ApiTypeStatus::{FeatureGated, Internal, Public, TestOnly};

        Self {
            types: vec![
                ApiTypeEntry::new(DiagnosticModule, "Diagnostic", Public, Author),
                ApiTypeEntry::new(DiagnosticModule, "DiagnosticSeverity", Public, Author),
                ApiTypeEntry::new(DiagnosticModule, "RuleId", Public, Author),
                ApiTypeEntry::new(DiagnosticModule, "SourceSpan", Public, Author),
                ApiTypeEntry::new(DiagnosticModule, "SuggestedFix", Public, Author),
                ApiTypeEntry::new(DiagnosticModule, "RelatedContext", Public, Author),
                ApiTypeEntry::new(ArtifactModule, "ArtifactId", Public, Build),
                ApiTypeEntry::new(ArtifactModule, "ArtifactHash", Public, Build),
                ApiTypeEntry::new(ArtifactModule, "ArtifactSchemaVersion", Public, Runtime),
                ApiTypeEntry::new(ArtifactModule, "ArtifactVersionError", Public, Runtime),
                ApiTypeEntry::new(ArtifactModule, "ArtifactCapability", Public, Build),
                ApiTypeEntry::new(ArtifactModule, "ArtifactManifestSnapshot", Public, Build),
                ApiTypeEntry::new(ArtifactModule, "CompiledAssetKind", Public, Build),
                ApiTypeEntry::new(ArtifactModule, "CompiledAssetRecord", Public, Build),
                ApiTypeEntry::new(ArtifactModule, "CompiledStyleRecord", Public, Build),
                ApiTypeEntry::new(ArtifactModule, "CompiledScriptRecord", Public, Build),
                ApiTypeEntry::new(ArtifactModule, "TargetKind", Public, Build),
                ApiTypeEntry::new(ArtifactModule, "TargetMetadata", Public, Build),
                ApiTypeEntry::new(SurfaceModule, "HostSurfaceContract", Public, Runtime),
                ApiTypeEntry::new(SurfaceModule, "SurfaceKind", Public, Runtime),
                ApiTypeEntry::new(SurfaceModule, "SurfaceMetrics", Public, Runtime),
                ApiTypeEntry::new(SurfaceModule, "MouseButton", Public, Runtime),
                ApiTypeEntry::new(SurfaceModule, "KeyModifiers", Public, Runtime),
                ApiTypeEntry::new(SurfaceModule, "KeyEvent", Public, Runtime),
                ApiTypeEntry::new(SurfaceModule, "InputEvent", Public, Runtime),
                ApiTypeEntry::new(SurfaceModule, "RepaintReason", Public, Runtime),
                ApiTypeEntry::new(SurfaceModule, "RepaintRequest", Public, Runtime),
                ApiTypeEntry::new(SurfaceModule, "FrameSchedule", Public, Runtime),
                ApiTypeEntry::new(RuntimeModule, "CapabilityKey", Public, Runtime),
                ApiTypeEntry::new(RuntimeModule, "RuntimePhase", Public, Runtime),
                ApiTypeEntry::new(RuntimeModule, "BindingDirection", Public, Runtime),
                ApiTypeEntry::new(RuntimeModule, "HostBindingContract", Public, Runtime),
                ApiTypeEntry::new(RuntimeModule, "RuntimeJobId", Public, Runtime),
                ApiTypeEntry::new(RuntimeModule, "RuntimeJobKind", Public, Runtime),
                ApiTypeEntry::new(RuntimeModule, "RuntimeJobStatus", Public, Runtime),
                ApiTypeEntry::new(RuntimeModule, "RuntimeJob", Public, Runtime),
                ApiTypeEntry::new(RuntimeModule, "RuntimeLifecycleHook", Public, Runtime),
                ApiTypeEntry::new(PluginModule, "ParameterId", Public, Plugin),
                ApiTypeEntry::new(PluginModule, "AutomationGesture", Public, Plugin),
                ApiTypeEntry::new(PluginModule, "PluginParameterContract", Public, Plugin),
                ApiTypeEntry::new(PluginModule, "PluginEditorKind", Public, Plugin),
                ApiTypeEntry::new(PluginModule, "PluginEditorContract", Public, Plugin),
                ApiTypeEntry::new(PluginModule, "PluginStateFormat", Public, Plugin),
                ApiTypeEntry::new(PluginModule, "PluginStateEntry", Public, Plugin),
                ApiTypeEntry::new(PluginModule, "PluginStateContract", Public, Plugin),
                ApiTypeEntry::new(PluginModule, "PluginPresetContract", Public, Plugin),
                ApiTypeEntry::new(PluginModule, "RealtimeDataKind", Public, Plugin),
                ApiTypeEntry::new(PluginModule, "RealtimeDataDirection", Public, Plugin),
                ApiTypeEntry::new(PluginModule, "RealtimeDataContract", Public, Plugin),
                ApiTypeEntry::new(
                    RuntimeModule,
                    "ExperimentalScriptEngineContract",
                    FeatureGated,
                    Runtime,
                ),
                ApiTypeEntry::new(ArtifactModule, "ArtifactBuilderInternals", Internal, Build),
                ApiTypeEntry::new(SurfaceModule, "SurfaceCompileFixture", TestOnly, Test),
            ],
        }
    }

    /// Returns public root modules in deterministic order.
    #[must_use]
    pub const fn root_modules(&self) -> [ApiModule; 5] {
        [
            ApiModule::Artifact,
            ApiModule::Diagnostic,
            ApiModule::Plugin,
            ApiModule::Runtime,
            ApiModule::Surface,
        ]
    }

    /// Returns all inventoried types.
    #[must_use]
    pub fn types(&self) -> &[ApiTypeEntry] {
        &self.types
    }

    /// Returns inventoried types for a module.
    #[must_use]
    pub fn types_for_module(&self, module: ApiModule) -> Vec<&ApiTypeEntry> {
        self.types
            .iter()
            .filter(|entry| entry.module == module)
            .collect()
    }
}

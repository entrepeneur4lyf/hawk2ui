//! Trust boundary records.

/// Security trust boundary for data entering or moving through the system.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrustBoundary {
    /// Author-controlled source files.
    AuthorSource,
    /// Build-produced sealed artifacts.
    CompiledArtifact,
    /// Runtime-owned state.
    RuntimeState,
    /// Native host-provided data.
    HostData,
    /// End-user-provided data.
    UserData,
    /// Plugin host-provided data.
    PluginHostData,
    /// Secret material.
    Secret,
    /// Asset payloads.
    Asset,
}

impl TrustBoundary {
    /// Human-readable trust boundary label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::AuthorSource => "author source",
            Self::CompiledArtifact => "compiled artifact",
            Self::RuntimeState => "runtime state",
            Self::HostData => "host data",
            Self::UserData => "user data",
            Self::PluginHostData => "plugin host data",
            Self::Secret => "secret",
            Self::Asset => "asset",
        }
    }

    /// Stable diagnostic label segment.
    #[must_use]
    pub const fn diagnostic_key(self) -> &'static str {
        match self {
            Self::AuthorSource => "author-source",
            Self::CompiledArtifact => "compiled-artifact",
            Self::RuntimeState => "runtime-state",
            Self::HostData => "host-data",
            Self::UserData => "user-data",
            Self::PluginHostData => "plugin-host-data",
            Self::Secret => "secret",
            Self::Asset => "asset",
        }
    }
}

/// Classified trust-boundary record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrustRecord {
    /// Stable record ID.
    pub id: String,
    /// Data trust boundary.
    pub boundary: TrustBoundary,
    /// Source or owner of the data.
    pub origin: String,
}

impl TrustRecord {
    /// Creates a trust record.
    #[must_use]
    pub fn new(id: impl Into<String>, boundary: TrustBoundary, origin: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            boundary,
            origin: origin.into(),
        }
    }

    /// Returns a stable diagnostic label for trust-boundary violations.
    #[must_use]
    pub fn diagnostic_label(&self) -> String {
        format!(
            "trust.{}:{}@{}",
            self.boundary.diagnostic_key(),
            self.id,
            self.origin
        )
    }
}

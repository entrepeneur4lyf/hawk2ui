//! Sealed JavaScript module graph metadata for production runtime artifacts.

use std::{collections::BTreeSet, error::Error, fmt};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::PackageManagerMetadata;

/// Sealed JavaScript module graph.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SealedJsModuleGraph {
    entrypoint: String,
    modules: Vec<SealedJsModule>,
    chunks: Vec<SealedJsChunk>,
    package_manager: PackageManagerMetadata,
}

impl SealedJsModuleGraph {
    /// Creates a sealed module graph with package-manager metadata.
    #[must_use]
    pub fn new(entrypoint: impl Into<String>, package_manager: PackageManagerMetadata) -> Self {
        Self {
            entrypoint: entrypoint.into(),
            modules: Vec::new(),
            chunks: Vec::new(),
            package_manager,
        }
    }

    /// Adds a sealed module.
    #[must_use]
    pub fn with_module(mut self, module: SealedJsModule) -> Self {
        self.modules.push(module);
        self
    }

    /// Adds a chunk record.
    #[must_use]
    pub fn with_chunk(mut self, chunk: SealedJsChunk) -> Self {
        self.chunks.push(chunk);
        self
    }

    /// Returns the graph entrypoint specifier.
    #[must_use]
    pub fn entrypoint(&self) -> &str {
        &self.entrypoint
    }

    /// Returns package-manager metadata captured for this graph.
    #[must_use]
    pub const fn package_manager(&self) -> &PackageManagerMetadata {
        &self.package_manager
    }

    /// Returns all sealed modules.
    #[must_use]
    pub fn modules(&self) -> &[SealedJsModule] {
        &self.modules
    }

    /// Returns all sealed chunks.
    #[must_use]
    pub fn chunks(&self) -> &[SealedJsChunk] {
        &self.chunks
    }

    /// Looks up a sealed module by specifier.
    #[must_use]
    pub fn module(&self, specifier: &str) -> Option<&SealedJsModule> {
        self.modules
            .iter()
            .find(|module| module.specifier == specifier)
    }

    /// Validates entrypoint existence, module hashes, import targets, and chunk references.
    ///
    /// # Errors
    ///
    /// Returns [`JsBundleError`] when the graph is incomplete or not reproducible.
    pub fn validate(&self) -> Result<(), JsBundleError> {
        let mut specifiers = BTreeSet::new();
        for module in &self.modules {
            if !specifiers.insert(module.specifier.as_str()) {
                return Err(JsBundleError::new(
                    "build.js-bundle.module-duplicate",
                    format!(
                        "module appears more than once in sealed graph: {}",
                        module.specifier
                    ),
                ));
            }
        }

        if !specifiers.contains(self.entrypoint.as_str()) {
            return Err(JsBundleError::new(
                "build.js-bundle.entrypoint-missing",
                format!(
                    "entrypoint is not present in sealed graph: {}",
                    self.entrypoint
                ),
            ));
        }

        for module in &self.modules {
            let actual_hash = sha256_hex(module.source.as_bytes());
            if actual_hash != module.sha256 {
                return Err(JsBundleError::new(
                    "build.js-bundle.hash-mismatch",
                    format!(
                        "module hash does not match sealed source for {}",
                        module.specifier
                    ),
                ));
            }
            for target in &module.static_imports {
                if !is_host_module_specifier(target) && !specifiers.contains(target.as_str()) {
                    return Err(JsBundleError::new(
                        "build.js-bundle.static-import-missing",
                        format!("static import target is not present in sealed graph: {target}"),
                    ));
                }
            }
            for target in &module.dynamic_imports {
                if !specifiers.contains(target.as_str()) {
                    return Err(JsBundleError::new(
                        "build.js-bundle.dynamic-import-missing",
                        format!("dynamic import target is not present in sealed graph: {target}"),
                    ));
                }
            }
        }

        for chunk in &self.chunks {
            for module in &chunk.modules {
                if !specifiers.contains(module.as_str()) {
                    return Err(JsBundleError::new(
                        "build.js-bundle.chunk-module-missing",
                        format!("chunk references missing sealed module: {module}"),
                    ));
                }
            }
        }

        Ok(())
    }
}

/// One sealed JavaScript module.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SealedJsModule {
    specifier: String,
    source: String,
    sha256: String,
    #[serde(default = "SealedJsDependencyOrigin::unspecified")]
    dependency_origin: SealedJsDependencyOrigin,
    source_map: Option<SealedJsSourceMap>,
    static_imports: Vec<String>,
    dynamic_imports: Vec<String>,
    chunk: Option<String>,
}

impl SealedJsModule {
    /// Creates a sealed module.
    #[must_use]
    pub fn new(
        specifier: impl Into<String>,
        source: impl Into<String>,
        sha256: impl Into<String>,
    ) -> Self {
        let specifier = specifier.into();
        Self {
            dependency_origin: SealedJsDependencyOrigin::workspace(specifier.clone()),
            specifier,
            source: source.into(),
            sha256: sha256.into(),
            source_map: None,
            static_imports: Vec::new(),
            dynamic_imports: Vec::new(),
            chunk: None,
        }
    }

    /// Adds dependency-origin metadata for this module.
    #[must_use]
    pub fn with_dependency_origin(mut self, dependency_origin: SealedJsDependencyOrigin) -> Self {
        self.dependency_origin = dependency_origin;
        self
    }

    /// Adds one static import specifier.
    #[must_use]
    pub fn with_static_import(mut self, specifier: impl Into<String>) -> Self {
        self.static_imports.push(specifier.into());
        self
    }

    /// Adds one dynamic import target specifier.
    #[must_use]
    pub fn with_dynamic_import(mut self, specifier: impl Into<String>) -> Self {
        self.dynamic_imports.push(specifier.into());
        self
    }

    /// Adds source map metadata.
    #[must_use]
    pub fn with_source_map(mut self, source_map: SealedJsSourceMap) -> Self {
        self.source_map = Some(source_map);
        self
    }

    /// Assigns this module to a chunk.
    #[must_use]
    pub fn with_chunk(mut self, chunk: impl Into<String>) -> Self {
        self.chunk = Some(chunk.into());
        self
    }

    /// Returns the module specifier.
    #[must_use]
    pub fn specifier(&self) -> &str {
        &self.specifier
    }

    /// Returns sealed module source.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Returns sealed module SHA-256 hex digest.
    #[must_use]
    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    /// Returns dependency-origin metadata for this sealed module.
    #[must_use]
    pub const fn dependency_origin(&self) -> &SealedJsDependencyOrigin {
        &self.dependency_origin
    }

    /// Returns source map metadata.
    #[must_use]
    pub const fn source_map(&self) -> Option<&SealedJsSourceMap> {
        self.source_map.as_ref()
    }

    /// Returns static import metadata.
    #[must_use]
    pub fn static_imports(&self) -> &[String] {
        &self.static_imports
    }

    /// Returns dynamic import metadata.
    #[must_use]
    pub fn dynamic_imports(&self) -> &[String] {
        &self.dynamic_imports
    }

    /// Returns assigned chunk id.
    #[must_use]
    pub fn chunk(&self) -> Option<&str> {
        self.chunk.as_deref()
    }
}

/// Dependency origin for one sealed JavaScript module.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub enum SealedJsDependencyOrigin {
    /// Module produced from a workspace source or build-output path.
    Workspace { path: String },
    /// Module produced from an installed package dependency.
    Package {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        version: Option<String>,
    },
    /// Module generated by a build tool or framework adapter.
    Generated { tool: String },
}

impl SealedJsDependencyOrigin {
    /// Creates workspace dependency-origin metadata.
    #[must_use]
    pub fn workspace(path: impl Into<String>) -> Self {
        Self::Workspace { path: path.into() }
    }

    /// Creates package dependency-origin metadata.
    #[must_use]
    pub fn package(name: impl Into<String>, version: Option<&str>) -> Self {
        Self::Package {
            name: name.into(),
            version: version.map(str::to_owned),
        }
    }

    /// Creates generated dependency-origin metadata.
    #[must_use]
    pub fn generated(tool: impl Into<String>) -> Self {
        Self::Generated { tool: tool.into() }
    }

    fn unspecified() -> Self {
        Self::Generated {
            tool: "unspecified".to_owned(),
        }
    }
}

/// Source map metadata for a sealed JavaScript module.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub enum SealedJsSourceMap {
    /// Inline source map payload.
    Inline(String),
    /// External source map record with content hash.
    External { path: String, sha256: String },
}

impl SealedJsSourceMap {
    /// Creates inline source map metadata.
    #[must_use]
    pub fn inline(source_map: impl Into<String>) -> Self {
        Self::Inline(source_map.into())
    }

    /// Creates external source map metadata.
    #[must_use]
    pub fn external(path: impl Into<String>, sha256: impl Into<String>) -> Self {
        Self::External {
            path: path.into(),
            sha256: sha256.into(),
        }
    }

    /// Returns the source map content hash recorded for release metadata.
    #[must_use]
    pub fn sha256(&self) -> String {
        match self {
            Self::Inline(source_map) => sha256_hex(source_map.as_bytes()),
            Self::External { sha256, .. } => sha256.clone(),
        }
    }
}

/// Sealed JavaScript chunk metadata.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SealedJsChunk {
    id: String,
    modules: Vec<String>,
}

impl SealedJsChunk {
    /// Creates a chunk record.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            modules: Vec::new(),
        }
    }

    /// Adds a module specifier carried by this chunk.
    #[must_use]
    pub fn with_module(mut self, specifier: impl Into<String>) -> Self {
        self.modules.push(specifier.into());
        self
    }

    /// Returns the chunk id.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns module specifiers carried by this chunk.
    #[must_use]
    pub fn modules(&self) -> &[String] {
        &self.modules
    }
}

/// Sealed JavaScript bundle validation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsBundleError {
    rule: String,
    message: String,
}

impl JsBundleError {
    fn new(rule: impl Into<String>, message: impl Into<String>) -> Self {
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

    /// Returns the human-readable diagnostic message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for JsBundleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.rule, self.message)
    }
}

impl Error for JsBundleError {}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

fn is_host_module_specifier(specifier: &str) -> bool {
    specifier.starts_with("hawk:")
}

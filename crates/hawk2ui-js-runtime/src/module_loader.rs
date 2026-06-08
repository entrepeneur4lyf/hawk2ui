//! Runtime-owned sealed JavaScript module graph loader.

use std::borrow::Cow;
use std::collections::HashMap;

use deno_core::{
    ModuleLoadOptions, ModuleLoadReferrer, ModuleLoadResponse, ModuleLoader, ModuleResolveResponse,
    ModuleSource, ModuleSourceCode, ModuleSpecifier, ModuleType, resolve_import, resolve_url,
};
use deno_error::JsErrorBox;

use crate::JsRuntimeError;
use crate::permissions::builtin_hawk_modules;

/// One JavaScript module available to the sealed runtime loader.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HawkJsModule {
    specifier: String,
    source: String,
}

impl HawkJsModule {
    /// Creates a sealed runtime module.
    #[must_use]
    pub fn new(specifier: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            specifier: specifier.into(),
            source: source.into(),
        }
    }

    /// Returns the module specifier.
    #[must_use]
    pub fn specifier(&self) -> &str {
        &self.specifier
    }

    /// Returns the module source.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }
}

/// Sealed runtime module graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HawkJsModuleGraph {
    entrypoint: String,
    modules: Vec<HawkJsModule>,
}

impl HawkJsModuleGraph {
    /// Creates a runtime module graph.
    #[must_use]
    pub fn new(entrypoint: impl Into<String>) -> Self {
        Self {
            entrypoint: entrypoint.into(),
            modules: Vec::new(),
        }
    }

    /// Adds one sealed module.
    #[must_use]
    pub fn with_module(mut self, module: HawkJsModule) -> Self {
        self.modules.push(module);
        self
    }

    /// Returns the graph entrypoint specifier.
    #[must_use]
    pub fn entrypoint(&self) -> &str {
        &self.entrypoint
    }

    /// Returns graph modules.
    #[must_use]
    pub fn modules(&self) -> &[HawkJsModule] {
        &self.modules
    }

    pub(crate) fn entrypoint_specifier(&self) -> Result<ModuleSpecifier, JsRuntimeError> {
        resolve_module_specifier(&self.entrypoint)
    }

    pub(crate) fn into_static_loader(self) -> Result<SealedModuleLoader, JsRuntimeError> {
        let mut modules = self
            .modules
            .into_iter()
            .map(|module| {
                resolve_module_specifier(&module.specifier)
                    .map(|specifier| (specifier, module.source))
            })
            .collect::<Result<Vec<_>, _>>()?;
        for (specifier, source) in builtin_hawk_modules() {
            if modules
                .iter()
                .any(|(existing, _)| existing.as_str() == *specifier)
            {
                continue;
            }
            modules.push((resolve_module_specifier(specifier)?, (*source).to_owned()));
        }
        Ok(SealedModuleLoader::new(modules))
    }
}

fn resolve_module_specifier(specifier: &str) -> Result<ModuleSpecifier, JsRuntimeError> {
    resolve_url(specifier).map_err(|error| {
        JsRuntimeError::new(
            "js-runtime.module.invalid-specifier",
            format!("module specifier is not a valid absolute URL: {error}"),
        )
    })
}

pub(crate) struct SealedModuleLoader {
    modules: HashMap<ModuleSpecifier, String>,
}

impl SealedModuleLoader {
    fn new(modules: impl IntoIterator<Item = (ModuleSpecifier, String)>) -> Self {
        Self {
            modules: modules.into_iter().collect(),
        }
    }
}

impl ModuleLoader for SealedModuleLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _kind: deno_core::ResolutionKind,
    ) -> ModuleResolveResponse {
        resolve_import(specifier, referrer).map_err(JsErrorBox::from_err)
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<&ModuleLoadReferrer>,
        _options: ModuleLoadOptions,
    ) -> ModuleLoadResponse {
        let response = self.modules.get(module_specifier).map_or_else(
            || Err(missing_module_error(module_specifier)),
            |source| {
                Ok(ModuleSource::new(
                    ModuleType::JavaScript,
                    ModuleSourceCode::String(source.clone().into()),
                    module_specifier,
                    None,
                ))
            },
        );
        ModuleLoadResponse::Sync(response)
    }

    fn load_external_source_map(&self, source_map_url: &str) -> Option<Cow<'_, [u8]>> {
        let specifier = resolve_url(source_map_url).ok()?;
        self.modules
            .get(&specifier)
            .map(|source_map| Cow::Borrowed(source_map.as_bytes()))
    }
}

fn missing_module_error(module_specifier: &ModuleSpecifier) -> JsErrorBox {
    let specifier = module_specifier.as_str();
    if specifier.starts_with("hawk:") {
        let supported = builtin_hawk_modules()
            .iter()
            .map(|(specifier, _)| *specifier)
            .collect::<Vec<_>>()
            .join(", ");
        JsErrorBox::generic(format!(
            "js-runtime.module.unsupported-hawk-import: `{specifier}` is not a supported Hawk2UI runtime module; supported modules: {supported}"
        ))
    } else {
        JsErrorBox::generic(format!(
            "js-runtime.module.not-sealed: `{specifier}` is not present in the sealed module graph"
        ))
    }
}

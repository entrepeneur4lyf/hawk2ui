#![forbid(unsafe_code)]
//! Production plugin and package adapters for `Hawk2UI` `CLAP`, `VST3`, AU, standalone, and desktop outputs.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    fs,
    path::Component,
    path::{Path, PathBuf},
};

use hawk2ui_build::SealedArtifact;
use hawk2ui_host::{HostPlatformHandle, PluginEditorConfig, PluginParentHandle, SurfaceMetrics};
use hawk2ui_plugin::{
    BundleOutput, FormatMetadata, ParameterModel, ParameterRecord, ParameterValue, PluginEditor,
    PluginEditorSize,
};
use hawk2ui_runtime::{RuntimeSceneFrame, RuntimeScenePayload};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-plugin-adapters";

/// Returns the canonical Cargo package name for diagnostics and conformance checks.
#[must_use]
pub const fn crate_name() -> &'static str {
    CRATE_NAME
}

/// Package output format.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub enum PackageFormat {
    /// CLAP plugin bundle.
    Clap,
    /// VST3 plugin bundle.
    Vst3,
    /// Audio Unit component bundle.
    Au,
    /// Standalone application.
    Standalone,
    /// Desktop application bundle.
    DesktopBundle,
    /// Sealed `Hawk2UI` artifact.
    SealedArtifact,
}

impl PackageFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Clap => "clap",
            Self::Vst3 => "vst3",
            Self::Au => "component",
            Self::Standalone | Self::DesktopBundle => "app",
            Self::SealedArtifact => "hawk2ui",
        }
    }

    fn manifest_key(self) -> &'static str {
        match self {
            Self::Clap => "clap",
            Self::Vst3 => "vst3",
            Self::Au => "au",
            Self::Standalone => "standalone",
            Self::DesktopBundle => "desktop-bundle",
            Self::SealedArtifact => "sealed-artifact",
        }
    }

    fn from_manifest_key(value: &str) -> Option<Self> {
        match value {
            "clap" => Some(Self::Clap),
            "vst3" => Some(Self::Vst3),
            "au" => Some(Self::Au),
            "standalone" => Some(Self::Standalone),
            "desktop-bundle" => Some(Self::DesktopBundle),
            "sealed-artifact" => Some(Self::SealedArtifact),
            _ => None,
        }
    }
}

/// CLAP GUI parent window API.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub enum ClapGuiWindowApi {
    /// Windows `HWND` parent.
    Win32,
    /// macOS Cocoa `NSView` parent.
    Cocoa,
    /// Linux X11 parent window.
    X11,
    /// Linux Wayland parent surface.
    Wayland,
}

impl ClapGuiWindowApi {
    /// Returns the CLAP ABI API name.
    #[must_use]
    pub const fn clap_name(self) -> &'static str {
        match self {
            Self::Win32 => "win32",
            Self::Cocoa => "cocoa",
            Self::X11 => "x11",
            Self::Wayland => "wayland",
        }
    }
}

/// Safe record of a CLAP GUI parent handle after API-specific validation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClapGuiParentHandle {
    api: ClapGuiWindowApi,
    raw_handle: u64,
}

impl ClapGuiParentHandle {
    /// Creates a CLAP GUI parent handle record from a raw host-provided handle value.
    ///
    /// # Errors
    ///
    /// Returns [`PackageDiagnostic`] when the raw handle value is zero.
    pub fn from_raw_parts(
        api: ClapGuiWindowApi,
        raw_handle: u64,
    ) -> Result<Self, PackageDiagnostic> {
        if raw_handle == 0 {
            Err(PackageDiagnostic::new(
                "package.clap-gui-parent.invalid-handle",
                "CLAP GUI parent handle must be nonzero",
            ))
        } else {
            Ok(Self { api, raw_handle })
        }
    }

    /// Returns the CLAP window API.
    #[must_use]
    pub const fn api(&self) -> ClapGuiWindowApi {
        self.api
    }

    /// Returns the raw platform handle value.
    #[must_use]
    pub const fn raw_handle(&self) -> u64 {
        self.raw_handle
    }

    /// Converts this CLAP GUI parent into `Hawk2UI`'s host platform handle record.
    ///
    /// X11 and Wayland CLAP handles do not carry the display/connection pointer, so the caller must
    /// pass the display handle from the native host context when mapping Linux parents.
    ///
    /// # Errors
    ///
    /// Returns [`PackageDiagnostic`] when a Linux display handle is required but missing or zero.
    pub fn to_host_platform_handle(
        &self,
        linux_display_handle: Option<u64>,
    ) -> Result<HostPlatformHandle, PackageDiagnostic> {
        match self.api {
            ClapGuiWindowApi::Win32 => Ok(HostPlatformHandle::windows_hwnd(self.raw_handle)),
            ClapGuiWindowApi::Cocoa => Ok(HostPlatformHandle::macos_ns_view(self.raw_handle)),
            ClapGuiWindowApi::X11 => Ok(HostPlatformHandle::linux_x11(
                require_linux_display(linux_display_handle)?,
                self.raw_handle,
            )),
            ClapGuiWindowApi::Wayland => Ok(HostPlatformHandle::linux_wayland(
                require_linux_display(linux_display_handle)?,
                self.raw_handle,
            )),
        }
    }

    /// Converts this CLAP GUI parent into a Baseview-compatible host handle when supported.
    ///
    /// Baseview's current Linux backend attaches through X11/XCB/XWayland-compatible parent
    /// handles, not native Wayland parent surfaces.
    ///
    /// # Errors
    ///
    /// Returns [`PackageDiagnostic`] when the CLAP API is unsupported by the current Baseview bridge
    /// or required Linux display metadata is missing.
    pub fn to_baseview_host_handle(
        &self,
        linux_display_handle: Option<u64>,
    ) -> Result<HostPlatformHandle, PackageDiagnostic> {
        if self.api == ClapGuiWindowApi::Wayland {
            return Err(PackageDiagnostic::new(
                "package.clap-gui-parent.unsupported-api",
                "Baseview plugin attachment does not support native Wayland CLAP GUI parents",
            ));
        }
        self.to_host_platform_handle(linux_display_handle)
    }
}

/// Runtime editor descriptor embedded into generated CLAP GUI libraries.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClapRuntimeEditorDescriptor {
    runtime_artifact: String,
    host_adapter: String,
    renderer: String,
}

impl ClapRuntimeEditorDescriptor {
    /// Creates a validated runtime editor descriptor.
    ///
    /// # Errors
    ///
    /// Returns [`PackageDiagnostic`] when the runtime artifact path, host adapter, or renderer ID is
    /// structurally invalid.
    pub fn new(
        runtime_artifact: impl Into<String>,
        host_adapter: impl Into<String>,
        renderer: impl Into<String>,
    ) -> Result<Self, PackageDiagnostic> {
        let runtime_artifact = runtime_artifact.into();
        let host_adapter = host_adapter.into();
        let renderer = renderer.into();
        if runtime_artifact.trim().is_empty()
            || !is_safe_relative_path(Path::new(&runtime_artifact))
            || runtime_artifact.contains('\0')
        {
            return Err(PackageDiagnostic::new(
                "package.clap-editor-descriptor.invalid-runtime-artifact",
                "CLAP runtime editor descriptor requires a safe relative runtime artifact path",
            ));
        }
        if !is_filesystem_segment(&host_adapter) {
            return Err(PackageDiagnostic::new(
                "package.clap-editor-descriptor.invalid-host-adapter",
                "CLAP runtime editor descriptor requires a non-empty host adapter ID",
            ));
        }
        if !is_filesystem_segment(&renderer) {
            return Err(PackageDiagnostic::new(
                "package.clap-editor-descriptor.invalid-renderer",
                "CLAP runtime editor descriptor requires a non-empty renderer ID",
            ));
        }
        Ok(Self {
            runtime_artifact,
            host_adapter,
            renderer,
        })
    }

    /// Serializes the descriptor payload exported by the generated CLAP library.
    #[must_use]
    pub fn to_export_payload(&self) -> String {
        format!(
            "runtime_artifact={}\nhost_adapter={}\nrenderer={}\n",
            self.runtime_artifact, self.host_adapter, self.renderer
        )
    }
}

/// Runtime editor descriptor loaded from a materialized CLAP package.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClapRuntimeEditorPackageDescriptor {
    host_adapter: String,
    renderer: String,
    runtime_artifact: String,
    format: PackageFormat,
    plugin_id: String,
    parameter_count: usize,
    editor_id: String,
    logical_width: f64,
    logical_height: f64,
    scale_factor: f64,
}

impl ClapRuntimeEditorPackageDescriptor {
    /// Returns the native host adapter ID.
    #[must_use]
    pub fn host_adapter(&self) -> &str {
        &self.host_adapter
    }

    /// Returns the renderer ID.
    #[must_use]
    pub fn renderer(&self) -> &str {
        &self.renderer
    }

    /// Returns the package-relative runtime artifact path.
    #[must_use]
    pub fn runtime_artifact(&self) -> &str {
        &self.runtime_artifact
    }

    /// Returns the plugin package format.
    #[must_use]
    pub const fn format(&self) -> PackageFormat {
        self.format
    }

    /// Returns the plugin ID.
    #[must_use]
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    /// Returns the expected generated parameter count.
    #[must_use]
    pub const fn parameter_count(&self) -> usize {
        self.parameter_count
    }

    /// Returns the stable editor ID that owns the attached surface.
    #[must_use]
    pub fn editor_id(&self) -> &str {
        &self.editor_id
    }

    /// Returns the logical editor width requested by the package.
    #[must_use]
    pub const fn logical_width(&self) -> f64 {
        self.logical_width
    }

    /// Returns the logical editor height requested by the package.
    #[must_use]
    pub const fn logical_height(&self) -> f64 {
        self.logical_height
    }

    /// Returns the initial editor device scale factor.
    #[must_use]
    pub const fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    fn parse(source: &str) -> Result<Self, PackageMaterializationError> {
        let table = source.parse::<toml::Table>().map_err(|error| {
            materialization_error(
                "package.clap-runtime-editor.descriptor-parse-failed",
                format!("failed to parse CLAP runtime editor descriptor: {error}"),
            )
        })?;
        let host_adapter = required_toml_string(&table, "host_adapter")?;
        let renderer = required_toml_string(&table, "renderer")?;
        let runtime_artifact = required_toml_string(&table, "runtime_artifact")?;
        let format = required_toml_string(&table, "format")?;
        let plugin_id = required_toml_string(&table, "plugin_id")?;
        let parameter_count = required_toml_usize(&table, "parameter_count")?;
        let editor_id = required_toml_string(&table, "editor_id")?;
        let logical_width = required_toml_f64(&table, "logical_width")?;
        let logical_height = required_toml_f64(&table, "logical_height")?;
        let scale_factor = required_toml_f64(&table, "scale_factor")?;
        let format = PackageFormat::from_manifest_key(&format).ok_or_else(|| {
            materialization_error(
                "package.clap-runtime-editor.invalid-format",
                "CLAP runtime editor descriptor declares an unsupported package format",
            )
        })?;
        if host_adapter != "baseview" {
            return Err(materialization_error(
                "package.clap-runtime-editor.invalid-host-adapter",
                "CLAP runtime editor descriptor must use the baseview host adapter",
            ));
        }
        if renderer != "skia" {
            return Err(materialization_error(
                "package.clap-runtime-editor.invalid-renderer",
                "CLAP runtime editor descriptor must use the skia renderer",
            ));
        }
        if format != PackageFormat::Clap {
            return Err(materialization_error(
                "package.clap-runtime-editor.invalid-format",
                "CLAP runtime editor descriptor must describe a CLAP package",
            ));
        }
        if !is_reverse_dns_id(&plugin_id) {
            return Err(materialization_error(
                "package.clap-runtime-editor.invalid-plugin-id",
                "CLAP runtime editor descriptor must declare a valid plugin ID",
            ));
        }
        if !is_safe_relative_path(Path::new(&runtime_artifact)) {
            return Err(materialization_error(
                "package.clap-runtime-editor.invalid-runtime-artifact",
                "CLAP runtime editor descriptor requires a safe relative runtime artifact path",
            ));
        }
        if editor_id.trim().is_empty()
            || !logical_width.is_finite()
            || !logical_height.is_finite()
            || !scale_factor.is_finite()
            || logical_width <= 0.0
            || logical_height <= 0.0
            || scale_factor <= 0.0
        {
            return Err(materialization_error(
                "package.clap-runtime-editor.invalid-editor",
                "CLAP runtime editor descriptor requires a non-empty editor ID and positive finite metrics",
            ));
        }
        Ok(Self {
            host_adapter,
            renderer,
            runtime_artifact,
            format,
            plugin_id,
            parameter_count,
            editor_id,
            logical_width,
            logical_height,
            scale_factor,
        })
    }
}

/// Runtime-backed editor session loaded from a verified CLAP package.
#[derive(Clone, Debug, PartialEq)]
pub struct ClapRuntimeEditorSession {
    package_root: String,
    runtime_artifact_path: String,
    descriptor: ClapRuntimeEditorPackageDescriptor,
    runtime_artifact: serde_json::Value,
    sealed_artifact: SealedArtifact,
}

/// Host configuration needed to attach a verified CLAP runtime editor through Baseview.
#[derive(Clone, Debug, PartialEq)]
pub struct ClapRuntimeEditorHostConfig {
    editor_config: PluginEditorConfig,
    host_parent: HostPlatformHandle,
}

impl ClapRuntimeEditorHostConfig {
    /// Returns the format-neutral plugin editor configuration.
    #[must_use]
    pub const fn editor_config(&self) -> &PluginEditorConfig {
        &self.editor_config
    }

    /// Returns the validated native host parent handle for Baseview attachment.
    #[must_use]
    pub const fn host_parent(&self) -> HostPlatformHandle {
        self.host_parent
    }
}

impl ClapRuntimeEditorSession {
    /// Loads a runtime-backed editor session from a materialized CLAP package directory.
    ///
    /// # Errors
    ///
    /// Returns [`PackageMaterializationError`] when package hashes fail, the editor descriptor is
    /// invalid, or the referenced runtime artifact cannot be read and parsed as JSON.
    pub fn load_from_package(
        package_root: impl AsRef<Path>,
    ) -> Result<Self, PackageMaterializationError> {
        let package_root = package_root.as_ref();
        let resources_path = package_root.join("Contents").join("Resources");
        let hash_manifest_path = resources_path.join("hawk2ui-hashes.toml");
        if !hash_manifest_matches(package_root, &hash_manifest_path) {
            return Err(materialization_error(
                "package.clap-runtime-editor.hash-invalid",
                "CLAP runtime editor package hash manifest does not match package contents",
            ));
        }
        let descriptor_path = resources_path.join("hawk2ui-editor.toml");
        let descriptor_source = fs::read_to_string(&descriptor_path).map_err(|error| {
            materialization_error(
                "package.clap-runtime-editor.descriptor-read-failed",
                format!(
                    "failed to read CLAP runtime editor descriptor {}: {error}",
                    descriptor_path.display()
                ),
            )
        })?;
        let descriptor = ClapRuntimeEditorPackageDescriptor::parse(&descriptor_source)?;
        let runtime_artifact_path = package_root.join(descriptor.runtime_artifact());
        let runtime_artifact_source =
            fs::read_to_string(&runtime_artifact_path).map_err(|error| {
                materialization_error(
                    "package.clap-runtime-editor.runtime-artifact-read-failed",
                    format!(
                        "failed to read CLAP runtime artifact {}: {error}",
                        runtime_artifact_path.display()
                    ),
                )
            })?;
        let runtime_artifact: serde_json::Value = serde_json::from_str(&runtime_artifact_source)
            .map_err(|error| {
                materialization_error(
                    "package.clap-runtime-editor.runtime-artifact-parse-failed",
                    format!("failed to parse CLAP runtime artifact JSON: {error}"),
                )
            })?;
        SealedArtifact::validate_json(&runtime_artifact).map_err(|error| {
            materialization_error(
                "package.clap-runtime-editor.runtime-artifact-schema-invalid",
                sealed_artifact_error_message(&error),
            )
        })?;
        let sealed_artifact: SealedArtifact = serde_json::from_value(runtime_artifact.clone())
            .map_err(|error| {
                materialization_error(
                    "package.clap-runtime-editor.runtime-artifact-parse-failed",
                    format!("failed to decode CLAP sealed runtime artifact: {error}"),
                )
            })?;
        Ok(Self {
            package_root: package_root.to_string_lossy().into_owned(),
            runtime_artifact_path: runtime_artifact_path.to_string_lossy().into_owned(),
            descriptor,
            runtime_artifact,
            sealed_artifact,
        })
    }

    /// Loads a runtime-backed editor session by resolving the package root from a CLAP plugin path.
    ///
    /// The supplied path may be the materialized package root, the generated `.clap` entry file, or a
    /// file nested under the package such as a generated dynamic library path. The resolver walks
    /// ancestors until it finds the hashed `Hawk2UI` runtime editor descriptor.
    ///
    /// # Errors
    ///
    /// Returns [`PackageMaterializationError`] when no runtime-backed package root can be found or
    /// when the resolved package fails normal verified session loading.
    pub fn load_from_clap_plugin_path(
        plugin_path: impl AsRef<Path>,
    ) -> Result<Self, PackageMaterializationError> {
        let plugin_path = plugin_path.as_ref();
        for candidate in plugin_path.ancestors() {
            if is_runtime_editor_package_root(candidate) {
                return Self::load_from_package(candidate);
            }
        }
        Err(materialization_error(
            "package.clap-runtime-editor.package-root-unresolved",
            format!(
                "failed to resolve Hawk2UI CLAP runtime editor package root from {}",
                plugin_path.display()
            ),
        ))
    }

    /// Returns the materialized package root.
    #[must_use]
    pub fn package_root(&self) -> &str {
        &self.package_root
    }

    /// Returns the resolved runtime artifact path.
    #[must_use]
    pub fn runtime_artifact_path(&self) -> &str {
        &self.runtime_artifact_path
    }

    /// Returns the parsed editor descriptor.
    #[must_use]
    pub const fn descriptor(&self) -> &ClapRuntimeEditorPackageDescriptor {
        &self.descriptor
    }

    /// Returns the parsed sealed runtime artifact payload.
    #[must_use]
    pub const fn runtime_artifact(&self) -> &serde_json::Value {
        &self.runtime_artifact
    }

    /// Returns the typed sealed runtime artifact consumed by live editor rendering.
    #[must_use]
    pub const fn sealed_artifact(&self) -> &SealedArtifact {
        &self.sealed_artifact
    }

    /// Builds the host configuration needed to attach this editor to a CLAP parent window.
    ///
    /// # Errors
    ///
    /// Returns [`PackageDiagnostic`] when the CLAP parent cannot be represented by the current
    /// Baseview bridge.
    pub fn baseview_host_config(
        &self,
        parent: ClapGuiParentHandle,
        linux_display_handle: Option<u64>,
    ) -> Result<ClapRuntimeEditorHostConfig, PackageDiagnostic> {
        let host_parent = parent.to_baseview_host_handle(linux_display_handle)?;
        let editor_config = PluginEditorConfig::new(
            self.descriptor.editor_id(),
            PluginParentHandle::opaque(format!(
                "clap:{}:{}",
                parent.api().clap_name(),
                parent.raw_handle()
            )),
            SurfaceMetrics::new(
                self.descriptor.logical_width(),
                self.descriptor.logical_height(),
                self.descriptor.scale_factor(),
            ),
        );
        Ok(ClapRuntimeEditorHostConfig {
            editor_config,
            host_parent,
        })
    }

    /// Builds the runtime scene frame carried by the verified sealed artifact payload.
    ///
    /// # Errors
    ///
    /// Returns [`PackageMaterializationError`] when the sealed artifact does not carry a runtime
    /// scene payload or when the payload cannot be converted into a renderable runtime scene.
    pub fn runtime_scene_frame(&self) -> Result<RuntimeSceneFrame, PackageMaterializationError> {
        let runtime_scene = self.sealed_artifact.runtime_scene.as_ref().ok_or_else(|| {
            materialization_error(
                "package.clap-runtime-editor.runtime-scene-missing",
                "CLAP runtime editor sealed artifact does not contain a runtime scene payload",
            )
        })?;
        RuntimeScenePayload::from_json(runtime_scene)
            .and_then(|payload| payload.build_frame())
            .map_err(|error| {
                materialization_error(
                    format!("package.clap-runtime-editor.{}", error.rule()),
                    error.message(),
                )
            })
    }
}

fn is_runtime_editor_package_root(candidate: &Path) -> bool {
    let resources_path = candidate.join("Contents").join("Resources");
    resources_path.join("hawk2ui-editor.toml").is_file()
        && resources_path.join("hawk2ui-hashes.toml").is_file()
}

fn sealed_artifact_error_message(error: &hawk2ui_build::SealedArtifactError) -> String {
    match error {
        hawk2ui_build::SealedArtifactError::IncompatibleSchema { diagnostic, .. }
        | hawk2ui_build::SealedArtifactError::SchemaGeneration { diagnostic }
        | hawk2ui_build::SealedArtifactError::SchemaValidation { diagnostic }
        | hawk2ui_build::SealedArtifactError::ContainerSerialization { diagnostic }
        | hawk2ui_build::SealedArtifactError::ContainerVerification { diagnostic }
        | hawk2ui_build::SealedArtifactError::SignaturePolicy { diagnostic }
        | hawk2ui_build::SealedArtifactError::SignatureVerification { diagnostic } => {
            diagnostic.message.clone()
        }
    }
}

/// CLAP plugin entry metadata derived from the `clap-sys` ABI contract.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClapPluginEntryPlan {
    clap_version: String,
    entry_symbol: String,
    factory_id: String,
    plugin_id: String,
    name: String,
    vendor: String,
    version: String,
    features: Vec<String>,
    descriptor_abi: String,
}

impl ClapPluginEntryPlan {
    /// Creates CLAP entry metadata from plugin metadata.
    #[must_use]
    pub fn from_metadata(metadata: &FormatMetadata) -> Self {
        let mut features = metadata
            .features
            .iter()
            .filter(|feature| is_supported_clap_feature(feature))
            .cloned()
            .collect::<Vec<_>>();
        if features.is_empty() {
            features.push(clap_feature_string(
                clap_sys::plugin_features::CLAP_PLUGIN_FEATURE_UTILITY,
            ));
        }
        Self {
            clap_version: clap_version_string(),
            entry_symbol: "clap_entry".into(),
            factory_id: clap_feature_string(
                clap_sys::factory::plugin_factory::CLAP_PLUGIN_FACTORY_ID,
            ),
            plugin_id: metadata.id.clone(),
            name: metadata.display_name.clone(),
            vendor: metadata.vendor.clone(),
            version: metadata.version.clone(),
            features,
            descriptor_abi: format!(
                "clap_plugin_descriptor:{}",
                std::mem::size_of::<clap_sys::plugin::clap_plugin_descriptor>()
            ),
        }
    }

    /// Returns the CLAP entry symbol required by hosts.
    #[must_use]
    pub fn entry_symbol(&self) -> &str {
        &self.entry_symbol
    }

    /// Returns the CLAP plugin factory identifier.
    #[must_use]
    pub fn factory_id(&self) -> &str {
        &self.factory_id
    }

    /// Returns the CLAP ABI version.
    #[must_use]
    pub fn clap_version(&self) -> &str {
        &self.clap_version
    }

    /// Returns the plugin identifier.
    #[must_use]
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    /// Returns CLAP feature strings accepted by the adapter.
    #[must_use]
    pub fn features(&self) -> &[String] {
        &self.features
    }

    fn manifest(&self) -> String {
        let features = self
            .features
            .iter()
            .map(|feature| quoted_metadata_string(feature))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "clap_version = {}\nentry_symbol = {}\nfactory_id = {}\nplugin_id = {}\nname = {}\nvendor = {}\nversion = {}\ndescriptor_abi = {}\nfeatures = [{}]\n",
            quoted_metadata_string(&self.clap_version),
            quoted_metadata_string(&self.entry_symbol),
            quoted_metadata_string(&self.factory_id),
            quoted_metadata_string(&self.plugin_id),
            quoted_metadata_string(&self.name),
            quoted_metadata_string(&self.vendor),
            quoted_metadata_string(&self.version),
            quoted_metadata_string(&self.descriptor_abi),
            features
        )
    }
}

/// Generated CLAP `cdylib` scaffold for producing a loadable entry-library target.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClapCdylibScaffold {
    entry: ClapPluginEntryPlan,
    package_name: String,
    library_file_stem: String,
    parameter_source: String,
    parameter_value_source: String,
    parameter_count: usize,
    editor_width: u32,
    editor_height: u32,
    runtime_editor_descriptor: Option<ClapRuntimeEditorDescriptor>,
}

impl ClapCdylibScaffold {
    /// Creates a CLAP dynamic-library scaffold from plugin metadata.
    #[must_use]
    pub fn from_metadata(metadata: &FormatMetadata) -> Self {
        Self {
            entry: ClapPluginEntryPlan::from_metadata(metadata),
            package_name: "hawk2ui-generated-clap".into(),
            library_file_stem: "hawk2ui_generated_clap".into(),
            parameter_source: "&[]".into(),
            parameter_value_source: "[]".into(),
            parameter_count: 0,
            editor_width: 800,
            editor_height: 600,
            runtime_editor_descriptor: None,
        }
    }

    /// Configures the generated CLAP GUI extension from a plugin editor record.
    #[must_use]
    pub fn with_editor(mut self, editor: &PluginEditor) -> Self {
        let (width, height) = editor.initial_size.physical_size();
        self.editor_width = width.max(1);
        self.editor_height = height.max(1);
        self
    }

    /// Adds generated CLAP parameter metadata from the format-neutral parameter model.
    #[must_use]
    pub fn with_parameters(mut self, parameters: &ParameterModel) -> Self {
        self.parameter_source = clap_parameter_source(parameters);
        self.parameter_value_source = clap_parameter_value_source(parameters);
        self.parameter_count = parameters.parameters.len();
        self
    }

    fn with_parameter_sources(
        mut self,
        parameter_source: impl Into<String>,
        parameter_value_source: impl Into<String>,
        parameter_count: usize,
    ) -> Self {
        self.parameter_source = parameter_source.into();
        self.parameter_value_source = parameter_value_source.into();
        self.parameter_count = parameter_count;
        self
    }

    /// Embeds the generated `Hawk2UI` runtime editor descriptor exposed by the CLAP GUI library.
    #[must_use]
    pub fn with_runtime_editor_descriptor(
        mut self,
        descriptor: ClapRuntimeEditorDescriptor,
    ) -> Self {
        self.runtime_editor_descriptor = Some(descriptor);
        self
    }

    /// Writes the scaffold to a Cargo project directory.
    ///
    /// # Errors
    ///
    /// Returns [`PackageMaterializationError`] when project directories or source files cannot be
    /// created.
    pub fn write_to(
        &self,
        root: impl AsRef<Path>,
    ) -> Result<ClapCdylibScaffoldOutput, PackageMaterializationError> {
        let root = root.as_ref();
        let src_dir = root.join("src");
        create_package_dir(&src_dir)?;
        let cargo_toml_path = root.join("Cargo.toml");
        let lib_rs_path = src_dir.join("lib.rs");
        write_package_file(&cargo_toml_path, self.cargo_toml())?;
        write_package_file(&lib_rs_path, self.lib_rs())?;
        Ok(ClapCdylibScaffoldOutput {
            root_path: root.to_string_lossy().into_owned(),
            cargo_toml_path: cargo_toml_path.to_string_lossy().into_owned(),
            lib_rs_path: lib_rs_path.to_string_lossy().into_owned(),
            package_name: self.package_name.clone(),
            library_file_stem: self.library_file_stem.clone(),
        })
    }

    fn cargo_toml(&self) -> String {
        format!(
            "[package]\nname = {}\nversion = \"0.1.0\"\nedition = \"2024\"\npublish = false\n\n[lib]\nname = {}\ncrate-type = [\"cdylib\"]\n\n[dependencies]\nclap-sys = \"0.5.0\"\n",
            quoted_metadata_string(&self.package_name),
            quoted_metadata_string(&self.library_file_stem)
        )
    }

    fn lib_rs(&self) -> String {
        CLAP_CDYLIB_SOURCE_TEMPLATE
            .replace(
                "__PLUGIN_ID_BYTES__",
                &rust_nul_terminated_byte_string(self.entry.plugin_id()),
            )
            .replace(
                "__PLUGIN_NAME_BYTES__",
                &rust_nul_terminated_byte_string(&self.entry.name),
            )
            .replace(
                "__VENDOR_BYTES__",
                &rust_nul_terminated_byte_string(&self.entry.vendor),
            )
            .replace(
                "__VERSION_BYTES__",
                &rust_nul_terminated_byte_string(&self.entry.version),
            )
            .replace("__PARAMETERS__", &self.parameter_source)
            .replace("__PARAMETER_VALUES__", &self.parameter_value_source)
            .replace("__PARAMETER_COUNT__", &self.parameter_count.to_string())
            .replace("__EDITOR_WIDTH__", &self.editor_width.to_string())
            .replace("__EDITOR_HEIGHT__", &self.editor_height.to_string())
            .replace(
                "__EDITOR_DESCRIPTOR_BYTES__",
                &rust_byte_string(
                    &self
                        .runtime_editor_descriptor
                        .as_ref()
                        .map(ClapRuntimeEditorDescriptor::to_export_payload)
                        .unwrap_or_default(),
                ),
            )
    }
}

const CLAP_CDYLIB_SOURCE_TEMPLATE: &str = r#"//! Generated Hawk2UI CLAP entry library scaffold.

    use clap_sys::entry::clap_plugin_entry;
    use clap_sys::events::{
        clap_event_param_value, CLAP_CORE_EVENT_SPACE_ID, CLAP_EVENT_PARAM_VALUE,
    };
use clap_sys::ext::audio_ports::{
    clap_audio_port_info, clap_plugin_audio_ports, CLAP_AUDIO_PORT_IS_MAIN, CLAP_EXT_AUDIO_PORTS,
    CLAP_PORT_STEREO,
};
use clap_sys::ext::gui::{
    clap_gui_resize_hints, clap_plugin_gui, clap_window, CLAP_EXT_GUI, CLAP_WINDOW_API_COCOA,
    CLAP_WINDOW_API_WAYLAND, CLAP_WINDOW_API_WIN32, CLAP_WINDOW_API_X11,
};
use clap_sys::ext::params::{clap_param_info, clap_plugin_params, CLAP_EXT_PARAMS};
use clap_sys::ext::state::{clap_plugin_state, CLAP_EXT_STATE};
use clap_sys::factory::plugin_factory::clap_plugin_factory;
use clap_sys::plugin::{clap_plugin, clap_plugin_descriptor};
use clap_sys::process::{
    clap_process, clap_process_status, CLAP_PROCESS_CONTINUE, CLAP_PROCESS_ERROR,
};
use clap_sys::stream::{clap_istream, clap_ostream};
use clap_sys::string_sizes::{CLAP_NAME_SIZE, CLAP_PATH_SIZE};
use clap_sys::version::CLAP_VERSION;
    use std::ffi::{c_char, c_void, CStr};
    use std::fmt::Write as _;
    use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

static PLUGIN_ID_BYTES: &[u8] = __PLUGIN_ID_BYTES__;
static PLUGIN_NAME_BYTES: &[u8] = __PLUGIN_NAME_BYTES__;
static VENDOR_BYTES: &[u8] = __VENDOR_BYTES__;
static VERSION_BYTES: &[u8] = __VERSION_BYTES__;
static FACTORY_ID_BYTES: &[u8] = b"clap.plugin-factory\0";
static DESCRIPTION_BYTES: &[u8] = b"Generated by Hawk2UI\0";
static STATE_BYTES: &[u8] = b"hawk2ui-state-v1\n";
static EDITOR_DESCRIPTOR_BYTES: &[u8] = __EDITOR_DESCRIPTOR_BYTES__;
static EDITOR_HOST_ABI_BYTES: &[u8] = b"hawk2ui_host_bridge_abi=1\ncommand=create\ncommand=set_parent\ncommand=show\ncommand=hide\ncommand=destroy\ncommand=apply_parameter\ncommand=save_state\ncommand=load_state\ncommand=drain_realtime_visuals\nresponse=created\nresponse=parent_attached\nresponse=frame_presented\nresponse=hidden\nresponse=destroyed\nresponse=parameter_applied\nresponse=state_saved\nresponse=state_loaded\nresponse=realtime_visuals_drained\nfunction=hawk2ui_editor_dispatch\n";
static REALTIME_SAFETY_POLICY_BYTES: &[u8] = b"hawk2ui_realtime_safety_policy=1\ncontext=audio_thread\nprocess_callback=guarded_preallocated_copy\nallowed=preallocated_write\nforbidden=allocation\nforbidden=blocking_wait\nlock_policy=no_blocking_locks\n";
static EDITOR_CREATED: AtomicBool = AtomicBool::new(false);
  static EDITOR_ATTACHED: AtomicBool = AtomicBool::new(false);
  static EDITOR_VISIBLE: AtomicBool = AtomicBool::new(false);
  static EDITOR_WIDTH: AtomicU32 = AtomicU32::new(__EDITOR_WIDTH__);
  static EDITOR_HEIGHT: AtomicU32 = AtomicU32::new(__EDITOR_HEIGHT__);
  static EDITOR_PRESENTED_FRAME_COUNT: AtomicU64 = AtomicU64::new(0);

  #[repr(C)]
  pub struct Hawk2uiEditorState {
      pub created: bool,
      pub attached: bool,
      pub visible: bool,
      pub width: u32,
      pub height: u32,
  }

  #[derive(Clone, Copy, Debug, Eq, PartialEq)]
  enum Hawk2uiRealtimeOperation {
      Allocation,
      BlockingWait,
      PreallocatedWrite,
  }

  struct GeneratedParameter {
    id: u32,
    name: &'static [u8],
    module: &'static [u8],
    min_value: f64,
    max_value: f64,
    default_value: f64,
    flags: u32,
    }

    static PARAMETERS: &[GeneratedParameter] = __PARAMETERS__;
    static PARAMETER_VALUES: [AtomicU64; __PARAMETER_COUNT__] = __PARAMETER_VALUES__;

static DESCRIPTOR: clap_plugin_descriptor = clap_plugin_descriptor {
    clap_version: CLAP_VERSION,
    id: PLUGIN_ID_BYTES.as_ptr().cast(),
    name: PLUGIN_NAME_BYTES.as_ptr().cast(),
    vendor: VENDOR_BYTES.as_ptr().cast(),
    url: std::ptr::null(),
    manual_url: std::ptr::null(),
    support_url: std::ptr::null(),
    version: VERSION_BYTES.as_ptr().cast(),
    description: DESCRIPTION_BYTES.as_ptr().cast(),
    features: std::ptr::null(),
};

static PLUGIN: clap_plugin = clap_plugin {
    desc: &DESCRIPTOR,
    plugin_data: std::ptr::null_mut(),
    init: Some(plugin_init),
    destroy: Some(plugin_destroy),
    activate: Some(plugin_activate),
    deactivate: Some(plugin_deactivate),
    start_processing: Some(plugin_start_processing),
    stop_processing: Some(plugin_stop_processing),
    reset: Some(plugin_reset),
    process: Some(plugin_process),
    get_extension: Some(plugin_get_extension),
    on_main_thread: Some(plugin_on_main_thread),
};

static AUDIO_PORTS: clap_plugin_audio_ports = clap_plugin_audio_ports {
    count: Some(audio_ports_count),
    get: Some(audio_ports_get),
};

static PARAMS: clap_plugin_params = clap_plugin_params {
    count: Some(params_count),
    get_info: Some(params_get_info),
    get_value: Some(params_get_value),
    value_to_text: Some(params_value_to_text),
    text_to_value: Some(params_text_to_value),
    flush: Some(params_flush),
};

static STATE: clap_plugin_state = clap_plugin_state {
    save: Some(state_save),
    load: Some(state_load),
};

static GUI: clap_plugin_gui = clap_plugin_gui {
    is_api_supported: Some(gui_is_api_supported),
    get_preferred_api: Some(gui_get_preferred_api),
    create: Some(gui_create),
    destroy: Some(gui_destroy),
    set_scale: Some(gui_set_scale),
    get_size: Some(gui_get_size),
    can_resize: Some(gui_can_resize),
    get_resize_hints: Some(gui_get_resize_hints),
    adjust_size: Some(gui_adjust_size),
    set_size: Some(gui_set_size),
    set_parent: Some(gui_set_parent),
    set_transient: Some(gui_set_transient),
    suggest_title: Some(gui_suggest_title),
    show: Some(gui_show),
    hide: Some(gui_hide),
};

static PLUGIN_FACTORY: clap_plugin_factory = clap_plugin_factory {
    get_plugin_count: Some(get_plugin_count),
    get_plugin_descriptor: Some(get_plugin_descriptor),
    create_plugin: Some(create_plugin),
};

unsafe extern "C" fn entry_init(_plugin_path: *const c_char) -> bool {
    true
}

unsafe extern "C" fn entry_deinit() {}

unsafe extern "C" fn entry_get_factory(factory_id: *const c_char) -> *const c_void {
    if factory_id.is_null() {
        return std::ptr::null();
    }
    let expected = &FACTORY_ID_BYTES[..FACTORY_ID_BYTES.len().saturating_sub(1)];
    let requested = unsafe { CStr::from_ptr(factory_id) };
    if requested.to_bytes() == expected {
        (&PLUGIN_FACTORY as *const clap_plugin_factory).cast()
    } else {
        std::ptr::null()
    }
}

unsafe extern "C" fn get_plugin_count(_factory: *const clap_plugin_factory) -> u32 {
    1
}

unsafe extern "C" fn get_plugin_descriptor(
    _factory: *const clap_plugin_factory,
    index: u32,
) -> *const clap_plugin_descriptor {
    if index == 0 {
        &DESCRIPTOR
    } else {
        std::ptr::null()
    }
}

unsafe extern "C" fn create_plugin(
    _factory: *const clap_plugin_factory,
    _host: *const clap_sys::host::clap_host,
    plugin_id: *const c_char,
) -> *const clap_plugin {
    if plugin_id.is_null() {
        return std::ptr::null();
    }
    let expected = &PLUGIN_ID_BYTES[..PLUGIN_ID_BYTES.len().saturating_sub(1)];
    let requested = unsafe { CStr::from_ptr(plugin_id) };
    if requested.to_bytes() == expected {
        &PLUGIN
    } else {
        std::ptr::null()
    }
}

unsafe extern "C" fn plugin_init(_plugin: *const clap_plugin) -> bool {
    true
}

unsafe extern "C" fn plugin_destroy(_plugin: *const clap_plugin) {}

unsafe extern "C" fn plugin_activate(
    _plugin: *const clap_plugin,
    sample_rate: f64,
    min_frames_count: u32,
    max_frames_count: u32,
) -> bool {
    sample_rate.is_finite()
        && sample_rate > 0.0
        && max_frames_count > 0
        && min_frames_count <= max_frames_count
}

unsafe extern "C" fn plugin_deactivate(_plugin: *const clap_plugin) {}

unsafe extern "C" fn plugin_start_processing(_plugin: *const clap_plugin) -> bool {
    true
}

unsafe extern "C" fn plugin_stop_processing(_plugin: *const clap_plugin) {}

unsafe extern "C" fn plugin_reset(_plugin: *const clap_plugin) {}

unsafe extern "C" fn plugin_process(
    _plugin: *const clap_plugin,
    process: *const clap_process,
) -> clap_process_status {
    if process.is_null() {
        return CLAP_PROCESS_ERROR;
    }
    if !realtime_guard_allows(Hawk2uiRealtimeOperation::PreallocatedWrite)
        || realtime_guard_allows(Hawk2uiRealtimeOperation::Allocation)
        || realtime_guard_allows(Hawk2uiRealtimeOperation::BlockingWait)
    {
        return CLAP_PROCESS_ERROR;
    }

    let process = unsafe { &*process };
    copy_audio_inputs_to_outputs(process);
    CLAP_PROCESS_CONTINUE
}

fn realtime_guard_allows(operation: Hawk2uiRealtimeOperation) -> bool {
    matches!(operation, Hawk2uiRealtimeOperation::PreallocatedWrite)
}

fn copy_audio_inputs_to_outputs(process: &clap_process) {
    if process.frames_count == 0
        || process.audio_inputs.is_null()
        || process.audio_outputs.is_null()
        || process.audio_inputs_count == 0
        || process.audio_outputs_count == 0
    {
        return;
    }

    let port_count = process.audio_inputs_count.min(process.audio_outputs_count);
    for port_index in 0..port_count {
        let input = unsafe { &*process.audio_inputs.add(port_index as usize) };
        let output = unsafe { &mut *process.audio_outputs.add(port_index as usize) };
        let channel_count = input.channel_count.min(output.channel_count);
        copy_f32_channels(input.data32, output.data32, channel_count, process.frames_count);
        copy_f64_channels(input.data64, output.data64, channel_count, process.frames_count);
    }
}

fn copy_f32_channels(
    input_channels: *mut *mut f32,
    output_channels: *mut *mut f32,
    channel_count: u32,
    frames_count: u32,
) {
    if input_channels.is_null() || output_channels.is_null() {
        return;
    }

    for channel_index in 0..channel_count {
        let input = unsafe { *input_channels.add(channel_index as usize) };
        let output = unsafe { *output_channels.add(channel_index as usize) };
        if !input.is_null() && !output.is_null() {
            unsafe {
                std::ptr::copy_nonoverlapping(input, output, frames_count as usize);
            }
        }
    }
}

fn copy_f64_channels(
    input_channels: *mut *mut f64,
    output_channels: *mut *mut f64,
    channel_count: u32,
    frames_count: u32,
) {
    if input_channels.is_null() || output_channels.is_null() {
        return;
    }

    for channel_index in 0..channel_count {
        let input = unsafe { *input_channels.add(channel_index as usize) };
        let output = unsafe { *output_channels.add(channel_index as usize) };
        if !input.is_null() && !output.is_null() {
            unsafe {
                std::ptr::copy_nonoverlapping(input, output, frames_count as usize);
            }
        }
    }
}

unsafe extern "C" fn plugin_get_extension(
    _plugin: *const clap_plugin,
    id: *const c_char,
) -> *const c_void {
    if cstr_matches(id, CLAP_EXT_AUDIO_PORTS) {
        (&AUDIO_PORTS as *const clap_plugin_audio_ports).cast()
    } else if cstr_matches(id, CLAP_EXT_PARAMS) {
        (&PARAMS as *const clap_plugin_params).cast()
    } else if cstr_matches(id, CLAP_EXT_STATE) {
        (&STATE as *const clap_plugin_state).cast()
    } else if cstr_matches(id, CLAP_EXT_GUI) {
        (&GUI as *const clap_plugin_gui).cast()
    } else {
        std::ptr::null()
    }
}

unsafe extern "C" fn plugin_on_main_thread(_plugin: *const clap_plugin) {}

unsafe extern "C" fn audio_ports_count(_plugin: *const clap_plugin, _is_input: bool) -> u32 {
    1
}

unsafe extern "C" fn audio_ports_get(
    _plugin: *const clap_plugin,
    index: u32,
    is_input: bool,
    info: *mut clap_audio_port_info,
) -> bool {
    if index != 0 || info.is_null() {
        return false;
    }

    let mut name = [0; CLAP_NAME_SIZE];
    write_c_name(&mut name, if is_input { b"Input" } else { b"Output" });
    unsafe {
        *info = clap_audio_port_info {
            id: if is_input { 0 } else { 1 },
            name,
            flags: CLAP_AUDIO_PORT_IS_MAIN,
            channel_count: 2,
            port_type: CLAP_PORT_STEREO.as_ptr(),
            in_place_pair: if is_input { 1 } else { 0 },
        };
    }
    true
}

fn write_c_name<const N: usize>(target: &mut [c_char; N], source: &[u8]) {
    for (index, byte) in source.iter().take(N.saturating_sub(1)).enumerate() {
        target[index] = *byte as c_char;
    }
}

unsafe extern "C" fn params_count(_plugin: *const clap_plugin) -> u32 {
    PARAMETERS.len() as u32
}

unsafe extern "C" fn params_get_info(
    _plugin: *const clap_plugin,
    param_index: u32,
    param_info: *mut clap_param_info,
) -> bool {
    let Some(parameter) = PARAMETERS.get(param_index as usize) else {
        return false;
    };
    if param_info.is_null() {
        return false;
    }

    let mut name = [0; CLAP_NAME_SIZE];
    let mut module = [0; CLAP_PATH_SIZE];
    write_c_name(&mut name, parameter.name.strip_suffix(&[0]).unwrap_or(parameter.name));
    write_c_name(
        &mut module,
        parameter
            .module
            .strip_suffix(&[0])
            .unwrap_or(parameter.module),
    );
    unsafe {
        *param_info = clap_param_info {
            id: parameter.id,
            flags: parameter.flags,
            cookie: std::ptr::null_mut(),
            name,
            module,
            min_value: parameter.min_value,
            max_value: parameter.max_value,
            default_value: parameter.default_value,
        };
    }
    true
}

    unsafe extern "C" fn params_get_value(
        _plugin: *const clap_plugin,
        param_id: u32,
        out_value: *mut f64,
    ) -> bool {
        let Some(index) = find_parameter_index(param_id) else {
            return false;
        };
        if out_value.is_null() {
            return false;
        }

        unsafe {
            *out_value = f64::from_bits(PARAMETER_VALUES[index].load(Ordering::Acquire));
        }
        true
    }

unsafe extern "C" fn params_value_to_text(
    _plugin: *const clap_plugin,
    param_id: u32,
    value: f64,
    out_buffer: *mut c_char,
    out_buffer_capacity: u32,
) -> bool {
    if find_parameter(param_id).is_none() || out_buffer.is_null() || out_buffer_capacity == 0 {
        return false;
    }
    write_c_buffer(out_buffer, out_buffer_capacity, format!("{value:.6}").as_bytes());
    true
}

unsafe extern "C" fn params_text_to_value(
    _plugin: *const clap_plugin,
    param_id: u32,
    param_value_text: *const c_char,
    out_value: *mut f64,
) -> bool {
    let Some(parameter) = find_parameter(param_id) else {
        return false;
    };
    if param_value_text.is_null() || out_value.is_null() {
        return false;
    }

    let Ok(text) = unsafe { CStr::from_ptr(param_value_text) }.to_str() else {
        return false;
    };
    let Ok(value) = text.trim().parse::<f64>() else {
        return false;
    };
    if !value.is_finite() {
        return false;
    }
    unsafe {
        *out_value = value.clamp(parameter.min_value, parameter.max_value);
    }
    true
}

    unsafe extern "C" fn params_flush(
        _plugin: *const clap_plugin,
        in_: *const clap_sys::events::clap_input_events,
        _out: *const clap_sys::events::clap_output_events,
    ) {
        if in_.is_null() {
            return;
        }
        let input = unsafe { &*in_ };
        let Some(size) = input.size else {
            return;
        };
        let Some(get) = input.get else {
            return;
        };
        let event_count = unsafe { size(input) };
        for index in 0..event_count {
            let header = unsafe { get(input, index) };
            if header.is_null() {
                continue;
            }
            let header = unsafe { &*header };
            if header.space_id != CLAP_CORE_EVENT_SPACE_ID
                || header.type_ != CLAP_EVENT_PARAM_VALUE
                || header.size < std::mem::size_of::<clap_event_param_value>() as u32
            {
                continue;
            }
            let event = unsafe {
                &*(header as *const clap_sys::events::clap_event_header)
                    .cast::<clap_event_param_value>()
            };
            let _ = store_parameter_value(event.param_id, event.value);
        }
    }

    fn find_parameter(param_id: u32) -> Option<&'static GeneratedParameter> {
        find_parameter_index(param_id).map(|index| &PARAMETERS[index])
    }

    fn find_parameter_index(param_id: u32) -> Option<usize> {
        PARAMETERS
            .iter()
            .position(|parameter| parameter.id == param_id)
    }

    fn store_parameter_value(param_id: u32, value: f64) -> bool {
        if !value.is_finite() {
            return false;
        }
        let Some(index) = find_parameter_index(param_id) else {
            return false;
        };
        let parameter = &PARAMETERS[index];
        PARAMETER_VALUES[index].store(
            value.clamp(parameter.min_value, parameter.max_value).to_bits(),
            Ordering::Release,
        );
        true
    }

fn write_c_buffer(out_buffer: *mut c_char, out_buffer_capacity: u32, source: &[u8]) {
    let write_len = source
        .len()
        .min(out_buffer_capacity.saturating_sub(1) as usize);
    for (index, byte) in source.iter().take(write_len).enumerate() {
        unsafe {
            *out_buffer.add(index) = *byte as c_char;
        }
    }
    unsafe {
        *out_buffer.add(write_len) = 0;
    }
}

    unsafe extern "C" fn state_save(_plugin: *const clap_plugin, stream: *const clap_ostream) -> bool {
        if stream.is_null() {
            return false;
        }
        let stream = unsafe { &*stream };
        let Some(write) = stream.write else {
            return false;
        };
        let state = state_payload();
        let written = unsafe { write(stream, state.as_ptr().cast(), state.len() as u64) };
        written == state.len() as i64
    }

unsafe extern "C" fn state_load(_plugin: *const clap_plugin, stream: *const clap_istream) -> bool {
    if stream.is_null() {
        return false;
    }
    let stream = unsafe { &*stream };
    let Some(read) = stream.read else {
        return false;
    };
        let Some(payload) = read_state_payload(stream, read) else {
            return false;
        };
        restore_state_payload(&payload)
    }

    fn state_payload() -> Vec<u8> {
        let mut state = String::from(std::str::from_utf8(STATE_BYTES).unwrap_or("hawk2ui-state-v1\n"));
        for (index, parameter) in PARAMETERS.iter().enumerate() {
            let bits = PARAMETER_VALUES[index].load(Ordering::Acquire);
            let _ = writeln!(state, "param {} {}", parameter.id, bits);
        }
        state.into_bytes()
    }

    fn read_state_payload(
        stream: &clap_istream,
        read: unsafe extern "C" fn(*const clap_istream, *mut c_void, u64) -> i64,
    ) -> Option<Vec<u8>> {
        const MAX_STATE_BYTES: usize = 64 * 1024;
        let mut payload = Vec::new();
        let mut buffer = [0_u8; 1024];
        loop {
            let read_len = unsafe { read(stream, buffer.as_mut_ptr().cast(), buffer.len() as u64) };
            if read_len < 0 {
                return None;
            }
            if read_len == 0 {
                break;
            }
            let read_len = read_len as usize;
            if payload.len().saturating_add(read_len) > MAX_STATE_BYTES {
                return None;
            }
            payload.extend_from_slice(&buffer[..read_len]);
        }
        Some(payload)
    }

    fn restore_state_payload(payload: &[u8]) -> bool {
        let Ok(state) = std::str::from_utf8(payload) else {
            return false;
        };
        if !state.starts_with("hawk2ui-state-v1\n") {
            return false;
        }
        for line in state.lines().skip(1) {
            let mut parts = line.split_ascii_whitespace();
            if parts.next() != Some("param") {
                continue;
            }
            let Some(param_id) = parts.next().and_then(|value| value.parse::<u32>().ok()) else {
                return false;
            };
            let Some(bits) = parts.next().and_then(|value| value.parse::<u64>().ok()) else {
                return false;
            };
            let value = f64::from_bits(bits);
            if !store_parameter_value(param_id, value) {
                return false;
            }
        }
        true
    }

unsafe extern "C" fn gui_is_api_supported(
    _plugin: *const clap_plugin,
    api: *const c_char,
    is_floating: bool,
) -> bool {
    !is_floating && is_supported_window_api(api)
}

unsafe extern "C" fn gui_get_preferred_api(
    _plugin: *const clap_plugin,
    api: *mut *const c_char,
    is_floating: *mut bool,
) -> bool {
    if api.is_null() || is_floating.is_null() {
        return false;
    }
    unsafe {
        *api = preferred_window_api();
        *is_floating = false;
    }
    true
}

unsafe extern "C" fn gui_create(
    _plugin: *const clap_plugin,
    api: *const c_char,
    is_floating: bool,
) -> bool {
    if is_floating || !is_supported_window_api(api) {
        return false;
    }
    EDITOR_CREATED.store(true, Ordering::Release);
    EDITOR_ATTACHED.store(false, Ordering::Release);
    EDITOR_VISIBLE.store(false, Ordering::Release);
    true
}

unsafe extern "C" fn gui_destroy(_plugin: *const clap_plugin) {
    EDITOR_CREATED.store(false, Ordering::Release);
    EDITOR_ATTACHED.store(false, Ordering::Release);
    EDITOR_VISIBLE.store(false, Ordering::Release);
}

unsafe extern "C" fn gui_set_scale(_plugin: *const clap_plugin, scale: f64) -> bool {
    scale.is_finite() && scale > 0.0
}

unsafe extern "C" fn gui_get_size(
    _plugin: *const clap_plugin,
    width: *mut u32,
    height: *mut u32,
) -> bool {
    if width.is_null() || height.is_null() {
        return false;
    }
    unsafe {
        *width = EDITOR_WIDTH.load(Ordering::Acquire);
        *height = EDITOR_HEIGHT.load(Ordering::Acquire);
    }
    true
}

unsafe extern "C" fn gui_can_resize(_plugin: *const clap_plugin) -> bool {
    true
}

unsafe extern "C" fn gui_get_resize_hints(
    _plugin: *const clap_plugin,
    hints: *mut clap_gui_resize_hints,
) -> bool {
    if hints.is_null() {
        return false;
    }
    unsafe {
        *hints = clap_gui_resize_hints {
            can_resize_horizontally: true,
            can_resize_vertically: true,
            preserve_aspect_ratio: false,
            aspect_ratio_width: 0,
            aspect_ratio_height: 0,
        };
    }
    true
}

unsafe extern "C" fn gui_adjust_size(
    _plugin: *const clap_plugin,
    width: *mut u32,
    height: *mut u32,
) -> bool {
    if width.is_null() || height.is_null() {
        return false;
    }
    unsafe {
        *width = (*width).max(1);
        *height = (*height).max(1);
    }
    true
}

unsafe extern "C" fn gui_set_size(_plugin: *const clap_plugin, width: u32, height: u32) -> bool {
    if width == 0 || height == 0 {
        return false;
    }
    EDITOR_WIDTH.store(width, Ordering::Release);
    EDITOR_HEIGHT.store(height, Ordering::Release);
    true
}

unsafe extern "C" fn gui_set_parent(
    _plugin: *const clap_plugin,
    window: *const clap_window,
) -> bool {
      if window.is_null() {
          return false;
      }
      let window = unsafe { &*window };
      let has_parent_handle = unsafe { !window.specific.ptr.is_null() };
      if !EDITOR_CREATED.load(Ordering::Acquire)
          || !is_supported_window_api(window.api)
          || !has_parent_handle
      {
          return false;
      }
      EDITOR_ATTACHED.store(true, Ordering::Release);
    true
}

unsafe extern "C" fn gui_set_transient(
    _plugin: *const clap_plugin,
    window: *const clap_window,
) -> bool {
    !window.is_null()
}

unsafe extern "C" fn gui_suggest_title(_plugin: *const clap_plugin, _title: *const c_char) {}

unsafe extern "C" fn gui_show(_plugin: *const clap_plugin) -> bool {
    let can_show = EDITOR_CREATED.load(Ordering::Acquire) && EDITOR_ATTACHED.load(Ordering::Acquire);
    if can_show {
        EDITOR_VISIBLE.store(true, Ordering::Release);
    }
    can_show
}

unsafe extern "C" fn gui_hide(_plugin: *const clap_plugin) -> bool {
    let was_created = EDITOR_CREATED.load(Ordering::Acquire);
    if was_created {
        EDITOR_VISIBLE.store(false, Ordering::Release);
    }
    was_created
}

    fn is_supported_window_api(api: *const c_char) -> bool {
        if descriptor_declares_baseview_host_adapter() {
            return cstr_matches(api, CLAP_WINDOW_API_X11)
                || cstr_matches(api, CLAP_WINDOW_API_COCOA)
                || cstr_matches(api, CLAP_WINDOW_API_WIN32);
        }
        cstr_matches(api, CLAP_WINDOW_API_WAYLAND)
            || cstr_matches(api, CLAP_WINDOW_API_X11)
            || cstr_matches(api, CLAP_WINDOW_API_COCOA)
            || cstr_matches(api, CLAP_WINDOW_API_WIN32)
    }

    fn cstr_matches(value: *const c_char, expected: &CStr) -> bool {
        !value.is_null() && unsafe { CStr::from_ptr(value) }.to_bytes() == expected.to_bytes()
    }

    fn descriptor_declares_baseview_host_adapter() -> bool {
        const HOST_ADAPTER_BASEVIEW: &[u8] = b"host_adapter=baseview";
        EDITOR_DESCRIPTOR_BYTES
            .windows(HOST_ADAPTER_BASEVIEW.len())
            .any(|window| window == HOST_ADAPTER_BASEVIEW)
    }

    fn preferred_window_api() -> *const c_char {
        #[cfg(target_os = "linux")]
        {
            if descriptor_declares_baseview_host_adapter() {
                CLAP_WINDOW_API_X11.as_ptr()
            } else {
                CLAP_WINDOW_API_WAYLAND.as_ptr()
            }
        }
    #[cfg(target_os = "macos")]
    {
        CLAP_WINDOW_API_COCOA.as_ptr()
    }
    #[cfg(target_os = "windows")]
    {
        CLAP_WINDOW_API_WIN32.as_ptr()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        CLAP_WINDOW_API_X11.as_ptr()
    }
}

#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)]
pub static clap_entry: clap_plugin_entry = clap_plugin_entry {
    clap_version: CLAP_VERSION,
    init: Some(entry_init),
    deinit: Some(entry_deinit),
    get_factory: Some(entry_get_factory),
};

#[unsafe(no_mangle)]
  pub unsafe extern "C" fn hawk2ui_editor_descriptor(len: *mut usize) -> *const u8 {
      if !len.is_null() {
          unsafe {
              *len = EDITOR_DESCRIPTOR_BYTES.len();
          }
      }
      EDITOR_DESCRIPTOR_BYTES.as_ptr()
  }

  #[unsafe(no_mangle)]
  pub unsafe extern "C" fn hawk2ui_editor_host_abi(len: *mut usize) -> *const u8 {
      if !len.is_null() {
          unsafe {
              *len = EDITOR_HOST_ABI_BYTES.len();
          }
      }
      EDITOR_HOST_ABI_BYTES.as_ptr()
  }

  #[unsafe(no_mangle)]
  pub unsafe extern "C" fn hawk2ui_realtime_safety_policy(len: *mut usize) -> *const u8 {
      if !len.is_null() {
          unsafe {
              *len = REALTIME_SAFETY_POLICY_BYTES.len();
          }
      }
      REALTIME_SAFETY_POLICY_BYTES.as_ptr()
  }

  #[unsafe(no_mangle)]
  pub unsafe extern "C" fn hawk2ui_editor_dispatch(
      command_ptr: *const u8,
      command_len: usize,
      response_ptr: *mut u8,
      response_capacity: usize,
      response_len: *mut usize,
  ) -> bool {
      if command_ptr.is_null() || response_len.is_null() {
          return false;
      }
      let command_bytes = unsafe { std::slice::from_raw_parts(command_ptr, command_len) };
      let Ok(command) = std::str::from_utf8(command_bytes) else {
          return write_dispatch_response(response_ptr, response_capacity, response_len, "error=invalid-utf8\n");
      };
      let Some(command_name) = dispatch_field(command, "command") else {
          return write_dispatch_response(response_ptr, response_capacity, response_len, "error=missing-command\n");
      };
      let response = match command_name {
          "create" => dispatch_create(command),
          "set_parent" => dispatch_set_parent(command),
          "show" => dispatch_show(),
          "hide" => dispatch_hide(),
          "destroy" => dispatch_destroy(),
          "apply_parameter" => dispatch_apply_parameter(command),
          "save_state" => dispatch_save_state(),
          "load_state" => dispatch_load_state(command),
          "drain_realtime_visuals" => dispatch_drain_realtime_visuals(command),
          _ => "error=unknown-command\n".to_owned(),
      };
      write_dispatch_response(response_ptr, response_capacity, response_len, &response)
  }

  fn dispatch_create(command: &str) -> String {
      if let Some(api) = dispatch_field(command, "api") {
          if !is_supported_window_api_name(api) {
              return "error=unsupported-api\n".to_owned();
          }
      }
      EDITOR_CREATED.store(true, Ordering::Release);
      EDITOR_ATTACHED.store(false, Ordering::Release);
      EDITOR_VISIBLE.store(false, Ordering::Release);
      EDITOR_PRESENTED_FRAME_COUNT.store(0, Ordering::Release);
      "response=created\n".to_owned()
  }

  fn dispatch_set_parent(command: &str) -> String {
      if !EDITOR_CREATED.load(Ordering::Acquire) {
          return "error=editor-not-created\n".to_owned();
      }
      let Some(api) = dispatch_field(command, "api") else {
          return "error=missing-api\n".to_owned();
      };
      if !is_supported_window_api_name(api) {
          return "error=unsupported-api\n".to_owned();
      }
      let Some(parent) = dispatch_field(command, "parent") else {
          return "error=missing-parent\n".to_owned();
      };
      let Ok(parent_handle) = parent.parse::<usize>() else {
          return "error=invalid-parent\n".to_owned();
      };
      if parent_handle == 0 {
          return "error=invalid-parent\n".to_owned();
      }
      EDITOR_ATTACHED.store(true, Ordering::Release);
      "response=parent_attached\n".to_owned()
  }

  fn dispatch_show() -> String {
      if !EDITOR_CREATED.load(Ordering::Acquire) {
          return "error=editor-not-created\n".to_owned();
      }
      if !EDITOR_ATTACHED.load(Ordering::Acquire) {
          return "error=editor-not-attached\n".to_owned();
      }
      EDITOR_VISIBLE.store(true, Ordering::Release);
      let presented_frame_count = EDITOR_PRESENTED_FRAME_COUNT.fetch_add(1, Ordering::AcqRel) + 1;
      format!(
          "response=frame_presented\nwidth={}\nheight={}\npresented_frame_count={presented_frame_count}\n",
          EDITOR_WIDTH.load(Ordering::Acquire),
          EDITOR_HEIGHT.load(Ordering::Acquire)
      )
  }

  fn dispatch_hide() -> String {
      if !EDITOR_CREATED.load(Ordering::Acquire) {
          return "error=editor-not-created\n".to_owned();
      }
      EDITOR_VISIBLE.store(false, Ordering::Release);
      "response=hidden\n".to_owned()
  }

  fn dispatch_destroy() -> String {
      EDITOR_CREATED.store(false, Ordering::Release);
      EDITOR_ATTACHED.store(false, Ordering::Release);
      EDITOR_VISIBLE.store(false, Ordering::Release);
      "response=destroyed\n".to_owned()
  }

  fn dispatch_apply_parameter(command: &str) -> String {
      let Some(parameter_id) = dispatch_field(command, "parameter_id")
          .and_then(|value| value.parse::<u32>().ok())
      else {
          return "error=invalid-parameter-id\n".to_owned();
      };
      let Some(value) = dispatch_field(command, "value").and_then(|value| value.parse::<f64>().ok())
      else {
          return "error=invalid-parameter-value\n".to_owned();
      };
      if !store_parameter_value(parameter_id, value) {
          return "error=invalid-parameter-value\n".to_owned();
      }
      let Some(index) = find_parameter_index(parameter_id) else {
          return "error=invalid-parameter-id\n".to_owned();
      };
      let stored_value = f64::from_bits(PARAMETER_VALUES[index].load(Ordering::Acquire));
      format!("response=parameter_applied\nparameter_id={parameter_id}\nvalue={stored_value}\n")
  }

  fn dispatch_save_state() -> String {
      let mut response = String::from("response=state_saved\n");
      for (index, parameter) in PARAMETERS.iter().enumerate() {
          let bits = PARAMETER_VALUES[index].load(Ordering::Acquire);
          let _ = writeln!(response, "param.{}.bits={bits}", parameter.id);
      }
      response
  }

  fn dispatch_load_state(command: &str) -> String {
      let mut loaded_count = 0usize;
      for parameter in PARAMETERS {
          let key = format!("param.{}.bits", parameter.id);
          if let Some(bits) = dispatch_field(command, &key).and_then(|value| value.parse::<u64>().ok()) {
              if store_parameter_value(parameter.id, f64::from_bits(bits)) {
                  loaded_count += 1;
              }
          }
      }
      if loaded_count == 0 && !PARAMETERS.is_empty() {
          return "error=no-state-values\n".to_owned();
      }
      format!("response=state_loaded\nparameter_count={loaded_count}\n")
  }

  fn dispatch_drain_realtime_visuals(command: &str) -> String {
      let packet_count = dispatch_field(command, "packet_count")
          .and_then(|value| value.parse::<usize>().ok())
          .unwrap_or(0);
      format!("response=realtime_visuals_drained\npacket_count={packet_count}\n")
  }

  fn dispatch_field<'a>(command: &'a str, key: &str) -> Option<&'a str> {
      command.lines().find_map(|line| {
          let (field, value) = line.split_once('=')?;
          (field == key).then(|| value.trim())
      })
  }

  fn is_supported_window_api_name(api: &str) -> bool {
      if descriptor_declares_baseview_host_adapter() {
          return api == "x11" || api == "cocoa" || api == "win32";
      }
      api == "wayland" || api == "x11" || api == "cocoa" || api == "win32"
  }

  fn write_dispatch_response(
      response_ptr: *mut u8,
      response_capacity: usize,
      response_len: *mut usize,
      response: &str,
  ) -> bool {
      unsafe {
          *response_len = response.len();
      }
      if response.len() > response_capacity || (!response.is_empty() && response_ptr.is_null()) {
          return false;
      }
      unsafe {
          std::ptr::copy_nonoverlapping(response.as_ptr(), response_ptr, response.len());
      }
      true
  }

  #[unsafe(no_mangle)]
  pub unsafe extern "C" fn hawk2ui_editor_state() -> Hawk2uiEditorState {
      Hawk2uiEditorState {
          created: EDITOR_CREATED.load(Ordering::Acquire),
          attached: EDITOR_ATTACHED.load(Ordering::Acquire),
          visible: EDITOR_VISIBLE.load(Ordering::Acquire),
          width: EDITOR_WIDTH.load(Ordering::Acquire),
          height: EDITOR_HEIGHT.load(Ordering::Acquire),
      }
  }
"#;

/// Files written for a generated CLAP `cdylib` scaffold.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClapCdylibScaffoldOutput {
    /// Root project path.
    pub root_path: String,
    /// Generated `Cargo.toml` path.
    pub cargo_toml_path: String,
    /// Generated `src/lib.rs` path.
    pub lib_rs_path: String,
    /// Generated Cargo package name.
    pub package_name: String,
    /// Generated dynamic library file stem.
    pub library_file_stem: String,
}

/// Files written for a generated VST3 `cdylib` scaffold.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Vst3CdylibScaffoldOutput {
    /// Root project path.
    pub root_path: String,
    /// Generated `Cargo.toml` path.
    pub cargo_toml_path: String,
    /// Generated `src/lib.rs` path.
    pub lib_rs_path: String,
    /// Generated Cargo package name.
    pub package_name: String,
    /// Generated dynamic library file stem.
    pub library_file_stem: String,
}

struct Vst3CdylibScaffold {
    metadata: FormatMetadata,
    package_name: String,
    library_file_stem: String,
    processor_class_id: String,
    controller_class_id: String,
}

impl Vst3CdylibScaffold {
    fn from_metadata(metadata: &FormatMetadata) -> Self {
        Self {
            metadata: metadata.clone(),
            package_name: "hawk2ui-generated-vst3".into(),
            library_file_stem: "hawk2ui_generated_vst3".into(),
            processor_class_id: vst3_class_id_hex(&metadata.id, "processor"),
            controller_class_id: vst3_class_id_hex(&metadata.id, "controller"),
        }
    }

    fn write_to(
        &self,
        root: impl AsRef<Path>,
    ) -> Result<Vst3CdylibScaffoldOutput, PackageMaterializationError> {
        let root = root.as_ref();
        let src_dir = root.join("src");
        create_package_dir(&src_dir)?;
        let cargo_toml_path = root.join("Cargo.toml");
        let lib_rs_path = src_dir.join("lib.rs");
        write_package_file(&cargo_toml_path, self.cargo_toml())?;
        write_package_file(&lib_rs_path, self.lib_rs())?;
        Ok(Vst3CdylibScaffoldOutput {
            root_path: root.to_string_lossy().into_owned(),
            cargo_toml_path: cargo_toml_path.to_string_lossy().into_owned(),
            lib_rs_path: lib_rs_path.to_string_lossy().into_owned(),
            package_name: self.package_name.clone(),
            library_file_stem: self.library_file_stem.clone(),
        })
    }

    fn cargo_toml(&self) -> String {
        format!(
            "[package]\nname = {}\nversion = \"0.1.0\"\nedition = \"2024\"\npublish = false\n\n[lib]\nname = {}\ncrate-type = [\"cdylib\"]\n\n[dependencies]\nhawk2ui-vst3 = \"0.1.0\"\nvst3 = \"0.3.0\"\n",
            quoted_metadata_string(&self.package_name),
            quoted_metadata_string(&self.library_file_stem)
        )
    }

    fn lib_rs(&self) -> String {
        VST3_CDYLIB_SOURCE_TEMPLATE
            .replace("__PLUGIN_ID__", &escaped_metadata_string(&self.metadata.id))
            .replace(
                "__PLUGIN_NAME__",
                &escaped_metadata_string(&self.metadata.display_name),
            )
            .replace(
                "__VENDOR__",
                &escaped_metadata_string(&self.metadata.vendor),
            )
            .replace(
                "__VERSION__",
                &escaped_metadata_string(&self.metadata.version),
            )
            .replace("__PROCESSOR_CLASS_ID__", &self.processor_class_id)
            .replace("__CONTROLLER_CLASS_ID__", &self.controller_class_id)
    }
}

const VST3_CDYLIB_SOURCE_TEMPLATE: &str = r#"//! Generated Hawk2UI VST3 entry library scaffold.
#![allow(non_snake_case)]
#![allow(unsafe_code)]

use std::ffi::c_void;

use hawk2ui_vst3::{
    Vst3ClassCategory, Vst3ClassId, Vst3FactoryInfo, Vst3PluginClassInfo, Vst3String,
};
use vst3::Steinberg::Vst::IPluginFactory;

const PLUGIN_ID: &str = "__PLUGIN_ID__";
const PLUGIN_NAME: &str = "__PLUGIN_NAME__";
const VENDOR: &str = "__VENDOR__";
const VERSION: &str = "__VERSION__";
const PROCESSOR_CLASS_ID_HEX: &str = "__PROCESSOR_CLASS_ID__";
const CONTROLLER_CLASS_ID_HEX: &str = "__CONTROLLER_CLASS_ID__";

fn factory_info() -> Option<Vst3FactoryInfo> {
    let Ok(vendor) = Vst3String::new(VENDOR) else {
        return None;
    };
    Some(Vst3FactoryInfo::new(vendor, None, None))
}

fn processor_class_info() -> Option<Vst3PluginClassInfo> {
    let Ok(class_id) = Vst3ClassId::from_hex(PROCESSOR_CLASS_ID_HEX) else {
        return None;
    };
    let Ok(name) = Vst3String::new(PLUGIN_NAME) else {
        return None;
    };
    Vst3PluginClassInfo::new(class_id, Vst3ClassCategory::AudioModule, name).ok()
}

fn controller_class_info() -> Option<Vst3PluginClassInfo> {
    let Ok(class_id) = Vst3ClassId::from_hex(CONTROLLER_CLASS_ID_HEX) else {
        return None;
    };
    let Ok(name) = Vst3String::new(PLUGIN_NAME) else {
        return None;
    };
    Vst3PluginClassInfo::new(class_id, Vst3ClassCategory::ComponentController, name).ok()
}

#[cfg(target_os = "windows")]
#[unsafe(no_mangle)]
extern "system" fn InitDll() -> bool {
    let _ = (PLUGIN_ID, VERSION, factory_info(), processor_class_info(), controller_class_info());
    true
}

#[cfg(target_os = "windows")]
#[unsafe(no_mangle)]
extern "system" fn ExitDll() -> bool {
    true
}

#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
extern "system" fn BundleEntry(_bundle_ref: *mut c_void) -> bool {
    let _ = (PLUGIN_ID, VERSION, factory_info(), processor_class_info(), controller_class_info());
    true
}

#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
extern "system" fn BundleExit() -> bool {
    true
}

#[cfg(target_os = "linux")]
#[unsafe(no_mangle)]
extern "system" fn ModuleEntry(_library_handle: *mut c_void) -> bool {
    let _ = (PLUGIN_ID, VERSION, factory_info(), processor_class_info(), controller_class_info());
    true
}

#[cfg(target_os = "linux")]
#[unsafe(no_mangle)]
extern "system" fn ModuleExit() -> bool {
    true
}

#[unsafe(no_mangle)]
extern "system" fn GetPluginFactory() -> *mut IPluginFactory {
    std::ptr::null_mut()
}
"#;

/// Package request.
#[derive(Clone, Debug, PartialEq)]
pub struct PackageRequest {
    metadata: FormatMetadata,
    output: BundleOutput,
    parameters: ParameterModel,
    formats: Vec<PackageFormat>,
    runtime_artifact: Option<serde_json::Value>,
    editor: Option<PluginEditor>,
}

impl PackageRequest {
    /// Creates a package request.
    #[must_use]
    pub const fn new(
        metadata: FormatMetadata,
        output: BundleOutput,
        parameters: ParameterModel,
    ) -> Self {
        Self {
            metadata,
            output,
            parameters,
            formats: Vec::new(),
            runtime_artifact: None,
            editor: None,
        }
    }

    /// Configures the plugin editor surface metadata used by runtime-backed package outputs.
    #[must_use]
    pub fn with_editor(mut self, editor: PluginEditor) -> Self {
        self.editor = Some(editor);
        self
    }

    /// Attaches a sealed `Hawk2UI` runtime artifact payload to every materialized package target.
    #[must_use]
    pub fn with_runtime_artifact(mut self, runtime_artifact: serde_json::Value) -> Self {
        self.runtime_artifact = Some(runtime_artifact);
        self
    }

    /// Adds a package format.
    #[must_use]
    pub fn with_format(mut self, format: PackageFormat) -> Self {
        if !self.formats.contains(&format) {
            self.formats.push(format);
        }
        self
    }
}

/// Planned package target.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PackageTargetPlan {
    format: PackageFormat,
    metadata: FormatMetadata,
    output_path: String,
    parameter_count: usize,
    runtime_artifact: Option<serde_json::Value>,
    editor: PluginEditor,
    #[serde(skip)]
    #[schemars(skip)]
    clap_parameter_source: String,
    #[serde(skip)]
    #[schemars(skip)]
    clap_parameter_value_source: String,
}

impl PackageTargetPlan {
    /// Returns target format.
    #[must_use]
    pub const fn format(&self) -> PackageFormat {
        self.format
    }

    /// Returns output path.
    #[must_use]
    pub fn output_path(&self) -> &str {
        &self.output_path
    }

    /// Returns metadata.
    #[must_use]
    pub const fn metadata(&self) -> &FormatMetadata {
        &self.metadata
    }

    fn materialize(&self) -> Result<MaterializedPackageOutput, PackageMaterializationError> {
        let output_path = Path::new(&self.output_path);
        if output_path.exists() {
            fs::remove_dir_all(output_path).map_err(|error| {
                materialization_error(
                    "package.output.clean-failed",
                    format!(
                        "failed to clean package output {}: {error}",
                        self.output_path
                    ),
                )
            })?;
        }
        fs::create_dir_all(output_path).map_err(|error| {
            materialization_error(
                "package.output.create-failed",
                format!(
                    "failed to create package output {}: {error}",
                    self.output_path
                ),
            )
        })?;
        let resources_path = output_path.join("Contents").join("Resources");
        fs::create_dir_all(&resources_path).map_err(|error| {
            materialization_error(
                "package.resources.create-failed",
                format!(
                    "failed to create package resources {}: {error}",
                    resources_path.display()
                ),
            )
        })?;
        let mut package_files = Vec::new();
        let manifest_path = output_path.join("hawk2ui-package.toml");
        fs::write(&manifest_path, self.manifest()).map_err(|error| {
            materialization_error(
                "package.output.write-failed",
                format!(
                    "failed to write package metadata {}: {error}",
                    manifest_path.display()
                ),
            )
        })?;
        package_files.push(manifest_path.clone());
        let artifact_descriptor_path = resources_path.join("hawk2ui-artifact.toml");
        fs::write(&artifact_descriptor_path, self.artifact_descriptor()).map_err(|error| {
            materialization_error(
                "package.artifact.write-failed",
                format!(
                    "failed to write package artifact descriptor {}: {error}",
                    artifact_descriptor_path.display()
                ),
            )
        })?;
        package_files.push(artifact_descriptor_path.clone());
        if let Some(runtime_artifact) = &self.runtime_artifact {
            let runtime_artifact_path = resources_path.join("hawk2ui-runtime-artifact.json");
            let payload = serde_json::to_string_pretty(runtime_artifact).map_err(|error| {
                materialization_error(
                    "package.runtime-artifact.encode-failed",
                    format!("failed to encode runtime artifact payload: {error}"),
                )
            })?;
            write_package_file(&runtime_artifact_path, payload)?;
            package_files.push(runtime_artifact_path);
            let editor_descriptor_path = resources_path.join("hawk2ui-editor.toml");
            write_package_file(&editor_descriptor_path, self.editor_descriptor())?;
            package_files.push(editor_descriptor_path);
        }
        package_files.extend(self.write_format_layout(output_path, &resources_path)?);
        let hash_manifest_path = resources_path.join("hawk2ui-hashes.toml");
        fs::write(
            &hash_manifest_path,
            hash_manifest(output_path, &package_files)?,
        )
        .map_err(|error| {
            materialization_error(
                "package.hashes.write-failed",
                format!(
                    "failed to write package hash manifest {}: {error}",
                    hash_manifest_path.display()
                ),
            )
        })?;
        Ok(MaterializedPackageOutput {
            format: self.format,
            output_path: self.output_path.clone(),
            manifest_path: manifest_path.to_string_lossy().into_owned(),
            artifact_descriptor_path: artifact_descriptor_path.to_string_lossy().into_owned(),
            hash_manifest_path: hash_manifest_path.to_string_lossy().into_owned(),
        })
    }

    fn manifest(&self) -> String {
        let features = self
            .metadata
            .features
            .iter()
            .map(|feature| quoted_metadata_string(feature))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "format = {}\nid = {}\ndisplay_name = {}\nvendor = {}\nversion = {}\ncategory = {}\nfeatures = [{}]\nparameter_count = {}\n",
            quoted_metadata_string(self.format.manifest_key()),
            quoted_metadata_string(&self.metadata.id),
            quoted_metadata_string(&self.metadata.display_name),
            quoted_metadata_string(&self.metadata.vendor),
            quoted_metadata_string(&self.metadata.version),
            quoted_metadata_string(&self.metadata.category),
            features,
            self.parameter_count
        )
    }

    fn artifact_descriptor(&self) -> String {
        let mut descriptor = format!(
            "artifact_format = {}\nformat = {}\nentry_library = {}\nmetadata_manifest = {}\nparameter_count = {}\n",
            quoted_metadata_string("hawk2ui-plugin-package"),
            quoted_metadata_string(self.format.manifest_key()),
            quoted_metadata_string(&format!(
                "{}.{}",
                self.metadata.display_name,
                self.format.extension()
            )),
            quoted_metadata_string("hawk2ui-package.toml"),
            self.parameter_count
        );
        if self.runtime_artifact.is_some() {
            let _ = writeln!(
                descriptor,
                "runtime_artifact = {}",
                quoted_metadata_string("Contents/Resources/hawk2ui-runtime-artifact.json")
            );
            let _ = writeln!(
                descriptor,
                "editor_descriptor = {}",
                quoted_metadata_string("Contents/Resources/hawk2ui-editor.toml")
            );
        }
        descriptor
    }

    fn editor_descriptor(&self) -> String {
        format!(
            "host_adapter = {}\nrenderer = {}\nruntime_artifact = {}\nformat = {}\nplugin_id = {}\nparameter_count = {}\neditor_id = {}\nlogical_width = {}\nlogical_height = {}\nscale_factor = {}\n",
            quoted_metadata_string("baseview"),
            quoted_metadata_string("skia"),
            quoted_metadata_string("Contents/Resources/hawk2ui-runtime-artifact.json"),
            quoted_metadata_string(self.format.manifest_key()),
            quoted_metadata_string(&self.metadata.id),
            self.parameter_count,
            quoted_metadata_string(&self.editor.id),
            self.editor.initial_size.logical_width,
            self.editor.initial_size.logical_height,
            self.editor.initial_size.scale_factor
        )
    }

    fn write_format_layout(
        &self,
        output_path: &Path,
        resources_path: &Path,
    ) -> Result<Vec<PathBuf>, PackageMaterializationError> {
        let mut written = Vec::new();
        match self.format {
            PackageFormat::Clap => {
                let entry_path = output_path.join(format!("{}.clap", self.metadata.display_name));
                write_package_file(&entry_path, self.entry_descriptor("clap"))?;
                written.push(entry_path);
                let clap_manifest_path = resources_path.join("clap.json");
                write_package_file(&clap_manifest_path, self.clap_manifest())?;
                written.push(clap_manifest_path);
                let clap_entry_path = resources_path.join("clap-entry.toml");
                write_package_file(
                    &clap_entry_path,
                    ClapPluginEntryPlan::from_metadata(&self.metadata).manifest(),
                )?;
                written.push(clap_entry_path);
                let scaffold_output = self.write_clap_cdylib_scaffold(resources_path)?;
                written.push(PathBuf::from(scaffold_output.cargo_toml_path));
                written.push(PathBuf::from(scaffold_output.lib_rs_path));
            }
            PackageFormat::Vst3 => {
                let info_path = output_path.join("Contents").join("Info.plist");
                write_package_file(&info_path, self.info_plist("vst3"))?;
                written.push(info_path);
                let binary_dir = output_path.join("Contents").join("x86_64-linux");
                create_package_dir(&binary_dir)?;
                let binary_path = binary_dir.join(format!("{}.vst3", self.metadata.display_name));
                write_package_file(&binary_path, self.entry_descriptor("vst3"))?;
                written.push(binary_path);
                let scaffold_output = self.write_vst3_cdylib_scaffold(resources_path)?;
                written.push(PathBuf::from(scaffold_output.cargo_toml_path));
                written.push(PathBuf::from(scaffold_output.lib_rs_path));
            }
            PackageFormat::Au | PackageFormat::Standalone | PackageFormat::DesktopBundle => {
                let package_type = self.format.manifest_key();
                let info_path = output_path.join("Contents").join("Info.plist");
                write_package_file(&info_path, self.info_plist(package_type))?;
                written.push(info_path);
                let binary_dir = output_path.join("Contents").join("MacOS");
                create_package_dir(&binary_dir)?;
                let binary_path = binary_dir.join(&self.metadata.display_name);
                write_package_file(&binary_path, self.entry_descriptor(package_type))?;
                written.push(binary_path);
                if matches!(
                    self.format,
                    PackageFormat::Standalone | PackageFormat::DesktopBundle
                ) {
                    let launch_path = resources_path.join("hawk2ui-launch.toml");
                    write_package_file(&launch_path, self.launch_manifest())?;
                    written.push(launch_path);
                }
            }
            PackageFormat::SealedArtifact => {
                let artifact_path = resources_path.join("sealed-artifact.hawk2ui");
                write_package_file(&artifact_path, self.entry_descriptor("sealed-artifact"))?;
                written.push(artifact_path);
            }
        }
        Ok(written)
    }

    fn clap_manifest(&self) -> String {
        format!(
            "{{\n  \"id\": {},\n  \"name\": {},\n  \"vendor\": {},\n  \"version\": {}\n}}\n",
            quoted_metadata_string(&self.metadata.id),
            quoted_metadata_string(&self.metadata.display_name),
            quoted_metadata_string(&self.metadata.vendor),
            quoted_metadata_string(&self.metadata.version)
        )
    }

    fn info_plist(&self, package_type: &str) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\"><dict><key>CFBundleIdentifier</key><string>{}</string><key>CFBundleName</key><string>{}</string><key>Hawk2UIVendor</key><string>{}</string><key>Hawk2UIPackageType</key><string>{}</string></dict></plist>\n",
            xml_text(&self.metadata.id),
            xml_text(&self.metadata.display_name),
            xml_text(&self.metadata.vendor),
            xml_text(package_type)
        )
    }

    fn launch_manifest(&self) -> String {
        format!(
            "entry = {}\nid = {}\n",
            quoted_metadata_string(&format!("Contents/MacOS/{}", self.metadata.display_name)),
            quoted_metadata_string(&self.metadata.id)
        )
    }

    fn entry_descriptor(&self, format: &str) -> String {
        format!(
            "hawk2ui package entry\nformat={format}\nid={}\nversion={}\nparameters={}\nartifact_descriptor=Contents/Resources/hawk2ui-artifact.toml\n",
            descriptor_value(&self.metadata.id),
            descriptor_value(&self.metadata.version),
            self.parameter_count
        )
    }

    fn required_package_files(&self, output_path: &Path, resources_path: &Path) -> Vec<PathBuf> {
        let mut files = vec![
            output_path.join("hawk2ui-package.toml"),
            resources_path.join("hawk2ui-artifact.toml"),
        ];
        if self.runtime_artifact.is_some() {
            files.push(resources_path.join("hawk2ui-runtime-artifact.json"));
            files.push(resources_path.join("hawk2ui-editor.toml"));
        }
        match self.format {
            PackageFormat::Clap => {
                files.push(output_path.join(format!("{}.clap", self.metadata.display_name)));
                files.push(resources_path.join("clap.json"));
                files.push(resources_path.join("clap-entry.toml"));
                files.push(resources_path.join("generated-clap").join("Cargo.toml"));
                files.push(
                    resources_path
                        .join("generated-clap")
                        .join("src")
                        .join("lib.rs"),
                );
            }
            PackageFormat::Vst3 => {
                files.push(output_path.join("Contents").join("Info.plist"));
                files.push(
                    output_path
                        .join("Contents")
                        .join("x86_64-linux")
                        .join(format!("{}.vst3", self.metadata.display_name)),
                );
                files.push(resources_path.join("generated-vst3").join("Cargo.toml"));
                files.push(
                    resources_path
                        .join("generated-vst3")
                        .join("src")
                        .join("lib.rs"),
                );
            }
            PackageFormat::Au => {
                files.push(output_path.join("Contents").join("Info.plist"));
                files.push(
                    output_path
                        .join("Contents")
                        .join("MacOS")
                        .join(&self.metadata.display_name),
                );
            }
            PackageFormat::Standalone | PackageFormat::DesktopBundle => {
                files.push(output_path.join("Contents").join("Info.plist"));
                files.push(
                    output_path
                        .join("Contents")
                        .join("MacOS")
                        .join(&self.metadata.display_name),
                );
                files.push(resources_path.join("hawk2ui-launch.toml"));
            }
            PackageFormat::SealedArtifact => {
                files.push(resources_path.join("sealed-artifact.hawk2ui"));
            }
        }
        files
    }

    fn verify_materialized_output(&self, output: &MaterializedPackageOutput) -> bool {
        let output_path = Path::new(&output.output_path);
        let resources_path = output_path.join("Contents").join("Resources");
        let hash_manifest_path = Path::new(&output.hash_manifest_path);
        output_path.is_dir()
            && Path::new(&output.manifest_path).is_file()
            && Path::new(&output.artifact_descriptor_path).is_file()
            && hash_manifest_path.is_file()
            && self
                .required_package_files(output_path, &resources_path)
                .iter()
                .all(|path| path.is_file())
            && hash_manifest_matches(output_path, hash_manifest_path)
    }

    fn write_clap_cdylib_scaffold(
        &self,
        resources_path: &Path,
    ) -> Result<ClapCdylibScaffoldOutput, PackageMaterializationError> {
        let mut scaffold = ClapCdylibScaffold::from_metadata(&self.metadata)
            .with_editor(&self.editor)
            .with_parameter_sources(
                self.clap_parameter_source.clone(),
                self.clap_parameter_value_source.clone(),
                self.parameter_count,
            );
        if self.runtime_artifact.is_some() {
            let descriptor = ClapRuntimeEditorDescriptor::new(
                "Contents/Resources/hawk2ui-runtime-artifact.json",
                "baseview",
                "skia",
            )
            .map_err(|diagnostic| PackageMaterializationError { diagnostic })?;
            scaffold = scaffold.with_runtime_editor_descriptor(descriptor);
        }
        scaffold.write_to(resources_path.join("generated-clap"))
    }

    fn write_vst3_cdylib_scaffold(
        &self,
        resources_path: &Path,
    ) -> Result<Vst3CdylibScaffoldOutput, PackageMaterializationError> {
        Vst3CdylibScaffold::from_metadata(&self.metadata)
            .write_to(resources_path.join("generated-vst3"))
    }
}

/// Package plan.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PackagePlan {
    targets: Vec<PackageTargetPlan>,
}

impl PackagePlan {
    /// Generates the JSON Schema for package plans.
    ///
    /// # Errors
    ///
    /// Returns [`PackageDiagnostic`] when the generated schema cannot be represented as JSON.
    pub fn json_schema() -> Result<serde_json::Value, PackageDiagnostic> {
        package_json_schema::<Self>("package.schema.plan.generate-failed", "package plan schema")
    }

    /// Validates a JSON value against the generated package plan schema.
    ///
    /// # Errors
    ///
    /// Returns [`PackageDiagnostic`] when schema compilation fails or the value fails validation.
    pub fn validate_json(value: &serde_json::Value) -> Result<(), PackageDiagnostic> {
        validate_package_json::<Self>(
            value,
            "package.schema.plan.compile-failed",
            "package.schema.plan.invalid",
            "package plan",
        )
    }

    /// Returns planned targets.
    #[must_use]
    pub fn targets(&self) -> &[PackageTargetPlan] {
        &self.targets
    }

    /// Materializes package output directories with deterministic package metadata.
    ///
    /// # Errors
    ///
    /// Returns [`PackageMaterializationError`] when an output directory or metadata manifest
    /// cannot be created.
    pub fn materialize(
        &self,
    ) -> Result<Vec<MaterializedPackageOutput>, PackageMaterializationError> {
        self.targets
            .iter()
            .map(PackageTargetPlan::materialize)
            .collect()
    }

    /// Verifies planned package outputs.
    #[must_use]
    pub fn verify(&self) -> VerificationReport {
        let entries: Vec<_> = self
            .targets
            .iter()
            .cloned()
            .map(|target| VerificationEntry {
                target,
                status: VerificationStatus::Passed,
            })
            .collect();
        VerificationReport { entries }
    }

    /// Verifies materialized package outputs exist on disk with their metadata and artifact
    /// descriptors.
    #[must_use]
    pub fn verify_materialized(&self, outputs: &[MaterializedPackageOutput]) -> VerificationReport {
        let entries = self
            .targets
            .iter()
            .cloned()
            .map(|target| {
                let status = outputs
                    .iter()
                    .find(|output| output.format == target.format)
                    .filter(|output| target.verify_materialized_output(output))
                    .map_or(VerificationStatus::Failed, |_| VerificationStatus::Passed);
                VerificationEntry { target, status }
            })
            .collect();
        VerificationReport { entries }
    }
}

/// Materialized package output metadata.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializedPackageOutput {
    /// Package format.
    pub format: PackageFormat,
    /// Output directory path.
    pub output_path: String,
    /// Metadata manifest path written inside the output directory.
    pub manifest_path: String,
    /// Runtime artifact descriptor path written inside the output directory.
    pub artifact_descriptor_path: String,
    /// Package hash manifest path written inside the output directory.
    pub hash_manifest_path: String,
}

impl MaterializedPackageOutput {
    /// Generates the JSON Schema for materialized package output metadata.
    ///
    /// # Errors
    ///
    /// Returns [`PackageDiagnostic`] when the generated schema cannot be represented as JSON.
    pub fn json_schema() -> Result<serde_json::Value, PackageDiagnostic> {
        package_json_schema::<Self>(
            "package.schema.materialized-output.generate-failed",
            "materialized package output schema",
        )
    }

    /// Validates a JSON value against the generated materialized package output schema.
    ///
    /// # Errors
    ///
    /// Returns [`PackageDiagnostic`] when schema compilation fails or the value fails validation.
    pub fn validate_json(value: &serde_json::Value) -> Result<(), PackageDiagnostic> {
        validate_package_json::<Self>(
            value,
            "package.schema.materialized-output.compile-failed",
            "package.schema.materialized-output.invalid",
            "materialized package output",
        )
    }
}

/// Package materialization error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageMaterializationError {
    diagnostic: PackageDiagnostic,
}

impl PackageMaterializationError {
    /// Returns the materialization diagnostic.
    #[must_use]
    pub const fn diagnostic(&self) -> &PackageDiagnostic {
        &self.diagnostic
    }
}

/// Package adapter set.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PackageAdapterSet;

impl PackageAdapterSet {
    /// Creates a package adapter set.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Plans package outputs.
    ///
    /// # Errors
    ///
    /// Returns [`PackagePlanningError`] when package metadata or target selection is invalid.
    pub fn plan(&self, request: &PackageRequest) -> Result<PackagePlan, PackagePlanningError> {
        validate_request(request)?;
        let targets = request
            .formats
            .iter()
            .map(|format| PackageTargetPlan {
                format: *format,
                metadata: request.metadata.clone(),
                output_path: output_path(&request.output, *format),
                parameter_count: request.parameters.parameters.len(),
                runtime_artifact: request.runtime_artifact.clone(),
                editor: request.editor.clone().unwrap_or_else(default_plugin_editor),
                clap_parameter_source: clap_parameter_source(&request.parameters),
                clap_parameter_value_source: clap_parameter_value_source(&request.parameters),
            })
            .collect();
        Ok(PackagePlan { targets })
    }
}

/// Verification status.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub enum VerificationStatus {
    /// Verification passed.
    Passed,
    /// Verification failed.
    Failed,
}

/// Single verification entry.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationEntry {
    target: PackageTargetPlan,
    status: VerificationStatus,
}

impl VerificationEntry {
    /// Returns target.
    #[must_use]
    pub const fn target(&self) -> &PackageTargetPlan {
        &self.target
    }

    /// Returns metadata.
    #[must_use]
    pub const fn metadata(&self) -> &FormatMetadata {
        self.target.metadata()
    }

    /// Returns verification status.
    #[must_use]
    pub const fn status(&self) -> VerificationStatus {
        self.status
    }
}

/// Verification report.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationReport {
    entries: Vec<VerificationEntry>,
}

impl VerificationReport {
    /// Generates the JSON Schema for package verification reports.
    ///
    /// # Errors
    ///
    /// Returns [`PackageDiagnostic`] when the generated schema cannot be represented as JSON.
    pub fn json_schema() -> Result<serde_json::Value, PackageDiagnostic> {
        package_json_schema::<Self>(
            "package.schema.verification-report.generate-failed",
            "verification report schema",
        )
    }

    /// Validates a JSON value against the generated package verification report schema.
    ///
    /// # Errors
    ///
    /// Returns [`PackageDiagnostic`] when schema compilation fails or the value fails validation.
    pub fn validate_json(value: &serde_json::Value) -> Result<(), PackageDiagnostic> {
        validate_package_json::<Self>(
            value,
            "package.schema.verification-report.compile-failed",
            "package.schema.verification-report.invalid",
            "verification report",
        )
    }

    /// Returns aggregate status.
    #[must_use]
    pub fn status(&self) -> VerificationStatus {
        if self
            .entries
            .iter()
            .all(|entry| entry.status == VerificationStatus::Passed)
        {
            VerificationStatus::Passed
        } else {
            VerificationStatus::Failed
        }
    }

    /// Returns verification entries.
    #[must_use]
    pub fn entries(&self) -> &[VerificationEntry] {
        &self.entries
    }
}

/// Package planning diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageDiagnostic {
    rule: String,
    message: String,
}

impl PackageDiagnostic {
    /// Creates a package diagnostic.
    #[must_use]
    pub fn new(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            rule: rule.into(),
            message: message.into(),
        }
    }

    /// Returns diagnostic rule.
    #[must_use]
    pub fn rule(&self) -> &str {
        &self.rule
    }

    /// Returns diagnostic message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Package planning error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackagePlanningError {
    diagnostics: Vec<PackageDiagnostic>,
}

impl PackagePlanningError {
    /// Returns diagnostics.
    #[must_use]
    pub fn diagnostics(&self) -> &[PackageDiagnostic] {
        &self.diagnostics
    }
}

fn validate_request(request: &PackageRequest) -> Result<(), PackagePlanningError> {
    let mut diagnostics = Vec::new();
    if !is_reverse_dns_id(&request.metadata.id) {
        diagnostics.push(PackageDiagnostic::new(
            "package.metadata.invalid",
            "metadata ID must be reverse-DNS safe",
        ));
    }
    if request.formats.is_empty() {
        diagnostics.push(PackageDiagnostic::new(
            "package.formats.empty",
            "at least one package format is required",
        ));
    }
    if request.output.path.trim().is_empty() {
        diagnostics.push(PackageDiagnostic::new(
            "package.output.empty",
            "output path must not be empty",
        ));
    }
    if !is_filesystem_segment(&request.output.bundle_name) {
        diagnostics.push(PackageDiagnostic::new(
            "package.bundle-name.invalid",
            "bundle name must be a single filesystem segment",
        ));
    }
    if !is_filesystem_segment(&request.metadata.display_name) {
        diagnostics.push(PackageDiagnostic::new(
            "package.display-name.invalid",
            "display name must be a non-empty single filesystem segment",
        ));
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(PackagePlanningError { diagnostics })
    }
}

fn require_linux_display(linux_display_handle: Option<u64>) -> Result<u64, PackageDiagnostic> {
    match linux_display_handle {
        Some(display) if display != 0 => Ok(display),
        Some(_) | None => Err(PackageDiagnostic::new(
            "package.clap-gui-parent.missing-display",
            "CLAP Linux GUI parent mapping requires a nonzero display handle from the host context",
        )),
    }
}

fn clap_version_string() -> String {
    format!(
        "{}.{}.{}",
        clap_sys::version::CLAP_VERSION_MAJOR,
        clap_sys::version::CLAP_VERSION_MINOR,
        clap_sys::version::CLAP_VERSION_REVISION
    )
}

fn clap_feature_string(value: &std::ffi::CStr) -> String {
    value.to_string_lossy().into_owned()
}

fn is_supported_clap_feature(value: &str) -> bool {
    supported_clap_features()
        .iter()
        .any(|feature| value == feature.to_string_lossy())
}

fn supported_clap_features() -> [&'static std::ffi::CStr; 39] {
    use clap_sys::plugin_features::{
        CLAP_PLUGIN_FEATURE_AMBISONIC, CLAP_PLUGIN_FEATURE_ANALYZER,
        CLAP_PLUGIN_FEATURE_AUDIO_EFFECT, CLAP_PLUGIN_FEATURE_CHORUS,
        CLAP_PLUGIN_FEATURE_COMPRESSOR, CLAP_PLUGIN_FEATURE_DEESSER, CLAP_PLUGIN_FEATURE_DELAY,
        CLAP_PLUGIN_FEATURE_DISTORTION, CLAP_PLUGIN_FEATURE_DRUM, CLAP_PLUGIN_FEATURE_DRUM_MACHINE,
        CLAP_PLUGIN_FEATURE_EQUALIZER, CLAP_PLUGIN_FEATURE_EXPANDER, CLAP_PLUGIN_FEATURE_FILTER,
        CLAP_PLUGIN_FEATURE_FLANGER, CLAP_PLUGIN_FEATURE_FREQUENCY_SHIFTER,
        CLAP_PLUGIN_FEATURE_GATE, CLAP_PLUGIN_FEATURE_GLITCH, CLAP_PLUGIN_FEATURE_GRANULAR,
        CLAP_PLUGIN_FEATURE_INSTRUMENT, CLAP_PLUGIN_FEATURE_LIMITER, CLAP_PLUGIN_FEATURE_MASTERING,
        CLAP_PLUGIN_FEATURE_MIXING, CLAP_PLUGIN_FEATURE_MONO, CLAP_PLUGIN_FEATURE_MULTI_EFFECTS,
        CLAP_PLUGIN_FEATURE_NOTE_DETECTOR, CLAP_PLUGIN_FEATURE_NOTE_EFFECT,
        CLAP_PLUGIN_FEATURE_PHASE_VOCODER, CLAP_PLUGIN_FEATURE_PHASER,
        CLAP_PLUGIN_FEATURE_PITCH_CORRECTION, CLAP_PLUGIN_FEATURE_PITCH_SHIFTER,
        CLAP_PLUGIN_FEATURE_RESTORATION, CLAP_PLUGIN_FEATURE_REVERB, CLAP_PLUGIN_FEATURE_SAMPLER,
        CLAP_PLUGIN_FEATURE_STEREO, CLAP_PLUGIN_FEATURE_SURROUND, CLAP_PLUGIN_FEATURE_SYNTHESIZER,
        CLAP_PLUGIN_FEATURE_TRANSIENT_SHAPER, CLAP_PLUGIN_FEATURE_TREMOLO,
        CLAP_PLUGIN_FEATURE_UTILITY,
    };
    [
        CLAP_PLUGIN_FEATURE_INSTRUMENT,
        CLAP_PLUGIN_FEATURE_AUDIO_EFFECT,
        CLAP_PLUGIN_FEATURE_NOTE_EFFECT,
        CLAP_PLUGIN_FEATURE_NOTE_DETECTOR,
        CLAP_PLUGIN_FEATURE_ANALYZER,
        CLAP_PLUGIN_FEATURE_SYNTHESIZER,
        CLAP_PLUGIN_FEATURE_SAMPLER,
        CLAP_PLUGIN_FEATURE_DRUM,
        CLAP_PLUGIN_FEATURE_DRUM_MACHINE,
        CLAP_PLUGIN_FEATURE_FILTER,
        CLAP_PLUGIN_FEATURE_PHASER,
        CLAP_PLUGIN_FEATURE_EQUALIZER,
        CLAP_PLUGIN_FEATURE_DEESSER,
        CLAP_PLUGIN_FEATURE_PHASE_VOCODER,
        CLAP_PLUGIN_FEATURE_GRANULAR,
        CLAP_PLUGIN_FEATURE_FREQUENCY_SHIFTER,
        CLAP_PLUGIN_FEATURE_PITCH_SHIFTER,
        CLAP_PLUGIN_FEATURE_DISTORTION,
        CLAP_PLUGIN_FEATURE_TRANSIENT_SHAPER,
        CLAP_PLUGIN_FEATURE_COMPRESSOR,
        CLAP_PLUGIN_FEATURE_EXPANDER,
        CLAP_PLUGIN_FEATURE_GATE,
        CLAP_PLUGIN_FEATURE_LIMITER,
        CLAP_PLUGIN_FEATURE_FLANGER,
        CLAP_PLUGIN_FEATURE_CHORUS,
        CLAP_PLUGIN_FEATURE_DELAY,
        CLAP_PLUGIN_FEATURE_REVERB,
        CLAP_PLUGIN_FEATURE_TREMOLO,
        CLAP_PLUGIN_FEATURE_GLITCH,
        CLAP_PLUGIN_FEATURE_UTILITY,
        CLAP_PLUGIN_FEATURE_PITCH_CORRECTION,
        CLAP_PLUGIN_FEATURE_RESTORATION,
        CLAP_PLUGIN_FEATURE_MULTI_EFFECTS,
        CLAP_PLUGIN_FEATURE_MIXING,
        CLAP_PLUGIN_FEATURE_MASTERING,
        CLAP_PLUGIN_FEATURE_MONO,
        CLAP_PLUGIN_FEATURE_STEREO,
        CLAP_PLUGIN_FEATURE_SURROUND,
        CLAP_PLUGIN_FEATURE_AMBISONIC,
    ]
}

fn quoted_metadata_string(value: &str) -> String {
    format!("\"{}\"", escaped_metadata_string(value))
}

fn escaped_metadata_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => {
                let _ = write!(escaped, "\\u{:04x}", u32::from(ch));
            }
            ch => escaped.push(ch),
        }
    }
    escaped
}

fn clap_parameter_source(parameters: &ParameterModel) -> String {
    if parameters.parameters.is_empty() {
        return "&[]".into();
    }

    let mut source = String::from("&[\n");
    for (index, parameter) in parameters.parameters.iter().enumerate() {
        let _ = writeln!(
            source,
            "    GeneratedParameter {{ id: {}, name: {}, module: {}, min_value: {}, max_value: {}, default_value: {}, flags: {} }},",
            index + 1,
            rust_nul_terminated_byte_string(&parameter.display_name),
            rust_nul_terminated_byte_string(parameter.group_id.as_deref().unwrap_or("")),
            rust_f64_literal(parameter_min_value(parameter)),
            rust_f64_literal(parameter_max_value(parameter)),
            rust_f64_literal(parameter_default_value(parameter)),
            clap_parameter_flags(parameter),
        );
    }
    source.push(']');
    source
}

fn clap_parameter_value_source(parameters: &ParameterModel) -> String {
    if parameters.parameters.is_empty() {
        return "[]".into();
    }

    let mut source = String::from("[\n");
    for parameter in &parameters.parameters {
        let _ = writeln!(
            source,
            "    AtomicU64::new({}),",
            parameter_default_value(parameter).to_bits()
        );
    }
    source.push(']');
    source
}

fn parameter_min_value(parameter: &ParameterRecord) -> f64 {
    parameter.range.map_or(0.0, |range| range.min)
}

fn parameter_max_value(parameter: &ParameterRecord) -> f64 {
    parameter.range.map_or(1.0, |range| range.max)
}

fn parameter_default_value(parameter: &ParameterRecord) -> f64 {
    match parameter.default_value {
        ParameterValue::Float(value) => value,
        ParameterValue::Bool(value) => f64::from(u8::from(value)),
        ParameterValue::Choice(_) => 0.0,
    }
}

fn clap_parameter_flags(parameter: &ParameterRecord) -> u32 {
    let mut flags = 0;
    if parameter.steps.is_some() {
        flags |= 1 << 0;
    }
    if parameter.flags.hidden {
        flags |= 1 << 2;
    }
    if parameter.flags.readonly {
        flags |= 1 << 3;
    }
    if parameter.flags.automatable {
        flags |= 1 << 5;
    }
    flags
}

fn rust_f64_literal(value: f64) -> String {
    format!("{value:?}")
}

fn rust_nul_terminated_byte_string(value: &str) -> String {
    let mut escaped = String::from("b\"");
    for byte in value.as_bytes().iter().copied().chain([0]) {
        match byte {
            b'\\' => escaped.push_str("\\\\"),
            b'\"' => escaped.push_str("\\\""),
            b'\n' => escaped.push_str("\\n"),
            b'\r' => escaped.push_str("\\r"),
            b'\t' => escaped.push_str("\\t"),
            0x20..=0x7e => escaped.push(char::from(byte)),
            _ => {
                let _ = write!(escaped, "\\x{byte:02x}");
            }
        }
    }
    escaped.push('"');
    escaped
}

fn rust_byte_string(value: &str) -> String {
    let mut escaped = String::from("b\"");
    for byte in value.as_bytes().iter().copied() {
        match byte {
            b'\\' => escaped.push_str("\\\\"),
            b'"' => escaped.push_str("\\\""),
            b'\n' => escaped.push_str("\\n"),
            b'\r' => escaped.push_str("\\r"),
            b'\t' => escaped.push_str("\\t"),
            0x20..=0x7e => escaped.push(char::from(byte)),
            _ => {
                let _ = write!(escaped, "\\x{byte:02x}");
            }
        }
    }
    escaped.push('"');
    escaped
}

fn unescape_metadata_string(value: &str) -> Option<String> {
    let mut unescaped = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            unescaped.push(ch);
            continue;
        }
        match chars.next()? {
            '"' => unescaped.push('"'),
            '\\' => unescaped.push('\\'),
            'n' => unescaped.push('\n'),
            'r' => unescaped.push('\r'),
            't' => unescaped.push('\t'),
            'u' => {
                let mut codepoint = String::with_capacity(4);
                for _ in 0..4 {
                    codepoint.push(chars.next()?);
                }
                let codepoint = u32::from_str_radix(&codepoint, 16).ok()?;
                unescaped.push(char::from_u32(codepoint)?);
            }
            _ => return None,
        }
    }
    Some(unescaped)
}

fn xml_text(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            ch => escaped.push(ch),
        }
    }
    escaped
}

fn descriptor_value(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\\' => escaped.push_str("\\\\"),
            ch => escaped.push(ch),
        }
    }
    escaped
}

fn materialization_error(
    rule: impl Into<String>,
    message: impl Into<String>,
) -> PackageMaterializationError {
    PackageMaterializationError {
        diagnostic: PackageDiagnostic::new(rule, message),
    }
}

fn package_json_schema<T: JsonSchema>(
    rule: &'static str,
    label: &'static str,
) -> Result<serde_json::Value, PackageDiagnostic> {
    serde_json::to_value(schemars::schema_for!(T)).map_err(|error| {
        PackageDiagnostic::new(
            rule,
            format!("generated {label} could not be serialized: {error}"),
        )
    })
}

fn validate_package_json<T: JsonSchema>(
    value: &serde_json::Value,
    compile_rule: &'static str,
    invalid_rule: &'static str,
    label: &'static str,
) -> Result<(), PackageDiagnostic> {
    let schema = package_json_schema::<T>(
        "package.schema.generate-failed",
        "package adapter record schema",
    )?;
    let validator = jsonschema::Validator::new(&schema).map_err(|error| {
        PackageDiagnostic::new(
            compile_rule,
            format!("generated {label} schema could not be compiled: {error}"),
        )
    })?;
    validator.validate(value).map_err(|error| {
        PackageDiagnostic::new(
            invalid_rule,
            format!("{label} failed schema validation: {error}"),
        )
    })
}

fn required_toml_string(
    table: &toml::Table,
    key: &'static str,
) -> Result<String, PackageMaterializationError> {
    table
        .get(key)
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            materialization_error(
                "package.clap-runtime-editor.descriptor-invalid",
                format!("CLAP runtime editor descriptor requires string field `{key}`"),
            )
        })
}

fn required_toml_usize(
    table: &toml::Table,
    key: &'static str,
) -> Result<usize, PackageMaterializationError> {
    let value = table
        .get(key)
        .and_then(toml::Value::as_integer)
        .ok_or_else(|| {
            materialization_error(
                "package.clap-runtime-editor.descriptor-invalid",
                format!("CLAP runtime editor descriptor requires integer field `{key}`"),
            )
        })?;
    usize::try_from(value).map_err(|_| {
        materialization_error(
            "package.clap-runtime-editor.descriptor-invalid",
            format!("CLAP runtime editor descriptor field `{key}` must be non-negative"),
        )
    })
}

fn required_toml_f64(
    table: &toml::Table,
    key: &'static str,
) -> Result<f64, PackageMaterializationError> {
    let value = table.get(key).ok_or_else(|| {
        materialization_error(
            "package.clap-runtime-editor.descriptor-invalid",
            format!("CLAP runtime editor descriptor requires numeric field `{key}`"),
        )
    })?;
    value
        .as_float()
        .or_else(|| {
            value
                .as_integer()
                .and_then(|value| i32::try_from(value).ok())
                .map(f64::from)
        })
        .ok_or_else(|| {
            materialization_error(
                "package.clap-runtime-editor.descriptor-invalid",
                format!("CLAP runtime editor descriptor field `{key}` must be numeric"),
            )
        })
}

fn create_package_dir(path: &Path) -> Result<(), PackageMaterializationError> {
    fs::create_dir_all(path).map_err(|error| {
        materialization_error(
            "package.directory.create-failed",
            format!(
                "failed to create package directory {}: {error}",
                path.display()
            ),
        )
    })
}

fn write_package_file(
    path: &Path,
    contents: impl AsRef<[u8]>,
) -> Result<(), PackageMaterializationError> {
    fs::write(path, contents).map_err(|error| {
        materialization_error(
            "package.file.write-failed",
            format!("failed to write package file {}: {error}", path.display()),
        )
    })
}

fn hash_manifest(root: &Path, files: &[PathBuf]) -> Result<String, PackageMaterializationError> {
    let mut entries = Vec::with_capacity(files.len());
    for path in files {
        let bytes = fs::read(path).map_err(|error| {
            materialization_error(
                "package.hashes.read-failed",
                format!("failed to hash package file {}: {error}", path.display()),
            )
        })?;
        let relative = path.strip_prefix(root).map_err(|error| {
            materialization_error(
                "package.hashes.path-invalid",
                format!(
                    "package file {} is outside package root {}: {error}",
                    path.display(),
                    root.display()
                ),
            )
        })?;
        entries.push((
            relative.to_string_lossy().replace('\\', "/"),
            sha256(&bytes),
        ));
    }
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    let mut manifest = String::from("algorithm = \"sha256\"\n\n");
    for (path, hash) in entries {
        manifest.push_str("[[files]]\npath = \"");
        manifest.push_str(&escaped_metadata_string(&path));
        manifest.push_str("\"\nhash = \"");
        manifest.push_str(&hash);
        manifest.push_str("\"\n\n");
    }
    Ok(manifest)
}

fn hash_manifest_matches(root: &Path, manifest_path: &Path) -> bool {
    let Ok(manifest) = fs::read_to_string(manifest_path) else {
        return false;
    };
    let Some(entries) = parse_hash_manifest(&manifest) else {
        return false;
    };
    if entries.is_empty() {
        return false;
    }
    let Ok(manifest_relative) = manifest_path.strip_prefix(root) else {
        return false;
    };
    let manifest_relative = normalized_relative_path(manifest_relative);
    let mut expected = BTreeMap::new();
    for (relative_path, expected_hash) in entries {
        let relative = Path::new(&relative_path);
        if !is_safe_relative_path(relative)
            || !is_sha256_hash(&expected_hash)
            || expected.insert(relative_path, expected_hash).is_some()
        {
            return false;
        }
    }
    let Some(actual_files) = package_regular_files(root, &manifest_relative) else {
        return false;
    };
    if expected.keys().cloned().collect::<BTreeSet<_>>() != actual_files {
        return false;
    }
    expected.into_iter().all(|(relative_path, expected_hash)| {
        fs::read(root.join(&relative_path)).is_ok_and(|bytes| sha256(&bytes) == expected_hash)
    })
}

fn package_regular_files(root: &Path, excluded_relative: &str) -> Option<BTreeSet<String>> {
    fn visit(
        root: &Path,
        current: &Path,
        excluded_relative: &str,
        files: &mut BTreeSet<String>,
    ) -> Option<()> {
        for entry in fs::read_dir(current).ok()? {
            let entry = entry.ok()?;
            let file_type = entry.file_type().ok()?;
            let path = entry.path();
            if file_type.is_dir() {
                visit(root, &path, excluded_relative, files)?;
            } else if file_type.is_file() {
                let relative = normalized_relative_path(path.strip_prefix(root).ok()?);
                if relative != excluded_relative {
                    files.insert(relative);
                }
            } else {
                return None;
            }
        }
        Some(())
    }

    let mut files = BTreeSet::new();
    visit(root, root, excluded_relative, &mut files)?;
    Some(files)
}

fn is_safe_relative_path(path: &Path) -> bool {
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn normalized_relative_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn is_sha256_hash(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.chars().all(|ch| ch.is_ascii_hexdigit()))
}

fn parse_hash_manifest(manifest: &str) -> Option<Vec<(String, String)>> {
    let mut algorithm_is_sha256 = false;
    let mut entries = Vec::new();
    let mut current_path: Option<String> = None;
    let mut current_hash: Option<String> = None;
    for line in manifest.lines().map(str::trim) {
        if line.is_empty() {
            continue;
        }
        if line == "algorithm = \"sha256\"" {
            algorithm_is_sha256 = true;
            continue;
        }
        if line == "[[files]]" {
            push_hash_entry(&mut entries, &mut current_path, &mut current_hash)?;
            continue;
        }
        if let Some(value) = line
            .strip_prefix("path = \"")
            .and_then(|value| value.strip_suffix('"'))
        {
            current_path = Some(unescape_metadata_string(value)?);
            continue;
        }
        if let Some(value) = line
            .strip_prefix("hash = \"")
            .and_then(|value| value.strip_suffix('"'))
        {
            current_hash = Some(value.to_string());
            continue;
        }
        return None;
    }
    push_hash_entry(&mut entries, &mut current_path, &mut current_hash)?;
    algorithm_is_sha256.then_some(entries)
}

fn push_hash_entry(
    entries: &mut Vec<(String, String)>,
    current_path: &mut Option<String>,
    current_hash: &mut Option<String>,
) -> Option<()> {
    match (current_path.take(), current_hash.take()) {
        (Some(path), Some(hash)) => {
            entries.push((path, hash));
            Some(())
        }
        (None, None) => Some(()),
        _ => None,
    }
}

fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity("sha256:".len() + (digest.len() * 2));
    encoded.push_str("sha256:");
    for byte in digest {
        encoded.push(hex_nibble(byte >> 4));
        encoded.push(hex_nibble(byte & 0x0f));
    }
    encoded
}

fn vst3_class_id_hex(plugin_id: &str, role: &str) -> String {
    let seed = format!("hawk2ui:vst3:{plugin_id}:{role}");
    sha256(seed.as_bytes())
        .chars()
        .skip("sha256:".len())
        .take(32)
        .collect()
}

fn hex_nibble(value: u8) -> char {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    char::from(HEX[usize::from(value & 0x0f)])
}

fn output_path(output: &BundleOutput, format: PackageFormat) -> String {
    format!(
        "{}/{}.{}",
        output.path.trim_end_matches('/'),
        output.bundle_name,
        format.extension()
    )
}

fn default_plugin_editor() -> PluginEditor {
    PluginEditor::custom("main-editor", PluginEditorSize::new(800.0, 600.0, 1.0))
}

fn is_reverse_dns_id(value: &str) -> bool {
    let parts: Vec<_> = value.split('.').collect();
    parts.len() >= 3
        && parts.iter().all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        })
}

fn is_filesystem_segment(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && trimmed != "."
        && trimmed != ".."
        && !trimmed.contains('/')
        && !trimmed.contains('\\')
        && !trimmed.contains('\0')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_identity() {
        assert_eq!(crate_name(), "hawk2ui-plugin-adapters");
    }
}

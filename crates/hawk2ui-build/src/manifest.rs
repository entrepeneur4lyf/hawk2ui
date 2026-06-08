//! Hawk manifest schema and validation.

use std::collections::BTreeSet;

use hawk2ui_plugin::{
    EnumVariant, METER_ID_BASE, MeterRecord, ParameterModel, ParameterRange, ParameterRecord,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use toml_edit::DocumentMut;

use crate::package_manager::PackageManagerKind;
use crate::param_codegen::{field_ident, pascal_ident};

/// Supported package target class.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PackageTarget {
    /// Desktop package target.
    Desktop,
    /// Plugin package target.
    Plugin,
}

/// Source framework used to compile the entrypoint.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceFramework {
    /// Direct native authoring package.
    Native,
    /// React compiler package.
    React,
    /// Solid compiler package.
    Solid,
    /// Svelte compiler package.
    Svelte,
    /// Vue compiler package.
    Vue,
}

/// Validated Hawk manifest.
#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HawkManifest {
    /// App identity.
    pub identity: ManifestIdentity,
    /// Optional package metadata.
    pub package: Option<PackageMetadata>,
    /// Source entrypoint.
    pub source: SourceEntrypoint,
    /// Capability keys.
    pub capabilities: Vec<String>,
    /// Package targets.
    pub targets: Vec<TargetDeclaration>,
    /// Optional plugin identity.
    pub plugin: Option<PluginIdentity>,
    /// Optional editor metadata.
    pub editor: Option<EditorMetadata>,
    /// Plugin parameters.
    pub parameters: Vec<PluginParameter>,
    /// Plugin meters (read-only level outputs).
    pub meters: Vec<PluginMeter>,
    /// Asset declarations.
    pub assets: Vec<AssetDeclaration>,
    /// Preset declarations.
    pub presets: Vec<PresetDeclaration>,
    /// Build output metadata.
    pub build: BuildOptions,
}

#[derive(Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct RawManifest {
    identity: Option<ManifestIdentity>,
    package: Option<PackageMetadata>,
    source: Option<SourceEntrypoint>,
    capabilities: Option<CapabilityDeclaration>,
    #[serde(default)]
    targets: Vec<TargetDeclaration>,
    plugin: Option<PluginIdentity>,
    editor: Option<EditorMetadata>,
    #[serde(default)]
    parameters: Vec<PluginParameter>,
    #[serde(default)]
    meters: Vec<PluginMeter>,
    #[serde(default)]
    assets: Vec<AssetDeclaration>,
    #[serde(default)]
    presets: Vec<PresetDeclaration>,
    #[serde(default)]
    build: BuildOptions,
}

#[derive(Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct RawJsonManifest {
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    schema: Option<String>,
    #[serde(rename = "schemaVersion")]
    schema_version: u32,
    package: JsonPackage,
    app: JsonApp,
    targets: JsonTargets,
    #[serde(skip_serializing_if = "Option::is_none")]
    plugin: Option<JsonPlugin>,
    #[serde(default, skip_serializing_if = "JsonAssets::is_empty")]
    assets: JsonAssets,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    presets: Vec<PresetDeclaration>,
    #[serde(default, skip_serializing_if = "JsonPermissions::is_empty")]
    permissions: JsonPermissions,
    #[serde(default, skip_serializing_if = "JsonBuild::is_empty")]
    build: JsonBuild,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct JsonPackage {
    id: String,
    name: String,
    version: String,
    #[serde(rename = "bundleId", skip_serializing_if = "Option::is_none")]
    bundle_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct JsonApp {
    entry: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    framework: Option<SourceFramework>,
    #[serde(skip_serializing_if = "Option::is_none")]
    style: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    script: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct JsonTargets {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    desktop: Vec<JsonDesktopTarget>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    plugin: Vec<JsonPluginTarget>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct JsonDesktopTarget {
    name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    platforms: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    window: Option<JsonWindow>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct JsonPluginTarget {
    name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    formats: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    editor: Option<JsonPluginEditor>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct JsonWindow {
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    height: Option<u32>,
    #[serde(rename = "minWidth", skip_serializing_if = "Option::is_none")]
    min_width: Option<u32>,
    #[serde(rename = "minHeight", skip_serializing_if = "Option::is_none")]
    min_height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resizable: Option<bool>,
    #[serde(
        rename = "presentationBackend",
        skip_serializing_if = "Option::is_none"
    )]
    presentation_backend: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct JsonPluginEditor {
    width: u32,
    height: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    resizable: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct JsonPlugin {
    id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    vendor: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    parameters: Vec<JsonPluginParameter>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    meters: Vec<PluginMeter>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<JsonPluginState>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct JsonPluginParameter {
    id: String,
    #[serde(rename = "paramId", default, skip_serializing_if = "Option::is_none")]
    param_id: Option<u32>,
    name: String,
    #[serde(default)]
    kind: PluginParameterKind,
    #[serde(default = "unit_interval_min")]
    min: f64,
    #[serde(default = "unit_interval_max")]
    max: f64,
    default: f64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    unit: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    variants: Vec<PluginEnumVariant>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct JsonPluginState {
    version: u32,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct JsonAssets {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    include: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    entries: Vec<AssetDeclaration>,
}

impl JsonAssets {
    fn is_empty(&self) -> bool {
        self.include.is_empty() && self.entries.is_empty()
    }
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct JsonPermissions {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    network: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    filesystem: Vec<String>,
}

impl JsonPermissions {
    fn is_empty(&self) -> bool {
        self.capabilities.is_empty() && self.network.is_none() && self.filesystem.is_empty()
    }
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct JsonBuild {
    #[serde(skip_serializing_if = "Option::is_none")]
    output: Option<String>,
    #[serde(rename = "packageManager", skip_serializing_if = "Option::is_none")]
    package_manager: Option<PackageManagerKind>,
}

impl JsonBuild {
    fn is_empty(&self) -> bool {
        self.output.is_none() && self.package_manager.is_none()
    }
}

/// App identity metadata.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestIdentity {
    /// Stable product ID.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Product version.
    pub version: String,
}

/// Source entrypoint declaration.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceEntrypoint {
    /// Source entry path.
    pub entry: String,
    /// Optional source framework for framework-specific compiler dispatch.
    pub framework: Option<SourceFramework>,
    /// Optional style entry path.
    pub style: Option<String>,
    /// Optional script entry path.
    pub script: Option<String>,
}

/// Package metadata.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PackageMetadata {
    /// Package name.
    pub name: String,
    /// Native bundle identifier.
    pub bundle_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CapabilityDeclaration {
    keys: Vec<String>,
}

/// Target declaration.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TargetDeclaration {
    /// Target kind.
    pub kind: PackageTarget,
    /// Stable target name.
    pub name: String,
}

/// Plugin identity metadata.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PluginIdentity {
    /// Plugin ID.
    pub id: String,
    /// Plugin display name.
    pub name: String,
}

/// Editor metadata.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EditorMetadata {
    /// Initial editor width.
    pub width: u32,
    /// Initial editor height.
    pub height: u32,
}

/// Plugin parameter metadata.
///
/// `min`/`max` bound the parameter's plain (denormalized) value and default to
/// the unit interval, so a parameter that declares neither behaves like a
/// normalized `0.0..=1.0` control — the historical shape. `default` is a plain
/// value inside `[min, max]`. `unit`, when present, must be one of the
/// host-display units the parameter codegen understands (`dB`, `Hz`, `ms`,
/// `s`, `%`, `st`, `pan`); an empty unit is unitless.
/// The value kind of a plugin parameter.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginParameterKind {
    /// Continuous floating-point parameter.
    #[default]
    Float,
    /// Discrete integer parameter.
    Int,
    /// Boolean on/off parameter.
    Bool,
    /// Indexed-choice parameter with named variants.
    Enum,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PluginParameter {
    /// Parameter ID.
    pub id: String,
    /// Stable numeric automation id (truce `ParamId`).
    ///
    /// Omit to let the build assign and pin it on first build. Once assigned it
    /// must never change: host automation, presets, and saved state persist this
    /// number. Must be unique across parameters and below truce's reserved meter
    /// range (`< 2^24`).
    #[serde(default)]
    pub param_id: Option<u32>,
    /// Display name.
    pub name: String,
    /// Value kind: `float` (default), `int`, `bool`, or `enum`.
    #[serde(default)]
    pub kind: PluginParameterKind,
    /// Minimum plain value.
    #[serde(default = "unit_interval_min")]
    pub min: f64,
    /// Maximum plain value.
    #[serde(default = "unit_interval_max")]
    pub max: f64,
    /// Default plain value. For numeric kinds it is a plain value inside
    /// `[min, max]`; for an `enum` it is the 0-based index into `variants`.
    pub default: f64,
    /// Host-display unit label, or empty when unitless.
    #[serde(default)]
    pub unit: String,
    /// Named variants for an `enum` parameter; empty for every other kind.
    #[serde(default)]
    pub variants: Vec<PluginEnumVariant>,
}

/// A named variant of an `enum` parameter.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PluginEnumVariant {
    /// Variant ID (the codegen derives the Rust variant identifier from it).
    pub id: String,
    /// Display name.
    pub name: String,
}

/// Plugin meter metadata.
///
/// A meter is a read-only level output (`0.0..=1.0`) the editor reads by id; it
/// carries no host-writable value, range, or unit. Meter ids share the
/// parameter id namespace because both become fields of the one generated
/// `Params` struct and keys in the one editor address space.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PluginMeter {
    /// Meter ID.
    pub id: String,
    /// Display name.
    pub name: String,
}

/// Asset declaration.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssetDeclaration {
    /// Stable asset ID.
    pub id: String,
    /// Asset kind.
    pub kind: String,
    /// Asset source path.
    pub path: String,
}

/// Preset declaration.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PresetDeclaration {
    /// Stable preset ID.
    pub id: String,
    /// Display name.
    pub name: String,
}

/// Build output options.
#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BuildOptions {
    /// Package-manager-produced JavaScript bundle output path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// Explicit package manager used to resolve ambiguous lockfiles.
    #[serde(rename = "packageManager", skip_serializing_if = "Option::is_none")]
    pub package_manager: Option<PackageManagerKind>,
}

impl HawkManifest {
    /// Parses and validates a Hawk manifest from canonical `hawk.json` or legacy TOML.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestError`] when parsing fails or validation rejects the manifest.
    pub fn parse(input: &str) -> Result<Self, ManifestError> {
        if is_json_manifest(input) {
            Self::parse_json(input)
        } else {
            Self::parse_legacy_toml(input)
        }
    }

    fn parse_legacy_toml(input: &str) -> Result<Self, ManifestError> {
        validate_legacy_toml_manifest_schema(input)?;
        let raw: RawManifest =
            toml::from_str(input).map_err(|error| ManifestError::Parse(error.to_string()))?;
        let manifest = Self {
            identity: raw
                .identity
                .ok_or(ManifestError::MissingSection("identity"))?,
            package: raw.package,
            source: raw.source.ok_or(ManifestError::MissingSection("source"))?,
            capabilities: raw.capabilities.map_or_else(Vec::new, |cap| cap.keys),
            targets: raw.targets,
            plugin: raw.plugin,
            editor: raw.editor,
            parameters: raw.parameters,
            meters: raw.meters,
            assets: raw.assets,
            presets: raw.presets,
            build: raw.build,
        };
        manifest.validate()?;
        Ok(manifest)
    }

    fn parse_json(input: &str) -> Result<Self, ManifestError> {
        validate_json_manifest_schema(input)?;
        let raw: RawJsonManifest =
            serde_json::from_str(input).map_err(|error| ManifestError::Parse(error.to_string()))?;
        raw.into_manifest()
    }

    /// Generates the JSON Schema used to validate canonical `hawk.json` documents.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestError`] when the generated schema cannot be represented as JSON.
    pub fn json_schema() -> Result<serde_json::Value, ManifestError> {
        serde_json::to_value(schemars::schema_for!(RawJsonManifest))
            .map_err(|error| ManifestError::Parse(error.to_string()))
    }

    /// Returns true when the manifest declares a capability key.
    #[must_use]
    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|key| key == capability)
    }

    /// Returns true when the manifest declares a target kind.
    #[must_use]
    pub fn has_target(&self, target: PackageTarget) -> bool {
        self.targets.iter().any(|decl| decl.kind == target)
    }

    /// Creates a stable manifest snapshot string for hashing.
    #[must_use]
    pub fn snapshot(&self) -> String {
        format!(
            "{}:{}:{}:{}:{}:{}",
            self.identity.id,
            self.identity.name,
            self.identity.version,
            self.source.entry,
            self.build.output.as_deref().unwrap_or_default(),
            self.build
                .package_manager
                .map(PackageManagerKind::as_str)
                .unwrap_or_default()
        )
    }

    /// Builds the validated parameter model that parameter/meter code
    /// generation consumes.
    ///
    /// Each manifest parameter becomes a [`ParameterRecord`] of the matching
    /// kind — numeric, integer, or boolean — over its plain `[min, max]` range,
    /// and each manifest meter becomes a read-only [`MeterRecord`]. Manifest
    /// validation already guarantees the ranges, defaults, and ids are
    /// well-formed, so the returned model is valid by construction and ready to
    /// drive the truce `Params` and editor-side TypeScript emitters from this
    /// single source.
    #[must_use]
    pub fn parameter_model(&self) -> ParameterModel {
        ParameterModel::new(self.parameters.iter().map(|parameter| {
            let range = ParameterRange::new(parameter.min, parameter.max, parameter.default);
            let record = match parameter.kind {
                PluginParameterKind::Float => ParameterRecord::numeric(
                    parameter.id.clone(),
                    parameter.name.clone(),
                    parameter.unit.clone(),
                    range,
                ),
                PluginParameterKind::Int => ParameterRecord::integer(
                    parameter.id.clone(),
                    parameter.name.clone(),
                    parameter.unit.clone(),
                    range,
                ),
                // Validation guarantees a 0/1 default; `>= 0.5` selects `true`.
                PluginParameterKind::Bool => ParameterRecord::boolean(
                    parameter.id.clone(),
                    parameter.name.clone(),
                    parameter.default >= 0.5,
                ),
                // Validation guarantees an in-range integer default and a
                // non-empty, unique variant set.
                PluginParameterKind::Enum => ParameterRecord::enumerated(
                    parameter.id.clone(),
                    parameter.name.clone(),
                    enum_default_index(parameter.default),
                    parameter
                        .variants
                        .iter()
                        .map(|variant| EnumVariant::new(variant.id.clone(), variant.name.clone())),
                ),
            };
            // Carry the author-pinned numeric id through to the record; an
            // unpinned parameter (`None`) is resolved positionally later by
            // `ParameterModel::resolved_param_ids`.
            match parameter.param_id {
                Some(param_id) => record.param_id(param_id),
                None => record,
            }
        }))
        .with_meters(
            self.meters
                .iter()
                .map(|meter| MeterRecord::new(meter.id.clone(), meter.name.clone())),
        )
    }

    fn validate(&self) -> Result<(), ManifestError> {
        require_non_empty("identity.id", &self.identity.id)?;
        require_non_empty("identity.name", &self.identity.name)?;
        require_non_empty("identity.version", &self.identity.version)?;
        require_non_empty("source.entry", &self.source.entry)?;

        let mut target_names = BTreeSet::new();
        for target in &self.targets {
            require_non_empty("target.name", &target.name)?;
            if !target_names.insert(target.name.clone()) {
                return Err(ManifestError::DuplicateTarget(target.name.clone()));
            }
        }

        for capability in &self.capabilities {
            if capability.trim().is_empty() || capability.contains(' ') {
                return Err(ManifestError::InvalidCapability(capability.clone()));
            }
        }

        if (!self.parameters.is_empty() || !self.meters.is_empty()) && self.plugin.is_none() {
            return Err(ManifestError::InvalidPluginMetadata(
                "parameters and meters require [plugin] metadata",
            ));
        }
        // Parameters and meters share one id namespace: both become fields of
        // the generated `Params` struct and keys in the one editor address
        // space, so a collision would emit a struct that does not compile.
        let mut plugin_ids = BTreeSet::new();
        // Params and meters share the one generated `Params` struct, so their
        // *derived* `field_ident`s must also be unique: `a.b` and `a_b` are
        // distinct ids that map to the same field and would emit a struct that
        // does not compile. Enum params additionally generate a `ParamEnum`
        // *type* keyed by `pascal_ident`, which collapses separators where
        // `field_ident` keeps their count — so `a.b`/`a..b` collide on the type
        // without colliding on the field, an enum-only check.
        let mut field_idents = BTreeSet::new();
        let mut enum_type_idents = BTreeSet::new();
        let mut param_ids = BTreeSet::new();
        for parameter in &self.parameters {
            require_non_empty("parameter.id", &parameter.id)?;
            require_non_empty("parameter.name", &parameter.name)?;
            if !is_stable_id(&parameter.id) {
                return Err(ManifestError::InvalidPluginParameter(parameter.id.clone()));
            }
            validate_parameter_kind(parameter)?;
            validate_enum_variants(parameter)?;
            if !plugin_ids.insert(parameter.id.clone()) {
                return Err(ManifestError::DuplicateParameter(parameter.id.clone()));
            }
            if !field_idents.insert(field_ident(&parameter.id)) {
                return Err(ManifestError::CollidingFieldIdentifier(
                    parameter.id.clone(),
                ));
            }
            if parameter.kind == PluginParameterKind::Enum
                && !enum_type_idents.insert(pascal_ident(&parameter.id))
            {
                return Err(ManifestError::CollidingEnumType(parameter.id.clone()));
            }
            // A pinned numeric id must be unique and below truce's reserved
            // meter range. Caught here (exit code 10) for a clear diagnostic
            // rather than as a downstream Rust compile error in generated code.
            if let Some(param_id) = parameter.param_id {
                if param_id >= METER_ID_BASE {
                    return Err(ManifestError::ReservedParameterId {
                        id: parameter.id.clone(),
                        param_id,
                    });
                }
                if !param_ids.insert(param_id) {
                    return Err(ManifestError::DuplicateParameterId {
                        id: parameter.id.clone(),
                        param_id,
                    });
                }
            }
        }
        for meter in &self.meters {
            require_non_empty("meter.id", &meter.id)?;
            require_non_empty("meter.name", &meter.name)?;
            if !is_stable_id(&meter.id) {
                return Err(ManifestError::InvalidPluginMeter(meter.id.clone()));
            }
            if !plugin_ids.insert(meter.id.clone()) {
                return Err(ManifestError::DuplicateMeter(meter.id.clone()));
            }
            if !field_idents.insert(field_ident(&meter.id)) {
                return Err(ManifestError::CollidingFieldIdentifier(meter.id.clone()));
            }
        }

        if let Some(package) = &self.package {
            require_non_empty("package.name", &package.name)?;
            require_non_empty("package.bundle_id", &package.bundle_id)?;
        }

        let mut asset_ids = BTreeSet::new();
        for asset in &self.assets {
            require_non_empty("asset.id", &asset.id)?;
            require_non_empty("asset.kind", &asset.kind)?;
            require_non_empty("asset.path", &asset.path)?;
            if !asset_ids.insert(asset.id.clone()) {
                return Err(ManifestError::DuplicateAsset(asset.id.clone()));
            }
        }

        let mut preset_ids = BTreeSet::new();
        for preset in &self.presets {
            require_non_empty("preset.id", &preset.id)?;
            require_non_empty("preset.name", &preset.name)?;
            if !preset_ids.insert(preset.id.clone()) {
                return Err(ManifestError::DuplicatePreset(preset.id.clone()));
            }
        }

        if let Some(output) = &self.build.output {
            require_non_empty("build.output", output)?;
        }

        Ok(())
    }
}

impl RawJsonManifest {
    fn into_manifest(self) -> Result<HawkManifest, ManifestError> {
        if self.schema_version != 1 {
            return Err(ManifestError::SchemaValidation {
                path: "/schemaVersion".into(),
                message: "unsupported schemaVersion; expected 1".into(),
            });
        }
        validate_json_targets(&self.targets)?;

        let mut targets =
            Vec::with_capacity(self.targets.desktop.len() + self.targets.plugin.len());
        targets.extend(
            self.targets
                .desktop
                .into_iter()
                .map(|target| TargetDeclaration {
                    kind: PackageTarget::Desktop,
                    name: target.name,
                }),
        );
        let editor = self.targets.plugin.iter().find_map(|target| {
            target
                .editor
                .as_ref()
                .map(JsonPluginEditor::editor_metadata)
        });
        targets.extend(
            self.targets
                .plugin
                .into_iter()
                .map(|target| TargetDeclaration {
                    kind: PackageTarget::Plugin,
                    name: target.name,
                }),
        );

        let (plugin, parameters, meters) = self.plugin.map_or_else(
            || (None, Vec::new(), Vec::new()),
            |plugin| {
                (
                    Some(PluginIdentity {
                        id: plugin.id,
                        name: plugin.name,
                    }),
                    plugin
                        .parameters
                        .into_iter()
                        .map(PluginParameter::from)
                        .collect(),
                    plugin.meters,
                )
            },
        );

        let manifest = HawkManifest {
            identity: ManifestIdentity {
                id: self.package.id.clone(),
                name: self.package.name.clone(),
                version: self.package.version.clone(),
            },
            package: Some(PackageMetadata {
                name: self.package.name,
                bundle_id: self.package.bundle_id.unwrap_or(self.package.id),
            }),
            source: SourceEntrypoint {
                entry: self.app.entry,
                framework: self.app.framework,
                style: self.app.style,
                script: self.app.script,
            },
            capabilities: self.permissions.capabilities,
            targets,
            plugin,
            editor,
            parameters,
            meters,
            assets: self.assets.entries,
            presets: self.presets,
            build: BuildOptions {
                output: self.build.output,
                package_manager: self.build.package_manager,
            },
        };
        manifest.validate()?;
        Ok(manifest)
    }

    fn from_manifest(manifest: &HawkManifest) -> Self {
        let package = JsonPackage {
            id: manifest.identity.id.clone(),
            name: manifest.identity.name.clone(),
            version: manifest.identity.version.clone(),
            bundle_id: manifest
                .package
                .as_ref()
                .map(|package| package.bundle_id.clone())
                .or_else(|| Some(manifest.identity.id.clone())),
        };
        let targets = JsonTargets {
            desktop: manifest
                .targets
                .iter()
                .filter(|target| target.kind == PackageTarget::Desktop)
                .map(|target| JsonDesktopTarget {
                    name: target.name.clone(),
                    platforms: Vec::new(),
                    window: None,
                })
                .collect(),
            plugin: manifest
                .targets
                .iter()
                .filter(|target| target.kind == PackageTarget::Plugin)
                .map(|target| JsonPluginTarget {
                    name: target.name.clone(),
                    formats: Vec::new(),
                    editor: manifest
                        .editor
                        .as_ref()
                        .map(JsonPluginEditor::from_editor_metadata),
                })
                .collect(),
        };
        let plugin = manifest.plugin.as_ref().map(|plugin| JsonPlugin {
            id: plugin.id.clone(),
            name: plugin.name.clone(),
            vendor: None,
            parameters: manifest
                .parameters
                .iter()
                .cloned()
                .map(JsonPluginParameter::from)
                .collect(),
            meters: manifest.meters.clone(),
            state: None,
        });
        Self {
            schema: Some("https://hawk2ui.dev/schemas/hawk.schema.json".into()),
            schema_version: 1,
            package,
            app: JsonApp {
                entry: manifest.source.entry.clone(),
                framework: manifest.source.framework,
                style: manifest.source.style.clone(),
                script: manifest.source.script.clone(),
            },
            targets,
            plugin,
            assets: JsonAssets {
                include: Vec::new(),
                entries: manifest.assets.clone(),
            },
            presets: manifest.presets.clone(),
            permissions: JsonPermissions {
                capabilities: manifest.capabilities.clone(),
                network: None,
                filesystem: Vec::new(),
            },
            build: JsonBuild {
                output: manifest.build.output.clone(),
                package_manager: manifest.build.package_manager,
            },
        }
    }
}

fn validate_json_targets(targets: &JsonTargets) -> Result<(), ManifestError> {
    for (index, target) in targets.desktop.iter().enumerate() {
        for platform in &target.platforms {
            if !matches!(
                platform.as_str(),
                "windows" | "macos" | "linux-wayland" | "linux-x11"
            ) {
                return Err(ManifestError::SchemaValidation {
                    path: format!("/targets/desktop/{index}/platforms"),
                    message: format!("unsupported desktop platform: {platform}"),
                });
            }
        }
        if let Some(window) = &target.window
            && let Some(backend) = &window.presentation_backend
            && !matches!(
                backend.as_str(),
                "software" | "gpu-preferred" | "gpu-required"
            )
        {
            return Err(ManifestError::SchemaValidation {
                path: format!("/targets/desktop/{index}/window/presentationBackend"),
                message: format!("unsupported presentation backend: {backend}"),
            });
        }
    }
    for (index, target) in targets.plugin.iter().enumerate() {
        for format in &target.formats {
            if !matches!(format.as_str(), "clap" | "vst3" | "au" | "standalone") {
                return Err(ManifestError::SchemaValidation {
                    path: format!("/targets/plugin/{index}/formats"),
                    message: format!("unsupported plugin format: {format}"),
                });
            }
        }
    }
    Ok(())
}

impl JsonPluginEditor {
    fn editor_metadata(&self) -> EditorMetadata {
        EditorMetadata {
            width: self.width,
            height: self.height,
        }
    }

    fn from_editor_metadata(editor: &EditorMetadata) -> Self {
        Self {
            width: editor.width,
            height: editor.height,
            resizable: None,
        }
    }
}

impl From<JsonPluginParameter> for PluginParameter {
    fn from(parameter: JsonPluginParameter) -> Self {
        Self {
            id: parameter.id,
            param_id: parameter.param_id,
            name: parameter.name,
            kind: parameter.kind,
            min: parameter.min,
            max: parameter.max,
            default: parameter.default,
            unit: parameter.unit,
            variants: parameter.variants,
        }
    }
}

impl From<PluginParameter> for JsonPluginParameter {
    fn from(parameter: PluginParameter) -> Self {
        Self {
            id: parameter.id,
            param_id: parameter.param_id,
            name: parameter.name,
            kind: parameter.kind,
            min: parameter.min,
            max: parameter.max,
            default: parameter.default,
            unit: parameter.unit,
            variants: parameter.variants,
        }
    }
}

fn require_non_empty(field: &'static str, value: &str) -> Result<(), ManifestError> {
    if value.trim().is_empty() {
        Err(ManifestError::MissingField(field))
    } else {
        Ok(())
    }
}

fn is_stable_id(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|ch| {
            ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '.' | '_' | '-')
        })
}

/// Default lower bound for a parameter that omits `min` — the unit interval.
fn unit_interval_min() -> f64 {
    0.0
}

/// Default upper bound for a parameter that omits `max` — the unit interval.
fn unit_interval_max() -> f64 {
    1.0
}

/// Whether a parameter unit label maps onto a host-display unit the codegen
/// understands. Empty means unitless; the rest mirror the canonical truce
/// `ParamUnit` spellings.
fn is_supported_unit(unit: &str) -> bool {
    matches!(unit, "" | "dB" | "Hz" | "ms" | "s" | "%" | "st" | "pan")
}

/// Validates a parameter's bounds, default, and unit for its declared kind.
fn validate_parameter_kind(parameter: &PluginParameter) -> Result<(), ManifestError> {
    let reject = || ManifestError::InvalidPluginParameter(parameter.id.clone());
    match parameter.kind {
        PluginParameterKind::Float => {
            if !is_finite_range(parameter.min, parameter.max, parameter.default)
                || !is_supported_unit(&parameter.unit)
            {
                return Err(reject());
            }
        }
        PluginParameterKind::Int => {
            if !is_finite_range(parameter.min, parameter.max, parameter.default)
                || !is_integer_valued(parameter.min)
                || !is_integer_valued(parameter.max)
                || !is_integer_valued(parameter.default)
                || !is_supported_unit(&parameter.unit)
            {
                return Err(reject());
            }
        }
        PluginParameterKind::Bool => {
            // A boolean's default must be exactly 0 or 1, and it carries no unit.
            if !((0.0..=1.0).contains(&parameter.default)
                && is_integer_valued(parameter.default)
                && parameter.unit.is_empty())
            {
                return Err(reject());
            }
        }
        PluginParameterKind::Enum => {
            // An enum carries no unit, needs at least two variants, and its
            // default is a 0-based index into them. (Per-variant id/name/
            // uniqueness is checked in `validate`, which can derive the Rust
            // identifiers and report each offending variant precisely.)
            if !parameter.unit.is_empty()
                || parameter.variants.len() < 2
                || !is_valid_enum_default(parameter.default, parameter.variants.len())
            {
                return Err(reject());
            }
        }
    }
    Ok(())
}

/// Validates the variant set of a parameter: variants belong only to the `Enum`
/// kind, and each enum variant has a non-empty name and an id that yields a
/// unique, non-empty Rust identifier under the codegen's `pascal_ident`.
///
/// Checking the *derived* identifier (not the raw id) is the load-bearing guard:
/// two distinct ids that collapse to the same identifier — e.g. `low-pass` and
/// `low_pass` both `LowPass` — or an exact duplicate would emit a
/// `#[derive(ParamEnum)]` enum with duplicate variants that fails to compile.
/// Rejecting here surfaces a clear diagnostic instead of a rustc error from
/// generated source. (Derived-identifier uniqueness across top-level
/// parameter/meter field names is a separate, pre-existing gap, tracked apart
/// from enum support.)
fn validate_enum_variants(parameter: &PluginParameter) -> Result<(), ManifestError> {
    if parameter.kind != PluginParameterKind::Enum {
        return if parameter.variants.is_empty() {
            Ok(())
        } else {
            Err(ManifestError::InvalidPluginParameter(parameter.id.clone()))
        };
    }
    let mut derived_idents = BTreeSet::new();
    for variant in &parameter.variants {
        require_non_empty("variant.id", &variant.id)?;
        require_non_empty("variant.name", &variant.name)?;
        let ident = pascal_ident(&variant.id);
        if !is_stable_id(&variant.id) || ident.is_empty() {
            return Err(ManifestError::InvalidEnumVariant(variant.id.clone()));
        }
        if !derived_idents.insert(ident) {
            return Err(ManifestError::CollidingEnumVariant(variant.id.clone()));
        }
    }
    Ok(())
}

/// Whether a `[min, max]` range with `default` inside it is finite and ordered.
fn is_finite_range(min: f64, max: f64, default: f64) -> bool {
    min.is_finite()
        && max.is_finite()
        && default.is_finite()
        && max > min
        && default >= min
        && default <= max
}

/// Whether a value has no fractional part (an exact integer).
fn is_integer_valued(value: f64) -> bool {
    value.fract() == 0.0
}

/// Converts a validated enum default (a non-negative integer-valued plain value)
/// to a 0-based variant index. A stray fractional or negative value saturates to
/// `0` rather than panicking; `validate` rejects such values up front.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn enum_default_index(default: f64) -> u32 {
    default.max(0.0) as u32
}

/// Whether `default` is a valid 0-based variant index for an enum with `count`
/// variants: a non-negative integer strictly less than `count`.
#[allow(clippy::cast_possible_truncation)]
fn is_valid_enum_default(default: f64, count: usize) -> bool {
    is_integer_valued(default)
        && default >= 0.0
        && usize::try_from(default as i64).is_ok_and(|index| index < count)
}

/// Manifest validation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManifestError {
    /// TOML parsing failed.
    Parse(String),
    /// Required section is missing.
    MissingSection(&'static str),
    /// Required field is empty.
    MissingField(&'static str),
    /// Duplicate target name.
    DuplicateTarget(String),
    /// Duplicate asset ID.
    DuplicateAsset(String),
    /// Duplicate preset ID.
    DuplicatePreset(String),
    /// Duplicate parameter ID.
    DuplicateParameter(String),
    /// A parameter pins a numeric id already pinned by another parameter.
    DuplicateParameterId {
        /// Stable string id of the parameter whose pinned numeric id collides.
        id: String,
        /// The duplicated pinned numeric id.
        param_id: u32,
    },
    /// A parameter pins a numeric id in truce's reserved meter range (>= 2^24).
    ReservedParameterId {
        /// Stable string id of the offending parameter.
        id: String,
        /// The out-of-range pinned numeric id.
        param_id: u32,
    },
    /// Duplicate meter ID (within the shared parameter/meter id namespace).
    DuplicateMeter(String),
    /// Invalid capability key.
    InvalidCapability(String),
    /// Invalid plugin metadata.
    InvalidPluginMetadata(&'static str),
    /// Invalid plugin parameter metadata.
    InvalidPluginParameter(String),
    /// Invalid plugin meter metadata.
    InvalidPluginMeter(String),
    /// Invalid enum parameter variant (id is empty, not a stable identifier, or
    /// derives an empty Rust identifier).
    InvalidEnumVariant(String),
    /// Two enum variants share an id or derive the same Rust identifier.
    CollidingEnumVariant(String),
    /// Two distinct parameter/meter ids derive the same generated struct field
    /// identifier (the shared `Params` struct cannot have two fields of one
    /// name).
    CollidingFieldIdentifier(String),
    /// Two distinct enum parameter ids derive the same generated `ParamEnum`
    /// type name (the module cannot define one type twice).
    CollidingEnumType(String),
    /// Manifest failed generated JSON Schema validation.
    SchemaValidation {
        /// JSON pointer to the invalid manifest value.
        path: String,
        /// Validator-provided failure detail.
        message: String,
    },
}

fn is_json_manifest(input: &str) -> bool {
    input.trim_start().starts_with('{')
}

fn legacy_toml_json_schema() -> Result<serde_json::Value, ManifestError> {
    serde_json::to_value(schemars::schema_for!(RawManifest))
        .map_err(|error| ManifestError::Parse(error.to_string()))
}

fn validate_legacy_toml_manifest_schema(input: &str) -> Result<(), ManifestError> {
    let toml_value: toml::Value =
        toml::from_str(input).map_err(|error| ManifestError::Parse(error.to_string()))?;
    let json_value = serde_json::to_value(toml_value)
        .map_err(|error| ManifestError::Parse(error.to_string()))?;
    let schema = legacy_toml_json_schema()?;
    validate_json_value_against_schema(&json_value, &schema)
}

fn validate_json_manifest_schema(input: &str) -> Result<(), ManifestError> {
    let json_value: serde_json::Value =
        serde_json::from_str(input).map_err(|error| ManifestError::Parse(error.to_string()))?;
    let schema = HawkManifest::json_schema()?;
    validate_json_value_against_schema(&json_value, &schema)
}

fn validate_json_value_against_schema(
    json_value: &serde_json::Value,
    schema: &serde_json::Value,
) -> Result<(), ManifestError> {
    let validator = jsonschema::Validator::new(schema)
        .map_err(|error| ManifestError::Parse(error.to_string()))?;
    validator
        .validate(json_value)
        .map_err(|error| ManifestError::SchemaValidation {
            path: error.instance_path().as_str().to_string(),
            message: error.to_string(),
        })
}

/// Outcome of [`pin_param_ids`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PinParamIds {
    /// One or more parameters were assigned a numeric id; carries the rewritten
    /// manifest text (comments and formatting preserved) and the assignments.
    Pinned {
        /// The rewritten `manifest.hawk.toml` contents.
        source: String,
        /// Each newly pinned `(parameter string id, assigned numeric id)`, in
        /// declaration order.
        assigned: Vec<(String, u32)>,
    },
    /// Every parameter already had a pinned id; nothing changed.
    Unchanged,
}

/// Assigns a stable numeric `param_id` to every `[[parameter]]` that lacks one,
/// preserving the manifest's comments and formatting.
///
/// Unpinned parameters take the ids [`ParameterModel::resolved_param_ids`]
/// computes (lowest-free in declaration order, avoiding any author-pinned id),
/// so the result is exactly what the codegen would emit — only now it is written
/// into the manifest, pinning it against future reorders. Idempotent: a manifest
/// whose parameters are all pinned returns [`PinParamIds::Unchanged`].
///
/// # Errors
///
/// Returns [`ManifestError`] when `source` is not a valid Hawk manifest — it is
/// parsed and validated before any rewrite, so an invalid manifest is never
/// mutated.
pub fn pin_param_ids(source: &str) -> Result<PinParamIds, ManifestError> {
    let manifest = HawkManifest::parse(source)?;
    let resolved = manifest.parameter_model().resolved_param_ids();

    let mut document = source
        .parse::<DocumentMut>()
        .map_err(|error| ManifestError::Parse(error.to_string()))?;
    let Some(parameters) = document
        .get_mut("parameters")
        .and_then(toml_edit::Item::as_array_of_tables_mut)
    else {
        return Ok(PinParamIds::Unchanged);
    };

    let mut assigned = Vec::new();
    for (index, table) in parameters.iter_mut().enumerate() {
        if table.contains_key("param_id") {
            continue;
        }
        let (Some(&param_id), Some(parameter)) =
            (resolved.get(index), manifest.parameters.get(index))
        else {
            continue;
        };
        table.insert("param_id", toml_edit::value(i64::from(param_id)));
        assigned.push((parameter.id.clone(), param_id));
    }

    if assigned.is_empty() {
        Ok(PinParamIds::Unchanged)
    } else {
        Ok(PinParamIds::Pinned {
            source: document.to_string(),
            assigned,
        })
    }
}

/// Converts a legacy `manifest.hawk.toml` document into canonical `hawk.json`.
///
/// # Errors
///
/// Returns [`ManifestError`] when the legacy manifest is invalid or the canonical
/// JSON document cannot be rendered.
pub fn migrate_toml_manifest_to_json(source: &str) -> Result<String, ManifestError> {
    let manifest = HawkManifest::parse_legacy_toml(source)?;
    let raw_json = RawJsonManifest::from_manifest(&manifest);
    serde_json::to_string_pretty(&raw_json)
        .map(|mut rendered| {
            rendered.push('\n');
            rendered
        })
        .map_err(|error| ManifestError::Parse(error.to_string()))
}

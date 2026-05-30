//! Hawk manifest schema and validation.

use std::collections::BTreeSet;

use hawk2ui_plugin::{EnumVariant, MeterRecord, ParameterModel, ParameterRange, ParameterRecord};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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

impl HawkManifest {
    /// Parses and validates a Hawk manifest from TOML.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestError`] when parsing fails or validation rejects the manifest.
    pub fn parse(input: &str) -> Result<Self, ManifestError> {
        validate_manifest_schema(input)?;
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
        };
        manifest.validate()?;
        Ok(manifest)
    }

    /// Generates the JSON Schema used to validate raw manifest documents.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestError`] when the generated schema cannot be represented as JSON.
    pub fn json_schema() -> Result<serde_json::Value, ManifestError> {
        serde_json::to_value(schemars::schema_for!(RawManifest))
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
            "{}:{}:{}:{}",
            self.identity.id, self.identity.name, self.identity.version, self.source.entry
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
            match parameter.kind {
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

        Ok(())
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

fn validate_manifest_schema(input: &str) -> Result<(), ManifestError> {
    let toml_value: toml::Value =
        toml::from_str(input).map_err(|error| ManifestError::Parse(error.to_string()))?;
    let json_value = serde_json::to_value(toml_value)
        .map_err(|error| ManifestError::Parse(error.to_string()))?;
    let schema = HawkManifest::json_schema()?;
    let validator = jsonschema::Validator::new(&schema)
        .map_err(|error| ManifestError::Parse(error.to_string()))?;
    validator
        .validate(&json_value)
        .map_err(|error| ManifestError::SchemaValidation {
            path: error.instance_path().as_str().to_string(),
            message: error.to_string(),
        })
}

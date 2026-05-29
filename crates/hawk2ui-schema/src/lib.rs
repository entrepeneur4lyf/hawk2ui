#![forbid(unsafe_code)]
//! Shared typed schema records for `Hawk2UI` products, manifests, artifacts, capabilities, and diagnostics.

pub mod product;

use serde::{Deserialize, Serialize};

pub use product::{
    HostTarget, ProductCapability, ProductModel, ProductModelError, SchemaValidationError,
    SurfaceKind,
};

/// Stable semantic version for the schema catalog document.
pub const SCHEMA_CATALOG_VERSION: &str = "1.0.0";

/// A generated JSON Schema entry in the central `Hawk2UI` schema catalog.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SchemaCatalogEntry {
    /// Stable schema identifier.
    pub id: String,
    /// Schema version for compatibility checks.
    pub version: String,
    /// Owning crate that defines the source record.
    pub owner: String,
    /// Generated JSON Schema document.
    pub schema: serde_json::Value,
}

/// Central generated schema catalog for production `Hawk2UI` boundaries.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SchemaCatalog {
    /// Catalog schema version.
    pub schema_version: String,
    /// Generated schema entries in deterministic order.
    pub schemas: Vec<SchemaCatalogEntry>,
}

/// Generates the JSON Schema for [`ProductModel`].
///
/// # Errors
///
/// Returns [`SchemaValidationError`] when the generated schema cannot be represented as JSON.
pub fn product_model_json_schema() -> Result<serde_json::Value, SchemaValidationError> {
    serde_json::to_value(schemars::schema_for!(ProductModel)).map_err(|error| {
        SchemaValidationError::new(
            "schema.product.generate-failed",
            format!("generated ProductModel schema could not be serialized: {error}"),
        )
    })
}

/// Validates a JSON value against the [`ProductModel`] contract.
///
/// Enforces both the generated JSON Schema (structure, types, `additionalProperties: false`) and
/// the model's semantic invariants — no duplicate members, and the required production surfaces
/// ([`ProductModel::validate_required_surfaces`]) — so a document accepted here is one the
/// in-process builder would also accept.
///
/// # Errors
///
/// Returns [`SchemaValidationError`] when the schema cannot be compiled, the value fails structural
/// validation, carries duplicate members, or is missing a required surface.
pub fn validate_product_model_json(value: &serde_json::Value) -> Result<(), SchemaValidationError> {
    let schema = product_model_json_schema()?;
    validate_json_value(&schema, value, "schema.product.invalid")?;

    // Structural validity is not full conformance: enforce the invariants the in-process builder
    // upholds, so the public JSON gate accepts only models the constructor would.
    let model: ProductModel = serde_json::from_value(value.clone()).map_err(|error| {
        SchemaValidationError::new(
            "schema.product.invalid",
            format!("product model document could not be decoded: {error}"),
        )
    })?;
    enforce_no_duplicate_members(&model)?;
    model.validate_required_surfaces().map_err(|error| {
        SchemaValidationError::new(
            "schema.product.surface.missing",
            format!("product model is missing a required surface: {error:?}"),
        )
    })
}

/// Rejects a product model carrying duplicate host targets, surfaces, or capabilities.
///
/// The `with_*` builders dedup, but `Deserialize` does not, so duplicates can survive the untrusted
/// JSON boundary and bypass the builder's set invariant; the public gate rejects them.
fn enforce_no_duplicate_members(model: &ProductModel) -> Result<(), SchemaValidationError> {
    if has_duplicates(&model.host_targets)
        || has_duplicates(&model.surface_kinds)
        || has_duplicates(&model.capabilities)
    {
        return Err(SchemaValidationError::new(
            "schema.product.duplicate-member",
            "product model contains duplicate host targets, surfaces, or capabilities",
        ));
    }
    Ok(())
}

fn has_duplicates<T: PartialEq>(items: &[T]) -> bool {
    items
        .iter()
        .enumerate()
        .any(|(index, item)| items[..index].contains(item))
}

/// Generates the central production schema catalog.
///
/// # Errors
///
/// Returns [`SchemaValidationError`] when any owning crate fails to generate its schema.
pub fn schema_catalog() -> Result<SchemaCatalog, SchemaValidationError> {
    Ok(SchemaCatalog {
        schema_version: SCHEMA_CATALOG_VERSION.into(),
        schemas: vec![
            schema_entry(
                "hawk2ui.product-model",
                "hawk2ui-schema",
                product_model_json_schema()?,
            ),
            schema_entry(
                "hawk2ui.raw-manifest",
                "hawk2ui-build",
                hawk2ui_build::HawkManifest::json_schema()
                    .map_err(|error| schema_error("schema.raw-manifest.generate-failed", error))?,
            ),
            schema_entry(
                "hawk2ui.sealed-artifact",
                "hawk2ui-build",
                hawk2ui_build::SealedArtifact::json_schema().map_err(|error| {
                    schema_error("schema.sealed-artifact.generate-failed", error)
                })?,
            ),
            schema_entry(
                "hawk2ui.plugin-format-target",
                "hawk2ui-plugin",
                hawk2ui_plugin::PluginFormatTarget::json_schema().map_err(|error| {
                    schema_error("schema.plugin-format-target.generate-failed", error)
                })?,
            ),
            schema_entry(
                "hawk2ui.package-plan",
                "hawk2ui-plugin-adapters",
                hawk2ui_plugin_adapters::PackagePlan::json_schema()
                    .map_err(|error| schema_error("schema.package-plan.generate-failed", error))?,
            ),
            schema_entry(
                "hawk2ui.materialized-package-output",
                "hawk2ui-plugin-adapters",
                hawk2ui_plugin_adapters::MaterializedPackageOutput::json_schema().map_err(
                    |error| {
                        schema_error("schema.materialized-package-output.generate-failed", error)
                    },
                )?,
            ),
            schema_entry(
                "hawk2ui.package-verification-report",
                "hawk2ui-plugin-adapters",
                hawk2ui_plugin_adapters::VerificationReport::json_schema().map_err(|error| {
                    schema_error("schema.package-verification-report.generate-failed", error)
                })?,
            ),
            schema_entry(
                "hawk2ui.capability-record",
                "hawk2ui-platform",
                hawk2ui_platform::CapabilityRecord::json_schema().map_err(|error| {
                    schema_error("schema.capability-record.generate-failed", error)
                })?,
            ),
            schema_entry(
                "hawk2ui.capability-table",
                "hawk2ui-platform",
                hawk2ui_platform::CapabilityTable::json_schema().map_err(|error| {
                    schema_error("schema.capability-table.generate-failed", error)
                })?,
            ),
        ],
    })
}

/// Serializes the central production schema catalog to JSON.
///
/// # Errors
///
/// Returns [`SchemaValidationError`] when generation or serialization fails.
pub fn schema_catalog_json() -> Result<serde_json::Value, SchemaValidationError> {
    serde_json::to_value(schema_catalog()?).map_err(|error| {
        SchemaValidationError::new(
            "schema.catalog.serialize-failed",
            format!("schema catalog could not be serialized: {error}"),
        )
    })
}

fn validate_json_value(
    schema: &serde_json::Value,
    value: &serde_json::Value,
    rule: &'static str,
) -> Result<(), SchemaValidationError> {
    let validator = jsonschema::Validator::new(schema).map_err(|error| {
        SchemaValidationError::new(
            "schema.compile.failed",
            format!("generated JSON Schema could not be compiled: {error}"),
        )
    })?;
    validator.validate(value).map_err(|error| {
        SchemaValidationError::new(
            rule,
            format!("JSON value failed schema validation: {error}"),
        )
    })
}

fn schema_entry(id: &str, owner: &str, schema: serde_json::Value) -> SchemaCatalogEntry {
    SchemaCatalogEntry {
        id: id.into(),
        version: SCHEMA_CATALOG_VERSION.into(),
        owner: owner.into(),
        schema,
    }
}

fn schema_error(rule: &'static str, error: impl std::fmt::Debug) -> SchemaValidationError {
    SchemaValidationError::new(rule, format!("{error:?}"))
}

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-schema";

/// Returns the canonical Cargo package name for diagnostics and conformance checks.
#[must_use]
pub const fn crate_name() -> &'static str {
    CRATE_NAME
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_identity() {
        assert_eq!(crate_name(), "hawk2ui-schema");
    }

    #[test]
    fn schema_catalog_contains_all_production_schema_entries() {
        let catalog = schema_catalog().expect("schema catalog generates");
        let ids = catalog
            .schemas
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<std::collections::BTreeSet<_>>();

        for id in [
            "hawk2ui.product-model",
            "hawk2ui.raw-manifest",
            "hawk2ui.sealed-artifact",
            "hawk2ui.plugin-format-target",
            "hawk2ui.package-plan",
            "hawk2ui.materialized-package-output",
            "hawk2ui.package-verification-report",
            "hawk2ui.capability-record",
            "hawk2ui.capability-table",
        ] {
            assert!(ids.contains(id), "schema catalog missing {id}");
        }

        let json = schema_catalog_json().expect("schema catalog serializes");
        assert_eq!(json["schema_version"], "1.0.0");
        assert!(json["schemas"].is_array());
    }
}

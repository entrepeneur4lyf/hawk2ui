#![forbid(unsafe_code)]
//! Shared typed schema records for `Hawk2UI` products, manifests, artifacts, capabilities, and diagnostics.

pub mod product;

pub use product::{
    HostTarget, ProductCapability, ProductModel, ProductModelError, SchemaValidationError,
    SurfaceKind,
};

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

/// Validates a JSON value against the generated [`ProductModel`] schema.
///
/// # Errors
///
/// Returns [`SchemaValidationError`] when the schema cannot be compiled or the value fails
/// validation.
pub fn validate_product_model_json(value: &serde_json::Value) -> Result<(), SchemaValidationError> {
    let schema = product_model_json_schema()?;
    validate_json_value(&schema, value, "schema.product.invalid")
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
}

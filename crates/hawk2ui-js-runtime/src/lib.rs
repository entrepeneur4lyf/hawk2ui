#![forbid(unsafe_code)]
//! Deno-core JavaScript runtime and native capability bridge for `Hawk2UI`.
//!
//! This crate owns the production JavaScript runtime path. It intentionally does
//! not depend on the legacy Boa-backed script runtime or framework compiler wire
//! format.

mod error;
mod extensions;
mod module_loader;
mod permissions;
mod runtime;
mod scene_ops;
mod scene_tree;
mod v8_artifacts;

pub use error::JsRuntimeError;
pub use module_loader::{HawkJsModule, HawkJsModuleGraph};
pub use permissions::{
    HawkAudioTransportInfo, HawkHostContext, HawkNetworkRequest, HawkNetworkResponse,
    HawkPluginTransportInfo, HawkRuntimeCapabilities,
};
pub use runtime::{HawkJsRuntime, JsRuntimeValue};
pub use scene_ops::{SceneMeasurementRequest, SceneNodeKind, SceneOp, SceneOpBatch, SceneValue};
pub use scene_tree::{RuntimeSceneOpAdapter, SceneAccessibilitySemantics};
pub use v8_artifacts::{RustyV8ArtifactSet, sha256_file};

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-js-runtime";

/// Returns the canonical Cargo package name for diagnostics and conformance checks.
#[must_use]
pub const fn crate_name() -> &'static str {
    CRATE_NAME
}

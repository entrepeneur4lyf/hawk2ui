//! Safety limits for authoring tree traversal.

/// Maximum accepted depth for authoring and framework-emitted trees.
///
/// The cap is intentionally high enough for real UI trees while preventing stack
/// exhaustion at framework/compiler trust boundaries.
pub const MAX_AUTHORING_TREE_DEPTH: usize = 512;

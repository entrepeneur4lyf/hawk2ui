//! Scoped filesystem API records.

use crate::PlatformDiagnostic;

/// Filesystem access scope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FilesystemScope {
    /// Project asset bundle.
    ProjectAssets,
    /// Application data directory.
    AppData,
    /// Application cache directory.
    CacheData,
    /// User-selected file grant.
    UserSelectedFile,
    /// Plugin preset storage.
    PluginPresetStorage,
    /// Explicitly forbidden paths.
    Forbidden,
}

/// Filesystem grant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilesystemGrant {
    /// Scope for the grant.
    pub scope: FilesystemScope,
    /// Root path or exact path for user-selected files.
    pub root: String,
}

impl FilesystemGrant {
    /// Creates a filesystem grant.
    #[must_use]
    pub fn new(scope: FilesystemScope, root: impl Into<String>) -> Self {
        Self {
            scope,
            root: root.into(),
        }
    }
}

/// Resolved filesystem access.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilesystemAccess {
    /// Scope used for the access.
    pub scope: FilesystemScope,
    /// Resolved path.
    pub resolved_path: String,
}

/// Filesystem access denial.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilesystemDenied {
    /// Attempted path.
    pub path: String,
    /// Structured diagnostic.
    pub diagnostic: PlatformDiagnostic,
}

/// Scoped filesystem policy.
pub struct FilesystemPolicy;

impl FilesystemPolicy {
    /// Resolves a relative path against a filesystem grant.
    ///
    /// # Errors
    ///
    /// Returns [`FilesystemDenied`] when the path escapes scope or targets a forbidden scope.
    pub fn resolve(
        grant: &FilesystemGrant,
        relative_path: &str,
    ) -> Result<FilesystemAccess, FilesystemDenied> {
        if grant.scope == FilesystemScope::Forbidden {
            return Err(FilesystemDenied {
                path: relative_path.into(),
                diagnostic: PlatformDiagnostic::error(
                    "filesystem.path.forbidden",
                    "filesystem path is forbidden",
                ),
            });
        }
        if is_escaping(relative_path) {
            return Err(FilesystemDenied {
                path: relative_path.into(),
                diagnostic: PlatformDiagnostic::error(
                    "filesystem.path.escape",
                    "filesystem path escapes its scope",
                ),
            });
        }
        Ok(FilesystemAccess {
            scope: grant.scope,
            resolved_path: join_scope_path(&grant.root, relative_path),
        })
    }

    /// Creates a user-selected file grant.
    #[must_use]
    pub fn user_selected_file(path: impl Into<String>) -> FilesystemGrant {
        FilesystemGrant::new(FilesystemScope::UserSelectedFile, path)
    }

    /// Resolves an exact user-selected file grant.
    ///
    /// # Errors
    ///
    /// Returns [`FilesystemDenied`] when the requested path differs from the grant.
    pub fn resolve_user_selected(
        grant: &FilesystemGrant,
        requested_path: &str,
    ) -> Result<FilesystemAccess, FilesystemDenied> {
        if grant.scope != FilesystemScope::UserSelectedFile || grant.root != requested_path {
            return Err(FilesystemDenied {
                path: requested_path.into(),
                diagnostic: PlatformDiagnostic::error(
                    "filesystem.user-grant.denied",
                    "filesystem path is outside the user-selected grant",
                ),
            });
        }
        Ok(FilesystemAccess {
            scope: FilesystemScope::UserSelectedFile,
            resolved_path: requested_path.into(),
        })
    }
}

fn is_escaping(path: &str) -> bool {
    path.starts_with('/') || path.split('/').any(|segment| segment == "..")
}

fn join_scope_path(root: &str, relative_path: &str) -> String {
    format!("{}/{}", root.trim_end_matches('/'), relative_path)
}

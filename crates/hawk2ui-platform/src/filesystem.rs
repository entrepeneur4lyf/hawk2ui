//! Scoped filesystem API records.

use crate::PlatformDiagnostic;
use std::path::{Component, Path, PathBuf};

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
        let scoped_path =
            validate_relative_path(relative_path).ok_or_else(|| FilesystemDenied {
                path: relative_path.into(),
                diagnostic: PlatformDiagnostic::error(
                    "filesystem.path.escape",
                    "filesystem path escapes its scope",
                ),
            })?;
        if !is_valid_scope_root(&grant.root) {
            return Err(FilesystemDenied {
                path: relative_path.into(),
                diagnostic: PlatformDiagnostic::error(
                    "filesystem.root.invalid",
                    "filesystem grant root is invalid",
                ),
            });
        }
        Ok(FilesystemAccess {
            scope: grant.scope,
            resolved_path: join_scope_path(&grant.root, &scoped_path),
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

fn validate_relative_path(path: &str) -> Option<PathBuf> {
    if path.is_empty()
        || path.contains('\0')
        || path.contains('\\')
        || path.contains(':')
        || Path::new(path).is_absolute()
    {
        return None;
    }

    let mut normalized = PathBuf::new();
    for component in Path::new(path).components() {
        match component {
            Component::Normal(segment) => normalized.push(segment),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }

    if normalized.as_os_str().is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn is_valid_scope_root(root: &str) -> bool {
    !root.is_empty() && !root.contains('\0')
}

fn join_scope_path(root: &str, relative_path: &Path) -> String {
    Path::new(root)
        .join(relative_path)
        .to_string_lossy()
        .into_owned()
}

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
        let root = Path::new(&grant.root);
        let resolved_path =
            resolve_scoped_path(root, &scoped_path).ok_or_else(|| FilesystemDenied {
                path: relative_path.into(),
                diagnostic: PlatformDiagnostic::error(
                    "filesystem.path.escape",
                    "filesystem path escapes its scope",
                ),
            })?;
        Ok(FilesystemAccess {
            scope: grant.scope,
            resolved_path,
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
        if !is_valid_scope_root(&grant.root) {
            return Err(FilesystemDenied {
                path: requested_path.into(),
                diagnostic: PlatformDiagnostic::error(
                    "filesystem.user-grant.invalid",
                    "filesystem user-selected grant is invalid",
                ),
            });
        }
        if grant.scope != FilesystemScope::UserSelectedFile
            || !same_user_selected_path(&grant.root, requested_path)
        {
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
            resolved_path: canonical_or_original(requested_path),
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
    if root.is_empty() || root.contains('\0') || root.contains('\\') {
        return false;
    }
    let root = Path::new(root);
    root.is_absolute()
        && root.components().all(|component| {
            matches!(
                component,
                Component::RootDir | Component::Normal(_) | Component::CurDir
            )
        })
}

fn resolve_scoped_path(root: &Path, relative_path: &Path) -> Option<String> {
    let root_canonical = root.canonicalize().ok();
    let candidate = root.join(relative_path);
    let Some(root_canonical) = root_canonical else {
        return Some(candidate.to_string_lossy().into_owned());
    };
    let resolved = canonicalize_existing_prefix(&candidate)?;
    resolved
        .starts_with(&root_canonical)
        .then(|| resolved.to_string_lossy().into_owned())
}

fn canonicalize_existing_prefix(path: &Path) -> Option<PathBuf> {
    if path.exists() {
        return path.canonicalize().ok();
    }

    let mut missing = Vec::new();
    let mut current = path;
    while !current.exists() {
        let file_name = current.file_name()?.to_owned();
        missing.push(file_name);
        current = current.parent()?;
    }

    let mut canonical = current.canonicalize().ok()?;
    for component in missing.iter().rev() {
        canonical.push(component);
    }
    Some(canonical)
}

fn same_user_selected_path(granted_path: &str, requested_path: &str) -> bool {
    if granted_path == requested_path {
        return true;
    }
    let granted = Path::new(granted_path).canonicalize();
    let requested = Path::new(requested_path).canonicalize();
    matches!((granted, requested), (Ok(granted), Ok(requested)) if granted == requested)
}

fn canonical_or_original(path: &str) -> String {
    Path::new(path)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(path))
        .to_string_lossy()
        .into_owned()
}

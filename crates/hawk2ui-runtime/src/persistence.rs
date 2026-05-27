//! Runtime state persistence records and deterministic restore store.

use std::{
    collections::BTreeMap,
    path::{Component, Path},
};

use serde::{Deserialize, Serialize};

use crate::StructuredValue;

/// Runtime persistence scope.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum RuntimeStateScope {
    /// Application-level state.
    App,
    /// UI-only preferences.
    UiPreferences,
    /// Host-automatable plugin parameter state.
    PluginParameter,
    /// Non-parameter plugin state.
    PluginNonParameter,
    /// User-authored preset state.
    UserPreset,
}

/// One scoped runtime state entry.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeStateEntry {
    /// State scope.
    pub scope: RuntimeStateScope,
    /// Stable state key.
    pub key: String,
    /// Structured state value.
    pub value: StructuredValue,
}

impl RuntimeStateEntry {
    /// Creates a runtime state entry.
    #[must_use]
    pub fn new(scope: RuntimeStateScope, key: impl Into<String>, value: StructuredValue) -> Self {
        Self {
            scope,
            key: key.into(),
            value,
        }
    }
}

/// Host-specific opaque state chunk.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeHostStateChunk {
    /// Host format or storage key.
    pub format: String,
    /// Opaque host bytes.
    pub bytes: Vec<u8>,
}

impl RuntimeHostStateChunk {
    /// Creates a host state chunk.
    #[must_use]
    pub fn new(format: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            format: format.into(),
            bytes: bytes.into(),
        }
    }
}

/// Versioned runtime state snapshot.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeStateSnapshot {
    /// Snapshot schema version.
    pub schema_version: u32,
    entries: BTreeMap<(RuntimeStateScope, String), StructuredValue>,
    host_chunks: Vec<RuntimeHostStateChunk>,
}

impl RuntimeStateSnapshot {
    /// Creates an empty runtime state snapshot.
    #[must_use]
    pub fn new(schema_version: u32) -> Self {
        Self {
            schema_version,
            entries: BTreeMap::new(),
            host_chunks: Vec::new(),
        }
    }

    /// Adds or replaces a scoped state entry.
    #[must_use]
    pub fn with_entry(mut self, entry: RuntimeStateEntry) -> Self {
        self.entries.insert((entry.scope, entry.key), entry.value);
        self
    }

    /// Adds an opaque host state chunk.
    #[must_use]
    pub fn with_host_chunk(mut self, format: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        self.host_chunks
            .push(RuntimeHostStateChunk::new(format, bytes));
        self
    }

    /// Returns a scoped state entry.
    #[must_use]
    pub fn entry(&self, scope: RuntimeStateScope, key: &str) -> Option<&StructuredValue> {
        self.entries.get(&(scope, key.to_string()))
    }

    /// Returns host state chunks.
    #[must_use]
    pub fn host_chunks(&self) -> &[RuntimeHostStateChunk] {
        &self.host_chunks
    }

    /// Applies migrations in order.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeStatePersistenceError`] when a migration does not match the current schema version.
    pub fn migrate(
        mut self,
        migrations: impl IntoIterator<Item = RuntimeStateMigration>,
    ) -> Result<Self, RuntimeStatePersistenceError> {
        for migration in migrations {
            if migration.from_version != self.schema_version {
                return Err(RuntimeStatePersistenceError::new(
                    "state.migration-version-mismatch",
                    format!(
                        "migration expected version {}, snapshot has {}",
                        migration.from_version, self.schema_version
                    ),
                ));
            }
            match migration.kind {
                RuntimeStateMigrationKind::RenameKey { scope, from, to } => {
                    if let Some(value) = self.entries.remove(&(scope, from)) {
                        self.entries.insert((scope, to), value);
                    }
                }
            }
            self.schema_version = migration.to_version;
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum RuntimeStateMigrationKind {
    RenameKey {
        scope: RuntimeStateScope,
        from: String,
        to: String,
    },
}

/// Runtime state migration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeStateMigration {
    /// Source schema version.
    pub from_version: u32,
    /// Destination schema version.
    pub to_version: u32,
    kind: RuntimeStateMigrationKind,
}

impl RuntimeStateMigration {
    /// Creates a key rename migration for one state scope.
    #[must_use]
    pub fn rename_key(
        from_version: u32,
        to_version: u32,
        scope: RuntimeStateScope,
        from: impl Into<String>,
        to: impl Into<String>,
    ) -> Self {
        Self {
            from_version,
            to_version,
            kind: RuntimeStateMigrationKind::RenameKey {
                scope,
                from: from.into(),
                to: to.into(),
            },
        }
    }
}

/// Runtime storage root path.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeStoragePath {
    root: String,
}

impl RuntimeStoragePath {
    /// Creates a user-data storage root.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeStatePersistenceError`] when the path is not an absolute, normalized storage root.
    pub fn user_data(root: impl Into<String>) -> Result<Self, RuntimeStatePersistenceError> {
        let root = root.into();
        if !is_valid_storage_root(&root) {
            return Err(RuntimeStatePersistenceError::new(
                "state.storage-root.invalid",
                "runtime storage root must be absolute and normalized",
            ));
        }
        Ok(Self { root })
    }

    /// Returns the storage root.
    #[must_use]
    pub fn root(&self) -> &str {
        &self.root
    }
}

/// Runtime persistence error.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeStatePersistenceError {
    /// Stable error code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
}

impl RuntimeStatePersistenceError {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

/// Deterministic runtime persistence store.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimePersistenceStore {
    storage_path: RuntimeStoragePath,
    snapshots: BTreeMap<String, RuntimeStateSnapshot>,
}

impl RuntimePersistenceStore {
    /// Creates a runtime persistence store rooted at a validated path.
    #[must_use]
    pub fn new(storage_path: RuntimeStoragePath) -> Self {
        Self {
            storage_path,
            snapshots: BTreeMap::new(),
        }
    }

    /// Saves a state snapshot by stable identity.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeStatePersistenceError`] when the identity is not stable.
    pub fn save(
        &mut self,
        identity: &str,
        snapshot: RuntimeStateSnapshot,
    ) -> Result<(), RuntimeStatePersistenceError> {
        validate_stable_key(identity, "state.identity.invalid")?;
        self.snapshots.insert(identity.into(), snapshot);
        Ok(())
    }

    /// Restores a state snapshot by stable identity.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeStatePersistenceError`] when the identity is invalid or missing.
    pub fn restore(
        &self,
        identity: &str,
    ) -> Result<RuntimeStateSnapshot, RuntimeStatePersistenceError> {
        validate_stable_key(identity, "state.identity.invalid")?;
        self.snapshots.get(identity).cloned().ok_or_else(|| {
            RuntimeStatePersistenceError::new(
                "state.snapshot.missing",
                format!("runtime state snapshot is missing: {identity}"),
            )
        })
    }

    /// Materializes a user preset path under the store root.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeStatePersistenceError`] when the plugin or preset identifier is invalid.
    pub fn user_preset_path(
        &self,
        plugin_id: &str,
        preset_id: &str,
    ) -> Result<String, RuntimeStatePersistenceError> {
        validate_stable_key(plugin_id, "state.plugin-id.invalid")?;
        validate_stable_key(preset_id, "state.preset-id.invalid")?;
        Ok(format!(
            "{}/presets/{plugin_id}/{preset_id}.hawkstate",
            self.storage_path.root()
        ))
    }
}

fn validate_stable_key(key: &str, code: &str) -> Result<(), RuntimeStatePersistenceError> {
    if is_stable_key(key) {
        Ok(())
    } else {
        Err(RuntimeStatePersistenceError::new(
            code,
            "state key must be a stable identifier",
        ))
    }
}

fn is_stable_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
}

fn is_valid_storage_root(root: &str) -> bool {
    if root.is_empty()
        || root.contains('\0')
        || root.contains('\\')
        || root.contains("/./")
        || root.ends_with("/.")
    {
        return false;
    }
    let root = Path::new(root);
    root.is_absolute()
        && root
            .components()
            .all(|component| matches!(component, Component::RootDir | Component::Normal(_)))
}

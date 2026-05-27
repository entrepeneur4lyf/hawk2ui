//! Capability-scoped database API records.

use std::collections::BTreeSet;

use crate::{FilesystemGrant, FilesystemPolicy, PlatformDiagnostic};

/// Database migration record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DatabaseMigration {
    /// Monotonic migration version.
    pub version: u32,
    /// Stable migration ID.
    pub id: String,
}

impl DatabaseMigration {
    /// Creates a database migration record.
    #[must_use]
    pub fn new(version: u32, id: impl Into<String>) -> Self {
        Self {
            version,
            id: id.into(),
        }
    }
}

/// Database API denial.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DatabaseDenied {
    /// Structured diagnostic.
    pub diagnostic: PlatformDiagnostic,
}

/// Database API policy.
pub struct DatabasePolicy;

impl DatabasePolicy {
    /// Validates that migrations are strictly increasing.
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseDenied`] when migration versions are not strictly increasing.
    pub fn validate_migrations(migrations: &[DatabaseMigration]) -> Result<(), DatabaseDenied> {
        let mut ids = BTreeSet::new();
        for migration in migrations {
            if !is_valid_migration_id(&migration.id) || !ids.insert(migration.id.as_str()) {
                return Err(DatabaseDenied {
                    diagnostic: PlatformDiagnostic::error(
                        "database.migration.id-invalid",
                        "database migration IDs must be unique stable identifiers",
                    ),
                });
            }
        }
        for window in migrations.windows(2) {
            if window[0].version >= window[1].version {
                return Err(DatabaseDenied {
                    diagnostic: PlatformDiagnostic::error(
                        "database.migration.order",
                        "database migrations must be strictly increasing",
                    ),
                });
            }
        }
        Ok(())
    }

    /// Validates a database storage path against a filesystem grant.
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseDenied`] when storage resolution is unsafe.
    pub fn validate_storage_path(
        grant: &FilesystemGrant,
        relative_path: &str,
    ) -> Result<(), DatabaseDenied> {
        FilesystemPolicy::resolve(grant, relative_path)
            .map(|_| ())
            .map_err(|_| DatabaseDenied {
                diagnostic: PlatformDiagnostic::error(
                    "database.storage.unsafe",
                    "database storage path is unsafe",
                ),
            })
    }
}

fn is_valid_migration_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
}

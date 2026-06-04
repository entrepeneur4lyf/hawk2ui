//! Deterministic fixture catalogs and temporary project helpers.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{FixtureKind, TestFixture};

/// Error returned by fixture catalog lookups.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FixtureCatalogError {
    /// The requested fixture name is not registered.
    MissingFixture(String),
    /// The fixture exists but has a different kind.
    KindMismatch {
        /// Requested fixture name.
        name: String,
        /// Expected fixture kind.
        expected: FixtureKind,
        /// Actual fixture kind.
        actual: FixtureKind,
    },
}

/// Deterministic fixture catalog shared by domain tests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixtureCatalog {
    fixtures: Vec<TestFixture>,
}

impl FixtureCatalog {
    /// Creates a fixture catalog.
    #[must_use]
    pub fn new(fixtures: impl IntoIterator<Item = TestFixture>) -> Self {
        let mut fixtures = fixtures.into_iter().collect::<Vec<_>>();
        fixtures.sort_by(|left, right| left.name().cmp(right.name()));
        Self { fixtures }
    }

    /// Returns all registered fixtures in deterministic order.
    #[must_use]
    pub fn fixtures(&self) -> &[TestFixture] {
        &self.fixtures
    }

    /// Requires a fixture by name.
    ///
    /// # Errors
    ///
    /// Returns [`FixtureCatalogError::MissingFixture`] when no fixture has the
    /// requested name.
    pub fn require(&self, name: &str) -> Result<&TestFixture, FixtureCatalogError> {
        self.fixtures
            .iter()
            .find(|fixture| fixture.name() == name)
            .ok_or_else(|| FixtureCatalogError::MissingFixture(name.to_string()))
    }

    /// Requires a fixture by name and kind.
    ///
    /// # Errors
    ///
    /// Returns [`FixtureCatalogError`] when the fixture is missing or has a
    /// different kind.
    pub fn require_kind(
        &self,
        name: &str,
        kind: FixtureKind,
    ) -> Result<&TestFixture, FixtureCatalogError> {
        let fixture = self.require(name)?;
        if fixture.kind() == kind {
            Ok(fixture)
        } else {
            Err(FixtureCatalogError::KindMismatch {
                name: name.to_string(),
                expected: kind,
                actual: fixture.kind(),
            })
        }
    }
}

/// Temporary project directory removed when dropped.
#[derive(Debug)]
pub struct TempProject {
    root: PathBuf,
}

impl TempProject {
    /// Creates a new temporary project with a unique directory name.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the project directory cannot be created.
    pub fn new(prefix: &str) -> io::Result<Self> {
        let root = std::env::temp_dir().join(unique_name(prefix));
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    /// Returns the project root path.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Writes a UTF-8 file relative to the project root, creating parent directories.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the relative path escapes the project root or
    /// a parent directory or file cannot be written.
    pub fn write_file(&self, relative_path: impl AsRef<Path>, contents: &str) -> io::Result<()> {
        let path = self.resolve_relative(relative_path)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, contents)
    }

    /// Reads a UTF-8 file relative to the project root.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the relative path escapes the project root or
    /// the file cannot be read.
    pub fn read_to_string(&self, relative_path: impl AsRef<Path>) -> io::Result<String> {
        fs::read_to_string(self.resolve_relative(relative_path)?)
    }

    fn resolve_relative(&self, relative_path: impl AsRef<Path>) -> io::Result<PathBuf> {
        let relative_path = relative_path.as_ref();
        if relative_path.is_absolute()
            || relative_path.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "temporary project path must stay relative to the root: {}",
                    relative_path.display()
                ),
            ));
        }
        Ok(self.root.join(relative_path))
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn unique_name(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    format!("hawk2ui-{prefix}-{}-{nanos}", std::process::id())
}

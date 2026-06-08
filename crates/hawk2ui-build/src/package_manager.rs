//! Package-manager detection and command metadata for sealed build inputs.

use std::{
    error::Error,
    fmt::{self, Write as _},
    fs,
    path::{Path, PathBuf},
};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Supported JavaScript package managers.
#[derive(
    Clone, Copy, Debug, Deserialize, Eq, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "lowercase")]
pub enum PackageManagerKind {
    /// Bun package manager.
    Bun,
    /// npm package manager.
    Npm,
    /// pnpm package manager.
    Pnpm,
    /// Yarn package manager.
    Yarn,
}

impl PackageManagerKind {
    /// Returns the executable name used for command specs and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bun => "bun",
            Self::Npm => "npm",
            Self::Pnpm => "pnpm",
            Self::Yarn => "yarn",
        }
    }

    const fn lockfile_name(self) -> &'static str {
        match self {
            Self::Bun => "bun.lock",
            Self::Npm => "package-lock.json",
            Self::Pnpm => "pnpm-lock.yaml",
            Self::Yarn => "yarn.lock",
        }
    }

    fn install_args(self) -> Vec<&'static str> {
        match self {
            Self::Bun | Self::Pnpm => vec!["install", "--frozen-lockfile"],
            Self::Npm => vec!["ci"],
            Self::Yarn => vec!["install", "--immutable"],
        }
    }
}

/// Process command specification produced by package-manager selection.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PackageManagerCommand {
    program: String,
    args: Vec<String>,
}

impl PackageManagerCommand {
    fn new(kind: PackageManagerKind, args: Vec<&'static str>) -> Self {
        Self {
            program: kind.as_str().into(),
            args: args.into_iter().map(str::to_owned).collect(),
        }
    }

    /// Returns the executable program.
    #[must_use]
    pub fn program(&self) -> &str {
        &self.program
    }

    /// Returns command arguments.
    #[must_use]
    pub fn args(&self) -> &[String] {
        &self.args
    }
}

/// Package-manager metadata suitable for sealed build output.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PackageManagerMetadata {
    /// Selected package manager.
    pub kind: PackageManagerKind,
    /// Selected lockfile path, when present in the workspace.
    pub lockfile_path: Option<PathBuf>,
    /// SHA-256 hex digest of the selected lockfile bytes, when present.
    pub lockfile_sha256: Option<String>,
    /// Deterministic install command for this package manager.
    pub install_command: PackageManagerCommand,
    /// Deterministic build command for this package manager.
    pub build_command: PackageManagerCommand,
    /// Version command used to record package-manager version evidence.
    pub version_command: PackageManagerCommand,
    /// Resolved package-manager version captured from [`Self::version_command`].
    pub package_manager_version: Option<String>,
}

impl PackageManagerMetadata {
    /// Creates package-manager metadata for sealed build output.
    #[must_use]
    pub fn new(
        kind: PackageManagerKind,
        lockfile_path: Option<PathBuf>,
        lockfile_sha256: Option<String>,
    ) -> Self {
        Self {
            kind,
            lockfile_path,
            lockfile_sha256,
            install_command: PackageManagerCommand::new(kind, kind.install_args()),
            build_command: PackageManagerCommand::new(kind, vec!["run", "build"]),
            version_command: PackageManagerCommand::new(kind, vec!["--version"]),
            package_manager_version: None,
        }
    }

    /// Records resolved package-manager version evidence.
    #[must_use]
    pub fn with_package_manager_version(mut self, version: impl AsRef<str>) -> Self {
        self.package_manager_version = Some(version.as_ref().trim().to_owned());
        self
    }
}

/// Selected package manager and reproducibility metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageManagerSelection {
    kind: PackageManagerKind,
    lockfile_path: Option<PathBuf>,
    lockfile_sha256: Option<String>,
}

impl PackageManagerSelection {
    /// Detects the package manager from lockfiles or an explicit manifest selection.
    ///
    /// # Errors
    ///
    /// Returns [`PackageManagerError`] when lockfile detection is missing, ambiguous,
    /// or the selected lockfile cannot be read.
    pub fn detect(
        root: impl AsRef<Path>,
        explicit: Option<PackageManagerKind>,
    ) -> Result<Self, PackageManagerError> {
        let root = root.as_ref();
        let detected = detect_lockfiles(root);
        let kind = match explicit {
            Some(kind) => kind,
            None => match detected.as_slice() {
                [] => {
                    return Err(PackageManagerError::new(
                        "build.package-manager.missing",
                        "no supported package-manager lockfile was found; expected one of bun.lock, package-lock.json, pnpm-lock.yaml, or yarn.lock",
                    ));
                }
                [(kind, _)] => *kind,
                _ => {
                    return Err(PackageManagerError::new(
                        "build.package-manager.ambiguous",
                        format!(
                            "multiple package-manager lockfiles were found: {}",
                            detected
                                .iter()
                                .map(|(kind, path)| {
                                    let lockfile = path
                                        .file_name()
                                        .and_then(|name| name.to_str())
                                        .unwrap_or_else(|| kind.lockfile_name());
                                    format!("{} ({lockfile})", kind.as_str())
                                })
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                    ));
                }
            },
        };

        let lockfile_path = detected
            .iter()
            .find_map(|(detected_kind, path)| (*detected_kind == kind).then(|| path.clone()));
        let lockfile_sha256 = lockfile_path.as_deref().map(lockfile_sha256).transpose()?;

        Ok(Self {
            kind,
            lockfile_path,
            lockfile_sha256,
        })
    }

    /// Returns selected package manager.
    #[must_use]
    pub const fn kind(&self) -> PackageManagerKind {
        self.kind
    }

    /// Returns selected lockfile path.
    #[must_use]
    pub fn lockfile_path(&self) -> Option<&Path> {
        self.lockfile_path.as_deref()
    }

    /// Returns selected lockfile SHA-256 hex digest.
    #[must_use]
    pub fn lockfile_sha256(&self) -> Option<&str> {
        self.lockfile_sha256.as_deref()
    }

    /// Returns reproducibility metadata for build output.
    #[must_use]
    pub fn metadata(&self) -> PackageManagerMetadata {
        PackageManagerMetadata::new(
            self.kind,
            self.lockfile_path.clone(),
            self.lockfile_sha256.clone(),
        )
    }

    /// Returns the deterministic install command.
    #[must_use]
    pub fn install_command(&self) -> PackageManagerCommand {
        PackageManagerCommand::new(self.kind, self.kind.install_args())
    }

    /// Returns the package build command.
    #[must_use]
    pub fn build_command(&self) -> PackageManagerCommand {
        PackageManagerCommand::new(self.kind, vec!["run", "build"])
    }

    /// Returns the package-manager version command.
    #[must_use]
    pub fn version_command(&self) -> PackageManagerCommand {
        PackageManagerCommand::new(self.kind, vec!["--version"])
    }
}

/// Package-manager detection error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageManagerError {
    rule: String,
    message: String,
}

impl PackageManagerError {
    fn new(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            rule: rule.into(),
            message: message.into(),
        }
    }

    /// Returns the stable diagnostic rule.
    #[must_use]
    pub fn rule(&self) -> &str {
        &self.rule
    }

    /// Returns the human-readable diagnostic message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for PackageManagerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.rule, self.message)
    }
}

impl Error for PackageManagerError {}

fn detect_lockfiles(root: &Path) -> Vec<(PackageManagerKind, PathBuf)> {
    [
        PackageManagerKind::Bun,
        PackageManagerKind::Npm,
        PackageManagerKind::Pnpm,
        PackageManagerKind::Yarn,
    ]
    .into_iter()
    .filter_map(|kind| {
        let path = root.join(kind.lockfile_name());
        path.is_file().then_some((kind, path))
    })
    .collect()
}

fn lockfile_sha256(path: &Path) -> Result<String, PackageManagerError> {
    let bytes = fs::read(path).map_err(|error| {
        PackageManagerError::new(
            "build.package-manager.lockfile-unreadable",
            format!("package-manager lockfile could not be read: {error}"),
        )
    })?;
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(encoded, "{byte:02x}");
    }
    Ok(encoded)
}

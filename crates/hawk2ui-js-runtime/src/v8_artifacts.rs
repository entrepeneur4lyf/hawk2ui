use std::{
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

use crate::JsRuntimeError;

/// Verified upstream `rusty_v8` artifact pair for a target platform.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustyV8ArtifactSet {
    target: String,
    profile: String,
    archive_path: PathBuf,
    archive_sha256: String,
    binding_path: PathBuf,
    binding_sha256: String,
}

impl RustyV8ArtifactSet {
    /// Creates a `rusty_v8` artifact set declaration.
    #[must_use]
    pub fn new(
        target: impl Into<String>,
        profile: impl Into<String>,
        archive_path: impl Into<PathBuf>,
        archive_sha256: impl Into<String>,
        binding_path: impl Into<PathBuf>,
        binding_sha256: impl Into<String>,
    ) -> Self {
        Self {
            target: target.into(),
            profile: profile.into(),
            archive_path: archive_path.into(),
            archive_sha256: archive_sha256.into(),
            binding_path: binding_path.into(),
            binding_sha256: binding_sha256.into(),
        }
    }

    /// Rejects attempts to use a V8 source-build fallback.
    ///
    /// # Errors
    ///
    /// Returns [`JsRuntimeError`] when a source build was requested.
    pub fn reject_source_build_request(source_build_requested: bool) -> Result<(), JsRuntimeError> {
        if source_build_requested {
            return Err(JsRuntimeError::new(
                "js-runtime.v8-artifact.source-build-unsupported",
                "V8 source builds are unsupported; use verified prebuilt rusty_v8 artifacts",
            ));
        }
        Ok(())
    }

    /// Returns the target triple for this artifact set.
    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }

    /// Returns the build profile for this artifact set.
    #[must_use]
    pub fn profile(&self) -> &str {
        &self.profile
    }

    /// Returns the static archive path.
    #[must_use]
    pub fn archive_path(&self) -> &Path {
        &self.archive_path
    }

    /// Returns the generated binding path.
    #[must_use]
    pub fn binding_path(&self) -> &Path {
        &self.binding_path
    }

    /// Verifies that the artifact pair exists, matches the target/profile naming
    /// contract, and has the expected SHA-256 digests.
    ///
    /// # Errors
    ///
    /// Returns [`JsRuntimeError`] when either artifact is missing, named for the
    /// wrong target/profile, unreadable, or hash-mismatched.
    pub fn verify(&self) -> Result<(), JsRuntimeError> {
        self.verify_names()?;
        verify_file_hash(&self.archive_path, &self.archive_sha256, "archive")?;
        verify_file_hash(&self.binding_path, &self.binding_sha256, "binding")?;
        Ok(())
    }

    fn verify_names(&self) -> Result<(), JsRuntimeError> {
        let expected_archive = format!("librusty_v8_{}_{}.a.gz", self.profile, self.target);
        let expected_binding = format!("src_binding_{}_{}.rs", self.profile, self.target);

        verify_file_name(&self.archive_path, &expected_archive, "archive")?;
        verify_file_name(&self.binding_path, &expected_binding, "binding")?;
        Ok(())
    }
}

/// Computes the lower-case SHA-256 digest for a file.
///
/// # Errors
///
/// Returns [`io::Error`] when the file cannot be opened or read.
pub fn sha256_file(path: impl AsRef<Path>) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];

    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(hex_lower(&hasher.finalize()))
}

fn verify_file_name(path: &Path, expected: &str, kind: &str) -> Result<(), JsRuntimeError> {
    let actual = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            JsRuntimeError::new(
                "js-runtime.v8-artifact.invalid-name",
                format!(
                    "rusty_v8 {kind} path has no valid file name: {}",
                    path.display()
                ),
            )
        })?;

    if actual != expected {
        return Err(JsRuntimeError::new(
            "js-runtime.v8-artifact.invalid-name",
            format!("rusty_v8 {kind} artifact must be named {expected}, got {actual}"),
        ));
    }

    Ok(())
}

fn verify_file_hash(path: &Path, expected: &str, kind: &str) -> Result<(), JsRuntimeError> {
    if !path.is_file() {
        return Err(JsRuntimeError::new(
            "js-runtime.v8-artifact.missing",
            format!("rusty_v8 {kind} artifact is missing: {}", path.display()),
        ));
    }

    let actual = sha256_file(path).map_err(|error| {
        JsRuntimeError::new(
            "js-runtime.v8-artifact.unreadable",
            format!("rusty_v8 {kind} artifact cannot be read: {error}"),
        )
    })?;

    if !expected.eq_ignore_ascii_case(&actual) {
        return Err(JsRuntimeError::new(
            "js-runtime.v8-artifact.hash-mismatch",
            format!("rusty_v8 {kind} artifact hash mismatch: expected {expected}, got {actual}"),
        ));
    }

    Ok(())
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

//! Capability-scoped audio playback API records.

use crate::{CapabilityTable, PlatformContext, PlatformDiagnostic, PlatformOperation};

/// Audio playback manifest declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AudioManifest {
    /// Required capability key.
    pub capability_key: String,
    /// Allowed audio cue identifiers.
    pub allowed_cues: Vec<String>,
}

impl AudioManifest {
    /// Creates an audio manifest declaration.
    #[must_use]
    pub fn new(
        capability_key: impl Into<String>,
        allowed_cues: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            capability_key: capability_key.into(),
            allowed_cues: allowed_cues.into_iter().map(Into::into).collect(),
        }
    }
}

/// Allowed audio playback request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AudioPlaybackRequest {
    /// Audio cue identifier.
    pub cue_id: String,
}

/// Audio playback denial.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AudioDenied {
    /// Audio cue identifier.
    pub cue_id: String,
    /// Structured diagnostic.
    pub diagnostic: PlatformDiagnostic,
}

/// Capability-scoped audio policy.
pub struct AudioPolicy;

impl AudioPolicy {
    /// Validates audio playback against capabilities and declared cue IDs.
    ///
    /// # Errors
    ///
    /// Returns [`AudioDenied`] when the capability or cue ID is denied.
    pub fn request(
        capabilities: &CapabilityTable,
        manifest: &AudioManifest,
        cue_id: &str,
        context: PlatformContext,
    ) -> Result<AudioPlaybackRequest, AudioDenied> {
        capabilities
            .ensure_allowed(
                &manifest.capability_key,
                PlatformOperation::AudioPlayback,
                context,
            )
            .map_err(|denial| AudioDenied {
                cue_id: cue_id.into(),
                diagnostic: denial.diagnostic,
            })?;
        if !is_declared(&manifest.allowed_cues, cue_id) {
            return Err(AudioDenied {
                cue_id: cue_id.into(),
                diagnostic: PlatformDiagnostic::error(
                    "audio.cue.denied",
                    format!("audio cue is not declared: {cue_id}"),
                ),
            });
        }
        Ok(AudioPlaybackRequest {
            cue_id: cue_id.into(),
        })
    }
}

fn is_declared(allowed: &[String], value: &str) -> bool {
    is_stable_id(value) && allowed.iter().any(|entry| entry == value)
}

fn is_stable_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
}

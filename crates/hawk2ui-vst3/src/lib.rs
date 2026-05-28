//! Safe `Hawk2UI` VST3 binding records.
//!
//! This crate owns the safe boundary around VST3 concepts used by `Hawk2UI`.
//! Low-level VST3 ABI crates are implementation details; downstream crates use
//! validated records from this crate instead of raw pointers, unchecked strings,
//! or unbounded normalized values.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-vst3";

/// Returns the canonical Cargo package name for diagnostics and conformance checks.
#[must_use]
pub const fn crate_name() -> &'static str {
    CRATE_NAME
}

/// VST3 safe binding diagnostic.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Vst3Diagnostic {
    /// Stable diagnostic rule.
    pub rule: String,
    /// Human-readable diagnostic message.
    pub message: String,
}

impl Vst3Diagnostic {
    fn new(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            rule: rule.into(),
            message: message.into(),
        }
    }
}

/// VST3 class identifier.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Vst3ClassId {
    bytes: [u8; 16],
}

impl Vst3ClassId {
    /// Creates a class identifier from canonical VST3 bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self { bytes }
    }

    /// Creates a class identifier from the four 32-bit groups commonly used in VST3 examples.
    #[must_use]
    pub const fn from_u32s(a: u32, b: u32, c: u32, d: u32) -> Self {
        let a = a.to_be_bytes();
        let b = b.to_be_bytes();
        let c = c.to_be_bytes();
        let d = d.to_be_bytes();
        Self {
            bytes: [
                a[0], a[1], a[2], a[3], b[0], b[1], b[2], b[3], c[0], c[1], c[2], c[3], d[0], d[1],
                d[2], d[3],
            ],
        }
    }

    /// Parses a 32-character hexadecimal VST3 class identifier.
    ///
    /// # Errors
    ///
    /// Returns [`Vst3Diagnostic`] when the string is not exactly 32 hexadecimal characters.
    pub fn from_hex(value: &str) -> Result<Self, Vst3Diagnostic> {
        if value.len() != 32 || !value.chars().all(|ch| ch.is_ascii_hexdigit()) {
            return Err(Vst3Diagnostic::new(
                "vst3.class-id.invalid-hex",
                "VST3 class ID must be exactly 32 hexadecimal characters",
            ));
        }
        let mut bytes = [0_u8; 16];
        for (index, byte) in bytes.iter_mut().enumerate() {
            let start = index * 2;
            *byte = u8::from_str_radix(&value[start..start + 2], 16).map_err(|error| {
                Vst3Diagnostic::new(
                    "vst3.class-id.invalid-hex",
                    format!("VST3 class ID contains invalid hex: {error}"),
                )
            })?;
        }
        Ok(Self { bytes })
    }

    /// Returns the canonical class identifier bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> [u8; 16] {
        self.bytes
    }

    /// Returns the class identifier as a VST3 `TUID` value for low-level bindings.
    #[must_use]
    pub fn as_tuid(&self) -> vst3::Steinberg::TUID {
        self.bytes.map(|byte| i8::from_ne_bytes([byte]))
    }

    /// Returns the lowercase hexadecimal class identifier.
    #[must_use]
    pub fn to_hex(&self) -> String {
        let mut encoded = String::with_capacity(32);
        for byte in self.bytes {
            encoded.push(hex_nibble(byte >> 4));
            encoded.push(hex_nibble(byte & 0x0f));
        }
        encoded
    }
}

/// Bounded VST3 string payload.
#[derive(Clone, Debug, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Vst3String {
    value: String,
}

impl Vst3String {
    /// Creates a VST3-safe string.
    ///
    /// # Errors
    ///
    /// Returns [`Vst3Diagnostic`] when the string is empty, contains NUL bytes, or exceeds the
    /// conservative cross-field limit used by `Hawk2UI` metadata generation.
    pub fn new(value: impl Into<String>) -> Result<Self, Vst3Diagnostic> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(Vst3Diagnostic::new(
                "vst3.string.empty",
                "VST3 string fields must not be empty",
            ));
        }
        if value.contains('\0') {
            return Err(Vst3Diagnostic::new(
                "vst3.string.nul",
                "VST3 string fields must not contain NUL bytes",
            ));
        }
        if value.chars().count() > 127 {
            return Err(Vst3Diagnostic::new(
                "vst3.string.too-long",
                "VST3 string fields must not exceed 127 Unicode scalar values",
            ));
        }
        Ok(Self { value })
    }

    /// Returns the string value.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

/// VST3 factory metadata.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Vst3FactoryInfo {
    vendor: Vst3String,
    url: Option<Vst3String>,
    email: Option<Vst3String>,
}

impl Vst3FactoryInfo {
    /// Creates VST3 factory metadata.
    #[must_use]
    pub const fn new(
        vendor: Vst3String,
        url: Option<Vst3String>,
        email: Option<Vst3String>,
    ) -> Self {
        Self { vendor, url, email }
    }

    /// Returns the plugin vendor.
    #[must_use]
    pub const fn vendor(&self) -> &Vst3String {
        &self.vendor
    }

    /// Returns the vendor URL.
    #[must_use]
    pub const fn url(&self) -> Option<&Vst3String> {
        self.url.as_ref()
    }

    /// Returns the vendor email.
    #[must_use]
    pub const fn email(&self) -> Option<&Vst3String> {
        self.email.as_ref()
    }
}

/// VST3 class category.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub enum Vst3ClassCategory {
    /// VST3 audio processor component.
    AudioModule,
    /// VST3 edit controller component.
    ComponentController,
}

impl Vst3ClassCategory {
    /// Returns the VST3 category string used in class information records.
    #[must_use]
    pub const fn as_vst3_category(self) -> &'static str {
        match self {
            Self::AudioModule => "Audio Module Class",
            Self::ComponentController => "Component Controller Class",
        }
    }
}

/// VST3 plugin class metadata.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Vst3PluginClassInfo {
    class_id: Vst3ClassId,
    category: Vst3ClassCategory,
    name: Vst3String,
}

impl Vst3PluginClassInfo {
    /// Creates VST3 plugin class metadata.
    ///
    /// # Errors
    ///
    /// Returns [`Vst3Diagnostic`] when the class name does not fit the VST3 class info field.
    pub fn new(
        class_id: Vst3ClassId,
        category: Vst3ClassCategory,
        name: Vst3String,
    ) -> Result<Self, Vst3Diagnostic> {
        if name.as_str().chars().count() > 63 {
            return Err(Vst3Diagnostic::new(
                "vst3.class-info.name-too-long",
                "VST3 class names must fit the 64-byte SDK class info field including terminator",
            ));
        }
        Ok(Self {
            class_id,
            category,
            name,
        })
    }

    /// Returns the VST3 class ID.
    #[must_use]
    pub const fn class_id(&self) -> Vst3ClassId {
        self.class_id
    }

    /// Returns the VST3 class category.
    #[must_use]
    pub const fn category(&self) -> Vst3ClassCategory {
        self.category
    }

    /// Returns the VST3 class display name.
    #[must_use]
    pub const fn name(&self) -> &Vst3String {
        &self.name
    }
}

/// VST3 normalized parameter value.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Vst3NormalizedValue {
    value: f64,
}

impl Vst3NormalizedValue {
    /// Creates a normalized VST3 parameter value in the closed range `0.0..=1.0`.
    ///
    /// # Errors
    ///
    /// Returns [`Vst3Diagnostic`] when the value is non-finite or outside `0.0..=1.0`.
    pub fn new(value: f64) -> Result<Self, Vst3Diagnostic> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(Vst3Diagnostic::new(
                "vst3.normalized-value.out-of-range",
                "VST3 normalized parameter values must be finite and within 0.0..=1.0",
            ));
        }
        Ok(Self { value })
    }

    /// Returns the normalized value.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.value
    }
}

/// Host-owned parent handle for an embedded VST3 editor view.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Vst3EditorHostParent {
    raw_handle: u64,
}

impl Vst3EditorHostParent {
    /// Creates a validated host parent handle from a raw platform value.
    ///
    /// # Errors
    ///
    /// Returns [`Vst3Diagnostic`] when the raw handle is zero.
    pub fn from_raw(raw_handle: u64) -> Result<Self, Vst3Diagnostic> {
        if raw_handle == 0 {
            Err(Vst3Diagnostic::new(
                "vst3.editor-parent.invalid-handle",
                "VST3 editor parent handle must be nonzero",
            ))
        } else {
            Ok(Self { raw_handle })
        }
    }

    /// Returns the raw host parent handle value.
    #[must_use]
    pub const fn raw_handle(self) -> u64 {
        self.raw_handle
    }
}

fn hex_nibble(value: u8) -> char {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    char::from(HEX[usize::from(value & 0x0f)])
}

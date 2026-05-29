//! Safe `Hawk2UI` VST3 binding records.
//!
//! This crate owns the safe boundary around VST3 concepts used by `Hawk2UI`.
//! Low-level VST3 ABI crates are implementation details; downstream crates use
//! validated records from this crate instead of raw pointers, unchecked strings,
//! or unbounded normalized values.
//!
//! Every public record validates through its constructor, and its
//! [`serde::Deserialize`] implementation routes through that same constructor (via
//! `#[serde(try_from = …)]`). A deserialized value therefore carries the same
//! guarantees as a constructed one: untrusted snapshot/config/IPC input cannot
//! reconstruct an out-of-range, empty, NUL-bearing, over-length, nil-identity, or
//! zero-handle record field-by-field.

use core::ffi::c_char;
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

impl std::fmt::Display for Vst3Diagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.rule, self.message)
    }
}

impl std::error::Error for Vst3Diagnostic {}

/// VST3 class identifier.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize)]
#[serde(try_from = "RawVst3ClassId")]
pub struct Vst3ClassId {
    bytes: [u8; 16],
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RawVst3ClassId {
    bytes: [u8; 16],
}

impl TryFrom<RawVst3ClassId> for Vst3ClassId {
    type Error = Vst3Diagnostic;

    fn try_from(raw: RawVst3ClassId) -> Result<Self, Self::Error> {
        Self::from_validated_bytes(raw.bytes)
    }
}

impl Vst3ClassId {
    /// Creates a class identifier from canonical VST3 bytes.
    ///
    /// This is the trusted, infallible constructor; it does **not** reject the nil
    /// (all-zero) identifier. Use [`Vst3ClassId::from_hex`] (or the `Deserialize`
    /// path) when the bytes are untrusted and the nil sentinel must be rejected.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self { bytes }
    }

    /// Creates a class identifier from four 32-bit groups, laid out **big-endian**.
    ///
    /// This matches the `vst3` crate's non-Windows `uid()` byte order. It is **not**
    /// the Windows little-endian COM GUID layout that the SDK's `INLINE_UID` /
    /// `DECLARE_CLASS_IID` macros emit (those byte-swap the first three groups), so
    /// copying `(a, b, c, d)` groups from an SDK macro yields an identifier that
    /// disagrees with the SDK's own Windows-layout ID. For an exact canonical
    /// identifier use [`Vst3ClassId::from_hex`] or [`Vst3ClassId::from_bytes`]. Like
    /// `from_bytes`, this trusted constructor does not reject the nil identifier.
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
    /// Returns [`Vst3Diagnostic`] when the string is not exactly 32 hexadecimal
    /// characters, or when the parsed identifier is the nil (all-zero) sentinel.
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
        Self::from_validated_bytes(bytes)
    }

    /// Validates untrusted identifier bytes, rejecting the nil (all-zero) sentinel.
    fn from_validated_bytes(bytes: [u8; 16]) -> Result<Self, Vst3Diagnostic> {
        if bytes == [0_u8; 16] {
            return Err(Vst3Diagnostic::new(
                "vst3.class-id.nil",
                "VST3 class ID must not be the nil (all-zero) identifier",
            ));
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
    #[allow(clippy::cast_possible_wrap)]
    pub fn as_tuid(&self) -> vst3::Steinberg::TUID {
        // `TUID = [c_char; 16]` and `c_char` is `i8` or `u8` depending on the target
        // (`i8` on x86/Windows/Apple, `u8` on ARM/AArch64 Linux and others). Casting
        // each byte to `c_char` reinterprets the bit pattern correctly under both
        // definitions; the `u8 -> i8` case is an intentional, lossless wrap.
        self.bytes.map(|byte| byte as c_char)
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
#[serde(try_from = "RawVst3String")]
pub struct Vst3String {
    value: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RawVst3String {
    value: String,
}

impl TryFrom<RawVst3String> for Vst3String {
    type Error = Vst3Diagnostic;

    fn try_from(raw: RawVst3String) -> Result<Self, Self::Error> {
        Self::new(raw.value)
    }
}

impl Vst3String {
    /// Creates a VST3-safe string.
    ///
    /// # Errors
    ///
    /// Returns [`Vst3Diagnostic`] when the string is empty, contains NUL bytes, or exceeds the
    /// conservative cross-field **byte** limit (127 bytes) used by `Hawk2UI` metadata generation.
    /// VST3 string fields are byte-bounded `char8[N]`, so the limit is measured in UTF-8 bytes,
    /// not Unicode scalar values.
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
        if value.len() > 127 {
            return Err(Vst3Diagnostic::new(
                "vst3.string.too-long",
                "VST3 string fields must not exceed 127 bytes (UTF-8)",
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
///
/// Each field is a [`Vst3String`], so the `Deserialize` path validates the
/// vendor/URL/email through `Vst3String`'s own `try_from` boundary; this record
/// adds no further invariants, so it deserializes field-by-field directly.
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
#[serde(try_from = "RawVst3PluginClassInfo")]
pub struct Vst3PluginClassInfo {
    class_id: Vst3ClassId,
    category: Vst3ClassCategory,
    name: Vst3String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RawVst3PluginClassInfo {
    class_id: Vst3ClassId,
    category: Vst3ClassCategory,
    name: Vst3String,
}

impl TryFrom<RawVst3PluginClassInfo> for Vst3PluginClassInfo {
    type Error = Vst3Diagnostic;

    fn try_from(raw: RawVst3PluginClassInfo) -> Result<Self, Self::Error> {
        Self::new(raw.class_id, raw.category, raw.name)
    }
}

impl Vst3PluginClassInfo {
    /// Creates VST3 plugin class metadata.
    ///
    /// # Errors
    ///
    /// Returns [`Vst3Diagnostic`] when the class name does not fit the VST3 class info field
    /// (`char8[64]`, so at most 63 UTF-8 bytes plus a NUL terminator).
    pub fn new(
        class_id: Vst3ClassId,
        category: Vst3ClassCategory,
        name: Vst3String,
    ) -> Result<Self, Vst3Diagnostic> {
        if name.as_str().len() > 63 {
            return Err(Vst3Diagnostic::new(
                "vst3.class-info.name-too-long",
                "VST3 class names must fit the 64-byte SDK class info field (<=63 bytes plus terminator)",
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
#[serde(try_from = "RawVst3NormalizedValue")]
pub struct Vst3NormalizedValue {
    value: f64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RawVst3NormalizedValue {
    value: f64,
}

impl TryFrom<RawVst3NormalizedValue> for Vst3NormalizedValue {
    type Error = Vst3Diagnostic;

    fn try_from(raw: RawVst3NormalizedValue) -> Result<Self, Self::Error> {
        Self::new(raw.value)
    }
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
#[serde(try_from = "RawVst3EditorHostParent")]
pub struct Vst3EditorHostParent {
    raw_handle: u64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RawVst3EditorHostParent {
    raw_handle: u64,
}

impl TryFrom<RawVst3EditorHostParent> for Vst3EditorHostParent {
    type Error = Vst3Diagnostic;

    fn try_from(raw: RawVst3EditorHostParent) -> Result<Self, Self::Error> {
        Self::from_raw(raw.raw_handle)
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_identity() {
        assert_eq!(crate_name(), "hawk2ui-vst3");
    }

    #[test]
    fn deserialize_routes_through_validating_constructors() {
        // The headline guarantee: Serde must not reconstruct an invalid record field-by-field.
        assert!(serde_json::from_str::<Vst3NormalizedValue>("{\"value\":5.0}").is_err());
        assert!(serde_json::from_str::<Vst3NormalizedValue>("{\"value\":-0.1}").is_err());
        assert!(serde_json::from_str::<Vst3String>("{\"value\":\"\"}").is_err());
        assert!(serde_json::from_str::<Vst3String>("{\"value\":\"a\\u0000b\"}").is_err());
        assert!(serde_json::from_str::<Vst3EditorHostParent>("{\"raw_handle\":0}").is_err());
        assert!(
            serde_json::from_str::<Vst3ClassId>("{\"bytes\":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]}")
                .is_err()
        );

        // Valid payloads still deserialize and round-trip.
        let value: Vst3NormalizedValue = serde_json::from_str("{\"value\":0.25}").unwrap();
        assert!((value.get() - 0.25).abs() < f64::EPSILON);
        let parent: Vst3EditorHostParent = serde_json::from_str("{\"raw_handle\":42}").unwrap();
        assert_eq!(parent.raw_handle(), 42);
    }

    #[test]
    fn as_tuid_preserves_byte_values_on_every_target() {
        let id = Vst3ClassId::from_bytes([
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
            0x0f, 0xff,
        ]);
        let tuid = id.as_tuid();
        assert_eq!(tuid.len(), 16);
        // Bit pattern is preserved regardless of whether `c_char` is `i8` or `u8`
        // (`to_ne_bytes` resolves on either integer alias without a signedness cast).
        assert_eq!(tuid[0].to_ne_bytes(), [0x01]);
        assert_eq!(tuid[15].to_ne_bytes(), [0xff]);
    }

    #[test]
    fn from_hex_round_trips_and_rejects_nil() {
        assert!(Vst3ClassId::from_hex(&"0".repeat(32)).is_err());
        let id = Vst3ClassId::from_hex("0123456789abcdef0123456789abcdef").unwrap();
        assert_eq!(id.to_hex(), "0123456789abcdef0123456789abcdef");
    }

    #[test]
    fn string_limit_is_byte_bounded_not_char_bounded() {
        assert!(Vst3String::new("a".repeat(127)).is_ok());
        assert!(Vst3String::new("a".repeat(128)).is_err());
        // 50 three-byte chars = 150 bytes: rejected by the byte limit though only 50 scalars.
        assert!(Vst3String::new("\u{20ac}".repeat(50)).is_err());
    }

    #[test]
    fn class_info_name_limit_is_byte_bounded() {
        let class_id = Vst3ClassId::from_hex("0123456789abcdef0123456789abcdef").unwrap();
        let short = Vst3String::new("a".repeat(63)).unwrap();
        assert!(Vst3PluginClassInfo::new(class_id, Vst3ClassCategory::AudioModule, short).is_ok());
        let long = Vst3String::new("a".repeat(64)).unwrap();
        assert!(
            Vst3PluginClassInfo::new(class_id, Vst3ClassCategory::ComponentController, long)
                .is_err()
        );
    }
}

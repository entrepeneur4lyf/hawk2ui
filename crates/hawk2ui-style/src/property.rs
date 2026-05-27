//! Typed style property metadata and validation.

/// Stable style property identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PropertyId(String);

impl PropertyId {
    /// Creates a property identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the property identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Style property domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PropertyGroup {
    /// Layout property.
    Layout,
    /// Typography property.
    Typography,
    /// Color property.
    Color,
    /// Border property.
    Border,
    /// Radius property.
    Radius,
    /// Shadow property.
    Shadow,
    /// Transform property.
    Transform,
    /// Compositing property.
    Compositing,
    /// Overflow property.
    Overflow,
    /// Custom property.
    Custom,
    /// Transition property.
    Transition,
    /// Token-backed property.
    TokenReference,
}

/// Typed style value family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueType {
    /// Keyword value.
    Keyword,
    /// Length value.
    Length,
    /// Number value.
    Number,
    /// Color value.
    Color,
    /// Shadow value.
    Shadow,
    /// Transform value.
    Transform,
    /// Duration value.
    Duration,
    /// Token reference value.
    TokenReference,
}

/// Unit handling policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnitHandling {
    /// No unit is accepted.
    Unitless,
    /// Pixel lengths only.
    PxOnly,
    /// Milliseconds only.
    MsOnly,
    /// Value must be resolved through tokens.
    TokenOnly,
    /// Unit handling is not applicable.
    NotApplicable,
}

/// Downstream implementation requirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PropertyRequirement {
    /// Layout engine must consume this property.
    Layout,
    /// Renderer must consume this property.
    Renderer,
}

/// Runtime-safe typed style value.
#[derive(Clone, Debug, PartialEq)]
pub enum StyleValue {
    /// Keyword value.
    Keyword(String),
    /// Pixel length.
    LengthPx(f32),
    /// Unitless number.
    Number(f32),
    /// RGBA color.
    ColorRgba(u8, u8, u8, u8),
    /// Shadow expression.
    Shadow(String),
    /// Transform expression.
    Transform(String),
    /// Duration in milliseconds.
    DurationMs(u32),
    /// Token reference by stable token path.
    TokenRef(String),
}

impl StyleValue {
    fn value_type(&self) -> ValueType {
        match self {
            Self::Keyword(_) => ValueType::Keyword,
            Self::LengthPx(_) => ValueType::Length,
            Self::Number(_) => ValueType::Number,
            Self::ColorRgba(..) => ValueType::Color,
            Self::Shadow(_) => ValueType::Shadow,
            Self::Transform(_) => ValueType::Transform,
            Self::DurationMs(_) => ValueType::Duration,
            Self::TokenRef(_) => ValueType::TokenReference,
        }
    }
}

/// Style property metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct PropertyMetadata {
    id: PropertyId,
    group: PropertyGroup,
    value_type: ValueType,
    default_value: StyleValue,
    inherited: bool,
    unit_handling: UnitHandling,
    requirements: Vec<PropertyRequirement>,
}

impl PropertyMetadata {
    /// Creates property metadata.
    #[must_use]
    pub fn new(
        id: PropertyId,
        group: PropertyGroup,
        value_type: ValueType,
        default_value: StyleValue,
        inherited: bool,
        unit_handling: UnitHandling,
        requirements: Vec<PropertyRequirement>,
    ) -> Self {
        Self {
            id,
            group,
            value_type,
            default_value,
            inherited,
            unit_handling,
            requirements,
        }
    }

    /// Returns the property identifier.
    #[must_use]
    pub const fn id(&self) -> &PropertyId {
        &self.id
    }

    /// Returns the property group.
    #[must_use]
    pub const fn group(&self) -> PropertyGroup {
        self.group
    }

    /// Returns the accepted value type.
    #[must_use]
    pub const fn value_type(&self) -> ValueType {
        self.value_type
    }

    /// Returns the default typed value.
    #[must_use]
    pub const fn default_value(&self) -> &StyleValue {
        &self.default_value
    }

    /// Returns whether the property inherits by default.
    #[must_use]
    pub const fn inherited(&self) -> bool {
        self.inherited
    }

    /// Returns the unit handling policy.
    #[must_use]
    pub const fn unit_handling(&self) -> UnitHandling {
        self.unit_handling
    }

    /// Returns whether this property has a downstream requirement.
    #[must_use]
    pub fn requires(&self, requirement: PropertyRequirement) -> bool {
        self.requirements.contains(&requirement)
    }
}

/// Property validation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValidationError {
    /// Property does not exist in the registry.
    UnknownProperty(String),
    /// Value type does not match the metadata.
    WrongValueType {
        /// Property name.
        property: String,
        /// Expected value type.
        expected: ValueType,
        /// Actual value type.
        actual: ValueType,
    },
    /// Number is outside the accepted range.
    NumberOutOfRange {
        /// Property name.
        property: String,
    },
}

/// Production style property registry.
#[derive(Clone, Debug, PartialEq)]
pub struct PropertyRegistry {
    properties: Vec<PropertyMetadata>,
}

impl PropertyRegistry {
    /// Creates the production property registry.
    #[must_use]
    pub fn production() -> Self {
        Self {
            properties: PROPERTY_SPECS.iter().map(PropertyMetadata::from).collect(),
        }
    }

    /// Returns metadata for a property.
    #[must_use]
    pub fn metadata(&self, id: &PropertyId) -> Option<&PropertyMetadata> {
        self.properties
            .iter()
            .find(|metadata| metadata.id.as_str() == id.as_str())
    }

    /// Returns all registered production properties in deterministic registry order.
    #[must_use]
    pub fn properties(&self) -> &[PropertyMetadata] {
        &self.properties
    }

    /// Validates a typed style value for a property.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError`] when the property is unknown, the value type differs, or a
    /// property-specific range check fails.
    pub fn validate(&self, id: &PropertyId, value: &StyleValue) -> Result<(), ValidationError> {
        let Some(metadata) = self.metadata(id) else {
            return Err(ValidationError::UnknownProperty(id.as_str().to_string()));
        };
        let actual = value.value_type();
        if actual != metadata.value_type {
            return Err(ValidationError::WrongValueType {
                property: id.as_str().to_string(),
                expected: metadata.value_type,
                actual,
            });
        }
        if let StyleValue::LengthPx(value) = value
            && (!value.is_finite() || *value < 0.0)
        {
            return Err(ValidationError::NumberOutOfRange {
                property: id.as_str().to_string(),
            });
        }
        if id.as_str() == "opacity"
            && let StyleValue::Number(value) = value
            && (!value.is_finite() || !(0.0..=1.0).contains(value))
        {
            return Err(ValidationError::NumberOutOfRange {
                property: id.as_str().to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DefaultValueSpec {
    Keyword(&'static str),
    LengthPx(u16),
    NumberOne,
    ColorRgba(u8, u8, u8, u8),
    Shadow(&'static str),
    Transform(&'static str),
    DurationMs(u32),
    TokenRef(&'static str),
}

impl From<DefaultValueSpec> for StyleValue {
    fn from(value: DefaultValueSpec) -> Self {
        match value {
            DefaultValueSpec::Keyword(value) => Self::Keyword(value.to_string()),
            DefaultValueSpec::LengthPx(value) => Self::LengthPx(f32::from(value)),
            DefaultValueSpec::NumberOne => Self::Number(1.0),
            DefaultValueSpec::ColorRgba(r, g, b, a) => Self::ColorRgba(r, g, b, a),
            DefaultValueSpec::Shadow(value) => Self::Shadow(value.to_string()),
            DefaultValueSpec::Transform(value) => Self::Transform(value.to_string()),
            DefaultValueSpec::DurationMs(value) => Self::DurationMs(value),
            DefaultValueSpec::TokenRef(value) => Self::TokenRef(value.to_string()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PropertySpec {
    name: &'static str,
    group: PropertyGroup,
    value_type: ValueType,
    default_value: DefaultValueSpec,
    inherited: bool,
    unit_handling: UnitHandling,
    requirements: &'static [PropertyRequirement],
}

impl From<&PropertySpec> for PropertyMetadata {
    fn from(spec: &PropertySpec) -> Self {
        Self::new(
            PropertyId::new(spec.name),
            spec.group,
            spec.value_type,
            StyleValue::from(spec.default_value),
            spec.inherited,
            spec.unit_handling,
            spec.requirements.to_vec(),
        )
    }
}

const LAYOUT: &[PropertyRequirement] = &[PropertyRequirement::Layout];
const RENDERER: &[PropertyRequirement] = &[PropertyRequirement::Renderer];
const LAYOUT_AND_RENDERER: &[PropertyRequirement] =
    &[PropertyRequirement::Layout, PropertyRequirement::Renderer];

const PROPERTY_SPECS: &[PropertySpec] = &[
    PropertySpec {
        name: "display",
        group: PropertyGroup::Layout,
        value_type: ValueType::Keyword,
        default_value: DefaultValueSpec::Keyword("flex"),
        inherited: false,
        unit_handling: UnitHandling::NotApplicable,
        requirements: LAYOUT,
    },
    PropertySpec {
        name: "font-size",
        group: PropertyGroup::Typography,
        value_type: ValueType::Length,
        default_value: DefaultValueSpec::LengthPx(16),
        inherited: true,
        unit_handling: UnitHandling::PxOnly,
        requirements: LAYOUT_AND_RENDERER,
    },
    PropertySpec {
        name: "color",
        group: PropertyGroup::Color,
        value_type: ValueType::Color,
        default_value: DefaultValueSpec::ColorRgba(255, 255, 255, 255),
        inherited: true,
        unit_handling: UnitHandling::NotApplicable,
        requirements: RENDERER,
    },
    PropertySpec {
        name: "border-width",
        group: PropertyGroup::Border,
        value_type: ValueType::Length,
        default_value: DefaultValueSpec::LengthPx(0),
        inherited: false,
        unit_handling: UnitHandling::PxOnly,
        requirements: LAYOUT_AND_RENDERER,
    },
    PropertySpec {
        name: "border-radius",
        group: PropertyGroup::Radius,
        value_type: ValueType::Length,
        default_value: DefaultValueSpec::LengthPx(0),
        inherited: false,
        unit_handling: UnitHandling::PxOnly,
        requirements: RENDERER,
    },
    PropertySpec {
        name: "box-shadow",
        group: PropertyGroup::Shadow,
        value_type: ValueType::Shadow,
        default_value: DefaultValueSpec::Shadow("none"),
        inherited: false,
        unit_handling: UnitHandling::NotApplicable,
        requirements: RENDERER,
    },
    PropertySpec {
        name: "transform",
        group: PropertyGroup::Transform,
        value_type: ValueType::Transform,
        default_value: DefaultValueSpec::Transform("none"),
        inherited: false,
        unit_handling: UnitHandling::NotApplicable,
        requirements: RENDERER,
    },
    PropertySpec {
        name: "opacity",
        group: PropertyGroup::Compositing,
        value_type: ValueType::Number,
        default_value: DefaultValueSpec::NumberOne,
        inherited: false,
        unit_handling: UnitHandling::Unitless,
        requirements: RENDERER,
    },
    PropertySpec {
        name: "overflow",
        group: PropertyGroup::Overflow,
        value_type: ValueType::Keyword,
        default_value: DefaultValueSpec::Keyword("visible"),
        inherited: false,
        unit_handling: UnitHandling::NotApplicable,
        requirements: LAYOUT_AND_RENDERER,
    },
    PropertySpec {
        name: "--accent-color",
        group: PropertyGroup::Custom,
        value_type: ValueType::TokenReference,
        default_value: DefaultValueSpec::TokenRef("color.accent"),
        inherited: true,
        unit_handling: UnitHandling::TokenOnly,
        requirements: RENDERER,
    },
    PropertySpec {
        name: "transition-duration",
        group: PropertyGroup::Transition,
        value_type: ValueType::Duration,
        default_value: DefaultValueSpec::DurationMs(0),
        inherited: false,
        unit_handling: UnitHandling::MsOnly,
        requirements: RENDERER,
    },
    PropertySpec {
        name: "background-color",
        group: PropertyGroup::TokenReference,
        value_type: ValueType::TokenReference,
        default_value: DefaultValueSpec::TokenRef("color.surface"),
        inherited: false,
        unit_handling: UnitHandling::TokenOnly,
        requirements: RENDERER,
    },
];

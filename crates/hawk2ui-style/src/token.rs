//! Design token records and resolution.

/// Token kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenKind {
    /// Color token.
    Color,
    /// Spacing token.
    Spacing,
    /// Radius token.
    Radius,
    /// Typography token.
    Typography,
    /// Motion token.
    Motion,
    /// Preference hook token.
    PreferenceHook,
}

/// Typed token value.
#[derive(Clone, Debug, PartialEq)]
pub enum TokenValue {
    /// RGBA color.
    ColorRgba(u8, u8, u8, u8),
    /// Pixel length.
    LengthPx(f32),
    /// Typography face and size.
    Typography {
        /// Font family.
        family: String,
        /// Font size in pixels.
        size_px: f32,
    },
    /// Duration in milliseconds.
    DurationMs(u32),
    /// Preference hook target token.
    PreferenceHook(String),
}

/// Design token record.
#[derive(Clone, Debug, PartialEq)]
pub struct TokenRecord {
    name: String,
    kind: TokenKind,
    value: TokenValue,
}

impl TokenRecord {
    /// Creates a token record.
    #[must_use]
    pub fn new(name: impl Into<String>, kind: TokenKind, value: TokenValue) -> Self {
        Self {
            name: name.into(),
            kind,
            value,
        }
    }

    /// Returns the token name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the token kind.
    #[must_use]
    pub const fn kind(&self) -> TokenKind {
        self.kind
    }

    /// Returns the token value.
    #[must_use]
    pub const fn value(&self) -> &TokenValue {
        &self.value
    }
}

/// Token diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenDiagnostic {
    rule: String,
    message: String,
}

impl TokenDiagnostic {
    /// Creates a token diagnostic.
    #[must_use]
    pub fn new(rule: impl Into<String>, message: impl Into<String>) -> Self {
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

/// Token resolution error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenError {
    diagnostic: TokenDiagnostic,
}

impl TokenError {
    /// Creates a token error.
    #[must_use]
    pub fn new(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            diagnostic: TokenDiagnostic::new(rule, message),
        }
    }

    /// Returns the structured diagnostic.
    #[must_use]
    pub const fn diagnostic(&self) -> &TokenDiagnostic {
        &self.diagnostic
    }
}

/// Theme variant token overrides.
#[derive(Clone, Debug, PartialEq)]
pub struct ThemeVariant {
    name: String,
    tokens: Vec<TokenRecord>,
}

impl ThemeVariant {
    /// Creates a theme variant.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            tokens: Vec::new(),
        }
    }

    /// Adds or replaces a token override.
    #[must_use]
    pub fn with_token(mut self, name: impl Into<String>, value: TokenValue) -> Self {
        let name = name.into();
        upsert_token(
            &mut self.tokens,
            TokenRecord::new(&name, infer_kind(&name, &value), value),
        );
        self
    }

    /// Returns the theme variant name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Design token set.
#[derive(Clone, Debug, PartialEq)]
pub struct TokenSet {
    tokens: Vec<TokenRecord>,
    themes: Vec<ThemeVariant>,
}

impl TokenSet {
    /// Creates an empty production token set.
    #[must_use]
    pub const fn production() -> Self {
        Self {
            tokens: Vec::new(),
            themes: Vec::new(),
        }
    }

    /// Adds or replaces a color token.
    #[must_use]
    pub fn with_color(mut self, name: impl Into<String>, r: u8, g: u8, b: u8, a: u8) -> Self {
        let name = name.into();
        upsert_token(
            &mut self.tokens,
            TokenRecord::new(name, TokenKind::Color, TokenValue::ColorRgba(r, g, b, a)),
        );
        self
    }

    /// Adds or replaces a spacing token.
    #[must_use]
    pub fn with_spacing(mut self, name: impl Into<String>, value_px: f32) -> Self {
        let name = name.into();
        upsert_token(
            &mut self.tokens,
            TokenRecord::new(name, TokenKind::Spacing, TokenValue::LengthPx(value_px)),
        );
        self
    }

    /// Adds or replaces a radius token.
    #[must_use]
    pub fn with_radius(mut self, name: impl Into<String>, value_px: f32) -> Self {
        let name = name.into();
        upsert_token(
            &mut self.tokens,
            TokenRecord::new(name, TokenKind::Radius, TokenValue::LengthPx(value_px)),
        );
        self
    }

    /// Adds or replaces a typography token.
    #[must_use]
    pub fn with_typography(
        mut self,
        name: impl Into<String>,
        family: impl Into<String>,
        size_px: f32,
    ) -> Self {
        let name = name.into();
        upsert_token(
            &mut self.tokens,
            TokenRecord::new(
                name,
                TokenKind::Typography,
                TokenValue::Typography {
                    family: family.into(),
                    size_px,
                },
            ),
        );
        self
    }

    /// Adds or replaces a motion token.
    #[must_use]
    pub fn with_motion(mut self, name: impl Into<String>, duration_ms: u32) -> Self {
        let name = name.into();
        upsert_token(
            &mut self.tokens,
            TokenRecord::new(name, TokenKind::Motion, TokenValue::DurationMs(duration_ms)),
        );
        self
    }

    /// Adds or replaces a preference hook token.
    #[must_use]
    pub fn with_preference_hook(
        mut self,
        name: impl Into<String>,
        target: impl Into<String>,
    ) -> Self {
        let name = name.into();
        upsert_token(
            &mut self.tokens,
            TokenRecord::new(
                name,
                TokenKind::PreferenceHook,
                TokenValue::PreferenceHook(target.into()),
            ),
        );
        self
    }

    /// Adds or replaces a theme variant.
    #[must_use]
    pub fn with_theme(mut self, theme: ThemeVariant) -> Self {
        if let Some(existing) = self
            .themes
            .iter_mut()
            .find(|existing| existing.name == theme.name)
        {
            *existing = theme;
        } else {
            self.themes.push(theme);
        }
        self
    }

    /// Resolves a token by name.
    ///
    /// # Errors
    ///
    /// Returns [`TokenError`] when the token is missing.
    pub fn resolve(&self, name: &str) -> Result<&TokenRecord, TokenError> {
        let token = self
            .tokens
            .iter()
            .find(|token| token.name == name)
            .ok_or_else(|| missing_token(name))?;
        validate_token_record(token)?;
        Ok(token)
    }

    /// Resolves a token by name for a theme variant, falling back to base tokens.
    ///
    /// # Errors
    ///
    /// Returns [`TokenError`] when the theme or token is missing.
    pub fn resolve_for_theme(&self, name: &str, theme: &str) -> Result<&TokenRecord, TokenError> {
        let variant = self
            .themes
            .iter()
            .find(|variant| variant.name == theme)
            .ok_or_else(|| {
                TokenError::new("theme.missing", format!("theme `{theme}` is missing"))
            })?;
        if !is_valid_identifier(theme) {
            return Err(TokenError::new(
                "theme.name.invalid",
                format!("theme `{theme}` has an invalid name"),
            ));
        }
        let Some(token) = variant.tokens.iter().find(|token| token.name == name) else {
            return self.resolve(name);
        };
        validate_token_record(token)?;
        Ok(token)
    }
}

fn upsert_token(tokens: &mut Vec<TokenRecord>, token: TokenRecord) {
    if let Some(existing) = tokens
        .iter_mut()
        .find(|existing| existing.name == token.name)
    {
        *existing = token;
    } else {
        tokens.push(token);
    }
}

fn infer_kind(name: &str, value: &TokenValue) -> TokenKind {
    if name.starts_with("color.") {
        TokenKind::Color
    } else {
        match value {
            TokenValue::ColorRgba(..) => TokenKind::Color,
            TokenValue::LengthPx(_) => TokenKind::Spacing,
            TokenValue::Typography { .. } => TokenKind::Typography,
            TokenValue::DurationMs(_) => TokenKind::Motion,
            TokenValue::PreferenceHook(_) => TokenKind::PreferenceHook,
        }
    }
}

fn missing_token(name: &str) -> TokenError {
    TokenError::new("token.missing", format!("token `{name}` is missing"))
}

fn validate_token_record(token: &TokenRecord) -> Result<(), TokenError> {
    if !is_valid_token_name(&token.name) {
        return Err(TokenError::new(
            "token.name.invalid",
            format!("token `{}` has an invalid name", token.name),
        ));
    }
    match &token.value {
        TokenValue::LengthPx(value) if !value.is_finite() || *value < 0.0 => {
            Err(invalid_token_value(&token.name))
        }
        TokenValue::Typography { family, size_px }
            if family.trim().is_empty() || !size_px.is_finite() || *size_px <= 0.0 =>
        {
            Err(invalid_token_value(&token.name))
        }
        TokenValue::PreferenceHook(target) if !is_valid_token_name(target) => {
            Err(invalid_token_value(&token.name))
        }
        _ => Ok(()),
    }
}

fn invalid_token_value(name: &str) -> TokenError {
    TokenError::new(
        "token.value.invalid",
        format!("token `{name}` has an invalid value"),
    )
}

fn is_valid_token_name(name: &str) -> bool {
    let mut segments = name.split('.');
    let Some(first) = segments.next() else {
        return false;
    };
    is_valid_identifier(first)
        && segments.clone().next().is_some()
        && segments.all(is_valid_identifier)
}

fn is_valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
}

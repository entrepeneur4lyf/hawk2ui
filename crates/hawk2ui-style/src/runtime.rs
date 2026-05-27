//! Runtime typed style table.

use crate::{
    CompiledDeclaration, CompiledStyleSheet, PropertyId, PropertyMetadata, PropertyRegistry,
    SelectorPart, StyleValue, TokenSet, TokenValue,
};

/// Runtime style diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeStyleDiagnostic {
    rule: String,
    message: String,
}

impl RuntimeStyleDiagnostic {
    /// Creates a runtime style diagnostic.
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

/// Runtime style error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeStyleError {
    diagnostic: RuntimeStyleDiagnostic,
}

impl RuntimeStyleError {
    /// Creates a runtime style error.
    #[must_use]
    pub fn new(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            diagnostic: RuntimeStyleDiagnostic::new(rule, message),
        }
    }

    /// Returns the structured diagnostic.
    #[must_use]
    pub const fn diagnostic(&self) -> &RuntimeStyleDiagnostic {
        &self.diagnostic
    }
}

/// Node metadata used by the runtime style cascade.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeStyleNode {
    node_id: String,
    element: String,
    selector_id: Option<String>,
    classes: Vec<String>,
    states: Vec<String>,
}

impl RuntimeStyleNode {
    /// Creates a style node with a stable runtime node ID and element name.
    #[must_use]
    pub fn new(node_id: impl Into<String>, element: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            element: element.into(),
            selector_id: None,
            classes: Vec::new(),
            states: Vec::new(),
        }
    }

    /// Adds the CSS ID selector identity for this node.
    #[must_use]
    pub fn with_selector_id(mut self, selector_id: impl Into<String>) -> Self {
        self.selector_id = Some(selector_id.into());
        self
    }

    /// Adds a class selector identity for this node.
    #[must_use]
    pub fn with_class(mut self, class: impl Into<String>) -> Self {
        self.classes.push(class.into());
        self
    }

    /// Adds a `:hawk(...)` state identity for this node.
    #[must_use]
    pub fn with_state(mut self, state: impl Into<String>) -> Self {
        self.states.push(state.into());
        self
    }

    /// Returns the runtime node ID.
    #[must_use]
    pub fn node_id(&self) -> &str {
        &self.node_id
    }
}

/// Runtime style tree used for selector matching, inheritance, and invalidation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeStyleTree {
    entries: Vec<RuntimeStyleTreeEntry>,
}

impl RuntimeStyleTree {
    /// Creates a tree with a root node.
    #[must_use]
    pub fn new(root: RuntimeStyleNode) -> Self {
        Self {
            entries: vec![RuntimeStyleTreeEntry {
                node: root,
                parent_id: None,
            }],
        }
    }

    /// Adds a child node under an existing parent.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeStyleError`] when the parent is missing or the child ID is duplicated.
    pub fn with_child(
        mut self,
        parent_id: impl AsRef<str>,
        child: RuntimeStyleNode,
    ) -> Result<Self, RuntimeStyleError> {
        let parent_id = parent_id.as_ref();
        if self.entry(parent_id).is_none() {
            return Err(RuntimeStyleError::new(
                "runtime-style.parent.missing",
                format!("style parent `{parent_id}` was not found"),
            ));
        }
        if self.entry(child.node_id()).is_some() {
            return Err(RuntimeStyleError::new(
                "runtime-style.node.duplicate",
                format!("style node `{}` already exists", child.node_id()),
            ));
        }
        self.entries.push(RuntimeStyleTreeEntry {
            node: child,
            parent_id: Some(parent_id.to_string()),
        });
        Ok(self)
    }

    fn entry(&self, node_id: &str) -> Option<&RuntimeStyleTreeEntry> {
        self.entries
            .iter()
            .find(|entry| entry.node.node_id == node_id)
    }

    fn parent_entry(&self, node_id: &str) -> Option<&RuntimeStyleTreeEntry> {
        let parent_id = self.entry(node_id)?.parent_id.as_deref()?;
        self.entry(parent_id)
    }

    fn ancestors(&self, node_id: &str) -> Vec<&RuntimeStyleTreeEntry> {
        let mut ancestors = Vec::new();
        let mut current = self.parent_entry(node_id);
        while let Some(entry) = current {
            ancestors.push(entry);
            current = self.parent_entry(entry.node.node_id());
        }
        ancestors
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RuntimeStyleTreeEntry {
    node: RuntimeStyleNode,
    parent_id: Option<String>,
}

/// Runtime style environment that affects token and preference resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleRuntimeEnvironment {
    theme: Option<String>,
    preference_overrides: Vec<(String, String)>,
}

impl StyleRuntimeEnvironment {
    /// Creates the default runtime style environment.
    #[must_use]
    pub const fn production() -> Self {
        Self {
            theme: None,
            preference_overrides: Vec::new(),
        }
    }

    /// Selects a theme variant for token resolution.
    #[must_use]
    pub fn with_theme(mut self, theme: impl Into<String>) -> Self {
        self.theme = Some(theme.into());
        self
    }

    /// Overrides a preference hook token with a concrete target token.
    #[must_use]
    pub fn with_preference_override(
        mut self,
        preference: impl Into<String>,
        target: impl Into<String>,
    ) -> Self {
        let preference = preference.into();
        let target = target.into();
        if let Some((_, existing)) = self
            .preference_overrides
            .iter_mut()
            .find(|(entry, _)| entry == &preference)
        {
            *existing = target;
        } else {
            self.preference_overrides.push((preference, target));
        }
        self
    }

    fn theme(&self) -> Option<&str> {
        self.theme.as_deref()
    }

    fn preference_override(&self, preference: &str) -> Option<&str> {
        self.preference_overrides
            .iter()
            .find(|(entry, _)| entry == preference)
            .map(|(_, target)| target.as_str())
    }
}

/// Difference between two computed runtime style tables.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeStyleInvalidation {
    affected_node_ids: Vec<String>,
}

impl RuntimeStyleInvalidation {
    /// Returns whether render output must be invalidated.
    #[must_use]
    pub fn requires_render_invalidation(&self) -> bool {
        !self.affected_node_ids.is_empty()
    }

    /// Returns affected runtime node IDs in deterministic table order.
    #[must_use]
    pub fn affected_node_ids(&self) -> &[String] {
        &self.affected_node_ids
    }
}

/// Runtime style table keyed by node identity and property ID.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeStyleTable {
    values: Vec<NodeStyleValues>,
}

impl RuntimeStyleTable {
    /// Creates an empty runtime style table.
    #[must_use]
    pub const fn new() -> Self {
        Self { values: Vec::new() }
    }

    /// Resolves ordered style references for a node into typed runtime values.
    ///
    /// Later style references have higher precedence when they declare the same property.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeStyleError`] when any referenced style rule is missing.
    pub fn from_style_refs(
        node_id: impl Into<String>,
        sheet: &CompiledStyleSheet,
        refs: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<Self, RuntimeStyleError> {
        Self::from_style_refs_with_value_map(node_id, sheet, refs, |value| Ok(value.clone()))
    }

    /// Resolves ordered style references and token-backed declarations for a node.
    ///
    /// Token references are converted to renderer-ready typed runtime values.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeStyleError`] when any referenced style rule or token is missing.
    pub fn from_style_refs_with_tokens(
        node_id: impl Into<String>,
        sheet: &CompiledStyleSheet,
        refs: impl IntoIterator<Item = impl AsRef<str>>,
        tokens: &TokenSet,
    ) -> Result<Self, RuntimeStyleError> {
        Self::from_style_refs_with_value_map(node_id, sheet, refs, |value| {
            resolve_runtime_value(value, tokens)
        })
    }

    /// Resolves ordered style references and token-backed declarations for a theme variant.
    ///
    /// Theme token overrides are used first, with fallback to base tokens.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeStyleError`] when any referenced style rule, theme, or token is missing.
    pub fn from_style_refs_for_theme(
        node_id: impl Into<String>,
        sheet: &CompiledStyleSheet,
        refs: impl IntoIterator<Item = impl AsRef<str>>,
        tokens: &TokenSet,
        theme: &str,
    ) -> Result<Self, RuntimeStyleError> {
        Self::from_style_refs_with_value_map(node_id, sheet, refs, |value| {
            resolve_runtime_value_for_theme(value, tokens, theme)
        })
    }

    /// Computes full runtime styles for a tree using selector matching and cascade semantics.
    ///
    /// The computed table contains a value for every registered property on every node. Explicit
    /// matching declarations win by specificity and source order, inherited properties use the
    /// parent computed value, and all remaining properties use registry initial values.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeStyleError`] when token resolution fails.
    pub fn compute_for_tree(
        sheet: &CompiledStyleSheet,
        tree: &RuntimeStyleTree,
        tokens: &TokenSet,
        environment: &StyleRuntimeEnvironment,
    ) -> Result<Self, RuntimeStyleError> {
        let registry = PropertyRegistry::production();
        let mut table = Self::new();

        for entry in &tree.entries {
            for metadata in registry.properties() {
                let declared = winning_declaration(sheet, tree, entry, metadata);
                let value = if let Some(declaration) = declared {
                    resolve_runtime_value_with_environment(
                        declaration.value(),
                        tokens,
                        environment,
                    )?
                } else if metadata.inherited() {
                    inherited_or_default_value(&table, entry, metadata, tokens, environment)?
                } else {
                    resolve_runtime_value_with_environment(
                        metadata.default_value(),
                        tokens,
                        environment,
                    )?
                };
                table.set_value(entry.node.node_id.clone(), metadata.id().clone(), value);
            }
        }

        Ok(table)
    }

    /// Computes style invalidation between two computed tables.
    #[must_use]
    pub fn diff_from(&self, previous: &Self) -> RuntimeStyleInvalidation {
        let mut affected_node_ids = Vec::new();
        for node in &self.values {
            let previous_values = previous
                .values
                .iter()
                .find(|entry| entry.node_id == node.node_id)
                .map(|entry| entry.values.as_slice());
            if previous_values != Some(node.values.as_slice()) {
                affected_node_ids.push(node.node_id.clone());
            }
        }
        for node in &previous.values {
            if self
                .values
                .iter()
                .all(|entry| entry.node_id != node.node_id)
            {
                affected_node_ids.push(node.node_id.clone());
            }
        }
        RuntimeStyleInvalidation { affected_node_ids }
    }

    fn from_style_refs_with_value_map(
        node_id: impl Into<String>,
        sheet: &CompiledStyleSheet,
        refs: impl IntoIterator<Item = impl AsRef<str>>,
        mut map_value: impl FnMut(&StyleValue) -> Result<StyleValue, RuntimeStyleError>,
    ) -> Result<Self, RuntimeStyleError> {
        let node_id = node_id.into();
        let mut table = Self::new();
        for style_ref in refs {
            let style_ref = style_ref.as_ref();
            let selector_key = format!("class({style_ref})");
            let Some(rule) = sheet.rule(&selector_key) else {
                return Err(RuntimeStyleError::new(
                    "runtime-style.ref.missing",
                    format!("style reference `{style_ref}` was not found in the compiled sheet"),
                ));
            };
            for declaration in rule.declarations() {
                table.set_value(
                    node_id.clone(),
                    declaration.property().clone(),
                    map_value(declaration.value())?,
                );
            }
        }
        Ok(table)
    }

    /// Adds or replaces a typed style value.
    #[must_use]
    pub fn with_value(
        mut self,
        node_id: impl Into<String>,
        property: PropertyId,
        value: StyleValue,
    ) -> Self {
        self.set_value(node_id.into(), property, value);
        self
    }

    /// Rejects raw string values at the runtime boundary.
    ///
    /// # Errors
    ///
    /// Always returns [`RuntimeStyleError`] because runtime style values must be typed before
    /// insertion.
    pub fn try_with_raw_value(
        self,
        _node_id: impl Into<String>,
        _property: impl Into<String>,
        _value: impl Into<String>,
    ) -> Result<Self, RuntimeStyleError> {
        Err(RuntimeStyleError::new(
            "runtime-style.raw-value.rejected",
            "runtime style table accepts typed values only",
        ))
    }

    /// Returns a typed style value by node identity and property ID.
    #[must_use]
    pub fn typed_value(&self, node_id: &str, property: &PropertyId) -> Option<&StyleValue> {
        self.values
            .iter()
            .find(|node| node.node_id == node_id)
            .and_then(|node| {
                node.values
                    .iter()
                    .find(|(entry_property, _)| entry_property.as_str() == property.as_str())
                    .map(|(_, value)| value)
            })
    }

    fn set_value(&mut self, node_id: String, property: PropertyId, value: StyleValue) {
        let Some(node) = self.values.iter_mut().find(|node| node.node_id == node_id) else {
            self.values.push(NodeStyleValues {
                node_id,
                values: vec![(property, value)],
            });
            return;
        };
        if let Some((_, existing)) = node
            .values
            .iter_mut()
            .find(|(entry_property, _)| entry_property.as_str() == property.as_str())
        {
            *existing = value;
        } else {
            node.values.push((property, value));
        }
    }
}

impl Default for RuntimeStyleTable {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq)]
struct NodeStyleValues {
    node_id: String,
    values: Vec<(PropertyId, StyleValue)>,
}

fn resolve_runtime_value(
    value: &StyleValue,
    tokens: &TokenSet,
) -> Result<StyleValue, RuntimeStyleError> {
    let StyleValue::TokenRef(token_name) = value else {
        return Ok(value.clone());
    };
    let token = tokens
        .resolve(token_name)
        .map_err(|error| runtime_token_error(&error))?;
    Ok(token_to_style_value(token.value()))
}

fn resolve_runtime_value_for_theme(
    value: &StyleValue,
    tokens: &TokenSet,
    theme: &str,
) -> Result<StyleValue, RuntimeStyleError> {
    let StyleValue::TokenRef(token_name) = value else {
        return Ok(value.clone());
    };
    let token = tokens
        .resolve_for_theme(token_name, theme)
        .map_err(|error| runtime_token_error(&error))?;
    Ok(token_to_style_value(token.value()))
}

fn resolve_runtime_value_with_environment(
    value: &StyleValue,
    tokens: &TokenSet,
    environment: &StyleRuntimeEnvironment,
) -> Result<StyleValue, RuntimeStyleError> {
    let StyleValue::TokenRef(token_name) = value else {
        return Ok(value.clone());
    };
    resolve_token_reference(token_name, tokens, environment, 0)
}

fn resolve_token_reference(
    token_name: &str,
    tokens: &TokenSet,
    environment: &StyleRuntimeEnvironment,
    depth: usize,
) -> Result<StyleValue, RuntimeStyleError> {
    if depth > 16 {
        return Err(RuntimeStyleError::new(
            "runtime-style.token.cycle",
            format!("token `{token_name}` exceeded the resolution depth limit"),
        ));
    }
    let effective_name = environment
        .preference_override(token_name)
        .unwrap_or(token_name);
    let token = if let Some(theme) = environment.theme() {
        tokens
            .resolve_for_theme(effective_name, theme)
            .map_err(|error| runtime_token_error(&error))?
    } else {
        tokens
            .resolve(effective_name)
            .map_err(|error| runtime_token_error(&error))?
    };
    match token.value() {
        TokenValue::PreferenceHook(target) => {
            resolve_token_reference(target, tokens, environment, depth + 1)
        }
        value => Ok(token_to_style_value(value)),
    }
}

fn inherited_or_default_value(
    table: &RuntimeStyleTable,
    entry: &RuntimeStyleTreeEntry,
    metadata: &PropertyMetadata,
    tokens: &TokenSet,
    environment: &StyleRuntimeEnvironment,
) -> Result<StyleValue, RuntimeStyleError> {
    if let Some(parent_id) = &entry.parent_id
        && let Some(value) = table.typed_value(parent_id, metadata.id())
    {
        return Ok(value.clone());
    }
    resolve_runtime_value_with_environment(metadata.default_value(), tokens, environment)
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct CascadePrecedence {
    specificity: Specificity,
    source_order: usize,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Specificity {
    ids: u16,
    classes: u16,
    elements: u16,
}

fn winning_declaration<'a>(
    sheet: &'a CompiledStyleSheet,
    tree: &RuntimeStyleTree,
    entry: &RuntimeStyleTreeEntry,
    metadata: &PropertyMetadata,
) -> Option<&'a CompiledDeclaration> {
    let mut winner: Option<(CascadePrecedence, &CompiledDeclaration)> = None;
    for (source_order, rule) in sheet.rules().iter().enumerate() {
        if !selector_matches(rule.selector().parts(), tree, entry) {
            continue;
        }
        let Some(declaration) = rule.declaration(metadata.id()) else {
            continue;
        };
        let precedence = CascadePrecedence {
            specificity: specificity(rule.selector().parts()),
            source_order,
        };
        if winner
            .as_ref()
            .is_none_or(|(current, _)| precedence >= *current)
        {
            winner = Some((precedence, declaration));
        }
    }
    winner.map(|(_, declaration)| declaration)
}

fn selector_matches(
    parts: &[SelectorPart],
    tree: &RuntimeStyleTree,
    entry: &RuntimeStyleTreeEntry,
) -> bool {
    match parts {
        [simple] => simple_selector_matches(simple, entry),
        [simple, SelectorPart::State(state)] => {
            simple_selector_matches(simple, entry)
                && entry.node.states.iter().any(|item| item == state)
        }
        [parent, SelectorPart::DirectChild, child] => {
            simple_selector_matches(child, entry)
                && tree
                    .parent_entry(entry.node.node_id())
                    .is_some_and(|parent_entry| simple_selector_matches(parent, parent_entry))
        }
        [ancestor, SelectorPart::Descendant, descendant] => {
            simple_selector_matches(descendant, entry)
                && tree
                    .ancestors(entry.node.node_id())
                    .iter()
                    .any(|ancestor_entry| simple_selector_matches(ancestor, ancestor_entry))
        }
        _ => false,
    }
}

fn simple_selector_matches(part: &SelectorPart, entry: &RuntimeStyleTreeEntry) -> bool {
    match part {
        SelectorPart::Element(element) => entry.node.element == *element,
        SelectorPart::Class(class) => entry.node.classes.iter().any(|item| item == class),
        SelectorPart::Id(id) => entry.node.selector_id.as_deref() == Some(id.as_str()),
        SelectorPart::State(state) => entry.node.states.iter().any(|item| item == state),
        SelectorPart::DirectChild | SelectorPart::Descendant => false,
    }
}

fn specificity(parts: &[SelectorPart]) -> Specificity {
    let mut ids = 0;
    let mut classes = 0;
    let mut elements = 0;
    for part in parts {
        match part {
            SelectorPart::Id(_) => ids += 1,
            SelectorPart::Class(_) | SelectorPart::State(_) => classes += 1,
            SelectorPart::Element(_) => elements += 1,
            SelectorPart::DirectChild | SelectorPart::Descendant => {}
        }
    }
    Specificity {
        ids,
        classes,
        elements,
    }
}

fn runtime_token_error(error: &crate::TokenError) -> RuntimeStyleError {
    RuntimeStyleError::new(
        "runtime-style.token.missing",
        error.diagnostic().message().to_string(),
    )
}

fn token_to_style_value(value: &TokenValue) -> StyleValue {
    match value {
        TokenValue::ColorRgba(r, g, b, a) => StyleValue::ColorRgba(*r, *g, *b, *a),
        TokenValue::LengthPx(value) => StyleValue::LengthPx(*value),
        TokenValue::DurationMs(value) => StyleValue::DurationMs(*value),
        TokenValue::Typography { size_px, .. } => StyleValue::LengthPx(*size_px),
        TokenValue::PreferenceHook(target) => StyleValue::TokenRef(target.clone()),
    }
}

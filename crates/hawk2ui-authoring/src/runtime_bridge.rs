//! Native authoring to runtime view bridge.

use hawk2ui_api::Diagnostic;
use hawk2ui_layout::{FlexDirection, LayoutSizing, LayoutStyle};
use hawk2ui_render::{Color, CustomSurfaceCategory};
use hawk2ui_runtime::{
    RuntimeCustomSurfaceVisual, RuntimeSceneError, RuntimeTextVisual, RuntimeViewId,
    RuntimeViewNode, RuntimeViewTree, RuntimeVisual,
};
use hawk2ui_style::{
    CompiledStyleSheet, PropertyId, RuntimeStyleError, RuntimeStyleTable, StyleValue, TokenSet,
};

use crate::{
    AuthoringArtifact, ElementKind, NativeAuthoringArtifact, NativeAuthoringElement, NativeChild,
    PropValue, StyleRef,
};
use crate::{limits::MAX_AUTHORING_TREE_DEPTH, operation_keys};

/// Converts native authoring records into runtime view records.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NativeRuntimeBridge;

impl NativeRuntimeBridge {
    /// Creates a native runtime bridge.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Bridges a finalized native authoring artifact into a runtime tree and metadata.
    ///
    /// # Errors
    ///
    /// Returns [`NativeRuntimeBridgeError`] when runtime tree construction or property mapping
    /// fails.
    pub fn bridge_artifact(
        self,
        artifact: &NativeAuthoringArtifact,
    ) -> Result<NativeRuntimeBridgeArtifact, NativeRuntimeBridgeError> {
        let mut bridged = Self::bridge_element_with_style_resources(artifact.root(), None)?;
        bridged.operation_keys = artifact.operation_keys().to_vec();
        Ok(bridged)
    }

    /// Bridges a source-compiled authoring artifact into a runtime tree.
    ///
    /// The line compiler emits component records. This adapter materializes a single compiled
    /// component as a view root and lowers its default slot children, preserving events targeted at
    /// the component root. Multi-component source artifacts are rejected until the source dialect has
    /// explicit routing or composition semantics.
    ///
    /// # Errors
    ///
    /// Returns [`NativeRuntimeBridgeError`] when the artifact has compiler diagnostics, has no
    /// components, has multiple root components, or contains invalid runtime records.
    pub fn bridge_authoring_artifact(
        self,
        artifact: &AuthoringArtifact,
    ) -> Result<NativeRuntimeBridgeArtifact, NativeRuntimeBridgeError> {
        if let Some(diagnostic) = artifact.diagnostics().first() {
            return Err(Self::diagnostic_error(
                "native-runtime.authoring.diagnostic",
                diagnostic,
            ));
        }
        let Some(component) = artifact.components().first() else {
            return Err(NativeRuntimeBridgeError::new(
                "native-runtime.authoring.empty",
                "compiled authoring artifact contains no components",
            ));
        };
        if artifact.components().len() > 1 {
            return Err(NativeRuntimeBridgeError::new(
                "native-runtime.authoring.multiple-roots",
                "compiled authoring artifact contains multiple root components",
            ));
        }

        let mut root = NativeAuthoringElement::new(component.id().as_str(), ElementKind::View);
        for event in artifact
            .events()
            .iter()
            .filter(|event| event.target().as_str() == component.id().as_str())
        {
            root = root.with_event(
                event.event().clone(),
                event.handler().as_str(),
                event.payload_fields().iter().copied(),
            );
        }
        if let Some(default_slot) = component.slot("default") {
            for child in default_slot.iter() {
                root = root.with_child(NativeChild::ordered(element_node_to_native(child)));
            }
        }

        let mut bridged = Self::bridge_element_with_style_resources(&root, None)?;
        let mut operation_keys = vec![operation_keys::mount_component_key(component.id().as_str())];
        operation_keys.extend(artifact.events().iter().map(operation_keys::bind_event_key));
        bridged.operation_keys = operation_keys;
        Ok(bridged)
    }

    /// Bridges a finalized native authoring artifact and applies compiled style references.
    ///
    /// # Errors
    ///
    /// Returns [`NativeRuntimeBridgeError`] when runtime tree construction, style resolution, or
    /// property mapping fails.
    pub fn bridge_artifact_with_styles(
        self,
        artifact: &NativeAuthoringArtifact,
        sheet: &CompiledStyleSheet,
        tokens: &TokenSet,
    ) -> Result<NativeRuntimeBridgeArtifact, NativeRuntimeBridgeError> {
        let mut bridged = Self::bridge_element_with_style_resources(
            artifact.root(),
            Some(StyleResources {
                sheet,
                tokens,
                theme: None,
            }),
        )?;
        bridged.operation_keys = artifact.operation_keys().to_vec();
        Ok(bridged)
    }

    /// Bridges a finalized native authoring artifact and applies themed compiled style references.
    ///
    /// # Errors
    ///
    /// Returns [`NativeRuntimeBridgeError`] when runtime tree construction, theme token resolution,
    /// style resolution, or property mapping fails.
    pub fn bridge_artifact_with_theme(
        self,
        artifact: &NativeAuthoringArtifact,
        sheet: &CompiledStyleSheet,
        tokens: &TokenSet,
        theme: &str,
    ) -> Result<NativeRuntimeBridgeArtifact, NativeRuntimeBridgeError> {
        let mut bridged = Self::bridge_element_with_style_resources(
            artifact.root(),
            Some(StyleResources {
                sheet,
                tokens,
                theme: Some(theme),
            }),
        )?;
        bridged.operation_keys = artifact.operation_keys().to_vec();
        Ok(bridged)
    }

    /// Bridges a native authoring element tree into a runtime tree and metadata.
    ///
    /// # Errors
    ///
    /// Returns [`NativeRuntimeBridgeError`] when runtime tree construction or property mapping
    /// fails.
    pub fn bridge_element(
        self,
        root: &NativeAuthoringElement,
    ) -> Result<NativeRuntimeBridgeArtifact, NativeRuntimeBridgeError> {
        Self::bridge_element_with_style_resources(root, None)
    }

    /// Bridges a native authoring element tree and applies compiled style references.
    ///
    /// # Errors
    ///
    /// Returns [`NativeRuntimeBridgeError`] when runtime tree construction, style resolution, or
    /// property mapping fails.
    pub fn bridge_element_with_styles(
        self,
        root: &NativeAuthoringElement,
        sheet: &CompiledStyleSheet,
        tokens: &TokenSet,
    ) -> Result<NativeRuntimeBridgeArtifact, NativeRuntimeBridgeError> {
        Self::bridge_element_with_style_resources(
            root,
            Some(StyleResources {
                sheet,
                tokens,
                theme: None,
            }),
        )
    }

    /// Bridges a native authoring element tree and applies themed compiled style references.
    ///
    /// # Errors
    ///
    /// Returns [`NativeRuntimeBridgeError`] when runtime tree construction, theme token resolution,
    /// style resolution, or property mapping fails.
    pub fn bridge_element_with_theme(
        self,
        root: &NativeAuthoringElement,
        sheet: &CompiledStyleSheet,
        tokens: &TokenSet,
        theme: &str,
    ) -> Result<NativeRuntimeBridgeArtifact, NativeRuntimeBridgeError> {
        Self::bridge_element_with_style_resources(
            root,
            Some(StyleResources {
                sheet,
                tokens,
                theme: Some(theme),
            }),
        )
    }

    fn bridge_element_with_style_resources(
        root: &NativeAuthoringElement,
        styles: Option<StyleResources<'_>>,
    ) -> Result<NativeRuntimeBridgeArtifact, NativeRuntimeBridgeError> {
        let mut metadata = Vec::new();
        let root_node = runtime_node(root, true, styles)?;
        metadata.push(metadata_for(root));
        let mut tree = RuntimeViewTree::new(root_node);
        for child in root.children() {
            tree = bridge_child(
                root.id().as_str(),
                child.element(),
                tree,
                &mut metadata,
                styles,
                1,
            )?;
        }
        Ok(NativeRuntimeBridgeArtifact {
            runtime_tree: tree,
            metadata,
            operation_keys: element_operation_keys(root),
        })
    }

    fn diagnostic_error(
        rule: &str,
        diagnostic: &crate::AuthoringDiagnostic,
    ) -> NativeRuntimeBridgeError {
        NativeRuntimeBridgeError::new(
            rule,
            format!(
                "compiled authoring artifact contains diagnostic `{}`: {}",
                diagnostic.rule, diagnostic.message
            ),
        )
    }
}

/// Runtime bridge output for a native authoring tree.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeRuntimeBridgeArtifact {
    runtime_tree: RuntimeViewTree,
    metadata: Vec<NativeRuntimeNodeMetadata>,
    operation_keys: Vec<String>,
}

impl NativeRuntimeBridgeArtifact {
    /// Returns the bridged runtime view tree.
    #[must_use]
    pub const fn runtime_tree(&self) -> &RuntimeViewTree {
        &self.runtime_tree
    }

    /// Returns node metadata in traversal order.
    #[must_use]
    pub fn metadata(&self) -> &[NativeRuntimeNodeMetadata] {
        &self.metadata
    }

    /// Returns metadata for a runtime node ID.
    #[must_use]
    pub fn metadata_for(&self, node_id: &str) -> Option<&NativeRuntimeNodeMetadata> {
        self.metadata
            .iter()
            .find(|metadata| metadata.node_id() == node_id)
    }

    /// Returns preserved authoring operation keys.
    #[must_use]
    pub fn operation_keys(&self) -> &[String] {
        &self.operation_keys
    }
}

/// Non-render metadata preserved for framework and host integration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeRuntimeNodeMetadata {
    node_id: String,
    refs: Vec<String>,
    style_refs: Vec<String>,
    asset_paths: Vec<String>,
}

impl NativeRuntimeNodeMetadata {
    /// Returns the runtime node ID.
    #[must_use]
    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    /// Returns native ref names.
    #[must_use]
    pub fn refs(&self) -> &[String] {
        &self.refs
    }

    /// Returns style reference names.
    #[must_use]
    pub fn style_refs(&self) -> &[String] {
        &self.style_refs
    }

    /// Returns asset paths.
    #[must_use]
    pub fn asset_paths(&self) -> &[String] {
        &self.asset_paths
    }
}

/// Native runtime bridge error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeRuntimeBridgeError {
    rule: String,
    message: String,
}

impl NativeRuntimeBridgeError {
    /// Creates a native runtime bridge error.
    #[must_use]
    pub fn new(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            rule: rule.into(),
            message: message.into(),
        }
    }

    /// Returns the stable error rule.
    #[must_use]
    pub fn rule(&self) -> &str {
        &self.rule
    }

    /// Returns the human-readable error message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl From<RuntimeSceneError> for NativeRuntimeBridgeError {
    fn from(error: RuntimeSceneError) -> Self {
        match error {
            RuntimeSceneError::MissingParent(id) => Self::new(
                "native-runtime.tree.missing-parent",
                format!("runtime parent `{id}` does not exist"),
            ),
            RuntimeSceneError::MissingNode(id) => Self::new(
                "native-runtime.tree.missing-node",
                format!("runtime node `{id}` does not exist"),
            ),
            RuntimeSceneError::DuplicateNode(id) => Self::new(
                "native-runtime.tree.duplicate-node",
                format!("runtime node `{id}` already exists"),
            ),
            RuntimeSceneError::InvalidNode(id) => Self::new(
                "native-runtime.tree.invalid-node",
                format!("runtime node `{id}` contains invalid render data"),
            ),
            RuntimeSceneError::InvalidLayer(rule) => Self::new(
                "native-runtime.layer.invalid",
                format!("runtime layer export failed validation rule `{rule}`"),
            ),
        }
    }
}

impl From<RuntimeStyleError> for NativeRuntimeBridgeError {
    fn from(error: RuntimeStyleError) -> Self {
        Self::new(
            error.diagnostic().rule(),
            error.diagnostic().message().to_string(),
        )
    }
}

impl From<NativeRuntimeBridgeError> for Diagnostic {
    fn from(error: NativeRuntimeBridgeError) -> Self {
        Self::error(error.rule, error.message)
    }
}

#[derive(Clone, Copy)]
struct StyleResources<'a> {
    sheet: &'a CompiledStyleSheet,
    tokens: &'a TokenSet,
    theme: Option<&'a str>,
}

fn bridge_child(
    parent_id: &str,
    element: &NativeAuthoringElement,
    mut tree: RuntimeViewTree,
    metadata: &mut Vec<NativeRuntimeNodeMetadata>,
    styles: Option<StyleResources<'_>>,
    depth: usize,
) -> Result<RuntimeViewTree, NativeRuntimeBridgeError> {
    if depth > MAX_AUTHORING_TREE_DEPTH {
        return Err(NativeRuntimeBridgeError::new(
            "native-runtime.tree.depth-exceeded",
            format!("native authoring tree exceeds maximum depth of {MAX_AUTHORING_TREE_DEPTH}"),
        ));
    }
    tree = tree.with_child(
        &RuntimeViewId::new(parent_id),
        runtime_node(element, false, styles)?,
    )?;
    metadata.push(metadata_for(element));
    for child in element.children() {
        tree = bridge_child(
            element.id().as_str(),
            child.element(),
            tree,
            metadata,
            styles,
            depth + 1,
        )?;
    }
    Ok(tree)
}

fn runtime_node(
    element: &NativeAuthoringElement,
    is_root: bool,
    styles: Option<StyleResources<'_>>,
) -> Result<RuntimeViewNode, NativeRuntimeBridgeError> {
    let style_table = runtime_styles(element, styles)?;
    Ok(RuntimeViewNode::new(
        RuntimeViewId::new(element.id().as_str()),
        layout_style(element, is_root)?,
        visual(element, style_table.as_ref())?,
    ))
}

fn runtime_styles(
    element: &NativeAuthoringElement,
    styles: Option<StyleResources<'_>>,
) -> Result<Option<RuntimeStyleTable>, NativeRuntimeBridgeError> {
    let Some(styles) = styles else {
        return Ok(None);
    };
    if element.style_refs().is_empty() {
        return Ok(None);
    }
    let style_refs = element.style_refs().iter().map(StyleRef::name);
    if let Some(theme) = styles.theme {
        RuntimeStyleTable::from_style_refs_for_theme(
            element.id().as_str(),
            styles.sheet,
            style_refs,
            styles.tokens,
            theme,
        )
    } else {
        RuntimeStyleTable::from_style_refs_with_tokens(
            element.id().as_str(),
            styles.sheet,
            style_refs,
            styles.tokens,
        )
    }
    .map(Some)
    .map_err(Into::into)
}

fn layout_style(
    element: &NativeAuthoringElement,
    is_root: bool,
) -> Result<LayoutStyle, NativeRuntimeBridgeError> {
    let width = optional_number(element, "width")?;
    let height = optional_number(element, "height")?;
    let base = if is_root {
        LayoutStyle::flex_container(FlexDirection::Column)
    } else {
        LayoutStyle::custom_measured()
    };
    if let (Some(width), Some(height)) = (width, height) {
        Ok(base.with_size(LayoutSizing::fixed(width, height)))
    } else if is_root {
        Ok(base)
    } else {
        Ok(base.with_size(LayoutSizing::fixed(
            width.unwrap_or(140.0),
            height.unwrap_or(default_height(element.node().kind())),
        )))
    }
}

fn visual(
    element: &NativeAuthoringElement,
    styles: Option<&RuntimeStyleTable>,
) -> Result<RuntimeVisual, NativeRuntimeBridgeError> {
    match element.node().kind() {
        ElementKind::Text => Ok(RuntimeVisual::Text(RuntimeTextVisual::new(
            string_prop(element, "text").unwrap_or_default(),
            optional_positive_number(element, "font_size")?
                .or(styled_positive_length(element, styles, "font-size")?)
                .unwrap_or(16.0),
            color_prop(element, "color")?
                .or_else(|| styled_color(element, styles, "color"))
                .unwrap_or(Color::rgba(255, 255, 255, 255)),
        ))),
        ElementKind::View | ElementKind::Button => Ok(color_prop(element, "background")?
            .or_else(|| styled_color(element, styles, "background-color"))
            .map_or(RuntimeVisual::None, RuntimeVisual::Fill)),
        ElementKind::CustomSurface => Ok(RuntimeVisual::CustomSurface(
            RuntimeCustomSurfaceVisual::new(custom_surface_category(element)?),
        )),
    }
}

fn styled_positive_length(
    element: &NativeAuthoringElement,
    styles: Option<&RuntimeStyleTable>,
    property: &str,
) -> Result<Option<f32>, NativeRuntimeBridgeError> {
    let Some(value) = styles
        .and_then(|styles| styles.typed_value(element.id().as_str(), &PropertyId::new(property)))
    else {
        return Ok(None);
    };
    match value {
        StyleValue::LengthPx(value) if *value > 0.0 && value.is_finite() => Ok(Some(*value)),
        _ => Err(invalid_number(property, NumberDomain::Positive)),
    }
}

fn styled_color(
    element: &NativeAuthoringElement,
    styles: Option<&RuntimeStyleTable>,
    property: &str,
) -> Option<Color> {
    let value = styles?.typed_value(element.id().as_str(), &PropertyId::new(property))?;
    match value {
        StyleValue::ColorRgba(r, g, b, a) => Some(Color::rgba(*r, *g, *b, *a)),
        _ => None,
    }
}

fn metadata_for(element: &NativeAuthoringElement) -> NativeRuntimeNodeMetadata {
    NativeRuntimeNodeMetadata {
        node_id: element.id().as_str().to_string(),
        refs: element
            .refs()
            .iter()
            .map(|reference| reference.name().to_string())
            .collect(),
        style_refs: element
            .style_refs()
            .iter()
            .map(|style| style.name().to_string())
            .collect(),
        asset_paths: element
            .asset_refs()
            .iter()
            .map(|asset| asset.path().to_string())
            .collect(),
    }
}

fn optional_number(
    element: &NativeAuthoringElement,
    name: &str,
) -> Result<Option<f32>, NativeRuntimeBridgeError> {
    match element.node().prop(name) {
        Some(PropValue::Number(value)) => Ok(Some(narrow_number(
            *value,
            name,
            NumberDomain::NonNegative,
        )?)),
        _ => Ok(None),
    }
}

fn optional_positive_number(
    element: &NativeAuthoringElement,
    name: &str,
) -> Result<Option<f32>, NativeRuntimeBridgeError> {
    match element.node().prop(name) {
        Some(PropValue::Number(value)) => {
            Ok(Some(narrow_number(*value, name, NumberDomain::Positive)?))
        }
        _ => Ok(None),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NumberDomain {
    NonNegative,
    Positive,
}

fn narrow_number(
    value: f64,
    name: &str,
    domain: NumberDomain,
) -> Result<f32, NativeRuntimeBridgeError> {
    let valid_domain = match domain {
        NumberDomain::NonNegative => value >= 0.0,
        NumberDomain::Positive => value > 0.0,
    };
    if !value.is_finite() || !valid_domain {
        return Err(invalid_number(name, domain));
    }
    #[allow(clippy::cast_possible_truncation)]
    let narrowed = value as f32;
    if narrowed.is_finite() {
        Ok(narrowed)
    } else {
        Err(invalid_number(name, domain))
    }
}

fn invalid_number(name: &str, domain: NumberDomain) -> NativeRuntimeBridgeError {
    let range = match domain {
        NumberDomain::NonNegative => "finite and non-negative",
        NumberDomain::Positive => "finite and greater than zero",
    };
    NativeRuntimeBridgeError::new(
        "native-runtime.layout.invalid-number",
        format!("property `{name}` must be {range}"),
    )
}

fn string_prop(element: &NativeAuthoringElement, name: &str) -> Option<String> {
    match element.node().prop(name) {
        Some(PropValue::String(value)) => Some(value.clone()),
        _ => None,
    }
}

fn color_prop(
    element: &NativeAuthoringElement,
    name: &str,
) -> Result<Option<Color>, NativeRuntimeBridgeError> {
    string_prop(element, name)
        .map(|value| parse_hex_color(name, &value))
        .transpose()
}

fn parse_hex_color(name: &str, value: &str) -> Result<Color, NativeRuntimeBridgeError> {
    let Some(hex) = value.strip_prefix('#') else {
        return Err(invalid_color(name));
    };
    if hex.len() != 6 || !hex.as_bytes().iter().all(u8::is_ascii_hexdigit) {
        return Err(invalid_color(name));
    }
    let red = parse_hex_channel(name, &hex[0..2])?;
    let green = parse_hex_channel(name, &hex[2..4])?;
    let blue = parse_hex_channel(name, &hex[4..6])?;
    Ok(Color::rgba(red, green, blue, 255))
}

fn parse_hex_channel(name: &str, value: &str) -> Result<u8, NativeRuntimeBridgeError> {
    u8::from_str_radix(value, 16).map_err(|_| invalid_color(name))
}

fn invalid_color(name: &str) -> NativeRuntimeBridgeError {
    NativeRuntimeBridgeError::new(
        "native-runtime.color.invalid",
        format!("property `{name}` must be a #rrggbb color"),
    )
}

const fn default_height(kind: ElementKind) -> f32 {
    match kind {
        ElementKind::Text => 28.0,
        ElementKind::Button | ElementKind::View | ElementKind::CustomSurface => 40.0,
    }
}

fn element_node_to_native(node: &crate::ElementNode) -> NativeAuthoringElement {
    let mut element = NativeAuthoringElement::new(node.id().as_str(), node.kind());
    for (name, value) in node.props() {
        element = element.with_prop(name, value.clone());
    }
    element
}

fn element_operation_keys(root: &NativeAuthoringElement) -> Vec<String> {
    let mut keys = Vec::new();
    collect_element_operation_keys(root, &mut keys);
    keys
}

fn collect_element_operation_keys(element: &NativeAuthoringElement, keys: &mut Vec<String>) {
    keys.push(operation_keys::mount_element_key(element.id()));
    for child in element.children() {
        collect_element_operation_keys(child.element(), keys);
    }
}

fn custom_surface_category(
    element: &NativeAuthoringElement,
) -> Result<CustomSurfaceCategory, NativeRuntimeBridgeError> {
    let Some(category) = string_prop(element, "surface_category") else {
        return Err(NativeRuntimeBridgeError::new(
            "native-runtime.custom-surface.category-missing",
            "custom surface elements require a `surface_category` string property",
        ));
    };
    match category.as_str() {
        "knob" => Ok(CustomSurfaceCategory::Knob),
        "slider" => Ok(CustomSurfaceCategory::Slider),
        "meter" => Ok(CustomSurfaceCategory::Meter),
        "scope" => Ok(CustomSurfaceCategory::Scope),
        "analyzer" => Ok(CustomSurfaceCategory::Analyzer),
        "eq-curve" => Ok(CustomSurfaceCategory::EqCurve),
        "modulation" => Ok(CustomSurfaceCategory::Modulation),
        "timeline" => Ok(CustomSurfaceCategory::Timeline),
        "graph-editor" => Ok(CustomSurfaceCategory::GraphEditor),
        "inspector-panel" => Ok(CustomSurfaceCategory::InspectorPanel),
        _ => Err(NativeRuntimeBridgeError::new(
            "native-runtime.custom-surface.category-invalid",
            format!("custom surface category `{category}` is not supported"),
        )),
    }
}

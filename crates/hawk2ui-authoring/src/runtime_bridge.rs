//! Native authoring to runtime view bridge.

use hawk2ui_api::Diagnostic;
use hawk2ui_layout::{FlexDirection, LayoutSizing, LayoutStyle, LayoutValue};
use hawk2ui_render::{Color, CustomSurfaceCategory, Transform};
use hawk2ui_runtime::{
    RuntimeCustomSurfaceVisual, RuntimeGlowEffect, RuntimeLinearGradient, RuntimeSceneError,
    RuntimeShadowEffect, RuntimeStyledBoxVisual, RuntimeTextVisual, RuntimeViewId, RuntimeViewNode,
    RuntimeViewTree, RuntimeVisual,
};
use hawk2ui_style::{
    CompiledStyleSheet, PropertyId, RuntimeStyleError, RuntimeStyleTable, StyleValue, TokenSet,
};

use crate::adapter::{FrameworkDynamicBinding, FrameworkDynamicBindingTarget};
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
        bridged.dynamic_bindings = artifact.dynamic_bindings().to_vec();
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
        bridged.dynamic_bindings = artifact.dynamic_bindings().to_vec();
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
        bridged.dynamic_bindings = artifact.dynamic_bindings().to_vec();
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
            dynamic_bindings: Vec::new(),
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
    dynamic_bindings: Vec<FrameworkDynamicBinding>,
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

    /// Returns runtime dynamic bindings in compiler declaration order.
    #[must_use]
    pub fn dynamic_bindings(&self) -> &[FrameworkDynamicBinding] {
        &self.dynamic_bindings
    }

    /// Applies one evaluated dynamic binding value to the runtime tree.
    ///
    /// Text bindings patch the node's [`RuntimeVisual::Text`] payload and mark the node as
    /// invalidated. Non-text property targets are rejected until the runtime bridge has typed
    /// patchers for those native properties.
    ///
    /// # Errors
    ///
    /// Returns [`NativeRuntimeBridgeError`] when the binding target is unsupported, the target node
    /// is missing, or the target node is not a text visual.
    pub fn apply_dynamic_binding(
        mut self,
        binding: &FrameworkDynamicBinding,
        value: PropValue,
    ) -> Result<Self, NativeRuntimeBridgeError> {
        match binding.target() {
            FrameworkDynamicBindingTarget::Text => {
                let text = dynamic_value_text(value)?;
                self.runtime_tree = apply_dynamic_text(self.runtime_tree, binding.node_id(), text)?;
                Ok(self)
            }
            FrameworkDynamicBindingTarget::Prop { name } if name == "text" => {
                let text = dynamic_value_text(value)?;
                self.runtime_tree = apply_dynamic_text(self.runtime_tree, binding.node_id(), text)?;
                Ok(self)
            }
            FrameworkDynamicBindingTarget::Prop { name } if name == "width" || name == "height" => {
                let size_value = dynamic_value_layout_number(&value, name)?;
                self.runtime_tree = apply_dynamic_layout_size(
                    self.runtime_tree,
                    binding.node_id(),
                    name,
                    size_value,
                )?;
                Ok(self)
            }
            FrameworkDynamicBindingTarget::Prop { name } => Err(NativeRuntimeBridgeError::new(
                "native-runtime.dynamic-binding.unsupported-target",
                format!(
                    "dynamic binding target `{}:{name}` is not supported by the runtime bridge",
                    binding.node_id()
                ),
            )),
        }
    }
}

fn apply_dynamic_text(
    tree: RuntimeViewTree,
    node_id: &str,
    text: String,
) -> Result<RuntimeViewTree, NativeRuntimeBridgeError> {
    let runtime_id = RuntimeViewId::new(node_id);
    let Some(node) = tree.node(&runtime_id) else {
        return Err(NativeRuntimeBridgeError::new(
            "native-runtime.dynamic-binding.node-missing",
            format!("dynamic binding target node `{node_id}` is not present in the runtime tree"),
        ));
    };
    let visual = match node.visual().clone() {
        RuntimeVisual::Text(text_visual) => RuntimeVisual::Text(text_visual.with_text(text)),
        _ => {
            return Err(NativeRuntimeBridgeError::new(
                "native-runtime.dynamic-binding.visual-kind-invalid",
                format!("dynamic text binding target `{node_id}` is not a text visual"),
            ));
        }
    };
    tree.update_visual(&runtime_id, visual)
        .map_err(NativeRuntimeBridgeError::from)
}

fn apply_dynamic_layout_size(
    tree: RuntimeViewTree,
    node_id: &str,
    name: &str,
    value: f32,
) -> Result<RuntimeViewTree, NativeRuntimeBridgeError> {
    let runtime_id = RuntimeViewId::new(node_id);
    let Some(node) = tree.node(&runtime_id) else {
        return Err(NativeRuntimeBridgeError::new(
            "native-runtime.dynamic-binding.node-missing",
            format!("dynamic binding target node `{node_id}` is not present in the runtime tree"),
        ));
    };
    let mut layout_style = node.layout_style().clone();
    let current_size = layout_style.size();
    let next_size = if name == "width" {
        LayoutSizing::new(LayoutValue::px(value), current_size.height())
    } else {
        LayoutSizing::new(current_size.width(), LayoutValue::px(value))
    };
    layout_style = layout_style.with_size(next_size);
    tree.update_layout_style(&runtime_id, layout_style)
        .map_err(NativeRuntimeBridgeError::from)
}

fn dynamic_value_text(value: PropValue) -> Result<String, NativeRuntimeBridgeError> {
    match value {
        PropValue::String(value) => Ok(value),
        PropValue::Bool(value) => Ok(value.to_string()),
        PropValue::Number(value) if value.is_finite() => Ok(value.to_string()),
        PropValue::Number(_) => Err(NativeRuntimeBridgeError::new(
            "native-runtime.dynamic-binding.value-invalid",
            "dynamic text binding numeric value must be finite",
        )),
    }
}

fn dynamic_value_layout_number(
    value: &PropValue,
    name: &str,
) -> Result<f32, NativeRuntimeBridgeError> {
    match value {
        PropValue::Number(value) => narrow_number(*value, name, NumberDomain::NonNegative),
        PropValue::String(_) | PropValue::Bool(_) => Err(NativeRuntimeBridgeError::new(
            "native-runtime.dynamic-binding.value-invalid",
            format!("dynamic layout binding `{name}` requires a finite non-negative number"),
        )),
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
        ElementKind::View | ElementKind::Button => {
            Ok(styled_box_visual(element, styles)?.unwrap_or(RuntimeVisual::None))
        }
        ElementKind::CustomSurface => Ok(RuntimeVisual::CustomSurface(
            RuntimeCustomSurfaceVisual::new(custom_surface_category(element)?),
        )),
    }
}

fn styled_box_visual(
    element: &NativeAuthoringElement,
    styles: Option<&RuntimeStyleTable>,
) -> Result<Option<RuntimeVisual>, NativeRuntimeBridgeError> {
    let fill = color_prop(element, "background")?
        .or_else(|| styled_color(element, styles, "background-color"));
    let gradient = match (
        styled_color(element, styles, "background-gradient-start"),
        styled_color(element, styles, "background-gradient-end"),
    ) {
        (Some(start), Some(end)) if has_alpha(start) || has_alpha(end) => {
            Some(RuntimeLinearGradient::new(start, end))
        }
        _ => None,
    };
    let border_radius =
        styled_non_negative_length(element, styles, "border-radius")?.unwrap_or(0.0);
    let shadow = styled_shadow_effect(element, styles)?;
    let glow = styled_glow_effect(element, styles)?;
    let opacity = styled_opacity(element, styles)?;
    let transform = styled_transform(element, styles)?;

    let has_effects = gradient.is_some()
        || border_radius > 0.0
        || shadow.is_some()
        || glow.is_some()
        || opacity < 1.0
        || transform != Transform::identity();
    if !has_effects {
        return Ok(fill.map(RuntimeVisual::Fill));
    }

    let mut visual = RuntimeStyledBoxVisual::new()
        .with_border_radius(border_radius)
        .with_opacity(opacity)
        .with_transform(transform);
    if let Some(fill) = fill {
        visual = visual.with_fill(fill);
    }
    if let Some(gradient) = gradient {
        visual = visual.with_gradient(gradient);
    }
    if let Some(shadow) = shadow {
        visual = visual.with_shadow(shadow);
    }
    if let Some(glow) = glow {
        visual = visual.with_glow(glow);
    }
    Ok(Some(RuntimeVisual::StyledBox(visual)))
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

fn styled_non_negative_length(
    element: &NativeAuthoringElement,
    styles: Option<&RuntimeStyleTable>,
    property: &str,
) -> Result<Option<f32>, NativeRuntimeBridgeError> {
    let Some(value) = typed_style_value(element, styles, property) else {
        return Ok(None);
    };
    match value {
        StyleValue::LengthPx(value) if *value >= 0.0 && value.is_finite() => Ok(Some(*value)),
        _ => Err(invalid_number(property, NumberDomain::NonNegative)),
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

fn typed_style_value<'a>(
    element: &NativeAuthoringElement,
    styles: Option<&'a RuntimeStyleTable>,
    property: &str,
) -> Option<&'a StyleValue> {
    styles?.typed_value(element.id().as_str(), &PropertyId::new(property))
}

fn styled_shadow_effect(
    element: &NativeAuthoringElement,
    styles: Option<&RuntimeStyleTable>,
) -> Result<Option<RuntimeShadowEffect>, NativeRuntimeBridgeError> {
    let Some(value) = typed_style_value(element, styles, "box-shadow") else {
        return Ok(None);
    };
    match value {
        StyleValue::Shadow(value) if value.trim().eq_ignore_ascii_case("none") => Ok(None),
        StyleValue::Shadow(value) => parse_shadow_effect("box-shadow", value).map(Some),
        _ => Err(invalid_effect("box-shadow")),
    }
}

fn styled_glow_effect(
    element: &NativeAuthoringElement,
    styles: Option<&RuntimeStyleTable>,
) -> Result<Option<RuntimeGlowEffect>, NativeRuntimeBridgeError> {
    let Some(radius) = styled_non_negative_length(element, styles, "glow-radius")? else {
        return Ok(None);
    };
    let color = styled_color(element, styles, "glow-color").unwrap_or(Color::rgba(0, 0, 0, 0));
    if radius == 0.0 || !has_alpha(color) {
        return Ok(None);
    }
    Ok(Some(RuntimeGlowEffect::new(radius, color)))
}

fn styled_opacity(
    element: &NativeAuthoringElement,
    styles: Option<&RuntimeStyleTable>,
) -> Result<f32, NativeRuntimeBridgeError> {
    let Some(value) = typed_style_value(element, styles, "opacity") else {
        return Ok(1.0);
    };
    match value {
        StyleValue::Number(value) if value.is_finite() && (0.0..=1.0).contains(value) => Ok(*value),
        _ => Err(invalid_number("opacity", NumberDomain::NonNegative)),
    }
}

fn styled_transform(
    element: &NativeAuthoringElement,
    styles: Option<&RuntimeStyleTable>,
) -> Result<Transform, NativeRuntimeBridgeError> {
    let Some(value) = typed_style_value(element, styles, "transform") else {
        return Ok(Transform::identity());
    };
    match value {
        StyleValue::Transform(value) if value.trim().eq_ignore_ascii_case("none") => {
            Ok(Transform::identity())
        }
        StyleValue::Transform(value) => parse_transform_effect("transform", value),
        _ => Err(invalid_effect("transform")),
    }
}

fn parse_shadow_effect(
    property: &str,
    value: &str,
) -> Result<RuntimeShadowEffect, NativeRuntimeBridgeError> {
    let mut lengths = Vec::new();
    let mut color = None;
    for token in split_top_level_whitespace(value) {
        if token.eq_ignore_ascii_case("inset") {
            continue;
        }
        if let Ok(parsed_color) = parse_effect_color(property, &token) {
            color = Some(parsed_color);
            continue;
        }
        if let Some(length) = parse_px_or_zero(&token) {
            lengths.push(length);
            continue;
        }
        return Err(invalid_effect(property));
    }
    if lengths.len() < 2 || lengths.len() > 4 {
        return Err(invalid_effect(property));
    }
    let blur_radius = lengths.get(2).copied().unwrap_or(0.0);
    if blur_radius < 0.0 || lengths.get(3).is_some_and(|spread| *spread < 0.0) {
        return Err(invalid_effect(property));
    }
    Ok(RuntimeShadowEffect::new(
        lengths[0],
        lengths[1],
        blur_radius,
        color.unwrap_or(Color::rgba(0, 0, 0, 255)),
    ))
}

fn parse_transform_effect(
    property: &str,
    value: &str,
) -> Result<Transform, NativeRuntimeBridgeError> {
    let mut transform = Transform::identity();
    let functions = split_transform_functions(value);
    if functions.is_empty() {
        return Err(invalid_effect(property));
    }
    for (name, args) in functions {
        let next = parse_transform_function(property, &name, &args)?;
        transform = multiply_transform(transform, next);
    }
    if transform.is_finite() {
        Ok(transform)
    } else {
        Err(invalid_effect(property))
    }
}

fn parse_transform_function(
    property: &str,
    name: &str,
    args: &str,
) -> Result<Transform, NativeRuntimeBridgeError> {
    let args = split_function_arguments(args);
    match name.to_ascii_lowercase().as_str() {
        "translatex" if args.len() == 1 => Ok(Transform::translate(
            parse_transform_length(property, &args[0])?,
            0.0,
        )),
        "translatey" if args.len() == 1 => Ok(Transform::translate(
            0.0,
            parse_transform_length(property, &args[0])?,
        )),
        "translate" if (1..=2).contains(&args.len()) => Ok(Transform::translate(
            parse_transform_length(property, &args[0])?,
            args.get(1)
                .map_or(Ok(0.0), |arg| parse_transform_length(property, arg))?,
        )),
        "scale" if (1..=2).contains(&args.len()) => {
            let scale_x = parse_unitless_number(property, &args[0])?;
            let scale_y = args
                .get(1)
                .map_or(Ok(scale_x), |arg| parse_unitless_number(property, arg))?;
            Ok(Transform::affine(scale_x, 0.0, 0.0, scale_y, 0.0, 0.0))
        }
        "rotate" if args.len() == 1 => {
            let radians = parse_angle_radians(property, &args[0])?;
            let (sin, cos) = radians.sin_cos();
            Ok(Transform::affine(cos, -sin, sin, cos, 0.0, 0.0))
        }
        "matrix" if args.len() == 6 => Ok(Transform::affine(
            parse_unitless_number(property, &args[0])?,
            parse_unitless_number(property, &args[2])?,
            parse_unitless_number(property, &args[1])?,
            parse_unitless_number(property, &args[3])?,
            parse_unitless_number(property, &args[4])?,
            parse_unitless_number(property, &args[5])?,
        )),
        _ => Err(invalid_effect(property)),
    }
}

fn split_transform_functions(value: &str) -> Vec<(String, String)> {
    let mut functions = Vec::new();
    let mut input = value.trim();
    while !input.is_empty() {
        let Some(open) = input.find('(') else {
            return Vec::new();
        };
        let name = input[..open].trim();
        if name.is_empty() {
            return Vec::new();
        }
        let mut depth = 0i32;
        let mut close = None;
        for (index, character) in input.char_indices().skip(open) {
            match character {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(index);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(close) = close else {
            return Vec::new();
        };
        functions.push((name.to_string(), input[open + 1..close].trim().to_string()));
        input = input[close + 1..].trim_start();
    }
    functions
}

fn split_top_level_whitespace(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut depth = 0i32;
    for character in value.chars() {
        match character {
            '(' => {
                depth += 1;
                token.push(character);
            }
            ')' => {
                depth -= 1;
                token.push(character);
            }
            character if character.is_whitespace() && depth == 0 => {
                if !token.trim().is_empty() {
                    tokens.push(token.trim().to_string());
                    token.clear();
                }
            }
            _ => token.push(character),
        }
    }
    if !token.trim().is_empty() {
        tokens.push(token.trim().to_string());
    }
    tokens
}

fn split_function_arguments(value: &str) -> Vec<String> {
    let value = value.replace('/', " ");
    if value.contains(',') {
        value
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(ToString::to_string)
            .collect()
    } else {
        value
            .split_whitespace()
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(ToString::to_string)
            .collect()
    }
}

fn parse_transform_length(property: &str, value: &str) -> Result<f32, NativeRuntimeBridgeError> {
    parse_px_or_zero(value).ok_or_else(|| invalid_effect(property))
}

fn parse_px_or_zero(value: &str) -> Option<f32> {
    let parsed = if value == "0" {
        0.0
    } else {
        value.strip_suffix("px")?.parse::<f32>().ok()?
    };
    parsed.is_finite().then_some(parsed)
}

fn parse_unitless_number(property: &str, value: &str) -> Result<f32, NativeRuntimeBridgeError> {
    let value = value.parse::<f32>().map_err(|_| invalid_effect(property))?;
    value
        .is_finite()
        .then_some(value)
        .ok_or_else(|| invalid_effect(property))
}

fn parse_angle_radians(property: &str, value: &str) -> Result<f32, NativeRuntimeBridgeError> {
    if let Some(degrees) = value.strip_suffix("deg") {
        return Ok(parse_unitless_number(property, degrees)?.to_radians());
    }
    if let Some(radians) = value.strip_suffix("rad") {
        return parse_unitless_number(property, radians);
    }
    Err(invalid_effect(property))
}

fn multiply_transform(lhs: Transform, rhs: Transform) -> Transform {
    Transform::affine(
        lhs.scale_x.mul_add(rhs.scale_x, lhs.skew_x * rhs.skew_y),
        lhs.scale_x.mul_add(rhs.skew_x, lhs.skew_x * rhs.scale_y),
        lhs.skew_y.mul_add(rhs.scale_x, lhs.scale_y * rhs.skew_y),
        lhs.skew_y.mul_add(rhs.skew_x, lhs.scale_y * rhs.scale_y),
        lhs.scale_x.mul_add(
            rhs.translate_x,
            lhs.skew_x.mul_add(rhs.translate_y, lhs.translate_x),
        ),
        lhs.skew_y.mul_add(
            rhs.translate_x,
            lhs.scale_y.mul_add(rhs.translate_y, lhs.translate_y),
        ),
    )
}

fn parse_effect_color(property: &str, value: &str) -> Result<Color, NativeRuntimeBridgeError> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("transparent") {
        return Ok(Color::rgba(0, 0, 0, 0));
    }
    if value.eq_ignore_ascii_case("black") {
        return Ok(Color::rgba(0, 0, 0, 255));
    }
    if value.eq_ignore_ascii_case("white") {
        return Ok(Color::rgba(255, 255, 255, 255));
    }
    if value.starts_with('#') {
        return parse_effect_hex_color(property, value);
    }
    if value.starts_with("rgb(") || value.starts_with("rgba(") {
        return parse_effect_rgb_color(property, value);
    }
    Err(invalid_effect(property))
}

fn parse_effect_hex_color(property: &str, value: &str) -> Result<Color, NativeRuntimeBridgeError> {
    let hex = value
        .strip_prefix('#')
        .ok_or_else(|| invalid_effect(property))?;
    if !hex.as_bytes().iter().all(u8::is_ascii_hexdigit) {
        return Err(invalid_effect(property));
    }
    match hex.len() {
        3 | 4 => {
            let r = parse_short_hex_channel(property, &hex[0..1])?;
            let g = parse_short_hex_channel(property, &hex[1..2])?;
            let b = parse_short_hex_channel(property, &hex[2..3])?;
            let a = if hex.len() == 4 {
                parse_short_hex_channel(property, &hex[3..4])?
            } else {
                255
            };
            Ok(Color::rgba(r, g, b, a))
        }
        6 | 8 => {
            let r = parse_hex_channel(property, &hex[0..2])?;
            let g = parse_hex_channel(property, &hex[2..4])?;
            let b = parse_hex_channel(property, &hex[4..6])?;
            let a = if hex.len() == 8 {
                parse_hex_channel(property, &hex[6..8])?
            } else {
                255
            };
            Ok(Color::rgba(r, g, b, a))
        }
        _ => Err(invalid_effect(property)),
    }
}

fn parse_short_hex_channel(property: &str, value: &str) -> Result<u8, NativeRuntimeBridgeError> {
    let expanded = format!("{value}{value}");
    parse_hex_channel(property, &expanded)
}

fn parse_effect_rgb_color(property: &str, value: &str) -> Result<Color, NativeRuntimeBridgeError> {
    let open = value.find('(').ok_or_else(|| invalid_effect(property))?;
    let inner = value[open + 1..]
        .strip_suffix(')')
        .ok_or_else(|| invalid_effect(property))?;
    let args = split_function_arguments(inner);
    if args.len() != 3 && args.len() != 4 {
        return Err(invalid_effect(property));
    }
    Ok(Color::rgba(
        parse_rgb_channel(property, &args[0])?,
        parse_rgb_channel(property, &args[1])?,
        parse_rgb_channel(property, &args[2])?,
        args.get(3)
            .map_or(Ok(255), |alpha| parse_alpha_channel(property, alpha))?,
    ))
}

fn parse_rgb_channel(property: &str, value: &str) -> Result<u8, NativeRuntimeBridgeError> {
    if let Some(percent) = value.strip_suffix('%') {
        let value = parse_unitless_number(property, percent)?;
        if !(0.0..=100.0).contains(&value) {
            return Err(invalid_effect(property));
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        return Ok(((value / 100.0) * 255.0).round() as u8);
    }
    let value = parse_unitless_number(property, value)?;
    if !(0.0..=255.0).contains(&value) {
        return Err(invalid_effect(property));
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Ok(value.round() as u8)
}

fn parse_alpha_channel(property: &str, value: &str) -> Result<u8, NativeRuntimeBridgeError> {
    if let Some(percent) = value.strip_suffix('%') {
        let value = parse_unitless_number(property, percent)?;
        if !(0.0..=100.0).contains(&value) {
            return Err(invalid_effect(property));
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        return Ok(((value / 100.0) * 255.0).round() as u8);
    }
    let value = parse_unitless_number(property, value)?;
    if !(0.0..=1.0).contains(&value) {
        return Err(invalid_effect(property));
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Ok((value * 255.0).round() as u8)
}

const fn has_alpha(color: Color) -> bool {
    color.a > 0
}

fn invalid_effect(name: &str) -> NativeRuntimeBridgeError {
    NativeRuntimeBridgeError::new(
        "native-runtime.effect.invalid",
        format!("property `{name}` contains an unsupported renderer effect value"),
    )
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

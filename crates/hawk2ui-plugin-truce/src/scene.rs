//! Building a `Hawk2UI` editor scene from a compiled entry script.
//!
//! A plugin editor's UI is JS-driven: the author's entry script returns a node
//! tree, not a baked scene. [`build_editor_scene`] runs that script's `mount`
//! function through `hawk2ui-script`'s compatibility [`ScriptBackend`] and converts
//! the serialized node tree into a renderable [`RuntimeSceneFrame`], using the
//! [`entry_mount_bootstrap_with_host`] convention and [`EntryNode`] conversion —
//! the host-projecting sibling of the no-op `entry_mount_bootstrap` the desktop
//! host (driven by the CLI) uses, since a plugin editor reads parameters and
//! meters that a desktop app has none of.
//!
//! This module only *calls* the script engine and performs pure data
//! transformation: there is no windowing. The injected `__hawk2ui_host` carries
//! the [`HostSnapshot`] the caller passes (parameters and meters projected from
//! the plugin's parameter model — see `hawk2ui-build`'s `host_snapshot_from_model`).
//! The snapshot here is the model's declared defaults; re-projecting it from the
//! live truce `EditorBridge` happens in the live render cycle. Being pure, the
//! builder is unit-tested directly in the fast gate.

use hawk2ui_layout::Viewport;
use hawk2ui_runtime::{EntryNode, RuntimeSceneBridge, RuntimeSceneFrame};
use hawk2ui_script::{
    FrameInput, HostCallPolicy, HostEdit, HostSnapshot, ScriptBackend, ScriptModule,
    StructuredValue, TimerPolicy, entry_mount_bootstrap_with_host, parse_entry_envelope,
};

/// Error building an editor scene from a compiled entry script.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorSceneError {
    rule: String,
    message: String,
}

impl EditorSceneError {
    /// Creates an editor scene error.
    #[must_use]
    pub(crate) fn new(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            rule: rule.into(),
            message: message.into(),
        }
    }

    /// Returns the diagnostic rule.
    #[must_use]
    pub fn rule(&self) -> &str {
        &self.rule
    }

    /// Returns the human-readable message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for EditorSceneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.rule, self.message)
    }
}

impl std::error::Error for EditorSceneError {}

/// A single built editor frame: the renderable scene the entry produced, plus
/// the ordered parameter [`HostEdit`]s it emitted and the UI-state blob it
/// returned (the locked `{ tree, edits, ui }` envelope, split apart). The
/// construction path ([`build_editor_scene`]) keeps only the scene; the live
/// render cycle replays the edits onto the bridge and persists `ui_json` to
/// thread back into the next invocation.
pub(crate) struct EditorFrame {
    /// The renderable scene built from the entry's view tree.
    pub(crate) scene: RuntimeSceneFrame,
    /// The ordered parameter edits the entry emitted this invocation. The live
    /// render cycle replays them onto the bridge; the construction path drops
    /// them (no bridge yet).
    pub(crate) edits: Vec<HostEdit>,
    /// The entry's outgoing UI-state blob as a JSON string, re-embedded into the
    /// next invocation by the live render cycle; the construction path drops it.
    pub(crate) ui_json: String,
}

/// Runs a compiled entry script's `mount` once and returns the built
/// [`EditorFrame`] — the scene plus the entry's emitted edits and outgoing UI
/// state.
///
/// The script runs under [`HostCallPolicy::deny_all`] with deterministic timers.
/// `snapshot` is projected into the `__hawk2ui_host` the script reads (a frozen,
/// pure-data view of parameters and meters, embedded as a JSON literal — no host
/// call, `deny_all` preserved), and `incoming_ui` threads the prior invocation's
/// UI blob (`"null"` for the first frame). The scene is sized to `width` x
/// `height` points.
///
/// # Errors
///
/// Returns an [`EditorSceneError`] when the script declares no `mount` function,
/// fails to execute, returns a non-string result, or yields a node tree that
/// cannot be parsed or converted into a renderable scene.
pub(crate) fn build_editor_frame(
    compiled_source: &str,
    source_path: &str,
    snapshot: &HostSnapshot,
    events: &[FrameInput],
    incoming_ui: &str,
    width: f32,
    height: f32,
) -> Result<EditorFrame, EditorSceneError> {
    let Some(bootstrap) =
        entry_mount_bootstrap_with_host(compiled_source, snapshot, events, incoming_ui)
    else {
        return Err(EditorSceneError::new(
            "hawk2ui-truce.editor.no-mount",
            "editor entry script declares no `mount` function to build a scene from",
        ));
    };

    let mut backend = ScriptBackend::new(HostCallPolicy::deny_all(), TimerPolicy::deterministic());
    // The sealed artifact carries `compiled_source` already transpiled to
    // JavaScript (hawk2ui-build's `compiled_script` runs every entry through
    // `ScriptBackend::compile_module_source`), so run it as JavaScript. Inferring
    // the kind from `source_path` would re-run the TypeScript transform on
    // already-compiled output whenever the author's entry was a `.ts`/`.tsx` file.
    let execution = backend
        .execute_module(ScriptModule::javascript(source_path, bootstrap.as_str()))
        .map_err(|error| {
            EditorSceneError::new(
                "hawk2ui-truce.editor.entry-script-failed",
                format!(
                    "editor entry script failed to execute: {}",
                    error.diagnostic().rule()
                ),
            )
        })?;

    let StructuredValue::String(envelope_json) = execution.value() else {
        return Err(EditorSceneError::new(
            "hawk2ui-truce.editor.invalid-entry-tree",
            "editor `mount` must return a serializable view or text node tree",
        ));
    };

    let envelope = parse_entry_envelope(envelope_json).map_err(|error| {
        EditorSceneError::new(
            "hawk2ui-truce.editor.invalid-entry-tree",
            error.message().to_string(),
        )
    })?;
    let root = EntryNode::from_tree_json(&envelope.tree_json).map_err(|message| {
        EditorSceneError::new("hawk2ui-truce.editor.invalid-entry-tree", message)
    })?;
    let tree = root.to_view_tree(width, height).map_err(|error| {
        EditorSceneError::new(
            "hawk2ui-truce.editor.scene-build-failed",
            format!("{error:?}"),
        )
    })?;
    let scene = RuntimeSceneBridge::new(Viewport::new(width, height))
        .build(&tree)
        .map_err(|error| {
            EditorSceneError::new(
                "hawk2ui-truce.editor.scene-build-failed",
                format!("{error:?}"),
            )
        })?;
    Ok(EditorFrame {
        scene,
        edits: envelope.edits,
        ui_json: envelope.ui_json,
    })
}

/// Builds a renderable editor scene from a compiled entry script, sized to
/// `width` x `height` points — the construction-time path.
///
/// A thin wrapper over [`build_editor_frame`] that keeps only the scene: the
/// construction path has no prior UI state to thread (so it passes `"null"`) and
/// no bridge yet to replay the entry's edits onto (so they are dropped). The live
/// per-frame cycle uses [`build_editor_frame`] directly to also replay edits and
/// persist the UI blob. Pass an empty [`HostSnapshot`] for a paramless editor.
///
/// # Errors
///
/// Propagates the [`EditorSceneError`] from [`build_editor_frame`].
pub(crate) fn build_editor_scene(
    compiled_source: &str,
    source_path: &str,
    snapshot: &HostSnapshot,
    width: f32,
    height: f32,
) -> Result<RuntimeSceneFrame, EditorSceneError> {
    build_editor_frame(
        compiled_source,
        source_path,
        snapshot,
        &[],
        "null",
        width,
        height,
    )
    .map(|frame| frame.scene)
}

/// Builds a legible fallback scene that displays `message`.
///
/// Used when the author's entry script cannot produce a scene, so a plugin
/// editor embedded in a DAW can still present something diagnosable instead of
/// a blank or missing surface. The panel uses the default container/text
/// styling (a dark fill with light text), so it needs no colors of its own.
///
/// # Errors
///
/// Returns an [`EditorSceneError`] only if this trivial panel itself cannot be
/// laid out — unreachable in practice, but surfaced rather than panicked so the
/// caller can decide how to degrade further.
pub(crate) fn build_error_scene(
    message: &str,
    width: f32,
    height: f32,
) -> Result<RuntimeSceneFrame, EditorSceneError> {
    let root = EntryNode::view(
        "hawk2ui-editor-error",
        vec![EntryNode::text(
            "hawk2ui-editor-error-message",
            format!("Editor failed to build: {message}"),
        )],
    );
    let tree = root.to_view_tree(width, height).map_err(|error| {
        EditorSceneError::new(
            "hawk2ui-truce.editor.error-scene-failed",
            format!("{error:?}"),
        )
    })?;
    RuntimeSceneBridge::new(Viewport::new(width, height))
        .build(&tree)
        .map_err(|error| {
            EditorSceneError::new(
                "hawk2ui-truce.editor.error-scene-failed",
                format!("{error:?}"),
            )
        })
}

#[cfg(test)]
mod tests {
    use hawk2ui_runtime::RuntimeViewId;

    use super::*;

    /// An entry script whose `mount` returns a root view with a text child —
    /// the realistic editor shape (a fill plus text), which exercises the plain
    /// `RuntimeSceneBridge::build` path over a text visual without a text
    /// measurer (text nodes carry fixed sizes, so no measurement is needed).
    const ENTRY_WITH_TEXT: &str = r##"
export function mount(host) {
    return {
        id: "editor-root",
        type: "view",
        props: { backgroundColor: "#2060b4" },
        children: [
            { id: "title", type: "text", text: "Hello from the editor script" }
        ]
    };
}
"##;

    #[test]
    fn builds_a_scene_from_an_entry_script_with_a_text_node() {
        let frame = build_editor_scene(
            ENTRY_WITH_TEXT,
            "src/editor.js",
            &HostSnapshot::default(),
            320.0,
            180.0,
        )
        .expect("entry script builds an editor scene");
        assert!(
            !frame.draw_commands().is_empty(),
            "a built scene must emit draw commands"
        );
        assert!(
            frame
                .geometry_for(&RuntimeViewId::new("editor-root"))
                .is_some(),
            "the root view must have resolved geometry"
        );
        assert!(
            frame.geometry_for(&RuntimeViewId::new("title")).is_some(),
            "the text child must have resolved geometry"
        );
    }

    #[test]
    fn reports_a_missing_mount_function() {
        let error = build_editor_scene(
            "const value = 1;",
            "src/editor.js",
            &HostSnapshot::default(),
            320.0,
            180.0,
        )
        .expect_err("a script without a mount function must fail");
        assert_eq!(error.rule(), "hawk2ui-truce.editor.no-mount");
    }

    #[test]
    fn rejects_a_mount_result_that_is_not_a_node_tree() {
        // `mount` returns a bare number; its JSON serialization is the string
        // "42", which parses to a JSON number rather than a node-tree object,
        // so tree parsing rejects it as an invalid entry tree.
        let error = build_editor_scene(
            "export function mount(host) { return 42; }",
            "src/editor.js",
            &HostSnapshot::default(),
            320.0,
            180.0,
        )
        .expect_err("a non-object mount result must fail");
        assert_eq!(error.rule(), "hawk2ui-truce.editor.invalid-entry-tree");
    }

    #[test]
    fn builds_from_a_compiled_entry_with_a_typescript_source_path() {
        // `compiled_source` from a sealed artifact is always transpiled JS, even
        // when the author wrote TypeScript, so a `.ts` source path must not
        // trigger a second transform — the payload runs as JavaScript and builds.
        let frame = build_editor_scene(
            ENTRY_WITH_TEXT,
            "src/editor.ts",
            &HostSnapshot::default(),
            320.0,
            180.0,
        )
        .expect("a compiled entry with a .ts source path builds as JavaScript");
        assert!(!frame.draw_commands().is_empty());
    }

    #[test]
    fn projects_the_host_snapshot_into_the_entry_script() {
        use hawk2ui_script::{HostParam, HostParamKind, HostParamValue};

        // The script reads the projected param's text and uses it as the node
        // id, so the projection is observable through the scene's public geometry
        // API.
        const SCRIPT: &str = r#"
export function mount(host) {
    return { id: host.param("title").text, type: "view" };
}
"#;
        // Hand-built snapshot (this crate has no parameter model — the
        // model→snapshot mapping lives in hawk2ui-build).
        let snapshot = HostSnapshot {
            params: vec![HostParam {
                key: "title".into(),
                id: 0,
                kind: HostParamKind::Bool,
                value: HostParamValue::Bool(true),
                normalized: 1.0,
                text: "projected-title".into(),
                variants: Vec::new(),
            }],
            meters: Vec::new(),
        };
        let frame = build_editor_scene(SCRIPT, "src/editor.js", &snapshot, 320.0, 180.0)
            .expect("the entry script builds a scene from the projected snapshot");
        assert!(
            frame
                .geometry_for(&RuntimeViewId::new("projected-title"))
                .is_some(),
            "the projected param text must reach the scene as the node id"
        );
    }

    #[test]
    fn error_display_carries_rule_and_message_and_is_a_std_error() {
        let error = build_editor_scene(
            "const value = 1;",
            "src/editor.js",
            &HostSnapshot::default(),
            320.0,
            180.0,
        )
        .expect_err("a missing mount function fails");
        let rendered = error.to_string();
        assert!(rendered.contains(error.rule()), "{rendered}");
        assert!(rendered.contains(error.message()), "{rendered}");
        let _: &dyn std::error::Error = &error;
    }

    #[test]
    fn builds_a_legible_error_scene() {
        let frame = build_error_scene("the entry script threw", 320.0, 180.0)
            .expect("the fallback error scene builds");
        assert!(
            !frame.draw_commands().is_empty(),
            "an error scene must emit draw commands so the failure is visible"
        );
        assert!(
            frame
                .geometry_for(&RuntimeViewId::new("hawk2ui-editor-error-message"))
                .is_some(),
            "the error message node must have resolved geometry"
        );
    }
}

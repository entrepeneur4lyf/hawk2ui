//! The live editor render cycle: project the bridge, run the entry, replay its
//! edits, persist its UI state, produce the next scene.
//!
//! [`EditorRenderState::render`] is the per-frame step the editor's render loop
//! drives. It is **pure** with respect to windowing — it takes the bridge and
//! carries its own state, returning the new scene — so it is tested directly
//! against a fake bridge in the fast gate, independent of how it is driven. The
//! editor's [`crate::editor::Hawk2uiTruceEditor`] installs it as the Baseview GPU
//! handler's per-frame scene producer on `open` (task 0009.4b).

use std::collections::HashSet;

use hawk2ui_runtime::RuntimeSceneFrame;
use hawk2ui_script::{
    EditRouting, HostMeter, HostParam, HostParamKind, HostParamValue, HostSnapshot,
};
use truce_core::editor::EditorBridge;

use crate::replay::{EditReplayDiagnostic, replay_edits};
use crate::scene::{EditorSceneError, build_editor_frame};

/// Refreshes a snapshot from the live bridge: each entry keeps its **static**
/// fields (key / id / kind / enum variants) from `template` and takes its
/// **dynamic** fields from the bridge — a parameter's plain `value`,
/// `normalized`, and display `text` from `get_param_plain` / `get_param` /
/// `format_param`, a meter's level from `get_meter` (Decision 0003 D2, all
/// non-advancing "host→GUI sync" reads).
///
/// `template` is the construction-time default snapshot from `hawk2ui-build`'s
/// `host_snapshot_from_model`; reusing it as the static shape avoids re-deriving
/// kinds and variant names the bridge does not expose.
#[must_use]
pub(crate) fn refresh_snapshot_from_bridge(
    template: &HostSnapshot,
    bridge: &dyn EditorBridge,
) -> HostSnapshot {
    let params = template
        .params
        .iter()
        .map(|param| HostParam {
            key: param.key.clone(),
            id: param.id,
            kind: param.kind,
            value: host_param_value(param.kind, bridge.get_param_plain(param.id)),
            normalized: bridge.get_param(param.id),
            text: bridge.format_param(param.id),
            variants: param.variants.clone(),
        })
        .collect();
    let meters = template
        .meters
        .iter()
        .map(|meter| HostMeter {
            key: meter.key.clone(),
            id: meter.id,
            value: bridge.get_meter(meter.id),
        })
        .collect();
    HostSnapshot { params, meters }
}

/// Types a plain (denormalized) bridge value by kind, so editor JS sees a `bool`
/// as a boolean and an `int`/`enum` as an integer — never a flattened float
/// (Decision 0003 Lock 3).
fn host_param_value(kind: HostParamKind, plain: f64) -> HostParamValue {
    match kind {
        HostParamKind::Float => HostParamValue::Float(plain),
        HostParamKind::Int => HostParamValue::Int(plain_to_int(plain)),
        // Non-zero plain is `true`; `> EPSILON` rather than `!= 0.0` keeps clippy's
        // `float_cmp` quiet and treats a denormal as off.
        HostParamKind::Bool => HostParamValue::Bool(plain.abs() > f64::EPSILON),
        HostParamKind::Enum => HostParamValue::Enum(plain_to_index(plain)),
    }
}

#[allow(clippy::cast_possible_truncation)]
fn plain_to_int(plain: f64) -> i64 {
    // Rust float→int casts saturate (NaN → 0, out-of-range → the bound); rounding
    // first takes the nearest integer for a plain that carries fractional noise.
    plain.round() as i64
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn plain_to_index(plain: f64) -> u32 {
    // Enum indices are small and non-negative; floor a negative (out-of-range)
    // plain to 0 before the saturating cast.
    plain.round().max(0.0) as u32
}

/// The result of one [`EditorRenderState::render`]: the new scene plus any
/// non-fatal replay diagnostics gathered while applying the entry's edits.
pub(crate) struct RenderOutcome {
    /// The scene to present this frame.
    pub(crate) scene: RuntimeSceneFrame,
    /// Non-fatal anomalies from replaying the entry's edits (unknown key, double
    /// begin, …); empty on a clean cycle. The live producer drops these for now
    /// (a future enhancement could surface them to the host); they are asserted
    /// in this module's tests.
    #[allow(dead_code)]
    pub(crate) diagnostics: Vec<EditReplayDiagnostic>,
}

/// The editor's per-cycle render state — everything the live render loop carries
/// across frames: the compiled entry source, the static snapshot template and
/// write routing (built once from the parameter model), the persisted UI blob,
/// and the open-gesture set. The editor holds one of these and calls
/// [`Self::render`] each cadence tick; bundling the state here (rather than
/// threading a dozen arguments) is also what the editor composes when the live
/// loop is wired in 0009.4b.
pub(crate) struct EditorRenderState {
    compiled_source: String,
    source_path: String,
    template: HostSnapshot,
    routing: EditRouting,
    ui_json: String,
    open_gestures: HashSet<u32>,
    width: f32,
    height: f32,
}

impl EditorRenderState {
    /// Creates the render state for an editor of `width` x `height` points. The
    /// UI blob starts empty (`"null"`) and the gesture set starts closed.
    pub(crate) fn new(
        compiled_source: String,
        source_path: String,
        template: HostSnapshot,
        routing: EditRouting,
        width: f32,
        height: f32,
    ) -> Self {
        Self {
            compiled_source,
            source_path,
            template,
            routing,
            ui_json: String::from("null"),
            open_gestures: HashSet::new(),
            width,
            height,
        }
    }

    /// One live render cycle, the per-frame step of the editor render loop:
    ///
    /// 1. refresh the snapshot from the live `bridge` (current parameter / meter
    ///    values),
    /// 2. run the entry with that snapshot and the persisted UI blob,
    /// 3. replay the entry's edits onto the `bridge` (begin/set/end gestures,
    ///    tracked across cycles in `self.open_gestures`),
    /// 4. persist the entry's outgoing UI blob for the next cycle,
    /// 5. return the new scene and any replay diagnostics.
    ///
    /// Bridge calls run on the caller's (UI) thread — the channel truce's format
    /// wrappers expect.
    ///
    /// # Errors
    ///
    /// Returns the [`EditorSceneError`] from [`build_editor_frame`] when the entry
    /// fails to run or build a scene. The caller is expected to **degrade** (keep
    /// the last good scene, or present the error scene) rather than propagate the
    /// error into the host's UI thread.
    pub(crate) fn render(
        &mut self,
        bridge: &dyn EditorBridge,
    ) -> Result<RenderOutcome, EditorSceneError> {
        let snapshot = refresh_snapshot_from_bridge(&self.template, bridge);
        let frame = build_editor_frame(
            &self.compiled_source,
            &self.source_path,
            &snapshot,
            &self.ui_json,
            self.width,
            self.height,
        )?;
        let diagnostics =
            replay_edits(bridge, &frame.edits, &self.routing, &mut self.open_gestures);
        self.ui_json = frame.ui_json;
        Ok(RenderOutcome {
            scene: frame.scene,
            diagnostics,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use hawk2ui_runtime::RuntimeViewId;
    use hawk2ui_script::{HostParamValue, ParamRoute};
    use truce_core::TransportInfo;

    use super::{
        EditRouting, EditorBridge, EditorRenderState, HostMeter, HostParam, HostParamKind,
        HostSnapshot, host_param_value, refresh_snapshot_from_bridge,
    };

    /// A bridge that returns canned reads (so a refresh sees "live" values) and
    /// records its writes (so a replay is observable). Models the host→GUI sync
    /// reads truce exposes plus the gesture trio.
    #[derive(Default)]
    struct FakeBridge {
        normalized: HashMap<u32, f64>,
        plain: HashMap<u32, f64>,
        text: HashMap<u32, String>,
        meter: HashMap<u32, f32>,
        writes: Mutex<Vec<(&'static str, u32, f64)>>,
    }

    impl FakeBridge {
        fn writes(&self) -> Vec<(&'static str, u32, f64)> {
            self.writes.lock().expect("writes lock").clone()
        }
    }

    impl EditorBridge for FakeBridge {
        fn begin_edit(&self, id: u32) {
            self.writes
                .lock()
                .expect("writes lock")
                .push(("begin", id, 0.0));
        }
        fn set_param(&self, id: u32, normalized: f64) {
            self.writes
                .lock()
                .expect("writes lock")
                .push(("set", id, normalized));
        }
        fn end_edit(&self, id: u32) {
            self.writes
                .lock()
                .expect("writes lock")
                .push(("end", id, 0.0));
        }
        fn request_resize(&self, _w: u32, _h: u32) -> bool {
            false
        }
        fn get_param(&self, id: u32) -> f64 {
            self.normalized.get(&id).copied().unwrap_or(0.0)
        }
        fn get_param_plain(&self, id: u32) -> f64 {
            self.plain.get(&id).copied().unwrap_or(0.0)
        }
        fn format_param(&self, id: u32) -> String {
            self.text.get(&id).cloned().unwrap_or_default()
        }
        fn get_meter(&self, id: u32) -> f32 {
            self.meter.get(&id).copied().unwrap_or(0.0)
        }
        fn get_state(&self) -> Vec<u8> {
            Vec::new()
        }
        fn set_state(&self, _data: Vec<u8>) {}
        fn transport(&self) -> Option<TransportInfo> {
            None
        }
    }

    fn float_param(key: &str, id: u32) -> HostParam {
        HostParam {
            key: key.into(),
            id,
            kind: HostParamKind::Float,
            value: HostParamValue::Float(0.0),
            normalized: 0.0,
            text: String::new(),
            variants: Vec::new(),
        }
    }

    #[test]
    fn refresh_overwrites_dynamic_fields_and_keeps_static_ones() {
        let template = HostSnapshot {
            params: vec![HostParam {
                key: "mode".into(),
                id: 4,
                kind: HostParamKind::Enum,
                value: HostParamValue::Enum(0),
                normalized: 0.0,
                text: "Lowpass".into(),
                variants: vec!["Lowpass".into(), "Bandpass".into(), "Highpass".into()],
            }],
            meters: vec![HostMeter {
                key: "out".into(),
                id: 1 << 24,
                value: 0.0,
            }],
        };
        let mut bridge = FakeBridge::default();
        bridge.plain.insert(4, 2.0);
        bridge.normalized.insert(4, 1.0);
        bridge.text.insert(4, "Highpass".into());
        bridge.meter.insert(1 << 24, 0.7);

        let refreshed = refresh_snapshot_from_bridge(&template, &bridge);
        let mode = &refreshed.params[0];
        // Static fields preserved from the template...
        assert_eq!(mode.key, "mode");
        assert_eq!(mode.id, 4);
        assert_eq!(mode.kind, HostParamKind::Enum);
        assert_eq!(mode.variants, vec!["Lowpass", "Bandpass", "Highpass"]);
        // ...dynamic fields refreshed from the bridge, typed by kind.
        assert_eq!(mode.value, HostParamValue::Enum(2));
        assert!((mode.normalized - 1.0).abs() < 1e-9);
        assert_eq!(mode.text, "Highpass");
        assert!((f64::from(refreshed.meters[0].value) - 0.7).abs() < 1e-6);
    }

    #[test]
    fn host_param_value_types_each_kind_from_plain() {
        assert_eq!(
            host_param_value(HostParamKind::Float, 1200.0),
            HostParamValue::Float(1200.0)
        );
        assert_eq!(
            host_param_value(HostParamKind::Int, 4.0),
            HostParamValue::Int(4)
        );
        assert_eq!(
            host_param_value(HostParamKind::Bool, 1.0),
            HostParamValue::Bool(true)
        );
        assert_eq!(
            host_param_value(HostParamKind::Bool, 0.0),
            HostParamValue::Bool(false)
        );
        assert_eq!(
            host_param_value(HostParamKind::Enum, 2.0),
            HostParamValue::Enum(2)
        );
    }

    // The root view id encodes the threaded frame counter (carried in the ui
    // blob, not held in JS) and a child view id encodes the live param value, so
    // both the refresh and the ui threading are observable through the scene's
    // public geometry. The entry also drives a gesture each cycle.
    const CYCLE_SCRIPT: &str = r#"
export function mount(host) {
    const frames = (host.ui ? host.ui.frames : 0) + 1;
    host.setUi({ frames: frames });
    host.beginEdit("cutoff");
    host.setParam("cutoff", 0.6);
    host.endEdit("cutoff");
    return {
        id: "frame" + frames,
        type: "view",
        children: [{ id: "v" + host.param("cutoff").value, type: "view" }]
    };
}
"#;

    #[test]
    fn render_reflects_live_values_replays_edits_and_threads_ui() {
        let template = HostSnapshot {
            params: vec![float_param("cutoff", 3)],
            meters: Vec::new(),
        };
        let routing = EditRouting::new(vec![ParamRoute {
            key: "cutoff".into(),
            id: 3,
            kind: HostParamKind::Float,
            min: 0.0,
            max: 2000.0,
            variant_count: 0,
        }]);
        let mut bridge = FakeBridge::default();
        bridge.plain.insert(3, 1200.0);

        let mut state = EditorRenderState::new(
            CYCLE_SCRIPT.into(),
            "src/editor.js".into(),
            template,
            routing,
            320.0,
            180.0,
        );

        let first = state
            .render(&bridge)
            .expect("the first cycle builds a frame");
        // The refreshed live value (1200, from get_param_plain) reached the scene...
        assert!(
            first
                .scene
                .geometry_for(&RuntimeViewId::new("v1200"))
                .is_some(),
            "the refreshed bridge value must reach the scene"
        );
        // ...and the threaded ui frame counter is at 1.
        assert!(
            first
                .scene
                .geometry_for(&RuntimeViewId::new("frame1"))
                .is_some()
        );
        assert!(first.diagnostics.is_empty());
        // The entry's edits replayed onto the bridge, keyed through the routing.
        assert_eq!(
            bridge.writes(),
            vec![("begin", 3, 0.0), ("set", 3, 0.6), ("end", 3, 0.0)]
        );

        // A second cycle threads the persisted ui forward: the frame counter,
        // carried in the ui blob (not held in JS), advances 1 → 2.
        let second = state
            .render(&bridge)
            .expect("the second cycle builds a frame");
        assert!(
            second
                .scene
                .geometry_for(&RuntimeViewId::new("frame2"))
                .is_some(),
            "the persisted ui must thread the frame counter across cycles"
        );
    }
}

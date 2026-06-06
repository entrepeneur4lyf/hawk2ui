//! Replaying an editor entry script's parameter edits onto the truce bridge.
//!
//! An editor entry returns an ordered [`HostEdit`] list (see `hawk2ui-script`'s
//! `entry_mount_bootstrap_with_host`); [`replay_edits`] maps each edit's key to a
//! truce `ParamId` through an [`EditRouting`], normalizes a plain value via the
//! parameter's range, and replays the gesture onto the bridge's
//! `begin_edit`/`set_param`/`end_edit` — on the host/UI thread, the channel
//! truce's format wrappers expect. Because the edits ride the entry's return
//! JSON, this adds no host-call capability.
//!
//! The replay is **pure** (no windowing) and tracks open gestures in a caller-held
//! set, so it is exercised directly against a recording bridge in the fast gate.
//! The live per-frame invocation re-projects, re-runs the entry on input, and
//! replays its edits through this path.

use std::collections::{HashMap, HashSet};

use hawk2ui_script::{EditRouting, HostEdit};
use truce_core::editor::EditorBridge;

/// A non-fatal anomaly encountered while replaying an edit list. The replay
/// **skips and records** rather than panicking — a plugin editor embedded in a
/// DAW must never crash the host over a malformed edit, and an author bug (a
/// double-begin, an unmatched end, a typo'd key) should surface as a diagnostic,
/// not a dead gesture or a torn automation lane.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditReplayDiagnostic {
    /// Stable diagnostic rule id.
    pub rule: String,
    /// The parameter key the edit named.
    pub key: String,
}

impl EditReplayDiagnostic {
    fn new(rule: &str, key: &str) -> Self {
        Self {
            rule: rule.to_string(),
            key: key.to_string(),
        }
    }
}

/// Replays `edits` onto `bridge`, resolving each key through `routing` and
/// tracking open automation gestures in `open_gestures` so a gesture begun on one
/// invocation and ended on a later one replays as one bracket (the host threads
/// the set across frames). `last_pushed` carries the last value replayed per id
/// so a redundant per-frame set is suppressed.
///
/// Gesture rules, all skip-and-record (never panic):
/// - A `begin` for a key with no open gesture opens one; a second `begin` while
///   one is open is a double-begin — skipped, recorded.
/// - A bare `set`/`setPlain` with no surrounding `begin`/`end` is **valid** (it is
///   not auto-bracketed — truce's begin/end only mark the host's touched lane).
/// - A `set`/`setPlain` re-pushing the value already pushed for the id is
///   **suppressed**: the bridge is not called. A changed value, or any value
///   after a gesture boundary (which clears the memory), passes.
/// - `setPlain` is normalized to `0.0..=1.0` via the route before `set_param`.
/// - `automate` is one-shot `begin`+`set`+`end`, unless a gesture is already open
///   for the key (then it sets within the open gesture and records the overlap).
/// - An `end` with no matching open gesture is unmatched — skipped, recorded.
/// - An unknown key (including any meter key — meters are read-only) is skipped,
///   recorded.
///
/// Returns the diagnostics gathered; an empty vec means a clean replay.
pub fn replay_edits<S: std::hash::BuildHasher>(
    bridge: &dyn EditorBridge,
    edits: &[HostEdit],
    routing: &EditRouting,
    open_gestures: &mut HashSet<u32, S>,
    last_pushed: &mut HashMap<u32, f64, S>,
) -> Vec<EditReplayDiagnostic> {
    let mut diagnostics = Vec::new();
    for edit in edits {
        let key = edit_key(edit);
        let Some(route) = routing.route(key) else {
            diagnostics.push(EditReplayDiagnostic::new(
                "hawk2ui-truce.edit.unknown-key",
                key,
            ));
            continue;
        };
        let id = route.id;
        match edit {
            HostEdit::Begin { .. } => {
                if open_gestures.insert(id) {
                    bridge.begin_edit(id);
                    // A gesture boundary clears the suppression memory so the
                    // gesture's first set always re-asserts its value (D8).
                    last_pushed.remove(&id);
                } else {
                    diagnostics.push(EditReplayDiagnostic::new(
                        "hawk2ui-truce.edit.double-begin",
                        key,
                    ));
                }
            }
            HostEdit::Set { normalized, .. } => {
                push_set(bridge, id, normalized.clamp(0.0, 1.0), last_pushed);
            }
            HostEdit::SetPlain { plain, .. } => {
                push_set(bridge, id, route.normalize_plain(*plain), last_pushed);
            }
            HostEdit::Automate { normalized, .. } => {
                let normalized = normalized.clamp(0.0, 1.0);
                if open_gestures.contains(&id) {
                    // A one-shot landing inside an already-open gesture: set within
                    // it rather than nesting a second begin/end on the bridge.
                    push_set(bridge, id, normalized, last_pushed);
                    diagnostics.push(EditReplayDiagnostic::new(
                        "hawk2ui-truce.edit.automate-during-gesture",
                        key,
                    ));
                } else {
                    bridge.begin_edit(id);
                    last_pushed.remove(&id);
                    push_set(bridge, id, normalized, last_pushed);
                    bridge.end_edit(id);
                    last_pushed.remove(&id);
                }
            }
            HostEdit::End { .. } => {
                if open_gestures.remove(&id) {
                    bridge.end_edit(id);
                    last_pushed.remove(&id);
                } else {
                    diagnostics.push(EditReplayDiagnostic::new(
                        "hawk2ui-truce.edit.unmatched-end",
                        key,
                    ));
                }
            }
        }
    }
    diagnostics
}

/// The parameter key an edit addresses (every variant carries one).
fn edit_key(edit: &HostEdit) -> &str {
    match edit {
        HostEdit::Begin { key }
        | HostEdit::Set { key, .. }
        | HostEdit::SetPlain { key, .. }
        | HostEdit::Automate { key, .. }
        | HostEdit::End { key } => key,
    }
}

/// Replays a `set_param`, suppressing a redundant re-push of the value already
/// pushed for `id`. An entry that emits an unchanging `setParam` every frame
/// would otherwise storm the host's automation lane at vsync; a changed value —
/// or any value after a gesture boundary, which clears the memory — always
/// reaches the bridge.
fn push_set<S: std::hash::BuildHasher>(
    bridge: &dyn EditorBridge,
    id: u32,
    normalized: f64,
    last_pushed: &mut HashMap<u32, f64, S>,
) {
    if last_pushed
        .get(&id)
        .is_some_and(|last| (*last - normalized).abs() <= f64::EPSILON)
    {
        return;
    }
    bridge.set_param(id, normalized);
    last_pushed.insert(id, normalized);
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use hawk2ui_script::{HostParamKind, ParamRoute};
    use truce_core::TransportInfo;

    use super::{
        EditReplayDiagnostic, EditRouting, EditorBridge, HashMap, HashSet, HostEdit, replay_edits,
    };

    /// One bridge call, recorded so a test can assert replay order and values.
    #[derive(Clone, Debug, PartialEq)]
    enum Call {
        Begin(u32),
        Set(u32, f64),
        End(u32),
    }

    /// An [`EditorBridge`] that records its gesture/parameter calls. truce's own
    /// `for_test_params` bridge no-ops `begin_edit`/`set_param`/`end_edit`, so it
    /// cannot witness a replay; this one can.
    struct RecordingBridge {
        calls: Mutex<Vec<Call>>,
    }

    impl RecordingBridge {
        fn new() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
            }
        }

        fn recorded(&self) -> Vec<Call> {
            self.calls.lock().expect("calls lock").clone()
        }

        fn push(&self, call: Call) {
            self.calls.lock().expect("calls lock").push(call);
        }
    }

    impl EditorBridge for RecordingBridge {
        fn begin_edit(&self, id: u32) {
            self.push(Call::Begin(id));
        }
        fn set_param(&self, id: u32, normalized: f64) {
            self.push(Call::Set(id, normalized));
        }
        fn end_edit(&self, id: u32) {
            self.push(Call::End(id));
        }
        fn request_resize(&self, _w: u32, _h: u32) -> bool {
            false
        }
        fn get_param(&self, _id: u32) -> f64 {
            0.0
        }
        fn get_param_plain(&self, _id: u32) -> f64 {
            0.0
        }
        fn format_param(&self, _id: u32) -> String {
            String::new()
        }
        fn get_meter(&self, _id: u32) -> f32 {
            0.0
        }
        fn get_state(&self) -> Vec<u8> {
            Vec::new()
        }
        fn set_state(&self, _data: Vec<u8>) {}
        fn transport(&self) -> Option<TransportInfo> {
            None
        }
    }

    /// `cutoff` (float, id 0, range 0..100) and `bypass` (bool, id 1).
    fn routing() -> EditRouting {
        EditRouting::new(vec![
            ParamRoute {
                key: "cutoff".into(),
                id: 0,
                kind: HostParamKind::Float,
                min: 0.0,
                max: 100.0,
                variant_count: 0,
            },
            ParamRoute {
                key: "bypass".into(),
                id: 1,
                kind: HostParamKind::Bool,
                min: 0.0,
                max: 0.0,
                variant_count: 0,
            },
        ])
    }

    fn begin(key: &str) -> HostEdit {
        HostEdit::Begin { key: key.into() }
    }
    fn set(key: &str, normalized: f64) -> HostEdit {
        HostEdit::Set {
            key: key.into(),
            normalized,
        }
    }
    fn end(key: &str) -> HostEdit {
        HostEdit::End { key: key.into() }
    }

    #[test]
    fn replays_a_gesture_bracket_in_order() {
        let bridge = RecordingBridge::new();
        let mut open = HashSet::new();
        let mut last = HashMap::new();
        let diagnostics = replay_edits(
            &bridge,
            &[begin("cutoff"), set("cutoff", 0.55), end("cutoff")],
            &routing(),
            &mut open,
            &mut last,
        );
        assert!(diagnostics.is_empty());
        assert_eq!(
            bridge.recorded(),
            vec![Call::Begin(0), Call::Set(0, 0.55), Call::End(0)]
        );
        assert!(open.is_empty(), "the gesture closed");
    }

    #[test]
    fn a_bare_set_replays_without_a_bracket() {
        let bridge = RecordingBridge::new();
        let mut open = HashSet::new();
        let mut last = HashMap::new();
        let diagnostics = replay_edits(
            &bridge,
            &[set("cutoff", 0.3)],
            &routing(),
            &mut open,
            &mut last,
        );
        assert!(
            diagnostics.is_empty(),
            "a bare set is valid, not auto-bracketed"
        );
        assert_eq!(bridge.recorded(), vec![Call::Set(0, 0.3)]);
    }

    #[test]
    fn set_param_clamps_out_of_range_normalized() {
        let bridge = RecordingBridge::new();
        let mut open = HashSet::new();
        let mut last = HashMap::new();
        replay_edits(
            &bridge,
            &[set("cutoff", 1.5)],
            &routing(),
            &mut open,
            &mut last,
        );
        assert_eq!(bridge.recorded(), vec![Call::Set(0, 1.0)]);
    }

    #[test]
    fn set_plain_normalizes_via_the_route() {
        let bridge = RecordingBridge::new();
        let mut open = HashSet::new();
        let mut last = HashMap::new();
        // cutoff range 0..100; plain 50 → normalized 0.5.
        replay_edits(
            &bridge,
            &[HostEdit::SetPlain {
                key: "cutoff".into(),
                plain: 50.0,
            }],
            &routing(),
            &mut open,
            &mut last,
        );
        assert_eq!(bridge.recorded(), vec![Call::Set(0, 0.5)]);
    }

    #[test]
    fn automate_replays_a_full_one_shot_bracket() {
        let bridge = RecordingBridge::new();
        let mut open = HashSet::new();
        let mut last = HashMap::new();
        replay_edits(
            &bridge,
            &[HostEdit::Automate {
                key: "bypass".into(),
                normalized: 1.0,
            }],
            &routing(),
            &mut open,
            &mut last,
        );
        assert_eq!(
            bridge.recorded(),
            vec![Call::Begin(1), Call::Set(1, 1.0), Call::End(1)]
        );
        assert!(open.is_empty(), "a one-shot leaves no open gesture");
    }

    #[test]
    fn a_cross_frame_gesture_brackets_across_two_replays() {
        let bridge = RecordingBridge::new();
        let mut open = HashSet::new();
        let mut last = HashMap::new();
        let routing = routing();
        // Frame 1: begin + set, gesture stays open.
        replay_edits(
            &bridge,
            &[begin("cutoff"), set("cutoff", 0.2)],
            &routing,
            &mut open,
            &mut last,
        );
        assert!(open.contains(&0), "the gesture spans frames");
        // Frame 2: another set, then end — the host replays the open gesture's close.
        let diagnostics = replay_edits(
            &bridge,
            &[set("cutoff", 0.8), end("cutoff")],
            &routing,
            &mut open,
            &mut last,
        );
        assert!(diagnostics.is_empty());
        assert_eq!(
            bridge.recorded(),
            vec![
                Call::Begin(0),
                Call::Set(0, 0.2),
                Call::Set(0, 0.8),
                Call::End(0),
            ]
        );
        assert!(open.is_empty());
    }

    #[test]
    fn a_double_begin_is_skipped_and_recorded() {
        let bridge = RecordingBridge::new();
        let mut open = HashSet::new();
        let mut last = HashMap::new();
        let diagnostics = replay_edits(
            &bridge,
            &[begin("cutoff"), begin("cutoff")],
            &routing(),
            &mut open,
            &mut last,
        );
        assert_eq!(
            bridge.recorded(),
            vec![Call::Begin(0)],
            "the second begin does not reach the bridge"
        );
        assert_eq!(
            diagnostics,
            vec![EditReplayDiagnostic {
                rule: "hawk2ui-truce.edit.double-begin".into(),
                key: "cutoff".into(),
            }]
        );
    }

    #[test]
    fn an_unmatched_end_is_skipped_and_recorded() {
        let bridge = RecordingBridge::new();
        let mut open = HashSet::new();
        let mut last = HashMap::new();
        let diagnostics = replay_edits(&bridge, &[end("cutoff")], &routing(), &mut open, &mut last);
        assert!(bridge.recorded().is_empty(), "no end reaches the bridge");
        assert_eq!(
            diagnostics,
            vec![EditReplayDiagnostic {
                rule: "hawk2ui-truce.edit.unmatched-end".into(),
                key: "cutoff".into(),
            }]
        );
    }

    #[test]
    fn an_unknown_or_meter_key_is_skipped_and_recorded() {
        let bridge = RecordingBridge::new();
        let mut open = HashSet::new();
        let mut last = HashMap::new();
        // "out" is a meter — meters are not in the routing, so a write to one is
        // structurally impossible: it resolves to no route and is skipped.
        let diagnostics = replay_edits(
            &bridge,
            &[set("out", 0.5)],
            &routing(),
            &mut open,
            &mut last,
        );
        assert!(bridge.recorded().is_empty());
        assert_eq!(
            diagnostics,
            vec![EditReplayDiagnostic {
                rule: "hawk2ui-truce.edit.unknown-key".into(),
                key: "out".into(),
            }]
        );
    }

    #[test]
    fn suppresses_a_repeat_set_but_passes_changes_and_gesture_boundaries() {
        // A continuous loop re-pushing the same value every frame must not storm
        // the bridge, yet real moves and gesture re-asserts must always land.
        let bridge = RecordingBridge::new();
        let mut open = HashSet::new();
        let mut last = HashMap::new();
        let routing = routing();
        // Three identical bare sets at vsync: only the first reaches the bridge.
        replay_edits(
            &bridge,
            &[set("cutoff", 0.5)],
            &routing,
            &mut open,
            &mut last,
        );
        replay_edits(
            &bridge,
            &[set("cutoff", 0.5)],
            &routing,
            &mut open,
            &mut last,
        );
        replay_edits(
            &bridge,
            &[set("cutoff", 0.5)],
            &routing,
            &mut open,
            &mut last,
        );
        assert_eq!(
            bridge.recorded(),
            vec![Call::Set(0, 0.5)],
            "identical repeat sets are suppressed"
        );
        // A changed value passes; then a gesture re-asserting that same value
        // passes too, because the gesture boundary clears the suppression memory.
        replay_edits(
            &bridge,
            &[set("cutoff", 0.6)],
            &routing,
            &mut open,
            &mut last,
        );
        replay_edits(
            &bridge,
            &[begin("cutoff"), set("cutoff", 0.6), end("cutoff")],
            &routing,
            &mut open,
            &mut last,
        );
        assert_eq!(
            bridge.recorded(),
            vec![
                Call::Set(0, 0.5),
                Call::Set(0, 0.6),
                Call::Begin(0),
                Call::Set(0, 0.6),
                Call::End(0),
            ],
            "a changed value and a post-boundary re-assert both reach the bridge"
        );
    }
}

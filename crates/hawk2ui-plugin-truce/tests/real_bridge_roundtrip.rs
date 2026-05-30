#![cfg(target_os = "linux")]
//! A real truce `EditorBridge` round-trip: prove an editor edit lands a value in
//! a real truce parameter store (Phase 5, feasibility #2 — the load-bearing half).
//!
//! The replay unit tests (`src/replay.rs`) and the input-seam glue test
//! (`src/editor.rs`) drive [`replay_edits`] against a **hand-written**
//! `EditorBridge` that only records calls — so they prove our gesture logic, but
//! nothing about truce's actual bridge contract or whether a write takes effect.
//! truce's own `for_test_params` bridge is the opposite problem: it is real, but
//! no-ops every write, so it can't witness a value landing either.
//!
//! This closes the gap with the bridge a host actually delivers: truce's
//! [`ClosureBridge`] (the adapter its format wrappers construct) wrapped in a
//! real [`PluginContext`], whose `set_param` **applies** to a real
//! `#[derive(Params)]` store via `Params::set_normalized` *and* records the
//! gesture. Driving our `replay_edits` against the exact `context.bridge()`
//! accessor `Editor::open` captures then proves, with no window, that:
//!   1. a begin/set/end gesture reaches truce's real bridge in order, and
//!   2. the set value lands in the truce parameter store, read back through
//!      truce's own `get_normalized`.
//!
//! Scope: this proves the bridge/replay half headlessly. The per-frame producer
//! that drains input and calls this is covered by `editor.rs`'s glue test and
//! the gated Xvfb smoke; a real DAW host's bridge (truce-clap/vst3/standalone)
//! and on-window rendering remain hardware validation.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use hawk2ui_plugin_truce::{EditRouting, HostEdit, HostParamKind, ParamRoute, replay_edits};
use truce::prelude::{FloatParam, Params};
use truce_core::editor::{ClosureBridge, PluginContext};

/// One bridge call, recorded so the test can assert replay order.
#[derive(Clone, Debug, PartialEq)]
enum Call {
    Begin(u32),
    Set(u32, f64),
    End(u32),
}

/// A single continuous control at id 0, range 0..100, default plain 50
/// (normalized 0.5) — so a write to normalized 0.75 is unambiguously distinct
/// from the default, proving the write changed the store rather than the read
/// returning the default.
#[derive(Params)]
struct TestParams {
    #[param(
        id = 0,
        name = "Cutoff",
        range = "linear(0, 100)",
        unit = "Hz",
        default = 50,
        smooth = "exp(5)"
    )]
    cutoff: FloatParam,
}

/// Builds truce's real [`ClosureBridge`] over `params`: writes apply to the
/// store through `Params::set_normalized` and are recorded into `log`; reads
/// delegate to the store. This mirrors `truce_core::editor::for_test_params`'s
/// field set, but its write closures do real work instead of no-ops.
fn applying_recording_context(
    params: &Arc<dyn Params>,
    log: &Arc<Mutex<Vec<Call>>>,
) -> PluginContext<dyn Params> {
    let p_set = Arc::clone(params);
    let p_get = Arc::clone(params);
    let p_plain = Arc::clone(params);
    let p_fmt = Arc::clone(params);
    let l_begin = Arc::clone(log);
    let l_set = Arc::clone(log);
    let l_end = Arc::clone(log);
    let bridge = ClosureBridge {
        begin_edit: Box::new(move |id| l_begin.lock().expect("log").push(Call::Begin(id))),
        set_param: Box::new(move |id, value| {
            p_set.set_normalized(id, value);
            l_set.lock().expect("log").push(Call::Set(id, value));
        }),
        end_edit: Box::new(move |id| l_end.lock().expect("log").push(Call::End(id))),
        request_resize: Box::new(|_, _| false),
        get_param: Box::new(move |id| p_get.get_normalized(id).unwrap_or(0.0)),
        get_param_plain: Box::new(move |id| p_plain.get_plain(id).unwrap_or(0.0)),
        format_param: Box::new(move |id| {
            let plain = p_fmt.get_plain(id).unwrap_or(0.0);
            p_fmt.format_value(id, plain).unwrap_or_default()
        }),
        get_meter: Box::new(|_| 0.0),
        get_state: Box::new(Vec::new),
        set_state: Box::new(|_| {}),
        transport: Box::new(|| None),
    };
    PluginContext::from_closures(bridge, Arc::clone(params))
}

#[test]
fn an_edit_replays_onto_truces_real_bridge_and_lands_in_the_param_store() {
    let params: Arc<dyn Params> = Arc::new(TestParams::default());
    assert!(
        (params.get_normalized(0).expect("param 0") - 0.5).abs() <= 1e-9,
        "precondition: cutoff defaults to normalized 0.5"
    );

    let log = Arc::new(Mutex::new(Vec::new()));
    let context = applying_recording_context(&params, &log);

    let routing = EditRouting::new(vec![ParamRoute {
        key: "cutoff".into(),
        id: 0,
        kind: HostParamKind::Float,
        min: 0.0,
        max: 100.0,
        variant_count: 0,
    }]);
    let edits = [
        HostEdit::Begin {
            key: "cutoff".into(),
        },
        HostEdit::Set {
            key: "cutoff".into(),
            normalized: 0.75,
        },
        HostEdit::End {
            key: "cutoff".into(),
        },
    ];

    let mut open = HashSet::new();
    let mut last = HashMap::new();
    // Drive replay against the exact `Arc<dyn EditorBridge>` `Editor::open`
    // captures from the context — truce's real bridge, not a hand-written stub.
    let diagnostics = replay_edits(
        context.bridge().as_ref(),
        &edits,
        &routing,
        &mut open,
        &mut last,
    );

    assert!(diagnostics.is_empty(), "a clean gesture: {diagnostics:?}");
    assert_eq!(
        *log.lock().expect("log"),
        vec![Call::Begin(0), Call::Set(0, 0.75), Call::End(0)],
        "the gesture reached truce's real bridge in order"
    );
    let landed = params.get_normalized(0).expect("param 0");
    assert!(
        (landed - 0.75).abs() <= 1e-9,
        "the set value landed in the truce param store (read back {landed}, expected 0.75)"
    );
}

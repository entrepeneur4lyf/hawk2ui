//! Parameter-codegen spike: prove the emit → compile → bridge-read loop.
//!
//! `hawk2ui_plugin_truce::emit_truce_params_struct` turns a validated
//! [`ParameterModel`] into truce `#[derive(Params)]` source. A *string* match
//! alone can't catch a bad `::truce::` path or a malformed `#[param(...)]`
//! attribute — that only surfaces when the output is compiled against truce.
//! So the golden fixture is both `include!`d (rustc compiles it, catching path
//! / attribute / field-type errors) and `include_str!`d (the emitter must
//! reproduce it byte-for-byte, catching drift). The same fixture is then driven
//! through `for_test_params` to confirm the emitted parameters expose the right
//! `ParamInfo`s and read back through the `EditorBridge` by the `u32` ids the
//! editor side will use.
//!
//! `for_test_params`' bridge has a no-op `set_param`, so this verifies the
//! read/format path (defaults, ranges, units, ids), not host write-back — that
//! is supplied by the format wrappers, not this fixture.

use std::sync::Arc;

use hawk2ui_plugin::{ParameterModel, ParameterRange, ParameterRecord};
use hawk2ui_plugin_truce::emit_truce_params_struct;
use truce::params::{ParamRange, ParamUnit, ParamValueKind, Params};
use truce_core::editor::for_test_params;

/// The compiled golden. Cargo does not treat files under `tests/` subdirectories
/// as test targets, so it is only ever pulled in here via `include!`.
mod golden {
    include!("fixtures/golden_params.rs");
}

/// The model the golden fixture was generated from.
fn spike_model() -> ParameterModel {
    ParameterModel::new([
        ParameterRecord::numeric("osc.mix", "Osc Mix", "", ParameterRange::new(0.0, 1.0, 0.4)),
        ParameterRecord::numeric(
            "filter.cutoff",
            "Cutoff",
            "Hz",
            ParameterRange::new(20.0, 20000.0, 1000.0),
        ),
    ])
}

fn approx(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() < 1e-6
}

#[test]
fn emitter_reproduces_compiled_golden() {
    let model = spike_model();
    assert_eq!(
        emit_truce_params_struct("SpikeParams", &model),
        include_str!("fixtures/golden_params.rs"),
        "emitter output drifted from the compiled golden fixture; regenerate the fixture from the emitter"
    );
}

#[test]
fn generated_params_expose_truce_infos() {
    let params = golden::SpikeParams::default();
    let infos = params.param_infos();
    assert_eq!(infos.len(), 2, "expected the two emitted parameters");

    assert_eq!(infos[0].id, 0);
    assert_eq!(infos[0].name, "Osc Mix");
    assert_eq!(infos[0].unit, ParamUnit::None);
    assert_eq!(infos[0].kind, ParamValueKind::Float);
    assert!(
        matches!(infos[0].range, ParamRange::Linear { min, max } if approx(min, 0.0) && approx(max, 1.0)),
        "osc.mix range was {:?}",
        infos[0].range
    );
    assert!(approx(infos[0].default_plain, 0.4));

    assert_eq!(infos[1].id, 1);
    assert_eq!(infos[1].name, "Cutoff");
    assert_eq!(infos[1].unit, ParamUnit::Hz);
    assert!(
        matches!(infos[1].range, ParamRange::Linear { min, max } if approx(min, 20.0) && approx(max, 20000.0)),
        "filter.cutoff range was {:?}",
        infos[1].range
    );
    assert!(approx(infos[1].default_plain, 1000.0));
}

#[test]
fn generated_params_read_back_through_the_bridge_by_id() {
    let params: Arc<dyn Params> = Arc::new(golden::SpikeParams::default());
    let context = for_test_params(Arc::clone(&params));
    let bridge = context.bridge();

    // osc.mix: linear(0, 1), default 0.4 — plain and normalized coincide.
    assert!(approx(bridge.get_param_plain(0), 0.4));
    assert!(approx(bridge.get_param(0), 0.4));

    // filter.cutoff: linear(20, 20000), default 1000 Hz.
    assert!(approx(bridge.get_param_plain(1), 1000.0));
    assert_eq!(bridge.format_param(1), "1.0 kHz");
}

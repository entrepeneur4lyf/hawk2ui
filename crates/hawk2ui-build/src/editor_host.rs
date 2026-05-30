//! Projecting a [`ParameterModel`] into the editor [`HostSnapshot`] that an
//! entry script reads.
//!
//! A plugin editor's `mount(host)` call receives a frozen snapshot of the
//! plugin's parameters and meters (see `hawk2ui-script`'s
//! `entry_mount_bootstrap_with_host`). This module builds that snapshot from the
//! parameter *model* — the authoring-side source of truth that carries each
//! parameter's kind, default, unit, and enum variants.
//!
//! This crate is the right home for the mapping: it is the one layer that
//! depends on both `hawk2ui-plugin` (which owns [`ParameterModel`]) and
//! `hawk2ui-script` (which owns [`HostSnapshot`]). `hawk2ui-plugin-truce` stays
//! decoupled from the parameter model, knowing only the projected snapshot type.
//!
//! ## Scope (task 0009.1b): declared defaults only
//!
//! Every projected value is sourced from the model's **declared defaults**. The
//! live host→GUI sync — re-projecting from the truce `EditorBridge` on input, on
//! a host parameter change, or per-frame for meter vsync — is task 0009.4.
//! Meters therefore project at their floor (`0.0`) here, since the model carries
//! no persisted meter level.

use hawk2ui_plugin::{METER_ID_BASE, ParameterModel, ParameterRecord, ParameterValue};
use hawk2ui_script::{HostMeter, HostParam, HostParamKind, HostParamValue, HostSnapshot};

/// Projects a [`ParameterModel`] into the [`HostSnapshot`] an editor entry
/// script reads, sourcing every value from the model's declared defaults.
///
/// Infallible by construction: a [`ParameterModel`] that reaches the editor has
/// already passed [`ParameterModel::validate`], so each default is in range and
/// type-compatible and the per-record `normalize`/`display_value` calls cannot
/// fail in practice. A plugin editor embedded in a DAW must never panic, so the
/// unreachable error paths degrade to a sane projected value (`0.0` normalized,
/// an empty display string) rather than unwrapping.
#[must_use]
pub fn host_snapshot_from_model(model: &ParameterModel) -> HostSnapshot {
    // `resolved_param_ids` returns the truce `ParamId` u32 per parameter in
    // declaration order (pinned ids honored, unpinned filled lowest-free) — the
    // same ids the codegen emits, so the projection routes writes to the exact
    // discriminant the DSP uses (Decision 0003 Lock 1).
    let param_ids = model.resolved_param_ids();
    HostSnapshot {
        params: model
            .parameters
            .iter()
            .zip(param_ids)
            .map(|(record, id)| host_param_from_record(record, id))
            .collect(),
        // Meters carry no persisted level; they project at their floor until the
        // live bridge feeds real values (task 0009.4). truce auto-assigns meter
        // ids as `METER_ID_BASE + declaration_index` (Decision 0003 Lock 2), so
        // the projection mirrors that order.
        meters: model
            .meters
            .iter()
            .enumerate()
            .map(|(index, meter)| HostMeter {
                key: meter.id.clone(),
                id: METER_ID_BASE.saturating_add(u32::try_from(index).unwrap_or(u32::MAX)),
                value: 0.0,
            })
            .collect(),
    }
}

/// Projects one parameter record into its [`HostParam`], carrying the kind so
/// the value reaches editor JS as its true scalar (a `bool` as a boolean, an
/// enum as its variant index) rather than flattened to a float. `id` is the
/// parameter's resolved truce `ParamId` (the write-routing wire detail).
fn host_param_from_record(record: &ParameterRecord, id: u32) -> HostParam {
    let (kind, value) = match &record.default_value {
        ParameterValue::Float(value) => (HostParamKind::Float, HostParamValue::Float(*value)),
        ParameterValue::Int(value) => (HostParamKind::Int, HostParamValue::Int(*value)),
        ParameterValue::Bool(value) => (HostParamKind::Bool, HostParamValue::Bool(*value)),
        ParameterValue::Choice(value) => (HostParamKind::Enum, HostParamValue::Enum(*value)),
    };
    HostParam {
        key: record.id.clone(),
        id,
        kind,
        value,
        normalized: default_normalized(record).clamp(0.0, 1.0),
        text: record
            .display_value(&record.default_value)
            .unwrap_or_default(),
        variants: record
            .variants
            .iter()
            .map(|variant| variant.display_name.clone())
            .collect(),
    }
}

/// The default value's normalized `0.0..=1.0` position.
///
/// [`ParameterRecord::normalize`] covers float/int/bool but rejects an indexed
/// choice (it has no numeric range), so the enum case is computed here as
/// `index / (count - 1)` — the exact inverse of [`ParameterRecord::denormalize`]'s
/// step rounding, so the first variant maps to `0.0` and the last to `1.0`.
fn default_normalized(record: &ParameterRecord) -> f64 {
    if let ParameterValue::Choice(index) = &record.default_value {
        return match u32::try_from(record.variants.len()) {
            Ok(count) if count > 1 => f64::from(*index) / f64::from(count - 1),
            // Zero or one variant: the only position is the floor.
            _ => 0.0,
        };
    }
    record.normalize(&record.default_value).unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use hawk2ui_plugin::{
        EnumVariant, METER_ID_BASE, MeterRecord, ParameterModel, ParameterRange, ParameterRecord,
    };
    use hawk2ui_script::{
        HostCallPolicy, HostParamKind, HostParamValue, ScriptBackend, ScriptModule,
        StructuredValue, TimerPolicy, entry_mount_bootstrap_with_host,
    };

    use super::host_snapshot_from_model;

    /// A model exercising all four parameter kinds plus a meter. The enum
    /// default sits on the **last** variant — the case `ParameterRecord::normalize`
    /// cannot handle (no numeric range), which `default_normalized` computes so a
    /// last-variant default reads back as `normalized == 1.0`.
    fn synth_model() -> ParameterModel {
        ParameterModel::new([
            ParameterRecord::numeric(
                "cutoff",
                "Cutoff",
                "kHz",
                ParameterRange::new(20.0, 20000.0, 1200.0),
            ),
            ParameterRecord::integer("voices", "Voices", "", ParameterRange::new(1.0, 8.0, 4.0)),
            ParameterRecord::boolean("bypass", "Bypass", true),
            ParameterRecord::enumerated(
                "mode",
                "Mode",
                2,
                [
                    EnumVariant::new("lp", "Lowpass"),
                    EnumVariant::new("bp", "Bandpass"),
                    EnumVariant::new("hp", "Highpass"),
                ],
            ),
        ])
        .with_meters([MeterRecord::new("out", "Output")])
    }

    #[test]
    fn projects_each_parameter_kind_with_its_typed_default() {
        let snapshot = host_snapshot_from_model(&synth_model());
        assert_eq!(snapshot.params.len(), 4);

        let cutoff = &snapshot.params[0];
        assert_eq!(cutoff.key, "cutoff");
        assert_eq!(cutoff.id, 0, "unpinned ids fill in declaration order");
        assert_eq!(cutoff.kind, HostParamKind::Float);
        assert_eq!(cutoff.value, HostParamValue::Float(1200.0));
        assert_eq!(cutoff.text, "1200 kHz");
        assert!((cutoff.normalized - (1180.0 / 19980.0)).abs() < 1e-9);
        assert!(cutoff.variants.is_empty());

        let voices = &snapshot.params[1];
        assert_eq!(voices.id, 1);
        assert_eq!(voices.kind, HostParamKind::Int);
        assert_eq!(voices.value, HostParamValue::Int(4));

        let bypass = &snapshot.params[2];
        assert_eq!(bypass.id, 2);
        assert_eq!(bypass.kind, HostParamKind::Bool);
        assert_eq!(bypass.value, HostParamValue::Bool(true));
        assert!(
            (bypass.normalized - 1.0).abs() < 1e-9,
            "a default-true bool must normalize to 1.0, not a swallowed 0.0"
        );
    }

    #[test]
    fn projects_an_enum_default_on_the_last_variant_at_normalized_one() {
        let snapshot = host_snapshot_from_model(&synth_model());
        let mode = &snapshot.params[3];
        assert_eq!(mode.id, 3);
        assert_eq!(mode.kind, HostParamKind::Enum);
        assert_eq!(mode.value, HostParamValue::Enum(2));
        assert_eq!(mode.text, "Highpass");
        assert!(
            (mode.normalized - 1.0).abs() < 1e-9,
            "the last enum variant must normalize to 1.0"
        );
        assert_eq!(mode.variants, vec!["Lowpass", "Bandpass", "Highpass"]);
    }

    #[test]
    fn projects_meters_at_their_floor() {
        let snapshot = host_snapshot_from_model(&synth_model());
        assert_eq!(snapshot.meters.len(), 1);
        assert_eq!(snapshot.meters[0].key, "out");
        assert!((f64::from(snapshot.meters[0].value)).abs() < 1e-9);
    }

    /// Meter ids must be `METER_ID_BASE + declaration_index` — the truce
    /// auto-assignment the codegen relies on (Decision 0003 Lock 2). This holds
    /// only because the projection iterates meters in codegen order; assert it on
    /// a two-meter model so a future reorder can't silently de-sync the projected
    /// id from the real `ParamId`.
    #[test]
    fn projects_meter_ids_at_the_meter_base_offset() {
        let model = ParameterModel::new([ParameterRecord::boolean("bypass", "Bypass", false)])
            .with_meters([
                MeterRecord::new("in", "Input"),
                MeterRecord::new("out", "Output"),
            ]);
        let snapshot = host_snapshot_from_model(&model);
        assert_eq!(snapshot.meters[0].key, "in");
        assert_eq!(snapshot.meters[0].id, METER_ID_BASE);
        assert_eq!(snapshot.meters[1].key, "out");
        assert_eq!(snapshot.meters[1].id, METER_ID_BASE + 1);
    }

    /// Round-trips the projection through the real bootstrap the editor uses:
    /// `host_snapshot_from_model` → `entry_mount_bootstrap_with_host` → boa
    /// execution. Proves the build-side mapping produces a snapshot shaped
    /// correctly for the script surface — the two halves (this mapping and the
    /// JS projection) only meet here. Guards the enum-on-last-variant case end to
    /// end: the script reads `mode.normalized` back as `1`.
    #[test]
    fn the_projected_snapshot_reads_back_through_the_entry_host() {
        const SCRIPT: &str = r#"
export function mount(host) {
    const cutoff = host.param("cutoff");
    const mode = host.param("mode");
    return {
        id: "root",
        type: "text",
        text: cutoff.text + "|" + mode.text + ":" + mode.value + ":" + mode.normalized + ":" + mode.variants.length
    };
}
"#;
        let snapshot = host_snapshot_from_model(&synth_model());
        let bootstrap = entry_mount_bootstrap_with_host(SCRIPT, &snapshot, "null")
            .expect("the script declares mount");

        let mut backend =
            ScriptBackend::new(HostCallPolicy::deny_all(), TimerPolicy::deterministic());
        let execution = backend
            .execute_module(ScriptModule::javascript("src/editor.js", &bootstrap))
            .expect("the bootstrapped editor script executes");

        // The output is the `{ tree, edits, ui }` envelope; the projected text
        // lives in `tree`, so substring asserts hold without parsing it apart.
        let StructuredValue::String(envelope_json) = execution.value() else {
            panic!("the entry must return a serialized envelope");
        };
        // cutoff projects its host-formatted text (value plus unit)...
        assert!(envelope_json.contains("1200 kHz"), "{envelope_json}");
        // ...and the enum reads back typed: display text, variant index, and a
        // last-variant `normalized` of exactly 1, with all three variants.
        assert!(envelope_json.contains("Highpass:2:1:3"), "{envelope_json}");
    }
}

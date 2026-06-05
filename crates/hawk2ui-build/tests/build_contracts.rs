use ed25519_dalek::{Signer, SigningKey};
use hawk2ui_api::{Diagnostic, DiagnosticSeverity};
use hawk2ui_build::{
    ARTIFACT_SIGNATURE_ALGORITHM_ED25519_SHA256_V1, ArtifactHash, ArtifactSchemaVersion,
    ArtifactSignature, ArtifactSignaturePolicy, ArtifactSignatureVerificationKey,
    ArtifactSignatureVerifier, ArtifactSigningKey, AssetCompilationError, AssetCompilationPlan,
    AssetDimensions, AssetKind, AssetManifestEntry, AssetSanitizationStatus, AssetSource,
    AssetSourceIndex, BuildDiagnostic, BuildDiagnosticSeverity, BuildPhase, BuildPipeline,
    BuildPipelineError, BuildWorkspace, BuildWorkspaceError, CompiledAssetRecord,
    CompiledScriptRecord, CompiledStyleRecord, HawkManifest, ManifestError, PackageTarget,
    PackageTargetRecord, SealedArtifact, SealedArtifactError, SourceSpan, VerificationReport,
};
use hawk2ui_plugin::{ParameterRange, ParameterValue};
use image::{ColorType, ImageEncoder};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn sign_artifact(
    artifact: SealedArtifact,
    key_id: &str,
) -> (SealedArtifact, ArtifactSignatureVerifier) {
    let signing_key = SigningKey::from_bytes(&[7; 32]);
    let signature = signing_key.sign(&artifact.signature_payload_bytes());
    let verifier =
        ArtifactSignatureVerifier::new([ArtifactSignatureVerificationKey::ed25519_sha256_v1(
            key_id,
            signing_key.verifying_key().to_bytes(),
        )]);
    let signed = artifact.with_signature(ArtifactSignature::verified(
        ARTIFACT_SIGNATURE_ALGORITHM_ED25519_SHA256_V1,
        key_id,
        encode_hex(&signature.to_bytes()),
    ));
    (signed, verifier)
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn tiny_png() -> Vec<u8> {
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(&[255, 255, 255, 255], 1, 1, ColorType::Rgba8.into())
        .expect("test PNG encodes");
    bytes
}

fn simple_svg(fill: &str) -> Vec<u8> {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><path fill="{fill}" d="M0 0H16V16H0Z"/></svg>"#
    )
    .into_bytes()
}

fn fixture_font_bytes() -> Option<Vec<u8>> {
    [
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    ]
    .iter()
    .find_map(|path| fs::read(path).ok())
}

#[test]
fn build_diagnostic_converts_to_shared_diagnostic_with_location_context() {
    let diagnostic = Diagnostic::from(
        BuildDiagnostic::new(
            BuildDiagnosticSeverity::Warning,
            "build.asset.large",
            "asset is large",
        )
        .with_location("src/app.ts", SourceSpan::new(10, 25)),
    );

    assert_eq!(diagnostic.severity, DiagnosticSeverity::Warning);
    assert_eq!(diagnostic.rule.as_str(), "build.asset.large");
    assert_eq!(diagnostic.message, "asset is large");
    assert!(
        diagnostic
            .related
            .iter()
            .any(|context| context.label == "file" && context.value == "src/app.ts")
    );
    assert!(
        diagnostic
            .related
            .iter()
            .any(|context| context.label == "span" && context.value == "10..25")
    );
}

const VALID_MANIFEST: &str = r#"
[identity]
id = "com.hawk2ui.desktop-basic"
name = "Desktop Basic"
version = "0.1.0"

[source]
entry = "src/main.ts"

[capabilities]
keys = ["native-windowing", "sealed-artifacts"]

[[targets]]
kind = "desktop"
name = "linux-wayland"

[plugin]
id = "com.hawk2ui.plugin-basic"
name = "Plugin Basic"

[editor]
width = 960
height = 540

[[parameters]]
id = "gain"
name = "Gain"
default = 0.5
"#;

#[test]
fn manifest_validation_accepts_complete_manifest() {
    let manifest = HawkManifest::parse(VALID_MANIFEST).expect("valid manifest parses");

    assert_eq!(manifest.identity.id, "com.hawk2ui.desktop-basic");
    assert!(manifest.has_capability("native-windowing"));
    assert!(manifest.has_target(PackageTarget::Desktop));
    assert_eq!(manifest.parameters.len(), 1);
}

#[test]
fn manifest_validation_accepts_package_assets_entrypoints_and_presets() {
    let input = r#"
[identity]
id = "com.hawk2ui.full"
name = "Full"
version = "1.2.3"

[package]
name = "full"
bundle_id = "com.hawk2ui.full"

[source]
entry = "src/main.ts"
style = "src/style.hawk.css"
script = "src/main.ts"

[capabilities]
keys = ["native-windowing", "assets-read"]

[[assets]]
id = "hero"
kind = "image"
path = "assets/hero.png"

[[targets]]
kind = "desktop"
name = "linux-wayland"

[[presets]]
id = "default"
name = "Default"
"#;

    let manifest = HawkManifest::parse(input).expect("complete production manifest parses");

    assert_eq!(
        manifest.package.as_ref().unwrap().bundle_id,
        "com.hawk2ui.full"
    );
    assert_eq!(manifest.source.style.as_deref(), Some("src/style.hawk.css"));
    assert_eq!(manifest.assets[0].id, "hero");
    assert_eq!(manifest.presets[0].id, "default");
}

#[test]
fn manifest_validation_rejects_duplicate_assets_and_presets() {
    let input = r#"
[identity]
id = "com.hawk2ui.duplicates"
name = "Duplicates"
version = "1.0.0"

[source]
entry = "src/main.ts"

[[assets]]
id = "hero"
kind = "image"
path = "assets/hero.png"

[[assets]]
id = "hero"
kind = "image"
path = "assets/hero-copy.png"

[[presets]]
id = "default"
name = "Default"

[[presets]]
id = "default"
name = "Default Copy"
"#;

    let error = HawkManifest::parse(input).expect_err("duplicate assets must fail first");

    assert_eq!(error, ManifestError::DuplicateAsset("hero".into()));
}

#[test]
fn manifest_validation_rejects_missing_identity() {
    let input = r#"
[source]
entry = "src/main.ts"
"#;

    let error = HawkManifest::parse(input).expect_err("missing identity must fail");

    assert_eq!(error, ManifestError::MissingSection("identity"));
}

#[test]
fn manifest_validation_rejects_schema_invalid_unknown_fields() {
    let input = r#"[identity]
id = "com.example.schema-invalid"
name = "Schema Invalid"
version = "1.0.0"

[source]
entry = "src/main.ts"

[unknown]
enabled = true

[[targets]]
kind = "desktop"
name = "desktop"
"#;

    let error = HawkManifest::parse(input).expect_err("unknown manifest sections must fail schema");

    match error {
        ManifestError::SchemaValidation { path, message } => {
            assert_eq!(path, "");
            assert!(message.contains("unknown"));
        }
        other => panic!("expected schema validation error, got {other:?}"),
    }
}

#[test]
fn manifest_validation_rejects_duplicate_targets() {
    let input = r#"
[identity]
id = "com.hawk2ui.duplicate"
name = "Duplicate"
version = "0.1.0"

[source]
entry = "src/main.ts"

[[targets]]
kind = "desktop"
name = "linux-wayland"

[[targets]]
kind = "desktop"
name = "linux-wayland"
"#;

    let error = HawkManifest::parse(input).expect_err("duplicate targets must fail");

    assert_eq!(
        error,
        ManifestError::DuplicateTarget("linux-wayland".into())
    );
}

#[test]
fn manifest_validation_rejects_invalid_plugin_parameters() {
    let single = r#"
[identity]
id = "com.hawk2ui.bad-plugin"
name = "Bad Plugin"
version = "0.1.0"

[source]
entry = "src/main.ts"

[plugin]
id = "com.hawk2ui.bad-plugin"
name = "Bad Plugin"

[[parameters]]
id = "gain"
name = "Gain"
default = 0.5
"#;
    let duplicate = r#"
[identity]
id = "com.hawk2ui.bad-plugin"
name = "Bad Plugin"
version = "0.1.0"

[source]
entry = "src/main.ts"

[plugin]
id = "com.hawk2ui.bad-plugin"
name = "Bad Plugin"

[[parameters]]
id = "gain"
name = "Gain"
default = 0.5

[[parameters]]
id = "gain"
name = "Gain Copy"
default = 0.25
"#;
    let invalid_id = single.replace("id = \"gain\"", "id = \"Bad Id\"");
    let empty_name = single.replace("name = \"Gain\"", "name = \"   \"");
    // Default 2.0 sits above the implicit unit-interval max (no min/max given).
    let out_of_range = single.replace("default = 0.5", "default = 2.0");
    // A default below an explicit custom min must also be rejected.
    let below_custom_min =
        single.replace("default = 0.5", "min = 20.0\nmax = 100.0\ndefault = 5.0");
    // An unrecognized unit label is a typo, not silently dropped.
    let unknown_unit = single.replace("default = 0.5", "default = 0.5\nunit = \"decibels\"");
    // An integer parameter rejects a fractional default.
    let fractional_int = single.replace(
        "default = 0.5",
        "kind = \"int\"\nmin = 0.0\nmax = 10.0\ndefault = 2.5",
    );
    // A boolean parameter must default to exactly 0 or 1.
    let non_binary_bool = single.replace("default = 0.5", "kind = \"bool\"\ndefault = 0.5");

    assert_eq!(
        HawkManifest::parse(duplicate).expect_err("duplicate parameters must fail"),
        ManifestError::DuplicateParameter("gain".into())
    );
    assert_eq!(
        HawkManifest::parse(&invalid_id).expect_err("invalid parameter id must fail"),
        ManifestError::InvalidPluginParameter("Bad Id".into())
    );
    assert_eq!(
        HawkManifest::parse(&empty_name).expect_err("empty parameter name must fail"),
        ManifestError::MissingField("parameter.name")
    );
    assert_eq!(
        HawkManifest::parse(&out_of_range).expect_err("default above the implicit max must fail"),
        ManifestError::InvalidPluginParameter("gain".into())
    );
    assert_eq!(
        HawkManifest::parse(&below_custom_min).expect_err("default below a custom min must fail"),
        ManifestError::InvalidPluginParameter("gain".into())
    );
    assert_eq!(
        HawkManifest::parse(&unknown_unit).expect_err("unknown unit label must fail"),
        ManifestError::InvalidPluginParameter("gain".into())
    );
    assert_eq!(
        HawkManifest::parse(&fractional_int).expect_err("fractional int default must fail"),
        ManifestError::InvalidPluginParameter("gain".into())
    );
    assert_eq!(
        HawkManifest::parse(&non_binary_bool).expect_err("non-binary bool default must fail"),
        ManifestError::InvalidPluginParameter("gain".into())
    );
}

const RANGED_PARAMS_MANIFEST: &str = r#"
[identity]
id = "com.hawk2ui.ranged"
name = "Ranged"
version = "0.1.0"

[source]
entry = "src/main.ts"

[plugin]
id = "com.hawk2ui.ranged"
name = "Ranged"

[[parameters]]
id = "osc.mix"
name = "Osc Mix"
default = 0.4

[[parameters]]
id = "filter.cutoff"
name = "Cutoff"
min = 20.0
max = 20000.0
default = 1000.0
unit = "Hz"
"#;

#[test]
fn manifest_exposes_a_parameter_model_for_codegen() {
    let manifest = HawkManifest::parse(RANGED_PARAMS_MANIFEST).expect("ranged manifest parses");
    let model = manifest.parameter_model();
    model
        .validate()
        .expect("the generated parameter model is valid");

    assert_eq!(model.parameters.len(), 2);

    // A parameter that omits min/max behaves like a normalized unit-interval
    // control, and its plain default carries through unchanged.
    let mix = &model.parameters[0];
    assert_eq!(mix.id, "osc.mix");
    assert_eq!(mix.unit, "");
    assert_eq!(mix.range, Some(ParameterRange::new(0.0, 1.0, 0.4)));
    assert_eq!(mix.default_value, ParameterValue::Float(0.4));

    // An explicit plain range and unit flow straight into the model the truce
    // and TypeScript emitters consume.
    let cutoff = &model.parameters[1];
    assert_eq!(cutoff.id, "filter.cutoff");
    assert_eq!(cutoff.unit, "Hz");
    assert_eq!(
        cutoff.range,
        Some(ParameterRange::new(20.0, 20000.0, 1000.0))
    );
    assert_eq!(cutoff.default_value, ParameterValue::Float(1000.0));
}

#[test]
// Each manifest fixture is colocated with the assertion that consumes it, which
// reads better here than hoisting three large raw strings to the top of the fn.
#[allow(clippy::items_after_statements)]
fn manifest_rejects_a_reserved_or_duplicate_pinned_param_id() {
    // A pinned numeric id at or above truce's reserved meter range (2^24) is
    // rejected at parse time (exit code 10) with a precise diagnostic, rather
    // than surfacing as a downstream Rust compile error in generated truce
    // source.
    const RESERVED: &str = r#"
[identity]
id = "com.hawk2ui.pinned"
name = "Pinned"
version = "0.1.0"

[source]
entry = "src/main.ts"

[plugin]
id = "com.hawk2ui.pinned"
name = "Pinned"

[[parameters]]
id = "gain"
name = "Gain"
param_id = 16777216
default = 0.5
"#;
    let error = HawkManifest::parse(RESERVED).expect_err("a reserved pinned id must be rejected");
    assert!(
        matches!(error, hawk2ui_build::ManifestError::ReservedParameterId { param_id, .. } if param_id == 1 << 24),
        "expected ReservedParameterId, got {error:?}"
    );

    // Two parameters pinning the same id alias their saved automation/state.
    const DUPLICATE: &str = r#"
[identity]
id = "com.hawk2ui.pinned"
name = "Pinned"
version = "0.1.0"

[source]
entry = "src/main.ts"

[plugin]
id = "com.hawk2ui.pinned"
name = "Pinned"

[[parameters]]
id = "gain"
name = "Gain"
param_id = 3
default = 0.5

[[parameters]]
id = "mix"
name = "Mix"
param_id = 3
default = 0.5
"#;
    let error = HawkManifest::parse(DUPLICATE).expect_err("duplicate pinned ids must be rejected");
    assert!(
        matches!(error, hawk2ui_build::ManifestError::DuplicateParameterId { param_id, .. } if param_id == 3),
        "expected DuplicateParameterId, got {error:?}"
    );

    // A unique pinned id below the ceiling parses and survives as the resolved
    // truce ParamId.
    const PINNED_OK: &str = r#"
[identity]
id = "com.hawk2ui.pinned"
name = "Pinned"
version = "0.1.0"

[source]
entry = "src/main.ts"

[plugin]
id = "com.hawk2ui.pinned"
name = "Pinned"

[[parameters]]
id = "gain"
name = "Gain"
param_id = 5
default = 0.5
"#;
    let manifest = HawkManifest::parse(PINNED_OK).expect("a valid pinned id parses");
    assert_eq!(manifest.parameter_model().resolved_param_ids(), vec![5]);
}

#[test]
fn pin_param_ids_assigns_unpinned_preserves_comments_and_is_idempotent() {
    const SRC: &str = r#"
# Synth manifest
[identity]
id = "com.hawk2ui.pin"
name = "Pin"
version = "0.1.0"

[source]
entry = "src/main.ts"

[plugin]
id = "com.hawk2ui.pin"
name = "Pin"

[[parameters]]
id = "gain"   # output gain
name = "Gain"
default = 0.5

[[parameters]]
id = "mix"
name = "Mix"
param_id = 5
default = 0.5
"#;
    let hawk2ui_build::PinParamIds::Pinned { source, assigned } =
        hawk2ui_build::pin_param_ids(SRC).expect("valid manifest pins")
    else {
        panic!("expected a rewrite for the unpinned parameter");
    };
    // `gain` was unpinned and takes the lowest free id avoiding the pinned 5.
    assert_eq!(assigned, vec![("gain".to_string(), 0)]);
    // The rewrite preserves the author's comments and the already-pinned id.
    assert!(source.contains("param_id = 0"), "{source}");
    assert!(source.contains("# Synth manifest"), "{source}");
    assert!(source.contains("# output gain"), "{source}");
    assert!(source.contains("param_id = 5"), "{source}");
    // Re-pinning the rewritten manifest is a no-op.
    assert!(matches!(
        hawk2ui_build::pin_param_ids(&source).expect("rewritten manifest re-pins"),
        hawk2ui_build::PinParamIds::Unchanged
    ));
}

const KINDED_PARAMS_MANIFEST: &str = r#"
[identity]
id = "com.hawk2ui.kinded"
name = "Kinded"
version = "0.1.0"

[source]
entry = "src/main.ts"

[plugin]
id = "com.hawk2ui.kinded"
name = "Kinded"

[[parameters]]
id = "filter.cutoff"
name = "Cutoff"
min = 20.0
max = 20000.0
default = 1000.0
unit = "Hz"

[[parameters]]
id = "osc.voices"
name = "Voices"
kind = "int"
min = 1.0
max = 8.0
default = 4.0

[[parameters]]
id = "fx.bypass"
name = "Bypass"
kind = "bool"
default = 0.0
"#;

#[test]
fn manifest_parameter_model_covers_float_int_and_bool_kinds() {
    let manifest = HawkManifest::parse(KINDED_PARAMS_MANIFEST).expect("kinded manifest parses");
    let model = manifest.parameter_model();
    model
        .validate()
        .expect("the generated parameter model is valid");
    assert_eq!(model.parameters.len(), 3);

    assert_eq!(
        model.parameters[0].default_value,
        ParameterValue::Float(1000.0)
    );
    assert_eq!(model.parameters[1].default_value, ParameterValue::Int(4));
    assert_eq!(
        model.parameters[2].default_value,
        ParameterValue::Bool(false)
    );
}

const METERED_PLUGIN_MANIFEST: &str = r#"
[identity]
id = "com.hawk2ui.metered"
name = "Metered"
version = "0.1.0"

[source]
entry = "src/main.ts"

[plugin]
id = "com.hawk2ui.metered"
name = "Metered"

[[parameters]]
id = "osc.mix"
name = "Osc Mix"
default = 0.4

[[meters]]
id = "output.level"
name = "Output Level"

[[meters]]
id = "input.level"
name = "Input Level"
"#;

#[test]
fn manifest_exposes_meters_in_the_parameter_model() {
    let manifest = HawkManifest::parse(METERED_PLUGIN_MANIFEST).expect("metered manifest parses");
    let model = manifest.parameter_model();
    model
        .validate()
        .expect("the generated parameter model is valid");

    // Meters declared in the manifest reach the model in declaration order,
    // alongside (not mixed into) the parameters, ready for the truce emitter to
    // render each as a `#[meter]` field.
    assert_eq!(model.parameters.len(), 1);
    assert_eq!(
        model.meters.len(),
        2,
        "both declared meters reach the model"
    );
    assert_eq!(model.meters[0].id, "output.level");
    assert_eq!(model.meters[0].display_name, "Output Level");
    assert_eq!(model.meters[1].id, "input.level");
    assert_eq!(model.meters[1].display_name, "Input Level");
}

#[test]
fn manifest_validation_rejects_invalid_plugin_meters() {
    let single = r#"
[identity]
id = "com.hawk2ui.bad-meters"
name = "Bad Meters"
version = "0.1.0"

[source]
entry = "src/main.ts"

[plugin]
id = "com.hawk2ui.bad-meters"
name = "Bad Meters"

[[parameters]]
id = "gain"
name = "Gain"
default = 0.5

[[meters]]
id = "output.level"
name = "Output Level"
"#;
    // A meter id may not collide with a parameter id — they share one namespace.
    let collides_with_param = single.replace("id = \"output.level\"", "id = \"gain\"");
    // Two meters cannot share an id either.
    let duplicate_meter =
        format!("{single}\n[[meters]]\nid = \"output.level\"\nname = \"Output Level Copy\"\n");
    // Meter ids follow the same stable-id grammar as parameter ids.
    let invalid_meter_id = single.replace("id = \"output.level\"", "id = \"Bad Meter\"");
    // A meter needs a non-empty display name.
    let empty_meter_name = single.replace("name = \"Output Level\"", "name = \"   \"");

    assert_eq!(
        HawkManifest::parse(&collides_with_param)
            .expect_err("meter id colliding with a parameter id must fail"),
        ManifestError::DuplicateMeter("gain".into())
    );
    assert_eq!(
        HawkManifest::parse(&duplicate_meter).expect_err("duplicate meter id must fail"),
        ManifestError::DuplicateMeter("output.level".into())
    );
    assert_eq!(
        HawkManifest::parse(&invalid_meter_id).expect_err("invalid meter id must fail"),
        ManifestError::InvalidPluginMeter("Bad Meter".into())
    );
    assert_eq!(
        HawkManifest::parse(&empty_meter_name).expect_err("empty meter name must fail"),
        ManifestError::MissingField("meter.name")
    );
}

const ENUM_PARAMS_MANIFEST: &str = r#"
[identity]
id = "com.hawk2ui.enum"
name = "Enum"
version = "0.1.0"

[source]
entry = "src/main.ts"

[plugin]
id = "com.hawk2ui.enum"
name = "Enum"

[[parameters]]
id = "osc.shape"
name = "Osc Shape"
kind = "enum"
default = 1.0

[[parameters.variants]]
id = "sine"
name = "Sine"

[[parameters.variants]]
id = "saw"
name = "Saw"

[[parameters.variants]]
id = "square-pulse"
name = "Square / Pulse"
"#;

#[test]
fn manifest_parameter_model_covers_enum_kind() {
    let manifest = HawkManifest::parse(ENUM_PARAMS_MANIFEST).expect("enum manifest parses");
    let model = manifest.parameter_model();
    model
        .validate()
        .expect("the generated parameter model is valid");
    assert_eq!(model.parameters.len(), 1);

    let shape = &model.parameters[0];
    assert_eq!(shape.id, "osc.shape");
    // The enum default is the 0-based variant index, carried as a Choice value.
    assert_eq!(shape.default_value, ParameterValue::Choice(1));
    assert_eq!(shape.variants.len(), 3);
    assert_eq!(shape.variants[0].id, "sine");
    assert_eq!(shape.variants[2].id, "square-pulse");
    assert_eq!(shape.variants[2].display_name, "Square / Pulse");
}

#[test]
fn manifest_validation_rejects_invalid_plugin_enums() {
    let single = r#"
[identity]
id = "com.hawk2ui.bad-enum"
name = "Bad Enum"
version = "0.1.0"

[source]
entry = "src/main.ts"

[plugin]
id = "com.hawk2ui.bad-enum"
name = "Bad Enum"

[[parameters]]
id = "osc.shape"
name = "Osc Shape"
kind = "enum"
default = 1.0

[[parameters.variants]]
id = "sine"
name = "Sine"

[[parameters.variants]]
id = "saw"
name = "Saw"
"#;
    // A default index beyond the variant range would panic at truce
    // `EnumParam` construction, so the manifest is the compile-time gate.
    let default_out_of_range = single.replace("default = 1.0", "default = 5.0");
    // A fractional enum default is not a variant index.
    let fractional_default = single.replace("default = 1.0", "default = 0.5");
    // An enum needs at least two variants to be meaningful.
    let one_variant = single.replace(
        "\n\n[[parameters.variants]]\nid = \"saw\"\nname = \"Saw\"",
        "",
    );
    // Variant ids must form a valid Rust identifier.
    let invalid_variant = single.replace("id = \"saw\"", "id = \"Bad Variant\"");
    // Two ids that derive the same identifier (`sine`, `sine-` -> `Sine`) collide.
    let colliding_variant = single.replace("id = \"saw\"", "id = \"sine-\"");

    assert_eq!(
        HawkManifest::parse(&default_out_of_range)
            .expect_err("enum default beyond the variant range must fail"),
        ManifestError::InvalidPluginParameter("osc.shape".into())
    );
    assert_eq!(
        HawkManifest::parse(&fractional_default).expect_err("fractional enum default must fail"),
        ManifestError::InvalidPluginParameter("osc.shape".into())
    );
    assert_eq!(
        HawkManifest::parse(&one_variant).expect_err("a single-variant enum must fail"),
        ManifestError::InvalidPluginParameter("osc.shape".into())
    );
    assert_eq!(
        HawkManifest::parse(&invalid_variant).expect_err("a non-identifier variant id must fail"),
        ManifestError::InvalidEnumVariant("Bad Variant".into())
    );
    assert_eq!(
        HawkManifest::parse(&colliding_variant)
            .expect_err("variant ids deriving the same identifier must fail"),
        ManifestError::CollidingEnumVariant("sine-".into())
    );
}

#[test]
fn manifest_validation_rejects_colliding_field_identifiers() {
    // Two *distinct* ids that derive the same Rust field identifier — a
    // parameter `level.db` and a meter `level-db`, both mapping to `level_db` —
    // would emit a `Params` struct with two fields of one name. Params and
    // meters share the field namespace, so the collision must be rejected even
    // though the raw ids differ (the raw-id uniqueness check passes).
    let manifest = r#"
[identity]
id = "com.hawk2ui.field-collision"
name = "Field Collision"
version = "0.1.0"

[source]
entry = "src/main.ts"

[plugin]
id = "com.hawk2ui.field-collision"
name = "Field Collision"

[[parameters]]
id = "level.db"
name = "Level"
min = -60.0
max = 0.0
default = 0.0
unit = "dB"

[[meters]]
id = "level-db"
name = "Level Meter"
"#;

    assert_eq!(
        HawkManifest::parse(manifest)
            .expect_err("ids deriving the same struct field must fail validation"),
        ManifestError::CollidingFieldIdentifier("level-db".into())
    );
}

#[test]
fn manifest_validation_rejects_colliding_enum_type_identifiers() {
    // Two enum parameters whose ids derive the same generated `ParamEnum` type.
    // `pascal_ident` collapses separators while `field_ident` keeps their
    // count, so `osc.shape` and `osc..shape` collide on the type (both
    // `...OscShape`) without colliding on the field (`osc_shape` vs
    // `osc__shape`). The field-ident guard cannot see this collision, so the
    // enum-only type guard must — and the ids are deliberately chosen so the
    // field guard does not fire first and pass the test for the wrong reason.
    let manifest = r#"
[identity]
id = "com.hawk2ui.enum-type-collision"
name = "Enum Type Collision"
version = "0.1.0"

[source]
entry = "src/main.ts"

[plugin]
id = "com.hawk2ui.enum-type-collision"
name = "Enum Type Collision"

[[parameters]]
id = "osc.shape"
name = "Osc Shape"
kind = "enum"
default = 0.0

[[parameters.variants]]
id = "sine"
name = "Sine"

[[parameters.variants]]
id = "saw"
name = "Saw"

[[parameters]]
id = "osc..shape"
name = "Osc Shape Two"
kind = "enum"
default = 0.0

[[parameters.variants]]
id = "sine"
name = "Sine"

[[parameters.variants]]
id = "saw"
name = "Saw"
"#;

    assert_eq!(
        HawkManifest::parse(manifest)
            .expect_err("enum ids deriving the same ParamEnum type must fail validation"),
        ManifestError::CollidingEnumType("osc..shape".into())
    );
}

#[test]
fn sealed_artifact_hashes_manifest_snapshot_stably() {
    let manifest = HawkManifest::parse(VALID_MANIFEST).expect("valid manifest parses");
    let artifact = SealedArtifact::from_manifest(ArtifactSchemaVersion::new(1, 0), &manifest);

    assert_eq!(artifact.schema_version, ArtifactSchemaVersion::new(1, 0));
    assert_eq!(artifact.manifest_snapshot, manifest.snapshot());
    assert_eq!(
        artifact.manifest_snapshot_hash,
        ArtifactHash::from_bytes(manifest.snapshot().as_bytes())
    );
    assert!(artifact.is_compatible_with(ArtifactSchemaVersion::new(1, 2)));
    assert!(!artifact.is_compatible_with(ArtifactSchemaVersion::new(2, 0)));
}

#[test]
fn sealed_artifact_carries_compiled_records_and_metadata() {
    let manifest = HawkManifest::parse(VALID_MANIFEST).expect("valid manifest parses");
    let artifact = SealedArtifact::from_manifest(ArtifactSchemaVersion::new(1, 0), &manifest)
        .with_compiled_script(CompiledScriptRecord::new(
            "main",
            "src/main.ts",
            "scripts/main.hawk.js",
            ArtifactHash::from_bytes(b"script"),
        ))
        .with_compiled_style(CompiledStyleRecord::new(
            "main",
            "src/style.hawk.css",
            "styles/main.hawk.style",
            ArtifactHash::from_bytes(b"style"),
        ))
        .with_asset_manifest_entry(AssetManifestEntry::new(
            "hero",
            "image",
            "assets/hero.png",
            ArtifactHash::from_bytes(b"asset"),
        ))
        .with_compiled_asset(CompiledAssetRecord::new(
            "hero",
            "assets/hero.png",
            "assets/hero.pack",
            ArtifactHash::from_bytes(b"asset"),
        ));

    assert_eq!(artifact.compiled_scripts.len(), 1);
    assert_eq!(artifact.compiled_styles.len(), 1);
    assert_eq!(artifact.asset_manifest.len(), 1);
    assert_eq!(artifact.compiled_assets.len(), 1);
    assert_eq!(
        artifact.capabilities,
        vec![
            "native-windowing".to_string(),
            "sealed-artifacts".to_string()
        ]
    );
    assert_eq!(artifact.hashes.manifest, artifact.manifest_snapshot_hash);
    assert_eq!(artifact.build_metadata.generator, "hawk2ui-build");
    assert_eq!(artifact.target_metadata[0].kind, PackageTarget::Desktop);
    assert_eq!(artifact.target_metadata[0].name, "linux-wayland");
}

#[test]
fn sealed_artifact_generates_and_validates_json_schema() {
    let manifest = HawkManifest::parse(VALID_MANIFEST).expect("valid manifest parses");
    let artifact = SealedArtifact::from_manifest(ArtifactSchemaVersion::new(1, 0), &manifest)
        .with_compiled_script(CompiledScriptRecord::new(
            "main",
            "src/main.ts",
            "scripts/main.hawk.js",
            ArtifactHash::from_bytes(b"script"),
        ))
        .with_asset_manifest_entry(AssetManifestEntry::new(
            "hero",
            "image",
            "assets/hero.png",
            ArtifactHash::from_bytes(b"asset"),
        ));
    let artifact_json = serde_json::to_value(&artifact).expect("artifact serializes");

    let schema = SealedArtifact::json_schema().expect("artifact schema generates");
    SealedArtifact::validate_json(&artifact_json).expect("artifact schema accepts artifact JSON");

    let schema_text = schema.to_string();
    assert!(schema_text.contains("manifest_snapshot_hash"));
    assert!(schema_text.contains("compiled_scripts"));
    assert!(schema_text.contains("asset_manifest"));
    assert!(schema_text.contains("target_metadata"));
}

#[test]
fn sealed_artifact_content_hash_changes_when_compiled_payload_changes() {
    let manifest = HawkManifest::parse(VALID_MANIFEST).expect("valid manifest parses");
    let first = SealedArtifact::from_manifest(ArtifactSchemaVersion::new(1, 0), &manifest)
        .with_compiled_script(CompiledScriptRecord::new(
            "main",
            "src/main.ts",
            "scripts/main.hawk.js",
            ArtifactHash::from_bytes(b"script-a"),
        ));
    let second = SealedArtifact::from_manifest(ArtifactSchemaVersion::new(1, 0), &manifest)
        .with_compiled_script(CompiledScriptRecord::new(
            "main",
            "src/main.ts",
            "scripts/main.hawk.js",
            ArtifactHash::from_bytes(b"script-a"),
        ));
    let changed = SealedArtifact::from_manifest(ArtifactSchemaVersion::new(1, 0), &manifest)
        .with_compiled_script(CompiledScriptRecord::new(
            "main",
            "src/main.ts",
            "scripts/main.hawk.js",
            ArtifactHash::from_bytes(b"script-b"),
        ));

    assert_eq!(first.content_hash(), second.content_hash());
    assert_ne!(first.content_hash(), changed.content_hash());
}

#[test]
fn sealed_artifact_container_serializes_verifies_and_enforces_signature_policy() {
    let manifest = HawkManifest::parse(VALID_MANIFEST).expect("valid manifest parses");
    let artifact = SealedArtifact::from_manifest(ArtifactSchemaVersion::new(1, 0), &manifest)
        .with_compiled_script(CompiledScriptRecord::new(
            "main",
            "src/main.ts",
            "scripts/main.hawk.js",
            ArtifactHash::from_bytes(b"script"),
        ));

    let first = artifact
        .to_container_bytes(ArtifactSignaturePolicy::AllowUnsignedDevelopment)
        .expect("development container serializes");
    let second = artifact
        .to_container_bytes(ArtifactSignaturePolicy::AllowUnsignedDevelopment)
        .expect("development container serializes deterministically");
    assert_eq!(first, second);

    let restored = SealedArtifact::from_container_bytes(
        &first,
        ArtifactSchemaVersion::new(1, 0),
        ArtifactSignaturePolicy::AllowUnsignedDevelopment,
    )
    .expect("container verifies and deserializes");
    assert_eq!(restored, artifact);

    let mut tampered = first.clone();
    *tampered.last_mut().expect("container is non-empty") ^= 0x01;
    let error = SealedArtifact::from_container_bytes(
        &tampered,
        ArtifactSchemaVersion::new(1, 0),
        ArtifactSignaturePolicy::AllowUnsignedDevelopment,
    )
    .expect_err("tampered container fails verification");
    assert!(matches!(
        error,
        SealedArtifactError::ContainerVerification { .. }
    ));

    let release_error = artifact
        .to_container_bytes(ArtifactSignaturePolicy::RequireVerifiedSignature)
        .expect_err("release policy rejects unsigned artifacts");
    assert!(matches!(
        release_error,
        SealedArtifactError::SignaturePolicy { .. }
    ));

    let (signed, verifier) = sign_artifact(artifact, "release-key");
    let signed_bytes = signed
        .to_container_bytes(ArtifactSignaturePolicy::RequireVerifiedSignature)
        .expect("release policy accepts structurally valid signature metadata");
    let trusted = SealedArtifact::from_trusted_container_bytes(
        &signed_bytes,
        ArtifactSchemaVersion::new(1, 0),
        &verifier,
    )
    .expect("trusted release signature verifies");
    assert_eq!(trusted, signed);
}

#[test]
fn artifact_signing_key_signs_verifiable_release_container() {
    let manifest = HawkManifest::parse(VALID_MANIFEST).expect("valid manifest parses");
    let artifact = SealedArtifact::from_manifest(ArtifactSchemaVersion::new(1, 0), &manifest)
        .with_compiled_script(CompiledScriptRecord::new(
            "main",
            "src/main.ts",
            "scripts/main.hawk.js",
            ArtifactHash::from_bytes(b"script"),
        ));
    let signing_key = ArtifactSigningKey::ed25519_sha256_v1("release-key", [7; 32]);

    let signed = signing_key.sign(&artifact);
    let release_bytes = signed
        .to_container_bytes(ArtifactSignaturePolicy::RequireVerifiedSignature)
        .expect("signed release container serializes");
    let verifier = ArtifactSignatureVerifier::new([signing_key.verification_key()]);
    let trusted = SealedArtifact::from_trusted_container_bytes(
        &release_bytes,
        ArtifactSchemaVersion::new(1, 0),
        &verifier,
    )
    .expect("trusted signature verifies");

    assert_eq!(signed.signature.key_id, "release-key");
    assert_eq!(trusted, signed);
}

#[test]
fn sealed_artifact_rejects_untrusted_or_invalid_release_signature() {
    let manifest = HawkManifest::parse(VALID_MANIFEST).expect("valid manifest parses");
    let artifact = SealedArtifact::from_manifest(ArtifactSchemaVersion::new(1, 0), &manifest)
        .with_compiled_script(CompiledScriptRecord::new(
            "main",
            "src/main.ts",
            "scripts/main.hawk.js",
            ArtifactHash::from_bytes(b"script"),
        ));
    let (signed, verifier) = sign_artifact(artifact, "release-key");
    let signed_bytes = signed
        .to_container_bytes(ArtifactSignaturePolicy::RequireVerifiedSignature)
        .expect("release container serializes");
    let untrusted =
        ArtifactSignatureVerifier::new([ArtifactSignatureVerificationKey::ed25519_sha256_v1(
            "other-key",
            [3; 32],
        )]);
    let error = SealedArtifact::from_trusted_container_bytes(
        &signed_bytes,
        ArtifactSchemaVersion::new(1, 0),
        &untrusted,
    )
    .expect_err("untrusted key must fail");
    assert!(matches!(
        error,
        SealedArtifactError::SignatureVerification { .. }
    ));

    let tampered = signed.with_compiled_script(CompiledScriptRecord::new(
        "secondary",
        "src/secondary.ts",
        "scripts/secondary.hawk.js",
        ArtifactHash::from_bytes(b"secondary"),
    ));
    let tampered_bytes = tampered
        .to_container_bytes(ArtifactSignaturePolicy::RequireVerifiedSignature)
        .expect("tampered signed metadata still serializes");
    let error = SealedArtifact::from_trusted_container_bytes(
        &tampered_bytes,
        ArtifactSchemaVersion::new(1, 0),
        &verifier,
    )
    .expect_err("payload changed after signing must fail");
    assert!(matches!(
        error,
        SealedArtifactError::SignatureVerification { .. }
    ));
}

#[test]
fn sealed_artifact_rejects_manifest_snapshot_malleability() {
    let manifest = HawkManifest::parse(VALID_MANIFEST).expect("valid manifest parses");
    let artifact = SealedArtifact::from_manifest(ArtifactSchemaVersion::new(1, 0), &manifest);
    let (signed, _verifier) = sign_artifact(artifact, "release-key");
    let mut tampered = signed;
    tampered.manifest_snapshot = "attacker-controlled-snapshot".to_string();
    tampered.hashes.content = tampered.content_hash();

    let bytes = tampered
        .to_container_bytes(ArtifactSignaturePolicy::RequireVerifiedSignature)
        .expect("structural signature policy still serializes");
    let error = SealedArtifact::from_container_bytes(
        &bytes,
        ArtifactSchemaVersion::new(1, 0),
        ArtifactSignaturePolicy::RequireVerifiedSignature,
    )
    .expect_err("manifest snapshot tampering must be rejected");

    assert!(matches!(
        error,
        SealedArtifactError::ContainerVerification { .. }
    ));
}

#[test]
fn sealed_artifact_reports_incompatible_schema_diagnostic() {
    let manifest = HawkManifest::parse(VALID_MANIFEST).expect("valid manifest parses");
    let artifact = SealedArtifact::from_manifest(ArtifactSchemaVersion::new(2, 0), &manifest);

    let error = artifact
        .ensure_compatible_with(ArtifactSchemaVersion::new(1, 0))
        .expect_err("major version mismatch must fail");

    assert_eq!(
        error,
        SealedArtifactError::IncompatibleSchema {
            expected: ArtifactSchemaVersion::new(1, 0),
            actual: ArtifactSchemaVersion::new(2, 0),
            diagnostic: BuildDiagnostic::new(
                BuildDiagnosticSeverity::Error,
                "artifact.schema.incompatible",
                "sealed artifact schema version is incompatible"
            )
        }
    );
}

#[test]
fn build_pipeline_records_required_phase_order() {
    let pipeline = BuildPipeline::production();

    assert_eq!(
        pipeline.phase_names(),
        [
            "source-discovery",
            "manifest-validation",
            "asset-discovery",
            "source-validation",
            "style-compilation",
            "script-compilation",
            "asset-compilation",
            "artifact-generation",
            "packaging",
            "verification",
        ]
    );
}

#[test]
fn pipeline_phases_expose_required_phase_records() {
    let pipeline = BuildPipeline::production();

    assert_eq!(pipeline.phases.len(), 10);
    assert_eq!(
        pipeline
            .phase(BuildPhase::StyleCompilation)
            .expect("style phase must exist")
            .phase,
        BuildPhase::StyleCompilation
    );
    assert!(
        pipeline
            .phases
            .iter()
            .all(|record| record.diagnostics.is_empty())
    );
}

#[test]
fn pipeline_phases_collect_release_blocking_diagnostics_by_phase() {
    let diagnostic = BuildDiagnostic::new(
        BuildDiagnosticSeverity::Error,
        "script.unsupported.syntax",
        "script syntax is unsupported",
    );
    let pipeline = BuildPipeline::production()
        .with_diagnostic(BuildPhase::ScriptCompilation, diagnostic.clone());

    let blockers = pipeline.release_blocking_diagnostics();

    assert_eq!(blockers.len(), 1);
    assert_eq!(blockers[0].phase, BuildPhase::ScriptCompilation);
    assert_eq!(blockers[0].diagnostic, diagnostic);
}

#[test]
fn build_pipeline_propagates_phase_diagnostics() {
    let pipeline = BuildPipeline::production().with_diagnostic(
        BuildPhase::ManifestValidation,
        BuildDiagnostic::new(
            BuildDiagnosticSeverity::Error,
            "manifest.identity.missing",
            "manifest identity is required",
        ),
    );

    let error = pipeline
        .ensure_release_ready()
        .expect_err("error diagnostic must block release");

    assert_eq!(
        error,
        BuildPipelineError::ReleaseBlocked("manifest.identity.missing".into())
    );
}

#[test]
fn asset_compilation_records_metadata_for_supported_asset_kinds() {
    let input = r#"
[identity]
id = "com.hawk2ui.assets"
name = "Assets"
version = "1.0.0"

[source]
entry = "src/main.ts"

[[assets]]
id = "hero"
kind = "image"
path = "assets/hero.png"

[[assets]]
id = "logo"
kind = "vector"
path = "assets/logo.svg"

    [[assets]]
    id = "theme"
    kind = "design-token"
    path = "tokens/theme.json"
    "#;
    let font = fixture_font_bytes();
    let has_font = font.is_some();
    let input = if has_font {
        format!(
            "{input}\n[[assets]]\nid = \"display\"\nkind = \"font\"\npath = \"assets/display.ttf\"\n"
        )
    } else {
        input.to_string()
    };
    let manifest = HawkManifest::parse(&input).expect("asset manifest parses");
    let mut sources = vec![
        AssetSource::new("assets/hero.png", tiny_png()),
        AssetSource::new("assets/logo.svg", simple_svg("#ffffff")),
        AssetSource::new("tokens/theme.json", br##"{"color":{"surface":"#080a0e"}}"##),
    ];
    if let Some(font) = font {
        sources.push(AssetSource::new("assets/display.ttf", font));
    }
    let index = AssetSourceIndex::new(sources);

    let records = AssetCompilationPlan::compile_manifest(&manifest, &index)
        .expect("all declared assets compile");

    assert_eq!(records.len(), if has_font { 4 } else { 3 });
    assert_eq!(records[0].kind, AssetKind::Image);
    assert_eq!(records[0].dimensions, Some(AssetDimensions::new(1, 1)));
    assert_eq!(records[0].sanitization, AssetSanitizationStatus::Sanitized);
    assert_eq!(records[0].package.package_path, "assets/hero.pack");
    assert!(records[0].package.cache_key.starts_with("image:hero:"));
    assert_eq!(records[1].kind, AssetKind::Vector);
    assert_eq!(records[1].sanitization, AssetSanitizationStatus::Sanitized);
    assert_eq!(records[2].kind, AssetKind::DesignToken);
    if records.len() == 4 {
        assert_eq!(records[3].kind, AssetKind::Font);
    }
}

#[test]
fn asset_compilation_reports_missing_asset() {
    let input = r#"
[identity]
id = "com.hawk2ui.missing-asset"
name = "Missing Asset"
version = "1.0.0"

[source]
entry = "src/main.ts"

[[assets]]
id = "hero"
kind = "image"
path = "assets/hero.png"
"#;
    let manifest = HawkManifest::parse(input).expect("asset manifest parses");

    let error = AssetCompilationPlan::compile_manifest(&manifest, &AssetSourceIndex::empty())
        .expect_err("missing assets must fail");

    assert_eq!(
        error,
        AssetCompilationError::MissingAsset {
            id: "hero".into(),
            path: "assets/hero.png".into(),
            diagnostic: Box::new(BuildDiagnostic::new(
                BuildDiagnosticSeverity::Error,
                "asset.missing",
                "declared asset source is missing"
            ))
        }
    );
}

#[test]
fn asset_compilation_rejects_unsafe_asset() {
    let input = r#"
[identity]
id = "com.hawk2ui.unsafe-asset"
name = "Unsafe Asset"
version = "1.0.0"

[source]
entry = "src/main.ts"

[[assets]]
id = "hero"
kind = "image"
path = "assets/hero.png"
"#;
    let manifest = HawkManifest::parse(input).expect("asset manifest parses");
    let index =
        AssetSourceIndex::new([AssetSource::new("assets/hero.png", b"hero").unsafe_asset()]);

    let error = AssetCompilationPlan::compile_manifest(&manifest, &index)
        .expect_err("unsafe assets must fail");

    assert_eq!(
        error,
        AssetCompilationError::UnsafeAsset {
            id: "hero".into(),
            path: "assets/hero.png".into(),
            diagnostic: Box::new(BuildDiagnostic::new(
                BuildDiagnosticSeverity::Error,
                "asset.unsafe",
                "declared asset failed safety validation"
            ))
        }
    );
}

#[test]
fn asset_compilation_cache_metadata_changes_when_source_changes() {
    let input = r#"
[identity]
id = "com.hawk2ui.cache-asset"
name = "Cache Asset"
version = "1.0.0"

[source]
entry = "src/main.ts"

[[assets]]
id = "hero"
    kind = "vector"
    path = "assets/hero.svg"
    "#;
    let manifest = HawkManifest::parse(input).expect("asset manifest parses");
    let first = AssetSourceIndex::new([AssetSource::new("assets/hero.svg", simple_svg("#ffffff"))]);
    let second =
        AssetSourceIndex::new([AssetSource::new("assets/hero.svg", simple_svg("#000000"))]);

    let first_records =
        AssetCompilationPlan::compile_manifest(&manifest, &first).expect("first asset compiles");
    let second_records =
        AssetCompilationPlan::compile_manifest(&manifest, &second).expect("second asset compiles");

    assert_ne!(
        first_records[0].package.cache_key,
        second_records[0].package.cache_key
    );
}

#[test]
fn verification_report_tracks_package_targets_and_diagnostics() {
    let report = VerificationReport::new("com.hawk2ui.desktop-basic")
        .with_package_target(PackageTargetRecord::new(
            PackageTarget::Desktop,
            "linux-wayland",
        ))
        .with_diagnostic(BuildDiagnostic::new(
            BuildDiagnosticSeverity::Warning,
            "style.unsupported.warning",
            "style warning",
        ));

    assert_eq!(report.product_id, "com.hawk2ui.desktop-basic");
    assert_eq!(report.package_targets.len(), 1);
    assert_eq!(report.diagnostics.len(), 1);
    assert!(report.is_release_ready());
}

#[test]
fn verification_report_snapshots_diagnostics_with_locations() {
    let report = VerificationReport::new("com.hawk2ui.report")
        .with_package_target(PackageTargetRecord::new(
            PackageTarget::Desktop,
            "linux-wayland",
        ))
        .with_invalid_manifest(
            "Hawk.toml",
            SourceSpan::new(0, 12),
            "manifest identity is invalid",
        )
        .with_unsupported_style("src/app.css", SourceSpan::new(13, 21))
        .with_unsupported_script("src/app.ts", SourceSpan::new(22, 34))
        .with_unsafe_asset("assets/hero.svg", SourceSpan::new(35, 46))
        .with_missing_asset("assets/missing.png", SourceSpan::new(47, 58))
        .with_undeclared_capability("native-windowing", SourceSpan::new(59, 74))
        .with_target_incompatibility("linux-wayland", SourceSpan::new(75, 88));

    assert_eq!(
        report.render_text(),
        "\
product: com.hawk2ui.report
targets:
- desktop linux-wayland
diagnostics:
- error manifest.invalid Hawk.toml:0..12 manifest identity is invalid
- error style.unsupported src/app.css:13..21 style entrypoint is unsupported
- error script.unsupported src/app.ts:22..34 script entrypoint is unsupported
- error asset.unsafe assets/hero.svg:35..46 asset failed safety validation
- error asset.missing assets/missing.png:47..58 asset source is missing
- error capability.undeclared <manifest>:59..74 capability is not declared: native-windowing
- error target.incompatible <manifest>:75..88 target is incompatible: linux-wayland
"
    );
    assert!(!report.is_release_ready());
}

#[test]
fn build_workspace_reads_project_files_and_materializes_sealed_artifact() {
    let root = temp_build_workspace("complete");
    write_file(
        &root.join("manifest.hawk.toml"),
        r#"
[identity]
id = "com.hawk2ui.workspace"
name = "Workspace"
version = "1.0.0"

[source]
entry = "src/main.ts"
style = "styles/main.hawk.css"
script = "src/bootstrap.ts"

[capabilities]
keys = ["native-windowing", "sealed-artifacts"]

[[targets]]
kind = "desktop"
name = "linux-wayland"

[[assets]]
id = "logo"
kind = "vector"
path = "assets/logo.svg"
"#,
    );
    write_file(
        &root.join("src/main.ts"),
        "export const app: string = 'hawk';",
    );
    write_file(
        &root.join("src/bootstrap.ts"),
        "export const boot: boolean = true;",
    );
    write_file(
        &root.join("styles/main.hawk.css"),
        ".root { display: flex; font-size: 18px; background-color: token(color.surface); }",
    );
    write_file(
        &root.join("assets/logo.svg"),
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><path fill="#ffffff" d="M0 0H16V16H0Z"/></svg>"##,
    );

    let output = BuildWorkspace::load(&root)
        .and_then(|workspace| workspace.build(ArtifactSchemaVersion::new(1, 0)))
        .expect("workspace should build from real files");

    assert_eq!(output.manifest.identity.id, "com.hawk2ui.workspace");
    assert!(output.pipeline.ensure_release_ready().is_ok());
    assert!(output.verification.is_release_ready());
    assert_eq!(output.artifact.compiled_scripts.len(), 2);
    assert_eq!(output.artifact.compiled_styles.len(), 1);
    assert_eq!(output.artifact.asset_manifest.len(), 1);
    assert_eq!(output.artifact.compiled_assets.len(), 1);
    assert_eq!(
        output.artifact.compiled_scripts[0].source_hash,
        ArtifactHash::from_bytes(b"export const app: string = 'hawk';")
    );
    assert!(
        output.artifact.compiled_scripts[0]
            .compiled_source
            .contains("const app")
    );
    assert!(
        output.artifact.compiled_scripts[0]
            .compiled_source
            .contains("hawk")
    );
    assert!(
        !output.artifact.compiled_scripts[0]
            .compiled_source
            .contains(": string")
    );
    assert_eq!(
        output.artifact.compiled_styles[0].source_path,
        "styles/main.hawk.css"
    );
    assert_eq!(
        output.artifact.asset_manifest[0].artifact_path,
        "assets/logo.pack"
    );
}

#[test]
fn build_workspace_builds_a_plugin_with_enum_parameter_and_meter() {
    // The emitter unit/golden tests and the parse-level validator tests cover
    // enum parameters and meters in isolation, but nothing drove them through a
    // *real* `BuildWorkspace::load().build()`. Close that gap: a plugin manifest
    // carrying a `kind = "enum"` parameter (with variants) and a `[[meters]]`
    // output must validate, compile, and seal end to end, and the parameter
    // model recovered from the sealed manifest must preserve both the
    // indexed-choice variants and the meter — not merely build without error.
    let root = temp_build_workspace("plugin-enum-meter");
    write_file(
        &root.join("manifest.hawk.toml"),
        r#"
[identity]
id = "com.hawk2ui.plugin-enum-meter"
name = "Plugin Enum Meter"
version = "0.1.0"

[source]
entry = "src/editor.ts"

[capabilities]
keys = ["plugin-editor", "sealed-artifacts"]

[[targets]]
kind = "plugin"
name = "clap-vst3"

[plugin]
id = "com.hawk2ui.plugin-enum-meter"
name = "Plugin Enum Meter"

[editor]
width = 640
height = 360

[[parameters]]
id = "osc.mix"
name = "Osc Mix"
default = 0.4

[[parameters]]
id = "osc.shape"
name = "Osc Shape"
kind = "enum"
default = 1.0

[[parameters.variants]]
id = "sine"
name = "Sine"

[[parameters.variants]]
id = "saw"
name = "Saw"

[[parameters.variants]]
id = "square-pulse"
name = "Square / Pulse"

[[meters]]
id = "output.level"
name = "Output Level"
"#,
    );
    write_file(
        &root.join("src/editor.ts"),
        "export const editor: string = 'synth';",
    );

    let output = BuildWorkspace::load(&root)
        .and_then(|workspace| workspace.build(ArtifactSchemaVersion::new(1, 0)))
        .expect("a plugin workspace with an enum parameter and a meter should build");

    assert_eq!(output.manifest.identity.id, "com.hawk2ui.plugin-enum-meter");
    assert!(output.pipeline.ensure_release_ready().is_ok());
    assert!(output.verification.is_release_ready());

    // Recover the parameter model from the sealed manifest and confirm the enum
    // and the meter survived the full build path intact.
    let model = output.manifest.parameter_model();
    model
        .validate()
        .expect("the parameter model from the sealed manifest is valid");
    assert_eq!(model.parameters.len(), 2);

    let shape = &model.parameters[1];
    assert_eq!(shape.id, "osc.shape");
    // The enum default is carried as the 0-based variant index.
    assert_eq!(shape.default_value, ParameterValue::Choice(1));
    assert_eq!(shape.variants.len(), 3);
    assert_eq!(shape.variants[2].id, "square-pulse");
    assert_eq!(shape.variants[2].display_name, "Square / Pulse");

    assert_eq!(model.meters.len(), 1);
    assert_eq!(model.meters[0].id, "output.level");
    assert_eq!(model.meters[0].display_name, "Output Level");
}

#[test]
fn build_workspace_rejects_invalid_typescript_before_artifact_materialization() {
    let root = temp_build_workspace("invalid-typescript");
    write_file(
        &root.join("manifest.hawk.toml"),
        r#"
[identity]
id = "com.hawk2ui.invalid-typescript"
name = "Invalid TypeScript"
version = "1.0.0"

[source]
entry = "src/main.ts"
"#,
    );
    write_file(&root.join("src/main.ts"), "export const app: = ;");

    let error = BuildWorkspace::load(&root)
        .and_then(|workspace| workspace.build(ArtifactSchemaVersion::new(1, 0)))
        .expect_err("invalid TypeScript must fail before artifact materialization");

    assert!(matches!(
        error,
        BuildWorkspaceError::ScriptCompilation { path, .. } if path == "src/main.ts"
    ));
}

#[test]
fn build_workspace_rejects_unsupported_script_extensions() {
    let root = temp_build_workspace("unsupported-script");
    write_file(
        &root.join("manifest.hawk.toml"),
        r#"
[identity]
id = "com.hawk2ui.unsupported-script"
name = "Unsupported Script"
version = "1.0.0"

[source]
entry = "src/main.jsx"
"#,
    );
    write_file(&root.join("src/main.jsx"), "export const app = 'hawk';");

    let error = BuildWorkspace::load(&root)
        .and_then(|workspace| workspace.build(ArtifactSchemaVersion::new(1, 0)))
        .expect_err("unsupported script extension must fail");

    assert_eq!(
        error,
        BuildWorkspaceError::UnsupportedScriptExtension("src/main.jsx".into())
    );
}

#[test]
fn build_workspace_rejects_unsupported_styles_before_artifact_materialization() {
    let root = temp_build_workspace("unsupported-style");
    write_file(
        &root.join("manifest.hawk.toml"),
        r#"
[identity]
id = "com.hawk2ui.unsupported-style"
name = "Unsupported Style"
version = "1.0.0"

[source]
entry = "src/main.ts"
style = "styles/main.hawk.css"
"#,
    );
    write_file(&root.join("src/main.ts"), "export const app = 'hawk';");
    write_file(
        &root.join("styles/main.hawk.css"),
        "@media screen { .root { color: white; } }",
    );

    let error = BuildWorkspace::load(&root)
        .and_then(|workspace| workspace.build(ArtifactSchemaVersion::new(1, 0)))
        .expect_err("unsupported CSS must fail before artifact materialization");

    assert!(matches!(
        error,
        BuildWorkspaceError::StyleCompilation { path, .. } if path == "styles/main.hawk.css"
    ));
}

#[test]
fn build_workspace_rejects_missing_declared_source_file() {
    let root = temp_build_workspace("missing-source");
    write_file(
        &root.join("manifest.hawk.toml"),
        r#"
[identity]
id = "com.hawk2ui.missing-source"
name = "Missing Source"
version = "1.0.0"

[source]
entry = "src/main.ts"
"#,
    );

    let error = BuildWorkspace::load(&root)
        .and_then(|workspace| workspace.build(ArtifactSchemaVersion::new(1, 0)))
        .expect_err("missing source must fail");

    assert_eq!(
        error,
        BuildWorkspaceError::MissingFile("src/main.ts".into())
    );
}

#[cfg(unix)]
#[test]
fn build_workspace_rejects_symlinked_declared_files_outside_workspace() {
    let root = temp_build_workspace("symlink-escape");
    let outside = temp_build_workspace("symlink-outside");
    write_file(&outside.join("secret.ts"), "export const secret = true;");
    write_file(
        &root.join("manifest.hawk.toml"),
        r#"
[identity]
id = "com.hawk2ui.symlink"
name = "Symlink"
version = "1.0.0"

[source]
entry = "src/main.ts"
"#,
    );
    fs::create_dir_all(root.join("src")).expect("source directory should be created");
    std::os::unix::fs::symlink(outside.join("secret.ts"), root.join("src/main.ts"))
        .expect("test symlink should be created");

    let error = BuildWorkspace::load(&root)
        .and_then(|workspace| workspace.build(ArtifactSchemaVersion::new(1, 0)))
        .expect_err("symlink escape must fail");

    assert_eq!(error, BuildWorkspaceError::UnsafePath("src/main.ts".into()));
}

fn temp_build_workspace(label: &str) -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("hawk2ui-build-{label}-{now}"));
    fs::create_dir_all(&root).expect("temp build workspace should be created");
    root
}

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("test parent directory should be created");
    }
    fs::write(path, contents).expect("test file should be written");
}

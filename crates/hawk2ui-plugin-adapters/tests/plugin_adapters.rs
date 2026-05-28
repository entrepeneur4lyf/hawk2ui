use hawk2ui_plugin::{
    BundleOutput, FormatMetadata, ParameterFlags, ParameterModel, ParameterRange, ParameterRecord,
    PluginEditor, PluginEditorSize,
};
use hawk2ui_plugin_adapters::{
    ClapCdylibScaffold, ClapGuiParentHandle, ClapGuiWindowApi, ClapPluginEntryPlan,
    ClapRuntimeEditorDescriptor, MaterializedPackageOutput, PackageAdapterSet, PackageFormat,
    PackagePlan, PackageRequest, VerificationReport, VerificationStatus,
};
use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn plugin_adapters_generate_all_supported_package_targets() {
    let metadata = FormatMetadata::new("com.hawk2ui.demo", "Demo", "Hawk2UI").version("1.2.3");
    let parameters = ParameterModel::new([ParameterRecord::numeric(
        "gain",
        "Gain",
        "dB",
        ParameterRange::new(-60.0, 12.0, 0.0),
    )]);
    let request = PackageRequest::new(metadata, BundleOutput::new("dist", "Demo"), parameters)
        .with_format(PackageFormat::Clap)
        .with_format(PackageFormat::Vst3)
        .with_format(PackageFormat::Au)
        .with_format(PackageFormat::Standalone)
        .with_format(PackageFormat::DesktopBundle)
        .with_format(PackageFormat::SealedArtifact);

    let plan = PackageAdapterSet::new()
        .plan(&request)
        .expect("package plan succeeds");

    assert_eq!(plan.targets().len(), 6);
    assert!(
        plan.targets()
            .iter()
            .any(|target| target.format() == PackageFormat::Clap)
    );
    assert!(
        plan.targets()
            .iter()
            .any(|target| target.output_path().ends_with("Demo.clap"))
    );
    assert!(
        plan.targets()
            .iter()
            .any(|target| target.output_path().ends_with("Demo.vst3"))
    );
    assert!(
        plan.targets()
            .iter()
            .any(|target| target.output_path().ends_with("Demo.component"))
    );
    assert!(
        plan.targets()
            .iter()
            .any(|target| target.output_path().ends_with("Demo.app"))
    );
    assert!(
        plan.targets()
            .iter()
            .any(|target| target.output_path().ends_with("Demo.hawk2ui"))
    );
}

#[test]
fn plugin_adapters_emit_metadata_and_verification_reports() {
    let metadata =
        FormatMetadata::new("com.hawk2ui.demo", "Demo", "Hawk2UI").feature("audio-effect");
    let request = PackageRequest::new(
        metadata,
        BundleOutput::new("dist", "Demo"),
        ParameterModel::new([]),
    )
    .with_format(PackageFormat::Clap)
    .with_format(PackageFormat::Vst3);

    let plan = PackageAdapterSet::new()
        .plan(&request)
        .expect("package plan succeeds");
    let report = plan.verify();

    assert_eq!(report.status(), VerificationStatus::Passed);
    assert!(
        report
            .entries()
            .iter()
            .any(|entry| entry.target().format() == PackageFormat::Clap)
    );
    assert!(
        report
            .entries()
            .iter()
            .all(|entry| entry.metadata().id == "com.hawk2ui.demo")
    );
}

#[test]
fn plugin_adapters_generate_and_validate_verification_report_schema() {
    let metadata =
        FormatMetadata::new("com.hawk2ui.schema", "Schema", "Hawk2UI").feature("audio-effect");
    let request = PackageRequest::new(
        metadata,
        BundleOutput::new("dist", "Schema"),
        ParameterModel::new([]),
    )
    .with_format(PackageFormat::Clap)
    .with_format(PackageFormat::DesktopBundle);

    let plan = PackageAdapterSet::new()
        .plan(&request)
        .expect("package plan succeeds");
    let report = plan.verify();
    let schema = VerificationReport::json_schema().expect("verification report schema generates");
    let value = serde_json::to_value(&report).expect("verification report serializes");

    VerificationReport::validate_json(&value)
        .expect("serialized verification report validates against generated schema");
    assert_eq!(schema["title"], "VerificationReport");
    assert!(schema["properties"]["entries"].is_object());

    let mut invalid = value;
    invalid["unexpected"] = serde_json::json!(true);
    let error = VerificationReport::validate_json(&invalid)
        .expect_err("unknown verification report fields fail schema validation");
    assert_eq!(error.rule(), "package.schema.verification-report.invalid");
}

#[test]
fn plugin_adapters_generate_and_validate_package_output_schemas() {
    let metadata = FormatMetadata::new("com.hawk2ui.output", "Output", "Hawk2UI");
    let output_root = std::env::temp_dir().join(format!(
        "hawk2ui-plugin-output-schema-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ));
    let request = PackageRequest::new(
        metadata,
        BundleOutput::new(output_root.to_string_lossy(), "Output"),
        ParameterModel::new([]),
    )
    .with_format(PackageFormat::Clap);

    let plan = PackageAdapterSet::new()
        .plan(&request)
        .expect("package plan succeeds");
    let plan_schema = PackagePlan::json_schema().expect("package plan schema generates");
    let plan_value = serde_json::to_value(&plan).expect("package plan serializes");
    PackagePlan::validate_json(&plan_value).expect("serialized package plan validates");
    assert_eq!(plan_schema["title"], "PackagePlan");
    assert!(plan_schema["properties"]["targets"].is_object());

    let outputs = plan.materialize().expect("materialization succeeds");
    let output_schema =
        MaterializedPackageOutput::json_schema().expect("materialized output schema generates");
    let output_value = serde_json::to_value(&outputs[0]).expect("materialized output serializes");
    MaterializedPackageOutput::validate_json(&output_value)
        .expect("serialized materialized output validates");
    assert_eq!(output_schema["title"], "MaterializedPackageOutput");
    assert!(output_schema["properties"]["hash_manifest_path"].is_object());

    let mut invalid = output_value;
    invalid["unexpected"] = serde_json::json!(true);
    let error = MaterializedPackageOutput::validate_json(&invalid)
        .expect_err("unknown materialized output fields fail schema validation");
    assert_eq!(error.rule(), "package.schema.materialized-output.invalid");
}

#[test]
fn plugin_adapters_materialize_package_metadata_outputs() {
    let metadata =
        FormatMetadata::new("com.hawk2ui.demo", "Demo", "Hawk2UI").feature("audio-effect");
    let output_root = std::env::temp_dir().join(format!(
        "hawk2ui-plugin-adapters-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ));
    let request = PackageRequest::new(
        metadata,
        BundleOutput::new(output_root.to_string_lossy(), "Demo"),
        ParameterModel::new([]),
    )
    .with_format(PackageFormat::Clap);

    let plan = PackageAdapterSet::new()
        .plan(&request)
        .expect("package plan succeeds");
    let outputs = plan.materialize().expect("materialization succeeds");

    assert_eq!(outputs.len(), 1);
    assert!(std::path::Path::new(&outputs[0].manifest_path).is_file());
    assert!(std::path::Path::new(&outputs[0].artifact_descriptor_path).is_file());
    let manifest =
        std::fs::read_to_string(&outputs[0].manifest_path).expect("metadata manifest reads");
    assert!(manifest.contains("format = \"clap\""));
    assert!(manifest.contains("id = \"com.hawk2ui.demo\""));
    let artifact = std::fs::read_to_string(&outputs[0].artifact_descriptor_path)
        .expect("artifact descriptor reads");
    assert!(artifact.contains("artifact_format = \"hawk2ui-plugin-package\""));
    assert!(artifact.contains("entry_library = \"Demo.clap\""));
    let clap_entry = std::fs::read_to_string(
        Path::new(&outputs[0].output_path).join("Contents/Resources/clap-entry.toml"),
    )
    .expect("CLAP entry descriptor reads");
    assert!(clap_entry.contains("entry_symbol = \"clap_entry\""));
    assert!(clap_entry.contains("factory_id = \"clap.plugin-factory\""));
    assert!(clap_entry.contains("clap_version = \"1.2.2\""));
    assert!(clap_entry.contains("features = [\"audio-effect\"]"));

    let report = plan.verify_materialized(&outputs);
    assert_eq!(report.status(), VerificationStatus::Passed);
    std::fs::remove_file(&outputs[0].artifact_descriptor_path)
        .expect("artifact descriptor should be removable");
    let failed = plan.verify_materialized(&outputs);
    assert_eq!(failed.status(), VerificationStatus::Failed);
}

#[test]
fn plugin_adapters_materialize_runtime_artifact_payload_into_package_resources() {
    let metadata =
        FormatMetadata::new("com.hawk2ui.runtime", "Runtime", "Hawk2UI").feature("audio-effect");
    let runtime_artifact = serde_json::json!({
        "schema_version": { "major": 1, "minor": 0 },
        "manifest_snapshot_hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "asset_manifest": [
            { "id": "logo", "kind": "image", "hash": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb" }
        ],
        "compiled_assets": [
            { "id": "logo", "kind": "image", "bytes": 128 }
        ],
        "compiled_styles": [],
        "compiled_scripts": [],
        "capabilities": ["plugin-editor"],
    });
    let output_root = std::env::temp_dir().join(format!(
        "hawk2ui-plugin-runtime-artifact-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ));
    let request = PackageRequest::new(
        metadata,
        BundleOutput::new(output_root.to_string_lossy(), "Runtime"),
        ParameterModel::new([]),
    )
    .with_runtime_artifact(runtime_artifact.clone())
    .with_format(PackageFormat::Clap);

    let plan = PackageAdapterSet::new()
        .plan(&request)
        .expect("package plan succeeds");
    let outputs = plan.materialize().expect("materialization succeeds");
    let root = Path::new(&outputs[0].output_path);
    let runtime_artifact_path = root
        .join("Contents")
        .join("Resources")
        .join("hawk2ui-runtime-artifact.json");

    assert!(runtime_artifact_path.is_file());
    let materialized_artifact: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&runtime_artifact_path).expect("runtime artifact reads"),
    )
    .expect("runtime artifact is JSON");
    assert_eq!(materialized_artifact, runtime_artifact);

    let artifact_descriptor = std::fs::read_to_string(&outputs[0].artifact_descriptor_path)
        .expect("artifact descriptor reads");
    assert!(
        artifact_descriptor
            .contains("runtime_artifact = \"Contents/Resources/hawk2ui-runtime-artifact.json\"")
    );
    let editor_descriptor_path = root
        .join("Contents")
        .join("Resources")
        .join("hawk2ui-editor.toml");
    let editor_descriptor =
        std::fs::read_to_string(&editor_descriptor_path).expect("editor descriptor reads");
    assert!(editor_descriptor.contains("host_adapter = \"baseview\""));
    assert!(editor_descriptor.contains("renderer = \"skia\""));
    assert!(
        editor_descriptor
            .contains("runtime_artifact = \"Contents/Resources/hawk2ui-runtime-artifact.json\"")
    );

    let hashes =
        std::fs::read_to_string(&outputs[0].hash_manifest_path).expect("hash manifest reads");
    assert!(hashes.contains("Contents/Resources/hawk2ui-runtime-artifact.json"));
    assert!(hashes.contains("Contents/Resources/hawk2ui-editor.toml"));
    assert_eq!(
        plan.verify_materialized(&outputs).status(),
        VerificationStatus::Passed
    );
    std::fs::remove_file(runtime_artifact_path).expect("runtime artifact should be removable");
    assert_eq!(
        plan.verify_materialized(&outputs).status(),
        VerificationStatus::Failed
    );
}

#[test]
fn plugin_adapters_build_clap_entry_plan_from_clap_sys_contract() {
    let metadata = FormatMetadata::new("com.hawk2ui.clap", "Clap", "Hawk2UI")
        .version("1.0.0")
        .feature("audio-effect")
        .feature("utility");

    let entry = ClapPluginEntryPlan::from_metadata(&metadata);

    assert_eq!(entry.entry_symbol(), "clap_entry");
    assert_eq!(entry.factory_id(), "clap.plugin-factory");
    assert_eq!(entry.clap_version(), "1.2.2");
    assert_eq!(entry.plugin_id(), "com.hawk2ui.clap");
    assert_eq!(entry.features(), &["audio-effect", "utility"]);
}

#[test]
fn plugin_adapters_map_clap_gui_parent_handles_to_baseview_hosts() {
    let x11_parent = ClapGuiParentHandle::from_raw_parts(ClapGuiWindowApi::X11, 42)
        .expect("nonzero X11 handle maps");
    assert_eq!(x11_parent.api(), ClapGuiWindowApi::X11);
    assert_eq!(
        x11_parent
            .to_baseview_host_handle(Some(7))
            .expect("X11 parent with display maps to host handle"),
        hawk2ui_host::HostPlatformHandle::linux_x11(7, 42)
    );

    let windows_parent = ClapGuiParentHandle::from_raw_parts(ClapGuiWindowApi::Win32, 99)
        .expect("nonzero HWND maps");
    assert_eq!(
        windows_parent
            .to_baseview_host_handle(None)
            .expect("Windows parent maps directly"),
        hawk2ui_host::HostPlatformHandle::windows_hwnd(99)
    );

    let macos_parent = ClapGuiParentHandle::from_raw_parts(ClapGuiWindowApi::Cocoa, 123)
        .expect("nonzero NSView maps");
    assert_eq!(
        macos_parent
            .to_baseview_host_handle(None)
            .expect("macOS parent maps directly"),
        hawk2ui_host::HostPlatformHandle::macos_ns_view(123)
    );
}

#[test]
fn plugin_adapters_reject_invalid_clap_gui_parent_handles() {
    let zero = ClapGuiParentHandle::from_raw_parts(ClapGuiWindowApi::X11, 0)
        .expect_err("zero native parent handles must be rejected");
    assert_eq!(zero.rule(), "package.clap-gui-parent.invalid-handle");

    let missing_display = ClapGuiParentHandle::from_raw_parts(ClapGuiWindowApi::X11, 42)
        .expect("X11 handle maps")
        .to_baseview_host_handle(None)
        .expect_err("X11 Baseview attachment requires an explicit display handle");
    assert_eq!(
        missing_display.rule(),
        "package.clap-gui-parent.missing-display"
    );

    let wayland = ClapGuiParentHandle::from_raw_parts(ClapGuiWindowApi::Wayland, 42)
        .expect("CLAP Wayland handle is structurally valid")
        .to_baseview_host_handle(Some(7))
        .expect_err("Baseview cannot attach native Wayland parents");
    assert_eq!(wayland.rule(), "package.clap-gui-parent.unsupported-api");
}

#[test]
fn plugin_adapters_validate_clap_runtime_editor_descriptor() {
    let descriptor = ClapRuntimeEditorDescriptor::new(
        "Contents/Resources/hawk2ui-runtime-artifact.json",
        "baseview",
        "skia",
    )
    .expect("valid descriptor builds");

    assert_eq!(
        descriptor.to_export_payload(),
        "runtime_artifact=Contents/Resources/hawk2ui-runtime-artifact.json\nhost_adapter=baseview\nrenderer=skia\n"
    );
    assert_eq!(
        ClapRuntimeEditorDescriptor::new("", "baseview", "skia")
            .expect_err("empty runtime artifact path is rejected")
            .rule(),
        "package.clap-editor-descriptor.invalid-runtime-artifact"
    );
    assert_eq!(
        ClapRuntimeEditorDescriptor::new(
            "Contents/Resources/hawk2ui-runtime-artifact.json",
            "",
            "skia"
        )
        .expect_err("empty host adapter is rejected")
        .rule(),
        "package.clap-editor-descriptor.invalid-host-adapter"
    );
}

#[test]
fn plugin_adapters_generate_compilable_clap_cdylib_scaffold() {
    let metadata = FormatMetadata::new("com.hawk2ui.loadable", "Loadable", "Hawk2UI")
        .version("1.0.0")
        .feature("audio-effect");
    let editor = PluginEditor::custom("main", PluginEditorSize::new(1024.0, 640.0, 1.0));
    let parameters = ParameterModel::new([ParameterRecord::numeric(
        "gain",
        "Gain",
        "dB",
        ParameterRange::new(-60.0, 6.0, 0.0),
    )
    .flags(ParameterFlags::automatable())]);
    let output_root = std::env::temp_dir().join(format!(
        "hawk2ui-clap-cdylib-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ));

    let scaffold = ClapCdylibScaffold::from_metadata(&metadata)
        .with_editor(&editor)
        .with_parameters(&parameters)
        .with_runtime_editor_descriptor(
            ClapRuntimeEditorDescriptor::new(
                "Contents/Resources/hawk2ui-runtime-artifact.json",
                "baseview",
                "skia",
            )
            .expect("runtime editor descriptor builds"),
        );
    let output = scaffold
        .write_to(&output_root)
        .expect("CLAP scaffold should write");

    assert!(Path::new(&output.cargo_toml_path).is_file());
    assert!(Path::new(&output.lib_rs_path).is_file());
    let source = std::fs::read_to_string(&output.lib_rs_path).expect("generated source reads");
    assert!(source.contains("pub static clap_entry"));
    assert!(source.contains("clap_plugin_factory"));
    assert!(source.contains("clap_plugin_entry"));
    assert!(source.contains("get_plugin_descriptor"));
    assert!(source.contains("create_plugin"));
    assert!(source.contains("plugin_activate"));
    assert!(source.contains("plugin_process"));
    assert!(source.contains("clap_plugin_audio_ports"));
    assert!(source.contains("clap_plugin_gui"));
    assert!(source.contains("EDITOR_ATTACHED"));
    assert!(source.contains("clap_plugin_params"));
    assert!(source.contains("clap_plugin_state"));
    assert!(source.contains("PARAMETERS"));
    assert!(source.contains("Gain"));
    assert!(source.contains("hawk2ui_editor_descriptor"));
    assert!(source.contains("Contents/Resources/hawk2ui-runtime-artifact.json"));
    assert!(source.contains("host_adapter=baseview"));

    let target_dir = output_root.join("target");
    let status = std::process::Command::new("cargo")
        .arg("build")
        .arg("--release")
        .arg("--manifest-path")
        .arg(&output.cargo_toml_path)
        .env("CARGO_TARGET_DIR", &target_dir)
        .status()
        .expect("cargo build should launch for generated CLAP scaffold");
    assert!(status.success(), "generated CLAP scaffold should compile");

    let library_path = target_dir
        .join("release")
        .join(format!(
            "{}{}",
            std::env::consts::DLL_PREFIX,
            output.library_file_stem
        ))
        .with_extension(std::env::consts::DLL_EXTENSION);
    assert!(library_path.is_file());
    let library_bytes = std::fs::read(&library_path).expect("compiled CLAP library reads");
    assert!(
        library_bytes
            .windows("clap_entry".len())
            .any(|window| window == b"clap_entry")
    );
    assert!(
        library_bytes
            .windows("plugin-factory".len())
            .any(|window| window == b"plugin-factory")
    );
    assert!(
        library_bytes
            .windows("com.hawk2ui.loadable".len())
            .any(|window| window == b"com.hawk2ui.loadable")
    );

    let host_check_root = output_root.join("host-check");
    write_generated_clap_host_check(&host_check_root, &library_path);
    let host_target_dir = output_root.join("host-check-target");
    let status = std::process::Command::new("cargo")
        .arg("run")
        .arg("--release")
        .arg("--manifest-path")
        .arg(host_check_root.join("Cargo.toml"))
        .arg("--")
        .arg(&library_path)
        .env("CARGO_TARGET_DIR", &host_target_dir)
        .status()
        .expect("generated CLAP host check should launch");
    assert!(
        status.success(),
        "generated CLAP host check should load the compiled library"
    );
}

fn write_generated_clap_host_check(root: &Path, library_path: &Path) {
    assert!(
        library_path.is_file(),
        "host checker requires an already-built CLAP library"
    );
    std::fs::create_dir_all(root.join("src")).expect("host checker src directory writes");
    std::fs::write(
        root.join("Cargo.toml"),
        generated_clap_host_check_manifest(),
    )
    .expect("host checker manifest writes");
    std::fs::write(
        root.join("src").join("main.rs"),
        generated_clap_host_check_source(),
    )
    .expect("host checker source writes");
}

fn generated_clap_host_check_manifest() -> &'static str {
    r#"[package]
name = "hawk2ui-clap-host-check"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
clap-sys = "0.5.0"
libloading = "0.8.9"
"#
}

fn generated_clap_host_check_source() -> &'static str {
    r#"use std::{env, ffi::{c_void, CStr}, ptr};

fn main() {
    let library_path = env::args().nth(1).expect("library path argument");

    unsafe {
        let library = libloading::Library::new(library_path).expect("library loads");
        let entry_symbol: libloading::Symbol<*const clap_sys::entry::clap_plugin_entry> =
            library.get(b"clap_entry\0").expect("clap_entry resolves");
        let entry = &**entry_symbol;
        assert!((entry.init.expect("entry init"))(ptr::null()));

        let factory = (entry.get_factory.expect("factory"))(
            b"clap.plugin-factory\0".as_ptr().cast(),
        );
        assert!(!factory.is_null());
        let factory = &*(factory as *const clap_sys::factory::plugin_factory::clap_plugin_factory);
        assert_eq!((factory.get_plugin_count.expect("count"))(factory), 1);

        let descriptor = (factory.get_plugin_descriptor.expect("descriptor"))(factory, 0);
        assert!(!descriptor.is_null());
        let descriptor = &*descriptor;
        assert_eq!(
            CStr::from_ptr(descriptor.id).to_string_lossy(),
            "com.hawk2ui.loadable"
        );

        let plugin = (factory.create_plugin.expect("create"))(
            factory,
            ptr::null(),
            descriptor.id,
        );
        assert!(!plugin.is_null());
        assert_eq!((*plugin).desc, descriptor as *const _);

        assert!(((*plugin).init.expect("plugin init"))(plugin));
        assert!(((*plugin).activate.expect("activate"))(plugin, 48_000.0, 32, 1_024));
        assert!(((*plugin).start_processing.expect("start processing"))(plugin));
        let process = clap_sys::process::clap_process {
            steady_time: 0,
            frames_count: 0,
            transport: ptr::null(),
            audio_inputs: ptr::null(),
            audio_outputs: ptr::null_mut(),
            audio_inputs_count: 0,
            audio_outputs_count: 0,
            in_events: ptr::null(),
            out_events: ptr::null(),
        };
        assert_eq!(
            ((*plugin).process.expect("process"))(plugin, &process),
            clap_sys::process::CLAP_PROCESS_CONTINUE
        );
        ((*plugin).stop_processing.expect("stop processing"))(plugin);
        ((*plugin).deactivate.expect("deactivate"))(plugin);

        let audio_ports = ((*plugin).get_extension.expect("extension"))(
            plugin,
            b"clap.audio-ports\0".as_ptr().cast(),
        );
        assert!(!audio_ports.is_null());
        let audio_ports =
            &*(audio_ports as *const clap_sys::ext::audio_ports::clap_plugin_audio_ports);
        assert_eq!((audio_ports.count.expect("audio port count"))(plugin, true), 1);
        assert_eq!((audio_ports.count.expect("audio port count"))(plugin, false), 1);

        let gui = ((*plugin).get_extension.expect("gui extension"))(
            plugin,
            b"clap.gui\0".as_ptr().cast(),
        );
        assert!(!gui.is_null());
        let gui = &*(gui as *const clap_sys::ext::gui::clap_plugin_gui);
        let mut width = 0;
        let mut height = 0;
        assert!((gui.get_size.expect("gui size"))(plugin, &mut width, &mut height));
        assert_eq!((width, height), (1024, 640));
        let mut preferred_api = ptr::null();
        let mut is_floating = true;
        assert!((gui.get_preferred_api.expect("preferred gui api"))(
            plugin,
            &mut preferred_api,
            &mut is_floating,
        ));
        assert!(!preferred_api.is_null());
        assert!(!is_floating);
        assert!((gui.create.expect("gui create"))(plugin, preferred_api, false));
        assert!(!(gui.show.expect("gui show before parent"))(plugin));
        let parent = clap_sys::ext::gui::clap_window {
            api: preferred_api,
            specific: clap_sys::ext::gui::clap_window_handle {
                ptr: 0x1usize as *mut c_void,
            },
        };
        assert!((gui.set_parent.expect("gui set parent"))(plugin, &parent));
        assert!((gui.set_size.expect("gui set size"))(plugin, 1200, 720));
          assert!((gui.show.expect("gui show"))(plugin));
          assert!((gui.hide.expect("gui hide"))(plugin));
          (gui.destroy.expect("gui destroy"))(plugin);
          let editor_descriptor: libloading::Symbol<unsafe extern "C" fn(*mut usize) -> *const u8> =
              library.get(b"hawk2ui_editor_descriptor\0").expect("editor descriptor export resolves");
          let mut descriptor_len = 0usize;
          let descriptor_ptr = editor_descriptor(&mut descriptor_len);
          assert!(!descriptor_ptr.is_null());
          assert!(descriptor_len > 0);
          let descriptor = std::str::from_utf8(std::slice::from_raw_parts(
              descriptor_ptr,
              descriptor_len,
          ))
          .expect("editor descriptor is utf8");
          assert!(descriptor.contains("runtime_artifact=Contents/Resources/hawk2ui-runtime-artifact.json"));
          assert!(descriptor.contains("host_adapter=baseview"));
          assert!(descriptor.contains("renderer=skia"));

          let params = ((*plugin).get_extension.expect("params extension"))(
              plugin,
            b"clap.params\0".as_ptr().cast(),
        );
        assert!(!params.is_null());
        let params = &*(params as *const clap_sys::ext::params::clap_plugin_params);
        assert_eq!((params.count.expect("param count"))(plugin), 1);
        let mut info =
            std::mem::MaybeUninit::<clap_sys::ext::params::clap_param_info>::zeroed()
                .assume_init();
        assert!((params.get_info.expect("param info"))(plugin, 0, &mut info));
        assert_eq!(info.id, 1);
        assert_eq!(CStr::from_ptr(info.name.as_ptr()).to_string_lossy(), "Gain");
        assert_eq!(info.min_value, -60.0);
        assert_eq!(info.max_value, 6.0);
        assert_eq!(info.default_value, 0.0);
        let mut value = f64::NAN;
        assert!((params.get_value.expect("param value"))(plugin, 1, &mut value));
        assert_eq!(value, 0.0);

        let state = ((*plugin).get_extension.expect("state extension"))(
            plugin,
            b"clap.state\0".as_ptr().cast(),
        );
        assert!(!state.is_null());
        let state = &*(state as *const clap_sys::ext::state::clap_plugin_state);
        let mut saved = Vec::new();
        let ostream = clap_sys::stream::clap_ostream {
            ctx: (&mut saved as *mut Vec<u8>).cast(),
            write: Some(write_stream),
        };
        assert!((state.save.expect("state save"))(plugin, &ostream));
        assert_eq!(std::str::from_utf8(&saved).expect("state is utf8"), "hawk2ui-state-v1\n");
        let istream = clap_sys::stream::clap_istream {
            ctx: ptr::null_mut(),
            read: Some(empty_read),
        };
        assert!((state.load.expect("state load"))(plugin, &istream));
    }
}

unsafe extern "C" fn write_stream(
    stream: *const clap_sys::stream::clap_ostream,
    buffer: *const c_void,
    size: u64,
) -> i64 {
    let output = unsafe { &mut *((*stream).ctx as *mut Vec<u8>) };
    let bytes = unsafe { std::slice::from_raw_parts(buffer.cast::<u8>(), size as usize) };
    output.extend_from_slice(bytes);
    size as i64
}

unsafe extern "C" fn empty_read(
    _stream: *const clap_sys::stream::clap_istream,
    _buffer: *mut c_void,
    _size: u64,
) -> i64 {
    0
}
"#
}

#[test]
fn plugin_adapters_materialize_format_specific_layouts_and_hash_manifest() {
    let metadata = FormatMetadata::new("com.hawk2ui.layout", "Layout", "Hawk2UI").version("2.0.0");
    let output_root = std::env::temp_dir().join(format!(
        "hawk2ui-plugin-layouts-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ));
    let request = PackageRequest::new(
        metadata,
        BundleOutput::new(output_root.to_string_lossy(), "Layout"),
        ParameterModel::new([]),
    )
    .with_format(PackageFormat::Clap)
    .with_format(PackageFormat::Vst3)
    .with_format(PackageFormat::Au)
    .with_format(PackageFormat::Standalone);

    let plan = PackageAdapterSet::new()
        .plan(&request)
        .expect("package plan succeeds");
    let outputs = plan.materialize().expect("materialization succeeds");

    for output in &outputs {
        let root = Path::new(&output.output_path);
        assert!(
            root.join("Contents/Resources/hawk2ui-artifact.toml")
                .is_file()
        );
        assert!(
            root.join("Contents/Resources/hawk2ui-hashes.toml")
                .is_file()
        );
        let hashes = std::fs::read_to_string(root.join("Contents/Resources/hawk2ui-hashes.toml"))
            .expect("hash manifest reads");
        assert!(hashes.contains("algorithm = \"sha256\""));
        assert!(hashes.contains("hawk2ui-package.toml"));
        assert!(hashes.contains("Contents/Resources/hawk2ui-artifact.toml"));
        match output.format {
            PackageFormat::Clap => {
                assert!(root.join("Layout.clap").is_file());
                assert!(root.join("Contents/Resources/clap.json").is_file());
                assert!(hashes.contains("Contents/Resources/clap.json"));
            }
            PackageFormat::Vst3 => {
                assert!(root.join("Contents/Info.plist").is_file());
                assert!(root.join("Contents/x86_64-linux/Layout.vst3").is_file());
                assert!(hashes.contains("Contents/Info.plist"));
            }
            PackageFormat::Au => {
                assert!(root.join("Contents/Info.plist").is_file());
                assert!(root.join("Contents/MacOS/Layout").is_file());
                assert!(hashes.contains("Contents/MacOS/Layout"));
            }
            PackageFormat::Standalone => {
                assert!(root.join("Contents/Info.plist").is_file());
                assert!(root.join("Contents/MacOS/Layout").is_file());
                assert!(
                    root.join("Contents/Resources/hawk2ui-launch.toml")
                        .is_file()
                );
                assert!(hashes.contains("Contents/Resources/hawk2ui-launch.toml"));
            }
            PackageFormat::DesktopBundle | PackageFormat::SealedArtifact => {
                panic!("unexpected format in layout test");
            }
        }
    }
}

#[test]
fn plugin_adapters_materialize_removes_stale_output_payloads() {
    let metadata = FormatMetadata::new("com.hawk2ui.clean", "Clean", "Hawk2UI").version("1.0.0");
    let output_root = std::env::temp_dir().join(format!(
        "hawk2ui-plugin-clean-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ));
    let request = PackageRequest::new(
        metadata,
        BundleOutput::new(output_root.to_string_lossy(), "Clean"),
        ParameterModel::new([]),
    )
    .with_format(PackageFormat::Clap);

    let plan = PackageAdapterSet::new()
        .plan(&request)
        .expect("package plan succeeds");
    let outputs = plan
        .materialize()
        .expect("initial materialization succeeds");
    let stale_path = Path::new(&outputs[0].output_path).join("Contents/Resources/stale.bin");
    std::fs::write(&stale_path, "stale payload").expect("stale payload should be writable");

    let outputs = plan
        .materialize()
        .expect("repeat materialization should succeed");

    assert!(!stale_path.exists());
    assert_eq!(
        plan.verify_materialized(&outputs).status(),
        VerificationStatus::Passed
    );
}

#[test]
fn plugin_adapters_verify_materialized_rejects_tampered_package_payloads() {
    let metadata = FormatMetadata::new("com.hawk2ui.tamper", "Tamper", "Hawk2UI").version("3.0.0");
    let output_root = std::env::temp_dir().join(format!(
        "hawk2ui-plugin-tamper-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ));
    let request = PackageRequest::new(
        metadata,
        BundleOutput::new(output_root.to_string_lossy(), "Tamper"),
        ParameterModel::new([]),
    )
    .with_format(PackageFormat::Clap);

    let plan = PackageAdapterSet::new()
        .plan(&request)
        .expect("package plan succeeds");
    let outputs = plan.materialize().expect("materialization succeeds");
    assert_eq!(
        plan.verify_materialized(&outputs).status(),
        VerificationStatus::Passed
    );

    std::fs::write(
        Path::new(&outputs[0].output_path).join("Tamper.clap"),
        "tampered",
    )
    .expect("entry payload should be writable");

    assert_eq!(
        plan.verify_materialized(&outputs).status(),
        VerificationStatus::Failed
    );
}

#[test]
fn plugin_adapters_verify_materialized_rejects_incomplete_or_extra_hash_coverage() {
    let metadata =
        FormatMetadata::new("com.hawk2ui.coverage", "Coverage", "Hawk2UI").version("4.0.0");
    let output_root = std::env::temp_dir().join(format!(
        "hawk2ui-plugin-coverage-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ));
    let request = PackageRequest::new(
        metadata,
        BundleOutput::new(output_root.to_string_lossy(), "Coverage"),
        ParameterModel::new([]),
    )
    .with_format(PackageFormat::Clap);

    let plan = PackageAdapterSet::new()
        .plan(&request)
        .expect("package plan succeeds");
    let outputs = plan.materialize().expect("materialization succeeds");
    let output = &outputs[0];
    let original_hash_manifest =
        std::fs::read_to_string(&output.hash_manifest_path).expect("hash manifest reads");
    let incomplete_hash_manifest = original_hash_manifest
        .split("\n\n")
        .filter(|entry| !entry.contains("Contents/Resources/clap.json"))
        .collect::<Vec<_>>()
        .join("\n\n");
    std::fs::write(&output.hash_manifest_path, incomplete_hash_manifest)
        .expect("hash manifest should be writable");

    assert_eq!(
        plan.verify_materialized(&outputs).status(),
        VerificationStatus::Failed
    );

    std::fs::write(&output.hash_manifest_path, original_hash_manifest)
        .expect("hash manifest should be restorable");
    std::fs::write(
        Path::new(&output.output_path).join("Contents/Resources/injected.bin"),
        "unexpected payload",
    )
    .expect("extra payload should be writable");

    assert_eq!(
        plan.verify_materialized(&outputs).status(),
        VerificationStatus::Failed
    );
}

#[test]
fn plugin_adapters_escape_package_metadata_in_generated_descriptors() {
    let metadata = FormatMetadata::new("com.hawk2ui.escape", "Quote\"Name&<", "Hawk \"A&B\" <Co>")
        .category("audio \"effect\"")
        .feature("quoted \"feature\"");
    let output_root = std::env::temp_dir().join(format!(
        "hawk2ui-plugin-escape-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ));
    let request = PackageRequest::new(
        metadata,
        BundleOutput::new(output_root.to_string_lossy(), "Escape"),
        ParameterModel::new([]),
    )
    .with_format(PackageFormat::Clap)
    .with_format(PackageFormat::Standalone);

    let plan = PackageAdapterSet::new()
        .plan(&request)
        .expect("package plan succeeds");
    let outputs = plan.materialize().expect("materialization succeeds");

    let clap_root = outputs
        .iter()
        .find(|output| output.format == PackageFormat::Clap)
        .map(|output| Path::new(&output.output_path))
        .expect("clap output exists");
    let standalone_root = outputs
        .iter()
        .find(|output| output.format == PackageFormat::Standalone)
        .map(|output| Path::new(&output.output_path))
        .expect("standalone output exists");
    let package_manifest =
        std::fs::read_to_string(clap_root.join("hawk2ui-package.toml")).expect("manifest reads");
    let clap_manifest = std::fs::read_to_string(clap_root.join("Contents/Resources/clap.json"))
        .expect("clap manifest reads");
    let info_plist =
        std::fs::read_to_string(standalone_root.join("Contents/Info.plist")).expect("plist reads");
    let launch_manifest =
        std::fs::read_to_string(standalone_root.join("Contents/Resources/hawk2ui-launch.toml"))
            .expect("launch manifest reads");

    assert!(package_manifest.contains(r#"display_name = "Quote\"Name&<""#));
    assert!(package_manifest.contains(r#""quoted \"feature\"""#));
    assert!(clap_manifest.contains(r#""name": "Quote\"Name&<""#));
    assert!(clap_manifest.contains(r#""vendor": "Hawk \"A&B\" <Co>""#));
    assert!(info_plist.contains("Quote&quot;Name&amp;&lt;"));
    assert!(info_plist.contains("Hawk &quot;A&amp;B&quot; &lt;Co&gt;"));
    assert!(launch_manifest.contains(r#"entry = "Contents/MacOS/Quote\"Name&<""#));
}

#[test]
fn plugin_adapters_reject_invalid_package_metadata() {
    let request = PackageRequest::new(
        FormatMetadata::new("not-reverse-dns", "Demo", "Hawk2UI"),
        BundleOutput::new("dist", "Demo"),
        ParameterModel::new([]),
    )
    .with_format(PackageFormat::Clap);

    let error = PackageAdapterSet::new()
        .plan(&request)
        .expect_err("invalid metadata must fail");

    assert_eq!(error.diagnostics()[0].rule(), "package.metadata.invalid");
}

#[test]
fn plugin_adapters_reject_path_unsafe_metadata_names() {
    let request = PackageRequest::new(
        FormatMetadata::new("com.hawk2ui.unsafe", "../Escape", "Hawk2UI"),
        BundleOutput::new("dist", "SafeBundle"),
        ParameterModel::new([]),
    )
    .with_format(PackageFormat::Vst3);

    let error = PackageAdapterSet::new()
        .plan(&request)
        .expect_err("path-unsafe display names must fail before materialization");

    assert!(
        error
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.rule() == "package.display-name.invalid")
    );
}

#[test]
fn plugin_adapters_reject_reserved_bundle_names() {
    let request = PackageRequest::new(
        FormatMetadata::new("com.hawk2ui.reserved", "Reserved", "Hawk2UI"),
        BundleOutput::new("dist", "."),
        ParameterModel::new([]),
    )
    .with_format(PackageFormat::Clap);

    let error = PackageAdapterSet::new()
        .plan(&request)
        .expect_err("reserved bundle names must fail before materialization");

    assert!(
        error
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.rule() == "package.bundle-name.invalid")
    );
}

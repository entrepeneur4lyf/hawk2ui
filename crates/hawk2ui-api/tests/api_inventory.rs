use hawk2ui_api::{ApiInventory, ApiModule, ApiTypeAudience, ApiTypeEntry, ApiTypeStatus};

#[test]
fn api_inventory_classifies_only_real_public_production_contracts() {
    let inventory = ApiInventory::production_baseline();

    assert!(
        inventory
            .types_for_module(ApiModule::Diagnostic)
            .iter()
            .any(|ty| ty.name() == "Diagnostic" && ty.status() == ApiTypeStatus::Public)
    );
    assert!(
        inventory
            .types_for_module(ApiModule::Artifact)
            .iter()
            .any(|ty| ty.name() == "ArtifactSchemaVersion"
                && ty.audience() == ApiTypeAudience::Runtime)
    );
    assert!(
        inventory
            .types_for_module(ApiModule::Runtime)
            .iter()
            .any(|ty| ty.name() == "RuntimeJob" && ty.status() == ApiTypeStatus::Public)
    );
    assert!(
        inventory
            .types()
            .iter()
            .all(|ty| ty.status() == ApiTypeStatus::Public)
    );
    for phantom in [
        "ExperimentalScriptEngineContract",
        "ArtifactBuilderInternals",
        "SurfaceCompileFixture",
    ] {
        assert!(
            inventory.types().iter().all(|ty| ty.name() != phantom),
            "production inventory must not list phantom type {phantom}"
        );
    }
}

#[test]
fn api_inventory_exposes_only_documented_root_modules() {
    let inventory = ApiInventory::production_baseline();
    let modules = inventory.root_modules();

    assert_eq!(
        modules,
        [
            ApiModule::Artifact,
            ApiModule::Diagnostic,
            ApiModule::Plugin,
            ApiModule::Runtime,
            ApiModule::Surface,
        ]
    );
    assert!(
        modules
            .iter()
            .all(|module| !module.documentation().is_empty())
    );
}

#[test]
fn api_contract_inventory_includes_all_surface_runtime_and_plugin_contracts() {
    let inventory = ApiInventory::production_baseline();
    let names = inventory
        .types()
        .iter()
        .filter(|ty| ty.status() == ApiTypeStatus::Public)
        .map(ApiTypeEntry::name)
        .collect::<Vec<_>>();

    for required in [
        "InputEvent",
        "RepaintRequest",
        "FrameSchedule",
        "RuntimeJob",
        "RuntimeLifecycleHook",
        "BindingDirection",
        "PluginStateContract",
        "PluginPresetContract",
        "RealtimeDataContract",
        "RealtimeDataDirection",
    ] {
        assert!(
            names.contains(&required),
            "public API inventory is missing {required}"
        );
    }
}

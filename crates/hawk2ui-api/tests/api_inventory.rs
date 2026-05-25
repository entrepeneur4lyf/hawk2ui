use hawk2ui_api::{ApiInventory, ApiModule, ApiTypeAudience, ApiTypeStatus};

#[test]
fn api_inventory_classifies_public_internal_feature_gated_and_test_only_types() {
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
            .any(|ty| ty.status() == ApiTypeStatus::FeatureGated)
    );
    assert!(
        inventory
            .types()
            .iter()
            .any(|ty| ty.status() == ApiTypeStatus::TestOnly)
    );
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

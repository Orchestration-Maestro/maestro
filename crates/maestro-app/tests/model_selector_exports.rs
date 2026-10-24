//! Application paths retain their model-selection owners.
use crate::support::no_commands::no_commands;
use maestro_app::{
    AuthStorage, CatalogError, Model, ModelFileOperations, ModelRegistry, ProviderConfigInput,
    ProviderModelInput, ScopedModel, SettingsManager, SharedHandle, model_id_comparator,
    models_are_equal,
};

#[test]
fn application_exports_share_model_catalog_settings_and_locale_owners() {
    let _: Option<Model> = None;
    let _: Option<CatalogError> = None;
    let _: Option<Box<dyn ModelFileOperations>> = None;
    let _: Option<ProviderModelInput> = None;
    let _: Option<ScopedModel> = None;
    let _ = ProviderConfigInput::default();
    let auth = SharedHandle::new(AuthStorage::in_memory(std::iter::empty().collect()));
    let registry = ModelRegistry::in_memory(auth.clone(), no_commands()).unwrap();
    assert!(SharedHandle::ptr_eq(registry.auth_storage(), &auth));
    let model = registry.find("anthropic", "claude-sonnet-4-5").unwrap();
    assert!(std::sync::Arc::ptr_eq(
        &model,
        &registry.find("anthropic", "claude-sonnet-4-5").unwrap()
    ));
    assert!(models_are_equal(
        Some(&model.read().unwrap()),
        Some(&model.read().unwrap())
    ));
    let mut settings = SettingsManager::in_memory(serde_json::from_str("{}").unwrap());
    settings.set_default_model_and_provider("anthropic".into(), "claude-sonnet-4-5".into());
    assert_eq!(
        settings.get_default_provider().as_deref(),
        Some("anthropic")
    );
    assert_eq!(
        settings.get_default_model().as_deref(),
        Some("claude-sonnet-4-5")
    );
    let compare =
        model_id_comparator(&|name| (name == "LC_ALL").then(|| "en_US.UTF-8".into())).unwrap();
    assert_eq!(compare("a", "ä"), std::cmp::Ordering::Less);
    assert_eq!(compare("p10", "p2"), std::cmp::Ordering::Less);
}

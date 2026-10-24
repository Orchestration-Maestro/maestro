//! Application paths retain their model-selection owners.
use crate::support::no_commands::no_commands;
use maestro_app::{
    AuthStorage, CatalogError, Model, ModelFileOperations, ModelRegistry, ModelResolverOutput,
    ProviderConfigInput, ProviderModelInput, ScopedModel, SettingsManager, SharedHandle,
    ThinkingLevel, find_exact_model_reference_match, model_id_comparator, models_are_equal,
    resolve_model_scope,
};
use std::sync::Arc;

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

#[test]
fn model_frontend_exports_reuse_catalog_owners() {
    let auth = SharedHandle::new(AuthStorage::in_memory(std::iter::empty().collect()));
    auth.set_runtime_api_key("anthropic", "fixture");
    let registry = ModelRegistry::in_memory(auth, no_commands()).unwrap();
    let available = registry.get_available().unwrap();
    let sonnet = registry.find("anthropic", "claude-sonnet-4-5").unwrap();
    let found = find_exact_model_reference_match(" ANTHROPIC / claude-sonnet-4-5 ", &available);
    assert!(Arc::ptr_eq(&found.unwrap(), &sonnet));
    assert!(find_exact_model_reference_match("claude-sonnet", &available).is_none());
    let compare = model_id_comparator(&|_| None).unwrap();
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let patterns = ["anthropic/claude-sonnet-4-5:high", "missing/none"].map(String::from);
    let scope = resolve_model_scope(
        &patterns,
        &registry,
        &compare,
        &mut ModelResolverOutput {
            stdout: &mut stdout,
            stderr: &mut stderr,
            color: false,
        },
    )
    .unwrap();
    let [
        ScopedModel {
            model,
            thinking_level,
        },
    ] = <[_; 1]>::try_from(scope).ok().unwrap();
    assert!(Arc::ptr_eq(&model, &sonnet));
    assert_eq!(thinking_level, Some(ThinkingLevel::High));
    assert!(stdout.is_empty());
    assert_eq!(
        String::from_utf8(stderr).unwrap(),
        "Warning: No models match pattern \"missing/none\"\n"
    );
}

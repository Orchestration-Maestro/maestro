//! Application paths retain their model-selection owners.
use crate::support::no_commands::no_commands;
use maestro_app::{
    AuthStorage, BUILT_IN_PROVIDER_DISPLAY_NAMES, CatalogError, DEFAULT_MODEL_PER_PROVIDER, Model,
    ModelFileOperations, ModelRegistry, OAuthSelectOption, OAuthSelectPrompt, ProviderConfigInput,
    ProviderModelInput, ScopedModel, SettingsManager, SharedHandle, get_providers,
    model_id_comparator, models_are_equal,
};
use serde::Deserialize;

/// Recorded provider metadata in embedded order.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthenticationMetadata {
    providers: Vec<String>,
    display_names: Vec<(String, String)>,
    defaults: Vec<(String, String)>,
}

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
fn authentication_frontend_exports_share_existing_owners() {
    let expected: AuthenticationMetadata =
        serde_json::from_str(include_str!("fixtures/authentication_metadata.json")).unwrap();
    assert_eq!(get_providers(), expected.providers);
    let names: Vec<(String, String)> = BUILT_IN_PROVIDER_DISPLAY_NAMES
        .read()
        .unwrap()
        .iter()
        .map(|(provider, name)| (provider.clone(), name.clone()))
        .collect();
    assert_eq!(names, expected.display_names);
    let defaults: Vec<(String, String)> = DEFAULT_MODEL_PER_PROVIDER
        .read()
        .unwrap()
        .iter()
        .map(|(provider, id)| (provider.clone(), id.clone()))
        .collect();
    assert_eq!(defaults, expected.defaults);
    assert_display_names_are_shared_with_the_registry();
    assert_supplied_prompt_retains_option_order();
}

/// Write through the exported table and read the change back from its catalog reader.
fn assert_display_names_are_shared_with_the_registry() {
    let auth = SharedHandle::new(AuthStorage::in_memory(std::iter::empty().collect()));
    let registry = ModelRegistry::in_memory(auth, no_commands()).unwrap();
    assert_eq!(registry.get_provider_display_name("mistral"), "Mistral");
    BUILT_IN_PROVIDER_DISPLAY_NAMES
        .write()
        .unwrap()
        .insert("mistral".into(), "Mistral AI".into());
    assert_eq!(registry.get_provider_display_name("mistral"), "Mistral AI");
    BUILT_IN_PROVIDER_DISPLAY_NAMES
        .write()
        .unwrap()
        .insert("mistral".into(), "Mistral".into());
}

fn assert_supplied_prompt_retains_option_order() {
    let prompt = OAuthSelectPrompt {
        message: "Choose a workspace".into(),
        options: vec![
            OAuthSelectOption {
                id: "team".into(),
                label: "Team".into(),
            },
            OAuthSelectOption {
                id: "team".into(),
                label: "Team again".into(),
            },
            OAuthSelectOption {
                id: "personal".into(),
                label: "Team".into(),
            },
            OAuthSelectOption {
                id: "unnamed".into(),
                label: String::new(),
            },
            OAuthSelectOption {
                id: "intl".into(),
                label: "Ünïcøde 說明".into(),
            },
        ],
    };
    assert_eq!(prompt.message, "Choose a workspace");
    let supplied: Vec<(&str, &str)> = prompt
        .options
        .iter()
        .map(|option| (option.id.as_str(), option.label.as_str()))
        .collect();
    assert_eq!(
        supplied,
        [
            ("team", "Team"),
            ("team", "Team again"),
            ("personal", "Team"),
            ("unnamed", ""),
            ("intl", "Ünïcøde 說明"),
        ]
    );
}

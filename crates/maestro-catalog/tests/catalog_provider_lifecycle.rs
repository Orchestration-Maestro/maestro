#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
//! Dynamic provider retention and catalog replay.
use crate::dynamic;
use crate::support;
use maestro_catalog::{ModelRegistry, ProviderConfigInput};
use maestro_models::SharedHandle;
use std::sync::{Arc, RwLock};

#[test]
fn upsert_mutates_first_retained_config_and_keeps_defined_empty_values() {
    if dynamic::isolated("upsert_mutates_first_retained_config_and_keeps_defined_empty_values") {
        return;
    }
    let registry = ModelRegistry::in_memory(support::auth(), crate::support::values()).unwrap();
    let first = dynamic::config(&["m"]);
    let weak = SharedHandle::downgrade(&first);
    registry.register_provider("p", first.clone()).unwrap();
    let second = SharedHandle::new(RwLock::new(ProviderConfigInput {
        name: Some(String::new()),
        base_url: Some(String::new()),
        models: Some(vec![]),
        auth_header: Some(false),
        ..Default::default()
    }));
    registry.register_provider("p", second.clone()).unwrap();
    {
        let retained = first.read().unwrap();
        assert_eq!(retained.name.as_deref(), Some(""));
        assert_eq!(retained.base_url.as_deref(), Some(""));
        assert_eq!(retained.auth_header, Some(false));
        assert!(retained.models.as_ref().unwrap().is_empty());
        assert!(retained.api.is_some());
    }
    assert!(second.read().unwrap().api.is_none());
    registry.register_provider("p", first.clone()).unwrap();
    drop(first);
    assert!(weak.upgrade().is_some());
    registry.unregister_provider("p").unwrap();
    assert!(weak.upgrade().is_none());
}
#[test]
fn empty_retained_models_restore_underlying_catalog_on_refresh() {
    if dynamic::isolated("empty_retained_models_restore_underlying_catalog_on_refresh") {
        return;
    }
    let registry = ModelRegistry::in_memory(support::auth(), crate::support::values()).unwrap();
    registry
        .register_provider("p", dynamic::config(&["m"]))
        .unwrap();
    registry.refresh().unwrap();
    assert!(registry.find("p", "m").is_some());
    registry
        .register_provider(
            "p",
            SharedHandle::new(RwLock::new(ProviderConfigInput {
                models: Some(vec![]),
                ..Default::default()
            })),
        )
        .unwrap();
    assert!(registry.find("p", "m").is_some());
    registry.refresh().unwrap();
    assert!(registry.find("p", "m").is_none());
}
#[test]
fn display_name_precedence_keeps_present_empty_names() {
    if dynamic::isolated("display_name_precedence_keeps_present_empty_names") {
        return;
    }
    let registry = ModelRegistry::in_memory(support::auth(), crate::support::values()).unwrap();
    assert_eq!(registry.get_provider_display_name("unlisted"), "unlisted");
    assert_eq!(
        registry.get_provider_display_name("anthropic"),
        "Anthropic (Claude Pro/Max)"
    );
    let config = SharedHandle::new(RwLock::new(ProviderConfigInput {
        name: Some(String::new()),
        ..Default::default()
    }));
    registry.register_provider("anthropic", config).unwrap();
    assert_eq!(registry.get_provider_display_name("anthropic"), "");
    registry
        .register_provider(
            "unlisted",
            SharedHandle::new(RwLock::new(ProviderConfigInput {
                name: Some("Registered".into()),
                ..Default::default()
            })),
        )
        .unwrap();
    assert_eq!(registry.get_provider_display_name("unlisted"), "Registered");
}

#[test]
fn construction_preserves_global_registries_but_refresh_resets_other_instances() {
    if dynamic::isolated(
        "construction_preserves_global_registries_but_refresh_resets_other_instances",
    ) {
        return;
    }
    let first = ModelRegistry::in_memory(support::auth(), crate::support::values()).unwrap();
    let config = dynamic::config(&["m"]);
    {
        let mut value = config.write().unwrap();
        value.name = Some("P".into());
        value.api = Some("instance-api".into());
        value.callbacks.oauth = Some(dynamic::oauth("Dynamic OAuth"));
    }
    dynamic::callback(&config, "first");
    first.register_provider("p", config).unwrap();
    let second = ModelRegistry::in_memory(support::auth(), crate::support::values()).unwrap();
    assert!(maestro_models::get_api_provider("instance-api").is_some());
    assert!(maestro_models::get_oauth_provider("p").is_some());
    assert_eq!(second.get_provider_display_name("p"), "Dynamic OAuth");
    second.refresh().unwrap();
    assert!(maestro_models::get_api_provider("instance-api").is_none());
    assert!(maestro_models::get_oauth_provider("p").is_none());
    assert_eq!(first.get_provider_display_name("p"), "P");
    assert_eq!(second.get_provider_display_name("p"), "p");
    first.refresh().unwrap();
    assert_eq!(dynamic::failure(&first, "p", "m"), "first");
    assert!(maestro_models::get_oauth_provider("p").is_some());
}

#[test]
fn refresh_replays_initial_provider_order_and_defined_field_merges() {
    if dynamic::isolated("refresh_replays_initial_provider_order_and_defined_field_merges") {
        return;
    }
    let registry = ModelRegistry::in_memory(support::auth(), crate::support::values()).unwrap();
    let p = dynamic::config(&["old"]);
    dynamic::callback(&p, "p-first");
    registry.register_provider("p", p).unwrap();
    let q = dynamic::config(&["other"]);
    dynamic::callback(&q, "q");
    registry.register_provider("q", q).unwrap();
    let p = SharedHandle::new(RwLock::new(ProviderConfigInput {
        api: Some("openai-completions".into()),
        base_url: Some("https://latest.invalid".into()),
        ..Default::default()
    }));
    dynamic::callback(&p, "p-last");
    registry.register_provider("p", p).unwrap();
    assert_eq!(dynamic::failure(&registry, "p", "old"), "p-last");
    registry.refresh().unwrap();
    assert_eq!(dynamic::failure(&registry, "p", "old"), "q");
    assert_eq!(
        dynamic::selected(&registry, &["p", "q"]),
        [
            ("p".into(), "old".into(), "https://latest.invalid".into()),
            (
                "q".into(),
                "other".into(),
                "https://dynamic.invalid/v1".into()
            )
        ]
    );
    registry.unregister_provider("q").unwrap();
    assert_eq!(dynamic::failure(&registry, "p", "old"), "p-last");
    registry
        .register_provider("q", dynamic::config(&["again"]))
        .unwrap();
    registry.unregister_provider("p").unwrap();
    registry
        .register_provider("p", dynamic::config(&["new"]))
        .unwrap();
    registry.refresh().unwrap();
    assert_eq!(
        dynamic::selected(&registry, &["p", "q"])
            .iter()
            .map(|m| (m.0.as_str(), m.1.as_str()))
            .collect::<Vec<_>>(),
        [("q", "again"), ("p", "new")]
    );
}

#[test]
fn registered_oauth_label_precedes_current_global_provider() {
    if dynamic::isolated("registered_oauth_label_precedes_current_global_provider") {
        return;
    }
    let registry = ModelRegistry::in_memory(support::auth(), crate::support::values()).unwrap();
    for name in ["Retained OAuth", ""] {
        registry
            .register_provider(
                "p",
                SharedHandle::new(RwLock::new(ProviderConfigInput {
                    callbacks: maestro_catalog::ProviderCallbacks {
                        oauth: Some(dynamic::oauth(name)),
                        ..Default::default()
                    },
                    ..Default::default()
                })),
            )
            .unwrap();
        maestro_models::register_oauth_provider(Arc::new(dynamic::OAuth {
            calls: None,
            id: "p",
            name: "Other Instance",
            transform: Arc::new(|models| Ok(models)),
        }));
        assert_eq!(registry.get_provider_display_name("p"), name);
        assert_eq!(
            maestro_models::get_oauth_provider("p").unwrap().name(),
            "Other Instance"
        );
    }
    registry.unregister_provider("p").unwrap();
    assert_eq!(registry.get_provider_display_name("p"), "p");
}

#[test]
fn known_removal_restores_file_overrides_and_builtin_descriptors() {
    if dynamic::isolated("known_removal_restores_file_overrides_and_builtin_descriptors") {
        return;
    }
    let registry =
        support::registry(r#"{"providers":{"anthropic":{"baseUrl":"https://file.invalid"}}}"#);
    let original = maestro_models::get_models("anthropic");
    registry
        .register_provider("anthropic", dynamic::config(&["replacement"]))
        .unwrap();
    let old = registry.find("anthropic", "replacement").unwrap();
    registry.unregister_provider("anthropic").unwrap();
    assert!(registry.find("anthropic", "replacement").is_none());
    assert_eq!(old.read().unwrap().id, "replacement");
    let restored = dynamic::selected(&registry, &["anthropic"]);
    assert_eq!(
        restored.iter().map(|m| m.1.as_str()).collect::<Vec<_>>(),
        original.iter().map(|m| m.id.as_str()).collect::<Vec<_>>()
    );
    assert!(restored.iter().all(|m| m.2 == "https://file.invalid"));
}

#[test]
fn unknown_removal_does_not_reload_or_reset_shared_registries() {
    if dynamic::isolated("unknown_removal_does_not_reload_or_reset_shared_registries") {
        return;
    }
    let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let registry = ModelRegistry::with_operations(
        support::auth(),
        "counted",
        dynamic::CountFile(reads.clone()),
        crate::support::values(),
    )
    .unwrap();
    let config = dynamic::config(&["m"]);
    config.write().unwrap().callbacks.oauth = Some(dynamic::oauth("retained"));
    dynamic::callback(&config, "retained");
    registry.register_provider("p", config).unwrap();
    let list = registry.get_all();
    let api = maestro_models::get_api_provider("openai-completions").unwrap();
    let oauth = maestro_models::get_oauth_provider("p").unwrap();
    registry.unregister_provider("unknown").unwrap();
    assert!(Arc::ptr_eq(&list, &registry.get_all()));
    assert!(Arc::ptr_eq(
        &api.stream,
        &maestro_models::get_api_provider("openai-completions")
            .unwrap()
            .stream
    ));
    assert!(Arc::ptr_eq(
        &oauth,
        &maestro_models::get_oauth_provider("p").unwrap()
    ));
    assert_eq!(registry.get_provider_display_name("p"), "retained");
    assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
fn immediate_registration_uses_incoming_config_not_merged_config() {
    if dynamic::isolated("immediate_registration_uses_incoming_config_not_merged_config") {
        return;
    }
    let auth = support::auth();
    auth.set(
        "p",
        maestro_credentials::AuthCredential::OAuth(dynamic::tokens()),
    );
    let registry = ModelRegistry::in_memory(auth, crate::support::values()).unwrap();
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = calls.clone();
    let config = dynamic::config(&["first"]);
    config.write().unwrap().callbacks.oauth = Some(Arc::new(dynamic::OAuth {
        calls: None,
        id: "ignored",
        name: "Transform",
        transform: Arc::new(move |mut models| {
            observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            models
                .iter_mut()
                .find(|m| m.provider == "p")
                .unwrap()
                .to_mut()
                .name = "transformed".into();
            Ok(models)
        }),
    }));
    registry.register_provider("p", config).unwrap();
    registry
        .register_provider("p", dynamic::config(&["second"]))
        .unwrap();
    assert_eq!(
        registry.find("p", "second").unwrap().read().unwrap().name,
        "Name second"
    );
    registry
        .register_provider(
            "p",
            SharedHandle::new(RwLock::new(ProviderConfigInput {
                base_url: Some("https://override.invalid".into()),
                ..Default::default()
            })),
        )
        .unwrap();
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    registry.refresh().unwrap();
    let model = registry.find("p", "second").unwrap();
    let model = model.read().unwrap();
    assert_eq!(model.name, "transformed");
    assert_eq!(model.base_url, "https://override.invalid");
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
}

#[test]
fn failed_transform_keeps_applied_effects_without_upserting_config() {
    if dynamic::isolated("failed_transform_keeps_applied_effects_without_upserting_config") {
        return;
    }
    let auth = support::auth();
    auth.set(
        "p",
        maestro_credentials::AuthCredential::OAuth(dynamic::tokens()),
    );
    let registry = ModelRegistry::in_memory(auth, crate::support::values()).unwrap();
    let retained = dynamic::config(&["old"]);
    retained.write().unwrap().name = Some("retained".into());
    registry.register_provider("p", retained).unwrap();
    let config = dynamic::config(&["new"]);
    dynamic::callback(&config, "provider callback failed");
    {
        let mut config = config.write().unwrap();
        config.name = Some("failed".into());
        config.headers = Some([("X".into(), "new".into())].into());
        config.callbacks.oauth = Some(Arc::new(dynamic::OAuth {
            calls: None,
            id: "ignored",
            name: "Dynamic OAuth",
            transform: Arc::new(|_| Err(dynamic::account_failure("transform failed"))),
        }));
    }
    let error: maestro_catalog::CatalogError = registry.register_provider("p", config).unwrap_err();
    dynamic::assert_account_failure(&error, "transform failed");
    assert_eq!(registry.get_provider_display_name("p"), "retained");
    assert!(registry.find("p", "new").is_some());
    assert!(registry.find("p", "old").is_none());
    assert_eq!(
        dynamic::failure(&registry, "p", "new"),
        "provider callback failed"
    );
    assert_eq!(
        maestro_models::get_oauth_provider("p").unwrap().name(),
        "Dynamic OAuth"
    );
    request_support::success(
        request_support::run(registry.get_api_key_and_headers(&registry.find("p", "new").unwrap())),
        Some("unused-access"),
        &[("X", "new")],
    );
    registry.refresh().unwrap();
    assert!(registry.find("p", "old").is_some());
    assert!(registry.find("p", "new").is_none());
    assert!(maestro_models::get_oauth_provider("p").is_none());
}

#[test]
fn refresh_stops_after_first_replay_failure_with_partial_publication() {
    if dynamic::isolated("refresh_stops_after_first_replay_failure_with_partial_publication") {
        return;
    }
    let auth = support::auth();
    for id in ["p", "q", "r"] {
        auth.set(
            id,
            maestro_credentials::AuthCredential::OAuth(dynamic::tokens()),
        );
    }
    let registry = ModelRegistry::in_memory(auth.clone(), crate::support::values()).unwrap();
    let mut aliases = Vec::new();
    let fail = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
    for id in ["p", "q", "r"] {
        let config = dynamic::config(&[id]);
        config.write().unwrap().api_key = Some(id.into());
        config.write().unwrap().headers = Some([("X".into(), id.into())].into());
        let fail = fail.clone();
        let calls = calls.clone();
        config.write().unwrap().callbacks.oauth = Some(Arc::new(dynamic::OAuth {
            calls: None,
            id: "ignored",
            name: id,
            transform: Arc::new(move |models| {
                calls.lock().unwrap().push(id);
                if id == "q" && fail.load(std::sync::atomic::Ordering::SeqCst) {
                    Err(dynamic::account_failure("replay q failed"))
                } else {
                    Ok(models)
                }
            }),
        }));
        registry.register_provider(id, config).unwrap();
        aliases.push(registry.find(id, id).unwrap());
    }
    calls.lock().unwrap().clear();
    fail.store(true, std::sync::atomic::Ordering::SeqCst);
    let error: maestro_catalog::CatalogError = registry.refresh().unwrap_err();
    dynamic::assert_account_failure(&error, "replay q failed");
    assert_eq!(*calls.lock().unwrap(), ["p", "q"]);
    assert_eq!(
        dynamic::selected(&registry, &["p", "q", "r"])
            .iter()
            .map(|m| m.0.as_str())
            .collect::<Vec<_>>(),
        ["p", "q"]
    );
    assert!(maestro_models::get_oauth_provider("p").is_some());
    assert!(maestro_models::get_oauth_provider("q").is_some());
    assert!(maestro_models::get_oauth_provider("r").is_none());
    for id in ["p", "q", "r"] {
        auth.remove(id);
    }
    assert_partial_request_sources(&registry, &aliases);
}

/// Resolve retained descriptors after the replay stopped, independently of stored tokens.
fn assert_partial_request_sources(
    registry: &ModelRegistry,
    aliases: &[Arc<RwLock<maestro_models::Model>>],
) {
    for (model, value) in aliases.iter().zip(["p", "q", "r"]) {
        let headers = [("X", value)];
        request_support::success(
            request_support::run(registry.get_api_key_and_headers(model)),
            if value == "r" { None } else { Some(value) },
            if value == "r" { &[] } else { &headers },
        );
    }
}
#[test]
fn known_removal_stays_removed_when_survivor_replay_fails() {
    if dynamic::isolated("known_removal_stays_removed_when_survivor_replay_fails") {
        return;
    }
    let auth = support::auth();
    auth.set(
        "p",
        maestro_credentials::AuthCredential::OAuth(dynamic::tokens()),
    );
    let registry = ModelRegistry::in_memory(auth, crate::support::values()).unwrap();
    let fail = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let observed = fail.clone();
    let config = dynamic::config(&["m"]);
    config.write().unwrap().callbacks.oauth = Some(Arc::new(dynamic::OAuth {
        calls: None,
        id: "ignored",
        name: "Survivor",
        transform: Arc::new(move |models| {
            if observed.load(std::sync::atomic::Ordering::SeqCst) {
                Err(dynamic::account_failure("retained failure"))
            } else {
                Ok(models)
            }
        }),
    }));
    registry.register_provider("p", config).unwrap();
    registry
        .register_provider("q", dynamic::config(&["removed"]))
        .unwrap();
    fail.store(true, std::sync::atomic::Ordering::SeqCst);
    let error: maestro_catalog::CatalogError = registry.unregister_provider("q").unwrap_err();
    dynamic::assert_account_failure(&error, "retained failure");
    assert!(registry.find("p", "m").is_some());
    assert!(registry.find("q", "removed").is_none());
    fail.store(false, std::sync::atomic::Ordering::SeqCst);
    registry.refresh().unwrap();
    assert!(registry.find("q", "removed").is_none());
}

#[test]
fn removal_restores_builtin_oauth_and_deletes_custom_provider() {
    if dynamic::isolated("removal_restores_builtin_oauth_and_deletes_custom_provider") {
        return;
    }
    let registry = ModelRegistry::in_memory(support::auth(), crate::support::values()).unwrap();
    for id in ["anthropic", "custom", "surviving"] {
        registry
            .register_provider(
                id,
                SharedHandle::new(RwLock::new(ProviderConfigInput {
                    callbacks: maestro_catalog::ProviderCallbacks {
                        oauth: Some(dynamic::oauth("Override")),
                        ..Default::default()
                    },
                    ..Default::default()
                })),
            )
            .unwrap();
    }
    registry.unregister_provider("anthropic").unwrap();
    registry.unregister_provider("custom").unwrap();
    let restored = maestro_models::get_oauth_provider("anthropic").unwrap();
    assert_eq!(restored.id(), "anthropic");
    assert_eq!(restored.name(), "Anthropic (Claude Pro/Max)");
    let tokens = dynamic::tokens();
    assert!(restored.get_api_key(&tokens).unwrap().eq(&tokens.access));
    assert!(maestro_models::get_oauth_provider("custom").is_none());
    assert_eq!(
        maestro_models::get_oauth_provider("surviving")
            .unwrap()
            .name(),
        "Override"
    );
}
use crate::request_support;

#[test]
fn removal_keeps_literal_pair_header_identity() {
    if dynamic::isolated("removal_keeps_literal_pair_header_identity") {
        return;
    }
    let registry = ModelRegistry::in_memory(support::auth(), crate::support::values()).unwrap();
    let mut aliases = Vec::new();
    for (provider, id, header) in [("a:b", "c", "left"), ("a", "b:c", "right")] {
        let config = dynamic::config(&[id]);
        config.write().unwrap().models.as_mut().unwrap()[0].headers =
            Some([("X".into(), header.into())].into());
        registry.register_provider(provider, config).unwrap();
        let alias = registry.find(provider, id).unwrap();
        request_support::success(
            request_support::run(registry.get_api_key_and_headers(&alias)),
            Some("configured"),
            &[("X", header)],
        );
        aliases.push(alias);
    }
    registry.unregister_provider("a").unwrap();
    request_support::success(
        request_support::run(registry.get_api_key_and_headers(&aliases[0])),
        Some("configured"),
        &[("X", "left")],
    );
    request_support::success(
        request_support::run(registry.get_api_key_and_headers(&aliases[1])),
        None,
        &[],
    );
}

#[test]
fn refresh_clears_file_error_and_request_maps_before_replaying() {
    if dynamic::isolated("refresh_clears_file_error_and_request_maps_before_replaying") {
        return;
    }
    let text = support::Text(Arc::new(std::sync::Mutex::new(Some(r#"{"providers":{"p":{"baseUrl":"file","api":"test","apiKey":"file-key","headers":{"F":"file"},"models":[{"id":"retired"}]}}}"#.into()))));
    let registry = support::text_registry("models-input", &text, crate::support::values());
    let old = registry.find("p", "retired").unwrap();
    let config = dynamic::config(&["m"]);
    config.write().unwrap().headers = Some([("D".into(), "dynamic".into())].into());
    registry.register_provider("p", config).unwrap();
    for malformed in [true, false] {
        *text.0.lock().unwrap() = if malformed { Some("{".into()) } else { None };
        registry.refresh().unwrap();
        assert_eq!(registry.get_error().is_some(), malformed);
        assert!(registry.find("p", "retired").is_none());
        assert!(registry.find("p", "m").is_some());
        for model in [&old, &registry.find("p", "m").unwrap()] {
            request_support::success(
                request_support::run(registry.get_api_key_and_headers(model)),
                Some("configured"),
                &[("D", "dynamic")],
            );
        }
    }
}

#[test]
fn provider_overrides_and_replacements_survive_refresh() {
    if dynamic::isolated("provider_overrides_and_replacements_survive_refresh") {
        return;
    }
    for case in 0..6 {
        let registry = ModelRegistry::in_memory(support::auth(), crate::support::values()).unwrap();
        let provider = if case < 3 { "anthropic" } else { "p" };
        let ids = if case == 0 {
            maestro_models::get_models(provider)
                .iter()
                .map(|m| m.id.clone())
                .collect::<Vec<_>>()
        } else if case < 3 {
            vec!["custom-claude".into()]
        } else {
            vec!["custom-a".into(), "custom-b".into()]
        };
        if case != 0 {
            registry
                .register_provider(
                    provider,
                    dynamic::config(&ids.iter().map(String::as_str).collect::<Vec<_>>()),
                )
                .unwrap();
        }
        if [0, 2, 4, 5].contains(&case) {
            let config = SharedHandle::new(RwLock::new(ProviderConfigInput {
                base_url: (case != 5).then(|| "https://proxy.invalid".into()),
                headers: (case == 5).then(|| [("x-proxy".into(), "enabled".into())].into()),
                ..Default::default()
            }));
            registry.register_provider(provider, config).unwrap();
        }
        registry.refresh().unwrap();
        let selected = dynamic::selected(&registry, &[provider]);
        assert_eq!(
            selected.iter().map(|m| &m.1).collect::<Vec<_>>(),
            ids.iter().collect::<Vec<_>>()
        );
        assert!(selected.iter().all(|m| m.2
            == if [0, 2, 4].contains(&case) {
                "https://proxy.invalid"
            } else {
                "https://dynamic.invalid/v1"
            }));
        let model = registry.find(provider, &ids[0]).unwrap();
        request_support::success(
            request_support::run(registry.get_api_key_and_headers(&model)),
            (case != 0).then_some("configured"),
            if case == 5 {
                &[("x-proxy", "enabled")]
            } else {
                &[]
            },
        );
    }
}

#[test]
fn removal_restores_a_callable_builtin_handler_and_keeps_old_callback_handle() {
    if dynamic::isolated(
        "removal_restores_a_callable_builtin_handler_and_keeps_old_callback_handle",
    ) {
        return;
    }
    for api in ["openai-completions", "anthropic-messages"] {
        let registry = ModelRegistry::in_memory(support::auth(), crate::support::values()).unwrap();
        let config = dynamic::config(&["override"]);
        config.write().unwrap().api = Some(api.into());
        dynamic::callback(&config, "override");
        registry.register_provider("p", config).unwrap();
        let old = maestro_models::get_api_provider(api).unwrap();
        let old_model = registry
            .find("p", "override")
            .unwrap()
            .read()
            .unwrap()
            .clone();
        registry.unregister_provider("p").unwrap();
        let restored = maestro_models::get_api_provider(api).unwrap();
        assert!(!Arc::ptr_eq(&old.stream, &restored.stream));
        let model = maestro_models::get_providers()
            .iter()
            .flat_map(|p| maestro_models::get_models(p))
            .find(|m| m.api == api)
            .unwrap();
        assert_callable(&restored, model);
        let failure = old
            .stream
            .call(
                old_model,
                maestro_models::Context {
                    messages: vec![],
                    tools: None,
                    system_prompt: None,
                },
                maestro_models::ProviderInvocation::Threaded(None, None),
            )
            .err()
            .unwrap();
        assert_eq!(failure.message, "override");
    }
}

/// Observe completion through the restored protocol with no network effects.
fn assert_callable(restored: &maestro_models::ApiProvider, model: maestro_models::Model) {
    request_support::run(async {
        let signal = maestro_models::Cancellation::new();
        signal.abort();
        let options = maestro_models::ProviderStreamOptions {
            common: maestro_models::StreamOptions {
                signal: Some(signal),
                api_key: Some("unused".into()),
                fetch: Some(Arc::new(|_| {
                    Box::pin(async { Err(maestro_models::FetchError::Aborted) })
                })),
                ..Default::default()
            },
            ..Default::default()
        };
        let stream = restored
            .stream
            .call(
                model,
                maestro_models::Context {
                    messages: vec![],
                    tools: None,
                    system_prompt: None,
                },
                maestro_models::ProviderInvocation::Threaded(Some(options), None),
            )
            .unwrap();
        let terminal = stream.result().await;
        let terminal = terminal.read().unwrap();
        assert!(matches!(
            terminal.stop_reason,
            maestro_models::StopReason::Aborted
        ));
        assert!(
            terminal
                .error_message
                .as_ref()
                .is_some_and(|message| message.contains("aborted"))
        );
    });
}

#[test]
fn failed_same_handle_registration_preserves_prior_alias_edits() {
    if dynamic::isolated("failed_same_handle_registration_preserves_prior_alias_edits") {
        return;
    }
    let auth = support::auth();
    auth.set(
        "p",
        maestro_credentials::AuthCredential::OAuth(dynamic::tokens()),
    );
    let registry = ModelRegistry::in_memory(auth, crate::support::values()).unwrap();
    let retained = dynamic::config(&["m"]);
    registry.register_provider("p", retained.clone()).unwrap();
    {
        let mut config = retained.write().unwrap();
        config.name = Some("alias edit".into());
        config.callbacks.oauth = Some(Arc::new(dynamic::OAuth {
            calls: None,
            id: "ignored",
            name: "Failing",
            transform: Arc::new(|_| Err(dynamic::diagnostic("alias failure").into())),
        }));
    }
    assert_eq!(
        registry
            .register_provider("p", retained.clone())
            .unwrap_err()
            .to_string(),
        "alias failure"
    );
    assert_eq!(registry.get_provider_display_name("p"), "alias edit");
    assert_eq!(registry.refresh().unwrap_err().to_string(), "alias failure");
    retained.write().unwrap().callbacks.oauth = None;
    registry.refresh().unwrap();
    assert_eq!(registry.get_provider_display_name("p"), "alias edit");
    assert!(registry.find("p", "m").is_some());
}

#[test]
fn model_overrides_apply_to_registered_provider_models() {
    if dynamic::isolated("model_overrides_apply_to_registered_provider_models") {
        return;
    }
    let text = support::Text(Arc::new(std::sync::Mutex::new(Some(
        r#"{"providers":{"p":{"modelOverrides":{"m":{"name":"Overridden","contextWindow":4242,"headers":{"x-model-override":"enabled"}}}}}}"#.into(),
    ))));
    let registry = support::text_registry("models-input", &text, crate::support::values());
    registry
        .register_provider("p", dynamic::config(&["m", "other"]))
        .unwrap();
    for _ in 0..2 {
        let model = registry.find("p", "m").unwrap();
        assert_eq!(model.read().unwrap().name, "Overridden");
        assert!((model.read().unwrap().context_window - 4242.0).abs() < f64::EPSILON);
        request_support::success(
            request_support::run(registry.get_api_key_and_headers(&model)),
            Some("configured"),
            &[("x-model-override", "enabled")],
        );
        let other = registry.find("p", "other").unwrap();
        request_support::success(
            request_support::run(registry.get_api_key_and_headers(&other)),
            Some("configured"),
            &[],
        );
        registry.refresh().unwrap();
    }
}

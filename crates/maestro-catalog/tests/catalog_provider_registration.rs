#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
//! Dynamic provider admission and request observations.
use crate::support;
use maestro_catalog::{ModelRegistry, ProviderConfigInput};
use maestro_models::SharedHandle;
use maestro_models::{
    ProviderStreamOptions, SimpleStreamOptions, StreamOptions, StreamOptionsInput,
};
use std::sync::{Arc, RwLock};

#[test]
fn registration_checks_incoming_fields_before_any_effect() {
    if dynamic::isolated("registration_checks_incoming_fields_before_any_effect") {
        return;
    }
    let registry = ModelRegistry::in_memory(support::auth(), crate::support::values()).unwrap();
    let retained = dynamic::config(&["m"]);
    retained.write().unwrap().callbacks.oauth = Some(dynamic::oauth("retained"));
    dynamic::callback(&retained, "retained");
    registry.register_provider("p", retained.clone()).unwrap();
    let old = registry.get_all();
    let oauth = maestro_models::get_oauth_provider("p").unwrap();
    let api = maestro_models::get_api_provider("openai-completions").unwrap();
    for case in 0..9 {
        let config = invalid_config(case);
        let error: maestro_catalog::CatalogError =
            registry.register_provider("p", config).unwrap_err();
        assert!(matches!(
            error,
            maestro_catalog::CatalogError::Diagnostic(_)
        ));
        assert_eq!(error.to_string(), invalid_message(case));
        assert_eq!(
            error.diagnostic(),
            &maestro_models::DiagnosticErrorInfo {
                message: invalid_message(case).into(),
                name: None,
                stack: None,
                code: None,
            }
        );
        assert!(std::error::Error::source(&error).is_none());
        assert!(Arc::ptr_eq(&old, &registry.get_all()));
        assert!(Arc::ptr_eq(
            &oauth,
            &maestro_models::get_oauth_provider("p").unwrap()
        ));
        assert!(Arc::ptr_eq(
            &api.stream,
            &maestro_models::get_api_provider("openai-completions")
                .unwrap()
                .stream
        ));
        assert_eq!(retained.read().unwrap().models.as_ref().unwrap()[0].id, "m");
        request_support::success(request_support::request(&registry), Some("configured"), &[]);
    }
    registry.refresh().unwrap();
    assert_eq!(dynamic::failure(&registry, "p", "m"), "retained");
}
#[test]
fn provider_config_json_decodes_into_native_registration() {
    let shared: maestro_request::provider_config::ProviderModelConfig = dynamic::model("m");
    let _: maestro_catalog::ProviderModelInput = shared;
    let config: ProviderConfigInput = serde_json::from_str(
        r#"{"name":"","baseUrl":"https://json.invalid/v1","apiKey":"json-key",
            "api":"openai-completions","headers":{"b":"2","a":"1"},"authHeader":false,
            "models":[{"id":"j","name":"J","baseUrl":"","reasoning":true,
            "thinkingLevelMap":{"high":null},"input":["text","text"],
            "cost":{"input":-0.0,"output":0.5,"cacheRead":0,"cacheWrite":-2},
            "contextWindow":128000,"maxTokens":0.25,"compat":{"x":[1]}}]}"#,
    )
    .unwrap();
    assert!(config.callbacks.stream_simple.is_none() && config.callbacks.oauth.is_none());
    let registry = ModelRegistry::in_memory(support::auth(), crate::support::values()).unwrap();
    registry
        .register_provider("json", SharedHandle::new(RwLock::new(config)))
        .unwrap();
    let model = registry.find("json", "j").unwrap();
    let model = model.read().unwrap();
    assert_eq!(
        serde_json::to_value(&*model).unwrap(),
        serde_json::json!({"id":"j","name":"J","api":"openai-completions","provider":"json",
            "baseUrl":"","reasoning":true,"thinkingLevelMap":{"high":null},
            "input":["text","text"],
            "cost":{"input":-0.0,"output":0.5,"cacheRead":0.0,"cacheWrite":-2.0},
            "contextWindow":128_000.0,"maxTokens":0.25,"compat":{"x":[1]}})
    );
    assert!(model.cost.input.is_sign_negative());
    assert_eq!(registry.get_provider_display_name("json"), "");
}

/// Competing admission failures selected from independent incoming records.
fn invalid_config(case: usize) -> SharedHandle<RwLock<ProviderConfigInput>> {
    let config = dynamic::config(&["m"]);
    let mut incoming = config.write().unwrap();
    incoming.callbacks.oauth = Some(dynamic::oauth("incoming"));
    if case < 2 {
        incoming.api = (case == 1).then(String::new);
        incoming.base_url = None;
        incoming.callbacks.stream_simple = Some(Arc::new(UnexpectedProvider));
    } else if case < 5 {
        incoming.base_url = (case == 3).then(String::new);
        incoming.api_key = None;
        if case == 4 {
            incoming.models.as_mut().unwrap()[0].base_url = Some("model-url".into());
        }
    } else if case < 7 {
        incoming.callbacks.oauth = None;
        incoming.api_key = (case == 6).then(String::new);
        incoming.api = None;
    } else {
        incoming.api = None;
        if case == 8 {
            incoming.models.as_mut().unwrap()[0].api = Some("test".into());
            incoming
                .models
                .as_mut()
                .unwrap()
                .push(dynamic::model("second"));
        }
    }
    drop(incoming);
    config
}
/// Winning full diagnostics for each competing incoming record.
fn invalid_message(case: usize) -> &'static str {
    match case {
        0 | 1 => "Provider p: \"api\" is required when registering streamSimple.",
        2..=4 => "Provider p: \"baseUrl\" is required when defining models.",
        5 | 6 => "Provider p: \"apiKey\" or \"oauth\" is required when defining models.",
        7 => "Provider p, model m: no \"api\" specified.",
        _ => "Provider p, model second: no \"api\" specified.",
    }
}
use crate::dynamic;
#[test]
fn dynamic_definitions_keep_values_without_file_defaults() {
    if dynamic::isolated("dynamic_definitions_keep_values_without_file_defaults") {
        return;
    }
    let registry = ModelRegistry::in_memory(support::auth(), crate::support::values()).unwrap();
    for models in [None, Some(vec![])] {
        registry
            .register_provider(
                "empty",
                SharedHandle::new(RwLock::new(ProviderConfigInput {
                    models,
                    ..Default::default()
                })),
            )
            .unwrap();
    }
    assert!(dynamic::selected(&registry, &["empty"]).is_empty());
    for ids in [&[][..], &["m"][..]] {
        let config = dynamic::config(ids);
        if !ids.is_empty() {
            let mut config = config.write().unwrap();
            config.api = Some(" ".into());
            config.base_url = Some(" ".into());
            let model = &mut config.models.as_mut().unwrap()[0];
            model.id.clear();
            model.name.clear();
            model.input.clear();
            model.cost.input = -1.0;
            model.context_window = 0.0;
            model.max_tokens = -2.0;
            model.api = Some(String::new());
            model.base_url = Some(String::new());
        }
        registry.register_provider("p", config).unwrap();
    }
    let config = dynamic::config(&["named-id"]);
    config.write().unwrap().models.as_mut().unwrap()[0]
        .name
        .clear();
    registry.register_provider("q", config).unwrap();
    assert_eq!(
        registry.find("q", "named-id").unwrap().read().unwrap().name,
        ""
    );
    let value = registry.find("p", "").unwrap();
    let value = value.read().unwrap();
    assert_eq!(value.name, "");
    assert_eq!(value.api, " ");
    assert_eq!(value.base_url, "");
    assert!(value.input.is_empty());
    assert_eq!(value.context_window.to_bits(), (0.0_f64).to_bits());
    assert_eq!(value.max_tokens.to_bits(), (-2.0_f64).to_bits());
    assert_eq!(value.cost.input.to_bits(), (-1.0_f64).to_bits());
}
use crate::request_support;
#[test]
fn request_sources_replace_only_on_truthy_incoming_fields() {
    if dynamic::isolated("request_sources_replace_only_on_truthy_incoming_fields") {
        return;
    }
    for case in 0..8 {
        let registry = ModelRegistry::in_memory(support::auth(), crate::support::values()).unwrap();
        let first = dynamic::config(&["m"]);
        first.write().unwrap().headers = Some([("X".into(), "initial".into())].into());
        first.write().unwrap().auth_header = Some(true);
        registry.register_provider("p", first).unwrap();
        let incoming = request_update(case);
        registry.register_provider("p", incoming).unwrap();
        check_request_update(&registry, case, false);
        registry.refresh().unwrap();
        check_request_update(&registry, case, true);
    }
}
/// Source inputs distinguish truthy immediate storage from defined retained merging.
fn request_update(case: usize) -> SharedHandle<RwLock<ProviderConfigInput>> {
    let mut incoming = ProviderConfigInput::default();
    match case {
        1 => incoming.api_key = Some(String::new()),
        2 => incoming.auth_header = Some(false),
        3 => incoming.headers = Some(indexmap::IndexMap::new()),
        4 | 7 => {
            incoming.headers = Some([("Y".into(), "later".into())].into());
            if case == 7 {
                incoming.auth_header = Some(false);
            }
        }
        5 => incoming.auth_header = Some(true),
        6 => incoming.api_key = Some("later".into()),
        _ => {}
    }
    Arc::new(RwLock::new(incoming))
}
/// Exact independently recorded request observations before and after replay.
fn check_request_update(registry: &ModelRegistry, case: usize, replayed: bool) {
    let result = request_support::request(registry);
    if (case == 1 && replayed) || (case == 5 && !replayed) {
        request_support::error(result, "No API key found for \"p\"");
        return;
    }
    let key = if case == 6 {
        Some("later")
    } else if !replayed && [3, 4, 7].contains(&case) {
        None
    } else {
        Some("configured")
    };
    let headers: &[(&str, &str)] = match (case, replayed) {
        (3 | 6, false) => &[],
        (4 | 7, false) | (7, true) => &[("Y", "later")],
        (2, true) => &[("X", "initial")],
        (3, true) => &[("Authorization", "Bearer configured")],
        (4, true) => &[("Y", "later"), ("Authorization", "Bearer configured")],
        (6, true) => &[("X", "initial"), ("Authorization", "Bearer later")],
        _ => &[("X", "initial"), ("Authorization", "Bearer configured")],
    };
    request_support::success(result, key, headers);
}
#[test]
fn empty_lists_and_overrides_preserve_branch_specific_identity() {
    if dynamic::isolated("empty_lists_and_overrides_preserve_branch_specific_identity") {
        return;
    }
    let registry = ModelRegistry::in_memory(support::auth(), crate::support::values()).unwrap();
    registry
        .register_provider("p", dynamic::config(&["m"]))
        .unwrap();
    registry
        .register_provider("q", dynamic::config(&["m"]))
        .unwrap();
    let old = registry.get_all();
    let p = registry.find("p", "m").unwrap();
    let q = registry.find("q", "m").unwrap();
    for models in [None, Some(vec![])] {
        registry
            .register_provider(
                "p",
                SharedHandle::new(RwLock::new(ProviderConfigInput {
                    base_url: Some(String::new()),
                    models,
                    ..Default::default()
                })),
            )
            .unwrap();
        assert!(Arc::ptr_eq(&old, &registry.get_all()));
        assert!(Arc::ptr_eq(&p, &registry.find("p", "m").unwrap()));
    }
    registry
        .register_provider(
            "p",
            SharedHandle::new(RwLock::new(ProviderConfigInput {
                base_url: Some(String::new()),
                headers: Some(indexmap::IndexMap::default()),
                ..Default::default()
            })),
        )
        .unwrap();
    assert!(!Arc::ptr_eq(&old, &registry.get_all()));
    assert!(!Arc::ptr_eq(&p, &registry.find("p", "m").unwrap()));
    assert!(Arc::ptr_eq(&q, &registry.find("q", "m").unwrap()));
    assert_eq!(p.read().unwrap().base_url, "https://dynamic.invalid/v1");
    assert_eq!(
        registry.find("p", "m").unwrap().read().unwrap().base_url,
        ""
    );
}
#[test]
fn replacement_appends_duplicates_and_preserves_unrelated_handles() {
    if dynamic::isolated("replacement_appends_duplicates_and_preserves_unrelated_handles") {
        return;
    }
    let registry = ModelRegistry::in_memory(support::auth(), crate::support::values()).unwrap();
    registry
        .register_provider("p", dynamic::config(&["old"]))
        .unwrap();
    registry
        .register_provider("q", dynamic::config(&["other"]))
        .unwrap();
    let old = registry.get_all();
    let unrelated = registry.find("q", "other").unwrap();
    let config = dynamic::config(&["z", "a", "z"]);
    {
        let mut config = config.write().unwrap();
        let definitions = config.models.as_mut().unwrap();
        definitions[0].headers = Some([("X".into(), "first".into())].into());
        definitions[2].name = "second z".into();
        definitions[2].headers = Some([("X".into(), "last".into())].into());
    }
    registry.register_provider("p", config).unwrap();
    assert!(!Arc::ptr_eq(&old, &registry.get_all()));
    assert!(
        old.read()
            .unwrap()
            .iter()
            .any(|m| m.read().unwrap().id == "old")
    );
    assert!(Arc::ptr_eq(
        &unrelated,
        &registry.find("q", "other").unwrap()
    ));
    let selected: Vec<_> = registry
        .get_all()
        .read()
        .unwrap()
        .iter()
        .filter_map(|m| {
            let m = m.read().unwrap();
            (["p", "q"].contains(&m.provider.as_str())).then(|| (m.provider.clone(), m.id.clone()))
        })
        .collect();
    assert_eq!(
        selected,
        [
            ("q".into(), "other".into()),
            ("p".into(), "z".into()),
            ("p".into(), "a".into()),
            ("p".into(), "z".into())
        ]
    );
    let first = registry.find("p", "z").unwrap();
    assert_eq!(first.read().unwrap().name, "Name z");
    request_support::success(
        request_support::run(registry.get_api_key_and_headers(&first)),
        Some("configured"),
        &[("X", "last")],
    );
}
#[test]
fn oauth_registration_forces_id_and_delegates_existing_operations() {
    if dynamic::isolated("oauth_registration_forces_id_and_delegates_existing_operations") {
        return;
    }
    let registry = ModelRegistry::in_memory(support::auth(), crate::support::values()).unwrap();
    let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
    let supplied: maestro_models::OAuthProviderHandle = Arc::new(dynamic::OAuth {
        calls: Some(calls.clone()),
        id: "ignored",
        name: "Custom",
        transform: Arc::new(|mut models| {
            models[0].to_mut().name = "delegated transformation".into();
            Ok(models)
        }),
    });
    registry
        .register_provider(
            "p",
            SharedHandle::new(RwLock::new(ProviderConfigInput {
                callbacks: maestro_catalog::ProviderCallbacks {
                    oauth: Some(supplied.clone()),
                    ..Default::default()
                },
                ..Default::default()
            })),
        )
        .unwrap();
    let installed = maestro_models::get_oauth_provider("p").unwrap();
    assert_eq!(installed.id(), "p");
    assert_eq!(supplied.id(), "ignored");
    assert_eq!(installed.name(), "Custom");
    assert_eq!(installed.uses_callback_server(), Some(true));
    assert!(calls.lock().unwrap().is_empty());
    let tokens =
        request_support::run(installed.login(Arc::new(dynamic::NoInteraction), None)).unwrap();
    assert!(tokens.access.eq("unused-access"));
    assert!(installed.get_api_key(&tokens).unwrap().eq(&tokens.access));
    let refreshed = request_support::run(installed.refresh_token(tokens, None)).unwrap();
    assert_eq!(refreshed.expires.to_bits(), (42.0_f64).to_bits());
    let models = vec![maestro_models::get_model("anthropic", "claude-sonnet-4-5").unwrap()];
    let transformed = installed
        .modify_models(&models, &refreshed)
        .unwrap()
        .unwrap();
    assert_eq!(transformed[0].name, "delegated transformation");
    assert_ne!(models[0].name, transformed[0].name);
    assert_eq!(
        *calls.lock().unwrap(),
        ["login", "key", "refresh", "transform"]
    );
}
#[test]
fn callback_failures_propagate_without_invocation_during_listing() {
    if dynamic::isolated("callback_failures_propagate_without_invocation_during_listing") {
        return;
    }
    let registry = ModelRegistry::in_memory(support::auth(), crate::support::values()).unwrap();
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = calls.clone();
    let config = dynamic::config(&["m"]);
    config.write().unwrap().callbacks.stream_simple = Some(Arc::new(CountFailure(observed)));
    registry.register_provider("p", config).unwrap();
    let _ = registry.get_all();
    let _ = registry.get_available().unwrap();
    let _ = registry.get_provider_display_name("p");
    let _ = registry.get_provider_auth_status("p").unwrap();
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    for refreshed in [false, true] {
        if refreshed {
            registry.refresh().unwrap();
        }
        let provider = maestro_models::get_api_provider("openai-completions").unwrap();
        let model = registry.find("p", "m").unwrap().read().unwrap().clone();
        let context = maestro_models::Context {
            system_prompt: None,
            messages: vec![],
            tools: None,
        };
        let raw = provider
            .stream
            .call(
                model.clone(),
                context.clone(),
                maestro_models::ProviderInvocation::Threaded(None, None),
            )
            .err()
            .unwrap();
        let simple = provider
            .stream_simple
            .call(
                model,
                context,
                maestro_models::ProviderInvocation::Threaded(None, None),
            )
            .err()
            .unwrap();
        assert_eq!(raw, dynamic::diagnostic("provider callback failed"));
        assert_eq!(simple, raw);
    }
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 4);
}
#[test]
fn incoming_oauth_transform_runs_after_replacement_only_with_tokens() {
    if dynamic::isolated("incoming_oauth_transform_runs_after_replacement_only_with_tokens") {
        return;
    }
    let auth = support::auth();
    let registry = ModelRegistry::in_memory(auth.clone(), crate::support::values()).unwrap();
    registry
        .register_provider("q", dynamic::config(&["unrelated"]))
        .unwrap();
    let unrelated = registry
        .find("q", "unrelated")
        .unwrap()
        .read()
        .unwrap()
        .clone();
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = calls.clone();
    let oauth: maestro_models::OAuthProviderHandle = Arc::new(dynamic::OAuth {
        calls: None,
        id: "ignored",
        name: "Transform",
        transform: Arc::new(move |mut models| {
            observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let model = models.iter_mut().find(|m| m.provider == "p").unwrap();
            assert_eq!(model.id, "m");
            assert_eq!(model.cost.cache_write.to_bits(), (4.0_f64).to_bits());
            model.to_mut().base_url = "transformed".into();
            Ok(models)
        }),
    });
    for kind in 0..3 {
        set_credential_kind(&auth, kind);
        let config = dynamic::config(&["m"]);
        config.write().unwrap().callbacks.oauth = Some(oauth.clone());
        registry.register_provider("p", config).unwrap();
        assert_eq!(
            registry.find("p", "m").unwrap().read().unwrap().base_url,
            if kind == 2 {
                "transformed"
            } else {
                "https://dynamic.invalid/v1"
            }
        );
    }
    let empty = dynamic::config(&[]);
    empty.write().unwrap().callbacks.oauth = Some(oauth);
    registry.register_provider("p", empty).unwrap();
    assert_eq!(
        registry
            .find("q", "unrelated")
            .unwrap()
            .read()
            .unwrap()
            .clone(),
        unrelated
    );
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
fn replacement_updates_selected_headers_without_sweeping_retired_ids() {
    if dynamic::isolated("replacement_updates_selected_headers_without_sweeping_retired_ids") {
        return;
    }
    for headers in [
        None,
        Some(indexmap::IndexMap::new()),
        Some([("Y".into(), "new".into())].into()),
    ] {
        let registry = ModelRegistry::in_memory(support::auth(), crate::support::values()).unwrap();
        let config = dynamic::config(&["m", "retired"]);
        {
            let mut config = config.write().unwrap();
            for definition in config.models.as_mut().unwrap() {
                definition.headers = Some([("R".into(), "retained".into())].into());
            }
        }
        registry.register_provider("p", config).unwrap();
        let old = registry.find("p", "m").unwrap();
        let retired = registry.find("p", "retired").unwrap();
        let config = dynamic::config(&["m"]);
        config.write().unwrap().models.as_mut().unwrap()[0].headers = headers.clone();
        registry.register_provider("p", config).unwrap();
        for model in [&old, &registry.find("p", "m").unwrap()] {
            request_support::success(
                request_support::run(registry.get_api_key_and_headers(model)),
                Some("configured"),
                if headers.as_ref().is_some_and(|h| !h.is_empty()) {
                    &[("Y", "new")]
                } else {
                    &[]
                },
            );
        }
        request_support::success(
            request_support::run(registry.get_api_key_and_headers(&retired)),
            Some("configured"),
            &[("R", "retained")],
        );
        registry.refresh().unwrap();
        request_support::success(
            request_support::run(registry.get_api_key_and_headers(&retired)),
            Some("configured"),
            &[],
        );
    }
}

#[test]
fn empty_configured_keys_keep_presence_but_skip_resolution() {
    if dynamic::isolated("empty_configured_keys_keep_presence_but_skip_resolution") {
        return;
    }
    let registry = ModelRegistry::in_memory(support::auth(), crate::support::values()).unwrap();
    registry
        .register_provider("p", dynamic::config(&["m"]))
        .unwrap();
    registry
        .register_provider(
            "p",
            SharedHandle::new(RwLock::new(ProviderConfigInput {
                api_key: Some(String::new()),
                headers: Some(indexmap::IndexMap::default()),
                ..Default::default()
            })),
        )
        .unwrap();
    let model = registry.find("p", "m").unwrap();
    assert!(registry.has_configured_auth(&model).unwrap());
    let status = registry.get_provider_auth_status("p").unwrap();
    assert!(!status.configured);
    assert!(status.source.is_none());
    assert!(
        request_support::run(registry.get_api_key_for_provider("p"))
            .unwrap()
            .is_none()
    );
    request_support::success(request_support::request(&registry), None, &[]);
}

#[test]
fn builtin_oauth_transform_changes_only_matching_values() {
    if dynamic::isolated("builtin_oauth_transform_changes_only_matching_values") {
        return;
    }
    let oauth = maestro_models::get_oauth_provider("github-copilot").unwrap();
    let auth = support::auth();
    let mut credentials = dynamic::tokens();
    credentials.access = "proxy-ep=proxy.example.invalid;".into();
    auth.set(
        "github-copilot",
        maestro_credentials::AuthCredential::OAuth(credentials),
    );
    let registry = ModelRegistry::in_memory(Arc::clone(&auth), crate::support::values()).unwrap();
    registry
        .register_provider("other", dynamic::config(&["unrelated", "unrelated"]))
        .unwrap();
    let unrelated = registry.find("other", "unrelated").unwrap();
    let published = registry.get_all();
    let duplicates: Vec<_> = published
        .read()
        .unwrap()
        .iter()
        .filter(|handle| handle.read().unwrap().provider == "other")
        .cloned()
        .collect();
    assert!(!Arc::ptr_eq(&duplicates[0], &duplicates[1]));
    let config = dynamic::config(&["m"]);
    config.write().unwrap().callbacks.oauth = Some(Arc::clone(&oauth));
    registry
        .register_provider("github-copilot", Arc::clone(&config))
        .unwrap();
    assert_eq!(
        registry
            .find("github-copilot", "m")
            .unwrap()
            .read()
            .unwrap()
            .base_url,
        "https://api.example.invalid"
    );
    assert!(Arc::ptr_eq(
        &registry.find("other", "unrelated").unwrap(),
        &unrelated,
    ));
    assert!(
        published
            .read()
            .unwrap()
            .iter()
            .any(|model| Arc::ptr_eq(model, &unrelated))
    );
    check_duplicate_aliases(&registry, &duplicates, &unrelated);
    assert!(
        config.read().unwrap().models.as_ref().unwrap()[0]
            .base_url
            .is_none()
    );
}

/// Observe independent retained aliases in the newly published list.
fn check_duplicate_aliases(
    registry: &ModelRegistry,
    duplicates: &[Arc<RwLock<maestro_models::Model>>],
    unrelated: &Arc<RwLock<maestro_models::Model>>,
) {
    let live: Vec<_> = registry
        .get_all()
        .read()
        .unwrap()
        .iter()
        .filter(|handle| handle.read().unwrap().provider == "other")
        .cloned()
        .collect();
    assert!(Arc::ptr_eq(&live[0], &duplicates[0]));
    assert!(Arc::ptr_eq(&live[1], &duplicates[1]));
    duplicates[1].write().unwrap().name = "second alias".into();
    unrelated.write().unwrap().name = "edited through retained alias".into();
    assert_eq!(live[1].read().unwrap().name, "second alias");
    assert_eq!(
        registry
            .find("other", "unrelated")
            .unwrap()
            .read()
            .unwrap()
            .name,
        "edited through retained alias"
    );
}

#[test]
fn builtin_oauth_transform_detaches_changed_descriptors() {
    if dynamic::isolated("builtin_oauth_transform_detaches_changed_descriptors") {
        return;
    }
    let oauth = maestro_models::get_oauth_provider("github-copilot").unwrap();
    let auth = support::auth();
    let registry = ModelRegistry::in_memory(Arc::clone(&auth), crate::support::values()).unwrap();
    registry
        .register_provider("github-copilot", dynamic::config(&["m"]))
        .unwrap();
    let matching = registry.find("github-copilot", "m").unwrap();
    let mut credentials = dynamic::tokens();
    credentials.access = "proxy-ep=proxy.changed.invalid;".into();
    auth.set(
        "transform",
        maestro_credentials::AuthCredential::OAuth(credentials),
    );
    let transform = dynamic::config(&["trigger"]);
    transform.write().unwrap().callbacks.oauth = Some(oauth);
    registry.register_provider("transform", transform).unwrap();
    let changed = registry.find("github-copilot", "m").unwrap();
    assert!(!Arc::ptr_eq(&matching, &changed));
    matching.write().unwrap().name = "detached edit".into();
    assert_eq!(changed.read().unwrap().name, "Name m");
    assert_eq!(
        changed.read().unwrap().base_url,
        "https://api.changed.invalid"
    );
    assert_eq!(
        matching.read().unwrap().base_url,
        "https://dynamic.invalid/v1"
    );
}

#[test]
fn per_model_metadata_replaces_provider_defaults_without_compat_merging() {
    if dynamic::isolated("per_model_metadata_replaces_provider_defaults_without_compat_merging") {
        return;
    }
    let registry = ModelRegistry::in_memory(support::auth(), crate::support::values()).unwrap();
    let config = dynamic::config(&["m"]);
    let mut expected = dynamic::model("m");
    expected.api = Some("model-api".into());
    expected.base_url = Some("https://per-model.invalid".into());
    expected.reasoning = true;
    expected.thinking_level_map = Some(
        [
            (maestro_models::ModelThinkingLevel::Off, None),
            (
                maestro_models::ModelThinkingLevel::High,
                Some("effort".into()),
            ),
        ]
        .into(),
    );
    expected.input = vec![
        maestro_models::ModelInput::Image,
        maestro_models::ModelInput::Text,
        maestro_models::ModelInput::Image,
    ];
    expected.headers = Some([("X".into(), "model".into())].into());
    expected.compat = Some(maestro_models::ModelCompat(
        serde_json::from_value(
            serde_json::json!({"custom":{"z":1,"a":[2,1,2]},"supportsStrictMode":false}),
        )
        .unwrap(),
    ));
    config.write().unwrap().api = None;
    config.write().unwrap().models = Some(vec![expected.clone()]);
    registry.register_provider("p", config).unwrap();
    let model = registry.find("p", "m").unwrap();
    let model = model.read().unwrap();
    assert_eq!(model.api, "model-api");
    assert_eq!(model.base_url, "https://per-model.invalid");
    assert!(model.reasoning);
    assert_eq!(model.thinking_level_map, expected.thinking_level_map);
    assert_eq!(model.input, expected.input);
    assert_eq!(model.cost, expected.cost);
    assert_eq!(model.context_window.to_bits(), (12345.0_f64).to_bits());
    assert_eq!(model.max_tokens.to_bits(), (678.0_f64).to_bits());
    assert_eq!(model.compat, expected.compat);
    assert!(model.headers.is_none());
}

#[test]
fn raw_and_simple_callbacks_keep_open_options_results_and_source_ownership() {
    if dynamic::isolated("raw_and_simple_callbacks_keep_open_options_results_and_source_ownership")
    {
        return;
    }
    let registry = ModelRegistry::in_memory(support::auth(), crate::support::values()).unwrap();
    let terminal = dynamic::terminal();
    let output = maestro_models::AssistantMessageEventStream::new();
    output.end(Some(terminal.clone()));
    let signal = maestro_models::Cancellation::new();
    let hook: maestro_models::OnPayload = Arc::new(|value, _| Box::pin(async { Ok(value) }));
    let object = Arc::new(42_u32);
    let owner = Arc::new(());
    let weak = Arc::downgrade(&owner);
    let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
    let config = dynamic::config(&["m"]);
    config.write().unwrap().callbacks.stream_simple = Some(raw_callback(
        hook.clone(),
        object.clone(),
        owner.clone(),
        output,
        calls.clone(),
    ));
    registry.register_provider("p", config).unwrap();
    drop(owner);
    let handler = maestro_models::get_api_provider("openai-completions").unwrap();
    let model = registry.find("p", "m").unwrap().read().unwrap().clone();
    let context = maestro_models::Context {
        messages: vec![],
        tools: None,
        system_prompt: Some("instruction".into()),
    };
    let common = StreamOptions {
        max_tokens: Some(0.0),
        signal: Some(signal.clone()),
        on_payload: Some(hook),
        ..Default::default()
    };
    let results = invoke_option_forms(&handler, &model, &context, common, object);
    for result in results {
        assert!(Arc::ptr_eq(
            &terminal,
            &request_support::run(result.result())
        ));
    }
    assert!(signal.is_aborted());
    assert_eq!(*calls.lock().unwrap(), ["raw", "simple", "none", "none"]);
    check_callback_release(&registry, handler, model, &terminal, &weak);
}
/// Assert common options without displaying secret-bearing records.
fn check_common<E: maestro_models::ProviderExecution>(
    options: &maestro_models::StreamOptions<E>,
    hook: &maestro_models::OnPayload,
) {
    assert_eq!(options.max_tokens, Some(0.0));
    assert!(std::ptr::addr_eq(
        &raw const **options.on_payload.as_ref().unwrap(),
        &raw const **hook
    ));
    options.signal.as_ref().unwrap().abort();
}

/// Retain option observations and a shared result without capturing their registry.
fn raw_callback(
    observed_hook: maestro_models::OnPayload,
    observed_object: Arc<u32>,
    observed_owner: Arc<()>,
    returned: maestro_models::AssistantMessageEventStream,
    observed: Arc<std::sync::Mutex<Vec<&'static str>>>,
) -> maestro_models::StreamFunction<StreamOptionsInput> {
    Arc::new(OriginalOptions {
        observed_hook,
        observed_object,
        observed_owner,
        returned,
        observed,
    })
}
/// Exercise distinct option carriers and an absent outer operand.
fn invoke_option_forms(
    handler: &maestro_models::ApiProvider,
    model: &maestro_models::Model,
    context: &maestro_models::Context,
    common: StreamOptions,
    object: Arc<u32>,
) -> [maestro_models::AssistantMessageEventStream; 4] {
    let mut raw = ProviderStreamOptions {
        common: common.clone(),
        extra: serde_json::from_value(
            serde_json::json!({"reasoning":{"mode":"custom"},"custom":["z","a","z"]}),
        )
        .unwrap(),
        ..Default::default()
    };
    raw.objects.insert(object);
    [
        handler
            .stream
            .call(
                model.clone(),
                context.clone(),
                maestro_models::ProviderInvocation::Threaded(Some(raw), None),
            )
            .unwrap(),
        handler
            .stream_simple
            .call(
                model.clone(),
                context.clone(),
                maestro_models::ProviderInvocation::Threaded(
                    Some(SimpleStreamOptions {
                        common,
                        reasoning: Some(maestro_models::ThinkingLevel::High),
                        ..Default::default()
                    }),
                    None,
                ),
            )
            .unwrap(),
        handler
            .stream
            .call(
                model.clone(),
                context.clone(),
                maestro_models::ProviderInvocation::Threaded(None, None),
            )
            .unwrap(),
        handler
            .stream_simple
            .call(
                model.clone(),
                context.clone(),
                maestro_models::ProviderInvocation::Threaded(None, None),
            )
            .unwrap(),
    ]
}

/// Retained callbacks remain callable until their final independent handle is released.
fn check_callback_release(
    registry: &ModelRegistry,
    handler: maestro_models::ApiProvider,
    model: maestro_models::Model,
    terminal: &maestro_models::SharedAssistantMessage,
    weak: &std::sync::Weak<()>,
) {
    let context = maestro_models::Context {
        messages: vec![],
        tools: None,
        system_prompt: Some("instruction".into()),
    };
    let mut other = model.clone();
    other.api = "other".into();
    assert_eq!(
        handler
            .stream
            .call(
                other,
                context.clone(),
                maestro_models::ProviderInvocation::Threaded(None, None)
            )
            .err()
            .unwrap()
            .message,
        "Mismatched api: other expected openai-completions"
    );
    maestro_models::unregister_api_providers("provider:p");
    assert!(maestro_models::get_api_provider("openai-completions").is_none());
    registry.unregister_provider("p").unwrap();
    assert!(weak.upgrade().is_some());
    let result = handler
        .stream
        .call(
            model,
            context,
            maestro_models::ProviderInvocation::Threaded(None, None),
        )
        .unwrap();
    assert!(Arc::ptr_eq(
        terminal,
        &request_support::run(result.result())
    ));
    drop(handler);
    assert!(weak.upgrade().is_none());
}

/// Store each admitted credential kind independently of the incoming account operations.
fn set_credential_kind(auth: &maestro_credentials::AuthStorage, kind: usize) {
    if kind == 1 {
        auth.set(
            "p",
            maestro_credentials::AuthCredential::ApiKey(maestro_credentials::ApiKeyCredential {
                env: None,
                key: "unused".into(),
            }),
        );
    }
    if kind == 2 {
        auth.set(
            "p",
            maestro_credentials::AuthCredential::OAuth(dynamic::tokens()),
        );
    }
}

#[test]
fn absent_oauth_modifier_preserves_unrelated_descriptor_alias() {
    if dynamic::isolated("absent_oauth_modifier_preserves_unrelated_descriptor_alias") {
        return;
    }
    let auth = support::auth();
    let registry = ModelRegistry::in_memory(auth.clone(), crate::support::values()).unwrap();
    registry
        .register_provider("q", dynamic::config(&["other"]))
        .unwrap();
    let unrelated = registry.find("q", "other").unwrap();
    auth.set(
        "p",
        maestro_credentials::AuthCredential::OAuth(dynamic::tokens()),
    );
    let config = dynamic::config(&["m"]);
    config.write().unwrap().callbacks.oauth =
        Some(maestro_models::get_oauth_provider("anthropic").unwrap());
    registry.register_provider("p", config).unwrap();
    assert!(Arc::ptr_eq(
        &unrelated,
        &registry.find("q", "other").unwrap()
    ));
    unrelated.write().unwrap().name = "surviving alias".into();
    assert_eq!(
        registry.find("q", "other").unwrap().read().unwrap().name,
        "surviving alias"
    );
}

#[test]
fn nonempty_models_accept_oauth_without_configured_key() {
    if dynamic::isolated("nonempty_models_accept_oauth_without_configured_key") {
        return;
    }
    let registry = ModelRegistry::in_memory(support::auth(), crate::support::values()).unwrap();
    let config = dynamic::config(&["m"]);
    config.write().unwrap().api_key = None;
    config.write().unwrap().callbacks.oauth = Some(dynamic::oauth("OAuth only"));
    registry.register_provider("p", config).unwrap();
    assert_eq!(
        registry.find("p", "m").unwrap().read().unwrap().api,
        "openai-completions"
    );
    assert_eq!(
        maestro_models::get_oauth_provider("p").unwrap().name(),
        "OAuth only"
    );
}

#[test]
fn repeated_borrowed_descriptor_keeps_one_shared_alias() {
    if dynamic::isolated("repeated_borrowed_descriptor_keeps_one_shared_alias") {
        return;
    }
    let auth = support::auth();
    let registry = ModelRegistry::in_memory(auth.clone(), crate::support::values()).unwrap();
    registry
        .register_provider("other", dynamic::config(&["m"]))
        .unwrap();
    let retained = registry.find("other", "m").unwrap();
    auth.set(
        "p",
        maestro_credentials::AuthCredential::OAuth(dynamic::tokens()),
    );
    let config = dynamic::config(&["trigger"]);
    config.write().unwrap().callbacks.oauth = Some(Arc::new(dynamic::OAuth {
        calls: None,
        id: "ignored",
        name: "Repeated borrow",
        transform: Arc::new(|models| {
            static FOREIGN: std::sync::OnceLock<maestro_models::Model> = std::sync::OnceLock::new();
            let selected = models.into_iter().find(|m| m.provider == "other").unwrap();
            let foreign = FOREIGN.get_or_init(|| selected.as_ref().clone());
            let copied = std::borrow::Cow::Owned(selected.as_ref().clone());
            Ok(vec![
                selected.clone(),
                selected,
                copied,
                std::borrow::Cow::Borrowed(foreign),
                std::borrow::Cow::Borrowed(foreign),
            ])
        }),
    }));
    registry.register_provider("p", config).unwrap();
    let published = registry.get_all();
    let live = published.read().unwrap();
    assert_eq!(live.len(), 5);
    assert!(Arc::ptr_eq(&live[0], &retained));
    assert!(Arc::ptr_eq(&live[1], &retained));
    assert!(!Arc::ptr_eq(&live[2], &retained));
    assert!(!Arc::ptr_eq(&live[3], &retained));
    assert!(!Arc::ptr_eq(&live[2], &live[3]));
    assert!(Arc::ptr_eq(&live[3], &live[4]));
    retained.write().unwrap().name = "shared edit".into();
    assert_eq!(live[0].read().unwrap().name, "shared edit");
    assert_eq!(live[1].read().unwrap().name, "shared edit");
    assert_eq!(live[2].read().unwrap().name, "Name m");
    assert_eq!(live[3].read().unwrap().name, "Name m");
    live[3].write().unwrap().name = "foreign edit".into();
    assert_eq!(live[4].read().unwrap().name, "foreign edit");
}

#[test]
fn owned_descriptor_preserves_key_order_and_signed_zero() {
    if dynamic::isolated("owned_descriptor_preserves_key_order_and_signed_zero") {
        return;
    }
    let auth = support::auth();
    let registry = ModelRegistry::in_memory(auth.clone(), crate::support::values()).unwrap();
    registry
        .register_provider("other", dynamic::config(&["m"]))
        .unwrap();
    let retained = registry.find("other", "m").unwrap();
    {
        let mut model = retained.write().unwrap();
        model.cost.input = 0.0;
        model.compat = Some(
            serde_json::from_str(r#"{"supportsStore":true,"supportsDeveloperRole":false}"#)
                .unwrap(),
        );
    }
    auth.set(
        "p",
        maestro_credentials::AuthCredential::OAuth(dynamic::tokens()),
    );
    let config = dynamic::config(&["trigger"]);
    config.write().unwrap().callbacks.oauth = Some(Arc::new(dynamic::OAuth {
        calls: None,
        id: "ignored",
        name: "Exact returned values",
        transform: Arc::new(|mut models| {
            let model = models
                .iter_mut()
                .find(|m| m.provider == "other")
                .unwrap()
                .to_mut();
            model.cost.input = -0.0;
            model.compat = Some(
                serde_json::from_str(r#"{"supportsDeveloperRole":false,"supportsStore":true}"#)
                    .unwrap(),
            );
            Ok(models)
        }),
    }));
    registry.register_provider("p", config).unwrap();
    let returned = registry.find("other", "m").unwrap();
    assert!(!Arc::ptr_eq(&retained, &returned));
    let model = returned.read().unwrap();
    assert_eq!(model.cost.input.to_bits(), (-0.0_f64).to_bits());
    let compat = serde_json::to_value(model.compat.as_ref().unwrap()).unwrap();
    assert_eq!(
        compat
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["supportsDeveloperRole", "supportsStore"]
    );
    assert_eq!(
        retained.read().unwrap().cost.input.to_bits(),
        0.0_f64.to_bits()
    );
}

#[test]
fn constructor_modifier_repeated_input_keeps_one_shared_handle() {
    if dynamic::isolated("constructor_modifier_repeated_input_keeps_one_shared_handle") {
        return;
    }
    let auth = support::auth();
    auth.set(
        "constructor",
        maestro_credentials::AuthCredential::OAuth(dynamic::tokens()),
    );
    maestro_models::register_oauth_provider(Arc::new(dynamic::OAuth {
        calls: None,
        id: "constructor",
        name: "Constructor aliases",
        transform: Arc::new(|models| {
            let selected = models.into_iter().next().unwrap();
            Ok(vec![selected.clone(), selected])
        }),
    }));
    let registry = ModelRegistry::in_memory(auth, crate::support::values()).unwrap();
    let published = registry.get_all();
    let models = published.read().unwrap();
    assert_eq!(models.len(), 2);
    assert!(Arc::ptr_eq(&models[0], &models[1]));
    models[0].write().unwrap().name = "constructor edit".into();
    assert_eq!(models[1].read().unwrap().name, "constructor edit");
}

#[test]
fn chained_constructor_modifiers_preserve_first_step_alias() {
    if dynamic::isolated("chained_constructor_modifiers_preserve_first_step_alias") {
        return;
    }
    let auth = support::auth();
    for id in ["first", "second"] {
        auth.set(
            id,
            maestro_credentials::AuthCredential::OAuth(dynamic::tokens()),
        );
    }
    maestro_models::register_oauth_provider(Arc::new(dynamic::OAuth {
        calls: None,
        id: "first",
        name: "First",
        transform: Arc::new(|models| {
            let selected = models.into_iter().next().unwrap();
            Ok(vec![selected.clone(), selected])
        }),
    }));
    maestro_models::register_oauth_provider(Arc::new(dynamic::OAuth {
        calls: None,
        id: "second",
        name: "Second",
        transform: Arc::new(|models| Ok(models)),
    }));
    let registry = ModelRegistry::in_memory(auth, crate::support::values()).unwrap();
    let published = registry.get_all();
    let models = published.read().unwrap();
    assert_eq!(models.len(), 2);
    assert!(Arc::ptr_eq(&models[0], &models[1]));
    models[0].write().unwrap().name = "chained edit".into();
    assert_eq!(models[1].read().unwrap().name, "chained edit");
}

/// Retained invocation owner for the controlled provider.
struct UnexpectedProvider;
impl<E: maestro_models::ProviderExecution, O>
    maestro_models::Invoke<E, maestro_models::ProviderCall<E, O>> for UnexpectedProvider
{
    type Return =
        Result<maestro_models::AssistantMessageEventStream, maestro_models::DiagnosticErrorInfo>;
    fn invoke(&self, _call: maestro_models::ProviderCall<E, O>) -> Self::Return {
        panic!("unexpected callback")
    }
}

/// Retained invocation owner for the controlled provider.
struct CountFailure(Arc<std::sync::atomic::AtomicUsize>);
impl<E: maestro_models::ProviderExecution, O>
    maestro_models::Invoke<E, maestro_models::ProviderCall<E, O>> for CountFailure
{
    type Return =
        Result<maestro_models::AssistantMessageEventStream, maestro_models::DiagnosticErrorInfo>;
    fn invoke(&self, _call: maestro_models::ProviderCall<E, O>) -> Self::Return {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Err(dynamic::diagnostic("provider callback failed"))
    }
}

/// Retained invocation owner for the controlled provider.
struct OriginalOptions {
    observed_hook: maestro_models::OnPayload,
    observed_object: Arc<u32>,
    observed_owner: Arc<()>,
    returned: maestro_models::AssistantMessageEventStream,
    observed: Arc<std::sync::Mutex<Vec<&'static str>>>,
}
impl<E: maestro_models::ProviderExecution>
    maestro_models::Invoke<
        E,
        maestro_models::ProviderCall<E, maestro_models::StreamOptionsInput<E>>,
    > for OriginalOptions
{
    type Return =
        Result<maestro_models::AssistantMessageEventStream, maestro_models::DiagnosticErrorInfo>;
    fn invoke(
        &self,
        call: maestro_models::ProviderCall<E, maestro_models::StreamOptionsInput<E>>,
    ) -> Self::Return {
        let maestro_models::ProviderCall {
            model,
            context,
            options,
            ..
        } = call;

        let _ = &self.observed_owner;
        assert_eq!(model.id, "m");
        assert_eq!(context.system_prompt.as_deref(), Some("instruction"));
        let kind = match options {
            None => "none",
            Some(StreamOptionsInput::Raw(options)) => {
                assert_eq!(
                    options.extra["reasoning"],
                    serde_json::json!({"mode":"custom"})
                );
                assert_eq!(options.extra["custom"], serde_json::json!(["z", "a", "z"]));
                assert!(Arc::ptr_eq(
                    options.objects.get::<Arc<u32>>().unwrap(),
                    &self.observed_object
                ));
                check_common(&options.common, &self.observed_hook);
                "raw"
            }
            Some(StreamOptionsInput::Simple(options)) => {
                assert_eq!(options.reasoning, Some(maestro_models::ThinkingLevel::High));
                check_common(&options.common, &self.observed_hook);
                "simple"
            }
        };
        self.observed.lock().unwrap().push(kind);
        Ok(self.returned.clone())
    }
}

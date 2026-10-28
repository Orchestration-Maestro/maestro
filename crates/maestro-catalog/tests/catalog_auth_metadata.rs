#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
//! Authentication metadata and shared descriptor observations.
use crate::credential_fallback;
use crate::fallback;
use crate::request_support;
use crate::support;
#[test]
fn configured_auth_rereads_identity_after_fallback() {
    let registry = support::registry(
        r#"{"providers":{"p":{"api":"test","baseUrl":"url","apiKey":"key","models":[{"id":"m"}]}}}"#,
    );
    let model = registry.find("p", "m").unwrap();
    model.write().unwrap().provider = "previous".into();
    let changed = model.clone();
    registry.auth_storage().set_fallback_resolver(move |_| {
        changed.write().unwrap().provider = "p".into();
        Ok(None)
    });
    assert!(registry.has_configured_auth(&model).unwrap());
    registry.auth_storage().set_fallback_resolver(|_| Ok(None));
}
use maestro_catalog::{BUILT_IN_PROVIDER_DISPLAY_NAMES, ModelRegistry};
use std::sync::Arc;
#[test]
fn availability_walk_reloads_slots_after_callbacks() {
    let registry = support::registry(
        r#"{"providers":{"p":{"api":"test","baseUrl":"url","apiKey":"key","models":[{"id":"m"}]}}}"#,
    );
    let model = registry.find("p", "m").unwrap();
    let models: Vec<_> = ["a", "b", "c", "d", "e"]
        .into_iter()
        .map(|provider| {
            let mut value = model.read().unwrap().clone();
            value.provider = provider.into();
            Arc::new(std::sync::RwLock::new(value))
        })
        .collect();
    let list = registry.get_all();
    *list.write().unwrap() = models[..3].to_vec();
    let live = list;
    let appended = models[3..].to_vec();
    let visits = Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = visits.clone();
    registry
        .auth_storage()
        .set_fallback_resolver(move |provider| {
            seen.lock().unwrap().push(provider.to_owned());
            if provider == "a" {
                let mut list = live.write().unwrap();
                list.remove(1);
                list.extend(appended.iter().cloned());
            }
            Ok(Some("present".into()))
        });
    let selected = registry.get_available().unwrap();
    assert_eq!(*visits.lock().unwrap(), ["a", "c", "d"]);
    assert_eq!(
        selected
            .iter()
            .map(|m| m.read().unwrap().provider.clone())
            .collect::<Vec<_>>(),
        ["a", "c", "d"]
    );
    registry.auth_storage().set_fallback_resolver(|_| Ok(None));
}
#[test]
fn metadata_preserves_sources_without_helper_or_refresh_effects() {
    use maestro_credentials::{AuthSource, AuthStatus};
    if !maestro_test_process::selection().is_child() {
        let test = "metadata_preserves_sources_without_helper_or_refresh_effects";
        let process = maestro_test_process::TestProcess::current(test).unwrap();
        let environment = [
            ("ENV", Some("value")),
            ("EMPTY_ENV", Some("")),
            ("literal", None),
            ("OPENAI_API_KEY", Some("ambient-key")),
        ];
        return crate::subprocess::run_child(process.case("*"), &environment);
    }
    let marker = std::env::temp_dir().join(format!(
        "maestro-catalog-auth-marker-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&marker);
    let helper = format!("!touch '{}'", marker.display());
    for (field, source, label) in [
        ("literal", AuthSource::ModelsJsonKey, None),
        (helper.as_str(), AuthSource::ModelsJsonCommand, None),
        ("ENV", AuthSource::Environment, Some("ENV")),
    ] {
        let registry = support::registry_with(
            // A JSON string, so a Windows marker path keeps its backslashes.
            &request_support::document(&format!(r#""apiKey":{}"#, serde_json::json!(field)), ""),
            crate::support::forbidden(),
        );
        assert_eq!(
            registry.get_provider_auth_status("p").unwrap(),
            AuthStatus {
                configured: true,
                source: Some(source),
                label: label.map(str::to_owned)
            }
        );
        assert!(
            registry
                .has_configured_auth(&registry.find("p", "m").unwrap())
                .unwrap()
        );
        assert_eq!(
            registry
                .get_available()
                .unwrap()
                .iter()
                .filter(|m| m.read().unwrap().provider == "p")
                .count(),
            1
        );
        lower_layer_status(&registry);
    }
    assert!(
        !marker.exists(),
        "auth metadata ran the configured helper command"
    );
    metadata_fallback();
    metadata_empty_environment_and_oauth();
}
/// Observe absent and fallback-only provider metadata through the public registry.
fn metadata_fallback() {
    use maestro_credentials::{AuthSource, AuthStatus};
    let registry =
        maestro_catalog::ModelRegistry::in_memory(support::auth(), crate::support::values())
            .unwrap();
    assert_eq!(
        registry
            .get_provider_auth_status("unknown-metadata")
            .unwrap(),
        AuthStatus {
            configured: false,
            source: None,
            label: None
        }
    );
    registry
        .auth_storage()
        .set_fallback_resolver(|_| Ok(Some("fallback".into())));
    assert_eq!(
        registry
            .get_provider_auth_status("unknown-metadata")
            .unwrap(),
        AuthStatus {
            configured: false,
            source: Some(AuthSource::Fallback),
            label: Some("custom provider config".into())
        }
    );
    registry.auth_storage().set_fallback_resolver(|_| Ok(None));
}
#[test]
fn display_names_cover_table_oauth_and_identifier_precedence() {
    let registry =
        maestro_catalog::ModelRegistry::in_memory(support::auth(), crate::support::values())
            .unwrap();
    let expected = DISPLAY_NAMES;

    assert_eq!(
        maestro_catalog::BUILT_IN_PROVIDER_DISPLAY_NAMES
            .read()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect::<Vec<_>>(),
        expected
    );
    for &(provider, name) in expected {
        let name = if provider == "anthropic" {
            "Anthropic (Claude Pro/Max)"
        } else {
            name
        };
        assert_eq!(registry.get_provider_display_name(provider), name);
    }
    for (provider, name) in [
        ("github-copilot", "GitHub Copilot"),
        ("openai-codex", "ChatGPT Plus/Pro (Codex Subscription)"),
    ] {
        assert_eq!(registry.get_provider_display_name(provider), name);
    }
    assert_eq!(
        registry.get_provider_display_name("unknown-provider"),
        "unknown-provider"
    );
}
#[test]
fn oauth_discrimination_uses_stored_kind_not_current_key() {
    use maestro_credentials::{ApiKeyCredential, AuthCredential};
    let registry = support::registry(&request_support::document(r#""apiKey":"configured""#, ""));
    let model = registry.find("p", "m").unwrap();
    assert!(!registry.is_using_oauth(&model));
    registry.auth_storage().set(
        "p",
        AuthCredential::ApiKey(ApiKeyCredential {
            key: "stored".into(),
        }),
    );
    assert!(!registry.is_using_oauth(&model));
    for expires in [0.0, f64::MAX] {
        registry.auth_storage().set(
            "p",
            AuthCredential::OAuth(maestro_models::OAuthCredentials {
                refresh: "unused".into(),
                access: "unused".into(),
                expires,
                extra: maestro_models::JsonObject::new(),
            }),
        );
        registry.auth_storage().set_runtime_api_key("p", "runtime");
        assert!(registry.is_using_oauth(&model));
    }
}
#[test]
fn available_models_keep_order_duplicates_and_shared_descriptors() {
    let registry = support::registry(&request_support::document(r#""apiKey":"configured""#, ""));
    let model = registry.find("p", "m").unwrap();
    let list = registry.get_all();
    let mut unavailable = model.read().unwrap().clone();
    unavailable.provider = "unavailable".into();
    let unavailable = Arc::new(std::sync::RwLock::new(unavailable));
    *list.write().unwrap() = vec![model.clone(), unavailable, model.clone()];
    let mut distinct = model.read().unwrap().clone();
    distinct.id = "distinct".into();
    let distinct = Arc::new(std::sync::RwLock::new(distinct));
    list.write().unwrap().insert(2, distinct.clone());
    let selected = registry.get_available().unwrap();
    assert_eq!(selected.len(), 3);
    assert!(Arc::ptr_eq(&selected[0], &model));
    assert!(Arc::ptr_eq(&selected[1], &distinct));
    assert!(Arc::ptr_eq(&selected[2], &model));
    selected[2].write().unwrap().name = "edited".into();
    assert_eq!(model.read().unwrap().name, "edited");
    let weak = Arc::downgrade(&model);
    list.write().unwrap().clear();
    assert_eq!(selected[0].read().unwrap().name, "edited");
    drop(model);
    assert!(weak.upgrade().is_some());
    drop(selected);
    assert!(weak.upgrade().is_none());
}
#[test]
fn display_table_mutations_are_shared_between_registries() {
    if !maestro_test_process::selection().is_child() {
        let output = maestro_test_process::TestProcess::current(
            "display_table_mutations_are_shared_between_registries",
        )
        .unwrap()
        .case("*")
        .output()
        .unwrap();
        assert!(output.status.success(), "display subprocess failed");

        return;
    }
    let first = ModelRegistry::in_memory(support::auth(), crate::support::values()).unwrap();
    let second = ModelRegistry::in_memory(support::auth(), crate::support::values()).unwrap();
    let registration = request_support::OAuth::new("openai", "OAuth title").register();
    assert_eq!(first.get_provider_display_name("openai"), "OAuth title");
    drop(registration);
    let registration = request_support::OAuth::new("openai", "").register();
    assert_eq!(first.get_provider_display_name("openai"), "");
    drop(registration);
    BUILT_IN_PROVIDER_DISPLAY_NAMES
        .write()
        .unwrap()
        .insert("openai".into(), "Edited title".into());
    assert_eq!(first.get_provider_display_name("openai"), "Edited title");
    assert_eq!(second.get_provider_display_name("openai"), "Edited title");
    BUILT_IN_PROVIDER_DISPLAY_NAMES
        .write()
        .unwrap()
        .shift_remove("openai");
    assert_eq!(first.get_provider_display_name("openai"), "openai");
    BUILT_IN_PROVIDER_DISPLAY_NAMES
        .write()
        .unwrap()
        .insert("openai".into(), String::new());
    assert_eq!(second.get_provider_display_name("openai"), "");
}

/// Recorded shipped labels in source order.
const DISPLAY_NAMES: &[(&str, &str)] = &[
    ("anthropic", "Anthropic"),
    ("amazon-bedrock", "Amazon Bedrock"),
    ("azure-openai-responses", "Azure OpenAI Responses"),
    ("cerebras", "Cerebras"),
    ("cloudflare-ai-gateway", "Cloudflare AI Gateway"),
    ("cloudflare-workers-ai", "Cloudflare Workers AI"),
    ("deepseek", "DeepSeek"),
    ("fireworks", "Fireworks"),
    ("google", "Google Gemini"),
    ("google-vertex", "Google Vertex AI"),
    ("groq", "Groq"),
    ("huggingface", "Hugging Face"),
    ("kimi-coding", "Kimi For Coding"),
    ("mistral", "Mistral"),
    ("minimax", "MiniMax"),
    ("minimax-cn", "MiniMax (China)"),
    ("moonshotai", "Moonshot AI"),
    ("moonshotai-cn", "Moonshot AI (China)"),
    ("opencode", "OpenCode Zen"),
    ("opencode-go", "OpenCode Go"),
    ("openai", "OpenAI"),
    ("openrouter", "OpenRouter"),
    ("vercel-ai-gateway", "Vercel AI Gateway"),
    ("xai", "xAI"),
    ("zai", "ZAI"),
    ("xiaomi", "Xiaomi MiMo"),
    ("xiaomi-token-plan-cn", "Xiaomi MiMo Token Plan (China)"),
    (
        "xiaomi-token-plan-ams",
        "Xiaomi MiMo Token Plan (Amsterdam)",
    ),
    (
        "xiaomi-token-plan-sgp",
        "Xiaomi MiMo Token Plan (Singapore)",
    ),
];

/// Lower-layer metadata takes precedence over configured sources.
fn lower_layer_status(registry: &ModelRegistry) {
    use maestro_credentials::{ApiKeyCredential, AuthCredential, AuthSource, AuthStatus};
    registry.auth_storage().set_runtime_api_key("p", "runtime");
    assert_eq!(
        registry.get_provider_auth_status("p").unwrap(),
        AuthStatus {
            configured: false,
            source: Some(AuthSource::Runtime),
            label: Some("--api-key".into())
        }
    );
    registry.auth_storage().set(
        "p",
        AuthCredential::ApiKey(ApiKeyCredential {
            key: "!stored-helper".into(),
        }),
    );
    assert_eq!(
        registry.get_provider_auth_status("p").unwrap(),
        AuthStatus {
            configured: true,
            source: Some(AuthSource::Stored),
            label: None
        }
    );
}

/// An empty configured environment falls back; complete OAuth metadata does not refresh.
fn metadata_empty_environment_and_oauth() {
    use maestro_credentials::{AuthSource, AuthStatus};
    let registry = support::registry(&request_support::document(r#""apiKey":"EMPTY_ENV""#, ""));
    assert_eq!(
        registry.get_provider_auth_status("p").unwrap(),
        AuthStatus {
            configured: true,
            source: Some(AuthSource::ModelsJsonKey),
            label: None
        }
    );
    let _registration =
        request_support::OAuth::new("catalog-metadata-oauth", "controlled").register();
    registry
        .auth_storage()
        .set("catalog-metadata-oauth", request_support::tokens(0.0));
    let model = registry.find("p", "m").unwrap();
    model.write().unwrap().provider = "catalog-metadata-oauth".into();
    assert!(registry.has_configured_auth(&model).unwrap());
    assert!(registry.is_using_oauth(&model));
    assert!(
        registry
            .get_available()
            .unwrap()
            .iter()
            .any(|m| Arc::ptr_eq(m, &model))
    );
    assert_eq!(
        registry
            .get_provider_auth_status("catalog-metadata-oauth")
            .unwrap(),
        AuthStatus {
            configured: true,
            source: Some(AuthSource::Stored),
            label: None
        }
    );
    metadata_known_environment();
}
/// Native environment metadata keeps the lower-layer configured flag.
fn metadata_known_environment() {
    let registry = ModelRegistry::in_memory(support::auth(), crate::support::values()).unwrap();
    let status = registry.get_provider_auth_status("openai").unwrap();
    assert!(!status.configured);
    assert_eq!(
        status.source,
        Some(maestro_credentials::AuthSource::Environment)
    );
    assert_eq!(status.label.as_deref(), Some("OPENAI_API_KEY"));
    assert!(registry.auth_storage().has_auth("openai").unwrap());
}

#[test]
fn configured_key_does_not_mask_fallback_error() {
    for case in fallback::cases("configured_key_does_not_mask_fallback_error") {
        assert_eq!(case.input.fallback.as_deref(), Some("error"));
        let registry = fallback::registry(case.input.configured.unwrap());
        let calls = fallback::failing(&registry);
        let actual = if case.input.operation.as_deref() == Some("getAvailable") {
            fallback::outcome(registry.get_available(), &calls)
        } else {
            assert_eq!(case.input.operation.as_deref(), Some("hasConfiguredAuth"));
            fallback::outcome(
                registry
                    .has_configured_auth(&registry.find("maestro-custom-fallback", "m").unwrap()),
                &calls,
            )
        };
        assert!(actual == case.expected, "recorded presence failure");
    }
}
#[test]
fn provider_status_does_not_mask_fallback_failure() {
    for case in fallback::cases("provider_status_does_not_mask_fallback_failure") {
        assert_eq!(
            case.input.operation.as_deref(),
            Some("getProviderAuthStatus")
        );
        assert_eq!(case.input.fallback.as_deref(), Some("error"));
        let registry = fallback::registry(case.input.configured.unwrap());
        let calls = fallback::failing(&registry);
        let actual = fallback::outcome(
            registry.get_provider_auth_status("maestro-custom-fallback"),
            &calls,
        );
        assert!(
            actual == case.expected,
            "recorded provider metadata failure"
        );
    }
}
#[test]
fn availability_stops_on_first_error_without_partial_result() {
    for case in fallback::cases("availability_stops_on_first_error_without_partial_result") {
        let registry = fallback::registry(false);
        let models: Vec<_> = case
            .input
            .models
            .unwrap()
            .iter()
            .map(|model| fallback::descriptor(&registry, model))
            .collect();
        *registry.get_all().write().unwrap() = models.clone();
        let failing = models[case.input.failure_index.unwrap()]
            .read()
            .unwrap()
            .provider
            .clone();
        let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen = calls.clone();
        registry
            .auth_storage()
            .set_fallback_resolver(move |provider| {
                seen.lock().unwrap().push(provider.to_owned());
                if provider == failing {
                    Err(credential_fallback::failure())
                } else {
                    Ok(Some("present".into()))
                }
            });
        assert!(
            fallback::outcome(registry.get_available(), &calls) == case.expected,
            "recorded first failure"
        );
    }
}
#[test]
fn availability_uses_reentrant_slot_edits_before_failure() {
    for case in fallback::cases("availability_uses_reentrant_slot_edits_before_failure") {
        let registry = fallback::registry(false);
        let models: Vec<_> = case
            .input
            .models
            .unwrap()
            .iter()
            .map(|model| fallback::descriptor(&registry, model))
            .collect();
        let replacement = fallback::descriptor(
            &registry,
            &fallback::Descriptor {
                provider: "maestro-later".into(),
                id: "later".into(),
                name: "Later".into(),
            },
        );
        let live = registry.get_all();
        *live.write().unwrap() = models;
        let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen = calls.clone();
        registry
            .auth_storage()
            .set_fallback_resolver(move |provider| {
                seen.lock().unwrap().push(provider.to_owned());
                if provider == "maestro-first" {
                    live.write().unwrap()[1] = replacement.clone();
                    Ok(Some("present".into()))
                } else {
                    Err(credential_fallback::failure())
                }
            });
        assert!(
            fallback::outcome(registry.get_available(), &calls) == case.expected,
            "recorded live-slot failure"
        );
        registry.auth_storage().set_fallback_resolver(|_| Ok(None));
    }
}

#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
//! Authentication metadata and shared descriptor observations.
#[path = "support/request.rs"]
pub mod request_support;
#[path = "support/common.rs"]
pub mod support;
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
        None
    });
    assert!(registry.has_configured_auth(&model));
    registry.auth_storage().set_fallback_resolver(|_| None);
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
            Some("present".into())
        });
    let selected = registry.get_available();
    assert_eq!(*visits.lock().unwrap(), ["a", "c", "d"]);
    assert_eq!(
        selected
            .iter()
            .map(|m| m.read().unwrap().provider.clone())
            .collect::<Vec<_>>(),
        ["a", "c", "d"]
    );
    registry.auth_storage().set_fallback_resolver(|_| None);
}
#[test]
fn metadata_preserves_sources_without_helper_or_refresh_effects() {
    use maestro_credentials::{AuthSource, AuthStatus};
    use request_support::Operations;
    let operations = Operations::default();
    for (field, source, label) in [
        ("literal", AuthSource::ModelsJsonKey, None),
        ("!helper", AuthSource::ModelsJsonCommand, None),
        ("ENV", AuthSource::Environment, Some("ENV")),
    ] {
        let registry = support::registry(&request_support::document(
            &format!(r#""apiKey":"{field}""#),
            "",
        ));
        operations
            .environment
            .lock()
            .unwrap()
            .insert("ENV".into(), "value".into());
        assert_eq!(
            registry.get_provider_auth_status("p", &operations),
            AuthStatus {
                configured: true,
                source: Some(source),
                label: label.map(str::to_owned)
            }
        );
        assert!(registry.has_configured_auth(&registry.find("p", "m").unwrap()));
        assert_eq!(
            registry
                .get_available()
                .iter()
                .filter(|m| m.read().unwrap().provider == "p")
                .count(),
            1
        );
        lower_layer_status(&registry, &operations);
    }
    let registry = maestro_catalog::ModelRegistry::in_memory(support::auth()).unwrap();
    assert_eq!(
        registry.get_provider_auth_status("unknown-metadata", &operations),
        AuthStatus {
            configured: false,
            source: None,
            label: None
        }
    );
    registry
        .auth_storage()
        .set_fallback_resolver(|_| Some("fallback".into()));
    assert_eq!(
        registry.get_provider_auth_status("unknown-metadata", &operations),
        AuthStatus {
            configured: false,
            source: Some(AuthSource::Fallback),
            label: Some("custom provider config".into())
        }
    );
    registry.auth_storage().set_fallback_resolver(|_| None);
    metadata_empty_environment_and_oauth(&operations);
    assert!(operations.calls.lock().unwrap().is_empty());
}
#[test]
fn display_names_cover_table_oauth_and_identifier_precedence() {
    let registry = maestro_catalog::ModelRegistry::in_memory(support::auth()).unwrap();
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
    let selected = registry.get_available();
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
    if std::env::var_os("MAESTRO_DISPLAY_CHILD").is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "display_table_mutations_are_shared_between_registries",
            ])
            .env("MAESTRO_DISPLAY_CHILD", "1")
            .output()
            .unwrap();
        assert!(output.status.success(), "display subprocess failed");
        return;
    }
    let first = ModelRegistry::in_memory(support::auth()).unwrap();
    let second = ModelRegistry::in_memory(support::auth()).unwrap();
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
fn lower_layer_status(registry: &ModelRegistry, operations: &request_support::Operations) {
    use maestro_credentials::{ApiKeyCredential, AuthCredential, AuthSource, AuthStatus};
    registry.auth_storage().set_runtime_api_key("p", "runtime");
    assert_eq!(
        registry.get_provider_auth_status("p", operations),
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
        registry.get_provider_auth_status("p", operations),
        AuthStatus {
            configured: true,
            source: Some(AuthSource::Stored),
            label: None
        }
    );
}

/// An empty configured environment falls back; complete OAuth metadata does not refresh.
fn metadata_empty_environment_and_oauth(operations: &request_support::Operations) {
    use maestro_credentials::{AuthSource, AuthStatus};
    let registry = support::registry(&request_support::document(r#""apiKey":"ENV""#, ""));
    operations
        .environment
        .lock()
        .unwrap()
        .insert("ENV".into(), String::new());
    assert_eq!(
        registry.get_provider_auth_status("p", operations),
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
    assert!(registry.has_configured_auth(&model));
    assert!(registry.is_using_oauth(&model));
    assert!(
        registry
            .get_available()
            .iter()
            .any(|m| Arc::ptr_eq(m, &model))
    );
    assert_eq!(
        registry.get_provider_auth_status("catalog-metadata-oauth", operations),
        AuthStatus {
            configured: true,
            source: Some(AuthSource::Stored),
            label: None
        }
    );
    metadata_known_environment();
}
/// Isolated native environment metadata keeps the lower-layer configured flag.
fn metadata_known_environment() {
    // The child runs only the metadata test with a controlled environment.
    if std::env::var_os("MAESTRO_METADATA_CHILD").is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "metadata_preserves_sources_without_helper_or_refresh_effects",
            ])
            .env("MAESTRO_METADATA_CHILD", "1")
            .env("OPENAI_API_KEY", "ambient-key")
            .output()
            .unwrap();
        assert!(output.status.success(), "metadata subprocess failed");
        return;
    }
    let registry = ModelRegistry::in_memory(support::auth()).unwrap();
    let status =
        registry.get_provider_auth_status("openai", &request_support::Operations::default());
    assert!(!status.configured);
    assert_eq!(
        status.source,
        Some(maestro_credentials::AuthSource::Environment)
    );
    assert_eq!(status.label.as_deref(), Some("OPENAI_API_KEY"));
    assert!(registry.auth_storage().has_auth("openai"));
}

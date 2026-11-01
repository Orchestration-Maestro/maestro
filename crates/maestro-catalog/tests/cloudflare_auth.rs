//! Cloudflare configuration is checked without executing credential helpers.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::{request_support, support};
use maestro_credentials::AuthCredential;
use serde_json::json;
use std::sync::Arc;

#[test]
fn cloudflare_availability_requires_ids_without_resolving_key() {
    let Some(_) = maestro_test_process::run_per_environment(
        std::thread::current().name().unwrap(),
        [()],
        |()| std::collections::BTreeMap::<String, String>::new(),
    )
    .unwrap() else {
        return;
    };
    let operations = Arc::new(request_support::Operations::default());
    let registry = support::registry_with(
        r#"{"providers":{"cloudflare-ai-gateway":{"apiKey":"!never"}}}"#,
        Arc::clone(&operations) as maestro_catalog::ConfigValues,
    );
    let model = registry
        .get_all()
        .read()
        .unwrap()
        .iter()
        .find(|model| model.read().unwrap().provider == "cloudflare-ai-gateway")
        .cloned()
        .unwrap();
    assert!(!registry.has_configured_auth(&model).unwrap());
    for (env, expected) in [
        (json!({"CLOUDFLARE_ACCOUNT_ID":"account"}), false),
        (
            json!({"CLOUDFLARE_ACCOUNT_ID":"account","CLOUDFLARE_GATEWAY_ID":""}),
            false,
        ),
        (
            json!({"CLOUDFLARE_ACCOUNT_ID":"account","CLOUDFLARE_GATEWAY_ID":"gateway"}),
            true,
        ),
    ] {
        let credential: AuthCredential = serde_json::from_value(json!({
            "type":"api_key", "key":"!never", "env":env,
        }))
        .unwrap();
        registry
            .auth_storage()
            .set("cloudflare-ai-gateway", credential);
        assert_eq!(registry.has_configured_auth(&model).unwrap(), expected);
        assert!(operations.calls.lock().unwrap().is_empty());
    }
}

/// A stored account without its gateway is rejected before compatibility invocation.
#[test]
fn cloudflare_request_auth_names_missing_gateway() {
    let Some(_) = maestro_test_process::run_per_environment(
        std::thread::current().name().unwrap(),
        [()],
        |()| std::collections::BTreeMap::<String, String>::new(),
    )
    .unwrap() else {
        return;
    };
    let registry = support::memory_with(support::forbidden());
    registry.auth_storage().set(
        "cloudflare-ai-gateway",
        serde_json::from_value(json!({
            "type":"api_key", "key":"fixture-key", "env":{"CLOUDFLARE_ACCOUNT_ID":"account"},
        }))
        .unwrap(),
    );
    let model = registry
        .get_all()
        .read()
        .unwrap()
        .iter()
        .find(|model| model.read().unwrap().provider == "cloudflare-ai-gateway")
        .cloned()
        .unwrap();
    let result = request_support::run(registry.get_api_key_and_headers(&model));
    request_support::error(
        result,
        "CLOUDFLARE_GATEWAY_ID is required for provider cloudflare-ai-gateway but is not set.",
    );
}

/// Stored account values materialize built-in and custom descriptors without mutating them.
#[test]
fn cloudflare_auth_materializes_descriptor_without_mutation() {
    let Some(_) = maestro_test_process::run_per_environment(
        std::thread::current().name().unwrap(),
        [()],
        |()| {
            [
                ("CLOUDFLARE_ACCOUNT_ID", "environment-account"),
                ("CLOUDFLARE_GATEWAY_ID", "environment-gateway"),
            ]
            .into_iter()
            .collect()
        },
    )
    .unwrap() else {
        return;
    };
    let operations = Arc::new(request_support::Operations::default());
    operations.command("cloudflare-key", Some("resolved-key"));
    let registry = support::memory_with(operations.clone());
    for (provider, env, expected) in [
        (
            "cloudflare-ai-gateway",
            json!({"CLOUDFLARE_ACCOUNT_ID":"stored-account","CLOUDFLARE_GATEWAY_ID":""}),
            "https://custom.invalid/stored-account/environment-gateway/stored-account",
        ),
        (
            "cloudflare-ai-gateway",
            json!({"CLOUDFLARE_ACCOUNT_ID":"","CLOUDFLARE_GATEWAY_ID":"stored-gateway"}),
            "https://custom.invalid/environment-account/stored-gateway/environment-account",
        ),
        (
            "cloudflare-workers-ai",
            json!({"CLOUDFLARE_ACCOUNT_ID":"stored-account"}),
            "https://custom.invalid/stored-account",
        ),
    ] {
        check_materialized_auth(&registry, &operations, provider, &env, expected);
    }
}

/// Complete account values cannot make an empty stored key available.
#[test]
fn cloudflare_availability_requires_nonempty_key() {
    let Some(_) = maestro_test_process::run_per_environment(
        std::thread::current().name().unwrap(),
        [()],
        |()| std::collections::BTreeMap::<String, String>::new(),
    )
    .unwrap() else {
        return;
    };
    let registry = support::memory_with(support::forbidden());
    registry.auth_storage().set("cloudflare-ai-gateway", serde_json::from_value(json!({
        "type":"api_key", "key":"", "env":{"CLOUDFLARE_ACCOUNT_ID":"account", "CLOUDFLARE_GATEWAY_ID":"gateway"},
    })).unwrap());
    let descriptor = maestro_models::get_models("cloudflare-ai-gateway").remove(0);
    let model = Arc::new(std::sync::RwLock::new(descriptor));
    assert!(!registry.has_configured_auth(&model).unwrap());
}

/// Compare a resolved custom endpoint with its retained descriptor.
fn check_materialized_auth(
    registry: &maestro_catalog::ModelRegistry,
    operations: &request_support::Operations,
    provider: &str,
    env: &serde_json::Value,
    expected: &str,
) {
    registry.auth_storage().set(
        provider,
        serde_json::from_value(json!({
            "type":"api_key", "key":"!cloudflare-key", "env":env,
        }))
        .unwrap(),
    );
    let mut descriptor = maestro_models::get_models(provider).remove(0);
    descriptor.base_url = if provider == "cloudflare-ai-gateway" {
            "https://custom.invalid/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/{CLOUDFLARE_ACCOUNT_ID}"
        } else { "https://custom.invalid/{CLOUDFLARE_ACCOUNT_ID}" }.into();
    let original = descriptor.base_url.clone();
    let model = Arc::new(std::sync::RwLock::new(descriptor));
    assert!(registry.has_configured_auth(&model).unwrap());
    assert!(operations.calls.lock().unwrap().is_empty());
    let maestro_catalog::ResolvedRequestAuth::Ok {
        api_key, base_url, ..
    } = request_support::run(registry.get_api_key_and_headers(&model))
    else {
        panic!("expected resolved auth")
    };
    assert_eq!(api_key.as_deref(), Some("resolved-key"));
    assert_eq!(base_url.as_deref(), Some(expected));
    assert_eq!(model.read().unwrap().base_url, original);
    operations.calls.lock().unwrap().clear();
}

/// Registered OAuth credentials stay available without account lookup or token refresh.
#[test]
fn cloudflare_oauth_availability_defers_ids_and_refresh_until_request() {
    let Some(cases) = maestro_test_process::run_per_environment(
        std::thread::current().name().unwrap(),
        [false, true],
        |&complete| {
            if complete {
                [
                    ("CLOUDFLARE_ACCOUNT_ID", "account"),
                    ("CLOUDFLARE_GATEWAY_ID", "gateway"),
                ]
                .into_iter()
                .collect()
            } else {
                std::collections::BTreeMap::new()
            }
        },
    )
    .unwrap() else {
        return;
    };
    for complete in cases {
        oauth_configuration(complete);
    }
}

/// Observe metadata separately from the later authenticated endpoint resolution.
fn oauth_configuration(complete: bool) {
    let provider = "cloudflare-ai-gateway";
    let registry = support::memory_with(support::forbidden());
    let mut descriptor = maestro_models::get_models(provider).remove(0);
    descriptor.base_url =
        "https://custom.invalid/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}".into();
    let model = Arc::new(std::sync::RwLock::new(descriptor));
    registry
        .auth_storage()
        .set(provider, request_support::tokens(0.0));
    assert!(!registry.has_configured_auth(&model).unwrap());
    let registration = request_support::OAuth::new(provider, "Cloudflare OAuth").register();
    assert!(registry.auth_storage().has_auth(provider).unwrap());
    assert!(registry.has_configured_auth(&model).unwrap());
    registry
        .auth_storage()
        .set(provider, request_support::tokens(f64::MAX));
    let auth = request_support::run(registry.get_api_key_and_headers(&model));
    if complete {
        assert!(
            matches!(auth, maestro_catalog::ResolvedRequestAuth::Ok {base_url:Some(url), ..} if url == "https://custom.invalid/account/gateway")
        );
    } else {
        request_support::error(
            auth,
            "CLOUDFLARE_ACCOUNT_ID is required for provider cloudflare-ai-gateway but is not set.",
        );
    }
    drop(registration);
    assert!(!registry.has_configured_auth(&model).unwrap());
}

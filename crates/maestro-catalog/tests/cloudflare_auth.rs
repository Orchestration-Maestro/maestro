//! Cloudflare configuration is checked without executing credential helpers.
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

//! Resolved Cloudflare authentication reaches all compatibility adapters.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::{chat::TestResult, messages};
use maestro_models::{
    ProviderStreamOptions, SimpleStreamOptions, StreamOptions, Threaded, stream, stream_for,
    stream_simple, stream_simple_for,
};
use serde_json::{Value, json};

/// Both public entry points apply the auth endpoint before invoking bundled adapters.
#[test]
fn cloudflare_resolved_endpoint_reaches_raw_and_simple_compatibility_calls() -> TestResult {
    let Some(_) = maestro_test_process::run_per_environment(
        std::thread::current().name().unwrap(),
        [()],
        |()| std::collections::BTreeMap::<String, String>::new(),
    )?
    else {
        return Ok(());
    };
    crate::runtime::run(async {
        for (api, path) in [
            ("anthropic-messages", "/messages"),
            ("openai-completions", "/chat/completions"),
            ("openai-responses", "/responses"),
        ] {
            for entry in 0..4 {
                compatibility_call(api, path, entry).await?;
            }
        }
        Ok(())
    })
}

/// Observe one adapter through a standard or supplied-spawner invocation.
async fn compatibility_call(api: &str, path: &str, entry: u8) -> TestResult {
    let requests = messages::Requests::default();
    let model = messages::model(
        &json!({"api":api,"provider":"cloudflare-ai-gateway", "baseUrl":"https://unresolved.invalid/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}"}),
    )?;
    let common = StreamOptions {
        api_key: Some("fixture-key".into()),
        base_url: Some("https://resolved.invalid/account/gateway/api".into()),
        fetch: Some(messages::scripted_fetch(
            &messages::Case::default(),
            &requests,
        )),
        max_retries: Some(0.0),
        ..StreamOptions::default()
    };
    let context = messages::context(&Value::Null)?;
    let raw = ProviderStreamOptions {
        common: common.clone(),
        ..ProviderStreamOptions::default()
    };
    let simple = SimpleStreamOptions {
        common,
        ..SimpleStreamOptions::default()
    };
    let submit: maestro_models::ProviderSpawner<Threaded> = std::sync::Arc::new(|producer| {
        drop(tokio::spawn(producer));
        true
    });
    let stream = match entry {
        0 => stream(model, context, Some(raw)),
        1 => stream_simple(model, context, Some(simple)),
        2 => stream_for::<Threaded>(model, context, Some(raw), submit),
        _ => stream_simple_for::<Threaded>(model, context, Some(simple), submit),
    }?;
    let _message = stream.result().await;
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 1, "{api}, entry={entry}");
    assert_eq!(
        requests[0]["url"],
        format!("https://resolved.invalid/account/gateway/api{path}")
    );
    assert_eq!(
        requests[0]["headers"]["cf-aig-authorization"],
        "Bearer fixture-key"
    );
    assert_eq!(requests[0]["headers"]["authorization"], Value::Null);
    assert_eq!(requests[0]["headers"]["x-api-key"], Value::Null);
    Ok(())
}

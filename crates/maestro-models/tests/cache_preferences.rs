//! Controlled cache-preference qualification.

#[cfg(not(target_arch = "wasm32"))]
use crate::cache_overflow_images;
#[cfg(not(target_arch = "wasm32"))]
use crate::chat;

#[cfg(not(target_arch = "wasm32"))]
use cache_overflow_images::assert_rows;
#[cfg(not(target_arch = "wasm32"))]
use chat::TestResult;

#[cfg(not(target_arch = "wasm32"))]
/// Default, explicit and environment retention reach every protocol request.
#[test]
fn maestro_cache_preferences_reach_each_protocol() -> TestResult {
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/provider_local_rules.json"))?;
    let rows: Vec<_> = rows
        .iter()
        .filter(|row| row["kind"] == "retention")
        .collect();
    let case = maestro_test_process::selection();
    if case.is_child() {
        if let Some((_, row)) = rows
            .iter()
            .enumerate()
            .find(|(index, _)| case.runs_case(&format!("shared-retention:{index}")))
        {
            return assert_retention_row(row);
        }
        return assert_rows("maestro_cache_preferences_reach_each_protocol");
    }
    assert_rows("maestro_cache_preferences_reach_each_protocol")?;
    for (index, row) in rows.iter().enumerate() {
        let variables: Vec<_> = row["input"]["environment"]
            .as_str()
            .map(|value| ("MAESTRO_CACHE_RETENTION", value))
            .into_iter()
            .collect();
        {
            let output = maestro_test_process::TestProcess::current(
                "maestro_cache_preferences_reach_each_protocol",
            )?
            .nocapture()
            .env_clear()
            .envs(variables.iter().copied())
            .case(format!("shared-retention:{index}"))
            .output()?;
            assert!(output.status.success(), "{output:?}");
        };
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
/// Retention none removes cache markers and the affinity key.
#[test]
fn maestro_cache_none_omits_affinity_key() -> TestResult {
    assert_rows("maestro_cache_none_omits_affinity_key")
}

#[cfg(not(target_arch = "wasm32"))]
/// Long-retention support and marker format are independent.
#[test]
fn maestro_cache_flags_keep_independent_markers() -> TestResult {
    assert_rows("maestro_cache_flags_keep_independent_markers")
}

#[cfg(not(target_arch = "wasm32"))]
/// Observe retention through raw request construction in the isolated child environment.
fn assert_retention_row(row: &serde_json::Value) -> TestResult {
    use crate::bundled;
    use maestro_models::{ProviderStreamOptions, StreamOptions, stream};
    use serde_json::json;
    let _registry = bundled::registry();
    for api in [
        "openai-completions",
        "anthropic-messages",
        "openai-responses",
    ] {
        maestro_models::reset_api_providers();
        let base_url = if api == "anthropic-messages" {
            "https://api.anthropic.com"
        } else {
            "https://api.openai.com/v1"
        };
        let model = chat::model(
            &json!({"api": api, "provider": "fixture", "id": "controlled", "baseUrl": base_url,
            "compat": {"supportsLongCacheRetention": true}}),
        )?;
        let (fetch, requests) = bundled::fetch(bundled::text_body(api, "ok"));
        let options = ProviderStreamOptions {
            common: StreamOptions {
                api_key: Some(bundled::api_key(api)),
                session_id: Some("session".into()),
                fetch: Some(fetch),
                cache_retention: serde_json::from_value(row["input"]["explicit"].clone())?,
                ..StreamOptions::default()
            },
            ..ProviderStreamOptions::default()
        };
        let result = crate::runtime::run::<crate::TestResult<_>>(async {
            Ok(stream(model, bundled::conversation()?, Some(options))?
                .result()
                .await)
        })?;
        assert!(
            bundled::error_of(&result).is_none(),
            "{api}: request failed"
        );
        let requests = bundled::recorded(&requests);
        assert_eq!(requests.len(), 1);
        assert_retention_payload(
            api,
            &requests[0].body,
            row["expected"].as_str().ok_or("retention")?,
        );
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
/// Check the independently selected marker and affinity policy of each protocol.
fn assert_retention_payload(api: &str, body: &serde_json::Value, expected: &str) {
    use serde_json::json;
    if api == "anthropic-messages" {
        let marker = &body["messages"][0]["content"][0]["cache_control"];
        assert_eq!(
            *marker,
            match expected {
                "none" => serde_json::Value::Null,
                "long" => json!({"type": "ephemeral", "ttl": "1h"}),
                _ => json!({"type": "ephemeral"}),
            }
        );
    } else {
        assert_eq!(
            body["prompt_cache_key"],
            if expected == "none" {
                serde_json::Value::Null
            } else {
                json!("session")
            }
        );
        assert_eq!(
            body["prompt_cache_retention"],
            if expected == "long" {
                json!("24h")
            } else {
                serde_json::Value::Null
            }
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Send one controlled request with `session` and return its recorded body.
fn request_body(api: &str, base_url: &str, session: &str) -> TestResult<serde_json::Value> {
    use crate::bundled;
    use maestro_models::{ProviderStreamOptions, StreamOptions, stream};
    maestro_models::reset_api_providers();
    let model = bundled::descriptor(api, base_url)?;
    let (fetch, requests) = bundled::fetch(bundled::text_body(api, "ok"));
    let options = ProviderStreamOptions {
        common: StreamOptions {
            api_key: Some(bundled::api_key(api)),
            session_id: Some(session.to_owned()),
            fetch: Some(fetch),
            ..StreamOptions::default()
        },
        ..ProviderStreamOptions::default()
    };
    let result = crate::runtime::run::<crate::TestResult<_>>(async {
        Ok(stream(model, bundled::conversation()?, Some(options))?
            .result()
            .await)
    })?;
    let failure = bundled::error_of(&result);
    assert!(failure.is_none(), "{api}: request failed: {failure:?}");
    let requests = bundled::recorded(&requests);
    assert_eq!(requests.len(), 1, "{api}");
    Ok(requests[0].body.clone())
}

#[cfg(not(target_arch = "wasm32"))]
/// A session-derived cache key keeps its first 64 Unicode scalars in every OpenAI-style protocol.
#[test]
fn maestro_prompt_cache_keys_stop_at_64_characters() -> TestResult {
    let _registry = crate::bundled::registry();
    let sixty_three = "a".repeat(63);
    let sessions = [
        ("x".repeat(100), "x".repeat(64)),
        ("y".repeat(64), "y".repeat(64)),
        ("z".repeat(10), "z".repeat(10)),
        (format!("{sixty_three}éé"), format!("{sixty_three}é")),
    ];
    for (api, base_url) in [
        ("openai-completions", "https://api.openai.com/v1"),
        ("openai-responses", "https://api.openai.com/v1"),
        ("openai-codex-responses", "https://controlled.invalid"),
        (AZURE, AZURE_BASE_URL),
    ] {
        for (session, expected) in &sessions {
            let body = request_body(api, base_url, session)?;
            assert_eq!(
                body["prompt_cache_key"],
                serde_json::json!(expected),
                "{api} with a {}-character session",
                session.chars().count()
            );
        }
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
/// Azure Responses asks the service not to keep the response.
#[test]
fn maestro_azure_responses_payload_disables_storage() -> TestResult {
    let _registry = crate::bundled::registry();
    let body = request_body(AZURE, AZURE_BASE_URL, "session")?;
    assert_eq!(body["store"], serde_json::json!(false));
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
/// The Azure Responses protocol identifier.
const AZURE: &str = "azure-openai-responses";

#[cfg(not(target_arch = "wasm32"))]
/// The endpoint the Azure fixtures send to.
const AZURE_BASE_URL: &str = "https://controlled.openai.azure.com/openai/v1";

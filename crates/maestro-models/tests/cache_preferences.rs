//! Controlled cache-preference qualification.

use crate::cache_overflow_images;
use crate::chat;

use cache_overflow_images::assert_rows;
use chat::TestResult;

/// Default, explicit and environment retention reach every protocol request.
#[test]
fn maestro_cache_preferences_reach_each_protocol() -> TestResult {
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/provider_local_rules.json"))?;
    let rows: Vec<_> = rows
        .iter()
        .filter(|row| row["kind"] == "retention")
        .collect();
    if let Ok(case) = std::env::var("MAESTRO_CHILD_CASE") {
        if let Some(index) = case.strip_prefix("shared-retention:") {
            return assert_retention_row(rows[index.parse::<usize>()?]);
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
            .env("MAESTRO_CHILD_CASE", format!("shared-retention:{index}"))
            .output()?;
            assert!(output.status.success(), "{output:?}");
        };
    }
    Ok(())
}

/// Retention none removes cache markers and the affinity key.
#[test]
fn maestro_cache_none_omits_affinity_key() -> TestResult {
    assert_rows("maestro_cache_none_omits_affinity_key")
}

/// Long-retention support and marker format are independent.
#[test]
fn maestro_cache_flags_keep_independent_markers() -> TestResult {
    assert_rows("maestro_cache_flags_keep_independent_markers")
}

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
        let result = chat::block_on(false, async {
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

//! Raw extras and simple settings of each bundled protocol reach the wire as authored.
#![allow(
    dead_code,
    reason = "each test binary uses a subset of the shared support"
)]

#[path = "support/bundled.rs"]
mod bundled;
#[path = "support/chat.rs"]
mod chat;

use bundled::{PROTOCOLS, Protocol, Seen};
use chat::{TestResult, block_on};
use maestro_models::{
    Model, ProviderStreamOptions, SimpleStreamOptions, StreamOptions, ThinkingBudgets,
    ThinkingLevel, reset_api_providers, stream, stream_simple,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// The outcome of one controlled invocation.
struct Sent {
    /// The request that reached the transport, if any.
    request: Option<Seen>,
    /// The terminal error text, if the invocation failed.
    error: Option<String>,
}

/// A description of the descriptor routing to `api`, with `overrides` applied.
fn description(api: &str, overrides: &Value) -> TestResult<maestro_models::Model> {
    let protocol = PROTOCOLS
        .iter()
        .find(|protocol| protocol.api == api)
        .ok_or("protocol")?;
    let mut description =
        json!({"api": api, "baseUrl": protocol.base_url, "id": "m", "provider": "fixture"});
    if let (Some(target), Some(changes)) = (description.as_object_mut(), overrides.as_object()) {
        target.extend(
            changes
                .iter()
                .map(|(key, value)| (key.clone(), value.clone())),
        );
    }
    chat::model(&description)
}

/// Common options with a key, a zero temperature and the recording transport.
fn common(api: &str) -> (StreamOptions, bundled::Requests) {
    let (fetch, requests) = bundled::fetch(bundled::text_body(api, "ok"));
    let options = StreamOptions {
        api_key: Some("controlled-key".into()),
        fetch: Some(fetch),
        temperature: Some(0.0),
        ..StreamOptions::default()
    };
    (options, requests)
}

/// Send `extra` through generic raw dispatch to `api`.
fn sent(api: &str, overrides: &Value, extra: &Value) -> TestResult<Sent> {
    let _registry = bundled::registry();
    sent_while_registered(api, overrides, extra)
}

/// Send `extra` while the caller holds the registry.
fn sent_while_registered(api: &str, overrides: &Value, extra: &Value) -> TestResult<Sent> {
    reset_api_providers();
    let (common, requests) = common(api);
    let options = ProviderStreamOptions {
        common,
        extra: serde_json::from_value(extra.clone())?,
        ..ProviderStreamOptions::default()
    };
    let model = description(api, overrides)?;
    let result = block_on(false, async {
        Ok(stream(model, bundled::conversation()?, Some(options))?
            .result()
            .await)
    })?;
    Ok(Sent {
        request: bundled::recorded(&requests).into_iter().next(),
        error: bundled::error_of(&result),
    })
}

/// One recorded case: an input and the wire facts it must produce.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Row {
    /// Owning test.
    test: String,
    /// Protocol the case is sent to.
    api: String,
    /// Case description.
    label: String,
    /// Descriptor fields overriding the shared model.
    #[serde(default)]
    model: Value,
    /// Raw extras sent.
    extra: Value,
    /// JSON pointers into the body with their required values; null requires an explicit null.
    #[serde(default)]
    wire: BTreeMap<String, Value>,
    /// JSON pointers that must be missing from the body.
    #[serde(default)]
    absent: Vec<String>,
    /// Header names with their required values.
    #[serde(default)]
    headers: BTreeMap<String, String>,
    /// Header names that must be missing.
    #[serde(default)]
    absent_headers: Vec<String>,
    /// Required request URL.
    url: Option<String>,
    /// Required terminal error, with no request sent.
    error: Option<String>,
}

/// Check every recorded case of `test`.
fn assert_rows(test: &str) -> TestResult {
    let rows: Vec<Row> = chat::rows(include_str!("fixtures/bundled_options.json"), test)?
        .into_iter()
        .map(serde_json::from_value)
        .collect::<Result<_, _>>()?;
    assert!(!rows.is_empty(), "{test} has recorded cases");
    for row in rows {
        assert_row(&row)?;
    }
    Ok(())
}

/// Send one case and compare it with its recorded facts.
fn assert_row(row: &Row) -> TestResult {
    let label = format!("{} / {}", row.test, row.label);
    let seen = sent(&row.api, &row.model, &row.extra)?;
    let Some(request) = seen.request else {
        assert_eq!(
            seen.error, row.error,
            "{label}: no request without a recorded error"
        );
        return Ok(());
    };
    assert_eq!(
        row.error, None,
        "{label}: a rejected case reached the transport"
    );
    if let Some(url) = &row.url {
        assert_eq!(&request.url, url, "{label}");
    }
    for (pointer, expected) in &row.wire {
        assert_eq!(
            request.body.pointer(pointer),
            Some(expected),
            "{label} {pointer}"
        );
    }
    for pointer in &row.absent {
        assert_eq!(request.body.pointer(pointer), None, "{label} {pointer}");
    }
    let header = |name: &str| {
        request
            .headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value)
    };
    for (name, expected) in &row.headers {
        assert_eq!(header(name), Some(expected), "{label} header {name}");
    }
    for name in &row.absent_headers {
        assert_eq!(header(name), None, "{label} header {name}");
    }
    Ok(())
}

#[test]
fn maestro_anthropic_extras_reach_requests() -> TestResult {
    assert_rows("maestro_anthropic_extras_reach_requests")
}

#[test]
fn maestro_chat_extras_reach_requests() -> TestResult {
    assert_rows("maestro_chat_extras_reach_requests")
}

#[test]
fn maestro_mistral_extras_reach_requests() -> TestResult {
    assert_rows("maestro_mistral_extras_reach_requests")
}

#[test]
fn maestro_response_extras_keep_presence() -> TestResult {
    assert_rows("maestro_response_extras_keep_presence")
}

#[test]
fn maestro_azure_extras_select_target() -> TestResult {
    assert_rows("maestro_azure_extras_select_target")
}

/// Settings every simple route receives.
fn simple_settings(
    api: &str,
    reasoning: Option<ThinkingLevel>,
) -> (SimpleStreamOptions, bundled::Requests) {
    let (mut common, requests) = common(api);
    common.max_tokens = Some(3000.0);
    common.temperature = Some(0.3);
    let options = SimpleStreamOptions {
        common,
        reasoning,
        thinking_budgets: Some(ThinkingBudgets {
            high: Some(2000.0),
            ..ThinkingBudgets::default()
        }),
        ..SimpleStreamOptions::default()
    };
    (options, requests)
}

/// Wire facts each protocol's simple policy yields for the shared settings.
fn simple_facts(api: &str, high: bool) -> Vec<(&'static str, Option<Value>)> {
    fn pick<T>(high: bool, off: T, on: T) -> T {
        if high { on } else { off }
    }
    match api {
        "anthropic-messages" => vec![
            (
                "/thinking/type",
                Some(json!(pick(high, "disabled", "enabled"))),
            ),
            (
                "/thinking/budget_tokens",
                pick(high, None, Some(json!(2000))),
            ),
            ("/temperature", pick(high, Some(json!(0.3)), None)),
        ],
        "openai-completions" => vec![
            ("/reasoning_effort", pick(high, None, Some(json!("high")))),
            ("/max_completion_tokens", Some(json!(3000))),
            ("/temperature", Some(json!(0.3))),
        ],
        "mistral-conversations" => vec![
            ("/prompt_mode", pick(high, None, Some(json!("reasoning")))),
            ("/max_tokens", Some(json!(3000))),
            ("/temperature", Some(json!(0.3))),
        ],
        _ => vec![
            ("/reasoning/effort", Some(json!(pick(high, "none", "high")))),
            ("/max_output_tokens", Some(json!(3000))),
            ("/temperature", Some(json!(0.3))),
        ],
    }
}

/// Start the typed root simple entry of `api`.
fn root_simple(
    api: &str,
    model: maestro_models::Model,
    options: SimpleStreamOptions,
) -> TestResult<maestro_models::AssistantMessageEventStream> {
    let context = bundled::conversation()?;
    let options = Some(options);
    Ok(match api {
        "anthropic-messages" => maestro_models::stream_simple_anthropic(model, context, options),
        "openai-completions" => {
            maestro_models::stream_simple_openai_completions(model, context, options)
        }
        "mistral-conversations" => maestro_models::stream_simple_mistral(model, context, options),
        "openai-responses" => {
            maestro_models::stream_simple_openai_responses(model, context, options)
        }
        _ => maestro_models::stream_simple_azure_openai_responses(model, context, options),
    })
}

/// Send the shared simple settings through the generic and the typed root route.
fn simple_requests(
    protocol: &Protocol,
    reasoning: Option<ThinkingLevel>,
    model: &Model,
) -> TestResult<(Seen, Seen)> {
    let (generic_options, generic_requests) = simple_settings(protocol.api, reasoning);
    let (root_options, root_requests) = simple_settings(protocol.api, reasoning);
    block_on(false, async {
        let context = bundled::conversation()?;
        stream_simple(model.clone(), context, Some(generic_options))?
            .result()
            .await;
        root_simple(protocol.api, model.clone(), root_options)?
            .result()
            .await;
        Ok(())
    })?;
    let first = |requests: &bundled::Requests| bundled::recorded(requests).into_iter().next();
    Ok((
        first(&generic_requests).ok_or("generic request")?,
        first(&root_requests).ok_or("root request")?,
    ))
}

#[test]
fn maestro_simple_dispatch_keeps_adapter_policy() -> TestResult {
    let _registry = bundled::registry();
    reset_api_providers();
    let overrides =
        json!({"reasoning": true, "id": "claude-sonnet-4-5", "thinkingLevelMap": {"xhigh": null}});
    for protocol in &PROTOCOLS {
        for reasoning in [None, Some(ThinkingLevel::High)] {
            let model = description(protocol.api, &overrides)?;
            let (generic, root) = simple_requests(protocol, reasoning, &model)?;
            let label = format!("{} {reasoning:?}", protocol.api);
            assert_eq!(generic.body, root.body, "{label}");
            for (pointer, expected) in simple_facts(protocol.api, reasoning.is_some()) {
                assert_eq!(
                    generic.body.pointer(pointer),
                    expected.as_ref(),
                    "{label} {pointer}"
                );
            }
        }
    }
    let chat = &PROTOCOLS[1];
    let (clamped, _) = simple_requests(
        chat,
        Some(ThinkingLevel::Xhigh),
        &description(chat.api, &overrides)?,
    )?;
    assert_eq!(
        clamped.body["reasoning_effort"], "high",
        "simple effort is clamped to the model's levels"
    );
    let raw = sent_while_registered(chat.api, &overrides, &json!({"reasoningEffort": "xhigh"}))?;
    assert_eq!(
        raw.request.ok_or("raw request")?.body["reasoning_effort"],
        "xhigh",
        "raw effort is not clamped"
    );
    Ok(())
}

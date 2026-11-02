//! Public option conversion and preparation order.

use super::super::*;
use crate::tests::runtime::run;
use crate::{ModelThinkingLevel, SimpleStreamOptions, StreamOptions};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;

/// Strict option inputs retained by the controlled corpus.
#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Options {
    /// Requested simple effort.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reasoning: Option<crate::ThinkingLevel>,
    /// Requested raw effort, including none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<String>,
    /// Temperature override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    /// Token override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max_tokens: Option<f64>,
    /// Configured transport.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    transport: Option<crate::Transport>,
    /// Cache choice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cache_retention: Option<crate::CacheRetention>,
    /// Session identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
    /// Authored header layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    headers: Option<indexmap::IndexMap<String, String>>,
    /// Common timeout forwarded unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    timeout_ms: Option<f64>,
    /// Common retry count forwarded unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max_retries: Option<f64>,
    /// Common retry delay forwarded unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max_retry_delay_ms: Option<f64>,
    /// Ordered open metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    metadata: Option<crate::JsonObject>,
}
impl Options {
    /// Reconstruct the typed common options.
    fn common(&self) -> StreamOptions {
        StreamOptions {
            temperature: self.temperature,
            max_tokens: self.max_tokens,
            transport: self.transport.clone(),
            cache_retention: self.cache_retention,
            session_id: self.session_id.clone(),
            headers: self.headers.clone(),
            timeout_ms: self.timeout_ms,
            max_retries: self.max_retries,
            max_retry_delay_ms: self.max_retry_delay_ms,
            metadata: self.metadata.clone(),
            ..Default::default()
        }
    }
}

/// Descriptor-dependent raw/simple query.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// Options supplied to the public entry point.
    options: Options,
    /// Descriptor capability.
    #[serde(default)]
    model_reasoning: Option<bool>,
    /// Per-level spellings and disabled mappings.
    #[serde(default)]
    thinking_level_map: Option<crate::ThinkingLevelMap>,
    /// Token default input.
    #[serde(default)]
    model_max_tokens: Option<f64>,
}

/// A whole expected request and optional converted common record.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// Ordered wire body.
    #[serde(default)]
    body: Option<Value>,
    /// Base settings from the shared converter.
    #[serde(default)]
    base: Option<Value>,
}
/// One strict query/result fixture.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Full query.
    input: Input,
    /// Independently recorded result.
    expected: Expected,
}

/// Controlled descriptor and context matching the fixed corpus operands.
pub(super) fn corpus_setup() -> (crate::Model, crate::Context, OpenAICodexResponsesOptions) {
    let (model, _, options) = super::invocation();
    let mut model = (*model).clone();
    model.id = "gpt-5.5".into();
    model.provider = "fixture-no-key".into();
    model.cost.cache_read = 0.25;
    model.cost.cache_write = 0.0;
    let context = serde_json::from_value(
        json!({"messages":[{"role":"user","content":"Say Ω😀","timestamp":1}]}),
    )
    .unwrap();
    (model, context, options)
}

/// Observe the complete ordered body through the supplied HTTP transport.
pub(super) async fn body(
    model: crate::Model,
    context: crate::Context,
    mut options: OpenAICodexResponsesOptions,
    simple: Option<crate::ThinkingLevel>,
) -> Value {
    options.common.transport = Some(crate::Transport::Sse);
    let (sender, receiver) = std::sync::mpsc::channel();
    options.common.fetch = Some(Arc::new(move |request| {
        sender.send(request.body).unwrap();
        Box::pin(async { Ok(super::public_streams::response()) })
    }));
    let stream = if let Some(reasoning) = simple {
        stream_simple_openai_codex_responses(
            model,
            context,
            Some(SimpleStreamOptions {
                common: options.common,
                reasoning: Some(reasoning),
                ..Default::default()
            }),
        )
        .unwrap()
    } else {
        stream_openai_codex_responses(model, context, Some(options))
    };
    assert_eq!(
        stream.result().await.read().unwrap().stop_reason,
        crate::StopReason::Stop
    );
    serde_json::from_slice(&receiver.recv().unwrap()).unwrap()
}

/// Decode a requested effort without adding an option deserializer to the provider.
fn effort(text: &str) -> ModelThinkingLevel {
    if text == "none" {
        ModelThinkingLevel::Off
    } else {
        serde_json::from_value(json!(text)).unwrap()
    }
}

/// Compare the full wire request rather than only its reasoning field.
async fn assert_case(row: Case, simple: bool) {
    let (mut model, context, mut options) = corpus_setup();
    model.reasoning = row.input.model_reasoning.unwrap_or(true);
    model.thinking_level_map = row.input.thinking_level_map;
    if let Some(limit) = row.input.model_max_tokens {
        model.max_tokens = limit;
    }
    let key = options.common.api_key.take();
    options.common = row.input.options.common();
    options.common.api_key = key;
    options.reasoning_effort = row.input.options.reasoning_effort.as_deref().map(effort);
    let simple_options = SimpleStreamOptions {
        common: options.common.clone(),
        reasoning: row.input.options.reasoning,
        ..Default::default()
    };
    if let Some(expected) = row.expected.base {
        assert_base(&model, &simple_options, &options, expected);
    }
    if let Some(expected) = row.expected.body {
        // A missing simple effort still goes through the simple wrapper.
        let requested = if simple {
            row.input.options.reasoning
        } else {
            None
        };
        let actual = if simple && requested.is_none() {
            simple_body(model, context, simple_options.common).await
        } else {
            body(model, context, options, requested).await
        };
        assert_eq!(
            crate::providers::json_text::compact_json(&actual).unwrap(),
            crate::providers::json_text::compact_json(&expected).unwrap()
        );
    }
}

#[test]
fn maestro_response_sessions_simple_reasoning() {
    run(maestro_response_sessions_simple_reasoning_scenario());
}

/// Execute the controlled public-operation scenario.
async fn maestro_response_sessions_simple_reasoning_scenario() {
    let rows: Vec<Case> =
        super::fixture_rows(include_str!("fixtures/public-simple.json"), &["input"]).unwrap();
    assert_eq!(rows.len(), 48);
    for row in rows {
        assert_case(row, true).await;
    }
}

/// One body query outside the descriptor-dependent effort corpus.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct VariantInput {
    /// Explicit effort.
    #[serde(default)]
    reasoning_effort: Option<String>,
    /// Null or named summary.
    #[serde(default)]
    reasoning_summary: Option<String>,
    /// Preserve explicit null with the same presence decoder as request fixtures.
    #[serde(default, deserialize_with = "super::requests::present")]
    service_tier: Option<Value>,
    /// Named verbosity.
    #[serde(default)]
    text_verbosity: Option<String>,
}
/// One complete expected body.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VariantCase {
    /// Typed query.
    input: VariantInput,
    /// Full output.
    expected: Expected,
}

#[test]
fn maestro_response_sessions_raw_options_and_nulls() {
    run(maestro_response_sessions_raw_options_and_nulls_scenario());
}

/// Execute the controlled public-operation scenario.
async fn maestro_response_sessions_raw_options_and_nulls_scenario() {
    for text in [
        include_str!("fixtures/public-raw.json"),
        include_str!("fixtures/public-raw-nonreasoning.json"),
    ] {
        let rows: Vec<Case> = super::fixture_rows(text, &["input"]).unwrap();
        for row in rows {
            assert_case(row, false).await;
        }
    }
    for text in [
        include_str!("fixtures/public-summary.json"),
        include_str!("fixtures/public-tier.json"),
        include_str!("fixtures/public-verbosity.json"),
    ] {
        let rows: Vec<VariantCase> = super::fixture_rows(text, &["input"]).unwrap();
        for row in rows {
            let (model, context, mut options) = corpus_setup();
            options.reasoning_effort = row.input.reasoning_effort.as_deref().map(effort);
            options.reasoning_summary =
                row.input
                    .reasoning_summary
                    .as_deref()
                    .map(|text| match text {
                        "auto" => OpenAICodexReasoningSummary::Auto,
                        "concise" => OpenAICodexReasoningSummary::Concise,
                        "detailed" => OpenAICodexReasoningSummary::Detailed,
                        "off" => OpenAICodexReasoningSummary::Off,
                        "on" => OpenAICodexReasoningSummary::On,
                        _ => panic!("invalid summary fixture"),
                    });
            options.service_tier = row
                .input
                .service_tier
                .map(|value| serde_json::from_value(value).unwrap());
            options.text_verbosity = row.input.text_verbosity.as_deref().map(|text| match text {
                "low" => OpenAICodexTextVerbosity::Low,
                "medium" => OpenAICodexTextVerbosity::Medium,
                "high" => OpenAICodexTextVerbosity::High,
                _ => panic!("invalid verbosity fixture"),
            });
            let actual = body(model, context, options, None).await;
            assert!(row.expected.base.is_none());
            assert_eq!(
                crate::providers::json_text::compact_json(&actual).unwrap(),
                crate::providers::json_text::compact_json(&row.expected.body.unwrap()).unwrap()
            );
        }
    }
    for id in ["gpt-5.3-codex", "gpt-5.4", "gpt-5.5"] {
        for mapped in [false, true] {
            minimal_case(id, mapped).await;
        }
    }
}

#[test]
fn maestro_response_sessions_simple_common_options() {
    run(maestro_response_sessions_simple_common_options_scenario());
}

/// Execute the controlled public-operation scenario.
async fn maestro_response_sessions_simple_common_options_scenario() {
    let rows: Vec<Case> = super::fixture_rows(
        include_str!("fixtures/public-base-default.json"),
        &["input"],
    )
    .unwrap();
    assert_eq!(rows.len(), 6);
    for row in rows {
        assert_case(row, true).await;
    }
    let (model, context, mut options) = corpus_setup();
    let (sender, receiver) = std::sync::mpsc::channel();
    let payload_seen = sender.clone();
    options.common.transport = Some(crate::Transport::Sse);
    options.common.session_id = Some("simple-common".into());
    options.common.headers = Some(indexmap::IndexMap::from([(
        "x-common".into(),
        "forwarded".into(),
    )]));
    options.common.on_payload = Some(Arc::new(move |mut input, _| {
        payload_seen.send("payload").unwrap();
        input.payload["edited"] = json!([2, 1, 2]);
        Box::pin(async { Ok(input) })
    }));
    let response_seen = sender.clone();
    options.common.on_response = Some(Arc::new(move |response, _| {
        assert_eq!(response.status.to_bits(), 200.0_f64.to_bits());
        response_seen.send("response").unwrap();
        Box::pin(async { Ok(()) })
    }));
    options.common.fetch = Some(Arc::new(move |request| {
        sender.send("fetch").unwrap();
        assert_common_request(&request);
        Box::pin(async { Ok(super::public_streams::response()) })
    }));
    let stream = stream_simple_openai_codex_responses(
        model,
        context,
        Some(SimpleStreamOptions {
            common: options.common,
            ..Default::default()
        }),
    )
    .unwrap();
    assert_eq!(
        stream.result().await.read().unwrap().stop_reason,
        crate::StopReason::Stop
    );
    assert_eq!(
        receiver.try_iter().collect::<Vec<_>>(),
        ["payload", "fetch", "response"]
    );
}

#[test]
fn maestro_response_sessions_simple_signal_reaches_execution() {
    run(async {
        let (model, context, mut options) = corpus_setup();
        let signal = crate::Cancellation::new();
        signal.abort();
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = Arc::clone(&calls);
        options.common.signal = Some(signal);
        options.common.fetch = Some(Arc::new(move |_| {
            count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Box::pin(async { Ok(super::public_streams::response()) })
        }));
        let output = stream_simple_openai_codex_responses(
            model,
            context,
            Some(SimpleStreamOptions {
                common: options.common,
                ..Default::default()
            }),
        )
        .unwrap()
        .result()
        .await;
        assert_eq!(
            output.read().unwrap().stop_reason,
            crate::StopReason::Aborted
        );
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    });
}

#[test]
fn maestro_response_sessions_preparation_order() {
    run(maestro_response_sessions_preparation_order_scenario());
}

/// Execute the controlled public-operation scenario.
async fn maestro_response_sessions_preparation_order_scenario() {
    for (failure, aborted) in [
        (PreparationFailure::Account, false),
        (PreparationFailure::Hook, false),
        (PreparationFailure::Header, false),
        (PreparationFailure::Header, true),
    ] {
        preparation_case(failure, aborted).await;
    }
}

/// Session affinity query.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AffinityInput {
    /// Session, including empty.
    session_id: Option<String>,
    /// Whether caller supplies both affinity fields.
    supplied: bool,
}
/// Full body and the ordered selected header fields.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AffinityExpected {
    /// Whole request.
    body: Value,
    /// Selected header entries.
    headers: Vec<(String, String)>,
}
/// One affinity fixture.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AffinityCase {
    /// Query.
    input: AffinityInput,
    /// Public output.
    expected: AffinityExpected,
}

#[test]
fn maestro_response_sessions_forward_session_affinity() {
    run(maestro_response_sessions_forward_session_affinity_scenario());
}

/// Execute the controlled public-operation scenario.
async fn maestro_response_sessions_forward_session_affinity_scenario() {
    let rows: Vec<AffinityCase> =
        super::fixture_rows(include_str!("fixtures/public-affinity.json"), &["input"]).unwrap();
    assert_eq!(rows.len(), 6);
    for row in rows {
        let (model, context, mut options) = corpus_setup();
        options.common.transport = Some(crate::Transport::Sse);
        options.common.session_id = row.input.session_id;
        if row.input.supplied {
            options.common.headers = Some(indexmap::IndexMap::from([
                ("session_id".into(), "provided-session".into()),
                ("x-client-request-id".into(), "provided-request".into()),
            ]));
        }
        let (sender, receiver) = std::sync::mpsc::channel();
        options.common.fetch = Some(Arc::new(move |request| {
            let fields: Vec<_> = request
                .headers
                .into_iter()
                .filter(|(key, _)| key == "session_id" || key == "x-client-request-id")
                .collect();
            sender.send((request.body, fields)).unwrap();
            Box::pin(async { Ok(super::public_streams::response()) })
        }));
        let stream = stream_openai_codex_responses(model, context, Some(options));
        assert_eq!(
            stream.result().await.read().unwrap().stop_reason,
            crate::StopReason::Stop
        );
        let (body, headers) = receiver.recv().unwrap();
        assert_eq!(
            String::from_utf8(body).unwrap(),
            crate::providers::json_text::compact_json(&row.expected.body).unwrap()
        );
        assert!(headers == row.expected.headers, "affinity fields differ");
    }
}

/// Check the actual typed consumer rejects an added input member or a duplicate query.
fn strict_fixture<T: serde::de::DeserializeOwned>(text: &str) -> usize {
    let count = super::fixture_rows::<T>(text, &["input"]).unwrap().len();
    let mut rows: Value = serde_json::from_str(text).unwrap();
    rows[0]["input"]["unread"] = json!(true);
    assert!(super::fixture_rows::<T>(&rows.to_string(), &["input"]).is_err());
    let mut rows: Value = serde_json::from_str(text).unwrap();
    let repeated = rows[0].clone();
    rows.as_array_mut().unwrap().push(repeated);
    assert!(super::fixture_rows::<T>(&rows.to_string(), &["input"]).is_err());
    count
}

#[test]
fn maestro_response_sessions_dispatch_fixtures_are_consumed() {
    let mut count = 0;
    for text in [
        include_str!("fixtures/public-simple.json"),
        include_str!("fixtures/public-raw.json"),
        include_str!("fixtures/public-raw-nonreasoning.json"),
        include_str!("fixtures/public-base-default.json"),
    ] {
        count += strict_fixture::<Case>(text);
    }
    for text in [
        include_str!("fixtures/public-summary.json"),
        include_str!("fixtures/public-tier.json"),
        include_str!("fixtures/public-verbosity.json"),
    ] {
        count += strict_fixture::<VariantCase>(text);
    }
    count += strict_fixture::<AffinityCase>(include_str!("fixtures/public-affinity.json"));
    count += strict_fixture::<super::public_streams::PriceCase>(include_str!(
        "fixtures/public-price.json"
    ));
    count += strict_fixture::<super::dispatch::DiagnosticCase>(include_str!(
        "fixtures/public-diagnostic.json"
    ));
    assert_eq!(count, 118);
}

/// Observe competing preparation failures in their public order.
async fn preparation_case(failure: PreparationFailure, aborted: bool) {
    let invalid_key = matches!(failure, PreparationFailure::Account);
    let hook_error = matches!(failure, PreparationFailure::Hook);
    let (mut model, context, mut options) = corpus_setup();
    model.headers = Some(indexmap::IndexMap::from([(
        "x-bad".into(),
        "broken\nvalue".into(),
    )]));
    options.common.headers = Some(indexmap::IndexMap::from([(
        "x-bad".into(),
        "replacement".into(),
    )]));
    options.common.transport = Some(crate::Transport::Sse);
    let signal = crate::Cancellation::new();
    if aborted {
        signal.abort();
    }
    options.common.signal = Some(signal);
    if invalid_key {
        options.common.api_key = Some("not-an-account".into());
    }
    let (sender, receiver) = std::sync::mpsc::channel();
    let hook_seen = sender.clone();
    options.common.on_payload = Some(Arc::new(move |body, _| {
        hook_seen.send("payload").unwrap();
        Box::pin(async move {
            if hook_error {
                Err(super::super::request::diagnostic("hook failed first"))
            } else {
                Ok(body)
            }
        })
    }));
    options.common.fetch = Some(Arc::new(move |_| {
        sender.send("fetch").unwrap();
        Box::pin(async { Ok(super::public_streams::response()) })
    }));
    let stream = stream_openai_codex_responses(model, context, Some(options));
    let output = stream.result().await;
    assert_preparation(&output, &receiver, failure, aborted);
}

/// Keep mapped and unmapped minimal efforts distinct.
async fn minimal_case(id: &str, mapped: bool) {
    let (mut model, context, mut options) = corpus_setup();
    model.id = id.into();
    if mapped {
        model.thinking_level_map = Some(crate::ThinkingLevelMap::from([(
            ModelThinkingLevel::Minimal,
            Some("low".into()),
        )]));
    }
    options.reasoning_effort = Some(ModelThinkingLevel::Minimal);
    let actual = body(model, context, options, None).await;
    assert_eq!(
        actual["reasoning"]["effort"],
        if mapped { "low" } else { "minimal" }
    );
}

/// Inspect every recorded common field on the converter result.
fn assert_base(
    model: &crate::Model,
    simple_options: &SimpleStreamOptions,
    options: &OpenAICodexResponsesOptions,
    expected: Value,
) {
    let context = serde_json::from_value(json!({"messages": []})).unwrap();
    let base = crate::build_base_options(
        model,
        &context,
        Some(simple_options),
        options.common.api_key.as_deref(),
    );
    let mut actual = json!({});
    for (name, value) in [
        ("temperature", json!(base.temperature)),
        ("maxTokens", json!(base.max_tokens)),
        ("transport", json!(base.transport)),
        ("cacheRetention", json!(base.cache_retention)),
        ("sessionId", json!(base.session_id)),
        ("headers", json!(base.headers)),
        ("timeoutMs", json!(base.timeout_ms)),
        ("maxRetries", json!(base.max_retries)),
        ("maxRetryDelayMs", json!(base.max_retry_delay_ms)),
        ("metadata", json!(base.metadata)),
    ] {
        if !value.is_null() {
            actual[name] = value;
        }
    }
    assert!(
        base.api_key
            .as_deref()
            .zip(options.common.api_key.as_deref())
            .is_some_and(|(left, right)| left == right),
        "key forwarding differs"
    );
    assert!(
        expected["apiKey"]
            .as_str()
            .is_some_and(|value| value == "<synthetic-account-key>"),
        "synthetic key marker differs"
    );
    let mut expected = expected;
    expected.as_object_mut().unwrap().shift_remove("apiKey");
    for (name, value) in expected.as_object().unwrap() {
        assert!(
            crate::providers::json_text::compact_json(&actual[name]).unwrap()
                == crate::providers::json_text::compact_json(value).unwrap(),
            "common field {name} differs"
        );
    }
    assert_eq!(
        actual.as_object().unwrap().len(),
        expected.as_object().unwrap().len()
    );
}

/// Observe a simple invocation with omitted effort.
async fn simple_body(
    model: crate::Model,
    context: crate::Context,
    mut common: StreamOptions,
) -> Value {
    let (sender, receiver) = std::sync::mpsc::channel();
    common.transport = Some(crate::Transport::Sse);
    common.fetch = Some(Arc::new(move |request| {
        sender.send(request.body).unwrap();
        Box::pin(async { Ok(super::public_streams::response()) })
    }));
    let stream = stream_simple_openai_codex_responses(
        model,
        context,
        Some(SimpleStreamOptions {
            common,
            ..Default::default()
        }),
    )
    .unwrap();
    assert_eq!(
        stream.result().await.read().unwrap().stop_reason,
        crate::StopReason::Stop
    );
    serde_json::from_slice(&receiver.recv().unwrap()).unwrap()
}

/// The preparation operation selected to fail.
#[derive(Clone, Copy)]
enum PreparationFailure {
    /// Credential extraction.
    Account,
    /// Payload callback.
    Hook,
    /// Initial header validation.
    Header,
}

/// Check the winning failure and the closed callback observation set.
fn assert_preparation(
    output: &crate::SharedAssistantMessage,
    receiver: &std::sync::mpsc::Receiver<&str>,
    failure: PreparationFailure,
    aborted: bool,
) {
    let invalid_key = matches!(failure, PreparationFailure::Account);
    let hook_error = matches!(failure, PreparationFailure::Hook);
    let output = output.read().unwrap();
    let expected = if invalid_key {
        "Failed to extract accountId from token"
    } else if hook_error {
        "hook failed first"
    } else {
        "Header x-bad holds NUL or a line break."
    };
    assert!(
        output
            .error_message
            .as_deref()
            .unwrap()
            .starts_with(expected)
    );
    assert_eq!(
        output.stop_reason,
        if aborted {
            crate::StopReason::Aborted
        } else {
            crate::StopReason::Error
        }
    );
    assert_eq!(
        receiver.try_iter().collect::<Vec<_>>(),
        if invalid_key { vec![] } else { vec!["payload"] }
    );
}

/// Inspect common options without rendering any header value in a failure.
fn assert_common_request(request: &crate::HttpRequest) {
    assert!(
        request
            .headers
            .get("x-common")
            .is_some_and(|value| value == "forwarded"),
        "common header differs"
    );
    assert!(
        request
            .headers
            .get("session_id")
            .is_some_and(|value| value == "simple-common"),
        "session affinity differs"
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&request.body).unwrap()["edited"],
        json!([2, 1, 2])
    );
}

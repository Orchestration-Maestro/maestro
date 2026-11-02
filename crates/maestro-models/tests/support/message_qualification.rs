//! Recorded message requests observed through public dispatch.

#![cfg(not(target_arch = "wasm32"))]
use crate::{
    bundled, chat,
    json::{canonical, event_record, exact_context},
    message_probes as probes, messages,
};
use chat::TestResult;
use maestro_models::{
    CacheRetention, Context, Model, ProviderStreamOptions, SimpleStreamOptions, StreamOptions,
    ThinkingLevel, complete, get_model, stream, stream_simple,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};

/// Recorded operands and independent expected observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    /// Unique scenario label.
    id: String,
    /// Owning integration test.
    test: String,
    /// Public operation operands.
    input: Input,
    /// Complete retained observation tree.
    expected: Value,
}

/// Catalog, selection or wire invocation operands.
#[derive(Deserialize)]
#[serde(untagged)]
enum Input {
    /// Injected message protocol response.
    Wire(Box<Wire>),
    /// Pure source-owned probe setup.
    Probe(Probe),
}

/// Pure setup operation, interpreted by the probe helper.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Probe {
    /// Named setup operation.
    operation: Operation,
    /// Fixed selector cases when requested.
    cases: Option<Vec<probes::Case>>,
}

/// Source-owned setup discriminants.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum Operation {
    /// Enumerate catalog candidates.
    CatalogCandidates,
    /// Select distinguishing fixed cases.
    SelectProviderProbes,
}

/// One real adapter invocation.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Wire {
    /// Effective descriptor compared with the real catalog.
    model: Value,
    /// Only the explicit scenario compatibility override.
    #[serde(deserialize_with = "exact_patch", default)]
    model_patch: Option<Patch>,
    /// Caller conversation with lossless decode validation.
    #[serde(deserialize_with = "exact_context")]
    context: Context,
    /// Typed caller options.
    options: Options,
    /// Complete protocol envelope emitted into named SSE frames.
    events: Vec<Value>,
    /// Public operation used by this scenario.
    route: Route,
    /// Count whole-word pong occurrences.
    pongs: Option<bool>,
    /// Apply the source smoke assertion's trimming.
    smoke_text: Option<bool>,
}

/// Explicit compatibility scenario patch.
#[derive(Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct Patch {
    /// Message protocol compatibility fields.
    compat: maestro_models::AnthropicMessagesCompat,
}

/// Typed invocation options; provider-only fields enter the existing open record.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Options {
    /// Synthetic credential, never rendered in a failure message.
    api_key: String,
    /// Simple reasoning policy.
    reasoning: Option<ThinkingLevel>,
    /// Explicit output token budget.
    max_tokens: Option<f64>,
    /// Explicit sampling temperature.
    temperature: Option<f64>,
    /// Caller cache policy.
    cache_retention: Option<CacheRetention>,
    /// Raw thinking enablement.
    thinking_enabled: Option<bool>,
    /// Independent interleaved beta control.
    interleaved_thinking: Option<bool>,
    /// Raw effort preserves its selected spelling.
    effort: Option<maestro_models::AnthropicEffort>,
    /// Raw thinking presentation.
    thinking_display: Option<maestro_models::AnthropicThinkingDisplay>,
}

/// Public operation route.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum Route {
    /// Full-options stream.
    Stream,
    /// Simple-options stream.
    StreamSimple,
    /// Eagerly observed full-options completion.
    Complete,
}

/// Validate strict corpus ownership and semantic uniqueness before selecting rows.
fn rows(test: &str) -> TestResult<Vec<Row>> {
    let source = include_str!("../fixtures/message_qualification.json");
    let raw: Vec<Value> = serde_json::from_str(source)?;
    let mut ids = HashSet::new();
    let mut inputs = HashSet::new();
    let mut counts = BTreeMap::new();
    for row in &raw {
        assert!(ids.insert(row["id"].to_string()), "duplicate row id");
        let mut query = row["input"].clone();
        for field in ["pongs", "smokeText"] {
            query
                .as_object_mut()
                .ok_or("input object")?
                .shift_remove(field);
        }
        assert!(
            inputs.insert(canonical(query).to_string()),
            "duplicate query"
        );
        let owner = row["test"].as_str().ok_or("owner")?;
        assert!(OWNERS.iter().any(|(name, _)| *name == owner), "unowned row");
        *counts.entry(owner).or_insert(0) += 1;
    }
    for (owner, count) in OWNERS {
        assert_eq!(counts.get(owner).copied(), Some(count), "{owner} rows");
    }
    let decoded: Vec<Row> = serde_json::from_str(source)?;
    let selected: Vec<_> = decoded
        .into_iter()
        .filter(|row| row.test.rsplit("::").next() == Some(test))
        .collect();
    assert!(!selected.is_empty(), "test owns no inputs");
    Ok(selected)
}

/// Run only this test's closed set in an environment-isolated process.
pub fn assert_rows(test: &str) -> TestResult {
    let owned = rows(test)?;
    if !maestro_test_process::selection().is_child() {
        return {
            let output = maestro_test_process::TestProcess::current(test)?
                .nocapture()
                .env_clear()
                .envs(
                    [
                        ("CLOUDFLARE_ACCOUNT_ID", "account-id"),
                        ("CLOUDFLARE_GATEWAY_ID", "gateway-id"),
                    ]
                    .iter()
                    .copied(),
                )
                .case("qualification")
                .output()?;
            assert!(output.status.success(), "{output:?}");
            Ok(())
        };
    }
    let _registry = bundled::registry();
    maestro_models::reset_api_providers();
    crate::runtime::run(async {
        for row in owned {
            let actual = match &row.input {
                Input::Wire(wire) => invoke(wire, &row.id, &row.expected).await?,
                Input::Probe(probe) => probe_output(probe)?,
            };
            compare(&row.id, actual, row.expected)?;
        }
        Ok(())
    })
}

/// Compare every retained observation without exposing credential-bearing values.
fn compare(id: &str, actual: Value, expected: Value) -> TestResult {
    let actual = canonical(actual);
    let expected = canonical(expected);
    let fields = expected.as_object().ok_or("expected object")?;
    assert_eq!(
        actual.as_object().ok_or("actual object")?.len(),
        fields.len(),
        "{id} observation fields"
    );
    for (field, value) in fields {
        assert!(actual.get(field) == Some(value), "{id} {field} mismatch");
    }
    Ok(())
}

/// Pure setup is implemented in the test-owned selector module.
fn probe_output(probe: &Probe) -> TestResult<Value> {
    match probe.operation {
        Operation::CatalogCandidates => {
            assert!(probe.cases.is_none(), "catalog has fixed cases");
            probes::catalog()
        }
        Operation::SelectProviderProbes => {
            probes::fixed(probe.cases.as_deref().ok_or("fixed cases")?)
        }
    }
}

/// Reject compatibility patch fields not represented by the typed scenario.
fn exact_patch<'de, D: serde::Deserializer<'de>>(decoder: D) -> Result<Option<Patch>, D::Error> {
    use serde::de::Error as _;
    let value = Value::deserialize(decoder)?;
    let patch: Patch = serde_json::from_value(value.clone()).map_err(D::Error::custom)?;
    let retained = serde_json::to_value(&patch).map_err(D::Error::custom)?;
    if canonical(retained) != canonical(value) {
        return Err(D::Error::custom("patch fields were not retained"));
    }
    Ok(Some(patch))
}

/// Resolve the actual catalog API before applying explicit compatibility changes.
fn descriptor(wire: &Wire, id: &str) -> TestResult<Model> {
    let expected: Model = serde_json::from_value(wire.model.clone())?;
    assert!(
        canonical(serde_json::to_value(&expected)?) == canonical(wire.model.clone()),
        "descriptor fields retained"
    );
    let model = get_model(&expected.provider, &expected.id).ok_or("catalog entry")?;
    if id.starts_with("configured_") || id.starts_with("forced_") || id.starts_with("long_") {
        assert_eq!(
            model.id,
            probes::selected(&model.provider, id.starts_with("forced_"))?,
            "selected probe"
        );
    }
    assert_eq!(model.api.as_str(), "anthropic-messages", "catalog protocol");
    let mut actual = serde_json::to_value(model)?;
    if let Some(patch) = &wire.model_patch {
        let changes = serde_json::to_value(&patch.compat)?;
        if actual.get("compat").is_none() {
            actual["compat"] = json!({});
        }
        actual["compat"]
            .as_object_mut()
            .ok_or("compat object")?
            .extend(changes.as_object().ok_or("patch object")?.clone());
    }
    if id.starts_with("eager_") || id.starts_with("markers_") || id.starts_with("policies_") {
        if let Some(compat) = wire.model.get("compat") {
            actual["compat"] = compat.clone();
        } else {
            actual
                .as_object_mut()
                .ok_or("model object")?
                .shift_remove("compat");
        }
    }
    assert!(
        canonical(actual.clone()) == canonical(wire.model.clone()),
        "effective catalog metadata mismatch"
    );
    Ok(serde_json::from_value(actual)?)
}

/// Provider-only typed fields are serialized into the public open options seam.
fn raw_options(options: &Options, common: StreamOptions) -> TestResult<ProviderStreamOptions> {
    let mut extra = maestro_models::JsonObject::new();
    for (key, value) in [
        ("thinkingEnabled", options.thinking_enabled.map(json_value)),
        (
            "interleavedThinking",
            options.interleaved_thinking.map(json_value),
        ),
        (
            "effort",
            options.effort.map(|effort| {
                json_value(match effort {
                    maestro_models::AnthropicEffort::Low => "low",
                    maestro_models::AnthropicEffort::Medium => "medium",
                    maestro_models::AnthropicEffort::High => "high",
                    maestro_models::AnthropicEffort::Xhigh => "xhigh",
                    maestro_models::AnthropicEffort::Max => "max",
                })
            }),
        ),
        (
            "thinkingDisplay",
            options.thinking_display.map(|display| {
                json_value(match display {
                    maestro_models::AnthropicThinkingDisplay::Summarized => "summarized",
                    maestro_models::AnthropicThinkingDisplay::Omitted => "omitted",
                })
            }),
        ),
    ] {
        if let Some(value) = value {
            extra.insert(key.into(), value?);
        }
    }
    assert!(
        options.reasoning.is_none(),
        "raw route has simple reasoning"
    );
    Ok(ProviderStreamOptions {
        common,
        extra,
        ..ProviderStreamOptions::default()
    })
}

/// Serialize one typed provider field.
fn json_value(value: impl serde::Serialize) -> serde_json::Result<Value> {
    serde_json::to_value(value)
}

/// Common options retain their authored absence or explicit value.
fn common(wire: &Wire) -> (StreamOptions, bundled::Requests) {
    assert_eq!(
        wire.events.first().map(|event| &event["type"]),
        Some(&json!("message_start")),
        "message opening"
    );
    assert_eq!(
        wire.events.last().map(|event| &event["type"]),
        Some(&json!("message_stop")),
        "message terminal envelope"
    );
    let body = wire
        .events
        .iter()
        .map(messages::sse)
        .collect::<String>()
        .into_bytes();
    let (fetch, requests) = bundled::fetch(body);
    (
        StreamOptions {
            api_key: Some(wire.options.api_key.clone()),
            max_tokens: wire.options.max_tokens,
            temperature: wire.options.temperature,
            cache_retention: wire.options.cache_retention,
            fetch: Some(fetch),
            ..StreamOptions::default()
        },
        requests,
    )
}

/// Invoke the real adapter and reduce ordered events from this operation only.
async fn invoke(wire: &Wire, id: &str, expected: &Value) -> TestResult<Value> {
    let model = descriptor(wire, id)?;
    let (common, requests) = common(wire);
    let context = wire.context.clone();
    let (result, events) = match wire.route {
        Route::Complete => (
            complete(model, context, Some(raw_options(&wire.options, common)?)).await?,
            None,
        ),
        Route::Stream | Route::StreamSimple => {
            let started = match wire.route {
                Route::Stream => stream(model, context, Some(raw_options(&wire.options, common)?))?,
                _ => stream_simple(
                    model,
                    context,
                    Some(SimpleStreamOptions {
                        common,
                        reasoning: wire.options.reasoning,
                        ..SimpleStreamOptions::default()
                    }),
                )?,
            };
            let events = bundled::drain(&started).await;
            let result = started.result().await;
            assert_tool_handles(&events, &result, expected)?;
            let recorded = events
                .iter()
                .map(event_record)
                .collect::<TestResult<Vec<_>>>()?;
            (result, Some(recorded))
        }
    };
    let mut message = serde_json::to_value(&*result.read().map_err(|_| "message poisoned")?)?;
    assert!(
        matches!(message["stopReason"].as_str(), Some("stop" | "toolUse")),
        "terminal success"
    );
    message
        .as_object_mut()
        .ok_or("message object")?
        .shift_remove("timestamp");
    let seen = bundled::recorded(&requests);
    assert_eq!(seen.len(), 1, "single completed invocation");
    let mut output =
        json!({"requests": seen.iter().map(request_record).collect::<Vec<_>>(), "result": message});
    if let Some(events) = events {
        output["events"] = json!(events);
    }
    assertion_outputs(wire, &result, &mut output)?;
    Ok(output)
}

/// Additional source assertion observations never normalize the stored response.
fn assertion_outputs(
    wire: &Wire,
    result: &maestro_models::SharedAssistantMessage,
    output: &mut Value,
) -> TestResult {
    let text = bundled::text_of(result);
    if wire.pongs == Some(true) {
        output["pongCount"] = json!(
            regex::Regex::new(r"(?i)(?-u:\b)pong(?-u:\b)")?
                .find_iter(&text)
                .count()
        );
    }
    if wire.smoke_text == Some(true) {
        output["trimmedText"] = json!(text.trim_matches(source_whitespace));
    }
    Ok(())
}

/// Observe the tool-end handle from this invocation before checking the final response.
fn assert_tool_handles(
    events: &[maestro_models::AssistantMessageEvent],
    result: &maestro_models::SharedAssistantMessage,
    expected: &Value,
) -> TestResult {
    for (index, event) in events.iter().enumerate() {
        if let maestro_models::AssistantMessageEvent::ToolcallEnd {
            partial,
            content_index,
            tool_call,
        } = event
        {
            assert!(
                std::sync::Arc::ptr_eq(partial, result),
                "tool-end retains this invocation handle"
            );
            let wanted = &expected["events"][index]["toolCall"];
            assert!(
                serde_json::to_value(tool_call)? == *wanted,
                "toolcall_end original name and arguments"
            );
            let message = partial.read().map_err(|_| "partial poisoned")?;
            assert!(
                serde_json::to_value(&message.content[*content_index])? == *wanted,
                "tool-end live handle original name"
            );
        }
    }
    Ok(())
}

use maestro_text::is_whitespace_scalar as source_whitespace;

/// Compare the native adapter's complete retained request fields.
fn request_record(request: &bundled::Seen) -> Value {
    let headers: BTreeMap<_, _> = request.headers.iter().cloned().collect();
    json!({"url": request.url, "headers": headers, "body": request.body})
}
/// Exact corpus ownership and cardinalities.
const OWNERS: [(&str, usize); 20] = [
    (
        "crates/maestro-models/tests/message_eager_input.rs::maestro_message_expands_catalog_candidates",
        1,
    ),
    (
        "crates/maestro-models/tests/message_eager_input.rs::maestro_message_selects_provider_probes",
        15,
    ),
    (
        "crates/maestro-models/tests/message_eager_input.rs::maestro_message_eager_input_follows_compatibility",
        9,
    ),
    (
        "crates/maestro-models/tests/message_eager_input.rs::maestro_message_uses_configured_provider_probes",
        9,
    ),
    (
        "crates/maestro-models/tests/message_eager_input.rs::maestro_message_forces_only_supported_eager_probes",
        9,
    ),
    (
        "crates/maestro-models/tests/message_long_retention.rs::maestro_message_probes_long_retention_per_provider",
        9,
    ),
    (
        "crates/maestro-models/tests/message_long_retention.rs::maestro_message_places_retention_markers",
        13,
    ),
    (
        "crates/maestro-models/tests/message_long_retention.rs::maestro_message_keeps_compatibility_policies_independent",
        4,
    ),
    (
        "crates/maestro-models/tests/message_thinking_disabled.rs::maestro_message_disables_budget_thinking",
        1,
    ),
    (
        "crates/maestro-models/tests/message_thinking_disabled.rs::maestro_message_distinguishes_raw_and_simple_thinking",
        9,
    ),
    (
        "crates/maestro-models/tests/message_thinking_disabled.rs::maestro_message_disables_adaptive_thinking",
        2,
    ),
    (
        "crates/maestro-models/tests/message_adaptive_thinking.rs::maestro_message_preserves_adaptive_effort",
        12,
    ),
    (
        "crates/maestro-models/tests/message_adaptive_thinking.rs::maestro_message_streams_signed_arithmetic",
        3,
    ),
    (
        "crates/maestro-models/tests/message_thinking_disabled.rs::maestro_message_off_stream_contains_only_text",
        2,
    ),
    (
        "crates/maestro-models/tests/account_tool_names.rs::maestro_account_restores_original_tool_names",
        6,
    ),
    (
        "crates/maestro-models/tests/account_tool_names.rs::maestro_account_keeps_unrelated_tool_names",
        5,
    ),
    (
        "crates/maestro-models/tests/account_tool_names.rs::maestro_account_projects_names_without_mutating_callers",
        2,
    ),
    (
        "crates/maestro-models/tests/account_tool_names.rs::maestro_account_leaves_key_authenticated_names_plain",
        3,
    ),
    (
        "crates/maestro-models/tests/account_message_routing.rs::maestro_account_routes_message_authentication",
        3,
    ),
    (
        "crates/maestro-models/tests/account_message_routing.rs::maestro_account_selects_interleaved_beta_independently",
        13,
    ),
];

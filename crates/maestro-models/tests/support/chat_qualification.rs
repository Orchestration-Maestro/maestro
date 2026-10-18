//! Recorded controlled requests exercised through public model dispatch.

use crate::{
    bundled, chat, child_process,
    json::{canonical, event_record},
};
use chat::TestResult;
use maestro_models::{
    Context, Model, OnPayload, ProviderStreamOptions, SimpleStreamOptions, StreamOptions,
    ThinkingLevel, ToolChoice, complete, complete_simple, get_model, stream, stream_simple,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

/// Recorded input and expected output owned by one named test.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    /// Unique input label.
    id: String,
    /// Source-named test owner.
    test: String,
    /// Public operation's operands.
    input: Input,
    /// Observations without runtime timestamps or transport-added headers.
    expected: Value,
}

/// The three distinct operations in the corpus.
#[derive(Deserialize)]
#[serde(untagged)]
enum Input {
    /// Public invocation with an injected wire response.
    Wire(Box<Wire>),
    /// Catalog metadata lookup.
    Catalog(Catalog),
    /// Test-owned payload marker.
    Marker(Marker),
}

/// Operands of one public invocation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    /// Effective descriptor, compared against the catalog plus explicit overrides.
    model: Value,
    /// Conversation sent to the provider, with a lossless fixture decode check.
    #[serde(deserialize_with = "exact_context")]
    context: Context,
    /// Typed invocation controls.
    options: Options,
    /// Provider wire chunks, including explicit JSON null.
    chunks: Vec<Value>,
    /// Public operation to invoke.
    route: Route,
    /// Optional test-owned payload edit.
    hook: Option<MarkerKind>,
}

/// Invocation controls used by the recorded inputs.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Options {
    /// Synthetic credential, never printed on assertion failure.
    api_key: String,
    /// Requested thinking level.
    reasoning: Option<ThinkingLevel>,
    /// Tool selection as accepted by the public interfaces.
    tool_choice: Option<Value>,
    /// Caller headers in their authored order.
    headers: Option<maestro_models::JsonObject>,
    /// Session affinity value.
    session_id: Option<String>,
    /// Explicit output budget.
    max_tokens: Option<f64>,
    /// Explicit sampling temperature.
    temperature: Option<f64>,
}

/// Public dispatch route.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum Route {
    /// Full stream options.
    Stream,
    /// Simple stream options.
    StreamSimple,
    /// Full completion options.
    Complete,
    /// Simple completion options.
    CompleteSimple,
}

/// Catalog lookup operands.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Catalog {
    /// Models in authored order.
    catalog_models: Vec<CatalogKey>,
}

/// Identity of one catalog entry.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogKey {
    /// Provider identity.
    provider: String,
    /// Model identity.
    id: String,
}

/// Pure payload marker operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Marker {
    /// Unmarked payload, including malformed or absent messages.
    payload: Value,
    /// Marker operation discriminator.
    operation: MarkerKind,
}

/// The test-owned payload callback.
#[derive(Deserialize)]
enum MarkerKind {
    /// Add ephemeral caching to the last user text.
    #[serde(rename = "cache_marker")]
    CacheMarker,
}

/// Exact owner names and row cardinalities; prevents silently unowned fixture inputs.
const OWNERS: [(&str, usize); 21] = [
    (
        "crates/maestro-models/tests/chat_empty_tools.rs::maestro_chat_omits_unused_tool_declarations",
        2,
    ),
    (
        "crates/maestro-models/tests/chat_empty_tools.rs::maestro_chat_retains_history_required_tools",
        8,
    ),
    (
        "crates/maestro-models/tests/chat_empty_tools.rs::maestro_gateway_uses_conservative_chat_fields",
        1,
    ),
    (
        "crates/maestro-models/tests/chat_empty_tools.rs::maestro_gateway_preserves_upstream_credentials",
        1,
    ),
    (
        "crates/maestro-models/tests/chat_empty_tools.rs::maestro_gateway_sends_session_affinity",
        2,
    ),
    (
        "crates/maestro-models/tests/chat_tool_choices/payload.rs::maestro_chat_public_choices_reach_wire",
        10,
    ),
    (
        "crates/maestro-models/tests/chat_tool_choices/payload.rs::maestro_chat_omits_disabled_strictness",
        2,
    ),
    (
        "crates/maestro-models/tests/chat_tool_choices/payload.rs::maestro_chat_uses_catalog_reasoning_maps",
        12,
    ),
    (
        "crates/maestro-models/tests/chat_tool_choices/payload.rs::maestro_chat_tool_stream_follows_catalog_and_overrides",
        9,
    ),
    (
        "crates/maestro-models/tests/chat_tool_choices/payload.rs::maestro_router_uses_nested_reasoning",
        2,
    ),
    (
        "crates/maestro-models/tests/chat_response_identity.rs::maestro_chat_retains_routed_model_identity",
        8,
    ),
    (
        "crates/maestro-models/tests/chat_response_identity.rs::maestro_chat_preserves_provider_finish_outcomes",
        11,
    ),
    (
        "crates/maestro-models/tests/chat_response_identity.rs::maestro_chat_ignores_null_wire_chunks",
        1,
    ),
    (
        "crates/maestro-models/tests/chat_tool_choices/interleaved.rs::maestro_public_chat_keeps_stable_tool_indices",
        1,
    ),
    (
        "crates/maestro-models/tests/chat_tool_choices/interleaved.rs::maestro_public_chat_keeps_parallel_tool_deltas_independent",
        1,
    ),
    (
        "crates/maestro-models/tests/chat_tool_choices/interleaved.rs::maestro_chat_selects_first_nonempty_reasoning_field",
        4,
    ),
    (
        "crates/maestro-models/tests/chat_tool_choices/usage.rs::maestro_chat_counts_reasoning_once",
        1,
    ),
    (
        "crates/maestro-models/tests/chat_tool_choices/usage.rs::maestro_chat_keeps_both_cache_usage_locations",
        2,
    ),
    (
        "crates/maestro-models/tests/chat_tool_choices/usage.rs::maestro_chat_normalizes_cache_accounting",
        7,
    ),
    (
        "crates/maestro-models/tests/routed_cache_write.rs::maestro_cache_marker_selects_last_user_text",
        7,
    ),
    (
        "crates/maestro-models/tests/routed_cache_write.rs::maestro_routed_chat_retains_cache_writes",
        2,
    ),
];

/// Load strict fixture operands and check ownership and uniqueness before selection.
fn rows(test: &str) -> TestResult<Vec<Row>> {
    let source = include_str!("../fixtures/chat_qualification.json");
    let raw: Vec<Value> = serde_json::from_str(source)?;
    let mut ids = HashSet::new();
    let mut inputs = HashSet::new();
    let mut counts = BTreeMap::new();
    for row in &raw {
        assert!(
            ids.insert(row["id"].clone().to_string()),
            "duplicate row id"
        );
        assert!(inputs.insert(row["input"].to_string()), "duplicate input");
        let owner = row["test"]
            .as_str()
            .ok_or("owner")?
            .rsplit("::")
            .next()
            .ok_or("test name")?;
        assert!(
            OWNERS
                .iter()
                .any(|(name, _)| Some(*name) == row["test"].as_str()),
            "unowned row"
        );
        *counts.entry(owner).or_insert(0) += 1;
    }
    for (full_owner, count) in OWNERS {
        let owner = full_owner.rsplit("::").next().ok_or("test owner")?;
        assert_eq!(counts.get(owner).copied(), Some(count), "{owner} rows");
    }
    let rows: Vec<Row> = serde_json::from_str(source)?;
    let selected: Vec<_> = rows
        .into_iter()
        .filter(|row| row.test.rsplit("::").next() == Some(test))
        .collect();
    assert!(!selected.is_empty(), "test owns no rows");
    Ok(selected)
}

/// Exercise one owner's closed fixture set in a fresh environment.
pub fn assert_rows(test: &str) -> TestResult {
    let owned = rows(test)?;
    if child_process::child_case().is_none() {
        let variables = if test.starts_with("maestro_gateway_") {
            vec![
                ("CLOUDFLARE_ACCOUNT_ID", "account-id"),
                ("CLOUDFLARE_GATEWAY_ID", "gateway-id"),
            ]
        } else {
            Vec::new()
        };
        let full_name = if let Some((file, _)) = owned[0].test.split_once(".rs::")
            && let Some(module) = file.split("chat_tool_choices/").nth(1)
        {
            format!("chat_tool_choices::suite::{module}::{test}")
        } else {
            test.to_owned()
        };
        return child_process::rerun(&full_name, "qualification", &variables);
    }
    let _registry = bundled::registry();
    maestro_models::reset_api_providers();
    chat::block_on(false, async {
        for row in owned {
            let actual = match &row.input {
                Input::Wire(wire) => invoke(wire, &row.id).await?,
                Input::Catalog(catalog) => metadata(catalog)?,
                Input::Marker(marker) => marker_output(marker),
            };
            let actual = canonical(actual);
            let expected = canonical(row.expected);
            let expected = expected.as_object().ok_or("expected object")?;
            assert_eq!(
                actual.as_object().ok_or("actual object")?.len(),
                expected.len(),
                "{} observation fields",
                row.id
            );
            for (field, value) in expected {
                assert!(
                    actual.get(field) == Some(value),
                    "{} {field} mismatch",
                    row.id
                );
            }
        }
        Ok(())
    })
}

/// Apply the pure test-owned operation to its unmarked payload.
fn marker_output(marker: &Marker) -> Value {
    match marker.operation {
        MarkerKind::CacheMarker => json!({"payload": cache_marker(marker.payload.clone())}),
    }
}

/// Construct the catalog descriptor with only source-authored overrides.
fn descriptor(wire: &Wire, id: &str) -> TestResult<Model> {
    let expected: Model = serde_json::from_value(wire.model.clone())?;
    if expected.id == "openrouter/auto" {
        assert!(
            canonical(serde_json::to_value(&expected)?) == canonical(wire.model.clone()),
            "explicit descriptor fields"
        );
        return Ok(expected);
    }
    let mut model = get_model(&expected.provider, &expected.id).ok_or("catalog entry")?;
    if expected.provider == "openai" {
        model.api = "openai-completions".into();
        model.compat = if matches!(id, "strict_true" | "strict_false") {
            Some(serde_json::from_value(
                json!({"supportsStrictMode": id == "strict_true"}),
            )?)
        } else {
            None
        };
    }
    if matches!(id, "zai_override_true" | "zai_override_false") {
        let mut value = serde_json::to_value(&model)?;
        value["compat"]["zaiToolStream"] = json!(id == "zai_override_true");
        model = serde_json::from_value(value)?;
    }
    assert!(
        canonical(serde_json::to_value(&model)?) == canonical(wire.model.clone()),
        "effective catalog metadata mismatch"
    );
    Ok(model)
}

/// Catalog compatibility values in requested order, preserving absent overrides.
fn metadata(catalog: &Catalog) -> TestResult<Value> {
    let mut entries = Vec::new();
    for key in &catalog.catalog_models {
        let model = get_model(&key.provider, &key.id).ok_or("catalog entry")?;
        let mut entry = json!({"provider": key.provider, "id": key.id});
        let value = serde_json::to_value(model)?;
        if let Some(support) = value["compat"].get("zaiToolStream") {
            entry["zaiToolStream"] = support.clone();
        }
        entries.push(entry);
    }
    Ok(json!({"metadata": entries}))
}

/// Decode conversation records only when every fixture field survives in the public type.
fn exact_context<'de, D: serde::Deserializer<'de>>(decoder: D) -> Result<Context, D::Error> {
    use serde::de::Error as _;
    let value = Value::deserialize(decoder)?;
    let context: Context = serde_json::from_value(value.clone()).map_err(D::Error::custom)?;
    let retained = serde_json::to_value(&context).map_err(D::Error::custom)?;
    if canonical(retained) != canonical(value) {
        return Err(D::Error::custom("context fixture fields were not retained"));
    }
    Ok(context)
}

/// Common settings and the transport's request recorder for one invocation.
fn common(wire: &Wire) -> TestResult<(StreamOptions, bundled::Requests)> {
    let mut frames = bundled::sse_frames(&wire.chunks);
    frames.extend_from_slice(b"data: [DONE]\n\n");
    let (fetch, requests) = bundled::fetch(frames);
    let headers = wire
        .options
        .headers
        .as_ref()
        .map(|headers| serde_json::from_value(serde_json::to_value(headers)?))
        .transpose()?;
    let on_payload: Option<OnPayload> = wire.hook.as_ref().map(|_| {
        let hook: OnPayload = Arc::new(
            |maestro_models::PayloadHookInput {
                 payload,
                 signal: hook_signal,
             },
             _| {
                Box::pin(std::future::ready(Ok(maestro_models::PayloadHookInput {
                    payload: cache_marker(payload),
                    signal: hook_signal,
                })))
            },
        );
        hook
    });
    Ok((
        StreamOptions {
            api_key: Some(wire.options.api_key.clone()),
            session_id: wire.options.session_id.clone(),
            max_tokens: wire.options.max_tokens,
            temperature: wire.options.temperature,
            headers,
            fetch: Some(fetch),
            on_payload,
            ..StreamOptions::default()
        },
        requests,
    ))
}

/// Full options retain the authored choice as an open provider field.
fn raw_options(wire: &Wire, common: StreamOptions) -> ProviderStreamOptions {
    let mut extra = maestro_models::JsonObject::new();
    if let Some(choice) = &wire.options.tool_choice {
        extra.insert("toolChoice".into(), choice.clone());
    }
    assert!(
        wire.options.reasoning.is_none(),
        "raw corpus has simple reasoning"
    );
    ProviderStreamOptions {
        common,
        extra,
        ..ProviderStreamOptions::default()
    }
}

/// Simple options decode the named tool choice into the public enum.
fn simple_options(wire: &Wire, common: StreamOptions) -> TestResult<SimpleStreamOptions> {
    let tool_choice: Option<ToolChoice> = wire
        .options
        .tool_choice
        .as_ref()
        .map(|choice| serde_json::from_value(choice.clone()))
        .transpose()?;
    Ok(SimpleStreamOptions {
        common,
        reasoning: wire.options.reasoning,
        tool_choice,
        ..SimpleStreamOptions::default()
    })
}

/// Run one real public operation and capture only that invocation's observations.
async fn invoke(wire: &Wire, id: &str) -> TestResult<Value> {
    let model = descriptor(wire, id)?;
    let (common, requests) = common(wire)?;
    let context = wire.context.clone();
    let (result, events, same) = match wire.route {
        Route::Complete => (
            complete(model, context, Some(raw_options(wire, common))).await?,
            None,
            None,
        ),
        Route::CompleteSimple => (
            complete_simple(model, context, Some(simple_options(wire, common)?)).await?,
            None,
            None,
        ),
        Route::Stream | Route::StreamSimple => {
            let started = match wire.route {
                Route::Stream => stream(model, context, Some(raw_options(wire, common)))?,
                _ => stream_simple(model, context, Some(simple_options(wire, common)?))?,
            };
            let events = bundled::drain(&started).await;
            let result = started.result().await;
            let same = events
                .iter()
                .all(|event| Arc::ptr_eq(crate::response_output::handle(event), &result));
            let recorded: Vec<_> = events.iter().map(event_record).collect::<TestResult<_>>()?;
            (result, Some(recorded), Some(same))
        }
    };
    let mut message = serde_json::to_value(&*result.read().map_err(|_| "message poisoned")?)?;
    assert!(message["timestamp"].is_number(), "runtime timestamp");
    message
        .as_object_mut()
        .ok_or("message object")?
        .shift_remove("timestamp");
    let requests = bundled::recorded(&requests)
        .iter()
        .map(request_record)
        .collect::<Vec<_>>();
    let mut output = json!({"requests": requests, "result": message});
    if let (Some(events), Some(same)) = (events, same) {
        output["events"] = json!(events);
        output["sameHandle"] = json!(same);
    }
    Ok(output)
}

/// Transport request fields owned by the corpus, excluding SDK/runtime headers.
fn request_record(request: &bundled::Seen) -> Value {
    let headers: BTreeMap<_, _> = request
        .headers
        .iter()
        .filter(|(name, _)| {
            matches!(
                name.as_str(),
                "authorization"
                    | "cf-aig-authorization"
                    | "session_id"
                    | "x-client-request-id"
                    | "x-session-affinity"
            )
        })
        .cloned()
        .collect();
    json!({"url": request.url, "headers": headers, "body": request.body})
}

/// Mark the last user with string or array content; a textless array stops selection.
fn cache_marker(mut payload: Value) -> Value {
    let Some(messages) = payload.get_mut("messages").and_then(Value::as_array_mut) else {
        return payload;
    };
    let Some(user) = messages.iter_mut().rev().find(|message| {
        message["role"] == "user"
            && (message["content"].is_string() || message["content"].is_array())
    }) else {
        return payload;
    };
    if let Some(text) = user["content"].as_str() {
        user["content"] =
            json!([{"type": "text", "text": text, "cache_control": {"type": "ephemeral"}}]);
    } else if let Some(parts) = user["content"].as_array_mut()
        && let Some(text) = parts.iter_mut().rev().find(|part| part["type"] == "text")
    {
        text["cache_control"] = json!({"type": "ephemeral"});
    }
    payload
}

//! Recorded response invocations exercised through public dispatch.

use crate::{bundled, chat, child_process, json::canonical, messages, transport};
use chat::TestResult;
use maestro_models::{CacheRetention, Context, Model, ThinkingLevel};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};

#[path = "response_qualification/histories.rs"]
mod histories;
#[path = "response_qualification/invocation.rs"]
mod invocation;

/// One independently recorded query and its observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    /// Scenario label, safe to print.
    id: String,
    /// Exact owning test path.
    test: String,
    /// Public operation inputs.
    input: Input,
    /// Retained observations.
    expected: Expected,
}

/// Catalog lookup, raw-unit sanitization or wire invocation.
#[derive(Deserialize)]
#[serde(untagged)]
enum Input {
    /// Effective model invocation.
    Wire(Box<Wire>),
    /// Ordered catalog queries.
    Catalog(Catalog),
    /// Complete ordered descriptors.
    Models(Models),
    /// Raw sanitizer units.
    Sanitize(Sanitize),
}

/// Inputs passed to existing public model operations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    /// Catalog descriptor with explicit overrides.
    model: Value,
    /// Conversation, before adding default message clocks.
    context: Value,
    /// Invocation controls.
    options: Options,
    /// Native server events.
    events: Option<Vec<Value>>,
    /// Exact native byte chunks instead of event framing.
    chunks: Option<Vec<Vec<u8>>>,
    /// Success, failure, or cancellation phase.
    phase: Phase,
    /// Public operation selected by the source.
    route: Route,
    /// Controlled child environment.
    #[serde(default)]
    env: BTreeMap<String, String>,
}

/// Typed controls, rejecting unused fixture fields.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Options {
    /// Synthetic key supplied to the adapter.
    api_key: String,
    /// Independent output token limit.
    max_tokens: Option<f64>,
    /// Independent sampling temperature.
    temperature: Option<f64>,
    /// Simple reasoning policy.
    reasoning: Option<ThinkingLevel>,
    /// Raw response reasoning effort.
    reasoning_effort: Option<String>,
    /// Raw message thinking switch.
    thinking_enabled: Option<bool>,
    /// Raw message budget.
    thinking_budget_tokens: Option<f64>,
    /// Raw adaptive message effort.
    effort: Option<maestro_models::AnthropicEffort>,
    /// Raw conversation reasoning mode.
    prompt_mode: Option<maestro_models::MistralPromptMode>,
    /// Account transport selection.
    transport: Option<maestro_models::Transport>,
    /// Deployment override.
    azure_deployment_name: Option<String>,
    /// Cache retention control.
    cache_retention: Option<CacheRetention>,
    /// Session identity including present-empty.
    session_id: Option<String>,
    /// Caller headers.
    headers: Option<BTreeMap<String, String>>,
}

/// Invocation lifecycle chosen by the recorded input.
#[derive(Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Phase {
    /// Finite successful body.
    Success,
    /// Finite body ending with a provider failure.
    #[serde(rename = "error")]
    Failure,
    /// Signal already cancelled before dispatch.
    Pre,
    /// Cancel after the pending-read witness.
    Mid,
}

/// Public dispatch operations.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum Route {
    /// Raw streaming.
    Stream,
    /// Raw completion.
    Complete,
    /// Simple completion.
    CompleteSimple,
    /// Simple streaming.
    StreamSimple,
}

/// Ordered catalog enumeration operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    /// Catalog operation tag.
    operation: String,
    /// Authored model rows.
    pairs: Vec<Pair>,
}

/// One catalog identity with an optional explicit API.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Pair {
    /// Provider key.
    provider: String,
    /// Model key.
    model: String,
    /// Authored protocol override.
    api_override: Option<String>,
}

/// Ordered public model enumeration.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Models {
    /// Operation identity.
    operation: String,
    /// Catalog provider.
    provider: String,
}

/// Public sanitizer inputs.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Sanitize {
    /// Operation identity.
    operation: String,
    /// Raw UTF-16 input units.
    units: Vec<u16>,
}

/// Every retained expected subtree is compared.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// Ordered catalog APIs including absent models.
    apis: Option<Vec<Option<String>>>,
    /// Complete ordered catalog records.
    models: Option<Vec<Value>>,
    /// Sanitized text.
    text: Option<String>,
    /// Payload callback observations.
    payloads: Option<Vec<Value>>,
    /// Application-owned wire request observations.
    requests: Option<Vec<Value>>,
    /// Terminal assistant record without its clock.
    result: Option<Value>,
    /// Ordered stream events without their shared handles.
    events: Option<Vec<Value>>,
    /// Arguments observed before releasing each subsequent input fragment.
    progress: Option<Vec<Progress>>,
}

/// A tool delta and its live partial call at the producer gate.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Progress {
    /// Exact fragment emitted by the public stream.
    delta: String,
    /// Position in the assistant's content.
    index: usize,
    /// Actual call identity.
    id: String,
    /// Selected tool name.
    name: String,
    /// Live parsed arguments.
    arguments: Value,
}

/// Closed ownership counts prevent silently dropped fixture cases.
const OWNERS: [(&str, usize); 21] = [
    (
        "crates/maestro-models/tests/history_handoffs/cases.rs::maestro_handoff_table_keeps_distinct_routes",
        1,
    ),
    (
        "crates/maestro-models/tests/response_identity.rs::maestro_public_routes_retain_response_ids",
        9,
    ),
    (
        "crates/maestro-models/tests/response_identity.rs::maestro_failed_responses_keep_partial_identity",
        3,
    ),
    (
        "crates/maestro-models/tests/response_reasoning_replay.rs::maestro_replay_omits_aborted_reasoning",
        2,
    ),
    (
        "crates/maestro-models/tests/response_reasoning_replay.rs::maestro_handoff_omits_foreign_reasoning_pair_ids",
        1,
    ),
    (
        "crates/maestro-models/tests/response_reasoning_replay.rs::maestro_handoff_preserves_response_identity",
        2,
    ),
    (
        "crates/maestro-models/tests/response_reasoning_replay.rs::maestro_replay_selects_signatures_by_full_identity",
        32,
    ),
    (
        "crates/maestro-models/tests/response_affinity.rs::maestro_response_cache_keys_follow_retention",
        9,
    ),
    (
        "crates/maestro-models/tests/response_session_affinity.rs::maestro_account_sse_keeps_session_affinity",
        7,
    ),
    (
        "crates/maestro-models/tests/response_tool_images.rs::maestro_tool_images_stay_with_function_results",
        8,
    ),
    (
        "crates/maestro-models/tests/response_tool_images.rs::maestro_tool_outputs_preserve_content_classes",
        8,
    ),
    (
        "crates/maestro-models/tests/interleaved_reasoning.rs::maestro_reasoning_survives_tool_continuations",
        4,
    ),
    (
        "crates/maestro-models/tests/invocation_cancellation.rs::maestro_public_routes_honor_preabort",
        13,
    ),
    (
        "crates/maestro-models/tests/invocation_cancellation.rs::maestro_cancelled_streams_keep_partials_and_continue",
        29,
    ),
    (
        "crates/maestro-models/tests/history_handoffs/replay.rs::maestro_handoffs_generate_and_replay_each_route",
        60,
    ),
    (
        "crates/maestro-models/tests/history_handoffs/replay.rs::maestro_handoffs_consume_other_histories_in_order",
        30,
    ),
    (
        "crates/maestro-models/tests/handoff_tool_ids.rs::maestro_generated_tool_ids_survive_both_handoffs",
        4,
    ),
    (
        "crates/maestro-models/tests/handoff_tool_ids.rs::maestro_long_prefilled_ids_keep_result_pairing",
        2,
    ),
    (
        "crates/maestro-models/tests/history_handoffs/cases.rs::maestro_handoff_auth_selection_keeps_upstream_headers",
        4,
    ),
    (
        "crates/maestro-models/tests/history_handoffs/replay.rs::maestro_handoff_failures_keep_payload_evidence",
        4,
    ),
    (
        "crates/maestro-models/tests/handoff_tool_ids.rs::maestro_handoffs_pair_each_tool_occurrence",
        5,
    ),
];

/// Load the strict corpus and reject duplicate identities and effective inputs.
fn rows(source: &str, owners: &[(&str, usize)]) -> TestResult<Vec<Row>> {
    let raw: Vec<Value> = serde_json::from_str(source)?;
    let mut ids = HashSet::new();
    let mut inputs = HashSet::new();
    for row in &raw {
        assert!(ids.insert(row["id"].to_string()), "duplicate scenario");
        assert!(inputs.insert(row["input"].to_string()), "duplicate query");
    }
    for &(owner, count) in owners {
        assert_eq!(
            raw.iter().filter(|row| row["test"] == owner).count(),
            count,
            "{owner} case inventory"
        );
    }
    assert_eq!(
        raw.len(),
        owners.iter().map(|(_, count)| count).sum::<usize>(),
        "owned corpus size"
    );
    Ok(serde_json::from_str(source)?)
}

/// Run a test's recorded inputs in one isolated child process.
pub fn assert_rows(test: &str) -> TestResult {
    assert_corpus_rows(
        include_str!("../fixtures/response_qualification.json"),
        &OWNERS,
        test,
    )
}

/// Select a strict corpus with explicit ownership and run its public observations.
pub fn assert_corpus_rows(source: &str, owners: &[(&str, usize)], test: &str) -> TestResult {
    let all = rows(source, owners)?;
    let owned: Vec<_> = all
        .iter()
        .filter(|row| row.test.ends_with(&format!("::{test}")))
        .collect();
    assert!(!owned.is_empty(), "unowned test");
    let _registry = bundled::registry();
    if child_process::child_case().is_none() {
        let env = environment(&owned, &all)?;
        let full_name = test_path(&owned[0].test, test)?;
        return child_process::rerun(
            &full_name,
            "responses",
            &env.into_iter().collect::<Vec<_>>(),
        );
    }
    maestro_models::reset_api_providers();
    chat::block_on(false, async {
        let mut results = histories::Results::new();
        for row in owned {
            match &row.input {
                Input::Wire(wire) => execute(row, wire, &all, &mut results).await?,
                Input::Catalog(catalog) => catalog_rows(row, catalog),
                Input::Models(query) => model_rows(row, query)?,
                Input::Sanitize(query) => {
                    assert_eq!(query.operation, "sanitizeSurrogates");
                    assert_eq!(
                        Some(maestro_models::sanitize_surrogates(&query.units)),
                        row.expected.text,
                        "sanitized units"
                    );
                }
            }
        }
        Ok(())
    })
}

/// Translate the corpus's source-owned nested module into its Cargo test name.
fn test_path(owner: &str, test: &str) -> TestResult<String> {
    for directory in [
        "history_handoffs",
        "provider_unicode",
        "empty_content",
        "model_streaming",
    ] {
        if let Some((_, module)) = owner.split_once(&format!("{directory}/")) {
            return Ok(format!(
                "{directory}::{}::{test}",
                module.split('.').next().ok_or("module")?
            ));
        }
    }
    Ok(test.into())
}

/// Compare complete descriptors in the catalog's configured order.
fn model_rows(row: &Row, query: &Models) -> TestResult {
    assert_eq!(query.operation, "getModels");
    let models = maestro_models::get_models(&query.provider);
    assert!(!models.is_empty(), "configured models");
    assert!(
        canonical(json!(models)) == canonical(json!(row.expected.models.as_ref().ok_or("models")?)),
        "ordered descriptors"
    );
    Ok(())
}

/// Observe API keys in authored order, retaining absent catalog entries.
fn catalog_rows(row: &Row, catalog: &Catalog) {
    assert_eq!(catalog.operation, "catalog");
    let models: Vec<_> = catalog
        .pairs
        .iter()
        .map(|pair| maestro_models::get_model(&pair.provider, &pair.model))
        .collect();
    let mut routes = Vec::new();
    for (pair, model) in catalog.pairs.iter().zip(&models) {
        if let Some(model) = model {
            assert_eq!(model.provider, pair.provider, "catalog provider identity");
            assert_eq!(model.id, pair.model, "catalog model identity");
            let api = pair.api_override.as_deref().unwrap_or(&model.api);
            if bundled::apis().contains(&api) {
                routes.push((&model.provider, api, &model.id));
            }
        }
    }
    assert_eq!(routes.len(), 30, "invocable route sequence");
    assert_eq!(
        routes.iter().collect::<HashSet<_>>().len(),
        30,
        "distinct route identities"
    );
    let apis: Vec<_> = catalog
        .pairs
        .iter()
        .zip(models)
        .map(|(pair, model)| model.map(|model| pair.api_override.clone().unwrap_or(model.api)))
        .collect();
    assert_eq!(Some(apis), row.expected.apis, "{} ordered routes", row.id);
}

/// Verify descriptor provenance before applying source-authored overrides.
fn descriptor(wire: &Wire) -> TestResult<Model> {
    let expected: Model = serde_json::from_value(wire.model.clone())?;
    if expected.provider == "ollama" {
        return Ok(expected);
    }
    let mut model =
        maestro_models::get_model(&expected.provider, &expected.id).ok_or("catalog model")?;
    if expected.provider == "openai" && expected.api == "openai-completions" {
        model.api.clone_from(&expected.api);
        model.compat = None;
    }
    if wire.options.transport == Some(maestro_models::Transport::Websocket) {
        model.base_url.clone_from(&expected.base_url);
    }
    if wire.model["input"] == json!(["text"]) {
        model.input.clone_from(&expected.input);
    }
    assert!(
        canonical(serde_json::to_value(&model)?) == canonical(wire.model.clone()),
        "catalog descriptor"
    );
    Ok(model)
}

/// Remove only top-level message clocks from a comparison value.
fn without_clocks(mut context: Value) -> Value {
    if let Some(messages) = context["messages"].as_array_mut() {
        for message in messages {
            if let Some(object) = message.as_object_mut() {
                object.remove("timestamp");
            }
        }
    }
    context
}

/// Compatible controlled variables also cover prerequisites from another owner.
fn environment<'a>(owned: &[&'a Row], all: &'a [Row]) -> TestResult<BTreeMap<&'a str, &'a str>> {
    let mut env = BTreeMap::new();
    let mut pending = owned.to_vec();
    let mut visited = HashSet::new();
    while let Some(row) = pending.pop() {
        if !visited.insert(&row.id) {
            continue;
        }
        let Input::Wire(wire) = &row.input else {
            continue;
        };
        for (name, value) in &wire.env {
            let previous = env.insert(name.as_str(), value.as_str());
            assert!(
                previous.is_none_or(|previous| previous == value),
                "conflicting environment"
            );
        }
        for id in wire.context["messages"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|message| message["responseId"].as_str())
        {
            let source_id = id
                .strip_prefix("response_")
                .ok_or("prerequisite identity")?;
            pending.push(
                all.iter()
                    .find(|row| row.id == source_id)
                    .ok_or("prerequisite environment")?,
            );
        }
    }
    Ok(env)
}

/// One public operation publishes an actual result to this test's local history set.
async fn execute(
    row: &Row,
    wire: &Wire,
    all: &[Row],
    results: &mut histories::Results,
) -> TestResult {
    let context = histories::context(row, wire, all, results).await?;
    let result = invocation::invoke(row, wire, context).await?;
    let message = result.read().map_err(|_| "result poisoned")?.clone();
    if let Some(id) = &message.response_id {
        results.insert(id.clone(), message);
    }
    Ok(())
}

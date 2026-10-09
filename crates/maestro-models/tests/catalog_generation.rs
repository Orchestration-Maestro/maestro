//! Controlled catalog feed normalization through the public developer interface.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use maestro_models::catalog_generation::generate_models::{
    fetch_ai_gateway_models, fetch_open_router_models, load_models_dev_data,
};
use maestro_models::{DiagnosticErrorInfo, Fetch, FetchError, HttpResponse, Model};
use serde::Deserialize;
use serde_json::Value;
use std::sync::{Arc, Mutex};

/// One controlled feed query and its complete observations.
#[derive(Deserialize)]
struct Case {
    /// Owning behavior test.
    test: String,
    /// Failure context.
    id: String,
    /// Feed operation.
    source: String,
    /// Response construction mode.
    mode: String,
    /// JSON value or response bytes.
    input: Box<serde_json::value::RawValue>,
    /// Independent recorded expectations.
    expected: Expected,
}
/// Expected output of one completed query.
#[derive(Deserialize)]
struct Expected {
    /// Complete model descriptors.
    models: Value,
    /// Progress lines.
    stdout: Vec<String>,
    /// Exact diagnostics or native-cause prefixes.
    stderr: Vec<Diagnostic>,
    /// Completed request URLs.
    urls: Vec<String>,
}
/// Diagnostic comparison selects one observation, never both.
#[derive(Deserialize)]
#[serde(untagged)]
enum Diagnostic {
    /// Application-owned line.
    Exact { exact: String },
    /// Application prefix with a native cause.
    Prefix { prefix: String },
}
/// Construct a controlled native failure.
fn failure(message: &str) -> FetchError {
    FetchError::Connection(DiagnosticErrorInfo {
        name: Some("Error".into()),
        message: message.into(),
        stack: None,
        code: None,
    })
}
/// Exercise the selected public operation.
async fn invoke(
    source: &str,
    fetch: &Fetch,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> std::io::Result<Vec<Model>> {
    match source {
        "router" => fetch_open_router_models(fetch, out, err).await,
        "gateway" => fetch_ai_gateway_models(fetch, out, err).await,
        "dev" => load_models_dev_data(fetch, out, err).await,
        _ => panic!("unknown source"),
    }
}
/// Read response bytes without converting deeply nested feed metadata.
fn body(case: &Case) -> Vec<u8> {
    match case.mode.as_str() {
        "bytes" => serde_json::from_str(case.input.get()).unwrap(),
        "invalid_json" => b"{".to_vec(),
        _ => case.input.get().as_bytes().to_vec(),
    }
}
/// Normalize integer expectations to the descriptor's floating-point representation.
fn expected_numbers(value: &mut Value) {
    match value {
        Value::Object(map) => map.values_mut().for_each(expected_numbers),
        Value::Array(items) => items.iter_mut().for_each(expected_numbers),
        Value::Number(number) => *value = serde_json::json!(number.as_f64().unwrap()),
        _ => {}
    }
}
/// Run every unique input owned by one behavior test.
fn cases(name: &str) {
    let all: Vec<Case> = serde_json::from_str(include_str!("fixtures/catalog_feeds.json")).unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let selected: Vec<_> = all.into_iter().filter(|case| case.test == name).collect();
    assert!(!selected.is_empty(), "missing cases for {name}");
    for mut case in selected {
        let (fetch, urls) = case_fetch(&case);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let models = runtime
            .block_on(invoke(&case.source, &fetch, &mut out, &mut err))
            .unwrap();
        assert_observations(&mut case, models, out, err);
        assert_eq!(
            *urls.lock().unwrap(),
            case.expected.urls,
            "{} urls",
            case.id
        );
    }
}

#[test]
fn empty_feeds_emit_zero_counts() {
    cases("empty_feeds_emit_zero_counts");
}
#[test]
fn failed_sources_discard_partial_results() {
    cases("failed_sources_discard_partial_results");
}
#[test]
fn http_status_does_not_replace_json_semantics() {
    cases("http_status_does_not_replace_json_semantics");
}
#[test]
fn feed_structure_failures_keep_source_fallback() {
    cases("feed_structure_failures_keep_source_fallback");
}
#[test]
fn router_tool_and_modality_filters_match_inputs() {
    cases("router_tool_and_modality_filters_match_inputs");
}
#[test]
fn gateway_requires_array_tags_and_maps_capabilities() {
    cases("gateway_requires_array_tags_and_maps_capabilities");
}
#[test]
fn gateway_nonarray_data_is_empty() {
    cases("gateway_nonarray_data_is_empty");
}
#[test]
fn feed_prices_preserve_prefix_numbers_and_finite_scaling() {
    cases("feed_prices_preserve_prefix_numbers_and_finite_scaling");
}
#[test]
fn limits_keep_authored_defaults_and_finite_values() {
    cases("limits_keep_authored_defaults_and_finite_values");
}
#[test]
fn names_and_ids_preserve_external_text() {
    cases("names_and_ids_preserve_external_text");
}
#[test]
fn models_dev_routes_every_provider_and_preserves_fields() {
    cases("models_dev_routes_every_provider_and_preserves_fields");
}

#[test]
fn models_dev_requires_literal_tool_and_reasoning_flags() {
    cases("models_dev_requires_literal_tool_and_reasoning_flags");
}

#[test]
fn models_dev_modalities_follow_inclusion_rules() {
    cases("models_dev_modalities_follow_inclusion_rules");
}

#[test]
fn models_dev_rates_are_already_per_million() {
    cases("models_dev_rates_are_already_per_million");
}

#[test]
fn bedrock_filters_unsupported_ids_and_selects_region() {
    cases("bedrock_filters_unsupported_ids_and_selects_region");
}

#[test]
fn cloudflare_routes_split_once_and_keep_worker_prefix() {
    cases("cloudflare_routes_split_once_and_keep_worker_prefix");
}

#[test]
fn zai_stream_compat_excludes_exact_legacy_ids() {
    cases("zai_stream_compat_excludes_exact_legacy_ids");
}

#[test]
fn opencode_variants_select_sdk_routes() {
    cases("opencode_variants_select_sdk_routes");
}

#[test]
fn opencode_go_repairs_only_named_model_routes() {
    cases("opencode_go_repairs_only_named_model_routes");
}

#[test]
fn deprecation_filter_is_provider_specific() {
    cases("deprecation_filter_is_provider_specific");
}

#[test]
fn copilot_routing_headers_and_compat_are_branch_specific() {
    cases("copilot_routing_headers_and_compat_are_branch_specific");
}
#[test]
fn kimi_aliases_defer_to_included_canonical_model() {
    cases("kimi_aliases_defer_to_included_canonical_model");
}
#[test]
fn models_dev_preserves_authored_provider_and_entry_order() {
    cases("models_dev_preserves_authored_provider_and_entry_order");
}

#[test]
fn malformed_model_does_not_discard_valid_neighbors() {
    cases("malformed_model_does_not_discard_valid_neighbors");
}

#[test]
fn field_mapping_never_swaps_prices_or_limits() {
    cases("field_mapping_never_swaps_prices_or_limits");
}

#[test]
fn response_json_decodes_bom_and_split_utf8() {
    cases("response_json_decodes_bom_and_split_utf8");
}

#[test]
fn optional_containers_are_read_only_when_used() {
    cases("optional_containers_are_read_only_when_used");
}

#[test]
fn price_whitespace_is_explicit_not_rust_trim() {
    cases("price_whitespace_is_explicit_not_rust_trim");
}

#[test]
fn filtered_records_do_not_trigger_metadata_diagnostics() {
    cases("filtered_records_do_not_trigger_metadata_diagnostics");
}

#[test]
fn json_strings_replace_only_lone_surrogate_units() {
    cases("json_strings_replace_only_lone_surrogate_units");
}

#[test]
fn unused_metadata_does_not_acquire_a_depth_limit() {
    cases("unused_metadata_does_not_acquire_a_depth_limit");
}

#[test]
fn feed_collections_preserve_empty_and_failure_branches() {
    cases("feed_collections_preserve_empty_and_failure_branches");
}

#[test]
fn feed_results_retain_duplicates_before_catalog_assembly() {
    cases("feed_results_retain_duplicates_before_catalog_assembly");
}

#[test]
fn duplicate_members_keep_last_value_and_first_position() {
    cases("duplicate_members_keep_last_value_and_first_position");
}

/// Build a single-response fixture without provider credentials.
fn response_fetch(bytes: Vec<u8>) -> Fetch {
    Arc::new(move |_| {
        let bytes = bytes.clone();
        Box::pin(async move {
            Ok(HttpResponse {
                status: 200,
                headers: std::collections::BTreeMap::new(),
                body: Box::pin(futures_util::stream::iter([Ok(bytes)])),
            })
        })
    })
}
/// Empty feed bytes for each operation.
fn empty_body(source: &str) -> Vec<u8> {
    if source == "dev" {
        b"{}".to_vec()
    } else {
        b"{\"data\":[]}".to_vec()
    }
}

#[test]
fn feed_calls_use_one_get_without_auth_or_retries() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    for source in ["router", "gateway", "dev"] {
        for connection_failure in [false, true] {
            let requests = Arc::new(Mutex::new(Vec::new()));
            let recorded = requests.clone();
            let bytes = empty_body(source);
            let fetch = request_fetch(recorded, bytes, connection_failure);
            runtime
                .block_on(invoke(source, &fetch, &mut Vec::new(), &mut Vec::new()))
                .unwrap();
            let requests = std::mem::take(&mut *requests.lock().unwrap());
            assert_eq!(requests.len(), 1);
            let request = &requests[0];
            assert_eq!(request.method, "GET");
            assert!(request.headers.is_empty());
            assert!(request.body.is_empty());
            assert!(request.signal.is_none());
        }
    }
}

/// Writer witnessing that final count output follows the same body's EOF.
struct CountWriter {
    /// Whether the body producer has closed.
    closed: Arc<std::sync::atomic::AtomicBool>,
    /// Complete progress bytes.
    bytes: Vec<u8>,
}
impl std::io::Write for CountWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.starts_with(b"Fetched") || bytes.starts_with(b"Loaded") {
            assert!(self.closed.load(std::sync::atomic::Ordering::SeqCst));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
#[test]
fn feed_counts_wait_for_complete_body() {
    use futures_util::future::{Either, select};
    use std::sync::atomic::Ordering;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    for source in ["router", "gateway", "dev"] {
        runtime.block_on(async {
            let BodyGate {
                fetch,
                json_entered,
                release_tx,
                producer_close,
            } = body_gate(source);
            let mut output = CountWriter {
                closed: producer_close.clone(),
                bytes: Vec::new(),
            };
            let mut errors = Vec::new();
            let awaiting = Box::pin(invoke(source, &fetch, &mut output, &mut errors));
            let Either::Right((entered, awaiting)) = select(awaiting, json_entered).await else {
                panic!("body gate was bypassed");
            };
            entered.unwrap();
            assert!(!producer_close.load(Ordering::SeqCst));
            release_tx.send(()).unwrap();
            let awaited_feed_completion = awaiting.await.unwrap();
            assert!(producer_close.load(Ordering::SeqCst));
            assert_eq!(awaited_feed_completion.len(), 1);
            assert_eq!(awaited_feed_completion[0].name, "雪");
            assert!(String::from_utf8(output.bytes).unwrap().ends_with(&format!(
                "1 tool-capable models from {}\n",
                match source {
                    "router" => "OpenRouter",
                    "gateway" => "Vercel AI Gateway",
                    _ => "models.dev",
                }
            )));
            assert!(errors.is_empty());
        });
    }
}

/// Fail writes after a specified number of complete lines.
struct FailingWriter {
    /// Remaining complete lines accepted before failure.
    remaining: usize,
}
impl std::io::Write for FailingWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.remaining == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "controlled output failure",
            ));
        }
        self.remaining -= bytes
            .iter()
            .fold(0, |count, byte| count + usize::from(*byte == b'\n'));
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
#[test]
fn output_sink_errors_are_returned() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    for source in ["router", "gateway", "dev"] {
        for phase in ["start", "count", "source-error", "model-error"] {
            let bytes = if phase == "model-error" {
                match source {
                    "dev" => b"{\"anthropic\":{\"models\":{\"bad\":null}}}".to_vec(),
                    _ => b"{\"data\":[null]}".to_vec(),
                }
            } else {
                empty_body(source)
            };
            let fetch: Fetch = if phase == "start" {
                Arc::new(|_| panic!("fetch after failed initial progress write"))
            } else if phase == "source-error" {
                connection_fetch()
            } else {
                response_fetch(bytes)
            };
            let mut failing = FailingWriter {
                remaining: usize::from(phase == "count"),
            };
            let mut discard = Vec::new();
            let error = if phase.ends_with("error") {
                runtime.block_on(invoke(source, &fetch, &mut discard, &mut failing))
            } else {
                runtime.block_on(invoke(source, &fetch, &mut failing, &mut discard))
            }
            .unwrap_err();
            assert_eq!(
                error.kind(),
                std::io::ErrorKind::PermissionDenied,
                "{source} {phase}"
            );
            assert_eq!(error.to_string(), "controlled output failure");
        }
    }
}

#[test]
fn invalid_ids_are_diagnosed_without_recursive_conversion() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    for source in ["router", "gateway"] {
        let id = format!("{} null {}", "[ ".repeat(140), " ]".repeat(140));
        let capabilities = if source == "router" {
            r#""name":"Bad","supported_parameters":["tools"]"#
        } else {
            r#""tags":["tool-use"]"#
        };
        let neighbor = if source == "router" {
            r#"{"id":"good","name":"Good","supported_parameters":["tools"]}"#
        } else {
            r#"{"id":"good","tags":["tool-use"]}"#
        };
        let bytes = format!(r#"{{"data":[{{"id":{id},{capabilities}}},{neighbor}]}}"#).into_bytes();
        let mut errors = Vec::new();
        let models = runtime
            .block_on(invoke(
                source,
                &response_fetch(bytes),
                &mut Vec::new(),
                &mut errors,
            ))
            .unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "good");
        let label = if source == "router" {
            "OpenRouter"
        } else {
            "Vercel AI Gateway"
        };
        assert_eq!(
            String::from_utf8(errors).unwrap(),
            format!("Failed to fetch {label} models: skipping model {id}: invalid metadata\n")
        );
    }
}

/// A controlled transport with recorded completed request URLs.
fn case_fetch(case: &Case) -> (Fetch, Arc<Mutex<Vec<String>>>) {
    let urls = Arc::new(Mutex::new(Vec::new()));
    let recorded = urls.clone();
    let bytes = body(case);
    let mode = case.mode.clone();
    let fetch: Fetch = Arc::new(move |request| {
        recorded.lock().unwrap().push(request.url);
        let mode = mode.clone();
        let bytes = bytes.clone();
        Box::pin(futures_util::future::ready(controlled_response(
            &mode, bytes,
        )))
    });
    (fetch, urls)
}
/// Construct the fixture response after recording its request.
fn controlled_response(mode: &str, bytes: Vec<u8>) -> Result<HttpResponse, FetchError> {
    if mode == "connection" {
        return Err(failure("fixture connection failure"));
    }
    let chunk = if mode == "body" {
        Err(failure("fixture body failure"))
    } else {
        Ok(bytes)
    };
    Ok(HttpResponse {
        status: if mode == "http503" { 503 } else { 200 },
        headers: std::collections::BTreeMap::new(),
        body: Box::pin(futures_util::stream::iter([chunk])),
    })
}

/// Compare each descriptor field and every completed application line.
fn assert_observations(case: &mut Case, models: Vec<Model>, out: Vec<u8>, err: Vec<u8>) {
    expected_numbers(&mut case.expected.models);
    let actual = serde_json::to_value(models).unwrap();
    assert_eq!(actual, case.expected.models, "{} models", case.id);
    assert_number_bits(&actual, &case.expected.models, &case.id);
    assert_eq!(
        String::from_utf8(out).unwrap().lines().collect::<Vec<_>>(),
        case.expected.stdout,
        "{} stdout",
        case.id
    );
    let errors = String::from_utf8(err).unwrap();
    let lines: Vec<_> = errors.lines().collect();
    assert_eq!(
        lines.len(),
        case.expected.stderr.len(),
        "{} stderr: {errors}",
        case.id
    );
    for (line, expected) in lines.iter().zip(std::mem::take(&mut case.expected.stderr)) {
        match expected {
            Diagnostic::Exact { exact } => assert_eq!(*line, exact, "{}", case.id),
            Diagnostic::Prefix { prefix } => assert!(
                line.starts_with(&prefix) && line.len() > prefix.len(),
                "{}: {line}",
                case.id
            ),
        }
    }
}

/// Compare numeric leaves by bits, including the sign of zero.
fn assert_number_bits(actual: &Value, expected: &Value, id: &str) {
    match (actual, expected) {
        (Value::Number(actual), Value::Number(expected)) => {
            assert_eq!(
                actual.as_f64().unwrap().to_bits(),
                expected.as_f64().unwrap().to_bits(),
                "{id} numeric leaf"
            );
        }
        (Value::Object(actual), Value::Object(expected)) => {
            for (key, expected) in expected {
                assert_number_bits(&actual[key], expected, id);
            }
        }
        (Value::Array(actual), Value::Array(expected)) => {
            for (actual, expected) in actual.iter().zip(expected) {
                assert_number_bits(actual, expected, id);
            }
        }
        _ => {}
    }
}

/// Controlled body lifecycle witnesses for one feed operation.
struct BodyGate {
    /// One-shot transport.
    fetch: Fetch,
    /// JSON consumption has entered the body stream.
    json_entered: tokio::sync::oneshot::Receiver<()>,
    /// Permits the body producer to emit its bytes.
    release_tx: tokio::sync::oneshot::Sender<()>,
    /// EOF from that same stream has been observed.
    producer_close: Arc<std::sync::atomic::AtomicBool>,
}
/// Construct a gated split-scalar body with an EOF witness.
fn body_gate(source: &str) -> BodyGate {
    use futures_util::StreamExt;
    use std::sync::atomic::{AtomicBool, Ordering};
    let bytes = match source {
        "router" => r#"{"data":[{"id":"snow","name":"雪","supported_parameters":["tools"]}]}"#,
        "gateway" => r#"{"data":[{"id":"snow","name":"雪","tags":["tool-use"]}]}"#,
        _ => r#"{"anthropic":{"models":{"snow":{"name":"雪","tool_call":true}}}}"#,
    }
    .as_bytes()
    .to_vec();
    let split = bytes.iter().position(|byte| *byte == 0xe9).unwrap() + 1;
    let (entered_tx, json_entered) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let producer_close = Arc::new(AtomicBool::new(false));
    let closed = producer_close.clone();
    let first = bytes[..split].to_vec();
    let second = bytes[split..].to_vec();
    let stream = futures_util::stream::once(async move {
        entered_tx.send(()).unwrap();
        release_rx.await.unwrap();
        Ok(first)
    })
    .chain(futures_util::stream::iter([Ok(second)]))
    .chain(futures_util::stream::poll_fn(move |_| {
        closed.store(true, Ordering::SeqCst);
        std::task::Poll::Ready(None)
    }));
    let response = Mutex::new(Some(HttpResponse {
        status: 200,
        headers: std::collections::BTreeMap::new(),
        body: Box::pin(stream),
    }));
    let fetch: Fetch = Arc::new(move |_| {
        let response = response.lock().unwrap().take().unwrap();
        Box::pin(async move { Ok(response) })
    });
    BodyGate {
        fetch,
        json_entered,
        release_tx,
        producer_close,
    }
}

/// A request recorder covering both successful and failed single attempts.
fn request_fetch(
    recorded: Arc<Mutex<Vec<maestro_models::HttpRequest>>>,
    bytes: Vec<u8>,
    connection_failure: bool,
) -> Fetch {
    Arc::new(move |request| {
        recorded.lock().unwrap().push(request);
        let response = if connection_failure {
            Err(failure("controlled failure"))
        } else {
            Ok(HttpResponse {
                status: 503,
                headers: std::collections::BTreeMap::new(),
                body: Box::pin(futures_util::stream::iter([Ok(bytes.clone())])),
            })
        };
        Box::pin(futures_util::future::ready(response))
    })
}

/// A controlled failed source for diagnostic writer checks.
fn connection_fetch() -> Fetch {
    Arc::new(|_| {
        Box::pin(futures_util::future::ready(Err(failure(
            "controlled source failure",
        ))))
    })
}

#[test]
fn integer_entry_keys_are_reordered_from_raw_authored_text() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let text = r#"{"anthropic":{"models":{"10":{"tool_call":true},"z":{"tool_call":true},"2":{"tool_call":true},"01":{"tool_call":true},"0":{"tool_call":true},"4294967295":{"tool_call":true},"4294967294":{"tool_call":true}}}}"#;
    let models = runtime
        .block_on(invoke(
            "dev",
            &response_fetch(text.as_bytes().to_vec()),
            &mut Vec::new(),
            &mut Vec::new(),
        ))
        .unwrap();
    assert_eq!(
        models
            .iter()
            .map(|model| model.id.as_str())
            .collect::<Vec<_>>(),
        ["0", "2", "10", "4294967294", "z", "01", "4294967295"]
    );
}

#[test]
fn falsy_price_fields_keep_selected_models() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    for source in ["router", "gateway"] {
        let text = if source == "router" {
            r#"{"data":[{"id":"selected","name":"Selected","supported_parameters":["tools"],"pricing":{"prompt":false,"completion":"0.000007","input_cache_read":false,"input_cache_write":false}}]}"#
        } else {
            r#"{"data":[{"id":"selected","tags":["tool-use"],"pricing":{"input":false,"output":"0.000007","input_cache_read":false,"input_cache_write":false}}]}"#
        };
        let mut errors = Vec::new();
        let models = runtime
            .block_on(invoke(
                source,
                &response_fetch(text.as_bytes().to_vec()),
                &mut Vec::new(),
                &mut errors,
            ))
            .unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(
            models[0].cost,
            maestro_models::ModelCost {
                input: 0.0,
                output: 7.0,
                cache_read: 0.0,
                cache_write: 0.0
            }
        );
        assert!(errors.is_empty());
    }
}

#[test]
fn numeric_negative_zero_prices_keep_feed_specific_signs() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    for source in ["router", "gateway"] {
        let text = if source == "router" {
            r#"{"data":[{"id":"zero","name":"Zero","supported_parameters":["tools"],"pricing":{"prompt":-0,"completion":-0,"input_cache_read":-0,"input_cache_write":-0}}]}"#
        } else {
            r#"{"data":[{"id":"zero","tags":["tool-use"],"pricing":{"input":-0,"output":-0,"input_cache_read":-0,"input_cache_write":-0}}]}"#
        };
        let models = runtime
            .block_on(invoke(
                source,
                &response_fetch(text.as_bytes().to_vec()),
                &mut Vec::new(),
                &mut Vec::new(),
            ))
            .unwrap();
        let cost = &models[0].cost;
        for price in [cost.input, cost.output, cost.cache_read, cost.cache_write] {
            assert_eq!(
                price.to_bits(),
                if source == "gateway" {
                    (-0.0_f64).to_bits()
                } else {
                    0.0_f64.to_bits()
                }
            );
        }
    }
}

#[test]
fn invalid_id_diagnostics_retain_original_escape_spelling() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    for source in ["router", "gateway"] {
        let id = r#"{ "unit":"\ud800", "literal":"\\ud800", "paired":"\ud83d\ude00" }"#;
        let selected = if source == "router" {
            r#""name":"Bad","supported_parameters":["tools"]"#
        } else {
            r#""tags":["tool-use"]"#
        };
        let body = format!(" \n{{\"data\":[{{\"id\":{id},{selected}}}]}} ");
        let mut errors = Vec::new();
        let models = runtime
            .block_on(invoke(
                source,
                &response_fetch(body.into_bytes()),
                &mut Vec::new(),
                &mut errors,
            ))
            .unwrap();
        assert!(models.is_empty());
        let label = if source == "router" {
            "OpenRouter"
        } else {
            "Vercel AI Gateway"
        };
        assert_eq!(
            String::from_utf8(errors).unwrap(),
            format!("Failed to fetch {label} models: skipping model {id}: invalid metadata\n")
        );
    }
}

#[test]
fn distinct_surrogate_entry_keys_survive_selection() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    for provider in ["anthropic", "opencode"] {
        for (members, names) in [
            (
                r#""\ud800":{"tool_call":true,"name":"First"},"\ud801":{"tool_call":false,"name":"Rejected"}"#,
                vec!["First"],
            ),
            (
                r#""\ud800":{"tool_call":true,"name":"First"},"\ufffd":{"tool_call":true,"name":"Second"}"#,
                vec!["First", "Second"],
            ),
            (
                r#""\ud800":{"tool_call":true,"name":"First"},"\ud800":{"tool_call":true,"name":"Last"}"#,
                vec!["Last"],
            ),
        ] {
            let text = format!("{{\"{provider}\":{{\"models\":{{{members}}}}}}}");
            let mut errors = Vec::new();
            let models = runtime
                .block_on(invoke(
                    "dev",
                    &response_fetch(text.into_bytes()),
                    &mut Vec::new(),
                    &mut errors,
                ))
                .unwrap();
            assert_eq!(
                models
                    .iter()
                    .map(|model| model.name.as_str())
                    .collect::<Vec<_>>(),
                names
            );
            assert!(models.iter().all(|model| model.id == "\u{fffd}"));
            assert!(errors.is_empty());
        }
    }
}

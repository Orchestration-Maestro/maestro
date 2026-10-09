//! Consumed request oracle inputs.

use super::super::request;
use crate::{Context, MistralOptions, Model, StreamOptions};
use serde::Deserialize;
use serde_json::Value;

/// One owned operation and expectation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Owning test.
    test: String,
    /// Executed operation.
    operation: String,
    /// Operation operands.
    input: Value,
    /// Complete outcome.
    expected: Value,
}

/// Build the source fixture's default descriptor with supplied overrides.
pub(super) fn model(input: &Value) -> Model {
    let mut base =
        serde_json::to_value(crate::get_model("mistral", "magistral-medium-latest").unwrap())
            .unwrap();
    base["maxTokens"] = serde_json::json!(8192);
    base["input"] = serde_json::json!(["text", "image"]);
    if let Some(overrides) = input.get("model").and_then(Value::as_object) {
        for (key, value) in overrides {
            base[key] = value.clone();
        }
    }
    serde_json::from_value(base).unwrap()
}

/// Construct common settings at the typed host boundary.
pub(super) fn common(value: &Value) -> StreamOptions {
    StreamOptions {
        temperature: value.get("temperature").map(|v| v.as_f64().unwrap()),
        max_tokens: value.get("maxTokens").map(|v| v.as_f64().unwrap()),
        api_key: value.get("apiKey").map(|v| v.as_str().unwrap().to_owned()),
        headers: value
            .get("headers")
            .map(|v| serde_json::from_value(v.clone()).unwrap()),
        timeout_ms: value.get("timeoutMs").map(|v| v.as_f64().unwrap()),
        max_retries: value.get("maxRetries").map(|v| v.as_f64().unwrap()),
        max_retry_delay_ms: value.get("maxRetryDelayMs").map(|v| v.as_f64().unwrap()),
        transport: value
            .get("transport")
            .map(|v| serde_json::from_value(v.clone()).unwrap()),
        metadata: value
            .get("metadata")
            .map(|v| serde_json::from_value(v.clone()).unwrap()),
        session_id: value
            .get("sessionId")
            .map(|v| v.as_str().unwrap().to_owned()),
        ..Default::default()
    }
}

/// Construct raw provider options without introducing an ingress decoder.
pub(super) fn options(input: &Value) -> MistralOptions {
    let value = &input["options"];
    MistralOptions {
        common: common(value),
        tool_choice: value.get("toolChoice").map(|v| match v.as_str() {
            Some("auto") => crate::MistralToolChoice::Auto,
            Some("none") => crate::MistralToolChoice::None,
            Some("any") => crate::MistralToolChoice::Any,
            Some("required") => crate::MistralToolChoice::Required,
            None => crate::MistralToolChoice::Function {
                name: v["function"]["name"].as_str().unwrap().into(),
            },
            _ => panic!("unknown choice"),
        }),
        prompt_mode: value.get("promptMode").map(|v| {
            assert_eq!(v, "reasoning");
            crate::MistralPromptMode::Reasoning
        }),
        reasoning_effort: value
            .get("reasoningEffort")
            .map(|v| match v.as_str().unwrap() {
                "high" => crate::MistralReasoningEffort::High,
                "none" => crate::MistralReasoningEffort::None,
                _ => panic!("unknown effort"),
            }),
    }
}

/// Compare callback text and the owning post-hook codec's result.
fn payload_case(input: &Value, expected: &Value, simple: bool) {
    let model = model(input);
    let context: Context = serde_json::from_value(
        input
            .get("context")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({"messages":[]})),
    )
    .unwrap();
    let (raw, effort) = if simple {
        let value = &input["options"];
        request::simple_options(
            &model,
            &crate::SimpleStreamOptions {
                common: common(value),
                reasoning: value
                    .get("reasoning")
                    .map(|v| serde_json::from_value(v.clone()).unwrap()),
                tool_choice: value
                    .get("toolChoice")
                    .map(|v| serde_json::from_value(v.clone()).unwrap()),
                ..Default::default()
            },
        )
    } else {
        (options(input), None)
    };
    let payload = request::build_chat_payload(&model, &context, &raw, effort.as_deref()).unwrap();
    assert_eq!(
        crate::providers::json_text::compact_json(&payload).unwrap(),
        expected["payload"].as_str().unwrap()
    );
    let wire = super::super::wire::encode_payload(&payload);
    if let Some(text) = expected.get("wire") {
        assert_eq!(
            crate::providers::json_text::compact_json(&wire.unwrap()).unwrap(),
            text.as_str().unwrap()
        );
    } else {
        assert_eq!(expected["wire_error"], true);
        assert!(wire.is_err());
    }
}

/// Verify uniqueness across the entire fixture and execute one owner's cases.
pub(super) fn corpus(name: &str, count: usize) {
    let cases = cases();
    let mut selected = 0;
    for case in cases {
        if case.test != name {
            continue;
        }
        selected += 1;
        pure_case(&case);
    }
    assert_eq!(selected, count);
}

/// Run one transport owner's unique cases with isolated credential environments.
pub(super) async fn async_corpus(name: &str, count: usize) {
    let cases = cases();
    let mut selected = 0;
    for case in cases.into_iter().filter(|case| case.test == name) {
        selected += 1;
        if case.operation == "endpoint" {
            endpoint_case(&case);
            continue;
        }
        if case.operation == "media_type" {
            assert_eq!(
                request::transport::media_type(case.input.as_str(), "text/event-stream"),
                case.expected["sse"].as_bool().unwrap()
            );
            assert_eq!(
                request::transport::media_type(case.input.as_str(), "application/json"),
                case.expected["json"].as_bool().unwrap()
            );
            continue;
        }
        assert_eq!(case.operation, "send");
        if let Some(key) = case.input["envKey"].as_str() {
            let result = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "providers::reasoning::mistral::tests::request_corpus::credential_child",
                    "--nocapture",
                    "--ignored",
                ])
                .env("MISTRAL_API_KEY", key)
                .env(
                    "MAESTRO_REQUEST_CASE",
                    serde_json::to_string(
                        &serde_json::json!({"input":case.input,"expected":case.expected}),
                    )
                    .unwrap(),
                )
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stdout)
            );
        } else {
            super::request_sending::check(&case.input, &case.expected).await;
        }
    }
    assert_eq!(selected, count);
}

#[test]
#[ignore = "executed in a subprocess with controlled credentials"]
fn credential_child() {
    let Ok(text) = std::env::var("MAESTRO_REQUEST_CASE") else {
        return;
    };
    let case: Value = serde_json::from_str(&text).unwrap();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(super::request_sending::check(
            &case["input"],
            &case["expected"],
        ));
}

/// Check the request options boundary.
fn headers_case(case: &Case) {
    let headers = request::authored_headers(&model(&case.input), &common(&case.input["options"]));
    assert_eq!(
        serde_json::to_value(headers.keys().collect::<Vec<_>>()).unwrap(),
        case.expected["headerKeys"]
    );
    if headers.is_empty() {
        assert!(case.expected.get("headers").is_none());
    } else {
        assert_eq!(
            serde_json::to_value(headers).unwrap(),
            case.expected["headers"]
        );
    }
}

/// Check the negative zero boundary.
fn negative_zero_case(case: &Case) {
    assert!(case.input.as_object().unwrap().is_empty());
    let mut options = options(&case.input);
    options.common.temperature = Some(-0.0);
    options.common.max_tokens = Some(-0.0);
    let payload = request::build_chat_payload(
        &model(&case.input),
        &serde_json::from_value(serde_json::json!({"messages":[]})).unwrap(),
        &options,
        None,
    )
    .unwrap();
    assert_eq!(
        payload["temperature"].as_f64().unwrap().is_sign_negative(),
        case.expected["temperatureNegative"].as_bool().unwrap()
    );
    assert_eq!(
        payload["maxTokens"].as_f64().unwrap().is_sign_negative(),
        case.expected["tokensNegative"].as_bool().unwrap()
    );
    assert_eq!(
        crate::providers::json_text::compact_json(
            &super::super::wire::encode_payload(&payload).unwrap()
        )
        .unwrap(),
        case.expected["wire"].as_str().unwrap()
    );
}

/// Execute one pure operation.
fn pure_case(case: &Case) {
    match case.operation.as_str() {
        "format_error" => assert_eq!(
            request::errors::format_error(
                case.input
                    .get("status")
                    .map(|v| u16::try_from(v.as_u64().unwrap()).unwrap()),
                case.input.get("body").map(|v| v.as_str().unwrap()),
                case.input["message"].as_str().unwrap()
            ),
            case.expected.as_str().unwrap()
        ),
        "derive_id" => assert_eq!(
            request::tool_ids::derive(
                case.input["id"].as_str().unwrap(),
                case.input["attempt"].as_u64().unwrap()
            ),
            case.expected.as_str().unwrap()
        ),
        "normalize_ids" => {
            let mut ids = request::tool_ids::ToolCallIds::default();
            let actual: Vec<_> = case
                .input
                .as_array()
                .unwrap()
                .iter()
                .map(|v| ids.normalize(v.as_str().unwrap()))
                .collect();
            assert_eq!(serde_json::to_value(actual).unwrap(), case.expected);
        }
        "simple_payload" => payload_case(&case.input, &case.expected, true),
        "request_options" => headers_case(case),
        "payload" => payload_case(&case.input, &case.expected, false),
        "negative_zero" => negative_zero_case(case),
        other => panic!("unsupported operation: {other}"),
    }
}

/// Check URL success or the native failing phase.
fn endpoint_case(case: &Case) {
    let result = request::transport::base_url(case.input["baseUrl"].as_str().unwrap())
        .and_then(|base| request::transport::endpoint(&base));
    assert_eq!(result.is_ok(), case.expected["ok"].as_bool().unwrap());
    if let Some(url) = case.expected["url"].as_str() {
        assert_eq!(result.unwrap(), url);
    } else if let Some(message) = case.expected["message"].as_str() {
        assert_eq!(result.unwrap_err().message, message);
    } else {
        assert_eq!(case.expected["native_error"], true);
        assert!(!result.unwrap_err().message.is_empty());
    }
}

/// Read the strict fixture and retain operation/input identity after expansion.
fn cases() -> Vec<Case> {
    let mut cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/reasoning-request.json")).unwrap();
    let mut seen = std::collections::HashSet::new();
    for case in &mut cases {
        super::request_fixture::expand(&mut case.input);
        super::request_fixture::expand(&mut case.expected);
        super::request_fixture::expectation(&case.operation, &case.expected);
        assert!(seen.insert((
            case.operation.clone(),
            serde_json::to_string(&case.input).unwrap()
        )));
    }
    assert_eq!(cases.len(), 466);
    cases
}

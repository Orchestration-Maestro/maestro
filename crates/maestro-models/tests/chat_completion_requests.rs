//! Request construction for direct chat-completion invocations.

#[cfg(not(target_arch = "wasm32"))]
use crate::cases;
use crate::chat;
#[cfg(not(target_arch = "wasm32"))]
use crate::json;
#[cfg(not(target_arch = "wasm32"))]
use crate::loopback;
#[cfg(not(target_arch = "wasm32"))]
use crate::transport;

#[cfg(not(target_arch = "wasm32"))]
use cases::{assert_rows, run_case};
use chat::{TestResult, context, model, rows};
#[cfg(not(target_arch = "wasm32"))]
use json::canonical;
use maestro_models::providers::chat::openai_completions::ResolvedOpenAICompletionsCompat;
use maestro_models::providers::chat::openai_completions::messages::convert_messages;
#[cfg(not(target_arch = "wasm32"))]
use maestro_models::{Model, ProviderResponse, StopReason, StreamOptions, get_model};
use maestro_models::{ModelCompat, OpenAICompletionsCompat};
use serde_json::{Value, json};
#[cfg(not(target_arch = "wasm32"))]
use std::collections::BTreeMap;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, Mutex};

const FIXTURE: &str = include_str!("fixtures/chat_completions/requests.json");

/// Convert a fixture row's history under the compatibility it names.
fn converted(row: &Value) -> TestResult<Value> {
    let mut model = model(&row["model"])?;
    if let Some(overrides) = row.get("compat") {
        let compat: OpenAICompletionsCompat = serde_json::from_value(overrides.clone())?;
        model.compat = Some(ModelCompat::from(compat));
    }
    let compat = ResolvedOpenAICompletionsCompat::from(&model);
    let wire = convert_messages(&model, &context(&row["context"])?, &compat)?;
    Ok(serde_json::to_value(wire)?)
}

fn assert_history_rows(test: &str) -> TestResult {
    let rows = rows(FIXTURE, test)?;
    assert!(!rows.is_empty(), "{test} has fixture rows");
    for row in rows {
        assert_eq!(converted(&row)?, row["expected"], "{}", row["id"]);
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_chat_replays_thinking_as_text() -> TestResult {
    crate::runtime::run(async {
        assert_history_rows("maestro_chat_replays_thinking_as_text")?;
        if !maestro_test_process::selection().is_child() {
            return {
                let output = maestro_test_process::TestProcess::current(
                    "maestro_chat_replays_thinking_as_text",
                )?
                .nocapture()
                .env_clear()
                .case("*")
                .output()?;
                assert!(output.status.success(), "{output:?}");
                Ok(())
            };
        }
        let server = loopback::serve(
            loopback::EVENT_STREAM_HEAD,
            loopback::Step::writes(vec![b"data: [DONE]\n\n".to_vec()]),
        )?;
        let context = json!({"messages": [
        {"role": "user", "content": "question"},
        {"role": "assistant", "content": [
            {"type": "thinking", "thinking": "first"},
            {"type": "thinking", "thinking": "second"},
            {"type": "text", "text": "visible"}]},
        {"role": "user", "content": "follow up"}]});
        let case = json!({
            "model": {"baseUrl": format!("{}/v1", server.url), "compat": {"requiresThinkingAsText": true}},
            "context": context, "options": {}, "noFetch": true
        });
        run_case(&case).await?;
        let received = server.finish()?;
        let payload: Value = serde_json::from_slice(&received.body)?;
        assert_eq!(
            payload,
            json!({
                "model": "model", "stream": true, "stream_options": {"include_usage": true},
                "store": false,
                "messages": [
                    {"role": "user", "content": "question"},
                    {"role": "assistant", "content": [
                        {"type": "text", "text": "first\n\nsecond"},
                        {"type": "text", "text": "visible"}]},
                    {"role": "user", "content": "follow up"}]
            })
        );
        Ok(())
    })
}

#[test]
fn maestro_chat_preserves_assistant_content() -> TestResult {
    assert_history_rows("maestro_chat_preserves_assistant_content")
}

#[test]
fn maestro_chat_groups_tool_result_images() -> TestResult {
    assert_history_rows("maestro_chat_groups_tool_result_images")
}

#[test]
fn maestro_chat_preserves_unicode_boundaries() -> TestResult {
    assert_history_rows("maestro_chat_preserves_unicode_boundaries")
}

#[test]
fn maestro_chat_normalizes_tool_identifiers() -> TestResult {
    assert_history_rows("maestro_chat_normalizes_tool_identifiers")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_chat_serializes_replay_arguments() -> TestResult {
    assert_history_rows("maestro_chat_serializes_replay_arguments")?;
    let history = json!({"messages": [
        {"role": "assistant", "content": [{"type": "toolCall", "id": "call", "name": "lookup",
            "arguments": {}}]}]});
    let mut context = context(&history)?;
    let raw =
        r#"{"10":10,"2":2,"z":"line\n\"☃","a":1e21,"b":1e-7,"c":-0,"d":{"9":[1.0,0.5],"1":null}}"#;
    if let Some(maestro_models::Message::Assistant(message)) = context.messages.first_mut()
        && let Some(maestro_models::AssistantContent::ToolCall(call)) = message.content.first_mut()
    {
        call.arguments = serde_json::from_str(raw)?;
    }
    let model = model(&json!({}))?;
    let wire = convert_messages(
        &model,
        &context,
        &ResolvedOpenAICompletionsCompat::from(&model),
    )?;
    let wire = serde_json::to_value(wire)?;
    assert_eq!(
        wire[0]["tool_calls"][0]["function"]["arguments"],
        json!(
            r#"{"2":2,"10":10,"z":"line\n\"☃","a":1e+21,"b":1e-7,"c":0,"d":{"1":null,"9":[1,0.5]}}"#
        )
    );
    crate::runtime::run_paused(replayed_numbers())
}

#[cfg(not(target_arch = "wasm32"))]
/// Replayed arguments and signatures carry numbers the way their double prints, in the request
/// text that goes out: integers beyond 2^53 round, and a number beyond the range of a double
/// is `null` once written but still counts as present when the signature is read.
async fn replayed_numbers() -> TestResult {
    let signatures = [
        r#"{"type":"reasoning.encrypted","id":"c1","data":9007199254740993}"#,
        r#"{"type":"reasoning.encrypted","id":"c2","data":1e400}"#,
        "1e400",
        "-0",
        "1e-400",
        "0",
        r#"{"a":[1e400,{"10":1e21,"2":-0}],"b":18446744073709551615}"#,
        "-1e400",
        r#""""#,
    ];
    let calls: Vec<Value> = signatures
        .iter()
        .enumerate()
        .map(|(position, signature)| {
            json!({"type": "toolCall", "id": format!("c{}", position + 1), "name": "lookup",
                "arguments": {}, "thoughtSignature": signature})
        })
        .collect();
    let history = json!({"messages": [
        {"role": "user", "content": "question"},
        {"role": "assistant", "content": calls}]});
    let target = transport::transport(vec![transport::Attempt::success()]);
    let (model, _, common) = transport::call_inputs(&target)?;
    let mut context = context(&history)?;
    if let Some(maestro_models::Message::Assistant(message)) = context.messages.get_mut(1)
        && let Some(maestro_models::AssistantContent::ToolCall(call)) = message.content.first_mut()
    {
        call.arguments = serde_json::from_str(
            r#"{"u":18446744073709551615,"i":9007199254740993,"neg":-9007199254740993,"f":0.1}"#,
        )?;
    }
    transport::finish(model, context, common).await?;
    let body = String::from_utf8(target.first_request()?.1)?;
    let sent: Value = serde_json::from_str(&body)?;
    assert_eq!(
        sent["messages"][1]["tool_calls"][0]["function"]["arguments"],
        r#"{"u":18446744073709552000,"i":9007199254740992,"neg":-9007199254740992,"f":0.1}"#
    );
    let details = concat!(
        r#""reasoning_details":[{"type":"reasoning.encrypted","id":"c1","data":9007199254740992},"#,
        r#"{"type":"reasoning.encrypted","id":"c2","data":null},null,"#,
        r#"{"a":[null,{"2":0,"10":1e+21}],"b":18446744073709552000},null]"#
    );
    assert!(body.contains(details), "{body}");
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
/// Check one fixture row's request body, and the headers it names, against what was sent.
///
/// A `headers` object lists header values the request must carry; `null` means absent.
async fn payload_matches(row: &Value) -> TestResult {
    let case = json!({"model": row["model"], "context": row["context"], "options": row["options"]});
    let observed = run_case(&case).await?;
    let request = observed.requests.first().ok_or("no request")?;
    assert_eq!(
        canonical(request["body"].clone()),
        canonical(row["expected"].clone()),
        "{}",
        row["id"]
    );
    for (name, expected) in row["headers"].as_object().into_iter().flatten() {
        assert_eq!(
            request["headers"].get(name).unwrap_or(&Value::Null),
            expected,
            "{} header {name}",
            row["id"]
        );
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
/// Check payload rows in fresh processes so ambient environment cannot reach them.
///
/// Rows with the same environment run together in one child holding exactly those
/// variables.
async fn assert_payload_rows(test: &str) -> TestResult {
    let owned = rows(FIXTURE, test)?;
    assert!(!owned.is_empty(), "{test} has fixture rows");
    let group = maestro_test_process::run_per_environment(test, &owned, |row| {
        row.get("env")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .filter_map(|(name, value)| Some((name.clone(), value.as_str()?.to_owned())))
            .collect::<BTreeMap<_, _>>()
    })?;
    for row in group.into_iter().flatten() {
        payload_matches(row).await?;
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
/// Send one call with the given numeric options; return the payload the hook saw and the body
/// that went out. A `replacement` makes the hook swap the payload.
async fn send_numbers(
    temperature: f64,
    max_tokens: f64,
    replacement: Option<Value>,
) -> TestResult<(Value, Value)> {
    let target = transport::transport(vec![transport::Attempt::success()]);
    let (model, history, mut common) = transport::call_inputs(&target)?;
    common.temperature = Some(temperature);
    common.max_tokens = Some(max_tokens);
    let seen: Arc<Mutex<Vec<Value>>> = Arc::default();
    let kept = Arc::clone(&seen);
    common.on_payload = Some(Arc::new(
        move |maestro_models::PayloadHookInput {
                  payload,
                  signal: hook_signal,
              },
              _| {
            kept.lock().map(|mut kept| kept.push(payload.clone())).ok();
            Box::pin(std::future::ready(Ok(maestro_models::PayloadHookInput {
                payload: replacement.clone().unwrap_or(payload),
                signal: hook_signal,
            })))
        },
    ));
    let outcome = transport::finish(model, history, common).await?;
    assert!(
        matches!(outcome.stop_reason, StopReason::Stop),
        "{temperature} {max_tokens}: {:?}",
        outcome.error
    );
    let hooked = seen.lock().map_err(|e| e.to_string())?.remove(0);
    let sent = serde_json::from_slice(&target.first_request()?.1)?;
    Ok((hooked, sent))
}

#[cfg(not(target_arch = "wasm32"))]
/// Numbers no JSON number can hold are written as `null`, and are visible to, and replaceable
/// by, the payload hook.
async fn non_finite_numbers_become_null() -> TestResult {
    for (temperature, max_tokens, limit_sent) in [
        (f64::NAN, f64::INFINITY, true),
        (f64::INFINITY, f64::NEG_INFINITY, true),
        (f64::NEG_INFINITY, f64::NAN, false),
    ] {
        let (hooked, sent) = send_numbers(temperature, max_tokens, None).await?;
        assert_eq!(
            hooked, sent,
            "{temperature} {max_tokens}: the hook sees the body"
        );
        assert_eq!(sent.get("temperature"), Some(&Value::Null), "{temperature}");
        assert_eq!(
            sent.get("max_completion_tokens"),
            limit_sent.then_some(&Value::Null),
            "{max_tokens}"
        );

        let replacement = json!({"model": "replaced", "temperature": 0.5});
        let (_, sent) = send_numbers(temperature, max_tokens, Some(replacement.clone())).await?;
        assert_eq!(sent, replacement, "{temperature} {max_tokens}");
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_chat_preserves_numeric_options() -> TestResult {
    crate::runtime::run(async {
        assert_payload_rows("maestro_chat_preserves_numeric_options").await?;
        if !maestro_test_process::selection().is_child() {
            non_finite_numbers_become_null().await?;
        }
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// Send one call whose payload hook turns the payload it receives into the one to send; return
/// the body that went out.
async fn send_through_hook(
    hook: impl Fn(Value) -> Value + Send + Sync + 'static,
) -> TestResult<Value> {
    let target = transport::transport(vec![transport::Attempt::success()]);
    let (model, history, mut common) = transport::call_inputs(&target)?;
    common.temperature = Some(0.25);
    common.on_payload = Some(Arc::new(
        move |maestro_models::PayloadHookInput {
                  payload,
                  signal: hook_signal,
              },
              _| {
            Box::pin(std::future::ready(Ok(maestro_models::PayloadHookInput {
                payload: hook(payload),
                signal: hook_signal,
            })))
        },
    ));
    let outcome = transport::finish(model, history, common).await?;
    assert!(
        matches!(outcome.stop_reason, StopReason::Stop),
        "{:?}",
        outcome.error
    );
    Ok(serde_json::from_slice(&target.first_request()?.1)?)
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_chat_sends_the_payload_the_hook_returns() -> TestResult {
    crate::runtime::run(async {
        let unchanged = send_through_hook(|payload| payload).await?;
        assert_eq!(unchanged["temperature"], json!(0.25));
        assert_eq!(unchanged["model"], "model");

        let edited = send_through_hook(|mut payload| {
            payload["temperature"] = json!(0.75);
            payload
        })
        .await?;
        let mut expected = unchanged;
        expected["temperature"] = json!(0.75);
        assert_eq!(
            edited, expected,
            "an edit made in place reaches the request"
        );

        let replacement = json!({"model": "replaced", "temperature": 0.5});
        let sent = send_through_hook({
            let replacement = replacement.clone();
            move |_| replacement.clone()
        })
        .await?;
        assert_eq!(sent, replacement, "a new payload replaces the original");
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_chat_selects_reasoning_payloads() -> TestResult {
    crate::runtime::run(async {
        assert_payload_rows("maestro_chat_selects_reasoning_payloads").await
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_chat_preserves_routing_fields() -> TestResult {
    crate::runtime::run(async {
        assert_payload_rows("maestro_chat_preserves_routing_fields").await
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_chat_places_cache_markers() -> TestResult {
    crate::runtime::run(async { assert_payload_rows("maestro_chat_places_cache_markers").await })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_chat_preserves_prompt_cache_policy() -> TestResult {
    crate::runtime::run(async {
        assert_payload_rows("maestro_chat_preserves_prompt_cache_policy").await
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_chat_preserves_tool_choice() -> TestResult {
    crate::runtime::run(async { assert_payload_rows("maestro_chat_preserves_tool_choice").await })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_chat_preserves_tool_declarations() -> TestResult {
    crate::runtime::run(async {
        assert_payload_rows("maestro_chat_preserves_tool_declarations").await
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// Whether a request for `model` with or without tools asks the provider to stream tool calls.
async fn requests_tool_stream(model: &maestro_models::Model, with_tools: bool) -> TestResult<bool> {
    let ping = json!({"name": "ping", "description": "Ping tool", "parameters": {
        "type": "object", "properties": {"ok": {"type": "boolean"}}, "required": ["ok"]}});
    let case = json!({
        "model": serde_json::to_value(model)?,
        "context": {"messages": [{"role": "user", "content": "Call ping with ok=true"}],
            "tools": if with_tools { json!([ping]) } else { Value::Null }},
        "options": {}
    });
    let observed = run_case(&case).await?;
    let body = &observed.requests.first().ok_or("no request")?["body"];
    Ok(body.get("tool_stream") == Some(&json!(true)))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_chat_streams_tools_for_catalog_models() -> TestResult {
    crate::runtime::run(async {
        let supported = get_model("zai", "glm-5.1").ok_or("glm-5.1")?;
        assert!(
            requests_tool_stream(&supported, true).await?,
            "a marked model streams tools"
        );
        assert!(
            !requests_tool_stream(&supported, false).await?,
            "no tools, no tool stream"
        );

        let mut model = get_model("zai", "glm-4.5-air").ok_or("glm-4.5-air")?;
        assert!(
            !requests_tool_stream(&model, true).await?,
            "an unmarked model does not"
        );
        if let Some(compat) = &mut model.compat {
            compat.0.insert("zaiToolStream".into(), true.into());
        }
        assert!(
            requests_tool_stream(&model, true).await?,
            "an explicit mark overrides"
        );
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// A history and options that make every compatibility capability visible in the request.
fn probe_case(model: &Value, thinking_format: Option<&str>) -> TestResult<Value> {
    let mut model = model.clone();
    let target = model.as_object_mut().ok_or("model overrides")?;
    if let Some(format) = thinking_format {
        let compat = target.entry("compat").or_insert_with(|| json!({}));
        compat["thinkingFormat"] = json!(format);
    }
    let provider = target.get("provider").cloned().unwrap_or(json!("fixture"));
    let id = target.get("id").cloned().unwrap_or(json!("model"));
    let assistant = |content: Value| {
        json!({"role": "assistant", "content": content, "api": "openai-completions",
            "provider": provider, "model": id})
    };
    Ok(json!({
        "model": model,
        "context": {
            "systemPrompt": "instructions",
            "tools": [{"name": "lookup", "description": "d", "parameters": {"type": "object"}}],
            "messages": [
                {"role": "user", "content": "first"},
                assistant(json!([{"type": "toolCall", "id": "call", "name": "lookup", "arguments": {}}])),
                {"role": "toolResult", "toolCallId": "call", "toolName": "lookup",
                    "content": [{"type": "text", "text": "found"}]},
                {"role": "user", "content": "second"},
                assistant(json!([{"type": "thinking", "thinking": "plan"},
                    {"type": "text", "text": "answer"}])),
                {"role": "user", "content": "third"}
            ]
        },
        "options": {"maxTokens": 7, "sessionId": "session", "cacheRetention": "long",
            "reasoningEffort": "high"}
    }))
}

#[cfg(not(target_arch = "wasm32"))]
/// Read the compatibility a model resolves to from the requests it produces.
async fn observed_compat(model: &Value) -> TestResult<Value> {
    let observed = run_case(&probe_case(model, None)?).await?;
    let request = observed.requests.first().ok_or("no request")?;
    let body = &request["body"];
    let messages = body["messages"].as_array().ok_or("messages")?;
    let with_role = |role: &'static str| messages.iter().filter(move |m| m["role"] == role);
    let reasoning_by_effort = {
        let plain = run_case(&probe_case(model, Some("openai"))?).await?;
        plain
            .requests
            .first()
            .is_some_and(|r| r["body"].get("reasoning_effort").is_some())
    };
    let format = if body.get("chat_template_kwargs").is_some() {
        "qwen-chat-template"
    } else if body.get("thinking").is_some() {
        // Both thinking-object formats send the object; only the first adds the effort field
        // once the model accepts one.
        let mut accepting = model.clone();
        let target = accepting.as_object_mut().ok_or("model overrides")?;
        let compat = target.entry("compat").or_insert_with(|| json!({}));
        compat["supportsReasoningEffort"] = json!(true);
        let forced = run_case(&probe_case(&accepting, None)?).await?;
        let sends_effort = forced
            .requests
            .first()
            .is_some_and(|r| r["body"].get("reasoning_effort").is_some());
        if sends_effort { "deepseek" } else { "zai" }
    } else if body.get("reasoning").is_some() {
        "openrouter"
    } else if body.get("enable_thinking").is_some() {
        "qwen"
    } else {
        "openai"
    };
    Ok(canonical(json!({
        "supportsStore": body.get("store").is_some(),
        "supportsDeveloperRole": messages[0]["role"] == "developer",
        "supportsReasoningEffort": reasoning_by_effort,
        "supportsUsageInStreaming": body.get("stream_options").is_some(),
        "maxTokensField": if body.get("max_tokens").is_some() { "max_tokens" } else { "max_completion_tokens" },
        "requiresToolResultName": with_role("tool").any(|m| m.get("name").is_some()),
        "requiresAssistantAfterToolResult": with_role("assistant")
            .any(|m| m["content"] == "I have processed the tool results."),
        "requiresThinkingAsText": with_role("assistant").any(|m| m["content"].is_array()),
        "requiresReasoningContentOnAssistantMessages": with_role("assistant")
            .any(|m| m.get("reasoning_content").is_some()),
        "thinkingFormat": format,
        "zaiToolStream": body.get("tool_stream").is_some(),
        "supportsStrictMode": body["tools"][0]["function"].get("strict").is_some(),
        "cacheControlFormat": body.to_string().contains("cache_control").then_some("anthropic"),
        "sendSessionAffinityHeaders": request["headers"].get("session_id").is_some(),
        "supportsLongCacheRetention": body["prompt_cache_retention"] == "24h"
    })))
}

#[cfg(not(target_arch = "wasm32"))]
async fn assert_compat_rows(test: &str) -> TestResult {
    let owned = rows(FIXTURE, test)?;
    assert!(!owned.is_empty(), "{test} has fixture rows");
    for row in owned {
        let mut expected = canonical(row["expected"].clone());
        if expected["cacheControlFormat"].is_null() {
            expected["cacheControlFormat"] = Value::Null;
        }
        assert_eq!(
            observed_compat(&row["model"]).await?,
            expected,
            "{}",
            row["id"]
        );
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_chat_derives_compatibility() -> TestResult {
    crate::runtime::run(async { assert_compat_rows("maestro_chat_derives_compatibility").await })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_chat_applies_compatibility_overrides() -> TestResult {
    crate::runtime::run(async {
        assert_compat_rows("maestro_chat_applies_compatibility_overrides").await
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_chat_preserves_header_precedence() -> TestResult {
    crate::runtime::run(async {
        assert_rows(FIXTURE, "maestro_chat_preserves_header_precedence").await
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// Run one call whose caller headers are `headers`; return the transport and the outcome.
async fn send_with_headers(
    headers: &[(&str, &str)],
) -> TestResult<(transport::Transport, transport::Outcome)> {
    let target = transport::transport(vec![transport::Attempt::success()]);
    let (model, history, mut common) = transport::call_inputs(&target)?;
    common.headers = Some(
        headers
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect(),
    );
    let outcome = transport::finish(model, history, common).await?;
    Ok((target, outcome))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_chat_normalizes_request_header_values() -> TestResult {
    crate::runtime::run_paused(async {
        let padded = [
            ("x-edge", " \t\r\nvalue\n\t "),
            ("x-empty", " \t"),
            ("x-nbsp", "\u{a0}v\u{a0}"),
            ("x-next-line", "\u{85}v\u{85}"),
            ("x-latin", "\u{e9}\u{80}\u{ff}"),
        ];
        let (target, outcome) = send_with_headers(&padded).await?;
        assert!(
            matches!(outcome.stop_reason, StopReason::Stop),
            "{:?}",
            outcome.error
        );
        let headers = {
            let sent = target.requests.lock().map_err(|error| error.to_string())?;
            sent.first().ok_or("no request")?.headers.clone()
        };
        let seen = |name: &str| headers.get(name).map(String::as_str);
        assert_eq!(seen("x-edge"), Some("value"));
        assert_eq!(seen("x-empty"), Some(""));
        assert_eq!(seen("x-nbsp"), Some("\u{a0}v\u{a0}"));
        assert_eq!(seen("x-next-line"), Some("\u{85}v\u{85}"));
        assert_eq!(seen("x-latin"), Some("\u{e9}\u{80}\u{ff}"));

        let unsendable = [
            ("x-wide", "\u{100}"),
            ("x-wide", "\u{feff}v\u{feff}"),
            ("x-wide", "a\u{1f600}"),
            ("x-line", "a\nb"),
            ("x-nul", "a\0b"),
            ("bad name", "v"),
        ];
        for (name, value) in unsendable {
            let (target, outcome) = send_with_headers(&[(name, value)]).await?;
            assert_eq!(
                target.attempts(),
                0,
                "{name}: {value:?} reached the transport"
            );
            assert!(matches!(outcome.stop_reason, StopReason::Error), "{name}");
            let error = outcome.error.unwrap_or_default();
            assert!(error.contains(name), "{error}");
        }
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_chat_preserves_gateway_authorization() -> TestResult {
    crate::runtime::run(async {
        assert_rows(FIXTURE, "maestro_chat_preserves_gateway_authorization").await
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// What the hooks of one call received, in the order they ran.
#[derive(Default)]
struct HookRecord {
    calls: Mutex<Vec<(&'static str, Arc<Model>)>>,
    payloads: Mutex<Vec<Value>>,
    responses: Mutex<Vec<ProviderResponse>>,
}

#[cfg(not(target_arch = "wasm32"))]
/// Append to a recorded list.
fn record<T>(list: &Mutex<Vec<T>>, item: T) {
    if let Ok(mut list) = list.lock() {
        list.push(item);
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Install hooks that record what they receive and change nothing.
fn record_hooks(common: &mut StreamOptions) -> Arc<HookRecord> {
    let seen = Arc::new(HookRecord::default());
    let on_payload = Arc::clone(&seen);
    common.on_payload = Some(Arc::new(
        move |maestro_models::PayloadHookInput {
                  payload,
                  signal: hook_signal,
              },
              model| {
            record(&on_payload.calls, ("payload", model));
            record(&on_payload.payloads, payload.clone());
            Box::pin(std::future::ready(Ok(maestro_models::PayloadHookInput {
                payload,
                signal: hook_signal,
            })))
        },
    ));
    let on_response = Arc::clone(&seen);
    common.on_response = Some(Arc::new(move |response, model| {
        record(&on_response.calls, ("response", model));
        record(&on_response.responses, response);
        Box::pin(std::future::ready(Ok(())))
    }));
    seen
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_chat_observes_callback_boundaries() -> TestResult {
    crate::runtime::run_paused(async {
        assert_rows(FIXTURE, "maestro_chat_observes_callback_boundaries").await?;

        let target = transport::transport(vec![
            transport::Attempt::body(503, &[("retry-after-ms", "1")], Vec::new()),
            transport::Attempt::body(
                200,
                &[("x-result", "fixture")],
                b"data: [DONE]\n\n".to_vec(),
            ),
        ]);
        let (model, history, mut common) = transport::call_inputs(&target)?;
        let seen = record_hooks(&mut common);
        transport::finish(model, history, common).await?;
        assert_eq!(target.attempts(), 2, "the first response was retried");

        let calls = seen.calls.lock().map_err(|e| e.to_string())?;
        let order: Vec<_> = calls.iter().map(|(hook, _)| *hook).collect();
        assert_eq!(order, ["payload", "response"]);
        assert_eq!(calls[0].1.id, "model");
        assert!(
            Arc::ptr_eq(&calls[0].1, &calls[1].1),
            "both hooks see the one model the request holds"
        );
        let responses = seen.responses.lock().map_err(|e| e.to_string())?;
        assert_eq!(responses.len(), 1, "retry intermediates are not reported");
        assert_eq!(responses[0].status.to_bits(), 200.0_f64.to_bits());
        assert_eq!(
            responses[0].headers.get("x-result").map(String::as_str),
            Some("fixture")
        );
        let sent = target.requests.lock().map_err(|e| e.to_string())?;
        let body: Value = serde_json::from_slice(&sent[0].body)?;
        assert_eq!(
            seen.payloads.lock().map_err(|e| e.to_string())?[0],
            body,
            "the hook sees the sent payload"
        );
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_chat_preserves_simple_options() -> TestResult {
    crate::runtime::run(async {
        assert_rows(FIXTURE, "maestro_chat_preserves_simple_options").await
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_chat_preserves_auth_boundaries() -> TestResult {
    crate::runtime::run(async {
        assert_rows(FIXTURE, "maestro_chat_preserves_auth_boundaries").await
    })
}

/// The replayed assistant message for a tool call whose thought signature is `signature`.
fn replayed_with_signature(signature: &str) -> TestResult<Value> {
    let call = json!({"type": "toolCall", "id": "call", "name": "lookup", "arguments": {},
        "thoughtSignature": signature});
    let row =
        json!({"model": {}, "context": {"messages": [{"role": "assistant", "content": [call]}]}});
    Ok(converted(&row)?[0].clone())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_chat_bounds_signature_replay_nesting() -> TestResult {
    if maestro_test_process::selection().is_child() {
        let deepest = cases::DEEPEST_NESTING;
        let accepted = cases::nested_json(deepest);
        assert_eq!(
            replayed_with_signature(&accepted)?["reasoning_details"],
            json!([serde_json::from_str::<Value>(&accepted)?]),
            "the deepest accepted signature is replayed whole"
        );
        for containers in [100_000, deepest + 1] {
            let message = replayed_with_signature(&cases::nested_json(containers))?;
            assert!(
                message.get("reasoning_details").is_none(),
                "{containers} containers are not a signature"
            );
        }
        return Ok(());
    }
    {
        let output = maestro_test_process::TestProcess::current(
            "maestro_chat_bounds_signature_replay_nesting",
        )?
        .nocapture()
        .env_clear()
        .case("nesting")
        .output()?;
        assert!(output.status.success(), "{output:?}");
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_chat_forwards_control_characters_to_the_transport() -> TestResult {
    crate::runtime::run_paused(async {
        let value = "\u{b}a\u{1}b\u{7f}c\u{c}";
        let (target, outcome) = send_with_headers(&[("x-control", value)]).await?;
        assert!(
            matches!(outcome.stop_reason, StopReason::Stop),
            "{:?}",
            outcome.error
        );
        let sent = target.requests.lock().map_err(|error| error.to_string())?;
        let header = sent.first().ok_or("no request")?.headers.get("x-control");
        assert_eq!(header.map(String::as_str), Some(value));
        Ok(())
    })
}

#[test]
fn maestro_chat_omits_unrepresentable_replayed_signatures() -> TestResult {
    let message = replayed_with_signature(r#"{"\ud800":7,"keep":1}"#)?;
    assert!(message.get("reasoning_details").is_none(), "{message}");
    assert_eq!(message["tool_calls"][0]["id"], "call");
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CompatibilityQuery {
    test: String,
    input: Value,
    expected: Value,
}

#[cfg(not(target_arch = "wasm32"))]
async fn open_chat_options(wrong_types: bool) -> TestResult {
    let rows: Vec<CompatibilityQuery> =
        serde_json::from_str(include_str!("fixtures/compatibility_requests.json"))?;
    for (index, row) in rows
        .into_iter()
        .enumerate()
        .filter(|(_, row)| row.test == "chat")
    {
        let unconventional = row.input["compat"]
            .as_object()
            .ok_or("compat object")?
            .values()
            .any(|value| {
                value.is_null() || value.is_number() || value.is_array() || value.is_object()
            });
        if unconventional != wrong_types {
            continue;
        }
        let case = json!({"model":{"compat":row.input["compat"]},"context":{"messages":[],"tools":[{"name":"test","description":"controlled","parameters":{"type":"object"}}]},"options":{"apiKey":"fixture-key","maxTokens":7,"sessionId":"session","cacheRetention":row.input["retention"]}});
        let observed = run_case(&case).await?;
        let body = &observed.requests[0]["body"];
        for key in [
            "store",
            "stream_options",
            "max_tokens",
            "max_completion_tokens",
            "tool_stream",
            "prompt_cache_retention",
            "tools",
        ] {
            assert_eq!(body[key], row.expected[key], "row {index} {key}");
        }
        let expected = row.expected["affinity"]
            .as_object()
            .ok_or("expected object")?;
        for key in ["session_id", "x-client-request-id", "x-session-affinity"] {
            assert_eq!(
                observed.requests[0]["headers"].get(key),
                expected.get(key),
                "row {index} header {key}"
            );
        }
    }
    open_reasoning_options(wrong_types).await?;
    if wrong_types {
        open_history_options()?;
    }
    Ok(())
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn chat_open_options_control_flags_and_literals() -> TestResult {
    crate::runtime::run(open_chat_options(false))
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn chat_open_options_keep_nullish_and_strict_decisions() -> TestResult {
    crate::runtime::run(open_chat_options(true))
}

#[cfg(not(target_arch = "wasm32"))]
fn key_paths(value: &Value) -> Vec<String> {
    match value {
        Value::Object(map) => map
            .iter()
            .flat_map(|(key, child)| {
                std::iter::once(key.clone()).chain(
                    key_paths(child)
                        .into_iter()
                        .map(move |path| format!("{key}.{path}")),
                )
            })
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn routing_options(gateway: bool) -> TestResult {
    let rows: Vec<CompatibilityQuery> =
        serde_json::from_str(include_str!("fixtures/compatibility_requests.json"))?;
    for (index, row) in rows
        .into_iter()
        .enumerate()
        .filter(|(_, row)| row.test == "routing")
    {
        let authored_gateway = row.input["compat"].get("vercelGatewayRouting").is_some();
        if authored_gateway != gateway {
            continue;
        }
        let observed = run_case(&json!({"model":{"baseUrl":row.input["baseUrl"],"compat":row.input["compat"]},"context":{"messages":[]},"options":{"apiKey":"fixture-key"}})).await?;
        for key in ["provider", "providerOptions"] {
            assert_eq!(
                observed.requests[0]["body"][key], row.expected[key],
                "routing row {index} {key}"
            );
            assert_eq!(
                key_paths(&observed.requests[0]["body"][key]),
                key_paths(&row.expected[key]),
                "routing row {index} {key} key order"
            );
        }
    }
    Ok(())
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn chat_forwards_router_values_without_narrowing() -> TestResult {
    crate::runtime::run(routing_options(false))
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn chat_gateway_selects_only_requested_routing_fields() -> TestResult {
    crate::runtime::run(routing_options(true))
}

#[cfg(not(target_arch = "wasm32"))]
async fn open_reasoning_options(wrong_types: bool) -> TestResult {
    let rows: Vec<CompatibilityQuery> =
        serde_json::from_str(include_str!("fixtures/compatibility_requests.json"))?;
    for (index, row) in rows
        .into_iter()
        .enumerate()
        .filter(|(_, row)| row.test == "reasoning")
    {
        let unconventional = row.input["compat"]
            .as_object()
            .ok_or("object")?
            .values()
            .any(|value| {
                value.is_null() || value.is_number() || value.is_array() || value.is_object()
            });
        if unconventional != wrong_types {
            continue;
        }
        let observed = run_case(&json!({"model":{"compat":row.input["compat"],"thinkingLevelMap":row.input["thinkingLevelMap"]},"context":{"messages":[]},"options":row.input["options"]})).await?;
        for key in [
            "enable_thinking",
            "chat_template_kwargs",
            "thinking",
            "reasoning_effort",
            "reasoning",
        ] {
            assert_eq!(
                observed.requests[0]["body"][key], row.expected[key],
                "reasoning row {index} {key}"
            );
        }
    }
    Ok(())
}
#[cfg(not(target_arch = "wasm32"))]
fn open_history_options() -> TestResult {
    let all: Vec<Value> = serde_json::from_str(FIXTURE)?;
    for row in all.iter().filter(|row| {
        row["compat"]
            .as_object()
            .is_some_and(|fields| fields.values().any(|flag| flag == &json!(true)))
    }) {
        for value in [json!(1), json!("enabled"), json!({}), json!([])] {
            let mut changed = row.clone();
            for flag in changed["compat"]
                .as_object_mut()
                .ok_or("compat")?
                .values_mut()
                .filter(|flag| **flag == json!(true))
            {
                *flag = value.clone();
            }
            let target = model(&changed["model"])?;
            let mut target = target;
            target.compat = Some(ModelCompat(serde_json::from_value(
                changed["compat"].clone(),
            )?));
            let compat = ResolvedOpenAICompletionsCompat::from(&target);
            let actual = convert_messages(&target, &context(&changed["context"])?, &compat)?;
            assert_eq!(serde_json::to_value(actual)?, row["expected"]);
        }
    }
    Ok(())
}

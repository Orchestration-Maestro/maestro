//! Request construction for direct chat-completion invocations.

#[path = "support/cases.rs"]
mod cases;
#[path = "support/chat.rs"]
mod chat;
#[path = "support/child_process.rs"]
mod child_process;
#[allow(
    dead_code,
    reason = "Each test binary uses part of the loopback server."
)]
#[path = "support/loopback.rs"]
mod loopback;
#[allow(
    dead_code,
    reason = "Each test binary uses part of the scripted transport."
)]
#[path = "support/transport.rs"]
mod transport;

use cases::{assert_rows, canonical, run_case};
use chat::{TestResult, context, model, rows};
use child_process::{child_case, rerun};
use maestro_models::providers::chat::openai_completions::ResolvedOpenAICompletionsCompat;
use maestro_models::providers::chat::openai_completions::messages::convert_messages;
use maestro_models::{ModelCompat, OpenAICompletionsCompat};
use serde_json::{Value, json};

const FIXTURE: &str = include_str!("fixtures/chat_completions/requests.json");

/// Convert a fixture row's history under the compatibility it names.
fn converted(row: &Value) -> TestResult<Value> {
    let mut model = model(&row["model"])?;
    if let Some(overrides) = row.get("compat") {
        let compat: OpenAICompletionsCompat = serde_json::from_value(overrides.clone())?;
        model.compat = Some(ModelCompat::OpenAICompletions(Box::new(compat)));
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

#[test]
fn maestro_chat_replays_thinking_as_text() -> TestResult {
    chat::block_on(false, async {
        assert_history_rows("maestro_chat_replays_thinking_as_text")?;
        if child_case().is_none() {
            return rerun("maestro_chat_replays_thinking_as_text", "*", &[]);
        }
        let server = loopback::serve(
            loopback::EVENT_STREAM_HEAD,
            vec![b"data: [DONE]\n\n".to_vec()],
            std::time::Duration::ZERO,
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
    Ok(())
}

/// Request body a stream case sent.
async fn sent_body(case: &Value) -> TestResult<Value> {
    let observed = run_case(case).await?;
    Ok(observed.requests.into_iter().next().ok_or("no request")?["body"].clone())
}

/// Check one fixture row's request body against the payload it names.
async fn payload_matches(row: &Value) -> TestResult {
    let case = json!({"model": row["model"], "context": row["context"], "options": row["options"]});
    assert_eq!(
        canonical(sent_body(&case).await?),
        canonical(row["expected"].clone()),
        "{}",
        row["id"]
    );
    Ok(())
}

/// Check payload rows in fresh processes so ambient environment cannot reach them.
///
/// Rows without environment run together in one child; each row with environment
/// runs alone in a child holding exactly those variables.
async fn assert_payload_rows(test: &str) -> TestResult {
    let owned = rows(FIXTURE, test)?;
    assert!(!owned.is_empty(), "{test} has fixture rows");
    if let Some(case) = child_case() {
        for row in owned
            .iter()
            .filter(|row| case == "*" && row.get("env").is_none() || row["id"] == case.as_str())
        {
            payload_matches(row).await?;
        }
        return Ok(());
    }
    rerun(test, "*", &[])?;
    for row in owned.iter().filter(|row| row.get("env").is_some()) {
        let variables: Vec<(&str, &str)> = row["env"]
            .as_object()
            .into_iter()
            .flatten()
            .filter_map(|(name, value)| Some((name.as_str(), value.as_str()?)))
            .collect();
        rerun(test, row["id"].as_str().ok_or("row id")?, &variables)?;
    }
    Ok(())
}

#[test]
fn maestro_chat_preserves_numeric_options() -> TestResult {
    chat::block_on(false, async {
        assert_payload_rows("maestro_chat_preserves_numeric_options").await
    })
}

#[test]
fn maestro_chat_selects_reasoning_payloads() -> TestResult {
    chat::block_on(false, async {
        assert_payload_rows("maestro_chat_selects_reasoning_payloads").await
    })
}

#[test]
fn maestro_chat_preserves_routing_fields() -> TestResult {
    chat::block_on(false, async {
        assert_payload_rows("maestro_chat_preserves_routing_fields").await
    })
}

#[test]
fn maestro_chat_places_cache_markers() -> TestResult {
    chat::block_on(false, async {
        assert_payload_rows("maestro_chat_places_cache_markers").await
    })
}

#[test]
fn maestro_chat_preserves_prompt_cache_policy() -> TestResult {
    chat::block_on(false, async {
        assert_payload_rows("maestro_chat_preserves_prompt_cache_policy").await
    })
}

#[test]
fn maestro_chat_preserves_tool_choice() -> TestResult {
    chat::block_on(false, async {
        assert_payload_rows("maestro_chat_preserves_tool_choice").await
    })
}

#[test]
fn maestro_chat_preserves_tool_declarations() -> TestResult {
    chat::block_on(false, async {
        assert_payload_rows("maestro_chat_preserves_tool_declarations").await
    })
}

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
        "deepseek"
    } else if body.get("reasoning").is_some() {
        "openrouter"
    } else if body.get("enable_thinking").is_some() {
        "zai"
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

#[test]
fn maestro_chat_derives_compatibility() -> TestResult {
    chat::block_on(false, async {
        assert_compat_rows("maestro_chat_derives_compatibility").await
    })
}

#[test]
fn maestro_chat_applies_compatibility_overrides() -> TestResult {
    chat::block_on(false, async {
        assert_compat_rows("maestro_chat_applies_compatibility_overrides").await
    })
}

#[test]
fn maestro_chat_preserves_header_precedence() -> TestResult {
    chat::block_on(false, async {
        assert_rows(FIXTURE, "maestro_chat_preserves_header_precedence").await
    })
}

#[test]
fn maestro_chat_preserves_gateway_authorization() -> TestResult {
    chat::block_on(false, async {
        assert_rows(FIXTURE, "maestro_chat_preserves_gateway_authorization").await
    })
}

#[test]
fn maestro_chat_observes_callback_boundaries() -> TestResult {
    chat::block_on(true, async {
        use maestro_models::ProviderResponse;
        use std::sync::{Arc, Mutex};

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
        let log: Arc<Mutex<Vec<String>>> = Arc::default();
        let payloads: Arc<Mutex<Vec<Value>>> = Arc::default();
        let responses: Arc<Mutex<Vec<ProviderResponse>>> = Arc::default();
        let (seen, kept) = (Arc::clone(&log), Arc::clone(&payloads));
        common.on_payload = Some(Arc::new(move |payload, model| {
            seen.lock()
                .map(|mut log| log.push(format!("payload for {}", model.id)))
                .ok();
            kept.lock().map(|mut kept| kept.push(payload)).ok();
            Box::pin(std::future::ready(Ok(None)))
        }));
        let (seen, kept) = (Arc::clone(&log), Arc::clone(&responses));
        common.on_response = Some(Arc::new(move |response, model| {
            seen.lock()
                .map(|mut log| log.push(format!("response for {}", model.id)))
                .ok();
            kept.lock().map(|mut kept| kept.push(response)).ok();
            Box::pin(std::future::ready(Ok(())))
        }));
        transport::finish(model, history, common).await?;
        assert_eq!(target.attempts(), 2, "the first response was retried");
        assert_eq!(
            *log.lock().map_err(|e| e.to_string())?,
            ["payload for model", "response for model"]
        );
        let responses = responses.lock().map_err(|e| e.to_string())?;
        assert_eq!(responses.len(), 1, "retry intermediates are not reported");
        assert_eq!(responses[0].status.to_bits(), 200.0_f64.to_bits());
        assert_eq!(
            responses[0].headers.get("x-result").map(String::as_str),
            Some("fixture")
        );
        let sent = target.requests.lock().map_err(|e| e.to_string())?;
        let body: Value = serde_json::from_slice(&sent[0].body)?;
        assert_eq!(
            payloads.lock().map_err(|e| e.to_string())?[0],
            body,
            "the hook sees the sent payload"
        );
        Ok(())
    })
}

#[test]
fn maestro_chat_preserves_simple_options() -> TestResult {
    chat::block_on(false, async {
        assert_rows(FIXTURE, "maestro_chat_preserves_simple_options").await
    })
}

#[test]
fn maestro_chat_preserves_auth_boundaries() -> TestResult {
    chat::block_on(false, async {
        assert_rows(FIXTURE, "maestro_chat_preserves_auth_boundaries").await
    })
}

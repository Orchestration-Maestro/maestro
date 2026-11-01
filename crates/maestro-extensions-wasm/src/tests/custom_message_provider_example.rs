//! Controlled authorization and streaming for the shipped message provider.
#[path = "../../../../examples/extensions/custom_message_provider/oauth.rs"]
mod oauth;

use super::{bus_callbacks::captured_api, controlled::Controlled};
use crate::{ExtensionResult, OAuthAuthInfo, OAuthLoginCallbacks, OAuthProviderInterface};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// Supplies an authorization code bound to the published proof key.
fn callbacks() -> Rc<OAuthLoginCallbacks> {
    let published = Rc::new(RefCell::new(String::new()));
    let url = published.clone();
    Rc::new(OAuthLoginCallbacks {
        on_auth: Rc::new(move |info: OAuthAuthInfo| {
            assert!(info.url.starts_with("https://claude.ai/oauth/authorize?"));
            assert!(info.url.contains("code_challenge_method=S256"));
            assert!(
                info.url
                    .contains("code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM")
            );
            assert!(info.instructions.is_none());
            *url.borrow_mut() = info.url;
            Ok(())
        }),
        on_prompt: Rc::new(move |prompt| {
            assert_eq!(prompt.message, "Paste the authorization code:");
            let url = published.borrow();
            let state = url
                .split("state=")
                .nth(1)
                .unwrap()
                .split('&')
                .next()
                .unwrap();
            let code = format!("controlled-code#{state}");
            Box::pin(async move { Ok(code) })
        }),
        on_progress: None,
        on_manual_code_input: None,
        on_select: None,
        signal: None,
    })
}

/// Supplies the host proof key, with no ambient entropy or clock reads.
fn authorization(host: &Controlled) -> Rc<oauth::CustomAnthropic> {
    let api = captured_api(host).unwrap();
    host.observe().actions.value = json!({"oauth.generate_pkce": {
        "verifier": "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
        "challenge": "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    }});
    Rc::new(oauth::CustomAnthropic {
        io: api.native_io(),
        api,
        now: Rc::new(|| 10_000.0),
    })
}

/// Installs a controlled token response without changing the host's HTTP implementation.
fn tokens(host: &Controlled, access: &str, refresh: &str, lifetime: u32) {
    host.observe().native_http.text = json!({
        "access_token": access, "refresh_token": refresh, "expires_in": lifetime
    })
    .to_string();
}

#[test]
fn custom_message_provider_authorizes_and_refreshes_through_host_http() -> ExtensionResult<()> {
    let host = Controlled::default();
    let provider = authorization(&host);
    tokens(&host, "access-first", "refresh-first", 3600);
    let credentials = super::block_on(provider.login(callbacks()))?;
    assert_eq!(credentials.access, "access-first");
    assert_eq!(credentials.refresh, "refresh-first");
    assert_eq!(json!(credentials.expires), json!(3_310_000.0));
    assert_eq!(provider.get_api_key(&credentials)?, "access-first");
    assert_eq!(provider.id(), "custom-anthropic");
    assert_eq!(provider.name(), "Custom Anthropic (Claude Pro/Max)");
    assert_eq!(provider.uses_callback_server(), None);
    assert!(provider.modify_models(&[], &credentials)?.is_none());
    tokens(&host, "access-next", "refresh-next", 7200);
    let refreshed = super::block_on(provider.refresh_token(credentials))?;
    assert_eq!(refreshed.access, "access-next");
    assert_eq!(refreshed.refresh, "refresh-next");
    assert_eq!(json!(refreshed.expires), json!(6_910_000.0));
    check_token_requests(&host);
    Ok(())
}

/// Checks both token exchanges at the host HTTP interface.
fn check_token_requests(host: &Controlled) {
    let observed = host.observed();
    assert_eq!(observed.native_http.requests.len(), 2);
    let requests = &observed.native_http.requests;
    for request in requests {
        assert_eq!(request.url, "https://console.anthropic.com/v1/oauth/token");
        assert_eq!(request.method.as_deref(), Some("POST"));
        assert_eq!(
            request.headers,
            [("Content-Type".into(), "application/json".into())]
        );
    }
    let login: Value = serde_json::from_slice(requests[0].body.as_ref().unwrap()).unwrap();
    assert_eq!(
        login,
        json!({
            "grant_type":"authorization_code", "client_id":"9d1c250a-e61b-44d9-88ed-5944d1962f5e",
            "code":"controlled-code", "state":"dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
            "redirect_uri":"https://console.anthropic.com/oauth/code/callback",
            "code_verifier":"dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"
        })
    );
    let refresh: Value = serde_json::from_slice(requests[1].body.as_ref().unwrap()).unwrap();
    assert_eq!(
        refresh,
        json!({"grant_type":"refresh_token", "client_id":"9d1c250a-e61b-44d9-88ed-5944d1962f5e", "refresh_token":"refresh-first"})
    );
}

#[test]
fn custom_message_provider_rejects_mismatched_authorization_state() {
    let host = Controlled::default();
    let provider = authorization(&host);
    tokens(&host, "must-not-use", "must-not-use", 3600);
    let mut interaction = callbacks();
    Rc::get_mut(&mut interaction).unwrap().on_prompt =
        Rc::new(|_| Box::pin(async { Ok("controlled-code#unrelated-state".into()) }));
    let result = super::block_on(provider.login(interaction));
    assert_eq!(result.unwrap_err(), "Authorization state mismatch");
    assert!(host.observed().native_http.requests.is_empty());
}

#[test]
fn custom_message_provider_keeps_host_pkce_failure_before_prompting() {
    let host = Controlled::default();
    let provider = authorization(&host);
    host.observe().actions.error = Some("secure entropy unavailable".into());
    let result = super::block_on(provider.login(callbacks()));
    assert_eq!(result.unwrap_err(), "secure entropy unavailable");
    assert!(host.observed().native_http.requests.is_empty());
}

#[test]
fn custom_message_provider_token_failure_keeps_status_when_body_fails() {
    let host = Controlled::default();
    let provider = authorization(&host);
    host.observe().native_http.status = 429;
    host.observe().native_http.read_error = Some("body interrupted".into());
    let result = super::block_on(provider.login(callbacks()));
    assert_eq!(
        result.unwrap_err(),
        "Token exchange failed: HTTP 429 body interrupted"
    );
}

#[path = "../../../../examples/extensions/custom_message_provider/stream.rs"]
mod stream;

/// The native owner supplies the buffer; this adapter only projects its public interface.
struct NativeStream(maestro_models::AssistantMessageEventStream);
impl crate::EventStreamPort<crate::AssistantMessageEvent, crate::SharedAssistantMessage>
    for NativeStream
{
    fn push(&self, event: crate::AssistantMessageEvent) -> ExtensionResult<()> {
        self.0.push(event);
        Ok(())
    }
    fn end(&self, result: Option<crate::SharedAssistantMessage>) -> ExtensionResult<()> {
        self.0.end(result);
        Ok(())
    }
    fn next(&self) -> crate::ExtensionFuture<'static, Option<Rc<crate::AssistantMessageEvent>>> {
        let next = self.0.next();
        Box::pin(async move { Ok(next.await.map(Rc::new)) })
    }
    fn result(&self) -> crate::ExtensionFuture<'static, Rc<crate::SharedAssistantMessage>> {
        let result = self.0.result();
        Box::pin(async move { Ok(Rc::new(result.await)) })
    }
}

/// A supplied model, independent of the ambient catalog.
fn model() -> crate::Model {
    serde_json::from_value(json!({
        "id":"claude-sonnet-4-5", "name":"Claude Sonnet 4.5 (Custom)",
        "api":"custom-anthropic-api", "provider":"custom-anthropic",
        "baseUrl":"https://api.anthropic.com", "reasoning":true, "input":["text","image"],
        "cost":{"input":3,"output":15,"cacheRead":0.3,"cacheWrite":3.75},
        "contextWindow":200_000,"maxTokens":64_000
    }))
    .unwrap()
}

/// Mixed conversation records observed at the provider request boundary.
fn conversation() -> crate::Context {
    serde_json::from_value(json!({
        "systemPrompt":"Prompt 雪🚀", "tools":[{
            "name":"write", "description":"Write a file",
            "parameters":{"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}
        }],
        "messages":[
            {"role":"user","content":"  ","timestamp":0},
            {"role":"user","content":[{"type":"text","text":"Question 雪🚀"},{"type":"image","mimeType":"image/png","data":"image-data"}],"timestamp":0},
            {"role":"assistant","api":"custom-anthropic-api","provider":"custom-anthropic","model":"claude-sonnet-4-5",
             "content":[{"type":"text","text":" "},{"type":"thinking","thinking":"unsigned"},{"type":"thinking","thinking":"signed","thinkingSignature":"prior-signature"},{"type":"thinking","thinking":"empty-signed","thinkingSignature":""},
                {"type":"toolCall","id":"call-first","name":"write","arguments":{"path":"demo"}}],
             "usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},"stopReason":"toolUse","timestamp":0},
            {"role":"toolResult","toolCallId":"call-first","toolName":"write","content":[{"type":"text","text":"alpha"},{"type":"text","text":"beta"}],"isError":false,"timestamp":0},
            {"role":"toolResult","toolCallId":"call-second","toolName":"write","content":[{"type":"image","mimeType":"image/png","data":"result-image"}],"isError":true,"timestamp":0}
        ]
    })).unwrap()
}

/// Provider events contain sparse wire indices and usage updates that omit prior counts.
fn response_events() -> String {
    let events = [
        json!({"type":"message_start","message":{"usage":{"input_tokens":100,"output_tokens":0,"cache_read_input_tokens":10,"cache_creation_input_tokens":4}}}),
        json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}),
        json!({"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"reason 雪"}}),
        json!({"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"signature"}}),
        json!({"type":"content_block_stop","index":0}),
        json!({"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}),
        json!({"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"answer 🚀"}}),
        json!({"type":"content_block_stop","index":1}),
        json!({"type":"content_block_start","index":3,"content_block":{"type":"tool_use","id":"call-response","name":"Write","input":{}}}),
        json!({"type":"content_block_delta","index":3,"delta":{"type":"input_json_delta","partial_json":"{\"path\":"}}),
        json!({"type":"content_block_delta","index":3,"delta":{"type":"input_json_delta","partial_json":"\"demo\"}"}}),
        json!({"type":"content_block_stop","index":3}),
        json!({"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":20}}),
        json!({"type":"message_stop"}),
    ];
    let mut wire = String::from("\u{feff}: heartbeat\r\n\r\n");
    for event in events {
        wire.push_str("data: ");
        wire.push_str(&event.to_string());
        wire.push_str("\r\n\r\n");
    }
    wire
}

#[test]
fn custom_message_provider_converts_and_streams_through_public_host_interfaces()
-> ExtensionResult<()> {
    let host = Controlled::default();
    host.observe().native_http.body =
        Some(response_events().bytes().map(|byte| vec![byte]).collect());
    let provider = authorization(&host);
    let output = crate::AssistantMessageEventStream::from_port(Rc::new(NativeStream(
        maestro_models::AssistantMessageEventStream::new(),
    )));
    let options = crate::SimpleStreamOptions {
        common: crate::StreamOptions {
            api_key: Some("sk-ant-oat-fixture".into()),
            max_tokens: Some(1234.0),
            ..crate::StreamOptions::default()
        },
        reasoning: Some(crate::RequestedThinkingLevel::High),
        ..crate::SimpleStreamOptions::default()
    };
    super::block_on(provider.stream_custom_anthropic(
        model(),
        conversation(),
        Some(options.into()),
        output.clone(),
    ))?;
    let result = super::block_on(output.result())?;
    check_stream_result(&result);
    check_stream_events(&output, &result)?;
    check_message_request(&host);
    Ok(())
}

/// Final content, complete usage and pricing come from the public result owner.
fn check_stream_result(result: &crate::SharedAssistantMessage) {
    let value = serde_json::to_value(&*result.read().unwrap()).unwrap();
    assert_eq!(
        value["content"],
        json!([
            {"type":"thinking","thinking":"reason 雪","thinkingSignature":"signature"},
            {"type":"text","text":"answer 🚀"},
            {"type":"toolCall","id":"call-response","name":"write","arguments":{"path":"demo"}}
        ])
    );
    assert_eq!(value["stopReason"], "toolUse");
    assert_eq!(value["timestamp"], json!(10000.0));
    assert_eq!(value["usage"]["input"], json!(100.0));
    assert_eq!(value["usage"]["output"], json!(20.0));
    assert_eq!(value["usage"]["cacheRead"], json!(10.0));
    assert_eq!(value["usage"]["cacheWrite"], json!(4.0));
    assert_eq!(value["usage"]["totalTokens"], json!(134.0));
    assert_eq!(
        value["usage"]["cost"]["input"].as_f64().unwrap().to_bits(),
        (3.0_f64 / 1_000_000.0 * 100.0).to_bits()
    );
    assert!((value["usage"]["cost"]["total"].as_f64().unwrap() - 0.000_618).abs() < f64::EPSILON);
}

/// Every event keeps the original shared message; deltas retain their authored order.
fn check_stream_events(
    output: &crate::AssistantMessageEventStream,
    result: &crate::SharedAssistantMessage,
) -> ExtensionResult<()> {
    let mut events = Vec::new();
    while let Some(event) = super::block_on(output.next())? {
        assert!(std::sync::Arc::ptr_eq(event.message(), result));
        events.push(serde_json::to_value(&*event).unwrap());
    }
    assert_eq!(
        events
            .iter()
            .map(|event| event["type"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "start",
            "thinking_start",
            "thinking_delta",
            "thinking_end",
            "text_start",
            "text_delta",
            "text_end",
            "toolcall_start",
            "toolcall_delta",
            "toolcall_delta",
            "toolcall_end",
            "done"
        ]
    );
    assert_eq!(events[8]["contentIndex"], 2);
    assert_eq!(events[8]["delta"], "{\"path\":");
    assert_eq!(events[9]["delta"], "\"demo\"}");
    Ok(())
}

/// Request conversion preserves images, adjacent results, schema constraints and subscription headers.
fn check_message_request(host: &Controlled) {
    let observed = host.observed();
    assert_eq!(observed.native_http.requests.len(), 1);
    let request = &observed.native_http.requests[0];
    assert_eq!(request.url, "https://api.anthropic.com/v1/messages");
    assert_eq!(request.method.as_deref(), Some("POST"));
    assert!(
        request
            .headers
            .contains(&("authorization".into(), "Bearer sk-ant-oat-fixture".into()))
    );
    assert!(request.headers.contains(&(
        "user-agent".into(),
        "claude-cli/2.1.2 (external, cli)".into()
    )));
    let body: Value = serde_json::from_slice(request.body.as_ref().unwrap()).unwrap();
    assert_eq!(body["model"], "claude-sonnet-4-5");
    assert_eq!(body["max_tokens"], json!(1234.0));
    assert_eq!(body["stream"], true);
    assert_eq!(
        body["thinking"],
        json!({"type":"enabled","budget_tokens":20480.0})
    );
    assert_eq!(
        body["tools"],
        json!([{"name":"Write","description":"Write a file","input_schema":{"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}}])
    );
    assert_eq!(
        body["messages"],
        json!([
            {"role":"user","content":[{"type":"text","text":"Question 雪🚀"},{"type":"image","source":{"type":"base64","media_type":"image/png","data":"image-data"}}]},
            {"role":"assistant","content":[{"type":"text","text":"unsigned"},{"type":"thinking","thinking":"signed","signature":"prior-signature"},{"type":"text","text":"empty-signed"},{"type":"tool_use","id":"call-first","name":"Write","input":{"path":"demo"}}]},
            {"role":"user","content":[{"type":"tool_result","tool_use_id":"call-first","content":"alpha\nbeta","is_error":false},{"type":"tool_result","tool_use_id":"call-second","content":[{"type":"text","text":"(see attached image)"},{"type":"image","source":{"type":"base64","media_type":"image/png","data":"result-image"}}],"is_error":true,"cache_control":{"type":"ephemeral"}}]}
        ])
    );
    assert_eq!(
        body["system"][1],
        json!({"type":"text","text":"Prompt 雪🚀","cache_control":{"type":"ephemeral"}})
    );
}

/// Observes a complete request through the same stream producer and host transport.
fn request_with_options(options: crate::StreamOptionsInput) -> ExtensionResult<Value> {
    let host = Controlled::default();
    host.observe().native_http.body = Some(vec![response_events().into_bytes()]);
    let provider = authorization(&host);
    let output = crate::AssistantMessageEventStream::from_port(Rc::new(NativeStream(
        maestro_models::AssistantMessageEventStream::new(),
    )));
    super::block_on(provider.stream_custom_anthropic(
        model(),
        conversation(),
        Some(options),
        output.clone(),
    ))?;
    let result = super::block_on(output.result())?;
    assert_eq!(
        result.read().unwrap().stop_reason,
        crate::StopReason::ToolUse
    );
    let observed = host.observed();
    assert_eq!(observed.native_http.requests.len(), 1);
    Ok(serde_json::from_slice(observed.native_http.requests[0].body.as_ref().unwrap()).unwrap())
}

#[test]
fn custom_message_provider_raw_reasoning_matches_simple_options() -> ExtensionResult<()> {
    let common = crate::StreamOptions {
        api_key: Some("sk-ant-oat-fixture".into()),
        max_tokens: Some(1234.0),
        ..crate::StreamOptions::default()
    };
    let raw = crate::ProviderStreamOptions {
        common: common.clone(),
        extra: [
            ("reasoning".into(), json!("high")),
            ("thinkingBudgets".into(), json!({"high":7777})),
            ("unrelated".into(), json!("must not be forwarded")),
        ]
        .into_iter()
        .collect(),
        ..crate::ProviderStreamOptions::default()
    };
    let simple = crate::SimpleStreamOptions {
        common,
        reasoning: Some(crate::RequestedThinkingLevel::High),
        thinking_budgets: Some(crate::ThinkingBudgets {
            high: Some(7777.0),
            ..crate::ThinkingBudgets::default()
        }),
        ..crate::SimpleStreamOptions::default()
    };
    let raw = request_with_options(raw.into())?;
    assert_eq!(
        raw["thinking"],
        json!({"type":"enabled","budget_tokens":7777.0})
    );
    assert_eq!(raw, request_with_options(simple.into())?);
    Ok(())
}

#[test]
fn custom_message_provider_zero_limit_uses_native_default() -> ExtensionResult<()> {
    for limit in [0.0, -0.0] {
        let options = crate::ProviderStreamOptions {
            common: crate::StreamOptions {
                api_key: Some("sk-ant-oat-fixture".into()),
                max_tokens: Some(limit),
                ..crate::StreamOptions::default()
            },
            ..crate::ProviderStreamOptions::default()
        };
        let request = request_with_options(options.into())?;
        assert_eq!(request["max_tokens"], json!(21_333.0));
    }
    Ok(())
}

#[test]
fn custom_message_provider_preabort_settles_without_http() -> ExtensionResult<()> {
    let host = Controlled::default();
    let flag = super::controlled::Flag::default();
    let signal = crate::component_adapter::signal(&host, host.signal(&flag));
    flag.abort();
    host.observe().native_http.body = Some(vec![response_events().into_bytes()]);
    let provider = authorization(&host);
    let output = crate::AssistantMessageEventStream::from_port(Rc::new(NativeStream(
        maestro_models::AssistantMessageEventStream::new(),
    )));
    let options = crate::SimpleStreamOptions {
        common: crate::StreamOptions {
            api_key: Some("sk-ant-oat-fixture".into()),
            signal: Some(signal),
            ..crate::StreamOptions::default()
        },
        ..crate::SimpleStreamOptions::default()
    };
    super::block_on(provider.stream_custom_anthropic(
        model(),
        conversation(),
        Some(options.into()),
        output.clone(),
    ))?;
    let result = super::block_on(output.result())?;
    assert_eq!(
        result.read().unwrap().stop_reason,
        crate::StopReason::Aborted
    );
    assert!(host.observed().native_http.requests.is_empty());
    let start = super::block_on(output.next())?.unwrap();
    let error = super::block_on(output.next())?.unwrap();
    assert!(matches!(
        start.as_ref(),
        crate::AssistantMessageEvent::Start { .. }
    ));
    assert!(matches!(
        error.as_ref(),
        crate::AssistantMessageEvent::Error {
            reason: crate::ErrorReason::Aborted,
            ..
        }
    ));
    assert!(std::sync::Arc::ptr_eq(error.message(), &result));
    assert!(super::block_on(output.next())?.is_none());
    Ok(())
}

#[test]
fn custom_message_provider_requires_credentials_before_http() -> ExtensionResult<()> {
    for key in [None, Some(String::new())] {
        let host = Controlled::default();
        host.observe().native_http.body = Some(vec![response_events().into_bytes()]);
        let provider = authorization(&host);
        let output = crate::AssistantMessageEventStream::from_port(Rc::new(NativeStream(
            maestro_models::AssistantMessageEventStream::new(),
        )));
        let options = crate::SimpleStreamOptions {
            common: crate::StreamOptions {
                api_key: key,
                ..crate::StreamOptions::default()
            },
            ..crate::SimpleStreamOptions::default()
        };
        super::block_on(provider.stream_custom_anthropic(
            model(),
            conversation(),
            Some(options.into()),
            output.clone(),
        ))?;
        let result = super::block_on(output.result())?;
        let value = serde_json::to_value(&*result.read().unwrap()).unwrap();
        assert_eq!(value["stopReason"], "error");
        assert!(
            value["errorMessage"]
                .as_str()
                .unwrap()
                .contains("authentication")
        );
        assert!(host.observed().native_http.requests.is_empty());
    }
    Ok(())
}

#[test]
fn custom_message_provider_stream_failure_keeps_status_when_body_fails() -> ExtensionResult<()> {
    let host = Controlled::default();
    host.observe().native_http.status = 503;
    host.observe().native_http.read_error = Some("body interrupted".into());
    let provider = authorization(&host);
    let output = crate::AssistantMessageEventStream::from_port(Rc::new(NativeStream(
        maestro_models::AssistantMessageEventStream::new(),
    )));
    let options = crate::SimpleStreamOptions {
        common: crate::StreamOptions {
            api_key: Some("sk-ant-oat-fixture".into()),
            ..crate::StreamOptions::default()
        },
        ..crate::SimpleStreamOptions::default()
    };
    super::block_on(provider.stream_custom_anthropic(
        model(),
        conversation(),
        Some(options.into()),
        output.clone(),
    ))?;
    let result = super::block_on(output.result())?;
    let value = serde_json::to_value(&*result.read().unwrap()).unwrap();
    assert_eq!(value["stopReason"], "error");
    assert_eq!(value["errorMessage"], "HTTP 503 body interrupted");
    Ok(())
}

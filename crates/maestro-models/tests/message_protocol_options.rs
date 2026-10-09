//! Authorization and thinking through the public message-protocol entries.
#[allow(dead_code, reason = "Each test binary uses part of the chat fixtures.")]
#[path = "support/chat.rs"]
mod chat;
#[path = "support/child_process.rs"]
mod child_process;
#[path = "support/json.rs"]
mod json;
#[path = "support/message_options.rs"]
mod message_options;
#[allow(
    dead_code,
    reason = "Each test binary uses part of the message fixtures."
)]
#[path = "support/messages.rs"]
mod messages;

#[test]
fn messages_select_subscription_auth() -> chat::TestResult {
    chat::block_on(
        false,
        message_options::assert_rows("messages_select_subscription_auth"),
    )
}

#[test]
fn messages_select_account_auth() -> chat::TestResult {
    chat::block_on(
        false,
        message_options::assert_rows("messages_select_account_auth"),
    )
}

#[test]
fn messages_select_gateway_auth() -> chat::TestResult {
    chat::block_on(
        false,
        message_options::assert_rows("messages_select_gateway_auth"),
    )
}

#[test]
fn messages_preserve_auth_header_precedence() -> chat::TestResult {
    chat::block_on(
        false,
        message_options::assert_rows("messages_preserve_auth_header_precedence"),
    )
}

#[test]
fn messages_freeze_auth_before_payload_hook() -> chat::TestResult {
    chat::block_on(
        false,
        message_options::assert_rows("messages_freeze_auth_before_payload_hook"),
    )
}

#[test]
fn messages_normalize_subscription_tool_names() -> chat::TestResult {
    chat::block_on(
        false,
        message_options::assert_rows("messages_normalize_subscription_tool_names"),
    )
}

#[test]
fn messages_restore_subscription_tool_names() -> chat::TestResult {
    chat::block_on(false, async {
        message_options::assert_rows("messages_restore_subscription_tool_names").await?;
        if child_process::child_case().is_none() {
            subscription_name_is_visible_at_open().await?;
        }
        Ok(())
    })
}

#[test]
fn messages_align_subscription_named_tool_choice() -> chat::TestResult {
    chat::block_on(
        false,
        message_options::assert_rows("messages_align_subscription_named_tool_choice"),
    )
}

#[test]
fn messages_place_subscription_identity() -> chat::TestResult {
    chat::block_on(
        false,
        message_options::assert_rows("messages_place_subscription_identity"),
    )
}

#[test]
fn messages_select_thinking_policy() -> chat::TestResult {
    chat::block_on(
        false,
        message_options::assert_rows("messages_select_thinking_policy"),
    )
}

#[test]
fn messages_bypass_auth_for_injected_client() -> chat::TestResult {
    chat::block_on(
        false,
        message_options::assert_rows("messages_bypass_auth_for_injected_client"),
    )
}

#[test]
fn messages_select_thinking_display() -> chat::TestResult {
    chat::block_on(
        false,
        message_options::assert_rows("messages_select_thinking_display"),
    )
}

#[test]
fn messages_select_raw_effort() -> chat::TestResult {
    chat::block_on(
        false,
        message_options::assert_rows("messages_select_raw_effort"),
    )
}

#[test]
fn messages_map_simple_effort() -> chat::TestResult {
    chat::block_on(
        false,
        message_options::assert_rows("messages_map_simple_effort"),
    )
}

#[test]
fn messages_keep_raw_and_simple_budgets_distinct() -> chat::TestResult {
    chat::block_on(
        false,
        message_options::assert_rows("messages_keep_raw_and_simple_budgets_distinct"),
    )
}

#[test]
fn messages_keep_adjusted_zero_budget() -> chat::TestResult {
    chat::block_on(
        false,
        message_options::assert_rows("messages_keep_adjusted_zero_budget"),
    )
}

#[test]
fn messages_disable_unrequested_thinking() -> chat::TestResult {
    chat::block_on(
        false,
        message_options::assert_rows("messages_disable_unrequested_thinking"),
    )
}

#[test]
fn messages_fail_simple_auth_before_stream() -> chat::TestResult {
    chat::block_on(
        false,
        message_options::assert_rows("messages_fail_simple_auth_before_stream"),
    )
}

#[test]
fn messages_forward_simple_callbacks() -> chat::TestResult {
    chat::block_on(
        false,
        message_options::assert_rows("messages_forward_simple_callbacks"),
    )
}

#[test]
fn messages_preserve_callback_failures() -> chat::TestResult {
    chat::block_on(
        false,
        message_options::assert_rows("messages_preserve_callback_failures"),
    )
}

/// A paused producer proves restored spelling before argument updates can happen.
async fn subscription_name_is_visible_at_open() -> chat::TestResult {
    use maestro_models::{
        AnthropicOptions, AssistantMessageEvent, HttpResponse, StreamOptions, stream_anthropic,
    };
    use serde_json::json;
    use std::sync::{Arc, Mutex};
    let (feed, body) = messages::feed();
    let body = Arc::new(Mutex::new(Some(body)));
    let mut options = AnthropicOptions {
        common: StreamOptions {
            api_key: Some("sk-ant-oat-open".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    options.common.fetch = Some(Arc::new(move |_| {
        let taken = body.lock().ok().and_then(|mut body| body.take());
        Box::pin(async move {
            let body = taken.ok_or_else(|| {
                maestro_models::FetchError::Connection(messages::diagnostic("body already taken"))
            })?;
            Ok(HttpResponse {
                status: 200,
                status_text: String::new(),
                headers: std::collections::BTreeMap::default(),
                body,
            })
        })
    }));
    let context = messages::context(
        &json!({"messages":[],"tools":[{"name":"bAsH","description":"shell","parameters":{"type":"object"}}]}),
    )?;
    let stream = stream_anthropic(messages::model(&json!({}))?, context, Some(options));
    assert!(matches!(
        stream.next().await,
        Some(AssistantMessageEvent::Start { .. })
    ));
    feed.chunks.send(messages::sse(&json!({"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"live","name":"BASH","input":{}}})).into_bytes())?;
    let Some(AssistantMessageEvent::ToolcallStart { partial, .. }) = stream.next().await else {
        return Err("tool call did not open".into());
    };
    let opening = partial.read().map_err(|error| error.to_string())?.clone();
    let opening = serde_json::to_value(opening)?;
    assert_eq!(opening["content"][0]["name"], "bAsH");
    feed.chunks.send(messages::STOP.as_bytes().to_vec())?;
    messages::collect(&stream).await?;
    feed.released.await?;
    Ok(())
}

/// Complete set of fixture owners; unknown rows cannot be silently filtered out.
const TEST_NAMES: [&str; 20] = [
    "messages_select_subscription_auth",
    "messages_select_account_auth",
    "messages_select_gateway_auth",
    "messages_preserve_auth_header_precedence",
    "messages_freeze_auth_before_payload_hook",
    "messages_normalize_subscription_tool_names",
    "messages_restore_subscription_tool_names",
    "messages_align_subscription_named_tool_choice",
    "messages_place_subscription_identity",
    "messages_select_thinking_policy",
    "messages_bypass_auth_for_injected_client",
    "messages_select_thinking_display",
    "messages_select_raw_effort",
    "messages_map_simple_effort",
    "messages_keep_raw_and_simple_budgets_distinct",
    "messages_keep_adjusted_zero_budget",
    "messages_disable_unrequested_thinking",
    "messages_fail_simple_auth_before_stream",
    "messages_forward_simple_callbacks",
    "messages_preserve_callback_failures",
];

/// Read a subscription call from a closed scripted response through the public stream.
async fn subscription_name_result(members: &str) -> chat::TestResult<messages::Run> {
    use serde_json::json;
    let opening = format!(
        "event: content_block_start\ndata: {{\"type\":\"content_block_start\",\"index\":0,\"content_block\":{{\"type\":\"tool_use\",\"id\":\"tool\",{members},\"input\":{{}}}}}}\n\n"
    );
    messages::run_case(&messages::Case {
        context: json!({"messages":[],"tools":[{"name":"�CuStOm","description":"replacement character","parameters":{"type":"object"}}]}),
        options: json!({"apiKey":"sk-ant-oat-name"}),
        chunks: Some(vec![messages::Chunk::Text(opening + messages::STOP)]),
        ..Default::default()
    }).await
}

#[test]
fn messages_report_native_subscription_name_error() -> chat::TestResult {
    chat::block_on(false, async {
        let run = subscription_name_result(r#""name":"\ud800""#).await?;
        assert!(
            run.result["errorMessage"]
                .as_str()
                .ok_or("missing error")?
                .contains("unexpected end of hex escape")
        );
        assert_eq!(
            run.events
                .iter()
                .map(|event| event["type"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["start", "error"]
        );
        Ok(())
    })
}

#[test]
fn messages_restore_replacement_character_subscription_name() -> chat::TestResult {
    chat::block_on(false, async {
        let run = subscription_name_result(r#""name":"\ufffdCUSTOM""#).await?;
        assert_eq!(run.result["content"][0]["name"], "�CuStOm");
        assert_eq!(
            run.events
                .iter()
                .map(|event| event["type"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["start", "toolcall_start", "done"]
        );
        Ok(())
    })
}

#[test]
fn messages_report_native_error_for_last_duplicate_subscription_name() -> chat::TestResult {
    chat::block_on(false, async {
        let run = subscription_name_result(r#""name":"Read","name":"\ud800""#).await?;
        assert!(
            run.result["errorMessage"]
                .as_str()
                .ok_or("missing error")?
                .contains("unexpected end of hex escape")
        );
        assert_eq!(
            run.events
                .iter()
                .map(|event| event["type"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["start", "error"]
        );
        Ok(())
    })
}

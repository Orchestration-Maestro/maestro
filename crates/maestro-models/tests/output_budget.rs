//! The output budget every simple entry point shares: the model cap, the room the context
//! leaves and the thinking budget that fits inside both.

#[cfg(not(target_arch = "wasm32"))]
use crate::{chat::TestResult, messages};
#[cfg(not(target_arch = "wasm32"))]
use maestro_models::{
    Context, SimpleStreamOptions, StreamOptions, ThinkingLevel, adjust_max_tokens_for_thinking,
    build_base_options, stream_simple_anthropic,
};
#[cfg(not(target_arch = "wasm32"))]
use serde_json::{Value, json};

#[cfg(not(target_arch = "wasm32"))]
/// A conversation whose system prompt and tool declarations are estimated at the given token
/// counts, at four characters per token.
fn context_with_tokens(system_tokens: usize, tool_tokens: usize) -> TestResult<Context> {
    let build = |description: &str| -> TestResult<Context> {
        Ok(serde_json::from_value(json!({
            "systemPrompt": "x".repeat(system_tokens * 4),
            "messages": [],
            "tools": [{"name": "t", "description": description, "parameters": {"type": "object"}}]
        }))?)
    };
    let bare = serde_json::to_string(&build("")?.tools)?.len();
    build(&"y".repeat(tool_tokens * 4 - bare))
}

#[cfg(not(target_arch = "wasm32"))]
/// A 64,000-token model with a 20,000-token window.
fn small_window_model() -> TestResult<maestro_models::Model> {
    messages::model(&json!({"maxTokens": 64_000, "contextWindow": 20_000}))
}

#[cfg(not(target_arch = "wasm32"))]
/// The request body a simple messages call sends for `context` under the given controls.
async fn sent_body(
    model: maestro_models::Model,
    context: Context,
    max_tokens: Option<f64>,
    reasoning: Option<ThinkingLevel>,
) -> TestResult<Value> {
    let requests = messages::Requests::default();
    let options = SimpleStreamOptions {
        common: StreamOptions {
            api_key: Some("fixture-key".into()),
            max_tokens,
            max_retries: Some(0.0),
            fetch: Some(messages::scripted_fetch(
                &messages::Case::default(),
                &requests,
            )),
            ..StreamOptions::default()
        },
        reasoning,
        ..SimpleStreamOptions::default()
    };
    let stream = stream_simple_anthropic(model, context, Some(options));
    messages::collect(&stream).await?;
    let sent = requests.lock().map_err(|error| error.to_string())?.clone();
    Ok(sent[0]["body"].clone())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn simple_messages_request_defaults_to_the_model_cap_and_fits_thinking() -> TestResult {
    crate::runtime::run(async {
        let model = messages::model(&json!({"maxTokens": 64_000, "contextWindow": 1_000_000}))?;
        let context = messages::context(&Value::Null)?;
        let plain = sent_body(model.clone(), context.clone(), None, None).await?;
        assert_eq!(plain["max_tokens"], json!(64_000));
        let thinking = sent_body(model, context, None, Some(ThinkingLevel::High)).await?;
        assert_eq!(thinking["max_tokens"], json!(64_000));
        assert_eq!(thinking["thinking"]["budget_tokens"], json!(16_384));
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn simple_budget_is_the_remaining_context_less_the_safety_margin() -> TestResult {
    crate::runtime::run(async {
        let context = context_with_tokens(4_000, 6_000)?;
        let model = small_window_model()?;
        let plain = sent_body(model.clone(), context.clone(), None, None).await?;
        assert_eq!(plain["max_tokens"], json!(5_904));
        let thinking = sent_body(
            model.clone(),
            context.clone(),
            None,
            Some(ThinkingLevel::High),
        )
        .await?;
        assert_eq!(thinking["max_tokens"], json!(5_904));
        assert_eq!(thinking["thinking"]["budget_tokens"], json!(4_880));
        let explicit = sent_body(model, context, Some(2_000.0), None).await?;
        assert_eq!(explicit["max_tokens"], json!(2_000));
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn simple_budget_never_falls_below_one_token() -> TestResult {
    let context = context_with_tokens(19_000, 100)?;
    let base = build_base_options(&small_window_model()?, &context, None, None);
    assert_eq!(base.max_tokens, Some(1.0));
    Ok(())
}

#[test]
fn thinking_without_an_explicit_limit_fits_inside_the_model_cap() {
    let roomy = adjust_max_tokens_for_thinking(None, 64_000.0, ThinkingLevel::High, None);
    assert_eq!(
        (roomy.max_tokens, roomy.thinking_budget),
        (64_000.0, 16_384.0)
    );
    let tight = adjust_max_tokens_for_thinking(None, 8_000.0, ThinkingLevel::High, None);
    assert_eq!(
        (tight.max_tokens, tight.thinking_budget),
        (8_000.0, 6_976.0)
    );
}

#[cfg(not(target_arch = "wasm32"))]
/// A 64,000-token model with a 40,000-token window, whose budget is the window less the
/// estimated context and 4,096 safety tokens.
fn estimate_window_model() -> TestResult<maestro_models::Model> {
    messages::model(&json!({"maxTokens": 64_000, "contextWindow": 40_000}))
}

#[cfg(not(target_arch = "wasm32"))]
/// An assistant message that reports `usage` and stops for `stop`, answering with `text`.
fn response(stop: &str, text: &str, usage: &Value) -> Value {
    json!({
        "role": "assistant", "content": [{"type": "text", "text": text}],
        "api": "anthropic-messages", "provider": "fixture", "model": "model",
        "usage": usage, "stopReason": stop, "timestamp": 2
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// The usage record of a response that reports `total` tokens in all.
fn usage_total(total: &Value) -> Value {
    json!({"input": 1, "output": 2, "cacheRead": 3, "cacheWrite": 4, "totalTokens": total,
        "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": 0}})
}

#[cfg(not(target_arch = "wasm32"))]
/// The output limit a simple request asks for after `messages` and a 1,000-token system prompt.
fn budget(messages: &Value) -> TestResult<Option<f64>> {
    let context: Context =
        serde_json::from_value(json!({"systemPrompt": "s".repeat(4_000), "messages": messages}))?;
    Ok(build_base_options(&estimate_window_model()?, &context, None, None).max_tokens)
}

#[cfg(not(target_arch = "wasm32"))]
/// The output limit left when the context is estimated at `tokens`.
fn left_after(tokens: f64) -> f64 {
    40_000.0 - 4_096.0 - tokens
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn estimate_anchors_on_the_newest_reported_usage_and_skips_the_prompt() -> TestResult {
    let messages = json!([
        {"role": "user", "content": "before", "timestamp": 1},
        response("stop", "reply", &usage_total(&json!(1_000))),
        {"role": "user", "content": "x".repeat(40), "timestamp": 3}
    ]);
    assert_eq!(budget(&messages)?, Some(left_after(1_010.0)));
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn estimate_skips_aborted_and_failed_responses_when_anchoring() -> TestResult {
    let messages = json!([
        response("stop", "reply", &usage_total(&json!(1_000))),
        response("aborted", &"a".repeat(8), &usage_total(&json!(5_000))),
        response("error", &"e".repeat(4), &usage_total(&json!(7_000)))
    ]);
    assert_eq!(budget(&messages)?, Some(left_after(1_003.0)));
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn estimate_sums_the_usage_categories_without_a_total() -> TestResult {
    let absent = json!({"input": 1, "output": 2, "cacheRead": 3, "cacheWrite": 4,
        "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": 0}});
    for usage in [usage_total(&json!(0)), usage_total(&Value::Null), absent] {
        let messages = json!([response("stop", "reply", &usage)]);
        assert_eq!(budget(&messages)?, Some(left_after(10.0)));
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn estimate_counts_images_at_a_fixed_size_and_text_in_utf16_units() -> TestResult {
    let image = json!({"type": "image", "data": "AAAA", "mimeType": "image/png"});
    let blocks = json!([{"type": "text", "text": "x".repeat(4)}, image]);
    let messages = json!([
        {"role": "user", "content": blocks, "timestamp": 1},
        {"role": "toolResult", "toolCallId": "c", "toolName": "t", "content": [image],
            "isError": false, "timestamp": 2},
        {"role": "user", "content": "😀😀😀😀", "timestamp": 3}
    ]);
    // 1,000 prompt tokens, (4 + 4,800) / 4 and 4,800 / 4 for the images, 8 units / 4 for the text.
    assert_eq!(
        budget(&messages)?,
        Some(left_after(1_000.0 + 1_201.0 + 1_200.0 + 2.0))
    );
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn estimate_writes_tool_arguments_as_compact_json_numbers() -> TestResult {
    let unreported = json!({"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0,
        "totalTokens": 0,
        "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": 0}});
    let mut call = response("toolUse", "", &unreported);
    call["content"] =
        json!([{"type": "toolCall", "id": "c", "name": "t", "arguments": {"n": 1.0}}]);
    // "t" and `{"n":1}` are 8 units, two tokens; `{"n":1.0}` would make it three.
    assert_eq!(budget(&json!([call]))?, Some(left_after(1_002.0)));
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn limit_without_a_context_window_is_raised_to_one_token_and_with_one_is_kept() -> TestResult {
    let context: Context = serde_json::from_value(json!({"messages": []}))?;
    let windowless = messages::model(&json!({"maxTokens": 64_000, "contextWindow": 0}))?;
    let windowed = messages::model(&json!({"maxTokens": 64_000, "contextWindow": 40_000}))?;
    let options = SimpleStreamOptions {
        common: StreamOptions {
            max_tokens: Some(0.0),
            ..StreamOptions::default()
        },
        ..SimpleStreamOptions::default()
    };
    let raised = build_base_options(&windowless, &context, Some(&options), None);
    assert_eq!(raised.max_tokens, Some(1.0));
    let kept = build_base_options(&windowed, &context, Some(&options), None);
    assert_eq!(kept.max_tokens, Some(0.0));
    Ok(())
}

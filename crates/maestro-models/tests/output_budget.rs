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

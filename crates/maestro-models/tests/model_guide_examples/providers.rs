//! Descriptors, destination projection and independently supplied options.

use crate::{bundled, chat::TestResult};
use maestro_models::{ProviderStreamOptions, complete, get_model, get_models};
use serde_json::json;

#[test]
fn maestro_guide_invokes_custom_descriptors() -> TestResult {
    let _registry = bundled::registry();
    assert!(get_model("openai", "gpt-4o-mini").is_some());
    assert!(get_models("missing-guide-provider").is_empty());
    assert!(get_model("openai", "missing-guide-model").is_none());
    crate::runtime::run(async {
        let model = crate::chat::model(&json!({
            "id":"local-weather", "provider":"local-guide", "baseUrl":"https://controlled.invalid/v1",
            "compat":{"supportsStore":false,"supportsDeveloperRole":false,"supportsUsageInStreaming":false,"maxTokensField":"max_tokens"}
        }))?;
        assert!(get_model(&model.provider, &model.id).is_none());
        let (mut common, requests) = super::response_options(&model.api, "Local answer");
        common.max_tokens = Some(73.0);
        common.headers = Some([("x-guide".into(), "local".into())].into());
        let mut context = bundled::conversation()?;
        context.system_prompt = Some("Be concise".into());
        let result = complete(
            model,
            context,
            Some(ProviderStreamOptions {
                common,
                ..Default::default()
            }),
        )
        .await?;
        assert_eq!(bundled::text_of(&result), "Local answer");
        let requests = bundled::recorded(&requests);
        assert_eq!(requests.len(), 1);
        let sent = &requests[0];
        assert_eq!(sent.url, "https://controlled.invalid/v1/chat/completions");
        assert!(
            sent.headers
                .iter()
                .any(|(name, value)| name == "x-guide" && value == "local")
        );
        assert!(sent.headers.iter().any(|(name, _)| name == "authorization"));
        assert_eq!(sent.body["model"], "local-weather");
        assert_eq!(sent.body["max_tokens"], 73);
        assert!(sent.body.get("max_completion_tokens").is_none());
        assert!(sent.body.get("store").is_none());
        assert!(sent.body.get("stream_options").is_none());
        assert_eq!(
            sent.body["messages"][0],
            json!({"role":"system","content":"Be concise"})
        );
        Ok(())
    })
}

/// Eight destination projections and one complete persisted conversation.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Histories {
    /// Exact input and expected output for each destination.
    projections: Vec<Projection>,
    /// Complete context JSON including explicit null details.
    context: ContextRoundtrip,
}

/// One typed input to the public projection operation.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Projection {
    /// Supplied destination.
    model: maestro_models::Model,
    /// Authored history, borrowed by the operation.
    messages: Vec<maestro_models::Message>,
    /// Expected serialized request history.
    expected: serde_json::Value,
}

/// The input and exact output of context serialization.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextRoundtrip {
    /// Complete context value to decode.
    input: serde_json::Value,
    /// Expected encoded value.
    expected: serde_json::Value,
}

#[test]
fn maestro_guide_projects_and_serializes_history() -> TestResult {
    use maestro_models::{Context, Message, transform_messages};
    let _registry = bundled::registry();
    let histories: Histories = serde_json::from_str(include_str!("history.json"))?;
    assert_eq!(histories.projections.len(), 8);
    for row in histories.projections {
        assert_eq!(
            crate::json::canonical(serde_json::to_value(transform_messages(
                &row.messages,
                &row.model,
                None
            ))?),
            row.expected
        );
    }
    let context: Context = serde_json::from_value(histories.context.input)?;
    assert_eq!(
        crate::json::canonical(serde_json::to_value(context)?),
        histories.context.expected
    );
    crate::runtime::run(async {
        let (assistant, call) = super::tools::invalid_completed_call().await?;
        let mut history = bundled::conversation()?;
        history.messages.push(Message::Assistant(assistant));
        history.messages.push(serde_json::from_value(json!({
            "role":"toolResult","toolCallId":call.id,"toolName":call.name,
            "content":[{"type":"text","text":"Choose supported units"}],"details":null,"isError":true,"timestamp":3
        }))?);
        let model = crate::chat::model(&json!({"provider":"other"}))?;
        let (common, requests) = super::response_options(&model.api, "Retrying");
        let result = complete(
            model,
            history,
            Some(ProviderStreamOptions {
                common,
                ..Default::default()
            }),
        )
        .await?;
        assert_eq!(bundled::text_of(&result), "Retrying");
        let sent = &bundled::recorded(&requests)[0].body;
        assert_eq!(sent["messages"][1]["tool_calls"][0]["id"], "weather-1");
        assert_eq!(
            sent["messages"][1]["tool_calls"][0]["function"]["arguments"],
            "{\"location\":\"London\",\"units\":\"kelvin\"}"
        );
        assert_eq!(
            sent["messages"][2],
            json!({"role":"tool","tool_call_id":"weather-1","content":"Choose supported units"})
        );
        Ok(())
    })
}

#[test]
fn maestro_guide_selects_thinking_and_transport_options() -> TestResult {
    use maestro_models::{
        CacheRetention, ModelThinkingLevel, Transport, clamp_thinking_level,
        get_supported_thinking_levels,
    };
    let _registry = bundled::registry();
    for (reasoning, levels, selected) in [
        (
            false,
            vec![ModelThinkingLevel::Off],
            ModelThinkingLevel::Off,
        ),
        (
            true,
            vec![
                ModelThinkingLevel::Off,
                ModelThinkingLevel::Medium,
                ModelThinkingLevel::High,
            ],
            ModelThinkingLevel::Medium,
        ),
    ] {
        let model = crate::chat::model(
            &json!({"reasoning":reasoning,"thinkingLevelMap":{"minimal":null,"low":null,"high":"high","xhigh":null}}),
        )?;
        assert_eq!(get_supported_thinking_levels(&model), levels);
        assert_eq!(
            clamp_thinking_level(&model, ModelThinkingLevel::Low),
            selected
        );
    }
    assert_eq!(
        serde_json::to_value([
            Transport::Sse,
            Transport::Websocket,
            Transport::WebsocketCached,
            Transport::Auto
        ])?,
        json!(["sse", "websocket", "websocket-cached", "auto"])
    );
    assert_eq!(
        serde_json::to_value([
            CacheRetention::None,
            CacheRetention::Short,
            CacheRetention::Long
        ])?,
        json!(["none", "short", "long"])
    );
    check_invocation_options()
}

/// Independently chosen raw and simple controls reach the actual HTTP request.
fn check_invocation_options() -> TestResult {
    use maestro_models::{SimpleStreamOptions, ThinkingLevel, complete_simple};
    crate::runtime::run(async {
        let model = crate::chat::model(&json!({"compat":{"supportsReasoningEffort":true}}))?;
        let (mut raw, requests) = super::response_options(&model.api, "Raw");
        raw.temperature = Some(0.0);
        raw.max_tokens = Some(73.0);
        let result = complete(
            model.clone(),
            bundled::conversation()?,
            Some(ProviderStreamOptions {
                common: raw,
                extra: serde_json::from_value(json!({"reasoningEffort":"medium"}))?,
                ..Default::default()
            }),
        )
        .await?;
        assert_eq!(bundled::text_of(&result), "Raw");
        let body = &bundled::recorded(&requests)[0].body;
        assert_eq!(body["temperature"], 0);
        assert_eq!(body["max_completion_tokens"], 73);
        assert_eq!(body["reasoning_effort"], "medium");
        let (mut common, requests) = super::response_options(&model.api, "Simple");
        common.temperature = Some(0.25);
        common.max_tokens = Some(31.0);
        let result = complete_simple(
            model,
            bundled::conversation()?,
            Some(SimpleStreamOptions {
                common,
                reasoning: Some(ThinkingLevel::High),
                ..Default::default()
            }),
        )
        .await?;
        assert_eq!(bundled::text_of(&result), "Simple");
        let body = &bundled::recorded(&requests)[0].body;
        assert_eq!(body["temperature"], 0.25);
        assert_eq!(body["max_completion_tokens"], 31);
        assert_eq!(body["reasoning_effort"], "high");
        Ok(())
    })
}

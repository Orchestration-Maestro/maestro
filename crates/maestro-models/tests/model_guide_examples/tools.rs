//! Tool declarations, argument progress and caller-owned execution.

use super::weather_tool;
#[cfg(not(target_arch = "wasm32"))]
use crate::bundled;
use crate::chat::TestResult;
use maestro_models::{StringEnumOptions, string_enum};
use serde_json::json;

#[test]
fn maestro_guide_builds_portable_tool_schemas() -> TestResult {
    let values = ["celsius", "fahrenheit"];
    let bare = json!({"type":"string","enum":["celsius","fahrenheit"]});
    assert_eq!(string_enum(&values, None), bare);
    assert_eq!(
        string_enum(
            &values,
            Some(StringEnumOptions {
                description: Some(""),
                default: Some("")
            })
        ),
        bare
    );
    assert_eq!(
        string_enum(
            &values,
            Some(StringEnumOptions {
                description: Some("Temperature units"),
                default: Some("celsius")
            })
        ),
        json!({"type":"string","enum":["celsius","fahrenheit"],"description":"Temperature units","default":"celsius"})
    );
    let tool = serde_json::to_value(weather_tool())?;
    assert_eq!(tool["name"], "get_weather");
    assert_eq!(tool["description"], "Look up weather");
    assert_eq!(
        tool["parameters"]["properties"]["units"],
        json!({"type":"string","enum":["celsius","fahrenheit"],"default":"celsius"})
    );
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
/// A controlled server completes a syntactically valid but disallowed enum value.
pub(super) async fn invalid_completed_call()
-> TestResult<(maestro_models::AssistantMessage, maestro_models::ToolCall)> {
    use maestro_models::{AssistantMessageEvent, ProviderStreamOptions, StreamOptions, stream};
    let frames = bundled::sse_frames(&[
        json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"weather-1","type":"function","function":{"name":"get_weather","arguments":"{\"location\":\"London\",\"units\":\"kelvin\"}"}}]},"finish_reason":null}]}),
        json!({"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}),
    ]);
    let (fetch, _) = bundled::fetch(frames);
    let events = stream(
        crate::chat::model(&json!({}))?,
        bundled::conversation()?,
        Some(ProviderStreamOptions {
            common: StreamOptions {
                api_key: Some("controlled-key".into()),
                fetch: Some(fetch),
                ..Default::default()
            },
            ..Default::default()
        }),
    )?;
    let mut completed = None;
    while let Some(event) = events.next().await {
        if let AssistantMessageEvent::ToolcallEnd { tool_call, .. } = event {
            completed = Some(tool_call);
        }
    }
    let assistant = events
        .result()
        .await
        .read()
        .map_err(|_| "message lock")?
        .clone();
    Ok((assistant, completed.ok_or("completed tool call")?))
}

#[cfg(not(target_arch = "wasm32"))]
/// Only checked arguments reach this example's tool effect.
fn execute_weather(
    call: &maestro_models::ToolCall,
    executions: &mut usize,
) -> Result<String, maestro_models::DiagnosticErrorInfo> {
    let arguments = maestro_models::validate_tool_call(&[weather_tool()], call)?;
    *executions += 1;
    Ok(format!(
        "Weather for {}",
        arguments["location"].as_str().unwrap_or_default()
    ))
}

#[cfg(not(target_arch = "wasm32"))]
/// Exact feedback and absence semantics of the recorded validation inputs.
fn check_validation_inputs(invalid: &maestro_models::ToolCall) -> TestResult {
    use maestro_models::validate_tool_call;
    let tools = [weather_tool()];
    for arguments in [
        json!({"location":"London","units":"celsius"}),
        json!({"location":"London"}),
    ] {
        let call = maestro_models::ToolCall {
            arguments: serde_json::from_value(arguments.clone())?,
            ..invalid.clone()
        };
        assert_eq!(
            serde_json::to_value(validate_tool_call(&tools, &call)?)?,
            arguments
        );
    }
    assert_eq!(
        validate_tool_call(&tools, invalid)
            .err()
            .ok_or("invalid enum accepted")?
            .message,
        "Validation failed for tool \"get_weather\":\n  - units: must be equal to one of the allowed values\n\nReceived arguments:\n{\n  \"location\": \"London\",\n  \"units\": \"kelvin\"\n}"
    );
    let missing = maestro_models::ToolCall {
        arguments: maestro_models::JsonObject::default().into(),
        ..invalid.clone()
    };
    assert_eq!(
        validate_tool_call(&tools, &missing)
            .err()
            .ok_or("required field accepted")?
            .message,
        "Validation failed for tool \"get_weather\":\n  - location: must have required properties location\n\nReceived arguments:\n{}"
    );
    let unknown = maestro_models::ToolCall {
        name: "missing".into(),
        arguments: serde_json::from_value(json!({"location":"London"}))?,
        ..invalid.clone()
    };
    assert_eq!(
        validate_tool_call(&tools, &unknown)
            .err()
            .ok_or("unknown tool accepted")?
            .message,
        "Tool \"missing\" not found"
    );
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_guide_validates_before_tool_execution() -> TestResult {
    use maestro_models::{Message, ProviderStreamOptions, complete};
    let _registry = bundled::registry();
    crate::runtime::run(async {
        let (assistant, call) = invalid_completed_call().await?;
        let mut executions = 0;
        let feedback = execute_weather(&call, &mut executions).unwrap_err();
        assert_eq!(executions, 0);
        check_validation_inputs(&call)?;
        let mut context = bundled::conversation()?;
        context.messages.push(Message::Assistant(assistant));
        context.messages.push(serde_json::from_value(
            json!({"role":"toolResult","toolCallId":call.id,"toolName":call.name,
            "content":[{"type":"text","text":feedback.message}],"isError":true,"timestamp":3}),
        )?);
        let valid = maestro_models::ToolCall {
            arguments: serde_json::from_value(json!({"location":"London","units":"celsius"}))?,
            ..call
        };
        assert_eq!(
            execute_weather(&valid, &mut executions)?,
            "Weather for London"
        );
        assert_eq!(executions, 1);
        let model = crate::chat::model(&json!({}))?;
        let (common, requests) = super::response_options(&model.api, "Please use celsius");
        let result = complete(
            model,
            context,
            Some(ProviderStreamOptions {
                common,
                ..Default::default()
            }),
        )
        .await?;
        assert_eq!(bundled::text_of(&result), "Please use celsius");
        let request = &bundled::recorded(&requests)[0].body;
        assert_eq!(request["messages"][2]["role"], "tool");
        assert_eq!(request["messages"][2]["tool_call_id"], "weather-1");
        assert_eq!(request["messages"][2]["content"], feedback.message);
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_guide_reads_partial_arguments_without_execution() -> TestResult {
    use maestro_models::{
        AnthropicOptions, AssistantMessageEvent, stream_anthropic, validate_tool_call,
    };
    let _registry = bundled::registry();
    check_partial_parser();
    crate::runtime::run(async {
        let (mut feed, common) = super::fed_options();
        let events = stream_anthropic(
            crate::messages::model(&json!({}))?,
            bundled::conversation()?,
            Some(AnthropicOptions {
                common,
                ..Default::default()
            }),
        );
        let mut chunks = argument_frames().into_iter();
        feed.chunks.send(chunks.next().ok_or("initial frames")?)?;
        let expected = [
            json!({}),
            json!({"location":"Lon"}),
            json!({"location":"London","units":"kelvin"}),
        ];
        let mut observed = 0;
        let mut completed = None;
        while let Some(event) = events.next().await {
            if let AssistantMessageEvent::ToolcallDelta {
                content_index,
                partial,
                ..
            } = &event
            {
                assert_eq!(*content_index, 0);
                check_live_arguments(partial, *content_index, &expected[observed])?;
                feed.read_pending().await?;
                feed.chunks
                    .send(chunks.next().ok_or("next argument frames")?)?;
                observed += 1;
            }
            if let AssistantMessageEvent::ToolcallEnd { tool_call, .. } = event {
                completed = Some(tool_call);
            }
        }
        feed.released.await?;
        assert_eq!(observed, 3);
        let call = completed.ok_or("completed call")?;
        assert_eq!(call.arguments["units"], "kelvin");
        assert!(validate_tool_call(&[weather_tool()], &call).is_err());
        assert_eq!(
            bundled::stop_of(&events.result().await),
            maestro_models::StopReason::ToolUse
        );
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// Recorded partial JSON values, before any execution validation.
fn check_partial_parser() {
    use maestro_models::parse_streaming_json;
    for (text, expected) in [
        ("", json!({})),
        ("{", json!({})),
        ("{\"location\":\"Lon", json!({"location":"Lon"})),
        (
            "{\"location\":\"London\",\"units\":\"celsius\"}",
            json!({"location":"London","units":"celsius"}),
        ),
        (
            "{\"location\":\"London\",\"units\":\"kelvin\"}",
            json!({"location":"London","units":"kelvin"}),
        ),
    ] {
        assert_eq!(parse_streaming_json(Some(text)), expected);
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Finite segments released after each live argument observation.
fn argument_frames() -> Vec<Vec<u8>> {
    let segments = [
        vec![
            json!({"type":"message_start","message":{"id":"m","usage":{"input_tokens":3,"output_tokens":0}}}),
            json!({"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"weather-1","name":"get_weather","input":{}}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{"}}),
        ],
        vec![
            json!({"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"\"location\":\"Lon"}}),
        ],
        vec![
            json!({"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"don\",\"units\":\"kelvin\"}"}}),
        ],
        vec![
            json!({"type":"content_block_stop","index":0}),
            json!({"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":2}}),
            json!({"type":"message_stop"}),
        ],
    ];
    segments
        .iter()
        .map(|events| {
            events
                .iter()
                .map(crate::messages::sse)
                .collect::<String>()
                .into_bytes()
        })
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
/// Borrow the live block only until the next body fragment is released.
fn check_live_arguments(
    partial: &maestro_models::SharedAssistantMessage,
    index: usize,
    expected: &serde_json::Value,
) -> TestResult {
    let message = partial.read().map_err(|_| "message lock")?;
    let maestro_models::AssistantContent::ToolCall(call) = &message.content[index] else {
        return Err("tool block".into());
    };
    assert_eq!(&serde_json::to_value(&call.arguments)?, expected);
    Ok(())
}

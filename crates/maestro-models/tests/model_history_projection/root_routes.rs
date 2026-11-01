//! Whole-root replay observed in each protocol's outgoing HTTP request.
#![cfg(test)]
use crate::{chat, root_routes, transport};
use maestro_models::providers::cloud::amazon_bedrock::{BedrockOptions, stream_bedrock};
use maestro_models::*;
use serde_json::{Value, json};

pub async fn check() -> chat::TestResult {
    for row in root_routes::recorded()?.history {
        let input = match row.input_json {
            Some(raw) => serde_json::from_str(&raw)?,
            None => row.input,
        };
        for (api, expected) in [
            ("openai-completions", &row.expected.chat),
            ("openai-responses", &row.expected.responses),
            ("anthropic-messages", &row.expected.message),
            ("google-generative-ai", &row.expected.content),
            ("bedrock-converse-stream", &row.expected.cloud),
            ("mistral-conversations", &row.expected.reasoning),
        ] {
            let request = outgoing(api, &input).await?;
            assert_call(api, &request, expected);
        }
    }
    Ok(())
}

async fn outgoing(api: &str, input: &Value) -> chat::TestResult<Value> {
    let scripted = transport::transport(vec![answer(api)?]);
    let model = chat::model(&json!({
        "id":"controlled-model", "api":api, "provider":"controlled", "reasoning":false,
        "baseUrl":"https://fixture.invalid/v1"
    }))?;
    let context = chat::context(&json!({"messages":[
        {"role":"assistant", "api":api, "provider":"controlled", "model":"controlled-model",
         "content":[{"type":"toolCall", "id":"call12345", "name":"inspect", "arguments":input}]},
        {"role":"toolResult", "toolCallId":"call12345", "toolName":"inspect",
         "content":[{"type":"text", "text":"answered"}], "isError":false}
    ]}))?;
    let common = StreamOptions {
        fetch: Some(scripted.fetch.clone()),
        api_key: Some("fixture-key".into()),
        ..Default::default()
    };
    let stream = invoke(api, model, context, common);
    let (_, result) = transport::drain(&stream).await?;
    assert_eq!(result.stop_reason, StopReason::Stop, "{api}");
    assert_eq!(scripted.attempts(), 1, "{api}");
    Ok(serde_json::from_slice(&scripted.first_request()?.1)?)
}

fn invoke(
    api: &str,
    model: Model,
    context: Context,
    common: StreamOptions,
) -> AssistantMessageEventStream {
    match api {
        "openai-completions" => stream_openai_completions(
            model,
            context,
            Some(OpenAICompletionsOptions {
                common,
                ..Default::default()
            }),
        ),
        "openai-responses" => stream_openai_responses(
            model,
            context,
            Some(OpenAIResponsesOptions {
                common,
                ..Default::default()
            }),
        ),
        "anthropic-messages" => stream_anthropic(
            model,
            context,
            Some(AnthropicOptions {
                common,
                ..Default::default()
            }),
        ),
        "google-generative-ai" => stream_google(
            model,
            context,
            Some(GoogleOptions {
                common,
                ..Default::default()
            }),
        ),
        "bedrock-converse-stream" => stream_bedrock(
            model,
            context,
            Some(BedrockOptions {
                common,
                bearer_token: Some("fixture-token".into()),
                region: Some("us-east-1".into()),
                ..Default::default()
            }),
        ),
        "mistral-conversations" => stream_mistral(
            model,
            context,
            Some(MistralOptions {
                common,
                ..Default::default()
            }),
        ),
        _ => panic!("unknown protocol"),
    }
}

fn answer(api: &str) -> chat::TestResult<transport::Attempt> {
    let body = match api {
        "openai-responses" => crate::response_endpoint::COMPLETE.as_bytes().to_vec(),
        "anthropic-messages" => [
            crate::messages::sse(&json!({"type":"message_start", "message":{"id":"reply", "type":"message", "role":"assistant", "content":[], "model":"controlled-model", "usage":{"input_tokens":0,"output_tokens":0}}})),
            crate::messages::sse(&json!({"type":"message_stop"})),
        ].concat().into_bytes(),
        "google-generative-ai" => Vec::new(),
        "bedrock-converse-stream" => return Ok(crate::bedrock::stream_of(200, &[], crate::bedrock::answer("reply")?)),
        _ => b"data: [DONE]\n\n".to_vec(),
    };
    Ok(transport::Attempt::body(
        200,
        &[("content-type", "text/event-stream")],
        body,
    ))
}

fn assert_call(api: &str, request: &Value, expected: &Value) {
    let (call, field, id) = match api {
        "openai-completions" | "mistral-conversations" => {
            let call = &request["messages"][0]["tool_calls"][0];
            assert_eq!(call["type"], "function");
            assert_eq!(call["id"], "call12345");
            (&call["function"], "arguments", None)
        }
        "openai-responses" => {
            let call = &request["input"][0];
            assert_eq!(call["type"], "function_call");
            (call, "arguments", Some("call_id"))
        }
        "anthropic-messages" => {
            let call = &request["messages"][0]["content"][0];
            assert_eq!(call["type"], "tool_use");
            (call, "input", Some("id"))
        }
        "google-generative-ai" => (
            &request["contents"][0]["parts"][0]["functionCall"],
            "args",
            None,
        ),
        "bedrock-converse-stream" => (
            &request["messages"][0]["content"][0]["toolUse"],
            "input",
            Some("toolUseId"),
        ),
        _ => panic!("unknown protocol"),
    };
    assert_eq!(call["name"], "inspect", "{api}");
    if let Some(id) = id {
        assert_eq!(call[id], "call12345", "{api}");
    }
    assert_eq!(&call[field], expected, "{api}");
    if let Some(object) = expected.as_object() {
        assert_eq!(
            call[field].as_object().unwrap().keys().collect::<Vec<_>>(),
            object.keys().collect::<Vec<_>>(),
            "{api}"
        );
    }
}

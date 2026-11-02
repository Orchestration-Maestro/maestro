//! Model invocation observations through the component's native imports.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    CacheRetention, Context, Model, ProviderStreamOptions, RequestedThinkingLevel,
    SimpleStreamOptions, StreamOptions, ThinkingBudgets, Transport,
};
use serde_json::{Value, json};

/// Invoke an unlisted descriptor through distinct raw and simple options.
pub async fn command() -> Result<String, String> {
    let model: Model = serde_json::from_value(json!({
        "id":"unlisted-雪", "name":"Unlisted", "api":"component-original-options", "provider":"custom",
        "baseUrl":"https://example.invalid/literal", "reasoning":true,"input":["text"],
        "cost":{"input":1.5,"output":2.5,"cacheRead":3.5,"cacheWrite":4.5},
        "contextWindow":12345.5,"maxTokens":88.5,"headers":{"x-model":"literal"}
    })).map_err(|e| e.to_string())?;
    let context: Context = serde_json::from_value(json!({"systemPrompt":"instruction 雪", "messages":[{"role":"user","content":"literal input","timestamp":1.5}], "tools":[]})).map_err(|e| e.to_string())?;
    let mut output = Vec::new();
    for simple in [false, true] {
        for row in 0..3 {
            let common = if row == 2 {
                common(simple)
            } else {
                StreamOptions::default()
            };
            let stream = if simple {
                maestro_extensions_wasm::stream_simple(
                    model.clone(),
                    context.clone(),
                    (row != 0).then_some(SimpleStreamOptions {
                        common,
                        reasoning: (row == 2).then_some(RequestedThinkingLevel::High),
                        thinking_budgets: (row == 2).then_some(ThinkingBudgets {
                            minimal: Some(1.25),
                            low: Some(2.5),
                            medium: Some(3.75),
                            high: Some(4.5),
                        }),
                        tool_choice: None,
                    }),
                )?
            } else {
                maestro_extensions_wasm::stream(
                    model.clone(),
                    context.clone(),
                    raw_options(common, row)?,
                )?
            };
            let first = stream.result().await?;
            let second = stream.result().await?;
            let event = stream.next().await?.ok_or("missing terminal")?;
            output.push(json!({"simple":simple,"row":row,"sameOwner":std::sync::Arc::ptr_eq(&first,&second),"event":serde_json::to_value(&*event).map_err(|e| e.to_string())?,"eof":stream.next().await?.is_none()}));
        }
    }
    Ok(Value::Array(output).to_string())
}

/// Open raw settings do not acquire the simple path's similarly named policies.
fn raw_options(common: StreamOptions, row: usize) -> Result<Option<ProviderStreamOptions>, String> {
    if row == 0 {
        return Ok(None);
    }
    let extra = if row == 2 {
        serde_json::from_value(json!({"reasoning":"raw-policy","thinkingBudgets":{"high":99},"timeout":999,"custom":["z","a","z"]})).map_err(|e|e.to_string())?
    } else {
        serde_json::Map::new()
    };
    Ok(Some(ProviderStreamOptions {
        common,
        extra,
        objects: None,
    }))
}

/// Common data deliberately differs between the two invocation paths.
fn common(simple: bool) -> StreamOptions {
    StreamOptions {
        temperature: Some(-0.0),
        max_tokens: Some(17.5),
        api_key: Some("controlled".into()),
        transport: Some(Transport::WebsocketCached),
        cache_retention: Some(CacheRetention::Long),
        session_id: Some("session 雪".into()),
        headers: Some(
            [
                ("z-header".into(), "first".into()),
                ("a-header".into(), "second".into()),
            ]
            .into(),
        ),
        timeout_ms: Some(if simple { 222.5 } else { 111.5 }),
        max_retries: Some(if simple { 4.5 } else { 2.5 }),
        max_retry_delay_ms: Some(if simple { 888.5 } else { 444.5 }),
        metadata: Some(
            [
                ("roots".into(), json!([null, false, 0, "", {}, []])),
                ("n".into(), json!(-0.0)),
            ]
            .into_iter()
            .collect(),
        ),
        ..Default::default()
    }
}

/// Distinguish synchronous setup rejection from an already-started provider's error stream.
pub async fn errors() -> Result<String, String> {
    let mut output = Vec::new();
    for api in ["absent", "component-errors"] {
        for id in ["setup", "terminal"] {
            let model:Model = serde_json::from_value(json!({"id":id,"name":"error","api":api,"provider":"custom","baseUrl":"https://example.invalid","reasoning":false,"input":[],"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"contextWindow":1,"maxTokens":1})).map_err(|e|e.to_string())?;
            for simple in [false, true] {
                output.push(error_case(model.clone(), simple).await?);
            }
        }
    }
    Ok(Value::Array(output).to_string())
}
/// Observe both call boundaries independently for one entry point and provider outcome.
async fn error_case(model: Model, simple: bool) -> Result<Value, String> {
    let context = Context {
        system_prompt: None,
        messages: Vec::new(),
        tools: None,
    };
    let stream = if simple {
        maestro_extensions_wasm::stream_simple(model.clone(), context.clone(), None)
    } else {
        maestro_extensions_wasm::stream(model.clone(), context.clone(), None)
    };
    let completion = if simple {
        maestro_extensions_wasm::complete_simple(model.clone(), context, None)
    } else {
        maestro_extensions_wasm::complete(model.clone(), context, None)
    };
    let (call_error, terminal, eof) = match stream {
        Err(error) => (Some(error), None, true),
        Ok(stream) => {
            let message = stream.result().await?;
            let event = stream.next().await?.ok_or("missing error event")?;
            let message = serde_json::to_value(message).map_err(|e| e.to_string())?;
            (
                None,
                Some(
                    json!({"message":message,"event":serde_json::to_value(event.as_ref()).map_err(|e|e.to_string())?}),
                ),
                stream.next().await?.is_none(),
            )
        }
    };
    let completion = match completion.await {
        Err(error) => json!({"error":error}),
        Ok(message) => json!({"message":serde_json::to_value(message).map_err(|e|e.to_string())?}),
    };
    Ok(
        json!({"api":model.api,"id":model.id,"simple":simple,"callError":call_error,"terminal":terminal,"eof":eof,"completion":completion}),
    )
}

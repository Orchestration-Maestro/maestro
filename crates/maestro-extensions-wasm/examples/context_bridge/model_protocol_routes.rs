//! Await original native protocol hooks without imposing one protocol's phase policy on another.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    Context, Model, OnPayload, OnResponse, ProviderStreamOptions, SimpleStreamOptions,
    StreamOptions, Transport,
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

/// Each raw/simple call has its own hook identities and response-retry counter.
pub async fn command() -> Result<String, String> {
    let mut output = Vec::new();
    for simple in [false, true] {
        for (index, api) in [
            "openai-completions",
            "google-generative-ai",
            "google-vertex",
            "openai-codex-responses",
        ]
        .into_iter()
        .enumerate()
        {
            output.push(invoke(index, api, simple).await?);
        }
    }
    Ok(Value::Array(output).to_string())
}
/// The native registered adapter consumes each original callback and emits its actual terminal result.
async fn invoke(index: usize, api: &'static str, simple: bool) -> Result<Value, String> {
    let model: Model = serde_json::from_value(json!({
        "id":"routed-model","name":"routed","api":format!("component-protocol-{index}"),"provider":"custom",
        "baseUrl":"https://controlled.invalid","reasoning":false,"input":["text"],
        "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"contextWindow":1000,"maxTokens":100
    })).map_err(|e| e.to_string())?;
    let context: Context = serde_json::from_value(
        json!({"messages":[{"role":"user","content":"routed input","timestamp":1}]}),
    )
    .map_err(|e| e.to_string())?;
    let calls = Rc::new(RefCell::new(Vec::new()));
    let common = options(api, &calls);
    let stream = if simple {
        maestro_extensions_wasm::stream_simple(
            model,
            context,
            Some(SimpleStreamOptions {
                common,
                ..Default::default()
            }),
        )?
    } else {
        maestro_extensions_wasm::stream(
            model,
            context,
            Some(ProviderStreamOptions {
                common,
                ..Default::default()
            }),
        )?
    };
    let message = stream.result().await?;
    let same_owner = Arc::ptr_eq(&message, &stream.result().await?);
    let mut terminal = Value::Null;
    while let Some(event) = stream.next().await? {
        terminal = serde_json::to_value(event.as_ref()).map_err(|e| e.to_string())?["type"].clone();
    }
    Ok(
        json!({"simple":simple,"api":api,"calls":*calls.borrow(),"message":message,"sameOwner":same_owner,"terminal":terminal,"eof":stream.next().await?.is_none()}),
    )
}
/// The supplied synthetic key prevents credential lookup or live authorization.
fn options(api: &'static str, calls: &Rc<RefCell<Vec<&'static str>>>) -> StreamOptions {
    StreamOptions {
        api_key: Some(if api == "openai-codex-responses" {
            "a.eyJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF9hY2NvdW50X2lkIjoiY29udHJvbGxlZCJ9fQ.b".into()
        } else {
            "controlled-key".into()
        }),
        transport: Some(Transport::Sse),
        on_payload: Some(payload(api, calls.clone())),
        on_response: Some(response(api, calls.clone())),
        ..Default::default()
    }
}
/// Observe the payload phase through its original authored export.
fn payload(api: &'static str, calls: Rc<RefCell<Vec<&'static str>>>) -> OnPayload {
    Rc::new(move |input, model| {
        let calls = calls.clone();
        Box::pin(async move {
            if model.api != api || model.id != "routed-model" || !input.payload.is_object() {
                return Err("wrong routed payload operands".into());
            }
            calls.borrow_mut().push("payload");
            Ok(input)
        })
    })
}
/// Reject only the first response-session attempt; native policy decides whether to retry it.
fn response(api: &'static str, calls: Rc<RefCell<Vec<&'static str>>>) -> OnResponse {
    let count = Cell::new(0);
    Rc::new(move |response, model| {
        let first = count.replace(count.get() + 1) == 0;
        let calls = calls.clone();
        Box::pin(async move {
            if model.api != api
                || model.id != "routed-model"
                || response.status.to_bits() != 200_f64.to_bits()
                || response.headers.get("x-controlled").map(String::as_str) != Some("literal")
            {
                return Err("wrong routed response operands".into());
            }
            calls.borrow_mut().push("response");
            if api == "openai-codex-responses" && first {
                return Err("response retry".into());
            }
            Ok(())
        })
    })
}

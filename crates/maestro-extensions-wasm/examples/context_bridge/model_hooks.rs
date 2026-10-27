//! Awaited guest hooks select the values consumed by the registered native provider.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    Context, Model, ProviderStreamOptions, SimpleStreamOptions, StreamOptions,
};
use serde_json::{Value, json};
use std::rc::Rc;

/// The same controlled registered protocol accepts distinguishable descriptors.
fn model(id: &str) -> Result<Model, String> {
    serde_json::from_value(json!({"id":id,"name":"hooks","api":if id=="signal-model" {"component-hooks-signals"} else {"component-hooks"},"provider":"custom","baseUrl":"https://example.invalid","reasoning":false,"input":[],"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"contextWindow":1,"maxTokens":1})).map_err(|e|e.to_string())
}

/// Preserve, replace or remove the caller's arbitrary preaborted signal in the awaited hook.
pub async fn signals() -> Result<String, String> {
    let mut output = Vec::new();
    for simple in [false, true] {
        for action in ["keep", "replace", "remove"] {
            let common = signal_options(action);
            let context = Context {
                system_prompt: None,
                messages: Vec::new(),
                tools: None,
            };
            let stream = if simple {
                maestro_extensions_wasm::stream_simple(
                    model("signal-model")?,
                    context,
                    Some(SimpleStreamOptions {
                        common,
                        ..Default::default()
                    }),
                )?
            } else {
                maestro_extensions_wasm::stream(
                    model("signal-model")?,
                    context,
                    Some(ProviderStreamOptions {
                        common,
                        ..Default::default()
                    }),
                )?
            };
            output.push(json!({"simple":simple,"action":action,"message":serde_json::to_value(stream.result().await?).map_err(|e|e.to_string())?}));
        }
    }
    Ok(Value::Array(output).to_string())
}

/// Each payload selector returns an independently chosen cancellation capability.
fn signal_options(action: &'static str) -> StreamOptions {
    StreamOptions {
        signal: Some(super::exec::preaborted()),
        on_payload: Some(Rc::new(move |mut input, _| {
            Box::pin(async move {
                match action {
                    "replace" => input.signal = Some(super::exec::preaborted()),
                    "remove" => input.signal = None,
                    _ => {}
                }
                Ok(input)
            })
        })),
        on_response: Some(Rc::new(|_, _| Box::pin(async { Ok(()) }))),
        ..Default::default()
    }
}

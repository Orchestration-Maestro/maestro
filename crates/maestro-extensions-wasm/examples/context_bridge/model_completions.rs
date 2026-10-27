//! Completion starts before its retained result future is polled.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{Context, ExtensionFuture, Model, SharedAssistantMessage};
use serde_json::json;
use std::cell::RefCell;
thread_local! {
    /// Only test observation futures survive between distinct guest command invocations.
    static RESULTS:RefCell<Vec<ExtensionFuture<'static,SharedAssistantMessage>>> = RefCell::default();
}
/// Start both invocation paths now, or observe their independently retained results later.
pub async fn command(action: &str) -> Result<String, String> {
    if action == "start" {
        let model:Model = serde_json::from_value(json!({"id":"owned-model","name":"owned","api":"component-completions","provider":"custom","baseUrl":"https://example.invalid","reasoning":false,"input":[],"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"contextWindow":1,"maxTokens":1})).map_err(|e|e.to_string())?;
        let context = Context {
            system_prompt: Some("owned input".into()),
            messages: Vec::new(),
            tools: None,
        };
        let raw = maestro_extensions_wasm::complete(model.clone(), context.clone(), None);
        let simple = maestro_extensions_wasm::complete_simple(model, context, None);
        RESULTS.with_borrow_mut(|results| results.extend([raw, simple]));
        return Ok("started".into());
    }
    let results = RESULTS.with_borrow_mut(std::mem::take);
    if results.len() != 2 {
        return Err("missing completion futures".into());
    }
    let mut output = Vec::new();
    for result in results {
        output.push(serde_json::to_value(result.await?).map_err(|e| e.to_string())?);
    }
    Ok(serde_json::Value::Array(output).to_string())
}

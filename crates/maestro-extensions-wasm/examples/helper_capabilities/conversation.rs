//! Compiled author conversation observations preserve complete shared records.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{ExtensionAPI, ExtensionResult};
use serde_json::{Value, json};
/// Reads an unchanged native projection or transcript.
pub(super) fn observe(api: &ExtensionAPI, request: &Value) -> ExtensionResult<Value> {
    match request["operation"].as_str() {
        Some("msg.convertToLlm") => {
            let messages: Vec<maestro_extensions_wasm::AgentMessage> =
                serde_json::from_value(request["args"][0].clone())
                    .map_err(|error| error.to_string())?;
            Ok(json!(maestro_extensions_wasm::convert_to_llm(
                api, &messages
            )?))
        }
        Some("co.serializeConversation") => {
            let messages: Vec<maestro_extensions_wasm::Message> =
                serde_json::from_value(request["args"][0].clone())
                    .map_err(|error| error.to_string())?;
            Ok(json!(
                maestro_extensions_wasm::compaction::utils::serialize_conversation(api, &messages)?
            ))
        }
        _ => Err("unknown helper observation".into()),
    }
}

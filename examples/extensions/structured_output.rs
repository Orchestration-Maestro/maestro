//! A final structured answer returned by a terminating tool.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use maestro_extensions_wasm::{
    AgentToolResult, ExtensionAPI, ExtensionFuture, Presence, TextContent, ToolDefinition,
    ToolMetadata, UserBlock,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::rc::Rc;

/// The final answer retained in the tool result.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StructuredOutputDetails {
    /// Short title for the result.
    headline: String,
    /// One-paragraph summary.
    summary: String,
    /// Concrete next steps or key bullets.
    action_items: Vec<String>,
}

/// Registers the structured answer tool on the supplied host.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        api.register_tool(ToolDefinition {
            metadata: ToolMetadata {
                name: "structured_output".into(),
                label: "Structured Output".into(),
                description: "Return a final structured answer. Use this as your last action when the user asks for structured output or a machine-readable summary.".into(),
                prompt_snippet: Presence::Present("Emit a final structured answer as a terminating tool result".into()),
                prompt_guidelines: Presence::Present(vec![
                    "Use structured_output as your final action when the user asks for structured output, JSON-like output, or a machine-readable summary.".into(),
                    "After calling structured_output, do not emit another assistant response in the same turn.".into(),
                ]),
                parameters: json!({"type":"object","required":["headline","summary","actionItems"],"properties":{
                    "headline":{"type":"string","description":"Short title for the result"},
                    "summary":{"type":"string","description":"One-paragraph summary"},
                    "actionItems":{"type":"array","items":{"type":"string"},"description":"Concrete next steps or key bullets"}
                }}).into(),
                render_shell: Presence::Missing,
                execution_mode: Presence::Missing,
            },
            prepare_arguments: None,
            execute: Rc::new(|_, params, _, _, _| Box::pin(async move {
                let details: StructuredOutputDetails = serde_json::from_value(params).map_err(|e| e.to_string())?;
                Ok(AgentToolResult {
                    content: vec![UserBlock::Text(TextContent::new(format!("Saved structured output: {}", details.headline)))],
                    details: Presence::Present(serde_json::to_string(&details).map_err(|e| e.to_string())?.into()),
                    terminate: Presence::Present(true),
                })
            })),
            render_call: None,
            render_result: None,
        })
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);

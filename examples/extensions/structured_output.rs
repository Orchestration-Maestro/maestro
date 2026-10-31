//! A final structured answer returned by a terminating tool.
// maestro-component: MAESTRO_STRUCTURED_OUTPUT_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use maestro_extensions_wasm::{
    AgentToolResult, Component, ExtensionAPI, ExtensionFuture, ExtensionResult, Presence, Text,
    TextContent, Theme, ToolDefinition, ToolMetadata, UserBlock,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::rc::Rc;

/// The final answer retained in the tool result.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StructuredOutputDetails {
    /// A brief answer title.
    headline: String,
    /// The answer in one paragraph.
    summary: String,
    /// Specific actions to take or points to retain.
    action_items: Vec<String>,
}

/// Renders the complete answer in either expansion state, or its first text block.
///
/// # Errors
/// Returns an error if details cannot be decoded or a theme or text host call fails.
fn render_result(result: &AgentToolResult, theme: &Theme) -> ExtensionResult<Component> {
    let details = result
        .details
        .present()
        .map(|details| serde_json::from_str::<serde_json::Value>(details))
        .transpose()
        .map_err(|e| e.to_string())?;
    let Some(details) = details.filter(maestro_json::is_truthy) else {
        let text = match result.content.first() {
            Some(UserBlock::Text(text)) => &text.text,
            _ => "",
        };
        return Ok((&Text::new(text, 0, 0, None)?).into());
    };
    let details: StructuredOutputDetails =
        serde_json::from_value(details).map_err(|e| e.to_string())?;
    let mut lines = vec![
        theme.fg("toolTitle", &theme.bold(&details.headline)?)?,
        theme.fg("text", &details.summary)?,
        String::new(),
    ];
    for (index, item) in details.action_items.iter().enumerate() {
        lines.push(theme.fg("muted", &format!("{}. {item}", index + 1))?);
    }
    Ok((&Text::new(&lines.join("\n"), 0, 0, None)?).into())
}

/// Registers the structured answer tool on the supplied host.
///
/// # Errors
/// The returned future fails if the host rejects tool registration.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        api.register_tool(ToolDefinition {
            metadata: ToolMetadata {
                name: "structured_output".into(),
                label: "Structured Answer".into(),
                description: "Finish with a structured answer when the user requests structured output or a machine-readable summary. Call this tool last.".into(),
                prompt_snippet: Presence::Present("End the turn with a structured answer in the tool result".into()),
                prompt_guidelines: Presence::Present(vec![
                    "For structured, JSON-like or machine-readable answers, make structured_output the final action.".into(),
                    "No assistant message may follow structured_output within that turn.".into(),
                ]),
                parameters: json!({"type":"object","required":["headline","summary","actionItems"],"properties":{
                    "headline":{"type":"string","description":"A brief answer title"},
                    "summary":{"type":"string","description":"The answer in one paragraph"},
                    "actionItems":{"type":"array","items":{"type":"string"},"description":"Specific actions to take or points to retain"}
                }}).into(),
                render_shell: Presence::Missing,
                execution_mode: Presence::Missing,
            },
            prepare_arguments: None,
            execute: Rc::new(|_, params, _, _, _| Box::pin(async move {
                let details: StructuredOutputDetails = serde_json::from_value(params).map_err(|e| e.to_string())?;
                Ok(AgentToolResult {
                    content: vec![UserBlock::Text(TextContent::new(format!("Structured answer ready: {}", details.headline)))],
                    details: Presence::Present(serde_json::to_string(&details).map_err(|e| e.to_string())?.into()),
                    terminate: Presence::Present(true),
                    usage: None,
                })
            })),
            render_call: None,
            render_result: Some(Rc::new(|result, _, theme, _, _| {
                render_result(&result.result, &theme)
            })),
        })
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);

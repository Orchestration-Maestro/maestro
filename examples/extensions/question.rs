//! An opt-in single-question tool with an inline custom answer.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    AgentToolResult, ExtensionAPI, ExtensionFuture, Presence, TextContent, ToolDefinition,
    ToolMetadata, UserBlock,
};
use serde::Deserialize;
use serde_json::json;
use std::rc::Rc;

/// One supplied choice, retained in authored order.
#[derive(Deserialize)]
struct OptionWithDesc {
    /// Displayed and returned choice text.
    label: String,
}
/// The question and its supplied choices.
#[derive(Deserialize)]
struct Params {
    /// Text shown above the choices.
    question: String,
    /// Original choices, excluding custom input.
    options: Vec<OptionWithDesc>,
}
/// Builds the headless result without inventing an answer.
fn headless(params: &Params) -> AgentToolResult {
    AgentToolResult {
        content: vec![UserBlock::Text(TextContent::new("Error: UI not available (running in non-interactive mode)".into()))],
        details: Presence::Present(json!({"question":params.question,"options":params.options.iter().map(|o| &o.label).collect::<Vec<_>>(),"answer":null}).to_string().into()),
        terminate: Presence::Missing,
    }
}
/// Registers the question tool with the supplied host.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        api.register_tool(ToolDefinition {
            metadata: ToolMetadata {
                name: "question".into(), label: "Question".into(),
                description: "Ask the user a question and let them pick from options. Use when you need user input to proceed.".into(),
                parameters: json!({"type":"object","required":["question","options"],"properties":{
                    "question":{"type":"string","description":"The question to ask the user"},
                    "options":{"type":"array","description":"Options for the user to choose from","items":{"type":"object","required":["label"],"properties":{
                        "label":{"type":"string","description":"Display label for the option"},
                        "description":{"type":"string","description":"Optional description shown below label"}
                    }}}
                }}).into(),
                prompt_snippet: Presence::Missing, prompt_guidelines: Presence::Missing,
                render_shell: Presence::Missing, execution_mode: Presence::Missing,
            },
            prepare_arguments: None,
            execute: Rc::new(|_, params, _, _, _| Box::pin(async move {
                let params: Params = serde_json::from_value(params).map_err(|e| e.to_string())?;
                Ok(headless(&params))
            })),
            render_call: None, render_result: None,
        })
    })
}
#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);

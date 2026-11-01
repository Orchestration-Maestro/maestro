//! An opt-in single-question tool with an inline custom answer.
// maestro-component: MAESTRO_QUESTION_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    AgentToolResult, ExtensionAPI, ExtensionContext, ExtensionFuture, ExtensionResult, Presence,
    TextContent, ToolDefinition, ToolMetadata, UserBlock,
};
use serde::Deserialize;
use serde_json::json;
use std::rc::Rc;

/// One supplied choice, retained in authored order.
#[derive(Clone, Deserialize)]
struct OptionWithDesc {
    /// Displayed and returned choice text.
    label: String,
    /// Optional explanation displayed below the label.
    description: Option<String>,
}
/// The question and its supplied choices.
#[derive(Clone, Deserialize)]
struct Params {
    /// Text shown above the choices.
    question: String,
    /// Original choices, excluding custom input.
    options: Vec<OptionWithDesc>,
}
/// Compact tool-call and result presentation.
#[path = "question/render.rs"]
mod render;
/// Retained choices and inline editing through the supplied terminal host.
#[path = "question/view.rs"]
mod view;
/// A supplied choice or a trimmed custom response.
struct Selection {
    /// Returned answer text.
    answer: String,
    /// Whether the editor supplied the answer.
    was_custom: bool,
    /// One-based supplied option position, absent for custom input.
    index: Option<usize>,
}
/// Builds ordinary result details, omitting the custom flag without an answer.
fn result(params: &Params, text: String, answer: Option<&Selection>) -> AgentToolResult {
    let mut details = json!({"question":params.question,"options":params.options.iter().map(|o| &o.label).collect::<Vec<_>>(),"answer":answer.map(|a| &a.answer)});
    if let Some(answer) = answer {
        details["wasCustom"] = json!(answer.was_custom);
    }
    AgentToolResult {
        content: vec![UserBlock::Text(TextContent::new(text))],
        details: Presence::Present(details.to_string().into()),
        terminate: Presence::Missing,
    }
}
/// Opens the view only when the host has UI and at least one supplied option.
async fn execute(
    api: ExtensionAPI,
    params: Params,
    ctx: ExtensionContext,
) -> ExtensionResult<AgentToolResult> {
    if !ctx.has_ui()? {
        return Ok(result(
            &params,
            "Error: UI not available (running in non-interactive mode)".into(),
            None,
        ));
    }
    if params.options.is_empty() {
        return Ok(result(&params, "Error: No options provided".into(), None));
    }
    let answer = view::ask(api, params.clone(), &ctx).await?;
    let text = match &answer {
        None => "User cancelled the selection".into(),
        Some(a) if a.was_custom => format!("User wrote: {}", a.answer),
        Some(a) => format!(
            "User selected: {}. {}",
            a.index.unwrap_or_default(),
            a.answer
        ),
    };
    Ok(result(&params, text, answer.as_ref()))
}
/// Registers the question tool with the supplied host.
///
/// # Errors
/// The returned future returns the host's rejection when it refuses the tool registration.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let tool_api = api.clone();
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
            execute: Rc::new(move |_, params, _, _, ctx| {
                let api = tool_api.clone();
                Box::pin(async move {
                    let params: Params = serde_json::from_value(params).map_err(|e| e.to_string())?;
                    execute(api, params, ctx).await
                })
            }),
            render_call: Some(Rc::new(|args, theme, _, _| render::call(args.as_ref(), &theme))),
            render_result: Some(Rc::new(|result, _, theme, _, _| render::result(&result.result, &theme))),
        })
    })
}
#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);

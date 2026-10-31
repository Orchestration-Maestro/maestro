//! A questionnaire tool with selectable options and inline custom answers.
// maestro-component: MAESTRO_QUESTIONNAIRE_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use maestro_extensions_wasm::{
    AgentToolResult, ExtensionAPI, ExtensionContext, ExtensionFuture, ExtensionResult, Presence,
    TextContent, ToolDefinition, ToolMetadata, UserBlock,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::rc::Rc;

/// Retained navigation and inline editor.
#[path = "questionnaire/interface.rs"]
mod interface;
/// Questionnaire frames and compact tool summaries.
#[path = "questionnaire/rendering.rs"]
mod rendering;

/// One selectable answer.
#[derive(Clone, Deserialize, Serialize)]
struct QuestionOption {
    /// Returned value.
    value: String,
    /// Visible option text.
    label: String,
    /// Optional explanation below the option.
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    /// Additional caller-authored option data retained in result details.
    #[serde(flatten)]
    extra: serde_json::Map<String, Value>,
}
/// A question with normalized display defaults.
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Question {
    /// Identifier retained in answers.
    id: String,
    /// Short tab title, defaulted before showing the view.
    #[serde(default)]
    label: String,
    /// Full question text.
    prompt: String,
    /// Supplied options in display order.
    options: Vec<QuestionOption>,
    /// Whether custom input is offered.
    #[serde(default = "allow_other")]
    allow_other: bool,
    /// Additional caller-authored question data retained in result details.
    #[serde(flatten)]
    extra: serde_json::Map<String, Value>,
}
/// Custom input is available unless explicitly disabled.
fn allow_other() -> bool {
    true
}
/// One answer, in first-answer insertion order.
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Answer {
    /// Question identifier.
    id: String,
    /// Selected value or trimmed custom text.
    value: String,
    /// Visible selected label or custom text.
    label: String,
    /// Whether the user wrote this answer.
    was_custom: bool,
    /// One-based option position for a selected answer.
    #[serde(skip_serializing_if = "Option::is_none")]
    index: Option<usize>,
}
/// Completed or cancelled questionnaire, including partial answers.
#[derive(Clone, Deserialize, Serialize)]
struct QuestionnaireResult {
    /// Normalized questions.
    questions: Vec<Question>,
    /// Answers in insertion order.
    answers: Vec<Answer>,
    /// Whether the user cancelled.
    cancelled: bool,
}
/// Tool arguments supplied by the caller.
#[derive(Deserialize)]
struct Params {
    /// Questions in tab order.
    questions: Vec<Question>,
}
/// Builds an ordinary tool result with structured questionnaire details.
fn result(text: String, details: &QuestionnaireResult) -> AgentToolResult {
    AgentToolResult {
        content: vec![UserBlock::Text(TextContent::new(text))],
        details: Presence::Present(json!(details).to_string().into()),
        terminate: Presence::Missing,
    }
}
/// Returns a cancelled result without opening a view.
fn error_result(message: &str) -> AgentToolResult {
    result(
        message.into(),
        &QuestionnaireResult {
            questions: vec![],
            answers: vec![],
            cancelled: true,
        },
    )
}
/// Asks through the host UI, or returns the headless/empty-question result.
async fn execute(
    api: ExtensionAPI,
    params: Value,
    ctx: ExtensionContext,
) -> ExtensionResult<AgentToolResult> {
    if !ctx.has_ui()? {
        return Ok(error_result(
            "Error: UI not available (running in non-interactive mode)",
        ));
    }
    let mut questions = serde_json::from_value::<Params>(params)
        .map_err(|e| e.to_string())?
        .questions;
    if questions.is_empty() {
        return Ok(error_result("Error: No questions provided"));
    }
    for (index, question) in questions.iter_mut().enumerate() {
        if question.label.is_empty() {
            question.label = format!("Q{}", index + 1);
        }
    }
    let details = interface::ask(api, questions, &ctx).await?;
    let text = if details.cancelled {
        "User cancelled the questionnaire".into()
    } else {
        details
            .answers
            .iter()
            .map(|answer| {
                let label = details
                    .questions
                    .iter()
                    .find(|q| q.id == answer.id)
                    .map_or(answer.id.as_str(), |q| q.label.as_str());
                if answer.was_custom {
                    format!("{label}: user wrote: {}", answer.label)
                } else {
                    format!(
                        "{label}: user selected: {}. {}",
                        answer.index.unwrap_or(0),
                        answer.label
                    )
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    Ok(result(text, &details))
}
/// The question and option schema exposed to the model.
fn parameters() -> Value {
    json!({"type":"object","required":["questions"],"properties":{"questions":{
    "type":"array","description":"Questions to ask the user","items":{
        "type":"object","required":["id","prompt","options"],"properties":{
            "id":{"type":"string","description":"Unique identifier for this question"},
            "label":{"type":"string","description":"Short contextual label for tab bar, e.g. 'Scope', 'Priority' (defaults to Q1, Q2)"},
            "prompt":{"type":"string","description":"The full question text to display"},
            "allowOther":{"type":"boolean","description":"Allow 'Type something' option (default: true)"},
            "options":{"type":"array","description":"Available options to choose from","items":{
                "type":"object","required":["value","label"],"properties":{
                    "value":{"type":"string","description":"The value returned when selected"},
                    "label":{"type":"string","description":"Display label for the option"},
                    "description":{"type":"string","description":"Optional description shown below label"}
                }}}
        }}}}})
}
/// Registers the questionnaire tool and its call/result renderers.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let execute_api = api.clone();
        let render_api = api.clone();
        api.register_tool(ToolDefinition {
            metadata: ToolMetadata {
                name: "questionnaire".into(), label: "Questionnaire".into(),
                description: "Ask the user one or more questions. Use for clarifying requirements, getting preferences, or confirming decisions. For single questions, shows a simple option list. For multiple questions, shows a tab-based interface.".into(),
                parameters: parameters().into(), prompt_snippet: Presence::Missing,
                prompt_guidelines: Presence::Missing, render_shell: Presence::Missing,
                execution_mode: Presence::Missing,
            },
            prepare_arguments: None,
            execute: Rc::new(move |_, params, _, _, ctx| Box::pin(execute(execute_api.clone(), params, ctx))),
            render_call: Some(Rc::new(move |args, theme, _, _| rendering::call(&render_api, args.as_ref(), &theme))),
            render_result: Some(Rc::new(move |result, _, theme, _, _| rendering::result(&result, &theme))),
        })
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);

//! A todo tool whose result snapshots follow the active session branch.
// maestro-component: MAESTRO_TODO_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use maestro_extensions_wasm::{
    AgentMessage, AgentToolResult, CommandOptions, ExtensionAPI, ExtensionFuture, ExtensionResult,
    Presence, SessionEntry, TextContent, ToolDefinition, ToolMetadata, UserBlock,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

#[path = "todo/view.rs"]
mod todo_view;

/// One item in a result snapshot.
#[derive(Clone, Deserialize, Serialize)]
struct Todo {
    /// Stable identifier until the list is cleared.
    id: f64,
    /// Authored text, without trimming.
    text: String,
    /// Whether the item has been completed.
    done: bool,
}

/// Complete branch state carried by every tool result, including errors.
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct TodoDetails {
    /// Action that produced this snapshot.
    action: String,
    /// Items in insertion order.
    todos: Vec<Todo>,
    /// Identifier assigned by the next add.
    next_id: f64,
    /// Action error, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}
impl Default for TodoDetails {
    fn default() -> Self {
        Self {
            action: "list".into(),
            todos: vec![],
            next_id: 1.0,
            error: None,
        }
    }
}

/// Executes one action and returns a detached snapshot for session history.
fn execute(state: &RefCell<TodoDetails>, params: &Value) -> ExtensionResult<AgentToolResult> {
    let mut state = state.borrow_mut();
    state.action = params["action"].as_str().unwrap_or_default().into();
    state.error = None;
    let text = match state.action.as_str() {
        "list" => {
            if state.todos.is_empty() {
                "No todos".into()
            } else {
                state
                    .todos
                    .iter()
                    .map(|todo| {
                        format!(
                            "[{}] #{}: {}",
                            if todo.done { "x" } else { " " },
                            todo.id,
                            todo.text
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
        "add" => add(&mut state, params["text"].as_str()),
        "toggle" => toggle(&mut state, params["id"].as_f64())?,
        "clear" => {
            let count = state.todos.len();
            state.todos.clear();
            state.next_id = 1.0;
            format!("Cleared {count} todos")
        }
        _ => {
            let action = state.action.clone();
            state.action = "list".into();
            state.error = Some(format!("unknown action: {action}"));
            format!("Unknown action: {action}")
        }
    };
    Ok(AgentToolResult {
        content: vec![UserBlock::Text(TextContent::new(text))],
        details: Presence::Present(
            serde_json::to_string(&*state)
                .map_err(|e| e.to_string())?
                .into(),
        ),
        terminate: Presence::Missing,
        usage: None,
    })
}

/// Adds nonempty text verbatim, leaving state unchanged when it is missing.
fn add(state: &mut TodoDetails, text: Option<&str>) -> String {
    let Some(text) = text.filter(|text| !text.is_empty()) else {
        state.error = Some("text required".into());
        return "Error: text required for add".into();
    };
    let id = state.next_id;
    state.todos.push(Todo {
        id,
        text: text.into(),
        done: false,
    });
    state.next_id += 1.0;
    format!("Added todo #{id}: {text}")
}

/// Toggles only an exactly matching identifier, including a second toggle back to open.
fn toggle(state: &mut TodoDetails, id: Option<f64>) -> ExtensionResult<String> {
    let Some(id) = id else {
        state.error = Some("id required".into());
        return Ok("Error: id required for toggle".into());
    };
    let id_text = maestro_json::compact_json(&json!(id)).map_err(|e| e.to_string())?;
    let Some(todo) = state
        .todos
        .iter_mut()
        .find(|todo| todo.id.partial_cmp(&id) == Some(std::cmp::Ordering::Equal))
    else {
        state.error = Some(format!("#{id_text} not found"));
        return Ok(format!("Todo #{id_text} not found"));
    };
    todo.done = !todo.done;
    Ok(format!(
        "Todo #{} {}",
        todo.id,
        if todo.done {
            "completed"
        } else {
            "uncompleted"
        }
    ))
}

/// Replays only this tool's snapshots on the branch the host currently supplies.
fn restore(
    ctx: &maestro_extensions_wasm::ExtensionContext,
    state: &RefCell<TodoDetails>,
) -> ExtensionResult<()> {
    let mut restored = TodoDetails::default();
    for entry in ctx.session_manager()?.get_branch(None)? {
        if let SessionEntry::Message(entry) = entry
            && let AgentMessage::Message(message) = entry.message
            && let maestro_extensions_wasm::Message::ToolResult(message) = *message
            && message.tool_name == "todo"
            && let Some(details) = message.details
        {
            restored = serde_json::from_value(details).map_err(|e| e.to_string())?;
        }
    }
    state.replace(restored);
    Ok(())
}

/// Registers the branch-aware tool, renderers and interactive list command.
///
/// # Errors
/// Returns host rejections or malformed retained snapshots.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let state = Rc::new(RefCell::new(TodoDetails::default()));
        for event in ["session_start", "session_tree"] {
            let state = state.clone();
            api.on(
                event,
                Rc::new(move |_, ctx| {
                    let result = restore(&ctx, &state);
                    Box::pin(std::future::ready(result.map(|()| None)))
                }),
            )?;
        }
        let tool_state = state.clone();
        api.register_tool(ToolDefinition {
            metadata: ToolMetadata {
                name: "todo".into(),
                label: "Todo".into(),
                description: "Manage a todo list. Actions: list, add (text), toggle (id), clear"
                    .into(),
                parameters: json!({"type":"object","required":["action"],"properties":{
                    "action":{"type":"string","enum":["list","add","toggle","clear"]},
                    "text":{"type":"string","description":"Todo text (for add)"},
                    "id":{"type":"number","description":"Todo ID (for toggle)"}
                }})
                .into(),
                prompt_snippet: Presence::Missing,
                prompt_guidelines: Presence::Missing,
                render_shell: Presence::Missing,
                execution_mode: Presence::Missing,
            },
            prepare_arguments: None,
            execute: Rc::new(move |_, params, _, _, _| {
                Box::pin(std::future::ready(execute(&tool_state, &params)))
            }),
            render_call: Some(Rc::new(|args, theme, _, _| {
                todo_view::render_call(args.as_ref(), &theme)
            })),
            render_result: Some(Rc::new(|result, options, theme, _, _| {
                todo_view::render_result(&result.result, options.expanded, &theme)
            })),
        })?;
        let view_api = api.clone();
        api.register_command(
            "todos",
            CommandOptions {
                description: Some("Show all todos on the current branch".into()),
                get_argument_completions: None,
                handler: Rc::new(move |_, ctx| {
                    Box::pin(todo_view::show(
                        view_api.clone(),
                        state.borrow().todos.clone(),
                        ctx,
                    ))
                }),
            },
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);

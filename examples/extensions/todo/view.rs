//! Tool rows and the interactive todo list, using the host's theme and terminal helpers.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use super::{Todo, TodoDetails};
use maestro_extensions_wasm::{
    AgentToolResult, CallbackEmitter, Component, ComponentView, CustomFactoryOutcome, ExtensionAPI,
    ExtensionCommandContext, ExtensionComponent, ExtensionResult, Text, Theme, UserBlock,
    keys::matches_key,
    utils::{TruncateOptions, truncate_to_width},
};
use serde_json::Value;
use std::rc::Rc;

/// Wraps styled text in the host's unpadded text widget.
fn text_component(text: &str) -> ExtensionResult<Component> {
    Ok((&Text::new(text, 0, 0, None)?).into())
}

/// Renders supplied call arguments without changing their text or numeric identifier.
pub(super) fn render_call(args: Option<&Value>, theme: &Theme) -> ExtensionResult<Component> {
    let mut text = theme.fg("toolTitle", &theme.bold("todo ")?)?;
    let action = args
        .and_then(|args| args["action"].as_str())
        .unwrap_or_default();
    text.push_str(&theme.fg("muted", action)?);
    if let Some(value) = args
        .and_then(|args| args["text"].as_str())
        .filter(|text| !text.is_empty())
    {
        text.push(' ');
        text.push_str(&theme.fg("dim", &format!("\"{value}\""))?);
    }
    if let Some(id) = args
        .and_then(|args| args.get("id"))
        .filter(|id| id.is_number())
    {
        let id = maestro_json::compact_json(id).map_err(|e| e.to_string())?;
        text.push(' ');
        text.push_str(&theme.fg("accent", &format!("#{id}"))?);
    }
    text_component(&text)
}

/// The first text content block, or empty text for a different first block.
fn first_text(result: &AgentToolResult) -> &str {
    match result.content.first() {
        Some(UserBlock::Text(text)) => &text.text,
        _ => "",
    }
}

/// Renders result snapshots, with at most five items in a collapsed list.
pub(super) fn render_result(
    result: &AgentToolResult,
    expanded: bool,
    theme: &Theme,
) -> ExtensionResult<Component> {
    let Some(details) = result.details.present() else {
        return text_component(first_text(result));
    };
    let details: TodoDetails = serde_json::from_str(details).map_err(|e| e.to_string())?;
    if let Some(error) = &details.error {
        return text_component(&theme.fg("error", &format!("Error: {error}"))?);
    }
    let text = match details.action.as_str() {
        "list" => list_text(&details.todos, expanded, theme)?,
        "add" => {
            let todo = details
                .todos
                .last()
                .ok_or("Added todo missing from result")?;
            format!(
                "{}{} {}",
                theme.fg("success", "✓ Added ")?,
                theme.fg("accent", &format!("#{}", todo.id))?,
                theme.fg("muted", &todo.text)?
            )
        }
        "toggle" => format!(
            "{}{}",
            theme.fg("success", "✓ ")?,
            theme.fg("muted", first_text(result))?
        ),
        "clear" => format!(
            "{}{}",
            theme.fg("success", "✓ ")?,
            theme.fg("muted", "Cleared all todos")?
        ),
        _ => return Err("Unknown todo result action".into()),
    };
    text_component(&text)
}

/// One themed check mark, identifier and item text.
fn item(todo: &Todo, theme: &Theme, open_role: &str) -> ExtensionResult<String> {
    let check = if todo.done {
        theme.fg("success", "✓")?
    } else {
        theme.fg("dim", "○")?
    };
    Ok(format!(
        "{check} {} {}",
        theme.fg("accent", &format!("#{}", todo.id))?,
        theme.fg(if todo.done { "dim" } else { open_role }, &todo.text)?
    ))
}

/// The tool result's count and ordered items, followed by its collapsed remainder.
fn list_text(todos: &[Todo], expanded: bool, theme: &Theme) -> ExtensionResult<String> {
    if todos.is_empty() {
        return theme.fg("dim", "No todos");
    }
    let mut text = theme.fg("muted", &format!("{} todo(s):", todos.len()))?;
    for todo in todos.iter().take(if expanded { todos.len() } else { 5 }) {
        text.push('\n');
        text.push_str(&item(todo, theme, "muted")?);
    }
    if !expanded && todos.len() > 5 {
        text.push('\n');
        text.push_str(&theme.fg("dim", &format!("... {} more", todos.len() - 5))?);
    }
    Ok(text)
}

/// Snapshot shown until Escape or Ctrl+C completes the custom view.
struct TodoList {
    /// Supplied snapshot, independent of subsequent tool calls.
    todos: Vec<Todo>,
    /// The host's theme handle.
    theme: Theme,
    /// Host operations for terminal width and key interpretation.
    api: ExtensionAPI,
    /// Completes this view.
    close: Rc<dyn Fn(()) -> ExtensionResult<()>>,
}
impl ComponentView for TodoList {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        let theme = &self.theme;
        let width = usize::try_from(width).map_err(|e| e.to_string())?;
        let header = format!(
            "{}{}{}",
            theme.fg("borderMuted", "───")?,
            theme.fg("accent", " Todos ")?,
            theme.fg("borderMuted", &"─".repeat(width.saturating_sub(10)))?
        );
        let mut rows = vec![String::new(), header, String::new()];
        if self.todos.is_empty() {
            rows.push(format!(
                "  {}",
                theme.fg("dim", "No todos yet. Ask the agent to add some!")?
            ));
        } else {
            let done = self.todos.iter().filter(|todo| todo.done).count();
            rows.push(format!(
                "  {}",
                theme.fg("muted", &format!("{done}/{} completed", self.todos.len()))?
            ));
            rows.push(String::new());
            for todo in &self.todos {
                rows.push(format!("  {}", item(todo, theme, "text")?));
            }
        }
        rows.extend([
            String::new(),
            format!("  {}", theme.fg("dim", "Press Escape to close")?),
            String::new(),
        ]);
        rows.into_iter()
            .map(|row| {
                if row.is_empty() {
                    Ok(row)
                } else {
                    truncate_to_width(&self.api, &row, width, TruncateOptions::default())
                }
            })
            .collect()
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
    fn handle_input(&self, data: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        if matches_key(&self.api, data, "escape")? || matches_key(&self.api, data, "ctrl+c")? {
            (self.close)(())?;
        }
        Ok(())
    }
}

/// Opens the current snapshot, or reports that the command needs an interactive host.
pub(super) async fn show(
    api: ExtensionAPI,
    todos: Vec<Todo>,
    ctx: ExtensionCommandContext,
) -> ExtensionResult<()> {
    if !ctx.has_ui()? {
        return ctx.ui()?.notify(
            "/todos requires interactive mode",
            Some(maestro_extensions_wasm::NotificationType::Error),
        );
    }
    ctx.ui()?
        .custom(
            Rc::new(move |_, theme, _, done, _| {
                Ok(CustomFactoryOutcome::Ready(ExtensionComponent {
                    component: Component::new(TodoList {
                        todos: todos.clone(),
                        theme,
                        api: api.clone(),
                        close: done,
                    }),
                    dispose: None,
                    set_expanded: None,
                    width: None,
                }))
            }),
            None,
        )
        .await?;
    Ok(())
}

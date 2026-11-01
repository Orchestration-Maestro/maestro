//! Transcript replay from the session context, pairing tool results with their calls.

use super::InteractiveMode;
use crate::{
    AssistantMessageComponent, ToolExecutionComponent, ToolExecutionOptions, ToolExecutionParams,
    ToolExecutionResult,
};
use maestro_app::agent_session::ToolsManager;
use maestro_app::{
    EventError, ReadonlySessionManager, SessionContext, TextContent, ToolName, ToolRendererBinding,
    ToolResultMessage, ToolsOptions, UserBlock, create_all_tool_definitions,
};
use maestro_tui::Component;
use serde_json::Value;
use std::{collections::HashMap, io, rc::Rc, sync::Arc};

/// The pending tool rows of replay or live events by call id, in call order. Pairing a
/// result retires its slot; reclaiming at half occupancy keeps total pairing work linear.
#[derive(Default)]
pub(super) struct PendingTools {
    /// The slot of each pending call id.
    slots: HashMap<String, usize>,
    /// The rows in call order; a paired row leaves its slot empty.
    rows: Vec<Option<(String, Rc<ToolExecutionComponent>)>>,
}

impl PendingTools {
    /// Registers `row` for call `id`; a pending row of the same id is replaced in its place,
    /// while a paired id registers again after the other rows.
    pub(super) fn insert(&mut self, id: String, row: Rc<ToolExecutionComponent>) {
        let pending = self
            .slots
            .get(&id)
            .and_then(|&slot| self.rows.get_mut(slot));
        if let Some(Some((_, pending))) = pending {
            *pending = row;
        } else {
            self.slots.insert(id.clone(), self.rows.len());
            self.rows.push(Some((id, row)));
        }
    }

    /// Removes and returns the pending row of call `id`.
    pub(super) fn remove(&mut self, id: &str) -> Option<Rc<ToolExecutionComponent>> {
        let slot = self.slots.remove(id)?;
        let (_, row) = self.rows.get_mut(slot)?.take()?;
        self.compact();
        Some(row)
    }

    /// Reclaims retired slots once at least half are retired, bounding later traversal
    /// work by the pending rows and spreading reclamation work over completed calls.
    fn compact(&mut self) {
        let live = self.slots.len();
        if live > self.rows.len() - live {
            return;
        }
        let mut index = 0;
        self.rows.retain(|entry| {
            let Some((id, _)) = entry else {
                return false;
            };
            if let Some(slot) = self.slots.get_mut(id) {
                *slot = index;
            }
            index += 1;
            true
        });
    }

    /// The pending row of call `id`.
    pub(super) fn get(&self, id: &str) -> Option<&Rc<ToolExecutionComponent>> {
        let slot = *self.slots.get(id)?;
        self.rows.get(slot)?.as_ref().map(|(_, row)| row)
    }

    /// The pending rows in call order.
    pub(super) fn values(&self) -> impl Iterator<Item = &Rc<ToolExecutionComponent>> {
        self.rows.iter().flatten().map(|(_, row)| row)
    }

    /// The pending call ids in call order.
    #[cfg(all(test, unix))]
    pub(super) fn keys(&self) -> impl Iterator<Item = &String> {
        self.rows.iter().flatten().map(|(id, _)| id)
    }
}

/// The effects a replay has besides the transcript rows.
#[derive(Clone, Copy, Default)]
pub(super) struct RenderSessionContextOptions {
    /// Refreshes the footer and the editor border.
    pub(super) update_footer: bool,
    /// Adds user messages to the editor history.
    pub(super) populate_history: bool,
}

impl InteractiveMode {
    /// Replays the current session into the transcript, refreshing the footer and border and
    /// filling the editor history, then reports how often the session was compacted.
    ///
    /// # Errors
    /// Returns the failure of reading the session context, of a stored record, of a row's
    /// rendering or of the published theme.
    pub fn render_initial_messages(&self) -> Result<(), EventError> {
        let session = self.runtime_host.session();
        let context = session.session_manager().borrow().build_session_context()?;
        let options = RenderSessionContextOptions {
            update_footer: true,
            populate_history: true,
        };
        self.render_session_context(context, options)?;
        let compactions = session
            .session_manager()
            .borrow()
            .get_entries()
            .iter()
            .filter(|entry| entry.get("type").and_then(Value::as_str) == Some("compaction"))
            .count();
        if compactions > 0 {
            let times = if compactions == 1 { "time" } else { "times" };
            self.show_status(&format!("Session compacted {compactions} {times}"))?;
        }
        Ok(())
    }

    /// Clears the transcript and replays the current session without other effects.
    ///
    /// # Errors
    /// Returns the failure of reading the session context, of a stored record or of a row's
    /// rendering.
    pub(super) fn rebuild_chat_from_messages(&self) -> Result<(), EventError> {
        self.chat_container.clear();
        let session = self.runtime_host.session();
        let context = session.session_manager().borrow().build_session_context()?;
        self.render_session_context(context, RenderSessionContextOptions::default())
    }

    /// Appends the rows of every context message: assistant records with a row per tool call,
    /// each result completing the latest pending row of its call id. Calls still pending at
    /// the end stay registered for live completion; earlier registrations are dropped.
    ///
    /// # Errors
    /// Returns the failure of a stored record, of a row's rendering or of the published theme.
    pub(super) fn render_session_context(
        &self,
        context: SessionContext,
        options: RenderSessionContextOptions,
    ) -> Result<(), EventError> {
        drop(self.pending_tools.take());
        let mut rendered = PendingTools::default();
        if options.update_footer {
            self.footer.invalidate();
            self.update_editor_border_color()?;
        }
        for record in context.messages {
            match record.get("role").and_then(Value::as_str) {
                Some("assistant") => self.render_assistant(&Rc::new(record), &mut rendered)?,
                Some("toolResult") => complete_tool_call(record, &mut rendered)?,
                _ => self.add_message_to_chat(
                    &serde_json::from_value(record)?,
                    options.populate_history,
                )?,
            }
        }
        self.pending_tools.replace(rendered);
        self.ui.request_render(false);
        Ok(())
    }

    /// Appends a stored assistant record and a row per tool call; a failed message completes
    /// its rows with the failure, otherwise they wait in `rendered` by call id.
    ///
    /// # Errors
    /// Returns the failure of a malformed tool call or of a row's rendering.
    fn render_assistant(
        &self,
        record: &Rc<Value>,
        rendered: &mut PendingTools,
    ) -> Result<(), EventError> {
        let component = AssistantMessageComponent::from_record(
            Rc::clone(record),
            self.assistant_options(),
            self.themes.theme(),
            self.get_markdown_theme_with_settings(),
            self.tool_environment.terminal_image.clone(),
        );
        self.chat_container.add_child(Rc::new(component));
        let failure = self.failure_text(record);
        let calls = record.get("content").and_then(Value::as_array);
        let calls = calls.into_iter().flatten();
        for call in
            calls.filter(|block| block.get("type").and_then(Value::as_str) == Some("toolCall"))
        {
            let (id, name) = (call_field(call, "id")?, call_field(call, "name")?);
            let row = self.render_tool_call(id, name, call.get("arguments").cloned())?;
            match &failure {
                Some(text) => row.update_result(failure_result(text), false)?,
                None => rendered.insert(id.to_owned(), row),
            }
        }
        Ok(())
    }

    /// The result text of the tool calls of an aborted or failed message.
    fn failure_text(&self, record: &Value) -> Option<String> {
        match record.get("stopReason").and_then(Value::as_str)? {
            "aborted" => Some(aborted_notice(self.runtime_host.session().retry_attempt())),
            "error" => Some(error_notice(
                record.get("errorMessage").and_then(Value::as_str),
            )),
            _ => None,
        }
    }

    /// Appends a row for tool call `id` of `name` with its raw arguments, the configured image
    /// policies, the session's registered renderers and the built-in ones of its name.
    ///
    /// # Errors
    /// Returns the failure of a renderer binding or of the row's rendering.
    pub(super) fn render_tool_call(
        &self,
        id: &str,
        name: &str,
        args: Option<Value>,
    ) -> Result<Rc<ToolExecutionComponent>, EventError> {
        let session = self.runtime_host.session();
        let options = {
            let settings = session.settings_manager().borrow();
            ToolExecutionOptions {
                show_images: settings.get_show_images(),
                image_width_cells: settings.get_image_width_cells(),
            }
        };
        let cwd = session.session_manager().borrow().get_cwd().to_owned();
        let row = Rc::new(ToolExecutionComponent::new(ToolExecutionParams {
            tool_definition: self.get_registered_tool_definition(name)?,
            built_in_tool_definition: built_in_tool_definition(name, &cwd)?,
            tool_name: name.to_owned(),
            tool_call_id: id.to_owned(),
            args,
            options,
            environment: Rc::clone(&self.tool_environment),
            cwd,
            request_render: self.ui.weak_render_request(),
        })?);
        row.set_expanded(self.tool_output_expanded.get())?;
        self.chat_container.add_child(row.clone());
        Ok(row)
    }

    /// The renderers of the session's registered tool `name`, for a row's custom rendering.
    ///
    /// # Errors
    /// Returns the failure of binding the registered renderers.
    fn get_registered_tool_definition(
        &self,
        name: &str,
    ) -> Result<Option<ToolRendererBinding>, maestro_app::DiagnosticErrorInfo> {
        self.runtime_host
            .session()
            .get_tool_definition(name)
            .map(|tool| tool.renderer_binding())
            .transpose()
    }
}

/// Completes the pending row of a stored result's call id, if any, with the result.
///
/// # Errors
/// Returns the failure of a malformed result record or of the row's rendering.
fn complete_tool_call(record: Value, rendered: &mut PendingTools) -> Result<(), EventError> {
    let id = record.get("toolCallId").and_then(Value::as_str);
    let Some(row) = id.and_then(|id| rendered.remove(id)) else {
        return Ok(());
    };
    let result: ToolResultMessage = serde_json::from_value(record)?;
    let result = ToolExecutionResult {
        content: result.content,
        details: result.details,
        is_error: result.is_error,
    };
    Ok(row.update_result(result, false)?)
}

/// The string field `key` of a stored tool call.
///
/// # Errors
/// Returns the field as invalid data when it is missing or not a string.
fn call_field<'a>(call: &'a Value, key: &str) -> io::Result<&'a str> {
    call.get(key).and_then(Value::as_str).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("tool call {key} is not a string"),
        )
    })
}

/// The failed result a tool call of a failed message shows: `text` as its only block.
pub(super) fn failure_result(text: &str) -> ToolExecutionResult {
    ToolExecutionResult {
        content: vec![UserBlock::Text(TextContent::new(text.to_owned()))],
        details: None,
        is_error: true,
    }
}

/// The result text of a failed message's tool calls: its nonempty error, else `Error`.
pub(super) fn error_notice(error: Option<&str>) -> String {
    error
        .filter(|text| !text.is_empty())
        .unwrap_or("Error")
        .to_owned()
}

/// The result text of an aborted message's tool calls after `attempts` retries.
pub(super) fn aborted_notice(attempts: u32) -> String {
    match attempts {
        0 => "Operation aborted".to_owned(),
        1 => "Aborted after 1 retry attempt".to_owned(),
        attempts => format!("Aborted after {attempts} retry attempts"),
    }
}

/// The renderers of the built-in tool `name` in `cwd`, when it names one.
///
/// # Errors
/// Returns the failure of binding the built-in renderers.
fn built_in_tool_definition(
    name: &str,
    cwd: &str,
) -> Result<Option<ToolRendererBinding>, maestro_app::DiagnosticErrorInfo> {
    let Ok(name) = name.parse::<ToolName>() else {
        return Ok(None);
    };
    let tools = Arc::new(ToolsManager::new(String::new(), None));
    let definitions = create_all_tool_definitions::<()>(
        cwd.to_owned(),
        ToolsOptions::default(),
        String::new(),
        tools,
    );
    definitions
        .get(&name)
        .map(maestro_app::ToolDef::renderer_binding)
        .transpose()
}

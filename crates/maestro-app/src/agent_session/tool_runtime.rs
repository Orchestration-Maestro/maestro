//! The agent's tool hooks and the built-in tool definitions the runtime builds on.
use super::state::thrown;
use super::{AgentSession, ToolDefinitionHandle};
use crate::utils::lock::{read, write};
use indexmap::IndexMap;
use maestro_agent::{
    AfterToolCallContext, AfterToolCallResult, Agent, AgentContextRecord, AgentLoopTurnUpdate,
    AgentToolRecord, BeforeToolCallContext, BeforeToolCallResult, PrepareNextTurnContext,
    SharedAgentTool,
};
use maestro_extensions::{ExtensionExecution, RegisteredToolInput, ToolCallEvent, ToolResultEvent};
use maestro_models::{DiagnosticErrorInfo, Local};
use maestro_session::{Presence, messages::CustomAgentMessages};
use maestro_settings::SettingsManager;
use maestro_tools::{ToolsManager, local::ToolDef};
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::{Arc, RwLock};

/// Installs the agent's tool hooks once; each reads the session's current runner when it
/// runs, so a rebuilt runtime needs no new hooks.
pub(super) fn install_agent_tool_hooks(
    agent: &Agent<CustomAgentMessages, Local>,
    session: &Weak<AgentSession<Local>>,
) {
    let before = Weak::clone(session);
    *write(&agent.before_tool_call) = Some(Rc::new(move |context, _signal| {
        Box::pin(before_tool_call(before.upgrade(), context))
    }));
    let next = Weak::clone(session);
    *write(&agent.prepare_next_turn) = Some(Rc::new(move |turn| {
        Box::pin(std::future::ready(Ok(prepare_next_turn(
            next.upgrade(),
            &turn,
        ))))
    }));
    let after = Weak::clone(session);
    *write(&agent.after_tool_call) = Some(Rc::new(move |context, _signal| {
        Box::pin(after_tool_call(after.upgrade(), context))
    }));
}

/// The context, model and thinking level the session's current state gives the next
/// request: the run's system prompt, the active tools and the selected model.
fn prepare_next_turn(
    session: Option<Rc<AgentSession<Local>>>,
    turn: &PrepareNextTurnContext<'_, CustomAgentMessages, Local>,
) -> Option<AgentLoopTurnUpdate<CustomAgentMessages, Local>> {
    let session = session?;
    let system_prompt = session.state.borrow().effective_system_prompt();
    let state = read(session.agent.state());
    let tools = read(state.tools()).clone();
    Some(AgentLoopTurnUpdate {
        context: Some(AgentContextRecord {
            system_prompt,
            messages: Arc::clone(&turn.context.messages),
            tools: Some(Rc::new(RwLock::new(tools))),
        }),
        model: Some(Arc::clone(&state.model)),
        thinking_level: Some(state.thinking_level.clone()),
    })
}

/// Lets the extensions edit or block a tool call once the queued session events are
/// processed; the edited arguments are the ones executed.
async fn before_tool_call(
    session: Option<Rc<AgentSession<Local>>>,
    context: BeforeToolCallContext<'_, CustomAgentMessages, Local>,
) -> Result<Option<BeforeToolCallResult>, DiagnosticErrorInfo> {
    let Some(session) = session else {
        return Ok(None);
    };
    let runner = session.extension_runner();
    if !runner.has_handlers("tool_call") {
        return Ok(None);
    }
    if let Some(queue) = session.agent_event_queue() {
        drop(queue.await);
    }
    let mut event = ToolCallEvent {
        tool_call_id: context.tool_call.id.clone(),
        tool_name: context.tool_call.name.clone(),
        input: context.args.clone(),
    };
    let result = runner.emit_tool_call(&mut event).await;
    *context.args = event.input;
    Ok(result.map_err(thrown)?.map(|result| BeforeToolCallResult {
        block: matches!(result.block, Presence::Present(true)),
        reason: result.reason.present().cloned(),
    }))
}

/// Lets the extensions replace a tool result's content, details, error flag and usage; the
/// continuation hint stays the tool's own.
async fn after_tool_call(
    session: Option<Rc<AgentSession<Local>>>,
    context: AfterToolCallContext<'_, CustomAgentMessages, Local>,
) -> Result<Option<maestro_agent::AfterToolCallResult>, DiagnosticErrorInfo> {
    let Some(session) = session else {
        return Ok(None);
    };
    let runner = session.extension_runner();
    if !runner.has_handlers("tool_result") {
        return Ok(None);
    }
    let event = ToolResultEvent {
        tool_call_id: context.tool_call.id.clone(),
        tool_name: context.tool_call.name.clone(),
        input: context.args.clone(),
        content: context.result.content.clone(),
        is_error: context.is_error,
        details: context
            .result
            .details
            .clone()
            .map_or(Presence::Missing, Presence::Present),
        usage: context
            .result
            .usage
            .clone()
            .map_or(Presence::Missing, Presence::Present),
    };
    let Some(result) = runner.emit_tool_result(&event).await.map_err(thrown)? else {
        return Ok(None);
    };
    Ok(Some(AfterToolCallResult {
        content: result.content.present().cloned(),
        details: match result.details {
            Presence::Present(details) => Some(details),
            Presence::Null => Some(serde_json::Value::Null),
            Presence::Missing => None,
        },
        is_error: Some(
            result
                .is_error
                .present()
                .copied()
                .unwrap_or(context.is_error),
        ),
        terminate: None,
        usage: result.usage.present().cloned(),
    }))
}

/// The built-in definitions by name: the override tools as definitions, else every built-in
/// tool configured from the settings.
///
/// # Errors
/// Returns an override tool's description failure.
pub(super) fn base_tool_definitions(
    settings: &RefCell<SettingsManager>,
    cwd: &str,
    bin_dir: &str,
    tools_manager: &Arc<ToolsManager>,
    base_tools_override: Option<&IndexMap<String, SharedAgentTool<Local>>>,
) -> Result<IndexMap<String, ToolDefinitionHandle<Local>>, DiagnosticErrorInfo> {
    if let Some(tools) = base_tools_override {
        return tools
            .iter()
            .map(|(name, tool)| {
                let copied = copy_tool(&read(tool));
                let definition =
                    maestro_tools::local::create_tool_definition_from_agent_tool(copied)?;
                Ok((name.clone(), retain(definition)))
            })
            .collect();
    }
    let settings = settings.borrow();
    let options = maestro_tools::local::ToolsOptions {
        read: maestro_tools::ReadToolOptions {
            auto_resize_images: Some(settings.get_image_auto_resize()),
            ..maestro_tools::ReadToolOptions::default()
        },
        bash: maestro_tools::local::BashToolOptions {
            command_prefix: settings.get_shell_command_prefix(),
            shell_path: settings.get_shell_path(),
            ..maestro_tools::local::BashToolOptions::default()
        },
        ..maestro_tools::local::ToolsOptions::default()
    };
    drop(settings);
    let definitions = maestro_tools::local::create_all_tool_definitions(
        cwd.to_owned(),
        options,
        bin_dir.to_owned(),
        Arc::clone(tools_manager),
    );
    Ok(definitions
        .into_iter()
        .map(|(name, definition)| (name.to_string(), retain_built_in(definition)))
        .collect())
}

/// A built-in definition retained like an extension registration.
fn retain_built_in(
    definition: ToolDef<maestro_extensions::ExtensionContext>,
) -> ToolDefinitionHandle<Local> {
    match definition {
        ToolDef::Read(definition) => retain_raw(definition),
        ToolDef::Bash(definition) => retain_raw(definition),
        ToolDef::Edit(definition) => retain_raw(definition),
        ToolDef::Write(definition) => retain_raw(definition),
        ToolDef::Grep(definition) => retain_raw(definition),
        ToolDef::Find(definition) => retain_raw(definition),
        ToolDef::Ls(definition) => retain_raw(definition),
    }
}

/// A definition retained like an extension registration.
fn retain<P, D, S>(
    definition: impl Into<RegisteredToolInput<P, D, S, Local>>,
) -> ToolDefinitionHandle<Local>
where
    P: serde::de::DeserializeOwned + 'static,
    D: serde::Serialize + 'static,
    S: Default + 'static,
{
    Local::retain_definition(definition.into())
}

/// A built-in definition retained with its factory renderers, whose details need no decoder.
fn retain_raw<P, D, S>(
    definition: maestro_tools::local::ToolDefinition<maestro_extensions::ExtensionContext, P, D, S>,
) -> ToolDefinitionHandle<Local>
where
    P: serde::de::DeserializeOwned + 'static,
    D: serde::Serialize + 'static,
    S: Default + 'static,
{
    Local::retain_definition(RegisteredToolInput::from_raw(definition))
}

/// A second entry over the same callbacks as `tool`.
fn copy_tool(tool: &AgentToolRecord<Local>) -> AgentToolRecord<Local> {
    AgentToolRecord {
        definition: tool.definition.clone(),
        description: tool.description.clone(),
        label: tool.label.clone(),
        prepare_arguments: tool.prepare_arguments.clone(),
        execute: Rc::clone(&tool.execute),
        execution_mode: tool.execution_mode,
    }
}

//! Dynamic tools: one echo tool registered when a session starts, and a command that adds more
//! echo tools while the session runs.
//!
//! `/add-echo-tool <tool_name>` normalizes the supplied name to lowercase ASCII letters, digits
//! and underscores, then registers `Echo <name>`, which echoes its message behind `[<name>] `.
//! A name is admitted once per factory instance, so a repeated spelling only reports that the
//! tool is already registered.
// maestro-component: MAESTRO_DYNAMIC_TOOLS_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::{cell::RefCell, collections::HashSet, rc::Rc};

use maestro_extensions_wasm::{
    AgentToolResult, CommandOptions, ExtensionAPI, ExtensionCommandContext, ExtensionContext,
    ExtensionFuture, ExtensionResult, NotificationType, Presence, TextContent, ToolDefinition,
    ToolMetadata, UserBlock,
};
use serde::Deserialize;
use serde_json::{Value, json};

/// The message required by each echo tool's schema.
#[derive(Deserialize)]
struct EchoParams {
    /// Text returned without changing its whitespace or contents.
    message: String,
}

/// Normalizes an admitted tool name and rejects other argument text.
fn normalize_tool_name(input: &str) -> Option<String> {
    let name = input.trim().to_lowercase();
    (!name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'))
    .then_some(name)
}

/// Returns the supplied message with this registered tool's prefix.
fn echo_result(params: Value, name: &str, prefix: &str) -> ExtensionResult<AgentToolResult> {
    let params: EchoParams = serde_json::from_value(params).map_err(|error| error.to_string())?;
    Ok(AgentToolResult {
        content: vec![UserBlock::Text(TextContent::new(format!(
            "{prefix}{}",
            params.message
        )))],
        details: Presence::Present(json!({"tool":name,"prefix":prefix}).to_string().into()),
        terminate: Presence::Missing,
    })
}

/// Admits each name once before requesting its registration from the host.
fn register_echo_tool(
    api: &ExtensionAPI,
    names: &RefCell<HashSet<String>>,
    name: &str,
    label: &str,
    prefix: &str,
) -> ExtensionResult<bool> {
    if !names.borrow_mut().insert(name.to_owned()) {
        return Ok(false);
    }
    let tool_name = name.to_owned();
    let tool_prefix = prefix.to_owned();
    api.register_tool(ToolDefinition {
        metadata: ToolMetadata {
            name: name.into(),
            label: label.into(),
            description: format!("Echo a message with prefix: {prefix}"),
            prompt_snippet: Presence::Present(format!(
                "Echo back user-provided text with {} prefix",
                prefix.trim()
            )),
            prompt_guidelines: Presence::Present(vec![
                "Use echo_session when the user asks for exact echo output.".into(),
            ]),
            parameters: json!({"type":"object","properties":{
                "message":{"description":"Message to echo","type":"string"}
            },"required":["message"]})
            .into(),
            render_shell: Presence::Missing,
            execution_mode: Presence::Missing,
        },
        prepare_arguments: None,
        render_call: None,
        render_result: None,
        execute: Rc::new(move |_, params, _, _, _| {
            Box::pin(std::future::ready(echo_result(
                params,
                &tool_name,
                &tool_prefix,
            )))
        }),
    })?;
    Ok(true)
}

/// Requests the initial session tool and reports it even if its name was already retained.
fn session_start(
    api: &ExtensionAPI,
    names: &RefCell<HashSet<String>>,
    ctx: &ExtensionContext,
) -> ExtensionResult<()> {
    register_echo_tool(api, names, "echo_session", "Echo Session", "[session] ")?;
    ctx.ui()?.notify(
        "Registered dynamic tool: echo_session",
        Some(NotificationType::Info),
    )
}

/// Adds the normalized command name or reports its invalid or duplicate outcome.
fn add_echo_tool(
    api: &ExtensionAPI,
    names: &RefCell<HashSet<String>>,
    args: &str,
    ctx: &ExtensionCommandContext,
) -> ExtensionResult<()> {
    let Some(name) = normalize_tool_name(args) else {
        return ctx.ui()?.notify(
            "Usage: /add-echo-tool <tool_name> (lowercase, numbers, underscores)",
            Some(NotificationType::Warning),
        );
    };
    let created = register_echo_tool(
        api,
        names,
        &name,
        &format!("Echo {name}"),
        &format!("[{name}] "),
    )?;
    let (message, kind) = if created {
        (
            format!("Registered dynamic tool: {name}"),
            NotificationType::Info,
        )
    } else {
        (
            format!("Tool already registered: {name}"),
            NotificationType::Warning,
        )
    };
    ctx.ui()?.notify(&message, Some(kind))
}

/// Registers the session hook and the command on the supplied extension host.
#[must_use]
pub fn dynamic_tools_extension(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let names = Rc::new(RefCell::new(HashSet::new()));
        let event_names = Rc::clone(&names);
        let event_api = api.clone();
        api.on(
            "session_start",
            Rc::new(move |_, ctx| {
                Box::pin(std::future::ready(
                    session_start(&event_api, &event_names, &ctx).map(|()| None),
                ))
            }),
        )?;
        let command_api = api.clone();
        api.register_command(
            "add-echo-tool",
            CommandOptions {
                description: Some(
                    "Register a new echo tool dynamically: /add-echo-tool <tool_name>".into(),
                ),
                get_argument_completions: None,
                handler: Rc::new(move |args, ctx| {
                    Box::pin(std::future::ready(add_echo_tool(
                        &command_api,
                        &names,
                        &args,
                        &ctx,
                    )))
                }),
            },
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(dynamic_tools_extension);

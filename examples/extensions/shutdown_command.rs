//! A quit command and two tools that ask the host to shut down.
// maestro-component: MAESTRO_SHUTDOWN_COMMAND_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use maestro_extensions_wasm::{
    AgentToolResult, CommandOptions, ExtensionAPI, ExtensionFuture, Presence, TextContent,
    ToolDefinition, ToolMetadata, UserBlock,
};
use serde::Deserialize;
use serde_json::{Value, json};

/// The environment supplied to the deployment tool.
#[derive(Deserialize)]
struct DeployParams {
    /// Target environment, kept exactly as supplied.
    environment: String,
}

/// A result holding one text block and the given JSON details, without termination.
fn result(text: String, details: String) -> AgentToolResult {
    AgentToolResult {
        content: vec![UserBlock::Text(TextContent::new(text))],
        details: Presence::Present(details.into()),
        terminate: Presence::Missing,
    }
}

/// Registration metadata whose prompt, rendering and execution settings are absent.
fn tool_metadata(name: &str, label: &str, description: &str, parameters: &Value) -> ToolMetadata {
    ToolMetadata {
        name: name.into(),
        label: label.into(),
        description: description.into(),
        parameters: parameters.clone().into(),
        prompt_snippet: Presence::Missing,
        prompt_guidelines: Presence::Missing,
        render_shell: Presence::Missing,
        execution_mode: Presence::Missing,
    }
}

/// The `quit` command, which requests shutdown and ignores its arguments.
fn quit() -> CommandOptions {
    CommandOptions {
        description: Some("Exit Maestro cleanly".into()),
        handler: Rc::new(|_args, ctx| Box::pin(async move { ctx.shutdown() })),
        get_argument_completions: None,
    }
}

/// The `finish_and_exit` tool, which requests shutdown before answering.
fn finish_and_exit() -> ToolDefinition {
    ToolDefinition {
        render_call: None,
        render_result: None,
        metadata: tool_metadata(
            "finish_and_exit",
            "Finish and Exit",
            "Complete a task and exit Maestro",
            &json!({"type": "object", "properties": {}}),
        ),
        prepare_arguments: None,
        execute: Rc::new(|_, _params, _signal, _update, ctx| {
            Box::pin(async move {
                ctx.shutdown()?;
                Ok(result(
                    "Shutdown requested. Exiting after this response.".into(),
                    "{}".into(),
                ))
            })
        }),
    }
}

/// The `deploy_and_exit` tool, which reports progress and then requests shutdown.
fn deploy_and_exit() -> ToolDefinition {
    ToolDefinition {
        render_call: None,
        render_result: None,
        metadata: tool_metadata(
            "deploy_and_exit",
            "Deploy and Exit",
            "Deploy the application and exit Maestro",
            &json!({
                "type": "object",
                "required": ["environment"],
                "properties": {
                    "environment": {
                        "type": "string",
                        "description": "Target environment (e.g., production, staging)"
                    }
                }
            }),
        ),
        prepare_arguments: None,
        execute: Rc::new(|_, params, _signal, update, ctx| {
            Box::pin(async move {
                let params: DeployParams =
                    serde_json::from_value(params).map_err(|error| error.to_string())?;
                if let Some(update) = update {
                    update(result(
                        format!("Deploying to {}...", params.environment),
                        "{}".into(),
                    ))?;
                    update(result(
                        "Deployment complete, exiting...".into(),
                        "{}".into(),
                    ))?;
                }
                ctx.shutdown()?;
                Ok(result(
                    "Done! Shutdown requested.".into(),
                    json!({"environment": params.environment}).to_string(),
                ))
            })
        }),
    }
}

/// Registers the quit command and both tools, stopping at the first refusal.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        api.register_command("quit", quit())?;
        api.register_tool(finish_and_exit())?;
        api.register_tool(deploy_and_exit())
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);

//! A command that reloads runtime resources and a tool that queues that command.
// maestro-component: MAESTRO_RELOAD_RUNTIME_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use maestro_extensions_wasm::{
    AgentToolResult, CommandOptions, ExtensionAPI, ExtensionFuture, Presence,
    SendUserMessageOptions, TextContent, ToolDefinition, ToolMetadata, UserBlock, UserContent,
    UserMessageDelivery,
};

/// A tool that requests the command through follow-up delivery.
fn reload_runtime_tool(api: ExtensionAPI) -> ToolDefinition {
    ToolDefinition {
        render_call: None,
        render_result: None,
        metadata: ToolMetadata {
            name: "reload_runtime".into(),
            label: "Reload Runtime".into(),
            description: "Reload extensions, skills, prompts, and themes".into(),
            parameters: serde_json::json!({"type":"object","properties":{}}).into(),
            prompt_snippet: Presence::Missing,
            prompt_guidelines: Presence::Missing,
            render_shell: Presence::Missing,
            execution_mode: Presence::Missing,
        },
        prepare_arguments: None,
        execute: Rc::new(move |_, _, _, _, _| {
            let api = api.clone();
            Box::pin(async move {
                api.send_user_message(
                    UserContent::Text("/reload-runtime".into()),
                    Some(SendUserMessageOptions {
                        deliver_as: Some(UserMessageDelivery::FollowUp),
                    }),
                )?;
                Ok(AgentToolResult {
                    content: vec![UserBlock::Text(TextContent::new(
                        "Queued /reload-runtime as a follow-up command.".into(),
                    ))],
                    details: Presence::Present(String::from("{}").into()),
                    terminate: Presence::Missing,
                    usage: None,
                })
            })
        }),
    }
}

/// Registers the reload command and its follow-up tool.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        api.register_command(
            "reload-runtime",
            CommandOptions {
                description: Some("Reload extensions, skills, prompts, and themes".into()),
                get_argument_completions: None,
                handler: Rc::new(|_, ctx| Box::pin(async move { ctx.reload().await })),
            },
        )?;
        api.register_tool(reload_runtime_tool(api.clone()))
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);

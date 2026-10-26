//! Asynchronous callbacks whose prefix runs before their first suspension.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    CommandOptions, ExtensionAPI, ExtensionCommandContext, ExtensionFuture, ExtensionResult,
    ShortcutOptions,
};
use serde_json::json;
use std::rc::Rc;

/// Names the session around an optional caller-controlled native wait.
fn prefixed(
    api: ExtensionAPI,
    kind: &'static str,
    wait: Option<ExtensionCommandContext>,
) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        api.set_session_name(&format!("{kind}.prefix"))?;
        if let Some(wait) = wait {
            api.events()
                .emit("trace", json!(format!("{kind}.emitted")))
                .await?;
            wait.wait_for_idle().await?;
        }
        api.set_session_name(&format!("{kind}.tail"))
    })
}

/// Registers callbacks sharing the original command context's controlled wait.
pub(super) fn register(api: &ExtensionAPI, wait: ExtensionCommandContext) -> ExtensionResult<()> {
    let (command, completion, session) = (api.clone(), api.clone(), api.clone());
    let completion_wait = wait.clone();
    api.register_command(
        "prefix-command",
        CommandOptions {
            description: None,
            handler: Rc::new(move |args, context| {
                prefixed(
                    command.clone(),
                    "command",
                    (args != "ready").then_some(context),
                )
            }),
            get_argument_completions: Some(Rc::new(move |_| {
                let body = prefixed(
                    completion.clone(),
                    "completion",
                    Some(completion_wait.clone()),
                );
                Box::pin(async move { body.await.map(|()| None) })
            })),
        },
    )?;
    api.register_command(
        "prefix-session",
        CommandOptions {
            description: None,
            handler: Rc::new(move |_, context| new_session(session.clone(), context)),
            get_argument_completions: None,
        },
    )?;
    let (shortcut, shortcut_wait) = (api.clone(), wait.clone());
    api.register_shortcut(
        "ctrl+alt+p",
        ShortcutOptions {
            description: None,
            handler: Rc::new(move |_| {
                prefixed(shortcut.clone(), "shortcut", Some(shortcut_wait.clone()))
            }),
        },
    )?;
    super::registrations::shell(api)?;
    tool(api, wait)
}

/// Uses the same pending body through the real registered-tool wrapper.
fn tool(api: &ExtensionAPI, wait: ExtensionCommandContext) -> ExtensionResult<()> {
    let tool = api.clone();
    api.register_tool(maestro_extensions_wasm::ToolDefinition {
        render_call: None,
        render_result: None,
        metadata: serde_json::from_value(json!({"name": "prefix-tool", "label": "Prefix tool", "description": "Prefix tool", "parameters": {"type": "object"}})).map_err(|error| error.to_string())?,
        prepare_arguments: None,
        execute: Rc::new(move |_, _, _, _, _| {
            let body = prefixed(tool.clone(), "tool", Some(wait.clone()));
            Box::pin(async move {
                body.await?;
                serde_json::from_value(json!({"content": [], "details": {}}))
                    .map_err(|error| error.to_string())
            })
        }),
    })
}

/// Hands a prefixed setup and replacement continuation to the native session operation.
fn new_session(
    api: ExtensionAPI,
    context: ExtensionCommandContext,
) -> ExtensionFuture<'static, ()> {
    let (with_session, setup_wait, with_wait) = (api.clone(), context.clone(), context.clone());
    Box::pin(async move {
        context
            .new_session(Some(maestro_extensions_wasm::NewSessionCommandOptions {
                data: maestro_extensions_wasm::NewSessionCommandData {
                    parent_session: None,
                },
                setup: Some(Box::new(move |_| prefixed(api, "setup", Some(setup_wait)))),
                with_session: Some(Box::new(move |_| {
                    prefixed(with_session, "with-session", Some(with_wait))
                })),
            }))
            .await
            .map(|_| ())
    })
}

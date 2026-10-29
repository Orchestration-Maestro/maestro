//! A command that sets or shows the session's friendly name.
//!
//! The name identifies the session in the session selector in place of its first message.
//! Usage: `/session-name [name]` sets the name, and `/session-name` shows the name now in effect.
// maestro-component: MAESTRO_SESSION_NAME_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use maestro_extensions_wasm::{
    CommandOptions, ExtensionAPI, ExtensionCommandContext, ExtensionFuture, ExtensionResult,
    NotificationType,
};

/// Registers the command that sets or shows the session name.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let commands = api.clone();
        api.register_command(
            "session-name",
            CommandOptions {
                description: Some(
                    "Set or show session name (usage: /session-name [new name])".into(),
                ),
                get_argument_completions: None,
                handler: Rc::new(move |args, ctx| {
                    let api = commands.clone();
                    Box::pin(async move { run(&api, &args, &ctx) })
                }),
            },
        )
    })
}

/// Writes the trimmed name and announces it, or announces the name now in effect.
fn run(api: &ExtensionAPI, args: &str, ctx: &ExtensionCommandContext) -> ExtensionResult<()> {
    let name = args.trim();
    let message = if name.is_empty() {
        match api.get_session_name()? {
            Some(current) if !current.is_empty() => format!("Session: {current}"),
            _ => "No session name set".into(),
        }
    } else {
        api.set_session_name(name)?;
        format!("Session named: {name}")
    };
    ctx.ui()?.notify(&message, Some(NotificationType::Info))
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);

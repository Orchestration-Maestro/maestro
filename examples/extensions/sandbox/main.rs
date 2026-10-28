//! An opt-in filesystem and network sandbox for bash commands.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use maestro_extensions_wasm::{
    BashToolOptions, CommandOptions, ExtensionAPI, ExtensionFuture, FlagOptions, FlagType,
    FlagValue, NotificationType,
};

/// Registers the sandbox example.
///
/// # Errors
/// Returns the host's registration failure.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        api.register_flag(
            "no-sandbox",
            FlagOptions {
                description: Some("Disable OS-level sandboxing for bash commands".into()),
                type_: FlagType::Boolean,
                default: Some(FlagValue::Boolean(false)),
            },
        )?;
        let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
        let mut bash = api.create_bash_tool_definition(
            cwd.to_string_lossy().into_owned(),
            BashToolOptions::default(),
        )?;
        bash.metadata.label = "bash (sandboxed)".into();
        api.register_tool(bash)?;
        api.register_command(
            "sandbox",
            CommandOptions {
                description: Some("Show sandbox configuration".into()),
                get_argument_completions: None,
                handler: Rc::new(|_, ctx| {
                    Box::pin(async move {
                        ctx.ui()?
                            .notify("Sandbox is disabled", Some(NotificationType::Info))
                    })
                }),
            },
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);

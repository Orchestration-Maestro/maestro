//! An opt-in bash override using the host's native shell factory.
// maestro-component: MAESTRO_BASH_SPAWN_HOOK_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use maestro_extensions_wasm::{BashToolOptions, ExtensionAPI, ExtensionFuture};

/// Registers a profile-sourcing bash override with the extension's initial directory.
///
/// # Errors
///
/// Returns an error if the current directory cannot be read, the host cannot create
/// the bash tool definition, or tool registration fails.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
        let tool = api.create_bash_tool_definition(
            cwd.to_string_lossy().into_owned(),
            BashToolOptions {
                spawn_hook: Some(Rc::new(|mut spawn, _| {
                    spawn.command = format!("source ~/.profile\n{}", spawn.command);
                    spawn
                        .env
                        .insert("MAESTRO_SPAWN_HOOK".into(), Some("1".into()));
                    Ok(spawn)
                })),
                ..BashToolOptions::default()
            },
        )?;
        api.register_tool(tool)
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);

//! An opt-in bash override using the host's native shell factory.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use maestro_extensions_wasm::{BashToolOptions, ExtensionAPI, ExtensionFuture};

/// Registers the native bash tool with the extension's initial working directory.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
        let tool = api.create_bash_tool_definition(
            cwd.to_string_lossy().into_owned(),
            BashToolOptions::default(),
        )?;
        api.register_tool(tool)
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);

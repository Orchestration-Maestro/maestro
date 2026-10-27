//! Compact displays for the original read, bash, edit and write tools.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    BashToolOptions, EditToolOptions, ExtensionAPI, ExtensionFuture, ReadToolOptions,
    WriteToolOptions,
};

/// Registers the original tool definitions without changing execution.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
        let cwd = cwd.to_str().ok_or("working directory is not valid UTF-8")?;
        api.register_tool(
            api.create_read_tool_definition(cwd.into(), ReadToolOptions::default())?,
        )?;
        api.register_tool(
            api.create_bash_tool_definition(cwd.into(), BashToolOptions::default())?,
        )?;
        api.register_tool(
            api.create_edit_tool_definition(cwd.into(), EditToolOptions::default())?,
        )?;
        api.register_tool(
            api.create_write_tool_definition(cwd.into(), WriteToolOptions::default())?,
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);

//! Opt-in SSH operations for the native coding tools.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionFuture, ExtensionResult, FlagOptions, FlagType, ToolsOptions,
};
use std::rc::Rc;

/// Keeps each built-in definition while selecting its invocation directory at execution.
fn register(api: &ExtensionAPI, name: &'static str) -> ExtensionResult<()> {
    let mut definition = api.create_tool_definition(name, ".".into(), ToolsOptions::default())?;
    let registered = api.clone();
    definition.execute = Rc::new(move |id, args, signal, update, ctx| {
        let api = registered.clone();
        Box::pin(async move {
            let tool = api.create_tool(name, ctx.cwd()?, ToolsOptions::default())?;
            (tool.execute)(id, args, signal, update).await
        })
    });
    api.register_tool(definition)
}

/// Registers coding tools that retain the host's original local behavior.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        api.register_flag(
            "ssh",
            FlagOptions {
                description: Some("SSH remote: user@host or user@host:/path".into()),
                type_: FlagType::String,
                default: None,
            },
        )?;
        for name in ["read", "write", "edit", "bash"] {
            register(&api, name)?;
        }
        Ok(())
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);

//! Retains a real native factory result across independent author callbacks.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::bindings::maestro::extension::host;
use maestro_extensions_wasm::{CommandOptions, ExtensionAPI, ExtensionResult};
use std::rc::Rc;

/// Constructs once in the author factory, then invokes that same native owner later.
pub(super) fn register(api: &ExtensionAPI) -> ExtensionResult<()> {
    let tool = Rc::new(host::create_read_tool("/", None, None)?);
    api.register_command(
        "builtin-read",
        CommandOptions {
            description: None,
            get_argument_completions: None,
            handler: Rc::new(move |path, _| {
                let tool = tool.clone();
                Box::pin(async move {
                    let args = serde_json::json!({"path": path}).to_string();
                    let args = tool.prepare(&args)?;
                    let result = tool
                        .execute("retained-read".into(), args, None, None)
                        .await?;
                    Err(result)
                })
            }),
        },
    )?;
    let edit = Rc::new(host::create_edit_tool("/nonexistent-builtin-root")?);
    api.register_command(
        "builtin-prepare-edit",
        CommandOptions {
            description: None,
            get_argument_completions: None,
            handler: Rc::new(move |args, _| {
                let result = edit.prepare(&args);
                Box::pin(async move { Err(result?) })
            }),
        },
    )
}

/// Uses native tools from later command callbacks, independently of the factory return.
pub(super) async fn command(
    api: &ExtensionAPI,
    context: &maestro_extensions_wasm::ExtensionCommandContext,
    args: &str,
) -> Option<ExtensionResult<()>> {
    if let Some(request) = args.strip_prefix("builtin-custom-read ") {
        let result = super::builtin_read::execute(api, (**context).clone(), request).await;
        return Some(Err(result.unwrap_or_else(std::convert::identity)));
    }
    args.strip_prefix("builtin-register-renderer ")
        .map(|mode| super::builtin_render::register(api, mode))
}

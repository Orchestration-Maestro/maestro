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
    register_read(api)?;
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

/// Retains a four-argument executable independently of every command context.
fn register_read(api: &ExtensionAPI) -> ExtensionResult<()> {
    let tool = Rc::new(api.create_read_tool(
        "/".into(),
        maestro_extensions_wasm::ReadToolOptions::default(),
    )?);
    for name in ["builtin-read", "builtin-read-executable"] {
        let tool = tool.clone();
        api.register_command(
            name,
            CommandOptions {
                description: None,
                get_argument_completions: None,
                handler: Rc::new(move |path, _| execute_read(tool.clone(), name, path)),
            },
        )?;
    }
    Ok(())
}

/// Observes only executable metadata; definition-only fields are not on this record.
fn metadata(tool: &maestro_extensions_wasm::Tool) -> ExtensionResult<serde_json::Value> {
    let mut metadata = serde_json::json!({"name":tool.name,"label":tool.label,"description":tool.description,"parameters":tool.parameters});
    if !tool.execution_mode.is_missing() {
        metadata["executionMode"] =
            serde_json::to_value(tool.execution_mode).map_err(|error| error.to_string())?;
    }
    Ok(metadata)
}

/// Executes the retained facade without passing the command's context to the tool.
fn execute_read(
    tool: Rc<maestro_extensions_wasm::Tool>,
    name: &'static str,
    path: String,
) -> maestro_extensions_wasm::ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let result = (tool.execute)(
            "retained-read".into(),
            serde_json::json!({"path":path}),
            None,
            None,
        )
        .await?;
        if name == "builtin-read" {
            return Err(serde_json::to_string(&result).map_err(|error| error.to_string())?);
        }
        Err(serde_json::json!({"metadata":metadata(&tool)?, "prepares":tool.prepare_arguments.is_some(),"result":result}).to_string())
    })
}

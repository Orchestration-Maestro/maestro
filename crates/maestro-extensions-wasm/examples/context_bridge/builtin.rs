//! Retains a real native factory result across independent author callbacks.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::bindings::maestro::extension::host;
use maestro_extensions_wasm::{CommandOptions, ExtensionAPI, ExtensionResult};
use std::rc::Rc;

/// Whether `path` ends in the `/`-separated components `tail`. The native tools join paths
/// with the host separator, which is `\` on Windows.
pub(super) fn ends_in(path: &str, tail: &str) -> bool {
    path.replace('\\', "/").ends_with(&format!("/{tail}"))
}

/// Constructs once in the author factory, then invokes that same native owner later.
pub(super) fn register(api: &ExtensionAPI) -> ExtensionResult<()> {
    register_read(api)?;
    let edit = Rc::new(host::create_edit_tool("/nonexistent-builtin-root", None)?);
    api.register_command(
        "builtin-prepare-edit",
        CommandOptions {
            description: None,
            get_argument_completions: None,
            handler: Rc::new(move |args, _| {
                let result = edit.prepare(&args).and_then(|result| result.prepared);
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
    if let Some(request) = args.strip_prefix("builtin-register-custom ") {
        return Some(super::builtin_registered::register(api, context, request));
    }
    if let Some(request) = args.strip_prefix("builtin-options ") {
        let result = super::builtin_options::execute(api, context, request).await;
        return Some(Err(result.unwrap_or_else(std::convert::identity)));
    }
    if let Some(action) = args.strip_prefix("builtin-lifetimes ") {
        let result = super::builtin_lifetimes::command(api, context, action).await;
        return Some(Err(result.unwrap_or_else(std::convert::identity)));
    }
    if let Some(cwd) = args.strip_prefix("builtin-fresh ") {
        let result = super::builtin_factories::fresh(api, context, cwd).await;
        return Some(Err(result.unwrap_or_else(std::convert::identity)));
    }
    if let Some(result) = factories_command(api, args).await {
        return Some(result);
    }
    if let Some(request) = args.strip_prefix("builtin-progress ") {
        let result = super::builtin_progress::execute(api, context.clone(), request).await;
        return Some(Err(result.unwrap_or_else(std::convert::identity)));
    }
    if let Some(request) = args.strip_prefix("builtin-custom-bash ") {
        let result = super::builtin_bash::execute(api, context.clone(), request).await;
        return Some(Err(result.unwrap_or_else(std::convert::identity)));
    }
    if let Some(request) = args.strip_prefix("builtin-queue ") {
        let result = super::builtin_queue::command(api, context.clone(), request).await;
        return Some(Err(result.unwrap_or_else(std::convert::identity)));
    }
    if let Some(request) = args.strip_prefix("builtin-register-edit ") {
        return Some(super::builtin_edit::register(api, request));
    }
    filesystem_command(api, context, args).await
}

/// Dispatches filesystem and rendering observations independently of shell and queue probes.
async fn filesystem_command(
    api: &ExtensionAPI,
    context: &maestro_extensions_wasm::ExtensionCommandContext,
    args: &str,
) -> Option<ExtensionResult<()>> {
    if let Some(request) = args.strip_prefix("builtin-custom-edit ") {
        let result = super::builtin_edit::execute(api, context.clone(), request).await;
        return Some(Err(result.unwrap_or_else(std::convert::identity)));
    }
    if let Some(request) = args.strip_prefix("builtin-custom-grep ") {
        let result = super::builtin_grep::execute(api, context.clone(), request).await;
        return Some(Err(result.unwrap_or_else(std::convert::identity)));
    }
    if let Some(request) = args.strip_prefix("builtin-custom-ls ") {
        let result = super::builtin_ls::execute(api, context.clone(), request).await;
        return Some(Err(result.unwrap_or_else(std::convert::identity)));
    }
    if let Some(request) = args.strip_prefix("builtin-custom-find ") {
        let result = super::builtin_find::execute(api, context.clone(), request).await;
        return Some(Err(result.unwrap_or_else(std::convert::identity)));
    }
    if let Some(request) = args.strip_prefix("builtin-custom-write ") {
        let result = super::builtin_write::execute(api, context.clone(), request).await;
        return Some(Err(result.unwrap_or_else(std::convert::identity)));
    }
    if let Some(request) = args.strip_prefix("builtin-custom-read ") {
        let result = super::builtin_read::execute(api, context.clone(), request).await;
        return Some(Err(result.unwrap_or_else(std::convert::identity)));
    }
    args.strip_prefix("builtin-register-renderer ")
        .map(|mode| super::builtin_render::register(api, mode))
        .or_else(|| {
            args.strip_prefix("builtin-signals ").map(|action| {
                Err(super::builtin_signals::command(api, context, action)
                    .unwrap_or_else(std::convert::identity))
            })
        })
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
pub(super) fn metadata(tool: &maestro_extensions_wasm::Tool) -> ExtensionResult<serde_json::Value> {
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

/// Dispatches factory observations without executing unrelated filesystem commands.
async fn factories_command(api: &ExtensionAPI, args: &str) -> Option<ExtensionResult<()>> {
    if let Some(cwd) = args.strip_prefix("builtin-membership ") {
        let result = super::builtin_factories::membership(api, cwd);
        return Some(Err(result.unwrap_or_else(std::convert::identity)));
    }
    if let Some(request) = args.strip_prefix("builtin-unknown ") {
        let result = super::builtin_factories::unknown(api, request);
        return Some(Err(result.unwrap_or_else(std::convert::identity)));
    }
    if let Some(request) = args.strip_prefix("builtin-defaults ") {
        let result = super::builtin_factories::defaults(api, request).await;
        return Some(Err(result.unwrap_or_else(std::convert::identity)));
    }
    if let Some(request) = args.strip_prefix("builtin-wrapped ") {
        let result = super::builtin_factories::wrapped(api, request).await;
        return Some(Err(result.unwrap_or_else(std::convert::identity)));
    }
    None
}

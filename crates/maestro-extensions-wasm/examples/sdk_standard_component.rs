//! A component tool exposing standard filesystem observations to an embedding session.
// maestro-component: MAESTRO_SDK_STANDARD_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

#[cfg(target_arch = "wasm32")]
#[path = "wasi_capabilities/filesystem.rs"]
mod filesystem;

/// Register the ordinary tool callback used by native embedding hosts.
#[cfg(target_arch = "wasm32")]
fn factory(
    api: maestro_extensions_wasm::ExtensionAPI,
) -> maestro_extensions_wasm::ExtensionFuture<'static, ()> {
    use maestro_extensions_wasm::ToolDefinition;
    use serde_json::json;
    use std::rc::Rc;
    Box::pin(async move {
        api.register_tool(ToolDefinition {
            render_call: None, render_result: None, prepare_arguments: None,
            metadata: serde_json::from_value(json!({"name":"sdk-standard", "label":"Standard interfaces",
                "description":"Observe standard filesystem operations", "parameters":{"type":"object"}}))
                .map_err(|error| error.to_string())?,
            execute: Rc::new(|_, params, _, _, context| Box::pin(async move {
                let mut observed = if params["operation"] == "link" {
                    filesystem::link(&params, params["target"].as_str().ok_or("missing target")?,
                        params["link"].as_str().ok_or("missing link")?).await?
                } else {
                    filesystem::operate(&params, params["path"].as_str().ok_or("missing path")?).await?
                };
                observed["cwd"] = json!(context.cwd()?);
                observed["standardCwd"] = filesystem::cwd();
                serde_json::from_value(json!({"content":[], "details":observed})).map_err(|error| error.to_string())
            })),
        })
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);

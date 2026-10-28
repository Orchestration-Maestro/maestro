//! A component tool exposing standard filesystem observations to an embedding session.
// maestro-component: MAESTRO_SDK_STANDARD_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

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
            execute: Rc::new(|_, _, _, _, _| Box::pin(async move {
                serde_json::from_value(json!({"content":[], "details":null})).map_err(|error| error.to_string())
            })),
        })
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);

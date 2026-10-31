//! A greeting tool registered by an extension factory.
// maestro-component: MAESTRO_HELLO_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use maestro_extensions_wasm::{
    AgentToolResult, ExtensionAPI, ExtensionFuture, Presence, TextContent, ToolDefinition,
    ToolMetadata, UserBlock,
};
use serde::Deserialize;
use serde_json::json;

/// The name supplied to the greeting tool.
#[derive(Deserialize)]
struct HelloParams {
    /// Name to greet without trimming or rewriting it.
    name: String,
}

/// A tool that returns a greeting and the supplied name.
fn hello_tool() -> ToolDefinition {
    ToolDefinition {
        render_call: None,
        render_result: None,
        metadata: ToolMetadata {
            name: "hello".into(),
            label: "Hello".into(),
            description: "A simple greeting tool".into(),
            parameters: json!({"type":"object","required":["name"],"properties":{
                "name":{"type":"string","description":"Name to greet"}
            }})
            .into(),
            prompt_snippet: Presence::Missing,
            prompt_guidelines: Presence::Missing,
            render_shell: Presence::Missing,
            execution_mode: Presence::Missing,
        },
        prepare_arguments: None,
        execute: Rc::new(|_, params, _, _, _| {
            Box::pin(async move {
                let params: HelloParams =
                    serde_json::from_value(params).map_err(|error| error.to_string())?;
                Ok(AgentToolResult {
                    content: vec![UserBlock::Text(TextContent::new(format!(
                        "Hello, {}!",
                        params.name
                    )))],
                    details: Presence::Present(json!({"greeted":params.name}).to_string().into()),
                    terminate: Presence::Missing,
                    usage: None,
                })
            })
        }),
    }
}

/// Registers the greeting tool through the supplied extension host.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move { api.register_tool(hello_tool()) })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);

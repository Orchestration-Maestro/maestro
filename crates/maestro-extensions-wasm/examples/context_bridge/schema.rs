//! Authored schema kinds survive native registration and enumeration.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{ExtensionAPI, ExtensionResult, ParameterSchema, SchemaKind};
use serde_json::{Value, json};
use std::rc::Rc;

/// Registers an authored schema, then observes the original host's enumerated declaration.
pub(super) fn observe(api: &ExtensionAPI) -> ExtensionResult<Value> {
    let mut metadata: maestro_extensions_wasm::ToolMetadata = serde_json::from_value(json!({"name":"schema-roundtrip", "label":"Schema", "description":"Authored schema", "parameters":{"type":"object","properties":{"n":{"type":"number"}}}})).map_err(|error| error.to_string())?;
    metadata.parameters = ParameterSchema::authored(
        metadata.parameters.into_json(),
        [
            (String::new(), SchemaKind::Object),
            ("/properties/n".into(), SchemaKind::Number),
        ],
    );
    api.register_tool(maestro_extensions_wasm::ToolDefinition {
        metadata,
        prepare_arguments: None,
        execute: Rc::new(|_, _, _, _, _| {
            Box::pin(async { Err("schema witness never executes".into()) })
        }),
    })?;
    let tools = api.get_all_tools()?;
    let tool = tools
        .iter()
        .find(|tool| tool.name == "schema-roundtrip")
        .ok_or("registered schema missing")?;
    Ok(json!({"parameters":tool.parameters.as_json(), "kinds":tool.parameters.conversions()}))
}

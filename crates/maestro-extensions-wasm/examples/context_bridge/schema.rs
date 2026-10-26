//! Authored schema kinds survive native registration and enumeration.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{ExtensionAPI, ExtensionResult, ParameterSchema, SchemaKind};
use serde_json::{Value, json};
use std::rc::Rc;

#[path = "foreign_schema.rs"]
mod foreign;

/// Runs the schema command `args` names, or returns `None` for any other command.
pub(super) fn command(api: &ExtensionAPI, args: &str) -> Option<ExtensionResult<Value>> {
    if let Some(chain) = args.strip_prefix("schema chain:") {
        return Some(
            chain
                .split_once(':')
                .ok_or_else(|| "missing chain name".into())
                .and_then(|(source, name)| foreign::chain(api, source, name)),
        );
    }
    if let Some(count) = args.strip_prefix("schema growth:") {
        let count = count.parse::<usize>().map_err(|e| e.to_string());
        return Some(count.and_then(|count| growth(api, count)));
    }
    match args {
        "schema methods" => Some(foreign::methods(api)),
        "schema leaf" => Some(foreign::leaf(api)),
        "schema inspect" => Some(foreign::inspect(api)),
        "schema executable" => Some(executable(api)),
        "schema relay" => Some(relay(api)),
        "schema imports" => Some(observe(api)),
        _ => None,
    }
}

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
        render_call: None,
        render_result: None,
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

/// Registers a guest check that edits the exact native operand.
pub(super) fn executable(api: &ExtensionAPI) -> ExtensionResult<Value> {
    use maestro_extensions_wasm::schema_code::{Refinement, SchemaCallback, SchemaNode};
    let metadata: maestro_extensions_wasm::ToolMetadata = serde_json::from_value(json!({"name":"schema-executable", "label":"Schema", "description":"Borrowed schema", "parameters":{"allOf":[{}, {"required":["seen"]}]}})).map_err(|e| e.to_string())?;
    let mut metadata = metadata;
    metadata.parameters = metadata.parameters.with_code(
        [(
            String::from("/allOf/0"),
            SchemaNode {
                base: None,
                refinements: vec![Refinement {
                    check: SchemaCallback::new(|value, _| {
                        assert!(value.kind()?.is_some());
                        assert!(value.keys()?.contains(&"remove".into()));
                        assert_eq!(value.member("remove").read()?, Some(json!(7)));
                        value.set("seen", &json!(true))?;
                        value.delete("remove")?;
                        assert_eq!(
                            value.member("values").splice(1, 1, &[json!(9)])?,
                            vec![json!(2)]
                        );
                        Ok(true)
                    }),
                    error: SchemaCallback::new(|_, _| Ok("not reached".into())),
                }],
            },
        )]
        .into_iter()
        .collect(),
    );
    api.register_tool(maestro_extensions_wasm::ToolDefinition {
        render_call: None,
        render_result: None,
        metadata,
        prepare_arguments: None,
        execute: Rc::new(|_, args, _, _, _| {
            Box::pin(async move {
                serde_json::from_value(json!({"content":[],"details":args}))
                    .map_err(|e| e.to_string())
            })
        }),
    })?;
    Ok(json!("registered"))
}

/// Re-registers live same-origin callbacks under their retained identities.
pub(super) fn relay(api: &ExtensionAPI) -> ExtensionResult<Value> {
    let tools = api.get_all_tools()?;
    let tool = tools
        .into_iter()
        .find(|tool| tool.name == "schema-executable")
        .ok_or("schema missing")?;
    let count = tool.parameters.code().len();
    let mut metadata: maestro_extensions_wasm::ToolMetadata = serde_json::from_value(
        json!({"name":"schema-relayed","label":"Relay","description":"","parameters":{}}),
    )
    .map_err(|e| e.to_string())?;
    metadata.parameters = tool.parameters;
    api.register_tool(maestro_extensions_wasm::ToolDefinition {
        render_call: None,
        render_result: None,
        metadata,
        prepare_arguments: None,
        execute: Rc::new(|_, args, _, _, _| {
            Box::pin(async move {
                serde_json::from_value(json!({"content":[],"details":args}))
                    .map_err(|e| e.to_string())
            })
        }),
    })?;
    Ok(json!(count))
}

/// Register independent checks that request only constant-sized operand fields.
pub(super) fn growth(api: &ExtensionAPI, count: usize) -> ExtensionResult<Value> {
    use maestro_extensions_wasm::schema_code::{Refinement, SchemaCallback, SchemaNode};
    let mut metadata:maestro_extensions_wasm::ToolMetadata=serde_json::from_value(json!({"name":"schema-growth","label":"Growth","description":"Local operands","parameters":{"allOf":vec![json!({});count]}})).map_err(|e|e.to_string())?;
    metadata.parameters = metadata.parameters.with_code(
        (0..count)
            .map(|index| {
                (
                    format!("/allOf/{index}"),
                    SchemaNode {
                        base: None,
                        refinements: vec![Refinement {
                            check: SchemaCallback::new(|value, _| {
                                assert_eq!(value.member("marker").read()?, Some(json!(1)));
                                value.set("touched", &json!(true))?;
                                Ok(true)
                            }),
                            error: SchemaCallback::new(|_, _| Ok("unreached".into())),
                        }],
                    },
                )
            })
            .collect(),
    );
    api.register_tool(maestro_extensions_wasm::ToolDefinition {
        render_call: None,
        render_result: None,
        metadata,
        prepare_arguments: None,
        execute: Rc::new(|_, _, _, _, _| Box::pin(async { Err("not executed".into()) })),
    })?;
    Ok(json!(count))
}

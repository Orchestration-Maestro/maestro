//! Retained native schema callbacks and their provider-data projection.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::{local_support as support, tool_support};
use maestro_models::{
    ParameterSchema, Refinement, SchemaCallback, SchemaCode, SchemaNode, ToolCall,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

#[test]
fn registered_schema_code_survives_native_retention() {
    let bus = support::LocalBus::new();
    let loaded = support::loaded("author", bus.controller.for_extension());
    let retained = support::loaded("recipient", bus.controller.for_extension());
    let capture = Rc::new(RefCell::new(Vec::new()));
    let weak = Rc::downgrade(&capture);
    let mut definition = tool_support::definition("echo", "Echo");
    definition.definition.parameters =
        definition
            .definition
            .parameters
            .with_code(SchemaCode::from([(
                String::new(),
                SchemaNode {
                    base: None,
                    refinements: vec![Refinement {
                        check: SchemaCallback::new(move |value| {
                            capture.borrow_mut().push("author");
                            value["seen"] = json!(true);
                            Ok(true)
                        }),
                        error: SchemaCallback::new(|_| Ok("author rejection".into())),
                    }],
                },
            )]));
    loaded.api.register_tool(definition).unwrap();
    let original = loaded.owner.read().unwrap().tools["echo"]
        .definition
        .metadata()
        .definition;
    let mut relay = tool_support::definition("echo", "Echo");
    relay.definition = original.clone();
    retained.api.register_tool(relay).unwrap();
    drop(original);
    drop(loaded);
    let parameters = retained.owner.read().unwrap().tools["echo"]
        .definition
        .metadata()
        .definition
        .parameters;
    let mut definition = tool_support::definition("echo", "Echo");
    definition.definition.parameters = parameters;
    let wrapped = maestro_tools::local::wrap_tool_definition(definition, None);
    let call = ToolCall {
        id: "id".into(),
        name: "echo".into(),
        arguments: json!({}),
        thought_signature: None,
    };
    let args = maestro_models::validate_tool_arguments(&wrapped.definition, &call).unwrap();
    assert_eq!(args, json!({"seen":true}));
    assert_eq!(call.arguments, json!({}));
    let result = crate::runtime_support::runtime()
        .block_on((wrapped.execute)("id".into(), args, None, None))
        .unwrap();
    assert_eq!(result.details, Some(json!({"seen":true})));
    assert_eq!(*weak.upgrade().unwrap().borrow(), ["author"]);
    drop(wrapped);
    drop(retained);
    assert!(weak.upgrade().is_none());
}

#[test]
fn data_only_context_keeps_threaded_consumers() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<maestro_models::Tool>();
    send_sync::<maestro_models::Context>();
    send_sync::<maestro_agent::AgentTool>();
    send_sync::<maestro_tools::ToolDefinition>();
    let mut definition = tool_support::definition("echo", "Echo");
    definition.definition.parameters = ParameterSchema::<()>::from(json!({"type":"object"}))
        .with_code(SchemaCode::from([(
            String::new(),
            SchemaNode {
                base: None,
                refinements: vec![Refinement {
                    check: SchemaCallback::new(|_| {
                        panic!("projection must not invoke schema code")
                    }),
                    error: SchemaCallback::new(|_| panic!("projection must not ask for errors")),
                }],
            },
        )]));
    let wrapped = maestro_tools::local::wrap_tool_definition(definition, None);
    let tools = std::sync::RwLock::new(vec![Rc::new(std::sync::RwLock::new(wrapped))]);
    let context = maestro_models::Context {
        system_prompt: None,
        messages: Vec::new(),
        tools: Some(
            maestro_agent::agent_loop::tool_definitions::<maestro_models::Local>(&tools).unwrap(),
        ),
    };
    assert_eq!(
        serde_json::to_value(context).unwrap()["tools"][0]["parameters"],
        json!({"type":"object"})
    );
    let encoded: Value = serde_json::to_value(
        tools.read().unwrap()[0]
            .read()
            .unwrap()
            .definition
            .parameters
            .clone(),
    )
    .unwrap();
    assert_eq!(encoded, json!({"type":"object"}));
}

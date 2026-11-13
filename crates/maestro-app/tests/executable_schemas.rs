//! Application enumeration keeps native schema author captures callable.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::sdk_definition;
use maestro_app::test_support::{HarnessOptions, harness, on_task_set, tool_definition};
use maestro_models::{Refinement, SchemaCallback, SchemaCode, SchemaNode, ToolCall};
use maestro_test_directory::Scratch;
use serde_json::json;
use std::{cell::Cell, rc::Rc};

#[test]
fn enumerated_schema_code_can_be_registered_again() {
    on_task_set(|_| async {
        let calls = Rc::new(Cell::new(0));
        let observed = Rc::clone(&calls);
        let mut original = tool_definition("schema", None, None);
        original.definition.parameters =
            original.definition.parameters.with_code(SchemaCode::from([(
                String::new(),
                SchemaNode {
                    base: None,
                    refinements: vec![Refinement {
                        check: SchemaCallback::new(move |value| {
                            observed.set(observed.get() + 1);
                            value["seen"] = json!(true);
                            Ok(true)
                        }),
                        error: SchemaCallback::new(|_| Ok("author rejection".into())),
                    }],
                },
            )]));
        let first = harness(
            Scratch::new().unwrap(),
            HarnessOptions {
                custom_tools: vec![sdk_definition(original)],
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let enumerated = first
            .session
            .get_all_tools()
            .into_iter()
            .find(|tool| tool.name == "schema")
            .unwrap();
        let mut relay = tool_definition("relayed", None, None);
        relay.definition.parameters = enumerated.parameters;
        let second = harness(
            Scratch::new().unwrap(),
            HarnessOptions {
                custom_tools: vec![sdk_definition(relay)],
                ..Default::default()
            },
        )
        .await
        .unwrap();
        drop(first);
        let registered = second.session.get_tool_definition("relayed").unwrap();
        let definition = registered.metadata().definition;
        let call = ToolCall {
            id: "id".into(),
            name: "relayed".into(),
            arguments: json!({}),
            thought_signature: None,
        };
        let args = maestro_models::validate_tool_arguments(&definition, &call).unwrap();
        assert_eq!(args, json!({"seen":true}));
        assert_eq!(call.arguments, json!({}));
        assert_eq!(calls.get(), 1);
    })
    .unwrap();
}

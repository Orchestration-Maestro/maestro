//! Native registration and enumeration reuse one authored-schema transport.
#![cfg(test)]
use maestro_extensions::{
    ExtensionContextActions, ExtensionExecution, Local, ToolInfo, types::ProviderActions,
};
use serde_json::json;
use std::rc::Rc;

#[test]
fn native_registration_and_enumeration_preserve_authored_schema_kinds() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = maestro_extensions::create_event_bus_for::<Local>(Rc::new(|_| panic!("no tails")), Rc::new(|_| panic!("no diagnostics")));
        let (owner, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let weak = Rc::downgrade(&owner);
        let mut actions = Local::unbound_actions();
        actions.get_all_tools = Rc::new(move || {
            let owner = weak.upgrade().ok_or("registration owner ended")?;
            let owner = owner.read().unwrap();
            Ok(owner.tools.values().map(|tool| {
                let metadata = tool.metadata();
                ToolInfo { name: metadata.definition.name, description: metadata.definition.description, parameters: metadata.definition.parameters, source_info: tool.source_info.read().unwrap().clone() }
            }).collect())
        });
        runner.bind_core(actions, ExtensionContextActions::default(), ProviderActions::default()).unwrap();
        let handler = runner.get_command("bridge-observe").unwrap().command.handler;
        let result = handler("schema imports".into(), runner.create_command_context()).await.unwrap_err().to_string();
        let observed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(observed, json!({"parameters":{"type":"object","properties":{"n":{"type":"number"}}},"kinds":{"":"Object","/properties/n":"Number"}}));
        let native = owner.read().unwrap().tools["schema-roundtrip"].metadata();
        assert_eq!(json!(native.definition.parameters.conversions()), observed["kinds"]);
        bus.clear();
    });
}

#[test]
fn schema_borrowed_edits_reach_checker_and_retained_registration() {
    tokio::task::LocalSet::new().block_on(
        &tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap(),
        async {
            let bus = maestro_extensions::create_event_bus_for::<Local>(
                Rc::new(|_| panic!("no tails")),
                Rc::new(|_| panic!("no errors")),
            );
            let (owner, runner, _) =
                super::retained_components::loaded_on(bus.for_extension()).await;
            bind_live_tools(&owner, &runner);
            let command = runner
                .get_command("bridge-observe")
                .unwrap()
                .command
                .handler;
            assert_eq!(
                command("schema executable".into(), runner.create_command_context())
                    .await
                    .unwrap_err()
                    .to_string(),
                "\"registered\""
            );
            let tool = owner.read().unwrap().tools["schema-executable"]
                .metadata()
                .definition;
            let call = maestro_models::ToolCall {
                id: "schema".into(),
                name: tool.name.clone(),
                arguments: json!({"remove":7,"values":[1,2,3]}),
                thought_signature: None,
            };
            assert_borrowed_edits(&tool, &call);
            assert_eq!(
                command("schema relay".into(), runner.create_command_context())
                    .await
                    .unwrap_err()
                    .to_string(),
                "1"
            );
            let relayed = owner.read().unwrap().tools["schema-relayed"]
                .metadata()
                .definition;
            assert_borrowed_edits(&relayed, &call);
            drop(tool);
            owner
                .write()
                .unwrap()
                .tools
                .shift_remove("schema-executable");
            assert_borrowed_edits(&relayed, &call);
            bus.clear();
        },
    );
}

/// Bind enumeration to the actual registration owner without copied schema metadata.
fn bind_live_tools(
    owner: &maestro_extensions::types::ExtensionHandle<Local>,
    runner: &maestro_extensions::ExtensionRunner<Local>,
) {
    let weak = Rc::downgrade(owner);
    let mut actions = Local::unbound_actions();
    actions.get_all_tools = Rc::new(move || {
        let owner = weak.upgrade().ok_or("owner ended")?;
        let owner = owner.read().unwrap();
        Ok(owner
            .tools
            .values()
            .map(|tool| {
                let metadata = tool.metadata();
                ToolInfo {
                    name: metadata.definition.name,
                    description: metadata.definition.description,
                    parameters: metadata.definition.parameters,
                    source_info: tool.source_info.read().unwrap().clone(),
                }
            })
            .collect())
    });
    runner
        .bind_core(
            actions,
            ExtensionContextActions::default(),
            ProviderActions::default(),
        )
        .unwrap();
}

/// The real checker observes guest edits before the later required-member rule.
fn assert_borrowed_edits(
    tool: &maestro_models::Tool<maestro_models::SchemaCode>,
    call: &maestro_models::ToolCall,
) {
    assert_eq!(
        maestro_models::validate_tool_arguments(tool, call).unwrap(),
        json!({"values":[1,9,3],"seen":true})
    );
}

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

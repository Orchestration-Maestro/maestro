//! Foreign schemas use their originating callbacks through the component route.
#![cfg(test)]
use maestro_extensions::{
    ExtensionContextActions, ExtensionExecution, Local, ToolInfo, types::ProviderActions,
};
use serde_json::json;
use std::rc::Rc;

#[test]
fn foreign_guest_schema_enumeration_retention_and_registration_keep_live_edits() {
    super::components::local(async {
        let bus = maestro_extensions::create_event_bus_for::<Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no diagnostics")),
        );
        let (origin, source, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let (receiver, target, _) =
            super::retained_components::loaded_on(bus.for_extension()).await;
        listing(&target, &origin);
        assert_eq!(
            command(&source, "schema executable").await,
            "\"registered\""
        );
        let count = command(&target, "schema relay").await;
        let tool = receiver.read().unwrap().tools["schema-relayed"]
            .metadata()
            .definition;
        let call = maestro_models::ToolCall {
            id: "foreign".into(),
            name: tool.name.clone(),
            arguments: json!({"remove":7,"values":[1,2,3]}),
            thought_signature: None,
        };
        origin
            .write()
            .unwrap()
            .tools
            .shift_remove("schema-executable");
        assert_eq!(
            maestro_models::validate_tool_arguments(&tool, &call).unwrap(),
            json!({"values":[1,9,3],"seen":true})
        );
        assert_eq!(count, "1");
        bus.clear();
    });
}

#[test]
fn native_origin_schema_preserves_methods_structured_errors_and_selected_view_edits() {
    super::components::local(async {
        let bus = maestro_extensions::create_event_bus_for::<Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no diagnostics")),
        );
        let (owner, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let mut actions = Local::unbound_actions();
        let source_info = owner.read().unwrap().source_info.read().unwrap().clone();
        actions.get_all_tools = Rc::new(move || {
            Ok(vec![ToolInfo {
                name: "schema-executable".into(),
                description: String::new(),
                parameters: maestro_extensions::ParameterSchema::<()>::from(json!({}))
                    .with_code([(String::new(), native_methods())].into_iter().collect()),
                source_info: source_info.clone(),
            }])
        });
        runner
            .bind_core(
                actions,
                ExtensionContextActions::default(),
                ProviderActions::default(),
            )
            .unwrap();
        let command = runner
            .get_command("bridge-observe")
            .unwrap()
            .command
            .handler;
        assert_eq!(
            command("schema inspect".into(), runner.create_command_context())
                .await
                .unwrap_err()
                .to_string(),
            "\"schema-inspected\""
        );
        let tool = owner.read().unwrap().tools["schema-inspected"]
            .metadata()
            .definition;
        let call = maestro_models::ToolCall {
            id: "native".into(),
            name: tool.name.clone(),
            arguments: json!({"a/b~":{}}),
            thought_signature: None,
        };
        assert_eq!(
            maestro_models::validate_tool_arguments(&tool, &call).unwrap(),
            json!({
                "a/b~":{"native":true,"error-edit":true},
                "observed":[true,null,true,[{"message":"native detail"}],false,"native refinement",{"name":"NativeSchemaError","message":"native failure","stack":"native stack","code":7.0}]
            })
        );
        bus.clear();
    });
}

/// Native callbacks retain the exact result domain and a structured error after editing.
fn native_methods() -> maestro_extensions::SchemaNode {
    use maestro_extensions::{
        Base, DiagnosticCode, DiagnosticErrorInfo, Refinement, SchemaCallback, SchemaNode,
    };
    SchemaNode {
        base: Some(Base {
            convert: SchemaCallback::new(|value| {
                value["native"] = json!(true);
                Ok(Some(serde_json::Value::Null))
            }),
            check: SchemaCallback::new(|value| Ok(value["native"] == true)),
            errors: SchemaCallback::new(|_| Ok(vec![json!({"message":"native detail"})])),
        }),
        refinements: vec![
            Refinement {
                check: SchemaCallback::new(|_| Ok(false)),
                error: SchemaCallback::new(|_| Ok("native refinement".into())),
            },
            Refinement {
                check: SchemaCallback::new(|value| {
                    value["error-edit"] = json!(true);
                    Err(DiagnosticErrorInfo {
                        name: Some("NativeSchemaError".into()),
                        message: "native failure".into(),
                        stack: Some("native stack".into()),
                        code: Some(DiagnosticCode::Number(7.0)),
                    })
                }),
                error: SchemaCallback::new(|_| Ok("unreached".into())),
            },
        ],
    }
}

/// Bind exactly the supplied owner's live tools, retaining neither owner nor callback copies.
fn listing(
    runner: &maestro_extensions::ExtensionRunner<Local>,
    owner: &maestro_extensions::types::ExtensionHandle<Local>,
) {
    let weak = Rc::downgrade(owner);
    let mut actions = Local::unbound_actions();
    actions.get_all_tools = Rc::new(move || {
        let owner = weak.upgrade().ok_or("origin unloaded")?;
        Ok(owner
            .read()
            .unwrap()
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

/// Invoke a fixture command through the actual domain command handler.
async fn command(runner: &maestro_extensions::ExtensionRunner<Local>, args: &str) -> String {
    (runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler)(args.into(), runner.create_command_context())
    .await
    .unwrap_err()
    .to_string()
}

/// The checker enters the fixture's registered synchronous refinement over the real route.
fn check(
    owner: &maestro_extensions::types::ExtensionHandle<Local>,
    name: &str,
) -> Result<serde_json::Value, maestro_extensions::DiagnosticErrorInfo> {
    let tool = owner.read().unwrap().tools[name].metadata().definition;
    let call = maestro_models::ToolCall {
        id: "schema".into(),
        name: name.into(),
        arguments: json!({"trace":[]}),
        thought_signature: None,
    };
    maestro_models::validate_tool_arguments(&tool, &call)
}

#[test]
fn synchronous_a_b_a_schema_reentry_keeps_same_origin_callbacks_local() {
    super::components::local(async {
        let bus = maestro_extensions::create_event_bus_for::<Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no diagnostics")),
        );
        let (a, first, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let (b, second, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        assert_eq!(command(&first, "schema leaf").await, "\"schema-leaf\"");
        listing(&first, &a);
        assert_eq!(
            command(&first, "schema chain:schema-leaf:schema-local").await,
            "true",
            "own callbacks keep their original capture identities"
        );
        assert_eq!(
            check(&a, "schema-local").unwrap()["trace"],
            json!(["schema-local", "schema-leaf"])
        );
        listing(&second, &a);
        assert_eq!(
            command(&second, "schema chain:schema-local:schema-middle").await,
            "false",
            "foreign enumerations retain independent identities"
        );
        listing(&first, &b);
        assert_eq!(
            command(&first, "schema chain:schema-middle:schema-outer").await,
            "false"
        );
        assert_eq!(
            check(&a, "schema-outer").unwrap(),
            json!({"trace":["schema-outer","schema-middle","schema-local","schema-leaf"],"admitted":true,"completed":true})
        );
        bus.clear();
    });
}

#[test]
fn origin_unload_keeps_an_admitted_schema_call_but_rejects_later_entry() {
    super::components::local(async {
        let bus = maestro_extensions::create_event_bus_for::<Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no diagnostics")),
        );
        let events = bus.for_extension();
        let (origin, source, _) = super::retained_components::loaded_on(events.clone()).await;
        let (receiver, target, _) =
            super::retained_components::loaded_on(bus.for_extension()).await;
        let source = Rc::new(source);
        assert_eq!(command(&source, "schema leaf").await, "\"schema-leaf\"");
        listing(&target, &origin);
        assert_eq!(
            command(&target, "schema chain:schema-leaf:schema-alias").await,
            "false"
        );
        let source = source.clone();
        let entries = Rc::new(std::cell::Cell::new(0));
        let entered = entries.clone();
        events.on(
            "schema-enter",
            Rc::new(move |_, _| {
                entered.set(entered.get() + 1);
                source.invalidate(Some("schema origin unloaded".into()));
                Ok(None)
            }),
        );
        assert_eq!(
            check(&receiver, "schema-alias").unwrap()["completed"],
            true,
            "unload must not cancel an admitted callback"
        );
        assert_eq!(
            check(&receiver, "schema-alias").unwrap_err().message,
            "schema origin unloaded"
        );
        assert_eq!(
            entries.get(),
            1,
            "unloaded origin must not be entered again"
        );
        bus.clear();
    });
}

#[test]
fn foreign_schema_alias_release_during_and_after_calls_drops_origin_captures_once() {
    super::components::local(async {
        for during in [false, true] {
            release_aliases(during).await;
        }
    });
}

/// Release the final foreign alias both at rest and while its own callback is running.
async fn release_aliases(during: bool) {
    let bus = maestro_extensions::create_event_bus_for::<Local>(
        Rc::new(|_| panic!("no tails")),
        Rc::new(|_| panic!("no diagnostics")),
    );
    let events = bus.for_extension();
    let (origin, source, _) = super::retained_components::loaded_on(events.clone()).await;
    let (receiver, target, _) = super::retained_components::loaded_on(bus.for_extension()).await;
    let (count, mut dropped) = observe_release(&source);
    assert_eq!(command(&source, "schema leaf").await, "\"schema-leaf\"");
    listing(&target, &origin);
    assert_eq!(
        command(&target, "schema chain:schema-leaf:schema-first").await,
        "false"
    );
    assert_eq!(
        command(&target, "schema chain:schema-leaf:schema-last").await,
        "false"
    );
    origin.write().unwrap().tools.shift_remove("schema-leaf");
    let weak = Rc::downgrade(&receiver);
    let entries = std::cell::Cell::new(0);
    events.on(
        "schema-enter",
        Rc::new(move |_, _| {
            let name = if during && entries.replace(entries.get() + 1) > 0 {
                "schema-last"
            } else {
                "schema-first"
            };
            weak.upgrade()
                .unwrap()
                .write()
                .unwrap()
                .tools
                .shift_remove(name);
            Ok(None)
        }),
    );
    assert_eq!(
        check(&receiver, "schema-first").unwrap()["completed"],
        true,
        "releasing the active registration must not cancel its callback"
    );
    assert!(!receiver.read().unwrap().tools.contains_key("schema-first"));
    assert_eq!(
        count.get(),
        0,
        "the independent foreign alias still owns captures"
    );
    assert_eq!(check(&receiver, "schema-last").unwrap()["completed"], true);
    receiver.write().unwrap().tools.shift_remove("schema-last");
    dropped.recv().await.unwrap();
    assert_eq!(
        count.get(),
        1,
        "last foreign release must eventually destroy origin captures exactly once without another origin call"
    );
    bus.clear();
}

#[test]
fn foreign_guest_schema_preserves_all_methods_structured_errors_and_selected_view_edits() {
    super::components::local(async {
        let bus = maestro_extensions::create_event_bus_for::<Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no diagnostics")),
        );
        let (origin, source, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let (receiver, target, _) =
            super::retained_components::loaded_on(bus.for_extension()).await;
        assert_eq!(
            command(&source, "schema methods").await,
            "\"schema-executable\""
        );
        listing(&target, &origin);
        assert_eq!(
            command(&target, "schema inspect").await,
            "\"schema-inspected\""
        );
        let tool = receiver.read().unwrap().tools["schema-inspected"]
            .metadata()
            .definition;
        let call = maestro_models::ToolCall {
            id: "guest".into(),
            name: tool.name.clone(),
            arguments: json!({"a/b~":{}}),
            thought_signature: None,
        };
        assert_eq!(
            maestro_models::validate_tool_arguments(&tool, &call).unwrap(),
            json!({
                "a/b~":{"native":true,"error-edit":true},
                "observed":[true,null,true,[{"message":"native detail"}],false,"native refinement",{"name":"NativeSchemaError","message":"native failure","stack":"native stack","code":"GUEST"}]
            })
        );
        bus.clear();
    });
}

/// Observe capture destruction without requiring another guest invocation.
fn observe_release(
    source: &maestro_extensions::ExtensionRunner<Local>,
) -> (
    Rc<std::cell::Cell<usize>>,
    tokio::sync::mpsc::UnboundedReceiver<()>,
) {
    let (send, dropped) = tokio::sync::mpsc::unbounded_channel();
    let count = Rc::new(std::cell::Cell::new(0));
    let observed = count.clone();
    let mut actions = Local::unbound_actions();
    actions.set_session_name = Rc::new(move |name| {
        assert_eq!(name, "schema-capture-dropped");
        observed.set(observed.get() + 1);
        send.send(()).unwrap();
        Ok(())
    });
    source
        .bind_core(
            actions,
            ExtensionContextActions::default(),
            ProviderActions::default(),
        )
        .unwrap();
    (count, dropped)
}

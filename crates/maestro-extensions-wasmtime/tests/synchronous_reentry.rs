//! Native code that calls back into the component it is serving, while that component waits
//! in a synchronous host call, gets the result and order the reference produces.
#![cfg(test)]
use super::retained_components::{loaded_on, view};
use maestro_extensions::{
    Cancellation, CompactOptions, EventBus, ExtensionActions, ExtensionContextActions,
    ExtensionExecution, ExtensionRunner, Local, create_event_bus_for, types::ProviderActions,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// Observable order shared by native and guest participants.
type Trace = Rc<RefCell<Vec<String>>>;

/// Runs one witness on a single-threaded local executor.
fn run(witness: impl std::future::Future<Output = ()>) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, witness);
}

/// A bus whose listener errors join the trace, so a rejected nested call is visible there.
fn bus(trace: &Trace) -> EventBus<Local> {
    let errors = trace.clone();
    let bus = create_event_bus_for::<Local>(
        Rc::new(|_| panic!("no tails")),
        Rc::new(move |error| errors.borrow_mut().push(format!("error:{error}"))),
    )
    .for_extension();
    let observed = trace.clone();
    bus.on(
        "trace",
        Rc::new(move |data, _| {
            observed
                .borrow_mut()
                .push(data.as_str().unwrap_or_default().to_owned());
            Ok(None)
        }),
    );
    bus
}

/// Runs `call` inside a native listener of the named renderer's creation, between markers.
fn during_render(
    bus: &EventBus<Local>,
    trace: &Trace,
    name: &'static str,
    call: impl Fn(maestro_extensions::CallbackEmitter<'_, Local>) + 'static,
) {
    let trace = trace.clone();
    bus.on(
        "renderer-created",
        Rc::new(move |data, emitter| {
            if data[0] == name {
                trace.borrow_mut().push("native.before".into());
                call(emitter);
                trace.borrow_mut().push("native.after".into());
            }
            Ok(None)
        }),
    );
}

/// Renders one retained component and returns its lines or its error text.
fn rendered(component: &maestro_extensions::ComponentHandle, width: usize) -> Value {
    match component.try_render(width) {
        Ok(lines) => json!(lines),
        Err(error) => json!(error.to_string()),
    }
}

#[test]
fn native_listener_reentry_runs_the_same_instance_listeners_of_its_nested_emission() {
    run(async {
        let trace = Trace::default();
        let (_owner, runner, events) = loaded_on(bus(&trace)).await;
        during_render(&events, &trace, "outer", |emitter| {
            emitter.emit("renderer-created", json!(["inner", false]));
        });
        let outer = view(&runner, &events, "outer").unwrap().unwrap();
        assert_eq!(rendered(&outer, 9), json!(["outer:9"]));
        assert_eq!(
            *trace.borrow(),
            [
                "g1:outer",
                "g2:outer",
                "native.before",
                "g1:inner",
                "g2:inner",
                "native.after"
            ]
        );
    });
}

#[test]
fn native_listener_reentry_completes_the_same_instance_renderer() {
    run(async {
        let trace = Trace::default();
        let (_owner, runner, events) = loaded_on(bus(&trace)).await;
        let runner = Rc::new(runner);
        let (calls, nested_events, seen) = (runner.clone(), events.clone(), trace.clone());
        during_render(&events, &trace, "outer", move |_| {
            let inner = view(&calls, &nested_events, "inner").map(|component| {
                component.map_or(json!(null), |component| rendered(&component, 7))
            });
            let inner = inner.unwrap_or_else(|error| json!(error.to_string()));
            seen.borrow_mut().push(inner.to_string());
        });
        let outer = view(&runner, &events, "outer").unwrap().unwrap();
        assert_eq!(rendered(&outer, 9), json!(["outer:9"]));
        assert_eq!(
            *trace.borrow(),
            [
                "g1:outer",
                "g2:outer",
                "native.before",
                "g1:inner",
                "g2:inner",
                r#"["inner:7"]"#,
                "native.after"
            ]
        );
    });
}

#[test]
fn native_listener_reentry_renders_another_retained_component_of_the_same_instance() {
    run(async {
        let trace = Trace::default();
        let (_owner, runner, events) = loaded_on(bus(&trace)).await;
        let retained = view(&runner, &events, "retained").unwrap().unwrap();
        let seen = trace.clone();
        during_render(&events, &trace, "outer", move |_| {
            let lines = rendered(&retained, 13);
            seen.borrow_mut().push(lines.to_string());
        });
        let outer = view(&runner, &events, "outer").unwrap().unwrap();
        assert_eq!(rendered(&outer, 9), json!(["outer:9"]));
        assert_eq!(
            *trace.borrow(),
            [
                "g1:retained",
                "g2:retained",
                "g1:outer",
                "g2:outer",
                "native.before",
                r#"["retained:13"]"#,
                "native.after"
            ]
        );
    });
}

#[test]
fn native_listener_reentry_invalidates_a_retained_component_before_returning() {
    run(async {
        let trace = Trace::default();
        let (_owner, runner, events) = loaded_on(bus(&trace)).await;
        let component = view(&runner, &events, "invalidate-error").unwrap().unwrap();
        let invalidated = component.clone();
        during_render(&events, &trace, "outer", move |_| invalidated.invalidate());
        view(&runner, &events, "outer").unwrap().unwrap();
        assert_eq!(rendered(&component, 5), json!("guest invalidate failure"));
    });
}

#[test]
fn native_listener_reentry_delivers_input_and_reads_the_key_release_preference() {
    run(async {
        let trace = Trace::default();
        let (_owner, runner, events) = loaded_on(bus(&trace)).await;
        let input = view(&runner, &events, "input").unwrap().unwrap();
        let key = view(&runner, &events, "key").unwrap().unwrap();
        let seen = trace.clone();
        during_render(&events, &trace, "outer", move |_| {
            let handler = input.input_handler().unwrap();
            for data in ["literal input", "error"] {
                let result = handler.try_handle_input(data);
                let result = result.map_or_else(|error| error.to_string(), |()| "ok".into());
                seen.borrow_mut().push(result);
            }
            seen.borrow_mut()
                .push(format!("release:{}", key.wants_key_release()));
        });
        view(&runner, &events, "outer").unwrap().unwrap();
        assert_eq!(
            *trace.borrow(),
            [
                "g1:input",
                "g2:input",
                "g1:key",
                "g2:key",
                "g1:outer",
                "g2:outer",
                "native.before",
                "ok",
                "guest input failure",
                "release:true",
                "native.after"
            ]
        );
    });
}

/// Registers the fixture's tool through its own registration command.
async fn registered_tool(
    runner: &ExtensionRunner<Local>,
) -> maestro_extensions::RegisteredTool<Local> {
    let command = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    command("register-tool".into(), runner.create_command_context())
        .await
        .unwrap();
    runner.get_all_registered_tools().remove(0)
}

#[test]
fn native_listener_reentry_prepares_tool_arguments_of_the_same_instance() {
    run(async {
        let trace = Trace::default();
        let (_owner, runner, events) = loaded_on(bus(&trace)).await;
        let runner = Rc::new(runner);
        let tool = registered_tool(&runner).await;
        let prepare = maestro_extensions::wrap_registered_tool(&tool, runner.clone())
            .prepare_arguments
            .unwrap();
        let seen = trace.clone();
        during_render(&events, &trace, "outer", move |_| {
            let prepared = prepare(&mut json!({"before": true}));
            let prepared = prepared.unwrap_or_else(|error| json!(error.to_string()));
            seen.borrow_mut().push(prepared.to_string());
        });
        view(&runner, &events, "outer").unwrap().unwrap();
        assert_eq!(
            *trace.borrow(),
            [
                "g1:outer",
                "g2:outer",
                "native.before",
                r#"{"prepared":{"before":true}}"#,
                "native.after"
            ]
        );
    });
}

#[test]
fn tool_progress_observer_reentry_completes_the_same_instance_renderer() {
    run(async {
        let trace = Trace::default();
        let (_owner, runner, events) = loaded_on(bus(&trace)).await;
        let runner = Rc::new(runner);
        let tool = registered_tool(&runner).await;
        let tool = maestro_extensions::wrap_registered_tool(&tool, runner.clone());
        let (calls, nested_events, seen) = (runner.clone(), events.clone(), trace.clone());
        let signal = Cancellation::new();
        let result = (tool.execute)(
            "literal".into(),
            json!({"before": true}),
            Some(signal),
            Some(Rc::new(move |_| {
                seen.borrow_mut().push("observer.before".into());
                let inner = view(&calls, &nested_events, "from-update");
                let inner = inner.map_or_else(
                    |error| json!(error.to_string()),
                    |component| rendered(&component.unwrap(), 3),
                );
                seen.borrow_mut().push(inner.to_string());
                seen.borrow_mut().push("observer.after".into());
            })),
        )
        .await
        .unwrap();
        trace.borrow_mut().push("execute.returned".into());
        assert_eq!(result.content.len(), 1);
        assert_eq!(
            *trace.borrow(),
            [
                "observer.before",
                "g1:from-update",
                "g2:from-update",
                r#"["from-update:3"]"#,
                "observer.after",
                "execute.returned"
            ]
        );
    });
}

/// Binds an application whose compaction notifies the supplied callbacks before returning.
fn immediate_compaction(runner: &ExtensionRunner<Local>, trace: &Trace) {
    let (seen, failures) = (trace.clone(), Rc::new(std::cell::Cell::new(0)));
    let compact = Rc::new(move |options: Option<CompactOptions>| {
        let options = options.unwrap();
        seen.borrow_mut().push("compact.before".into());
        failures.set(failures.get() + 1);
        if failures.get() == 1 {
            let result = serde_json::from_value(
                json!({"summary": "s", "firstKeptEntryId": "k", "tokensBefore": 1}),
            )
            .unwrap();
            (options.on_complete.unwrap())(result)?;
        } else {
            (options.on_error.unwrap())(" failed ".into())?;
        }
        seen.borrow_mut().push("compact.after".into());
        Ok(())
    });
    runner
        .bind_core(
            Local::unbound_actions(),
            ExtensionContextActions {
                compact,
                ..Default::default()
            },
            ProviderActions::default(),
        )
        .unwrap();
}

#[test]
fn compaction_notifications_delivered_before_compaction_returns_complete_in_order() {
    run(async {
        let trace = Trace::default();
        let (_owner, runner, _events) = loaded_on(bus(&trace)).await;
        immediate_compaction(&runner, &trace);
        let command = runner
            .get_command("bridge-observe")
            .unwrap()
            .command
            .handler;
        for _ in 0..2 {
            let returned = command("compact".into(), runner.create_command_context()).await;
            trace.borrow_mut().push(returned.unwrap_err().to_string());
        }
        assert_eq!(
            *trace.borrow(),
            [
                "compact.before",
                "compact-complete",
                "compact.after",
                "compact returned",
                "compact.before",
                "compact-error: failed ",
                "compact.after",
                "compact returned"
            ]
        );
    });
}

#[test]
fn sent_message_renders_through_the_same_instance_before_the_send_returns() {
    run(async {
        let trace = Trace::default();
        let (_owner, runner, events) = loaded_on(bus(&trace)).await;
        let runner = Rc::new(runner);
        let (calls, nested_events, seen) = (Rc::downgrade(&runner), events.clone(), trace.clone());
        let send_message = Rc::new(move |message: maestro_extensions::CustomMessageInput, _| {
            seen.borrow_mut().push("message_start".into());
            let calls = calls.upgrade().unwrap();
            let component = view(&calls, &nested_events, &message.custom_type)?;
            let lines = rendered(&component.unwrap(), 11);
            seen.borrow_mut().push(lines.to_string());
            Ok(())
        });
        runner
            .bind_core(
                ExtensionActions {
                    send_message,
                    ..Local::unbound_actions()
                },
                ExtensionContextActions::default(),
                ProviderActions::default(),
            )
            .unwrap();
        view(&runner, &events, "send-message").unwrap().unwrap();
        assert_eq!(
            *trace.borrow(),
            [
                "g1:send-message",
                "g2:send-message",
                "message_start",
                "g1:inner",
                "g2:inner",
                r#"["inner:11"]"#,
                "send-returned"
            ]
        );
    });
}

#[test]
fn consecutive_guest_listener_deliveries_return_without_the_caller_yielding() {
    run(async {
        let trace = Trace::default();
        let (_owner, _runner, events) = loaded_on(bus(&trace)).await;
        for _ in 0..64 {
            events
                .emit("renderer-created", json!(["repeat", false]))
                .await;
        }
        assert_eq!(*trace.borrow(), ["g1:repeat", "g2:repeat"].repeat(64));
    });
}

#[test]
fn consecutive_renders_return_whether_or_not_they_reach_guest_listeners() {
    run(async {
        let trace = Trace::default();
        let (_owner, runner, events) = loaded_on(bus(&trace)).await;
        let component = view(&runner, &events, "relay").unwrap().unwrap();
        for width in 0..64 {
            assert_eq!(
                rendered(&component, width),
                json!([format!("relay:{width}")])
            );
        }
        events.on(
            "component-rendered",
            Rc::new(|_, emitter| {
                emitter.emit("renderer-created", json!(["relayed", false]));
                Ok(None)
            }),
        );
        trace.borrow_mut().clear();
        for width in 0..64 {
            assert_eq!(
                rendered(&component, width),
                json!([format!("relay:{width}")])
            );
        }
        assert_eq!(*trace.borrow(), ["g1:relayed", "g2:relayed"].repeat(64));
    });
}

#[test]
fn native_listener_reentry_completes_while_another_component_still_renders() {
    run(async {
        let trace = Trace::default();
        let (_owner, runner, events) = loaded_on(bus(&trace)).await;
        let (_other_owner, other_runner, other_events) = loaded_on(bus(&trace)).await;
        let retained = view(&runner, &events, "normal").unwrap().unwrap();
        let runner = Rc::new(runner);
        let observed = Rc::new(RefCell::new(Vec::new()));
        let (calls, record, listener_events) = (runner.clone(), observed.clone(), events.clone());
        let subscription = events.on(
            "renderer-created",
            Rc::new(move |data, _| {
                if data[0] == "outer" {
                    let inner = view(&calls, &listener_events, "inner").unwrap().unwrap();
                    let other = view(&other_runner, &other_events, "normal")
                        .unwrap()
                        .unwrap();
                    record.borrow_mut().push((
                        rendered(&inner, 10),
                        rendered(&retained, 10),
                        rendered(&other, 31),
                    ));
                }
                Ok(None)
            }),
        );
        let outer = view(&runner, &events, "outer").unwrap().unwrap();
        assert_eq!(
            *observed.borrow(),
            [(
                json!(["inner:10"]),
                json!(["normal:10"]),
                json!(["normal:31"])
            )]
        );
        assert_eq!(outer.try_render(31).unwrap(), ["outer:31"]);
        let released = Rc::downgrade(&runner);
        drop(runner);
        assert!(
            released.upgrade().is_some(),
            "the listener retains the runner"
        );
        subscription.unsubscribe();
        assert!(
            released.upgrade().is_none(),
            "unsubscribe releases the runner"
        );
    });
}

#[test]
fn native_listener_reentry_checks_a_borrowed_schema_of_the_same_instance() {
    run(async {
        let trace = Trace::default();
        let (owner, runner, events) = loaded_on(bus(&trace)).await;
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
        let seen = trace.clone();
        during_render(&events, &trace, "outer", move |_| {
            let call = maestro_models::ToolCall {
                id: "nested-schema".into(),
                name: tool.name.clone(),
                arguments: json!({"remove": 7, "values": [1, 2, 3]}),
                thought_signature: None,
            };
            let checked = maestro_models::validate_tool_arguments(&tool, &call);
            assert_eq!(checked.unwrap(), json!({"seen": true, "values": [1, 9, 3]}));
            seen.borrow_mut().push("schema.checked".into());
        });
        view(&runner, &events, "outer").unwrap().unwrap();
        assert_eq!(
            *trace.borrow(),
            [
                "g1:outer",
                "g2:outer",
                "native.before",
                "schema.checked",
                "native.after",
            ]
        );
    });
}

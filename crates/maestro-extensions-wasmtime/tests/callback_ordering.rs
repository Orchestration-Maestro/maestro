//! Component callbacks deliver foreign listeners at each callback's own return.
#![cfg(test)]
use maestro_extensions::types::{ExtensionHandle, FlagValue};
use maestro_extensions::{
    EventBus, ExtensionRunner, Local, MessageRenderOptions, create_event_bus_for,
};
use std::{cell::RefCell, rc::Rc};

use super::retained_components::loaded_on;

/// Ordered entries recorded by native listeners.
type Trace = Rc<RefCell<Vec<String>>>;

/// Records an instance's trace channel under that instance's own identity, so entries are immediate.
fn observe(events: &EventBus<Local>, label: &str, trace: &Trace) {
    let observed = trace.clone();
    events.on(
        &format!("trace:{label}"),
        Rc::new(move |data, _| {
            observed
                .borrow_mut()
                .push(data.as_str().unwrap().to_owned());
            Ok(None)
        }),
    );
}

/// Loads one labelled instance whose listeners share the caller's bus.
async fn labelled(
    events: EventBus<Local>,
    label: &str,
    trace: &Trace,
) -> (ExtensionHandle<Local>, ExtensionRunner<Local>) {
    observe(&events, label, trace);
    let (owner, runner, _) = loaded_on(events).await;
    runner.set_flag_value("bridge-label".into(), FlagValue::String(label.into()));
    (owner, runner)
}

/// Reports whether the instance released the retained view named `name`.
fn released(owner: &ExtensionHandle<Local>, name: &str) -> bool {
    owner
        .read()
        .unwrap()
        .flags
        .contains_key(&format!("dropped-{name}"))
}

/// Makes one later Store call, after which queued resource releases have run.
async fn barrier(runner: &ExtensionRunner<Local>) {
    let command = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    let error = command("barrier".into(), runner.create_command_context())
        .await
        .unwrap_err();
    assert!(error.to_string().starts_with("command:barrier|"), "{error}");
}

#[test]
fn foreign_component_delivery_follows_each_callback_return() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let tails = Rc::new(RefCell::new(Vec::new()));
        let queued = tails.clone();
        let bus = create_event_bus_for::<Local>(
            Rc::new(move |tail| queued.borrow_mut().push(tail)),
            Rc::new(|error| panic!("{error}")),
        );
        let trace = Trace::default();
        let (owner, runner) = labelled(bus.for_extension(), "a", &trace).await;
        let _foreign = labelled(bus.for_extension(), "b", &trace).await;
        let renderer = runner
            .get_message_renderer_call::<serde_json::Value>("order-view")
            .unwrap();
        let message = serde_json::from_value(serde_json::json!({"role":"custom", "customType": "order-foreign", "content": "literal content", "display": true, "timestamp": 1.5})).unwrap();
        let component = renderer
            .call(
                Rc::new(message),
                MessageRenderOptions { expanded: false },
                maestro_theme::LiveTheme::default(),
            )
            .unwrap()
            .unwrap();
        trace.borrow_mut().push("caller:return".into());
        assert_eq!(
            *trace.borrow(),
            [
                "a:render:start",
                "a:inner:start",
                "a:inner:end",
                "a:after",
                "a:render:end",
                "b:inner:start",
                "b:back",
                "b:inner:end",
                "a:back",
                "b:after",
                "caller:return",
            ]
        );
        assert!(!released(&owner, "order-foreign"));
        let pending = std::mem::take(&mut *tails.borrow_mut());
        for tail in pending {
            tail.await;
        }
        assert_eq!(
            trace.borrow()[11..],
            ["a:inner:tail", "b:back:tail", "a:back:tail", "b:inner:tail"]
        );
        drop(component);
        barrier(&runner).await;
        assert!(released(&owner, "order-foreign"));
    });
}

//! Component callbacks deliver foreign listeners at each callback's own return.
#![cfg(test)]
use maestro_extensions::types::{ExtensionHandle, FlagValue};
use maestro_extensions::{
    ErrorReporter, EventBus, EventBusController, ExtensionRunner, Local, MessageRenderOptions,
    create_event_bus_for,
};
use serde_json::{Value, json};
use std::{cell::RefCell, future::Future, pin::Pin, rc::Rc};
use tokio::sync::mpsc;

use super::retained_components::loaded_on;

/// Ordered entries recorded by native listeners.
pub(super) type Trace = Rc<RefCell<Vec<String>>>;
/// Payloads forwarded unchanged by authored listeners.
pub(super) type Payloads = Rc<RefCell<Vec<Value>>>;
/// A listener continuation handed to the bus's spawner.
pub(super) type Queued = Pin<Box<dyn Future<Output = ()>>>;

/// One loaded instance and the bus identity it registered with.
pub(super) struct Instance {
    /// Registrations of the instance.
    pub(super) owner: ExtensionHandle<Local>,
    /// Runner over only this instance.
    pub(super) runner: Rc<ExtensionRunner<Local>>,
    /// The instance's own bus identity.
    pub(super) events: EventBus<Local>,
}

/// Two instances of the author fixture on one bus, with native observers.
pub(super) struct Pair {
    /// Shared bus controller.
    pub(super) bus: EventBusController<Local>,
    /// Instance labelled `a`.
    pub(super) a: Instance,
    /// Instance labelled `b`.
    pub(super) b: Instance,
    /// Trace entries of both instances.
    pub(super) trace: Trace,
    /// Payloads forwarded by either instance.
    pub(super) payloads: Payloads,
    /// Continuations in the order the bus handed them over.
    pub(super) tails: mpsc::UnboundedReceiver<Queued>,
}

impl Pair {
    /// Loads both instances; listener errors panic, continuations queue.
    pub(super) async fn new() -> Self {
        Self::reporting(Rc::new(|error| panic!("{error}"))).await
    }

    /// Loads both instances with listener diagnostics sent to `report`.
    pub(super) async fn reporting(report: ErrorReporter<Local>) -> Self {
        let (sender, tails) = mpsc::unbounded_channel();
        let bus =
            create_event_bus_for::<Local>(Rc::new(move |tail| sender.send(tail).unwrap()), report);
        let (trace, payloads) = (Trace::default(), Payloads::default());
        let a = Instance::labelled(bus.for_extension(), "a").await;
        let b = Instance::labelled(bus.for_extension(), "b").await;
        let pair = Self {
            bus,
            a,
            b,
            trace,
            payloads,
            tails,
        };
        pair.observe();
        pair
    }

    /// Registers each instance's trace and payload observers under that instance's identity.
    pub(super) fn observe(&self) {
        observe(&self.a.events, "a", &self.trace, &self.payloads);
        observe(&self.b.events, "b", &self.trace, &self.payloads);
    }

    /// Returns trace entries recorded since the previous call.
    pub(super) fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.trace.borrow_mut())
    }

    /// Returns every continuation handed over so far, in order.
    pub(super) fn queued(&mut self) -> Vec<Queued> {
        std::iter::from_fn(|| self.tails.try_recv().ok()).collect()
    }
}

impl Instance {
    /// Loads one instance and assigns the label its trace entries carry.
    async fn labelled(events: EventBus<Local>, label: &str) -> Self {
        let (owner, runner, events) = loaded_on(events).await;
        runner.set_flag_value("bridge-label".into(), FlagValue::String(label.into()));
        Self {
            owner,
            runner: Rc::new(runner),
            events,
        }
    }

    /// Runs authored steps in this instance's command.
    pub(super) async fn program(&self, steps: &str) {
        let command = self
            .runner
            .get_command("bridge-observe")
            .unwrap()
            .command
            .handler;
        command(
            format!("listen:{steps}"),
            self.runner.create_command_context(),
        )
        .await
        .unwrap();
    }

    /// Reports whether this instance released the retained view named `name`.
    pub(super) fn released(&self, name: &str) -> bool {
        self.owner
            .read()
            .unwrap()
            .flags
            .contains_key(&format!("dropped-{name}"))
    }

    /// Makes one later Store call, after which queued resource releases have run.
    pub(super) async fn barrier(&self) {
        let command = self
            .runner
            .get_command("bridge-observe")
            .unwrap()
            .command
            .handler;
        let error = command("barrier".into(), self.runner.create_command_context())
            .await
            .unwrap_err();
        assert!(error.to_string().starts_with("command:barrier|"), "{error}");
    }
}

/// Records one instance's channels under its own identity, so its entries are immediate.
fn observe(events: &EventBus<Local>, label: &str, trace: &Trace, payloads: &Payloads) {
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
    let forwarded = payloads.clone();
    events.on(
        &format!("payload:{label}"),
        Rc::new(move |data, _| {
            forwarded.borrow_mut().push((*data).clone());
            Ok(None)
        }),
    );
}

/// Runs a current-thread test inside a local task set.
pub(super) fn local(test: impl Future<Output = ()>) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, test);
}

/// Steps subscribing one listener per `(channel, id, extra)` entry.
fn listen(entries: &[(&str, &str, Value)]) -> String {
    let steps: Vec<Value> = entries
        .iter()
        .map(|(channel, id, extra)| {
            let mut step = json!({"op":"on", "channel":channel, "id":id, "name":id});
            step.as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            step
        })
        .collect();
    Value::from(steps).to_string()
}

/// Steps emitting each `(channel, data)` in order.
fn emits(entries: &[(&str, Value)]) -> String {
    let steps: Vec<Value> = entries
        .iter()
        .map(|(channel, data)| json!({"op":"emit", "channel":channel, "data":data}))
        .collect();
    Value::from(steps).to_string()
}

#[test]
fn foreign_component_delivery_follows_each_callback_return() {
    local(async {
        let mut pair = Pair::new().await;
        let renderer = pair
            .a
            .runner
            .get_message_renderer_call::<Value>("order-view")
            .unwrap();
        let message = serde_json::from_value(json!({"role":"custom", "customType": "order-foreign", "content": "literal content", "display": true, "timestamp": 1.5})).unwrap();
        let component = renderer
            .call(
                Rc::new(message),
                MessageRenderOptions { expanded: false },
                maestro_theme::LiveTheme::default(),
            )
            .unwrap()
            .unwrap();
        pair.trace.borrow_mut().push("caller:return".into());
        assert_eq!(
            pair.take(),
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
        assert!(!pair.a.released("order-foreign"));
        for tail in pair.queued() {
            tail.await;
        }
        assert_eq!(
            pair.take(),
            ["a:inner:tail", "b:back:tail", "a:back:tail", "b:inner:tail"]
        );
        drop(component);
        pair.a.barrier().await;
        assert!(pair.a.released("order-foreign"));
    });
}

#[test]
fn ordinary_component_emit_finishes_prefixes_before_return() {
    local(async {
        let mut pair = Pair::new().await;
        let ready = json!({"tail":"ready"});
        pair.b
            .program(&listen(&[("x", "a", ready.clone()), ("x", "b", ready)]))
            .await;
        pair.a.program(&emits(&[("x", Value::Null)])).await;
        assert_eq!(pair.take(), ["a:prefix", "b:prefix", "emit:return"]);
        let tails = pair.queued();
        assert_eq!(tails.len(), 2);
        for tail in tails {
            tail.await;
        }
        assert_eq!(pair.take(), ["a:tail", "b:tail"]);
        pending_tail_does_not_block_a_ready_one(&mut pair).await;
        typed_event_and_tool_emits_finish_prefixes(&mut pair).await;
        channels_and_payloads_stay_literal(&pair).await;
    });
}

/// A pending first tail neither delays a later prefix nor a later ready tail.
async fn pending_tail_does_not_block_a_ready_one(pair: &mut Pair) {
    pair.b
        .program(&listen(&[
            ("p", "a", json!({"tail":"wait"})),
            ("p", "b", json!({"tail":"open"})),
        ]))
        .await;
    pair.a.program(&emits(&[("p", Value::Null)])).await;
    assert_eq!(pair.take(), ["a:prefix", "b:prefix", "emit:return"]);
    let running: Vec<_> = pair
        .queued()
        .into_iter()
        .map(tokio::task::spawn_local)
        .collect();
    assert_eq!(running.len(), 2);
    for task in running {
        task.await.unwrap();
    }
    assert_eq!(pair.take(), ["b:tail", "a:tail"]);
}

/// Emits from a typed event handler and a tool execution also run every prefix first.
async fn typed_event_and_tool_emits_finish_prefixes(pair: &mut Pair) {
    pair.b
        .program(&listen(&[
            ("typed", "typed", json!({})),
            ("tool", "tool", json!({})),
        ]))
        .await;
    pair.a
        .program(&json!([{"op":"typed", "channel":"typed"}, {"op":"tool"}]).to_string())
        .await;
    let _errors = pair.a.runner.on_error(Rc::new(|_| Ok(())));
    let mut event = maestro_extensions::SessionStartEvent {
        reason: maestro_extensions::SessionStartReason::Startup,
        previous_session_file: maestro_extensions::Presence::Missing,
    };
    pair.a.runner.emit(&mut event).await.unwrap();
    assert_eq!(pair.take(), ["typed:prefix", "emit:return"]);
    let tools = pair.a.runner.get_all_registered_tools();
    let tool = tools
        .iter()
        .find(|tool| tool.definition.metadata().definition.name == "bridge-emit")
        .unwrap();
    let wrapped = maestro_extensions::wrap_registered_tool(tool, pair.a.runner.clone());
    (wrapped.execute)(
        "call".into(),
        json!({"channel":"tool", "data":null}),
        None,
        None,
    )
    .await
    .unwrap();
    assert_eq!(pair.take(), ["tool:prefix", "emit:return"]);
    assert!(pair.queued().is_empty());
}

/// Literal channels, near misses and every payload root reach only their own listeners.
async fn channels_and_payloads_stay_literal(pair: &Pair) {
    let channels = [
        "",
        "a:b",
        "\u{96ea}/\u{1f6a6}",
        "\u{feff}",
        "\u{85}",
        "__proto__",
        "constructor",
        "error",
        "newListener",
        "removeListener",
    ];
    let forward = json!({"record":"payload"});
    let mut listeners: Vec<_> = channels
        .iter()
        .map(|channel| (*channel, *channel, forward.clone()))
        .collect();
    let near = [" ", "a:b ", "\u{96ea}", "\u{feff}\u{feff}", "Constructor"];
    listeners.extend(near.iter().map(|channel| (*channel, *channel, json!({}))));
    listeners.push(("payload", "payload", forward.clone()));
    listeners.push(("literal", "literal", forward));
    pair.b.program(&listen(&listeners)).await;
    let data = json!({"2":"two","10":"ten","z":[null,false,0,""],"a":{"x":"\u{96ea}"}});
    let sent: Vec<_> = channels
        .iter()
        .map(|channel| (*channel, data.clone()))
        .collect();
    pair.a.program(&emits(&sent)).await;
    let roots = [
        json!(null),
        json!(false),
        json!(true),
        json!(0),
        json!(-1),
        json!(1.25),
        json!(""),
        json!([null, false, "\u{96ea}"]),
        json!({"missing":false,"present":null}),
    ];
    let sent: Vec<_> = roots.iter().map(|root| ("payload", root.clone())).collect();
    pair.a.program(&emits(&sent)).await;
    pair.a
        .program(r#"[{"op":"emit","channel":"literal","data":{"z":-0,"a":"雪"}}]"#)
        .await;
    assert_eq!(pair.take(), ["emit:return"; 20]);
    let received = std::mem::take(&mut *pair.payloads.borrow_mut());
    let text = |values: &[Value]| values.iter().map(Value::to_string).collect::<Vec<_>>();
    assert_eq!(text(&received[..10]), vec![data.to_string(); 10]);
    assert_eq!(text(&received[10..19]), text(&roots));
    let literal = received[19].as_object().unwrap();
    assert_eq!(literal.keys().collect::<Vec<_>>(), ["z", "a"]);
    assert!(literal["z"].as_f64().unwrap().is_sign_negative());
    assert_eq!(received.len(), 20);
}

#[test]
fn pending_listener_tail_can_resume_its_emitter() {
    local(async {
        let mut pair = Pair::new().await;
        let (entered, mut waiting) = mpsc::unbounded_channel();
        pair.a.runner.bind_command_context(Some(
            maestro_extensions::ExtensionCommandContextActions::new(
                maestro_extensions::CommandContextOperations {
                    wait_for_idle: Rc::new(move || {
                        let (resume, resumed) = tokio::sync::oneshot::channel();
                        entered.send(resume).unwrap();
                        Box::pin(async move {
                            resumed.await?;
                            Ok(())
                        })
                    }),
                    ..crate::context_actions::silent_command_operations()
                },
            ),
        ));
        let resume = Rc::new(RefCell::new(None::<tokio::sync::oneshot::Sender<()>>));
        let done = resume.clone();
        pair.bus.on(
            "done",
            Rc::new(move |_, _| {
                done.borrow_mut().take().unwrap().send(()).unwrap();
                Ok(None)
            }),
        );
        pair.b
            .program(&listen(&[("x", "listener", json!({"tail":"done"}))]))
            .await;
        let steps = json!([{"op":"emit-and-wait", "channel":"x"}]).to_string();
        let emitter = pair.a.program(&steps);
        let tails = &mut pair.tails;
        futures_util::join!(emitter, async {
            *resume.borrow_mut() = Some(waiting.recv().await.unwrap());
            tails.try_recv().unwrap().await;
        });
        assert_eq!(
            pair.take(),
            [
                "listener:prefix",
                "emit:return",
                "listener:tail",
                "emitter:resumed",
            ]
        );
        tail_emission_runs_ordinary_prefixes(&mut pair).await;
    });
}

/// A continuation's ordinary emission runs listener prefixes before it continues.
async fn tail_emission_runs_ordinary_prefixes(pair: &mut Pair) {
    let observed = pair.trace.clone();
    pair.bus.on(
        "inner",
        Rc::new(move |_, _| {
            observed.borrow_mut().push("inner:prefix".into());
            Ok(None)
        }),
    );
    pair.b
        .program(&listen(&[(
            "outer",
            "outer",
            json!({"tail":{"emit":"inner"}}),
        )]))
        .await;
    pair.a.program(&emits(&[("outer", Value::Null)])).await;
    assert_eq!(pair.take(), ["outer:prefix", "emit:return"]);
    for tail in pair.queued() {
        tail.await;
    }
    assert_eq!(pair.take(), ["inner:prefix", "outer:tail"]);
}

#[test]
fn callback_snapshot_edits_apply_to_the_next_emission() {
    local(async {
        let pair = Pair::new().await;
        let off = |id: &str| json!({"op":"off", "id":id});
        let named =
            |id: &str| json!({"op":"on", "channel":"dup", "id":id, "name":"f", "record":"data"});
        pair.b
            .program(&json!([named("fa"), named("fb")]).to_string())
            .await;
        pair.a.program(&emits(&[("empty", json!(0))])).await;
        pair.a.program(&emits(&[("dup", json!(1))])).await;
        pair.b
            .program(&json!([off("fa"), off("fa")]).to_string())
            .await;
        pair.a.program(&emits(&[("dup", json!(2))])).await;
        pair.b.program(&json!([off("fb")]).to_string()).await;
        pair.a.program(&emits(&[("dup", json!(3))])).await;
        assert_eq!(
            pair.take(),
            [
                "emit:return",
                "f1",
                "f1",
                "emit:return",
                "f2",
                "emit:return",
                "emit:return"
            ]
        );
        channels_clear_and_accept_new_listeners(&pair).await;
        guest_edits_wait_for_the_next_emission(&pair).await;
        snapshot_clear_keeps_the_current_emission(&pair).await;
        queued_foreign_delivery_keeps_its_snapshot(&pair).await;
    });
}

/// Clearing removes every channel; a later subscription receives only later emissions.
async fn channels_clear_and_accept_new_listeners(pair: &Pair) {
    let data = json!({"record":"data"});
    pair.b
        .program(&listen(&[
            ("cx", "x", data.clone()),
            ("cy", "y", data.clone()),
        ]))
        .await;
    pair.a
        .program(&emits(&[("cx", json!(1)), ("cy", json!(2))]))
        .await;
    pair.bus.clear();
    pair.bus.clear();
    pair.observe();
    pair.a.program(&emits(&[("cx", json!(3))])).await;
    pair.b.program(&listen(&[("cx", "new", data)])).await;
    pair.a.program(&emits(&[("cx", json!(4))])).await;
    assert_eq!(
        pair.take(),
        [
            "x1",
            "emit:return",
            "y2",
            "emit:return",
            "emit:return",
            "new4",
            "emit:return"
        ]
    );
}

/// Subscriptions added or removed by a listener apply from the next emission on.
async fn guest_edits_wait_for_the_next_emission(pair: &Pair) {
    let edits = |edits: Value| json!({"record":"data", "edits": edits});
    let on = |channel: &str, id: &str| json!({"op":"on", "channel":channel, "id":id, "name":"c", "record":"data"});
    let off = |id: &str| json!({"op":"off", "id":id});
    let data = json!({"record":"data"});
    let cases = [
        (
            "add",
            json!([on("add", "add-c")]),
            vec!["a1", "b1", "a2", "b2", "c2"],
        ),
        ("remove", json!([off("remove-b")]), vec!["a1", "b1", "a2"]),
    ];
    for (channel, steps, expected) in cases {
        let a = edits(steps);
        pair.b
            .program(&listen(&[
                (channel, &format!("{channel}-a"), a),
                (channel, &format!("{channel}-b"), data.clone()),
            ]))
            .await;
        pair.a
            .program(&emits(&[(channel, json!(1)), (channel, json!(2))]))
            .await;
        let entries: Vec<String> = pair
            .take()
            .into_iter()
            .filter(|entry| entry != "emit:return")
            .map(|entry| entry.replacen(&format!("{channel}-"), "", 1))
            .collect();
        assert_eq!(entries, expected, "{channel}");
    }
    let single = json!([off("single-a"), {"op":"on", "channel":"single", "id":"single-b", "name":"b", "record":"data"}]);
    pair.b
        .program(&listen(&[("single", "single-a", edits(single))]))
        .await;
    pair.a
        .program(&emits(&[("single", json!(1)), ("single", json!(2))]))
        .await;
    assert_eq!(
        pair.take(),
        ["single-a1", "emit:return", "b2", "emit:return"]
    );
    let nested = json!([off("nested-b"), on("nested", "nested-c"), {"op":"emit", "channel":"nested", "data":2}]);
    pair.b
        .program(&listen(&[
            ("nested", "nested-a", edits(nested)),
            ("nested", "nested-b", data),
        ]))
        .await;
    pair.a.program(&emits(&[("nested", json!(1))])).await;
    assert_eq!(
        pair.take(),
        ["nested-a1", "nested-a2", "c2", "nested-b1", "emit:return"]
    );
}

/// A host clear during delivery leaves the current snapshot intact and empties the next one.
async fn snapshot_clear_keeps_the_current_emission(pair: &Pair) {
    let (bus, trace, payloads) = (pair.bus.clone(), pair.trace.clone(), pair.payloads.clone());
    let (a, b) = (pair.a.events.clone(), pair.b.events.clone());
    let first = std::cell::Cell::new(true);
    pair.bus.for_extension().on(
        "sc",
        Rc::new(move |data, _| {
            trace.borrow_mut().push(format!("a{data}"));
            if first.replace(false) {
                bus.clear();
                observe(&a, "a", &trace, &payloads);
                observe(&b, "b", &trace, &payloads);
            }
            Ok(None)
        }),
    );
    pair.b
        .program(&listen(&[("sc", "b", json!({"record":"data"}))]))
        .await;
    pair.a
        .program(&emits(&[("sc", json!(1)), ("sc", json!(2))]))
        .await;
    assert_eq!(pair.take(), ["a1", "b1", "emit:return", "emit:return"]);
}

/// A foreign delivery queued by a callback keeps the listener the callback's emission saw.
async fn queued_foreign_delivery_keeps_its_snapshot(pair: &Pair) {
    pair.b
        .program(&listen(&[("snap", "foreign", json!({"record":"data"}))]))
        .await;
    let steps = json!([{"op":"emit", "channel":"snap", "data":1}, {"op":"emit", "channel":"snap-edit", "data":null}]);
    pair.a
        .program(&listen(&[(
            "snap-run",
            "deferred",
            json!({"edits": steps, "end": true}),
        )]))
        .await;
    let (bus, trace, payloads) = (pair.bus.clone(), pair.trace.clone(), pair.payloads.clone());
    let (a, b) = (pair.a.events.clone(), pair.b.events.clone());
    pair.a.events.on(
        "snap-edit",
        Rc::new(move |_, _| {
            bus.clear();
            observe(&a, "a", &trace, &payloads);
            observe(&b, "b", &trace, &payloads);
            let fresh = trace.clone();
            bus.on(
                "snap",
                Rc::new(move |data, _| {
                    fresh.borrow_mut().push(format!("new{data}"));
                    Ok(None)
                }),
            );
            Ok(None)
        }),
    );
    pair.bus.emit("snap-run", Value::Null).await;
    pair.bus.emit("snap", json!(2)).await;
    assert_eq!(
        pair.take(),
        ["deferred:prefix", "deferred:end", "foreign1", "new2"]
    );
}

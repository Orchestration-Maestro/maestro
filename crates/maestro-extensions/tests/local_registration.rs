//! Single-thread registration, query and channel delivery.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use maestro_extensions::{
    EventBus, EventBusController, EventError, EventListener, Extension, ExtensionAPI,
    ExtensionContext, ExtensionRunner,
    types::{
        ExtensionCollection, ExtensionFlagOptions, ExtensionHandle, ExtensionRuntimeState,
        ExtensionShortcutOptions, FlagType, RegisteredCommandOptions, ResolvedCommand,
    },
};
use maestro_models::{Execution, Local};
use maestro_theme::LiveTheme;
use maestro_tui::{AutocompleteItem, Input};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{Arc, RwLock},
};
use tokio::{sync::oneshot, task::yield_now};

#[path = "support/local.rs"]
mod local_support;
use local_support::{
    Loaded, LocalBus, Released, STALE, Trace, listener, loaded, local, mark, runner, source,
};

/// The terminal object every local callback shares.
type Ui = Rc<Input>;
/// A local callback future.
type LocalFuture<T> = <Local as Execution>::Future<'static, T>;

/// One ordered completion candidate.
fn item(value: &str, label: &str, description: Option<&str>) -> AutocompleteItem {
    AutocompleteItem {
        value: value.into(),
        label: label.into(),
        description: description.map(Into::into),
    }
}

/// Completion results selected by prefix.
fn completions(prefix: &str) -> Result<Option<Vec<AutocompleteItem>>, EventError> {
    match prefix {
        "" => Ok(None),
        "\u{feff}" => Ok(Some(Vec::new())),
        "\u{85}" => Err("completion failed".into()),
        _ => Ok(Some(vec![
            item("b", "B", Some("second")),
            item("a", "A", None),
        ])),
    }
}

/// Registers a command whose callbacks capture and edit the terminal object.
fn register_edit(api: &ExtensionAPI<Local>, input: &Ui) {
    let (ui, handler_ui) = (input.clone(), input.clone());
    api.register_command(
        "edit".into(),
        RegisteredCommandOptions {
            description: Some("edit text".into()),
            get_argument_completions: Some(Rc::new(move |prefix| {
                let ui = ui.clone();
                Box::pin(async move {
                    yield_now().await;
                    ui.set_value(format!("completing {prefix}"));
                    completions(&prefix)
                })
            })),
            handler: Rc::new(move |args, ctx| {
                let ui = handler_ui.clone();
                Box::pin(async move {
                    yield_now().await;
                    assert_eq!(ctx.cwd()?, "/work");
                    if args == "fail" {
                        return Err("command failed".into());
                    }
                    ui.set_value(args);
                    Ok(())
                })
            }),
        },
    )
    .unwrap();
    api.register_command(
        "bare".into(),
        RegisteredCommandOptions {
            description: None,
            get_argument_completions: None,
            handler: Rc::new(|_, _| Box::pin(std::future::ready(Ok(())))),
        },
    )
    .unwrap();
}

/// Registers a shortcut that edits its supplied terminal context.
fn register_edit_shortcut(api: &ExtensionAPI<Local>, input: &Ui) {
    let ui = input.clone();
    api.register_shortcut(
        "ctrl+y".into(),
        ExtensionShortcutOptions {
            description: None,
            handler: Rc::new(move |ctx: ExtensionContext| {
                let ui = ui.clone();
                Box::pin(async move {
                    yield_now().await;
                    assert_eq!(ctx.cwd()?, "/work");
                    if ui.get_value() == "shortcut" {
                        return Err("shortcut failed".into());
                    }
                    ui.set_value("shortcut".into());
                    Ok(())
                })
            }),
        },
    )
    .unwrap();
}

/// Invokes the resolved command and its completions, observing the terminal object.
async fn check_edit_command(runner: &ExtensionRunner<Local>, input: &Ui) {
    let command = runner.get_command("edit").unwrap();
    assert_eq!(command.description.as_deref(), Some("edit text"));
    let bare = runner.get_command("bare").unwrap();
    assert!(bare.description.is_none());
    assert!(bare.get_argument_completions.is_none());
    let raw = " \u{feff}雪 \u{85}a ";
    (command.handler)(raw.into(), runner.create_command_context())
        .await
        .unwrap();
    assert_eq!(input.get_value(), raw);
    let error = (command.handler)("fail".into(), runner.create_command_context())
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), "command failed");
    assert_eq!(input.get_value(), raw);
    check_edit_completions(&command, input).await;
}

/// Distinguishes absent, empty, ordered and failed completions.
async fn check_edit_completions(command: &ResolvedCommand<Local>, input: &Ui) {
    let complete = command.get_argument_completions.clone().unwrap();
    assert_eq!(complete(String::new()).await.unwrap(), None);
    assert_eq!(input.get_value(), "completing ");
    assert_eq!(complete("\u{feff}".into()).await.unwrap(), Some(Vec::new()));
    assert_eq!(input.get_value(), "completing \u{feff}");
    let ordered = vec![item("b", "B", Some("second")), item("a", "A", None)];
    assert_eq!(complete("x ".into()).await.unwrap(), Some(ordered));
    assert_eq!(input.get_value(), "completing x ");
    let error = complete("\u{85}".into()).await.unwrap_err();
    assert_eq!(error.to_string(), "completion failed");
    assert_eq!(input.get_value(), "completing \u{85}");
}

#[test]
fn local_registered_callbacks_retain_terminal_objects() {
    local(async {
        let input: Ui = Rc::new(Input::new());
        let bus = LocalBus::new();
        let loaded = loaded("/local", bus.controller.for_extension());
        register_edit(&loaded.api, &input);
        register_edit_shortcut(&loaded.api, &input);
        let runner = runner(&loaded.owners);
        check_edit_command(&runner, &input).await;
        let shortcuts = runner.get_shortcuts(&Vec::new());
        let (description, handler) = {
            let shortcut = shortcuts["ctrl+y"].read().unwrap();
            (shortcut.description.clone(), shortcut.handler.clone())
        };
        assert!(description.is_none());
        handler(runner.create_context()).await.unwrap();
        assert_eq!(input.get_value(), "shortcut");
        let error = handler(runner.create_context()).await.unwrap_err();
        assert_eq!(error.to_string(), "shortcut failed");
        assert_eq!(input.get_value(), "shortcut");
    });
}

/// A command recording its label and arguments after a suspension.
fn recording_command(
    trace: &Trace,
    input: &Ui,
    label: &'static str,
) -> RegisteredCommandOptions<Local> {
    let (trace, input) = (trace.clone(), input.clone());
    RegisteredCommandOptions {
        description: Some(label.into()),
        get_argument_completions: None,
        handler: Rc::new(move |args, ctx| {
            let input = input.clone();
            let trace = trace.clone();
            Box::pin(async move {
                yield_now().await;
                assert_eq!(ctx.cwd()?, "/work");
                input.set_value(format!("{label}:{args}"));
                mark(&trace, format!("{label}:{args}"));
                Ok(())
            })
        }),
    }
}

/// A shortcut editing `input` and recording its label after a suspension.
fn recording_shortcut(
    trace: &Trace,
    input: &Ui,
    label: &'static str,
) -> ExtensionShortcutOptions<Local> {
    let (trace, input) = (trace.clone(), input.clone());
    ExtensionShortcutOptions {
        description: Some(label.into()),
        handler: Rc::new(move |_: ExtensionContext| {
            let (trace, input) = (trace.clone(), input.clone());
            Box::pin(async move {
                yield_now().await;
                input.set_value(label.into());
                mark(&trace, label);
                Ok(())
            })
        }),
    }
}

/// Invokes each current resolved command and the selected shortcut.
async fn invoke_current(runner: &ExtensionRunner<Local>, input: &Ui) {
    for (name, label) in [
        ("dup:1", "a-new"),
        ("dup:2", "b-dup"),
        ("dup:1:2", "b-literal"),
    ] {
        let command = runner.get_command(name).unwrap();
        assert_eq!(command.description.as_deref(), Some(label));
        (command.handler)(name.into(), runner.create_command_context())
            .await
            .unwrap();
        assert_eq!(input.get_value(), format!("{label}:{name}"));
    }
    let shortcuts = runner.get_shortcuts(&Vec::new());
    assert_eq!(shortcuts.keys().collect::<Vec<_>>(), ["ctrl+k", "ctrl+j"]);
    let handler = shortcuts["ctrl+k"].read().unwrap().handler.clone();
    handler(runner.create_context()).await.unwrap();
}

#[test]
fn local_registration_keeps_order_and_retained_callbacks() {
    local(async {
        let (trace, input): (Trace, Ui) = (Trace::default(), Rc::new(Input::new()));
        let bus = LocalBus::new();
        let a = loaded("/a", bus.controller.for_extension());
        let shortcut = |label| recording_shortcut(&trace, &input, label);
        let command = |label| recording_command(&trace, &input, label);
        a.api
            .register_command("dup".into(), command("a-old"))
            .unwrap();
        a.api
            .register_command("other".into(), command("other"))
            .unwrap();
        a.api
            .register_shortcut("ctrl+k".into(), shortcut("key-old"))
            .unwrap();
        a.api
            .register_shortcut("ctrl+j".into(), shortcut("key-j"))
            .unwrap();
        let runner = runner(&a.owners);
        let old = runner.get_command("dup").unwrap();
        let old_key = runner.get_shortcuts(&Vec::new())["ctrl+k"]
            .read()
            .unwrap()
            .handler
            .clone();
        a.api
            .register_command("dup".into(), command("a-new"))
            .unwrap();
        a.api
            .register_shortcut("ctrl+k".into(), shortcut("key-new"))
            .unwrap();
        let b = loaded("/b", bus.controller.for_extension());
        a.owners.write().unwrap().push(b.owner.clone());
        b.api
            .register_command("dup".into(), command("b-dup"))
            .unwrap();
        b.api
            .register_command("dup:1".into(), command("b-literal"))
            .unwrap();
        check_resolved_names(&runner);
        invoke_current(&runner, &input).await;
        (old.handler)("old".into(), runner.create_command_context())
            .await
            .unwrap();
        assert_eq!(input.get_value(), "a-old:old");
        old_key(runner.create_context()).await.unwrap();
        let expected = [
            "a-new:dup:1",
            "b-dup:dup:2",
            "b-literal:dup:1:2",
            "key-new",
            "a-old:old",
            "key-old",
        ];
        assert_eq!(*trace.borrow(), expected);
        retained_sources_follow_owner(&a, &runner, &old);
    });
}

/// Edits and replaces the owner's source after a command was resolved.
fn retained_sources_follow_owner(
    a: &Loaded,
    runner: &ExtensionRunner<Local>,
    old: &ResolvedCommand<Local>,
) {
    a.owner.read().unwrap().source_info.write().unwrap().path = "/a/moved".into();
    assert_eq!(old.source_info.read().unwrap().path, "/a/moved");
    a.owner.write().unwrap().source_info = source("/a/replaced");
    assert_eq!(old.source_info.read().unwrap().path, "/a/moved");
    let current = runner.get_command("dup:1").unwrap();
    assert!(Arc::ptr_eq(&current.source_info, &old.source_info));
    let trace = Trace::default();
    a.api
        .register_command(
            "late".into(),
            recording_command(&trace, &Rc::new(Input::new()), "late"),
        )
        .unwrap();
    let late = runner.get_command("late").unwrap();
    assert_eq!(late.source_info.read().unwrap().path, "/a/replaced");
}

/// Retains one registered callback alias and invokes it.
type Invoke = Box<dyn Fn() -> LocalFuture<Result<(), EventError>>>;

/// Registers a callback that captures the facade and retains its alias.
fn capture_facade(kind: &str, loaded: &Loaded, released: Released) -> Invoke {
    let captured = loaded.api.clone();
    let probe = Rc::new(move || {
        let _keep = &released;
        captured.get_flag("missing").map(drop)
    });
    if kind == "shortcut" {
        let options = ExtensionShortcutOptions::<Local> {
            description: None,
            handler: Rc::new(move |_: ExtensionContext| Box::pin(std::future::ready(probe()))),
        };
        loaded
            .api
            .register_shortcut("ctrl+y".into(), options)
            .unwrap();
        let handler = loaded.owner.read().unwrap().shortcuts["ctrl+y"]
            .read()
            .unwrap()
            .handler
            .clone();
        let context = runner(&Rc::new(RwLock::new(Vec::new()))).create_context();
        return Box::new(move || handler(context.clone()));
    }
    let options = if kind == "command" {
        RegisteredCommandOptions::<Local> {
            description: None,
            get_argument_completions: None,
            handler: Rc::new(move |_, _| Box::pin(std::future::ready(probe()))),
        }
    } else {
        RegisteredCommandOptions::<Local> {
            description: None,
            get_argument_completions: Some(Rc::new(move |_| {
                Box::pin(std::future::ready(probe().map(|()| None)))
            })),
            handler: Rc::new(|_, _| Box::pin(std::future::ready(Ok(())))),
        }
    };
    loaded
        .api
        .register_command("captured".into(), options)
        .unwrap();
    let command = loaded.owner.read().unwrap().commands["captured"]
        .read()
        .unwrap()
        .clone();
    if kind == "command" {
        let ctx = runner(&Rc::new(RwLock::new(Vec::new()))).create_command_context();
        return Box::new(move || (command.handler)(String::new(), ctx.clone()));
    }
    let complete = command.get_argument_completions.unwrap();
    Box::new(move || {
        let result = complete(String::new());
        Box::pin(async move { result.await.map(drop) })
    })
}

/// Releases the final strong owner while a callback alias survives.
async fn captured_facade_releases_owner(kind: &str) {
    let bus = LocalBus::new();
    let loaded = loaded("/captured", bus.controller.for_extension());
    let released = Rc::new(Cell::new(false));
    let invoke = capture_facade(kind, &loaded, Released(released.clone()));
    invoke().await.unwrap();
    let weak = Rc::downgrade(&loaded.owner);
    let Loaded {
        owner, api, owners, ..
    } = loaded;
    drop((owner, owners));
    assert!(weak.upgrade().is_none(), "{kind} facade kept its owner");
    assert_eq!(invoke().await.unwrap_err().to_string(), STALE);
    let flag = ExtensionFlagOptions {
        description: None,
        r#type: FlagType::Boolean,
        default: None,
    };
    assert_eq!(
        api.register_flag("late".into(), flag)
            .unwrap_err()
            .to_string(),
        STALE
    );
    assert!(!released.get(), "{kind} alias released early");
    drop(invoke);
    assert!(released.get(), "{kind} alias kept its captures");
}

/// Rejects every registration after invalidation without changing the owner.
fn invalidated_registration_has_no_effect(message: Option<&str>) {
    let bus = LocalBus::new();
    let loaded = loaded("/stale", bus.controller.for_extension());
    loaded.runtime.invalidate(message.map(Into::into));
    loaded.runtime.invalidate(Some("later".into()));
    let expected = message.unwrap_or(STALE);
    let trace = Trace::default();
    let command = loaded.api.register_command(
        "c".into(),
        recording_command(&trace, &Rc::new(Input::new()), "c"),
    );
    assert_eq!(command.unwrap_err().to_string(), expected);
    let shortcut = loaded.api.register_shortcut(
        "ctrl+y".into(),
        recording_shortcut(&trace, &Rc::new(Input::new()), "s"),
    );
    assert_eq!(shortcut.unwrap_err().to_string(), expected);
    let flag = ExtensionFlagOptions {
        description: None,
        r#type: FlagType::String,
        default: Some(maestro_extensions::types::FlagValue::String(String::new())),
    };
    assert_eq!(
        loaded
            .api
            .register_flag("f".into(), flag)
            .unwrap_err()
            .to_string(),
        expected
    );
    let owner = loaded.owner.read().unwrap();
    assert!(owner.commands.is_empty() && owner.shortcuts.is_empty() && owner.flags.is_empty());
    assert!(loaded.runtime.flag_values.read().unwrap().is_empty());
}

#[test]
fn local_captured_facades_do_not_keep_registration_alive() {
    local(async {
        for kind in ["command", "completion", "shortcut"] {
            captured_facade_releases_owner(kind).await;
        }
    });
    for message in [Some(""), Some("custom"), None] {
        invalidated_registration_has_no_effect(message);
    }
}

/// Completion gates of the tail witness.
struct TailGates {
    /// Releases the pending tail.
    release: oneshot::Receiver<()>,
    /// Signals that the ready tail finished.
    ready: oneshot::Sender<()>,
    /// Signals that the pending tail finished.
    done: oneshot::Sender<()>,
}

/// A listener whose tail records completion and signals its gate.
fn ready_tail(trace: &Trace, gate: oneshot::Sender<()>) -> EventListener<Local> {
    let (trace, gate) = (trace.clone(), RefCell::new(Some(gate)));
    Rc::new(move |_, _| {
        mark(&trace, "ready:prefix");
        let (trace, gate) = (trace.clone(), gate.borrow_mut().take().unwrap());
        Ok(Some(Box::pin(async move {
            mark(&trace, "ready:tail");
            gate.send(()).unwrap();
            Ok(())
        })))
    })
}

/// Installs the five ordered listeners of the tail witness.
fn install_tail_listeners(
    events: &EventBus<Local>,
    trace: &Trace,
    input: &Ui,
    gates: TailGates,
) -> maestro_extensions::Subscription<Local> {
    events.on("edit", listener(trace, "no-tail"));
    events.on(
        "edit",
        Rc::new(|_, _| Err("prefix failed\nsecond line".into())),
    );
    events.on("edit", ready_tail(trace, gates.ready));
    let (pending, ui) = (trace.clone(), input.clone());
    let gates = RefCell::new(Some((gates.release, gates.done)));
    let subscription = events.on(
        "edit",
        Rc::new(move |data, _| {
            mark(&pending, "pending:prefix");
            let (pending, ui) = (pending.clone(), ui.clone());
            let (release, done) = gates.borrow_mut().take().unwrap();
            Ok(Some(Box::pin(async move {
                release.await.unwrap();
                ui.set_value(data["text"].as_str().unwrap().into());
                mark(&pending, "pending:tail");
                done.send(()).unwrap();
                Err("tail failed".into())
            })))
        }),
    );
    events.on("edit", listener(trace, "healthy"));
    subscription
}

/// Delivers every JSON value kind through a local listener.
async fn payload_domain(events: &EventBus<Local>) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let record = seen.clone();
    events.on(
        "domain",
        Rc::new(move |data, _| {
            record.borrow_mut().push(data);
            Ok(None)
        }),
    );
    let object = json!({"2": "two", "10": "ten", "z": 1, "a": 2});
    let values = [
        Value::Null,
        json!(""),
        json!(-0.0),
        json!(true),
        object,
        json!([null, false, "雪"]),
    ];
    for value in values.clone() {
        events.emit("domain", value).await;
    }
    let seen = seen.borrow();
    assert_eq!(
        seen.iter()
            .map(|value| (**value).clone())
            .collect::<Vec<_>>(),
        values
    );
    assert!(seen[2].as_f64().unwrap().is_sign_negative());
    let keys: Vec<_> = seen[4].as_object().unwrap().keys().collect();
    assert_eq!(keys, ["2", "10", "z", "a"]);
}

#[test]
fn local_listener_tails_keep_state_errors_and_admission() {
    local(async {
        let (bus, trace, input) = (LocalBus::new(), Trace::default(), Rc::new(Input::new()));
        let events = bus.controller.for_extension();
        let (release, gate) = oneshot::channel();
        let (ready, ready_done) = oneshot::channel();
        let (done, completed) = oneshot::channel();
        let gates = TailGates {
            release: gate,
            ready,
            done,
        };
        let subscription = install_tail_listeners(&events, &trace, &input, gates);
        events.emit("edit", json!({"text": "from tail"})).await;
        mark(&trace, "return");
        let prefixes = [
            "no-tail",
            "ready:prefix",
            "pending:prefix",
            "healthy",
            "return",
        ];
        assert_eq!(*trace.borrow(), prefixes);
        let prefix_error = "Event handler error (edit): prefix failed\nsecond line\n";
        assert_eq!(*bus.errors.borrow(), [prefix_error]);
        assert_eq!(bus.spawned.get(), 2);
        ready_done.await.unwrap();
        assert_eq!(trace.borrow()[5..], ["ready:tail"]);
        subscription.unsubscribe();
        bus.controller.clear();
        release.send(()).unwrap();
        completed.await.unwrap();
        assert_eq!(input.get_value(), "from tail");
        bus.settle().await;
        assert_eq!(trace.borrow()[5..], ["ready:tail", "pending:tail"]);
        let tail_error = "Event handler error (edit): tail failed\n";
        assert_eq!(*bus.errors.borrow(), [prefix_error, tail_error]);
        payload_domain(&events).await;
        assert_eq!(bus.spawned.get(), 2);
        assert_eq!(bus.errors.borrow().len(), 2);
    });
}

/// Same-owner delivery is immediate; foreign delivery waits for callback return.
fn same_and_foreign_at_return(bus: &LocalBus, trace: &Trace) {
    let owner = bus.controller.for_extension();
    bus.controller
        .for_extension()
        .on("x", listener(trace, "foreign"));
    owner.on("x", listener(trace, "same"));
    owner.invoke_callback(|emitter| {
        mark(trace, "body:start");
        emitter.emit("x", Value::Null);
        mark(trace, "body:end");
    });
    mark(trace, "release");
    assert_eq!(
        *trace.borrow(),
        ["body:start", "same", "body:end", "foreign", "release"]
    );
}

/// A listener that marks, emits one channel through its emitter, then marks again.
fn relay(trace: &Trace, name: &'static str, channel: &'static str) -> EventListener<Local> {
    let trace = trace.clone();
    Rc::new(move |_, emitter| {
        mark(&trace, format!("{name}:start"));
        emitter.emit(channel, Value::Null);
        mark(&trace, format!("{name}:end"));
        Ok(None)
    })
}

/// Foreign re-entry from A to B and back drains at each callback return.
async fn nested_reentry(bus: &LocalBus, trace: &Trace) {
    let (a, b) = (
        bus.controller.for_extension(),
        bus.controller.for_extension(),
    );
    a.on("outer", relay(trace, "a", "inner"));
    b.on("inner", relay(trace, "b", "back"));
    a.on("back", listener(trace, "a:back"));
    bus.controller.emit("outer", Value::Null).await;
    mark(trace, "return");
    assert_eq!(
        *trace.borrow(),
        ["a:start", "a:end", "b:start", "b:end", "a:back", "return"]
    );
}

/// Nested invocations keep independent foreign queues.
fn nested_queues(bus: &LocalBus, trace: &Trace) {
    let owner = bus.controller.for_extension();
    let foreign = trace.clone();
    bus.controller.for_extension().on(
        "x",
        Rc::new(move |data, _| {
            mark(&foreign, format!("foreign:{}", data.as_str().unwrap()));
            Ok(None)
        }),
    );
    owner.invoke_callback(|outer| {
        mark(trace, "outer:start");
        outer.emit("x", "outer".into());
        owner.invoke_callback(|inner| {
            mark(trace, "inner:start");
            inner.emit("x", "inner".into());
            mark(trace, "inner:end");
        });
        mark(trace, "outer:end");
    });
    mark(trace, "return");
    let expected = [
        "outer:start",
        "inner:start",
        "inner:end",
        "foreign:inner",
        "outer:end",
        "foreign:outer",
        "return",
    ];
    assert_eq!(*trace.borrow(), expected);
}

/// Errors returned by a listener or callback keep queued foreign delivery.
async fn error_returns(bus: &LocalBus, trace: &Trace) {
    let owner = bus.controller.for_extension();
    owner.on(
        "outer",
        Rc::new(|_, emitter| {
            emitter.emit("x", Value::Null);
            Err("prefix failure".into())
        }),
    );
    bus.controller
        .for_extension()
        .on("x", listener(trace, "foreign"));
    owner.emit("outer", Value::Null).await;
    let result: Result<(), &str> = owner.invoke_callback(|emitter| {
        emitter.emit("x", Value::Null);
        Err("factory failure")
    });
    assert_eq!(result, Err("factory failure"));
    assert_eq!(*trace.borrow(), ["foreign", "foreign"]);
    assert_eq!(
        *bus.errors.borrow(),
        ["Event handler error (outer): prefix failure\n"]
    );
}

/// Queued foreign delivery keeps its snapshot while the next emission sees edits.
async fn snapshot_edits(bus: &LocalBus, trace: &Trace) {
    let (owner, foreign) = (
        bus.controller.for_extension(),
        bus.controller.for_extension(),
    );
    let old = trace.clone();
    let subscription = foreign.on(
        "x",
        Rc::new(move |data, _| {
            mark(&old, format!("foreign:{data}"));
            Ok(None)
        }),
    );
    owner.invoke_callback(|emitter| {
        mark(trace, "start");
        emitter.emit("x", 1.into());
        subscription.unsubscribe();
        bus.controller.clear();
        let new = trace.clone();
        foreign.on(
            "x",
            Rc::new(move |data, _| {
                mark(&new, format!("new:{data}"));
                Ok(None)
            }),
        );
        mark(trace, "end");
    });
    owner.emit("x", 2.into()).await;
    assert_eq!(*trace.borrow(), ["start", "end", "foreign:1", "new:2"]);
}

/// A resumed tail emits through the ordinary path, running prefixes immediately.
async fn resumed_tail_emits(bus: &LocalBus, trace: &Trace) {
    let outer = trace.clone();
    bus.controller.for_extension().on(
        "outer",
        Rc::new(move |_, emitter| {
            mark(&outer, "outer:prefix");
            let (outer, events) = (outer.clone(), emitter.events());
            Ok(Some(Box::pin(async move {
                events.emit("inner", Value::Null).await;
                mark(&outer, "outer:tail");
                Ok(())
            })))
        }),
    );
    bus.controller
        .for_extension()
        .on("inner", listener(trace, "inner:prefix"));
    bus.controller.emit("outer", Value::Null).await;
    mark(trace, "return");
    bus.settle().await;
    assert_eq!(
        *trace.borrow(),
        ["outer:prefix", "return", "inner:prefix", "outer:tail"]
    );
}

#[test]
fn local_callback_origins_keep_nested_snapshot_order() {
    local(async {
        same_and_foreign_at_return(&LocalBus::new(), &Trace::default());
        nested_reentry(&LocalBus::new(), &Trace::default()).await;
        nested_queues(&LocalBus::new(), &Trace::default());
        error_returns(&LocalBus::new(), &Trace::default()).await;
        snapshot_edits(&LocalBus::new(), &Trace::default()).await;
        resumed_tail_emits(&LocalBus::new(), &Trace::default()).await;
    });
}

/// Re-enters the public bus from its destructor.
struct Reenter {
    /// Bus kept until explicit teardown releases this capture.
    bus: EventBusController<Local>,
    /// Destruction observations.
    trace: Trace,
}

impl Drop for Reenter {
    fn drop(&mut self) {
        let probe = self.bus.on("probe", listener(&self.trace, "probe"));
        drop(self.bus.emit("probe", Value::Null));
        probe.unsubscribe();
        self.bus.clear();
        mark(&self.trace, "dropped");
    }
}

/// Removes a self-referencing listener through one teardown path.
async fn reentrant_teardown(action: &str) {
    let bus = LocalBus::new();
    let trace = Trace::default();
    let capture = Rc::new(());
    let (held, weak) = (capture.clone(), Rc::downgrade(&capture));
    let cleanup = Reenter {
        bus: bus.controller.clone(),
        trace: trace.clone(),
    };
    let controller = bus.controller.clone();
    let snapshot = action == "snapshot";
    let handle = bus.controller.on(
        "x",
        Rc::new(move |_, _| {
            let _keep = &cleanup;
            if snapshot {
                controller.clear();
            }
            let held = held.clone();
            Ok(Some(Box::pin(async move {
                drop(held);
                Ok(())
            })))
        }),
    );
    match action {
        "unsubscribe" => handle.unsubscribe(),
        "clear" => bus.controller.clear(),
        _ => bus.controller.emit("x", Value::Null).await,
    }
    assert_eq!(*trace.borrow(), ["probe", "dropped"], "{action}");
    drop((capture, handle));
    bus.settle().await;
    assert!(weak.upgrade().is_none(), "{action} kept a tail capture");
    assert_last_bus_release(bus);
}

/// Dropping the last bus alias releases its spawner and reporter captures.
fn assert_last_bus_release(bus: LocalBus) {
    let tasks = Rc::downgrade(&bus.tasks);
    let errors = Rc::downgrade(&bus.errors);
    let LocalBus {
        controller,
        tasks: held_tasks,
        errors: held_errors,
        ..
    } = bus;
    drop((held_tasks, held_errors));
    assert!(tasks.upgrade().is_some() && errors.upgrade().is_some());
    drop(controller);
    assert!(tasks.upgrade().is_none() && errors.upgrade().is_none());
}

#[test]
fn local_bus_cleanup_releases_captures_without_reentry_locks() {
    local(async {
        for action in ["unsubscribe", "clear", "snapshot"] {
            reentrant_teardown(action).await;
        }
        let bus = LocalBus::new();
        let trace = Trace::default();
        drop(bus.controller.on("x", listener(&trace, "kept")));
        bus.controller.emit("x", Value::Null).await;
        assert_eq!(*trace.borrow(), ["kept"]);
        bus.controller.clear();
        assert_last_bus_release(bus);
    });
}

/// Requires a type usable from any thread.
fn send_sync<T: Send + Sync>() {}

/// Requires a value that can move to another thread.
fn send<T: Send>(value: T) -> T {
    value
}

/// Compile-checks the default callback aliases and owner graphs.
fn default_bounds() {
    use maestro_extensions::types::{
        ArgumentCompletions, CommandHandler, ExtensionShortcut, RegisteredCommand, ResolvedCommand,
        ShortcutHandler,
    };
    use maestro_extensions::{ErrorReporter, EventTail, Subscription, TailSpawner};
    send_sync::<CommandHandler>();
    send_sync::<ShortcutHandler>();
    send_sync::<ArgumentCompletions>();
    send_sync::<EventListener>();
    send_sync::<TailSpawner>();
    send_sync::<ErrorReporter>();
    send_sync::<EventBus>();
    send_sync::<EventBusController>();
    send_sync::<Subscription>();
    send_sync::<RegisteredCommand>();
    send_sync::<ResolvedCommand>();
    send_sync::<ExtensionShortcut>();
    send_sync::<Extension>();
    send_sync::<ExtensionHandle>();
    send_sync::<ExtensionCollection>();
    send_sync::<ExtensionAPI>();
    let _ = send::<EventTail>;
}

/// Registers default callbacks and runs their futures on another thread.
fn default_callbacks_run_on_another_thread(bus: &EventBusController) {
    let owner = Arc::new(RwLock::new(Extension::<maestro_models::Threaded>::new(
        "/default".into(),
        "/resolved".into(),
        source("/default"),
    )));
    let owners = Arc::new(RwLock::new(vec![Arc::clone(&owner)]));
    let runtime = Arc::new(ExtensionRuntimeState::new());
    let api = ExtensionAPI::new(&owner, Arc::clone(&runtime), bus.for_extension());
    let options = RegisteredCommandOptions::<maestro_models::Threaded> {
        description: None,
        get_argument_completions: None,
        handler: Arc::new(|args, ctx| {
            let cwd = ctx.cwd().map(str::to_owned);
            Box::pin(async move {
                if args == cwd? {
                    Ok(())
                } else {
                    Err("mismatch".into())
                }
            })
        }),
    };
    api.register_command("hello".into(), options).unwrap();
    let options: ExtensionShortcutOptions = ExtensionShortcutOptions {
        description: None,
        handler: Arc::new(|ctx: ExtensionContext| {
            let cwd = ctx.cwd().map(str::to_owned);
            Box::pin(async move { Err(format!("pressed in {}", cwd?).into()) })
        }),
    };
    api.register_shortcut("ctrl+y".into(), options).unwrap();
    let runner = ExtensionRunner::<maestro_models::Threaded>::new(
        owners,
        runtime,
        local_support::session::runner_session("/work").0,
        Arc::new(|_| {}),
        LiveTheme::default(),
    );
    let command = runner.get_command("hello").unwrap();
    let shortcuts = runner.get_shortcuts(&Vec::new());
    let shortcut = shortcuts["ctrl+y"].read().unwrap().handler.clone();
    let invoked = send((command.handler)(
        "/work".into(),
        runner.create_command_context(),
    ));
    let pressed = send(shortcut(runner.create_context()));
    let worker = std::thread::spawn(move || {
        let executor = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let invoked = executor.block_on(invoked).is_ok();
        (invoked, executor.block_on(pressed).unwrap_err().to_string())
    });
    assert_eq!(
        worker.join().unwrap(),
        (true, "pressed in /work".to_owned())
    );
}

#[test]
fn default_registration_retains_thread_safe_sdk_contracts() {
    default_bounds();
    let errors = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let report = Arc::clone(&errors);
    let bus = maestro_extensions::create_event_bus(
        Arc::new(|tail| drop(tokio::spawn(tail))),
        Arc::new(move |line| report.lock().unwrap().push(line.to_owned())),
    );
    default_callbacks_run_on_another_thread(&bus);
    let (done, mut completed) = tokio::sync::mpsc::unbounded_channel();
    bus.on(
        "x",
        Arc::new(move |_, _| {
            let done = done.clone();
            Ok(Some(Box::pin(async move {
                done.send(()).unwrap();
                Err("tail".into())
            })))
        }),
    );
    let executor = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    executor.block_on(async {
        bus.emit("x", Value::Null).await;
        completed.recv().await.unwrap();
    });
    assert_eq!(*errors.lock().unwrap(), ["Event handler error (x): tail\n"]);
}

/// Checks collision resolution before any command invocation.
fn check_resolved_names(runner: &ExtensionRunner<Local>) {
    let names: Vec<_> = runner
        .get_registered_commands()
        .into_iter()
        .map(|command| command.invocation_name)
        .collect();
    assert_eq!(names, ["dup:1", "other", "dup:2", "dup:1:2"]);
    assert!(runner.get_command("dup").is_none());
}

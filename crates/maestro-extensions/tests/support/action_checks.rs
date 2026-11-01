//! Detailed public action and listener witnesses.
#![cfg(test)]
use super::model_support::model;
use super::*;
use crate::runtime_support;
use maestro_extensions::types::ExtensionErrorListener;
use maestro_models::SharedHandle;
use maestro_models::UserContent;
use maestro_session::{
    Presence,
    messages::{MessageDelivery, SendMessageOptions, SendUserMessageOptions, UserMessageDelivery},
};
use serde_json::{Value, json};
use std::{cell::Cell, collections::VecDeque};

pub fn prebind() {
    let runtime = create_extension_runtime::<Local>();
    let (_one, one) = local_api(runtime.clone(), "/one");
    let (_two, two) = local_api(runtime.clone(), "/two");
    one.register_provider("p1".into(), good("a")).unwrap();
    two.register_provider("p2".into(), good("b")).unwrap();
    runtime
        .register_provider("p3".into(), good("c"), None)
        .unwrap();
    two.register_provider("p1".into(), good("d")).unwrap();
    one.unregister_provider("p1").unwrap();
    assert_eq!(pending_names(&runtime), ["p2", "p3"]);
    runtime.unregister_provider("absent").unwrap();
    assert_eq!(pending_names(&runtime), ["p2", "p3"]);
    two.unregister_provider("p2").unwrap();
    one.unregister_provider("p3").unwrap();
    assert!(pending_names(&runtime).is_empty());
}

/// Native error marker preserved across the application callback boundary.
#[derive(Debug)]
struct Marker;
impl std::fmt::Display for Marker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("marker")
    }
}
impl std::error::Error for Marker {}

/// A callback can return its original native error without message rebuilding.
pub fn errors() {
    let runtime = create_extension_runtime::<Local>();
    let runner = runner_local(runtime.clone(), catalog());
    let mut actions = Local::unbound_actions();
    macro_rules! fail {
        ($field:ident, $($args:pat),*) => { actions.$field = Rc::new(|$($args),*| Err(Box::new(Marker))); };
    }
    fail!(send_message, _, _);
    fail!(send_user_message, _, _);
    fail!(append_entry, _, _);
    fail!(set_session_name, _);
    actions.get_session_name = Rc::new(|| Err(Box::new(Marker)));
    fail!(set_label, _, _);
    actions.get_active_tools = Rc::new(|| Err(Box::new(Marker)));
    actions.get_all_tools = Rc::new(|| Err(Box::new(Marker)));
    fail!(set_active_tools, _);
    actions.get_commands = Rc::new(|| Err(Box::new(Marker)));
    actions.refresh_tools = Rc::new(|| Err(Box::new(Marker)));
    actions.get_thinking_level = Rc::new(|| Err(Box::new(Marker)));
    fail!(set_thinking_level, _);
    let started = Rc::new(Cell::new(false));
    let observed = started.clone();
    actions.set_model = Rc::new(move |_| {
        observed.set(true);
        Box::pin(async { Err(Box::new(Marker) as EventError) })
    });
    runner
        .bind_core(
            actions,
            ExtensionContextActions::default(),
            ProviderActions::default(),
        )
        .unwrap();
    let refresh = runtime.actions.read().unwrap().refresh_tools.clone();
    assert!(refresh().unwrap_err().downcast_ref::<Marker>().is_some());
    let (_owner, api) = local_api(runtime, "/work");
    let results = facade_calls(&api);
    for result in results {
        assert!(result.unwrap_err().downcast_ref::<Marker>().is_some());
    }
    let future = api.set_model(model("selected")).unwrap();
    assert!(started.get());
    assert!(
        runtime_support::runtime()
            .block_on(future)
            .unwrap_err()
            .downcast_ref::<Marker>()
            .is_some()
    );
}

fn message(details: Value) -> CustomMessageInput {
    CustomMessageInput {
        custom_type: " raw\u{feff}\u{85} ".into(),
        content: UserContent::Text("hello".into()),
        display: false,
        details: Presence::Present(details),
    }
}

/// All stale calls fail synchronously before a selected operation or native command runs.
pub fn stale() {
    for bound in [false, true] {
        let runtime = create_extension_runtime::<Local>();
        let runner = runner_local(runtime.clone(), catalog());
        if bound {
            let mut actions = Local::unbound_actions();
            actions.get_session_name = Rc::new(|| Ok(Some("direct".into())));
            runner
                .bind_core(
                    actions,
                    ExtensionContextActions::default(),
                    ProviderActions::default(),
                )
                .unwrap();
        }
        let (_owner, api) = local_api(runtime.clone(), "/work");
        runtime.invalidate(Some("first stale".into()));
        runtime.invalidate(Some("second stale".into()));
        for result in facade_calls(&api).into_iter().chain([
            api.register_provider("p".into(), good("m")),
            api.unregister_provider("p"),
        ]) {
            assert_eq!(result.unwrap_err().to_string(), "first stale");
        }
        assert_eq!(
            api.set_model(model("selected")).err().unwrap().to_string(),
            "first stale"
        );
        assert_eq!(
            api.exec(
                "never-executed",
                &[],
                None,
                maestro_tools::ExecOptions::default()
            )
            .err()
            .unwrap()
            .to_string(),
            "first stale"
        );
        assert!(pending_names(&runtime).is_empty());
        let retired = std::mem::replace(
            &mut runtime.actions.write().unwrap().get_session_name,
            Rc::new(|| Ok(Some("replacement".into()))),
        );
        drop(retired);
        let get = runtime.actions.read().unwrap().get_session_name.clone();
        assert_eq!(get().unwrap().as_deref(), Some("replacement"));
    }
}

/// Actions are chosen now, while the returned model future retains its original selection.
pub fn rebind() {
    let runtime = create_extension_runtime::<Local>();
    let runner = runner_local(runtime.clone(), catalog());
    let (_owner, api) = local_api(runtime.clone(), "/work");
    let (send, receive) = tokio::sync::oneshot::channel();
    let receive = Rc::new(RefCell::new(Some(receive)));
    let started = Rc::new(Cell::new(false));
    let observed = started.clone();
    let released = Rc::new(Cell::new(false));
    let retired = ObserveDrop {
        runtime: Rc::downgrade(&runtime),
        released: released.clone(),
    };
    let mut a = Local::unbound_actions();
    a.set_model = Rc::new(move |_| {
        let _ = &retired;
        observed.set(true);
        let receive = receive.borrow_mut().take().unwrap();
        Box::pin(async {
            receive.await.unwrap();
            Ok(true)
        })
    });
    runner
        .bind_core(
            a,
            ExtensionContextActions::default(),
            ProviderActions::default(),
        )
        .unwrap();
    let first = api.set_model(model("selected")).unwrap();
    assert!(started.get());
    let mut b = Local::unbound_actions();
    b.set_model = Rc::new(|_| Box::pin(async { Ok(false) }));
    b.get_session_name = Rc::new(|| Ok(Some("B".into())));
    runner
        .bind_core(
            b,
            ExtensionContextActions::default(),
            ProviderActions::default(),
        )
        .unwrap();
    assert!(released.get());
    send.send(()).unwrap();
    assert!(runtime_support::runtime().block_on(first).unwrap());
    assert!(
        !runtime_support::runtime()
            .block_on(api.set_model(model("selected")).unwrap())
            .unwrap()
    );
    assert_eq!(api.get_session_name().unwrap().as_deref(), Some("B"));
    let (_late, late) = local_api(runtime, "/other");
    assert_eq!(late.get_session_name().unwrap().as_deref(), Some("B"));
}

/// Live listener admission/removal has identity, ordering and explicit teardown.
pub fn listeners() {
    live_removal();
    let runtime = create_extension_runtime::<Local>();
    let runner = Rc::new(runner_local(runtime.clone(), catalog()));
    let log = Rc::new(RefCell::new(Vec::new()));
    let seen = log.clone();
    let one: ExtensionErrorListener = Rc::new(move |e| {
        assert_eq!(e.stack.as_deref(), Some(""));
        seen.borrow_mut().push(1);
        Ok(())
    });
    let off = runner.on_error(one.clone());
    let duplicate = runner.on_error(one.clone());
    let seen = log.clone();
    let two: ExtensionErrorListener = Rc::new(move |_| {
        seen.borrow_mut().push(2);
        Ok(())
    });
    let remove_two = Rc::new(runner.on_error(two.clone()));
    let seen = log.clone();
    let weak_runner = Rc::downgrade(&runner);
    let remove = remove_two.clone();
    let three: ExtensionErrorListener = Rc::new(move |_| {
        seen.borrow_mut().push(3);
        remove();
        let seen = seen.clone();
        let late: ExtensionErrorListener = Rc::new(move |_| {
            seen.borrow_mut().push(4);
            Ok(())
        });
        drop(weak_runner.upgrade().unwrap().on_error(late));
        Ok(())
    });
    let remove_three = runner.on_error(three);
    let event = ExtensionError {
        extension_path: String::new(),
        event: String::new(),
        error: String::new(),
        stack: Some(String::new()),
    };
    runner.emit_error(&event).unwrap();
    assert_eq!(*log.borrow(), [1, 2, 3, 4]);
    log.borrow_mut().clear();
    off();
    off();
    duplicate();
    remove_three();
    drop(runner.on_error(one));
    runner.emit_error(&event).unwrap();
    assert_eq!(*log.borrow(), [4, 1]);
    listener_teardown(runner, &runtime, &log, &event);
}

/// Forwarded JSON is checked before serialization can erase signed zero or key order.
fn check_json(value: &Value) {
    if value.is_number() && value.as_f64() == Some(0.0) {
        assert!(value.as_f64().unwrap().is_sign_negative());
    }
    if let Some(object) = value.as_object() {
        assert_eq!(
            object.keys().map(String::as_str).collect::<Vec<_>>(),
            ["z", "a"]
        );
    }
}

/// Supplied values exercise distinct handler slots with no normalization or defaults.
pub fn forwarding() {
    model_forwarding();
    let runtime = create_extension_runtime::<Local>();
    let runner = runner_local(runtime.clone(), catalog());
    let mut actions = Local::unbound_actions();
    let log = Rc::new(RefCell::new(Vec::<Value>::new()));
    let messages = log.clone();
    actions.send_message = Rc::new(move |m, o| {
        if let Presence::Present(details) = &m.details {
            check_json(details);
        }
        messages.borrow_mut().push(json!(["custom", m, o]));
        Ok(())
    });
    let users = log.clone();
    actions.send_user_message = Rc::new(move |m, o| {
        users.borrow_mut().push(json!(["user", m, o]));
        Ok(())
    });
    let entries = Rc::new(RefCell::new(Vec::new()));
    let recorded_entries = entries.clone();
    actions.append_entry = Rc::new(move |name, data| {
        if let Some(data) = &data {
            check_json(data);
        }
        recorded_entries.borrow_mut().push((name, data));
        Ok(())
    });
    install_scalar_actions(&mut actions, &log);
    runner
        .bind_core(
            actions,
            ExtensionContextActions::default(),
            ProviderActions::default(),
        )
        .unwrap();
    let (_owner, api) = local_api(runtime, "/work");
    let payloads = [
        Value::Null,
        json!(" raw\u{feff}\u{85} "),
        json!(1.5),
        json!(-0.0),
        json!(false),
        json!({"z":1,"a":2}),
        json!([1, "a"]),
    ];
    for value in payloads {
        api.append_entry(" entry ".into(), Some(value.clone()))
            .unwrap();
        api.send_message(message(value.clone()), None).unwrap();
        let expected_entry = (" entry ".into(), Some(value.clone()));
        assert_eq!(entries.borrow().last(), Some(&expected_entry));
        let got = log.borrow();
        assert_eq!(
            got[got.len() - 1],
            json!(["custom", {"customType":" raw\u{feff}\u{85} ","content":"hello","display":false,"details":value}, null])
        );
    }
    message_absence_and_images(&api, &log, &entries);
    exercise_options(&api, &log);
    exercise_scalars(&api, &log);
}

fn install_scalar_actions(actions: &mut ExtensionActions<Local>, log: &Rc<RefCell<Vec<Value>>>) {
    let l = log.clone();
    actions.set_session_name = Rc::new(move |v| {
        l.borrow_mut().push(json!(["name", v]));
        Ok(())
    });
    let names = Rc::new(RefCell::new(VecDeque::from([
        None,
        Some(String::new()),
        Some(" name ".into()),
    ])));
    actions.get_session_name = Rc::new(move || Ok(names.borrow_mut().pop_front().unwrap()));
    let l = log.clone();
    actions.set_label = Rc::new(move |id, v| {
        l.borrow_mut().push(json!(["label", id, v]));
        Ok(())
    });
    actions.get_active_tools = Rc::new(|| Ok(vec![" z ".into(), "a".into(), "a".into()]));
    let l = log.clone();
    actions.set_active_tools = Rc::new(move |v| {
        l.borrow_mut().push(json!(["tools", v]));
        Ok(())
    });
    actions.get_commands = Rc::new(|| {
        Ok(vec![serde_json::from_value(json!({"name":" command ","description":"","source":"custom-source","sourceInfo":{"path":"p","source":"custom","scope":"project","origin":"package","baseDir":"base"}})).unwrap()])
    });
    actions.get_all_tools = Rc::new(|| Ok(vec![]));
    actions.get_thinking_level = Rc::new(|| Ok(ThinkingLevel::High));
    let l = log.clone();
    actions.set_thinking_level = Rc::new(move |v| {
        l.borrow_mut().push(json!(["thinking", v]));
        Ok(())
    });
}
fn exercise_scalars(api: &ExtensionAPI<Local>, log: &Rc<RefCell<Vec<Value>>>) {
    api.set_session_name(" name\u{feff}\u{85} ".into()).unwrap();
    assert_eq!(
        log.borrow().last().unwrap(),
        &json!(["name", " name\u{feff}\u{85} "])
    );
    for expected in [None, Some(String::new()), Some(" name ".into())] {
        assert_eq!(api.get_session_name().unwrap(), expected);
    }
    for label in [None, Some(String::new()), Some(" raw ".into())] {
        api.set_label(" id ".into(), label.clone()).unwrap();
        assert_eq!(
            log.borrow().last().unwrap(),
            &json!(["label", " id ", label])
        );
    }
    assert_eq!(api.get_active_tools().unwrap(), [" z ", "a", "a"]);
    for tools in [vec![], vec![" z ".to_owned(), "a".into(), "a".into()]] {
        api.set_active_tools(tools.clone()).unwrap();
        assert_eq!(log.borrow().last().unwrap(), &json!(["tools", tools]));
    }
    assert!(api.get_all_tools().unwrap().is_empty());
    assert_eq!(
        serde_json::to_value(api.get_commands().unwrap()).unwrap(),
        json!([{"name":" command ","description":"","source":"custom-source","sourceInfo":{"path":"p","source":"custom","scope":"project","origin":"package","baseDir":"base"}}])
    );
    assert_eq!(api.get_thinking_level().unwrap(), ThinkingLevel::High);
    for level in [
        ThinkingLevel::Off,
        ThinkingLevel::Minimal,
        ThinkingLevel::Low,
        ThinkingLevel::Medium,
        ThinkingLevel::High,
        ThinkingLevel::Xhigh,
    ] {
        api.set_thinking_level(level.clone()).unwrap();
        assert_eq!(log.borrow().last().unwrap(), &json!(["thinking", level]));
    }
}
fn exercise_options(api: &ExtensionAPI<Local>, log: &Rc<RefCell<Vec<Value>>>) {
    for delivery in [
        MessageDelivery::Steer,
        MessageDelivery::FollowUp,
        MessageDelivery::NextTurn,
    ] {
        let options = SendMessageOptions {
            trigger_turn: Some(false),
            deliver_as: Some(delivery),
        };
        api.send_message(message(Value::Null), Some(options.clone()))
            .unwrap();
        assert_eq!(
            log.borrow().last().unwrap()[2],
            serde_json::to_value(options).unwrap()
        );
    }
    for delivery in [UserMessageDelivery::Steer, UserMessageDelivery::FollowUp] {
        let options = SendUserMessageOptions {
            deliver_as: Some(delivery),
        };
        api.send_user_message(UserContent::Text(" user ".into()), Some(options.clone()))
            .unwrap();
        assert_eq!(
            log.borrow().last().unwrap(),
            &json!(["user", " user ", options])
        );
    }
    api.send_message(message(Value::Null), Some(SendMessageOptions::default()))
        .unwrap();
    assert_eq!(log.borrow().last().unwrap()[2], json!({}));
    api.send_user_message(UserContent::Text(String::new()), None)
        .unwrap();
    assert_eq!(log.borrow().last().unwrap(), &json!(["user", "", null]));
    api.send_user_message(
        UserContent::Text(String::new()),
        Some(SendUserMessageOptions::default()),
    )
    .unwrap();
    assert_eq!(log.borrow().last().unwrap(), &json!(["user", "", {}]));
}

fn listener_teardown(
    runner: Rc<ExtensionRunner<Local>>,
    runtime: &ExtensionRuntime<Local>,
    log: &Rc<RefCell<Vec<i32>>>,
    event: &ExtensionError,
) {
    log.borrow_mut().clear();
    let off_error = runner.on_error(Rc::new(|_| Err(Box::new(Marker))));
    let seen = log.clone();
    let off_last = runner.on_error(Rc::new(move |_| {
        seen.borrow_mut().push(9);
        Ok(())
    }));
    runtime.invalidate(None);
    assert!(
        runner
            .emit_error(event)
            .unwrap_err()
            .downcast_ref::<Marker>()
            .is_some()
    );
    assert_eq!(*log.borrow(), [4, 1]);
    off_error();
    off_last();
    let capture = Rc::new(());
    let weak = Rc::downgrade(&capture);
    let off_capture = runner.on_error(Rc::new(move |_| {
        let _ = &capture;
        Ok(())
    }));
    off_capture();
    assert!(weak.upgrade().is_none());
    let weak_runner = Rc::downgrade(&runner);
    drop(runner);
    assert!(weak_runner.upgrade().is_none());
    off_capture();
}

pub fn loading(api: &ExtensionAPI<Local>, runtime: &ExtensionRuntime<Local>) {
    assert!(pending_names(runtime).is_empty());
    runtime.assert_active().unwrap();
    assert!(runtime.flag_values.read().unwrap().is_empty());
    let actions = runtime.actions.read().unwrap().clone();
    let m = message(Value::Null);
    let direct = [
        (actions.send_message)(m, None),
        (actions.send_user_message)(UserContent::Text("user".into()), None),
        (actions.append_entry)("entry".into(), None),
        (actions.set_session_name)("name".into()),
        (actions.get_session_name)().map(drop),
        (actions.set_label)("id".into(), None),
        (actions.get_active_tools)().map(drop),
        (actions.get_all_tools)().map(drop),
        (actions.set_active_tools)(vec![]),
        (actions.get_commands)().map(drop),
        (actions.get_thinking_level)().map(drop),
        (actions.set_thinking_level)(ThinkingLevel::Off),
    ];
    let facade = facade_calls(api);
    for error in direct.into_iter().chain(facade).map(Result::unwrap_err) {
        assert_eq!(error.to_string(), NOT_INITIALIZED);
    }
    (actions.refresh_tools)().unwrap();
    let direct_model = (actions.set_model)(model("selected"));
    let facade_model = api.set_model(model("selected")).unwrap();
    assert_eq!(
        runtime_support::runtime()
            .block_on(direct_model)
            .unwrap_err()
            .to_string(),
        "Extension runtime not initialized"
    );
    assert_eq!(
        runtime_support::runtime()
            .block_on(facade_model)
            .unwrap_err()
            .to_string(),
        "Extension runtime not initialized"
    );
}

pub fn provider_modes() {
    for modes in [(false, false), (true, true), (true, false), (false, true)] {
        check_provider_mode(modes);
    }
    provider_immediate_failures();
}
fn check_provider_mode((register_override, unregister_override): (bool, bool)) {
    let runtime = create_extension_runtime::<Local>();
    let registry = catalog();
    let runner = runner_local(runtime.clone(), registry.clone());
    let (_owner, api) = local_api(runtime, "/work");
    let log = Rc::new(RefCell::new(Vec::<String>::new()));
    for round in ["first", "second"] {
        registry.register_provider("target", good("seed")).unwrap();
        let registrations = log.clone();
        let removals = log.clone();
        let config = good(round);
        let expected = config.clone();
        let overrides = ProviderActions {
            register_provider: register_override.then(|| {
                Rc::new(
                    move |name: String, config: SharedHandle<RwLock<ProviderConfig>>| {
                        assert!(SharedHandle::ptr_eq(&config, &expected));
                        registrations.borrow_mut().push(format!("register:{name}"));
                        Ok(())
                    },
                ) as maestro_extensions::types::RegisterProviderHandler<Local>
            }),
            unregister_provider: unregister_override.then(|| {
                Rc::new(move |name| {
                    removals.borrow_mut().push(format!("remove:{name}"));
                    Ok(())
                }) as maestro_extensions::types::UnregisterProviderHandler<Local>
            }),
        };
        bind_empty(&runner, overrides).unwrap();
        let start = log.borrow().len();
        api.register_provider("target".into(), config).unwrap();
        assert_eq!(registry.find("target", round).is_some(), !register_override);
        assert_eq!(registry.find("target", "seed").is_some(), register_override);
        api.unregister_provider("target").unwrap();
        assert_eq!(
            registry
                .get_all()
                .read()
                .unwrap()
                .iter()
                .any(|m| m.read().unwrap().provider == "target"),
            unregister_override
        );
        let expected: Vec<_> = [
            register_override.then_some("register:target"),
            unregister_override.then_some("remove:target"),
        ]
        .into_iter()
        .flatten()
        .collect();
        assert_eq!(&log.borrow()[start..], expected);
    }
}

pub fn queued_validations() {
    let runtime = create_extension_runtime::<Local>();
    let registry = catalog();
    let runner = runner_local(runtime.clone(), registry.clone());
    let (owner, api) = local_api(runtime.clone(), "/work");
    let seen = Rc::new(RefCell::new(Vec::new()));
    let output = seen.clone();
    let _off = runner.on_error(Rc::new(move |error| {
        output.borrow_mut().push(error.clone());
        Ok(())
    }));
    let stream = invalid_stream();
    owner.write().unwrap().path = "/current".into();
    api.register_provider("stream".into(), stream).unwrap();
    let key = good("m");
    key.write().unwrap().api_key = None;
    runtime.register_provider("key".into(), key, None).unwrap();
    let protocol = good("m");
    protocol.write().unwrap().api = None;
    runtime
        .register_provider("protocol".into(), protocol, Some(String::new()))
        .unwrap();
    api.register_provider("valid".into(), good("valid"))
        .unwrap();
    bind_empty(&runner, ProviderActions::default()).unwrap();
    let errors = seen.borrow();
    assert_eq!(
        errors
            .iter()
            .map(|e| (
                e.extension_path.as_str(),
                e.event.as_str(),
                e.error.as_str(),
                e.stack.as_deref()
            ))
            .collect::<Vec<_>>(),
        [
            (
                "/current",
                "register_provider",
                "Provider stream: \"api\" is required when registering streamSimple.",
                None
            ),
            (
                "<unknown>",
                "register_provider",
                "Provider key: \"apiKey\" or \"oauth\" is required when defining models.",
                None
            ),
            (
                "",
                "register_provider",
                "Provider protocol, model m: no \"api\" specified.",
                None
            ),
        ]
    );
    assert!(registry.find("valid", "valid").is_some());
    assert!(pending_names(&runtime).is_empty());
}

pub fn local_future() {
    let runtime = create_extension_runtime::<Local>();
    let runner = runner_local(runtime.clone(), catalog());
    let (_owner, api) = local_api(runtime, "/work");
    let local = Rc::new(Cell::new(0));
    let retained = local.clone();
    let mut actions = Local::unbound_actions();
    actions.set_model = Rc::new(move |_| {
        let local = retained.clone();
        Box::pin(async move {
            tokio::task::yield_now().await;
            local.set(local.get() + 1);
            Ok(true)
        })
    });
    let provider_local = local.clone();
    runner
        .bind_core(
            actions,
            ExtensionContextActions::default(),
            ProviderActions {
                register_provider: Some(Rc::new(move |_, _| {
                    provider_local.set(provider_local.get() + 10);
                    Ok(())
                })),
                unregister_provider: None,
            },
        )
        .unwrap();
    api.register_provider("local".into(), good("m")).unwrap();
    assert!(
        runtime_support::runtime()
            .block_on(api.set_model(model("selected")).unwrap())
            .unwrap()
    );
    assert_eq!(local.get(), 11);
}

/// The retired callback must be destroyed only after the runtime guard is released.
struct ObserveDrop {
    runtime: std::rc::Weak<ExtensionRuntime<Local>>,
    released: Rc<Cell<bool>>,
}
impl Drop for ObserveDrop {
    fn drop(&mut self) {
        assert!(self.runtime.upgrade().unwrap().actions.try_write().is_ok());
        self.released.set(true);
    }
}

pub fn live_removal() {
    let runner = Rc::new(runner_local(create_extension_runtime::<Local>(), catalog()));
    let log = Rc::new(RefCell::new(Vec::new()));
    let second = Rc::new(RefCell::new(None::<Rc<dyn Fn()>>));
    let off_second = second.clone();
    let seen = log.clone();
    let first = runner.on_error(Rc::new(move |_| {
        seen.borrow_mut().push(1);
        let off = off_second.borrow().as_ref().unwrap().clone();
        off();
        Ok(())
    }));
    let seen = log.clone();
    let second_listener: ExtensionErrorListener = Rc::new(move |_| {
        seen.borrow_mut().push(2);
        Ok(())
    });
    *second.borrow_mut() = Some(Rc::new(runner.on_error(second_listener.clone())));
    let weak_runner = Rc::downgrade(&runner);
    let seen = log.clone();
    let third = runner.on_error(Rc::new(move |_| {
        seen.borrow_mut().push(3);
        drop(
            weak_runner
                .upgrade()
                .unwrap()
                .on_error(second_listener.clone()),
        );
        Ok(())
    }));
    runner
        .emit_error(&ExtensionError {
            extension_path: "path".into(),
            event: "event".into(),
            error: "error".into(),
            stack: None,
        })
        .unwrap();
    assert_eq!(*log.borrow(), [1, 3, 2]);
    first();
    third();
}

/// Each edit happens during registration, before the next pending config is selected.
pub fn queue_edits() {
    for edit in 0..4 {
        let runtime = create_extension_runtime::<Local>();
        let runner = runner_local(runtime.clone(), catalog());
        let (_owner, api) = local_api(runtime.clone(), "/work");
        let later = good("later");
        runtime
            .register_provider("first".into(), good("first"), None)
            .unwrap();
        runtime
            .register_provider("later".into(), later.clone(), None)
            .unwrap();
        let observed = Rc::new(RefCell::new(Vec::new()));
        let seen = observed.clone();
        let work = runtime.clone();
        let provider = ProviderActions::<Local> {
            register_provider: Some(Rc::new(
                move |name: String, config: SharedHandle<RwLock<ProviderConfig>>| {
                    seen.borrow_mut().push((
                        name.clone(),
                        config.read().unwrap().models.as_ref().unwrap()[0]
                            .id
                            .clone(),
                    ));
                    if name == "first" {
                        apply_queue_edit(&work, &later, edit);
                    }
                    Ok(())
                },
            )),
            unregister_provider: None,
        };
        bind_empty(&runner, provider).unwrap();
        let mut expected = vec![
            ("first".to_owned(), "first".to_owned()),
            (
                "later".into(),
                match edit {
                    0 => "mutated",
                    1 => "replaced",
                    _ => "later",
                }
                .into(),
            ),
        ];
        if edit == 2 {
            expected.push(("added".into(), "added".into()));
        }
        assert_eq!(*observed.borrow(), expected);
        if edit == 3 {
            assert_eq!(
                api.get_session_name().unwrap_err().to_string(),
                "during flush"
            );
        }
        // Release the provider's retained runtime without changing registration observations.
        bind_empty(&runner, ProviderActions::default()).unwrap();
    }
}

fn apply_queue_edit(
    work: &ExtensionRuntime<Local>,
    later: &SharedHandle<RwLock<ProviderConfig>>,
    edit: u8,
) {
    match edit {
        0 => later.write().unwrap().models.as_mut().unwrap()[0].id = "mutated".into(),
        1 => {
            work.pending_provider_registrations
                .write()
                .unwrap()
                .get_mut(&1)
                .unwrap()
                .config = good("replaced");
        }
        2 => work
            .register_provider("added".into(), good("added"), None)
            .unwrap(),
        _ => work.invalidate(Some("during flush".into())),
    }
}

fn provider_immediate_failures() {
    let runtime = create_extension_runtime::<Local>();
    let runner = runner_local(runtime.clone(), catalog());
    let (_owner, api) = local_api(runtime.clone(), "/work");
    let errors = Rc::new(Cell::new(0));
    let seen = errors.clone();
    let _off = runner.on_error(Rc::new(move |_| {
        seen.set(seen.get() + 1);
        Ok(())
    }));
    bind_empty(
        &runner,
        ProviderActions {
            register_provider: Some(Rc::new(|_, _| Err(Box::new(Marker)))),
            unregister_provider: Some(Rc::new(|_| Err(Box::new(Marker)))),
        },
    )
    .unwrap();
    assert!(
        api.register_provider("bad".into(), good("bad"))
            .unwrap_err()
            .downcast_ref::<Marker>()
            .is_some()
    );
    assert!(
        api.unregister_provider("bad")
            .unwrap_err()
            .downcast_ref::<Marker>()
            .is_some()
    );
    assert!(pending_names(&runtime).is_empty());
    assert_eq!(errors.get(), 0);
}

fn invalid_stream() -> SharedHandle<RwLock<ProviderConfig>> {
    let stream = good("m");
    {
        let mut c = stream.write().unwrap();
        c.api = None;
        c.callbacks.stream_simple = Some(Arc::new(
            super::bound_catalog::original_options::OptionsProvider::default(),
        ));
    }

    stream
}

/// Binds command operations whose wait for idle counts its calls.
fn bind_counting_wait(runner: &ExtensionRunner<Local>, count: Rc<Cell<usize>>) {
    runner.bind_command_context(Some(
        maestro_extensions::ExtensionCommandContextActions::new(
            maestro_extensions::CommandContextOperations {
                wait_for_idle: Rc::new(move || {
                    count.set(count.get() + 1);
                    Box::pin(async { Ok(()) })
                }),
                ..super::context_actions::silent_command_operations()
            },
        ),
    ));
}

pub fn binding_order() {
    let runtime = create_extension_runtime::<Local>();
    let runner = Rc::new(runner_local(runtime.clone(), catalog()));
    let (_owner, api) = local_api(runtime.clone(), "/work");
    let api = Rc::new(api);
    let recorder = Rc::new(super::context_actions::Recorder::new());
    let calls = Rc::new(Cell::new(0));
    let check = bound_checker(
        Rc::downgrade(&runner),
        Rc::downgrade(&api),
        recorder.clone(),
        calls.clone(),
    );
    let checked = check.clone();
    let off = runner.on_error(Rc::new(move |error| {
        assert_eq!(error.stack.as_deref(), Some("supplied stack"));
        checked();
        Ok(())
    }));
    runtime
        .register_provider("bad".into(), good("bad"), None)
        .unwrap();
    runtime
        .register_provider("good".into(), good("good"), None)
        .unwrap();
    let mut diagnostic = maestro_models::DiagnosticErrorInfo::error("diagnostic");
    diagnostic.stack = Some("supplied stack".into());
    let providers = ProviderActions::<Local> {
        register_provider: Some(Rc::new(move |name, _| {
            check();
            if name == "bad" {
                Err(Box::new(diagnostic.clone()))
            } else {
                Ok(())
            }
        })),
        unregister_provider: None,
    };
    let commands = Rc::new(Cell::new(0));
    bind_counting_wait(&runner, commands.clone());
    let mut actions = Local::unbound_actions();
    actions.get_session_name = Rc::new(|| Ok(Some("just bound".into())));
    runner
        .bind_core(actions, recorder.actions(), providers)
        .unwrap();
    assert_eq!(calls.get(), 3);
    assert_eq!(commands.get(), 3);
    runner.bind_command_context(None);
    assert_eq!(
        api.get_session_name().unwrap().as_deref(),
        Some("just bound")
    );
    assert!(!runner.create_context().is_idle().unwrap());
    off();
}

fn bound_checker(
    r: std::rc::Weak<ExtensionRunner<Local>>,
    facade: std::rc::Weak<ExtensionAPI<Local>>,
    expected: Rc<super::context_actions::Recorder>,
    observed: Rc<Cell<u32>>,
) -> Rc<dyn Fn()> {
    Rc::new(move || {
        let runner = r.upgrade().unwrap();
        let context = runner.create_context();
        super::context_actions::check_bound_operations(&context, &expected);
        super::context_actions::check_compaction_forwarding(&context, &expected);
        assert_eq!(
            facade
                .upgrade()
                .unwrap()
                .get_session_name()
                .unwrap()
                .as_deref(),
            Some("just bound")
        );
        runtime_support::runtime()
            .block_on(runner.create_command_context().wait_for_idle())
            .unwrap();
        observed.set(observed.get() + 1);
    })
}

fn facade_calls(api: &ExtensionAPI<Local>) -> [Result<(), EventError>; 12] {
    [
        api.send_message(message(Value::Null), None),
        api.send_user_message(UserContent::Text("user".into()), None),
        api.append_entry("entry".into(), None),
        api.set_session_name("name".into()),
        api.get_session_name().map(drop),
        api.set_label("id".into(), None),
        api.get_active_tools().map(drop),
        api.get_all_tools().map(drop),
        api.set_active_tools(vec![]),
        api.get_commands().map(drop),
        api.get_thinking_level().map(drop),
        api.set_thinking_level(ThinkingLevel::Off),
    ]
}

fn message_absence_and_images(
    api: &ExtensionAPI<Local>,
    log: &Rc<RefCell<Vec<Value>>>,
    entries: &RefCell<Vec<(String, Option<Value>)>>,
) {
    api.append_entry("missing".into(), None).unwrap();
    assert_eq!(entries.borrow().last(), Some(&("missing".into(), None)));
    let mut missing = message(Value::Null);
    missing.details = Presence::Missing;
    api.send_message(missing, None).unwrap();
    let observed = log.borrow().last().unwrap()[1].clone();
    assert!(!observed.as_object().unwrap().contains_key("details"));
    let blocks =
        json!([{"type":"text","text":"a"},{"type":"image","data":"eA==","mimeType":"image/png"}]);
    let content: UserContent = serde_json::from_value(blocks.clone()).unwrap();
    api.send_user_message(content.clone(), None).unwrap();
    assert_eq!(log.borrow().last().unwrap(), &json!(["user", blocks, null]));
    let custom = CustomMessageInput {
        custom_type: String::new(),
        content,
        display: true,
        details: Presence::Null,
    };
    api.send_message(
        custom,
        Some(SendMessageOptions {
            trigger_turn: Some(true),
            deliver_as: None,
        }),
    )
    .unwrap();
    assert_eq!(
        log.borrow().last().unwrap(),
        &json!(["custom",{"customType":"","content":blocks,"display":true,"details":null},{"triggerTurn":true}])
    );
}

fn model_forwarding() {
    let runtime = create_extension_runtime::<Local>();
    let runner = runner_local(runtime.clone(), catalog());
    let (_owner, api) = local_api(runtime, "/work");
    for result in [true, false] {
        let submitted = model("model identity");
        let expected = submitted.clone();
        let mut actions = Local::unbound_actions();
        actions.set_model = Rc::new(move |model| {
            assert!(Arc::ptr_eq(&model, &expected));
            Box::pin(async move { Ok(result) })
        });
        runner
            .bind_core(
                actions,
                ExtensionContextActions::default(),
                ProviderActions::default(),
            )
            .unwrap();
        assert_eq!(
            runtime_support::runtime()
                .block_on(api.set_model(submitted).unwrap())
                .unwrap(),
            result
        );
    }
}

#[cfg(unix)]
pub fn exec_outputs(api: &ExtensionAPI<Local>) {
    // The fixed script uses shell builtins only: one child, no recursive launches.
    let executable = std::fs::canonicalize("/bin/sh").unwrap();
    assert!(executable.is_file());
    let program = executable.to_str().unwrap();
    let args = vec![
        "-c".into(),
        "printf '%s' \"$1\"; printf 'failure' >&2; exit 7".into(),
        "sh".into(),
        "literal ; $value with spaces".into(),
    ];
    let executor = runtime_support::io_runtime();
    let result = executor
        .block_on(
            api.exec(program, &args, None, maestro_tools::ExecOptions::default())
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        (
            result.stdout.as_str(),
            result.stderr.as_str(),
            result.code,
            result.killed
        ),
        ("literal ; $value with spaces", "failure", 7, false)
    );
    let signal = maestro_models::Cancellation::new();
    signal.abort();
    let result = executor
        .block_on(
            api.exec(
                program,
                &args,
                None,
                maestro_tools::ExecOptions {
                    signal: Some(&signal),
                    timeout: None,
                },
            )
            .unwrap(),
        )
        .unwrap();
    assert!(result.killed);
}

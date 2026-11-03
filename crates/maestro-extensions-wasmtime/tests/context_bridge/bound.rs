//! Bound application operations preserve their values, calls and original failures.
#![cfg(test)]
use maestro_extensions::{
    Cancellation, CompactOptions, ContextUsage, EventError, ExtensionContextActions,
    ExtensionExecution, ExtensionRunner, Local, types::ProviderActions,
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{Arc, RwLock},
};

/// Independently observed calls and retained native operands.
#[derive(Default)]
struct State {
    /// Exact ordered application calls.
    calls: RefCell<Vec<&'static str>>,
    /// Whether application operations return their marker error.
    fail: Cell<bool>,
    /// Original error payload, observable before encoding.
    marker: Arc<()>,
    /// Original signal supplied by the application.
    signal: Cancellation,
    /// Compaction operands retained by their native consumer.
    compactions: RefCell<Vec<Option<CompactOptions>>>,
}

/// Typed error whose identity must survive the native context operation.
#[derive(Debug)]
struct Marker(&'static str, Arc<()>);
impl std::fmt::Display for Marker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} failed", self.0)
    }
}
impl std::error::Error for Marker {}

impl State {
    /// Records an actual application invocation rather than a guest-side prediction.
    fn answer<T>(&self, member: &'static str, value: T) -> Result<T, EventError> {
        self.calls.borrow_mut().push(member);
        if self.fail.get() {
            Err(Box::new(Marker(member, self.marker.clone())))
        } else {
            Ok(value)
        }
    }
}

/// Binds each operation to the real runner using unequal nondefault results.
fn bind(
    runner: &ExtensionRunner<Local>,
    state: &Rc<State>,
    model: Arc<RwLock<maestro_models::Model>>,
) {
    macro_rules! action {
        ($member:literal, $value:expr) => {{
            let state = state.clone();
            Rc::new(move || state.answer($member, $value))
        }};
    }
    let compact = state.clone();
    let signal = state.signal.clone();
    let aborted = state.clone();
    runner
        .bind_core(
            Local::unbound_actions(),
            ExtensionContextActions {
                get_model: action!("getModel", Some(model.clone())),
                is_idle: action!("isIdle", false),
                get_signal: action!("getSignal", Some(signal.clone())),
                abort: Rc::new(move || {
                    aborted.answer("abort", ())?;
                    aborted.signal.abort();
                    Ok(())
                }),
                has_pending_messages: action!("hasPendingMessages", true),
                shutdown: action!("shutdown", ()),
                get_context_usage: action!(
                    "getContextUsage",
                    Some(ContextUsage {
                        tokens: Some(12.5),
                        context_window: 100.0,
                        percent: Some(12.5),
                    })
                ),
                compact: Rc::new(move |options| {
                    compact.answer("compact", ())?;
                    compact.compactions.borrow_mut().push(options);
                    Ok(())
                }),
                get_system_prompt: action!("getSystemPrompt", "bound prompt".into()),
            },
            ProviderActions::default(),
        )
        .unwrap();
}

/// Verifies one original typed native error and the corresponding catchable component text.
async fn errors(runner: &ExtensionRunner<Local>, state: &State) {
    state.fail.set(true);
    let context = runner.create_context();
    let native = [
        ("B073", "model", context.model().err()),
        ("B074", "isIdle", context.is_idle().err()),
        ("B075", "signal", context.signal().err()),
        ("B076", "abort", context.abort().err()),
        (
            "B077",
            "hasPendingMessages",
            context.has_pending_messages().err(),
        ),
        ("B078", "shutdown", context.shutdown().err()),
        ("B079", "getContextUsage", context.get_context_usage().err()),
        ("B080", "compact", context.compact(None).err()),
        ("B081", "getSystemPrompt", context.get_system_prompt().err()),
    ];
    state.calls.borrow_mut().clear();
    let mut failures = Vec::new();
    for (id, member, error) in native {
        let error = error.expect(id);
        let marker = error.downcast_ref::<Marker>().expect(id);
        assert!(
            Arc::ptr_eq(&marker.1, &state.marker),
            "{id}: native error identity"
        );
        super::check(
            &mut failures,
            id,
            &super::observe(runner, member).await,
            &Err(error.to_string()),
        );
        assert_eq!(*state.calls.borrow(), [marker.0], "{id}: calls");
        state.calls.borrow_mut().clear();
    }
    let error = runner.shutdown().expect_err("B082");
    assert!(
        Arc::ptr_eq(
            &error
                .downcast_ref::<Marker>()
                .expect("B082 original native error")
                .1,
            &state.marker
        ),
        "B082"
    );
    assert_eq!(error.to_string(), "shutdown failed", "B082");
    assert_eq!(*state.calls.borrow(), ["shutdown"], "B082");
    state.calls.borrow_mut().clear();
    state.fail.set(false);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Literal observations reject loading defaults, unrelated owners and omitted native calls.
fn cases(
    model: Value,
    history: Value,
) -> Vec<(&'static str, &'static str, Value, Vec<&'static str>)> {
    vec![
        ("B017", "ui", Value::Null, vec![]),
        ("B018", "hasUI", json!(false), vec![]),
        ("B019", "cwd", json!("/work"), vec![]),
        ("B020", "sessionManager", history, vec![]),
        ("B021", "modelRegistry", json!([model.clone()]), vec![]),
        ("B022", "model", model, vec!["getModel"]),
        ("B023", "isIdle", json!(false), vec!["isIdle"]),
        ("B024", "signal", json!(false), vec!["getSignal"]),
        ("B025", "abort", Value::Null, vec!["abort"]),
        (
            "B026",
            "hasPendingMessages",
            json!(true),
            vec!["hasPendingMessages"],
        ),
        ("B027", "shutdown", Value::Null, vec!["shutdown"]),
        (
            "B028",
            "getContextUsage",
            json!({"tokens":12.5,"contextWindow":100.0,"percent":12.5}),
            vec!["getContextUsage"],
        ),
        ("B029", "compact", Value::Null, vec!["compact"]),
        (
            "B030",
            "getSystemPrompt",
            json!("bound prompt"),
            vec!["getSystemPrompt"],
        ),
    ]
}

/// Runs the bound base rows against the author component, then checks failures and signals.
pub(super) async fn observe(
    runner: &ExtensionRunner<Local>,
    events: &maestro_extensions::EventBus<Local>,
) {
    let state = Rc::new(State::default());
    let model = super::super::model_support::model("matrix model");
    *runner
        .create_context()
        .model_registry()
        .unwrap()
        .get_all()
        .write()
        .unwrap() = vec![model.clone()];
    bind(runner, &state, model.clone());
    let descriptor = serde_json::to_value(&*model.read().unwrap()).unwrap();
    let identity = {
        let history = runner.create_context().session_manager().unwrap();
        let history = history.borrow();
        json!({"cwd":"/work","id":history.get_session_id()})
    };
    let mut failures = Vec::new();
    for (id, member, expected, calls) in cases(descriptor, identity) {
        super::check(
            &mut failures,
            id,
            &super::observe(runner, member).await,
            &Ok(expected),
        );
        super::check(&mut failures, id, &state.calls.borrow().clone(), &calls);
        state.calls.borrow_mut().clear();
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    runner.shutdown().expect("B034");
    assert_eq!(*state.calls.borrow(), ["shutdown"], "B034");
    state.calls.borrow_mut().clear();
    state.compactions.borrow_mut().clear();
    compact(runner, &state, events).await;
    errors(runner, &state).await;
    let state = Rc::new(State::default());
    bind(runner, &state, model);
    assert_eq!(
        super::observe(runner, "retainedSignal").await,
        Ok(json!({"before":false,"after":true})),
        "B089"
    );
    assert!(state.signal.is_aborted(), "B089: original signal");
    assert_eq!(*state.calls.borrow(), ["getSignal", "abort"], "B089");
}

/// Distinguishes absent options from an empty record and from full authored callbacks.
async fn compact(
    runner: &ExtensionRunner<Local>,
    state: &State,
    events: &maestro_extensions::EventBus<Local>,
) {
    let notifications = Rc::new(RefCell::new(Vec::new()));
    let received = notifications.clone();
    let _subscription = events.on(
        "matrix-compaction",
        Rc::new(move |data, _| {
            received.borrow_mut().push((*data).clone());
            Ok(None)
        }),
    );
    let mut failures = Vec::new();
    for (id, options, expected) in [
        ("B031", "none", Value::Null),
        (
            "B032",
            "empty",
            json!({"instructions":null,"complete":false,"error":false}),
        ),
        (
            "B033",
            "full",
            json!({"instructions":"keep","complete":true,"error":true}),
        ),
    ] {
        super::check(
            &mut failures,
            id,
            &super::observe(runner, &format!("compact {options}")).await,
            &Ok(Value::Null),
        );
        super::check(
            &mut failures,
            id,
            &state.calls.borrow().clone(),
            &vec!["compact"],
        );
        state.calls.borrow_mut().clear();
        let received = state.compactions.borrow_mut().pop().expect(id);
        let actual = received.as_ref().map_or(Value::Null, |o|json!({"instructions":o.custom_instructions,"complete":o.on_complete.is_some(),"error":o.on_error.is_some()}));
        super::check(&mut failures, id, &actual, &expected);
        if options == "full" {
            complete(received.expect(id));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert_eq!(
        *notifications.borrow(),
        [
            json!({"complete":{"summary":" raw summary ","firstKeptEntryId":" entry ","tokensBefore":7.5,"details":{"n":2.5}}}),
            json!({"error":" raw compaction error "}),
        ],
        "B033: authored callback identities"
    );
}

/// Invokes the exact callback operands retained by the original native action.
fn complete(options: CompactOptions) {
    options.on_complete.unwrap()(serde_json::from_value(json!({
        "summary":" raw summary ","firstKeptEntryId":" entry ","tokensBefore":7.5,"details":{"n":2.5},
    })).unwrap()).unwrap();
    options.on_error.unwrap()(" raw compaction error ".into()).unwrap();
}

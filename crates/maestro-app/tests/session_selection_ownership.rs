//! Model selection notifications that start on the session's task facility, compared with
//! the recorded ownership cases: what each handler saw and when the caller saw it finish.
#![cfg(not(target_arch = "wasm32"))]
use crate::session_model_selection::selection_entries;
use crate::support::session_harness::{
    Auth, Harness, HarnessOptions, corpus, harness, on_task_set,
};
use maestro_app::agent_session::CycleDirection;
use maestro_app::{AgentSession, ModelCycleResult};
use maestro_extensions::{
    EventError, ExtensionFactory, ModelSelectEvent, ThinkingLevelSelectEvent,
};
use maestro_models::{Local, ModelThinkingLevel};
use serde_json::{Value, json};
use std::{cell::RefCell, future::Future, pin::Pin, rc::Rc, task::Poll};
use tokio::sync::{Notify, oneshot};

/// What a parked handler records when it starts.
const ENTRIES: [&str; 2] = ["first-enter", "second-enter"];

/// What a parked handler records when it finishes.
const EXITS: [&str; 2] = ["first-exit", "second-exit"];

/// The two scripted models most recorded cases register.
const TWO: [&str; 2] = ["first", "next"];

/// The single scripted model the candidate-cardinality cases register.
const ONE: [&str; 1] = ["first"];

/// Handler messages and recorded events, in the order the handlers produced them.
type Log = Rc<RefCell<Vec<Value>>>;

/// What the parked handlers recorded, in order.
type Steps = Rc<RefCell<Vec<String>>>;

/// Work the session submitted to its task facility.
type Tasks = Rc<RefCell<Vec<Pin<Box<dyn Future<Output = ()>>>>>>;

/// The session a handler reaches to change the thinking level.
type SharedSession = Rc<RefCell<Option<Rc<AgentSession<Local>>>>>;

/// A waiting handler's release signal.
type Waiting = Rc<RefCell<Option<oneshot::Receiver<()>>>>;

/// The recorded result of a selection, or its error message.
type Outcome = Result<Value, String>;

/// The recorded ownership case with `route`, and for the six lifetime rows also `observe`.
fn ownership_case(route: &str, observe: Option<bool>) -> Value {
    corpus("selection_ownership")
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["route"] == route && observe.is_none_or(|wanted| row["observe"] == wanted))
        .unwrap_or_else(|| panic!("no recorded case {route} {observe:?}"))
        .clone()
}

/// Polls `operation` once, so the test decides when the caller looks again.
fn poll_once<T>(operation: &mut Pin<Box<dyn Future<Output = T> + '_>>) -> Poll<T> {
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    operation.as_mut().poll(&mut context)
}

/// The recorded form of the selected model's identity: its id.
fn model_id(session: &AgentSession<Local>) -> Value {
    let model = session.model();
    let id = model.read().unwrap().id.clone();
    json!(id)
}

/// The recorded form of a cycling result; `null` when fewer than two models were eligible.
fn cycle_view(result: Option<ModelCycleResult>) -> Value {
    let Some(result) = result else {
        return Value::Null;
    };
    let model = result.model.read().unwrap().id.clone();
    json!({
        "model": model,
        "thinkingLevel": result.thinking_level,
        "isScoped": result.is_scoped,
    })
}

/// The recorded form of a model selection event.
fn event_view(event: &ModelSelectEvent) -> Value {
    let model = event.model.read().unwrap().id.clone();
    let previous = event
        .previous_model
        .present()
        .map(|previous| previous.read().unwrap().id.clone());
    json!({"model": model, "previous": previous, "source": event.source})
}

/// A harness over `options` that keeps the work the session submits to its task facility.
async fn capturing_harness(mut options: HarnessOptions, tasks: &Tasks) -> Harness {
    let queue = Rc::clone(tasks);
    options.spawn = Some(Rc::new(move |task| queue.borrow_mut().push(task)));
    harness(options).await
}

/// A harness whose two model handlers park at each entry, with the controls that drive them.
struct Parked {
    /// What the handlers recorded, in order.
    steps: Steps,
    /// Signalled when each handler starts.
    entered: [Rc<Notify>; 2],
    /// The release of each handler, absent once it was taken.
    releases: [RefCell<Option<oneshot::Sender<()>>>; 2],
    /// Work the session submitted to its task facility.
    tasks: Tasks,
}

impl Parked {
    /// The controls and the extension whose two model handlers wait for release.
    fn new() -> (Rc<Self>, ExtensionFactory<Local>) {
        let (first_release, first_waiting) = oneshot::channel();
        let (second_release, second_waiting) = oneshot::channel();
        let parked = Rc::new(Self {
            steps: Steps::default(),
            entered: [Rc::new(Notify::new()), Rc::new(Notify::new())],
            releases: [
                RefCell::new(Some(first_release)),
                RefCell::new(Some(second_release)),
            ],
            tasks: Tasks::default(),
        });
        let waiting = [
            Rc::new(RefCell::new(Some(first_waiting))),
            Rc::new(RefCell::new(Some(second_waiting))),
        ];
        (Rc::clone(&parked), parked.extension(waiting))
    }

    /// The extension registering one parked handler per waiting slot, in order.
    fn extension(self: &Rc<Self>, waiting: [Waiting; 2]) -> ExtensionFactory<Local> {
        let controls = Rc::clone(self);
        let waiting = Rc::new(waiting);
        Rc::new(move |api| {
            for (index, slot) in waiting.as_ref().iter().enumerate() {
                let parked = Rc::clone(&controls);
                let slot = Rc::clone(slot);
                let registered = api.on::<ModelSelectEvent, ()>(
                    "model_select".into(),
                    Rc::new(move |_, _| Box::pin(parked_work(&parked, &slot, index))),
                );
                drop(registered);
            }
            Box::pin(std::future::ready(Ok(())))
        })
    }

    /// A harness over the scripted models `first` and `next` that starts its work here.
    async fn harness(&self, factories: Vec<ExtensionFactory<Local>>) -> Harness {
        capturing_harness(
            HarnessOptions {
                models: vec![("first", true), ("next", true)],
                factories,
                ..HarnessOptions::default()
            },
            &self.tasks,
        )
        .await
    }

    /// Lets handler `index` finish; `None` once it was taken.
    fn release(&self, index: usize) -> Option<oneshot::Sender<()>> {
        self.releases[index].borrow_mut().take()
    }

    /// Starts the work the session submitted here on the caller's task set.
    fn start(&self) -> tokio::task::JoinHandle<()> {
        let queued = self.tasks.borrow_mut().pop();
        tokio::task::spawn_local(queued.expect("the session submitted the started notification"))
    }

    /// What the parked handlers recorded, in order.
    fn steps(&self) -> Value {
        json!(*self.steps.borrow())
    }
}

/// The future of the parked handler at `index`, holding its own controls.
fn parked_work(
    parked: &Rc<Parked>,
    waiting: &Waiting,
    index: usize,
) -> impl Future<Output = Result<Option<()>, EventError>> + 'static {
    parked_step(Rc::clone(parked), Rc::clone(waiting), index)
}

/// Records one handler's entry, waits for its release and records its exit.
async fn parked_step(
    parked: Rc<Parked>,
    waiting: Waiting,
    index: usize,
) -> Result<Option<()>, EventError> {
    parked.steps.borrow_mut().push(ENTRIES[index].to_owned());
    parked.entered[index].notify_one();
    let released = waiting.borrow_mut().take().unwrap();
    released.await.unwrap();
    parked.steps.borrow_mut().push(EXITS[index].to_owned());
    Ok(None)
}

/// An extension recording the model selection events the handlers received.
fn event_recorder(log: &Log) -> ExtensionFactory<Local> {
    let log = Rc::clone(log);
    Rc::new(move |api| {
        let events = Rc::clone(&log);
        let registered = api.on::<ModelSelectEvent, ()>(
            "model_select".into(),
            Rc::new(move |event, _| {
                events.borrow_mut().push(event_view(event));
                Box::pin(std::future::ready(Ok(None)))
            }),
        );
        drop(registered);
        Box::pin(std::future::ready(Ok(())))
    })
}

/// The recorded result of the selection `route` drives, or its error message.
fn selection<'a>(h: &'a Harness, route: &str) -> Pin<Box<dyn Future<Output = Outcome> + 'a>> {
    if route == "set" {
        return Box::pin(async move {
            h.session
                .set_model(h.model("next"))
                .await
                .map(|()| Value::Null)
                .map_err(|error| error.message)
        });
    }
    Box::pin(async move {
        h.session
            .cycle_model(CycleDirection::Forward)
            .await
            .map(cycle_view)
            .map_err(|error| error.message)
    })
}

/// Puts `route`'s scoped entries in place before the selection starts.
fn scope(route: &str, h: &Harness) {
    if route == "scoped" {
        h.session.set_scoped_models(vec![
            Harness::scoped(h.model("first"), None),
            Harness::scoped(h.model("next"), None),
        ]);
    }
}

/// Proves the session kept the started notification and lets the first handler finish.
fn release_first(parked: &Parked) -> tokio::task::JoinHandle<()> {
    assert_eq!(
        parked.tasks.borrow().len(),
        1,
        "the session keeps the started notification"
    );
    let released = parked.release(0).expect("the first handler still waits");
    let work = parked.start();
    released.send(()).unwrap();
    work
}

/// Compares the completed notification with the recorded case's `after` observations.
fn assert_finished(parked: &Parked, h: &Harness, events: &Log, after: &Value) {
    assert_eq!(parked.steps(), after["log"]);
    assert_eq!(json!(*events.borrow()), after["events"]);
    assert_eq!(json!(selection_entries(h)), after["entries"]);
    assert_eq!(model_id(&h.session), after["model"]);
    assert_eq!(json!(h.session.thinking_level()), after["thinkingLevel"]);
}

/// Compares a recorded case whose caller either observes the selection or stops waiting.
fn observed_case(route: &str, observe: bool) {
    let expected = ownership_case(route, Some(observe));
    on_task_set(|_| async move {
        let (parked, handlers) = Parked::new();
        let events = Log::default();
        let h = parked
            .harness(vec![handlers, event_recorder(&events)])
            .await;
        scope(route, &h);
        let mut operation = Some(selection(&h, route));
        assert!(
            poll_once(operation.as_mut().expect("the selection is observed")).is_pending(),
            "the caller waits for the first handler"
        );
        assert_eq!(parked.steps(), expected["before"]["log"]);
        assert_eq!(json!(selection_entries(&h)), expected["before"]["entries"]);
        if !observe {
            drop(operation.take());
        }
        let work = release_first(&parked);
        parked.entered[1].notified().await;
        assert_eq!(parked.steps(), expected["middle"]);
        if observe {
            assert!(
                poll_once(operation.as_mut().expect("the selection is observed")).is_pending(),
                "the caller waits for the second handler"
            );
        }
        parked
            .release(1)
            .expect("the second handler still waits")
            .send(())
            .unwrap();
        work.await.unwrap();
        let after = expected["after"].clone();
        assert_finished(&parked, &h, &events, &after);
        let result = if observe {
            let Poll::Ready(outcome) =
                poll_once(operation.as_mut().expect("the selection is observed"))
            else {
                panic!("the observed selection finishes after the handlers");
            };
            outcome.unwrap()
        } else {
            Value::Null
        };
        assert_eq!(result, after["result"]);
    });
}

#[test]
fn set_model_finishes_notifications_without_observer() {
    observed_case("set", false);
}

#[test]
fn scoped_cycle_finishes_notifications_without_observer() {
    observed_case("scoped", false);
}

#[test]
fn available_cycle_finishes_notifications_without_observer() {
    observed_case("available", false);
}

/// What the model handlers of a flat recorded case do.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Handler {
    /// Record the entry and finish.
    Record,
    /// Fail normally in the first handler.
    FailFirst,
    /// Record the entry, then set the thinking level to high.
    ChangeLevel,
    /// No handlers at all.
    Absent,
}

/// The selection a flat recorded case drives.
#[derive(Clone, Copy)]
enum Operation {
    /// Selects the named scripted model.
    Select(&'static str),
    /// Cycles forward.
    Cycle,
    /// Sets the thinking level to high.
    Think,
}

/// The setup a flat recorded case runs over.
#[derive(Clone, Copy)]
struct Case {
    /// The scripted models the catalog holds.
    models: &'static [&'static str],
    /// Whether the provider has configured credentials.
    configured: bool,
    /// The scoped entries, when the case sets them.
    scoped: &'static [&'static str],
    /// The operation the case drives.
    operation: Operation,
}

/// Records one model handler's entry, and only then fails or changes the thinking level.
fn flat_step(
    name: &'static str,
    log: &Log,
    failure: bool,
    session: Option<Rc<AgentSession<Local>>>,
) -> std::future::Ready<Result<Option<()>, EventError>> {
    log.borrow_mut().push(json!(name));
    if failure {
        return std::future::ready(Err("selection failed".into()));
    }
    if let Some(session) = session {
        session
            .set_thinking_level(ModelThinkingLevel::High)
            .unwrap();
    }
    std::future::ready(Ok(None))
}

/// The extension a flat recorded case installs: two model handlers and a thinking handler.
fn flat_handlers(log: &Log, session: &SharedSession, handler: Handler) -> ExtensionFactory<Local> {
    let (log, session) = (Rc::clone(log), Rc::clone(session));
    Rc::new(move |api| {
        let first = Rc::clone(&log);
        let failing = handler == Handler::FailFirst;
        let registered = api.on::<ModelSelectEvent, ()>(
            "model_select".into(),
            Rc::new(move |_, _| Box::pin(flat_step("first", &first, failing, None))),
        );
        drop(registered);
        let second = Rc::clone(&log);
        let changing = handler == Handler::ChangeLevel;
        let level_session = Rc::clone(&session);
        let registered = api.on::<ModelSelectEvent, ()>(
            "model_select".into(),
            Rc::new(move |_, _| {
                let target = if changing {
                    level_session.borrow().clone()
                } else {
                    None
                };
                Box::pin(flat_step("second", &second, false, target))
            }),
        );
        drop(registered);
        let thinking = Rc::clone(&log);
        let registered = api.on::<ThinkingLevelSelectEvent, ()>(
            "thinking_level_select".into(),
            Rc::new(move |_, _| Box::pin(flat_step("thinking", &thinking, false, None))),
        );
        drop(registered);
        Box::pin(std::future::ready(Ok(())))
    })
}

/// The owners and the observation of a flat recorded case.
#[derive(Default)]
struct Flat {
    /// Handler messages, in order.
    log: Log,
    /// The session a handler reaches to change the thinking level.
    session: SharedSession,
    /// Work the session submitted to its task facility.
    tasks: Tasks,
}

impl Flat {
    /// A harness over `case`'s models and scoped entries, starting its work here.
    async fn harness(&self, case: &Case, handler: Handler) -> Harness {
        let factories = match handler {
            Handler::Absent => Vec::new(),
            _ => vec![flat_handlers(&self.log, &self.session, handler)],
        };
        let h = capturing_harness(
            HarnessOptions {
                models: case.models.iter().map(|&id| (id, true)).collect(),
                auth: if case.configured {
                    Auth::Key
                } else {
                    Auth::None
                },
                factories,
                ..HarnessOptions::default()
            },
            &self.tasks,
        )
        .await;
        *self.session.borrow_mut() = Some(Rc::clone(&h.session));
        if !case.scoped.is_empty() {
            h.session.set_scoped_models(
                case.scoped
                    .iter()
                    .map(|&id| Harness::scoped(h.model(id), None))
                    .collect(),
            );
        }
        h
    }

    /// The session's observation of the finished `case`.
    fn observation(&self, route: &str, h: &Harness, outcome: Outcome) -> Value {
        let (result, error) = match outcome {
            Ok(result) => (result, None),
            Err(error) => (Value::Null, Some(error)),
        };
        json!({
            "route": route,
            "log": *self.log.borrow(),
            "entries": selection_entries(h),
            "model": model_id(&h.session),
            "thinkingLevel": h.session.thinking_level(),
            "result": result,
            "error": error,
        })
    }
}

/// Drives the selection a flat recorded case exercises and records its result or error.
async fn drive(h: &Harness, operation: Operation) -> Outcome {
    match operation {
        Operation::Select(id) => h
            .session
            .set_model(h.model(id))
            .await
            .map(|()| Value::Null)
            .map_err(|error| error.message),
        Operation::Cycle => h
            .session
            .cycle_model(CycleDirection::Forward)
            .await
            .map(cycle_view)
            .map_err(|error| error.message),
        Operation::Think => h
            .session
            .set_thinking_level(ModelThinkingLevel::High)
            .map(|()| Value::Null)
            .map_err(|error| error.message),
    }
}

/// Compares one recorded flat case against the session's observation.
fn recorded_case(route: &str, case: &Case, handler: Handler) {
    let expected = ownership_case(route, None);
    on_task_set(|_| async move {
        let flat = Flat::default();
        let h = flat.harness(case, handler).await;
        let outcome = drive(&h, case.operation).await;
        assert_eq!(flat.observation(route, &h, outcome), expected);
        assert!(
            flat.tasks.borrow().is_empty(),
            "the case starts no notification work"
        );
    });
}

/// The recorded cases that change the thinking level while the notification runs.
const HANDLER_LEVELS: [(&str, Case, Handler); 3] = [
    (
        "scoped-handler-level",
        Case {
            models: &TWO,
            configured: true,
            scoped: &TWO,
            operation: Operation::Cycle,
        },
        Handler::ChangeLevel,
    ),
    (
        "available-handler-level",
        Case {
            models: &TWO,
            configured: true,
            scoped: &[],
            operation: Operation::Cycle,
        },
        Handler::ChangeLevel,
    ),
    (
        "thinking",
        Case {
            models: &TWO,
            configured: true,
            scoped: &[],
            operation: Operation::Think,
        },
        Handler::Record,
    ),
];

/// The recorded cases that start no notification work, with their handler behaviour.
const NOOPS: [(&str, Case, Handler); 8] = [
    (
        "same",
        Case {
            models: &TWO,
            configured: true,
            scoped: &[],
            operation: Operation::Select("first"),
        },
        Handler::Record,
    ),
    (
        "duplicate-scoped",
        Case {
            models: &TWO,
            configured: true,
            scoped: &["first", "first"],
            operation: Operation::Cycle,
        },
        Handler::Record,
    ),
    (
        "single-scoped",
        Case {
            models: &ONE,
            configured: true,
            scoped: &ONE,
            operation: Operation::Cycle,
        },
        Handler::Record,
    ),
    (
        "single-available",
        Case {
            models: &ONE,
            configured: true,
            scoped: &[],
            operation: Operation::Cycle,
        },
        Handler::Record,
    ),
    (
        "empty-scoped",
        Case {
            models: &ONE,
            configured: false,
            scoped: &ONE,
            operation: Operation::Cycle,
        },
        Handler::Record,
    ),
    (
        "empty-available",
        Case {
            models: &ONE,
            configured: false,
            scoped: &[],
            operation: Operation::Cycle,
        },
        Handler::Record,
    ),
    (
        "missing-auth",
        Case {
            models: &TWO,
            configured: false,
            scoped: &[],
            operation: Operation::Select("next"),
        },
        Handler::Record,
    ),
    (
        "no-handlers",
        Case {
            models: &TWO,
            configured: true,
            scoped: &[],
            operation: Operation::Select("next"),
        },
        Handler::Absent,
    ),
];

#[test]
fn model_selection_observer_waits_for_ordered_handlers() {
    for route in ["set", "scoped", "available"] {
        observed_case(route, true);
    }
    for (route, case, handler) in HANDLER_LEVELS {
        recorded_case(route, &case, handler);
    }
}

#[test]
fn selection_handler_failure_does_not_skip_following_handlers() {
    recorded_case(
        "handler-error",
        &Case {
            models: &TWO,
            configured: true,
            scoped: &[],
            operation: Operation::Select("next"),
        },
        Handler::FailFirst,
    );
}

#[test]
fn selection_noop_paths_do_not_spawn_notification_work() {
    for (route, case, handler) in NOOPS {
        recorded_case(route, &case, handler);
    }
}

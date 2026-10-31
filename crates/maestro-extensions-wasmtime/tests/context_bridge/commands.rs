//! Command values and option presence observed at the real native operation owner.
#![cfg(test)]
use maestro_extensions::{
    CommandContextOperations, EventError, ExtensionCommandContextActions, ExtensionRunner, Local,
    types::SessionChangeResult,
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

/// Application calls and original error payload.
#[derive(Default)]
struct State {
    /// Actual ordered calls with their original operands.
    calls: RefCell<Vec<Value>>,
    /// Error mode selected by the current case.
    error: Cell<Option<&'static str>>,
    /// Distinguishable original native error payload.
    marker: Arc<()>,
}

/// A failure that remains typed until the component import encodes it.
#[derive(Debug)]
struct Marker(String, Arc<()>);
impl std::fmt::Display for Marker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}
impl std::error::Error for Marker {}
impl State {
    /// Returns an unequal cancellation value or the application's original failure.
    fn result<T>(&self, member: &str, value: T) -> Result<T, EventError> {
        match self.error.get() {
            Some(kind) => Err(Box::new(Marker(
                format!("{member} {kind}"),
                self.marker.clone(),
            ))),
            None => Ok(value),
        }
    }
}

/// Binds plain option readers; supplied callbacks are retained but deliberately not called.
fn bind(runner: &ExtensionRunner<Local>, state: &Rc<State>) {
    let (wait, new, fork, navigate, switch, reload) = (
        state.clone(),
        state.clone(),
        state.clone(),
        state.clone(),
        state.clone(),
        state.clone(),
    );
    runner.bind_command_context(Some(ExtensionCommandContextActions::new(CommandContextOperations {
        wait_for_idle: Rc::new(move || {
            wait.calls.borrow_mut().push(json!(["waitForIdle"]));
            let state = wait.clone();
            Box::pin(async move { state.result("waitForIdle", ()) })
        }),
        new_session: Rc::new(move |options| {
            let options = options.map(|o| json!({"parent":o.parent_session,"setup":o.setup.is_some(),"with":o.with_session.is_some()}));
            new.calls.borrow_mut().push(json!(["newSession", options]));
            let state = new.clone();
            Box::pin(async move { state.result("newSession", SessionChangeResult { cancelled: true }) })
        }),
        fork: Rc::new(move |id, options| {
            let options = options.map(|o| json!({"position":o.position,"with":o.with_session.is_some()}));
            fork.calls.borrow_mut().push(json!(["fork", id, options]));
            let state = fork.clone();
            if state.error.get() == Some("threw") {
                return Box::pin(std::future::ready(state.result("fork", SessionChangeResult::default())));
            }
            Box::pin(async move { state.result("fork", SessionChangeResult { cancelled: true }) })
        }),
        navigate_tree: Rc::new(move |id, options| {
            navigate.calls.borrow_mut().push(json!(["navigateTree", id, options]));
            let state = navigate.clone();
            Box::pin(async move { state.result("navigateTree", SessionChangeResult { cancelled: true }) })
        }),
        switch_session: Rc::new(move |path, options| {
            let options = options.map(|o| json!({"with":o.with_session.is_some()}));
            switch.calls.borrow_mut().push(json!(["switchSession", path, options]));
            let state = switch.clone();
            Box::pin(async move { state.result("switchSession", SessionChangeResult { cancelled: true }) })
        }),
        reload: Rc::new(move || {
            reload.calls.borrow_mut().push(json!(["reload"]));
            let state = reload.clone();
            Box::pin(async move { state.result("reload", ()) })
        }),
    })));
}

/// Exact absent, empty and fully populated option records as the native consumer sees them.
fn cases() -> Vec<(&'static str, &'static str, Value)> {
    vec![
        ("C013", "waitForIdle", json!(["waitForIdle"])),
        ("C014", "newSession", json!(["newSession", null])),
        ("C015", "fork", json!(["fork", "entry-1", null])),
        (
            "C016",
            "navigateTree",
            json!(["navigateTree", "target-1", null]),
        ),
        (
            "C017",
            "switchSession",
            json!(["switchSession", "/sessions/s.jsonl", null]),
        ),
        ("C018", "reload", json!(["reload"])),
        (
            "C019",
            "newSession empty",
            json!(["newSession",{"parent":null,"setup":false,"with":false}]),
        ),
        (
            "C020",
            "fork empty",
            json!(["fork","entry-1",{"position":null,"with":false}]),
        ),
        (
            "C021",
            "navigateTree empty",
            json!(["navigateTree", "target-1", {}]),
        ),
        (
            "C022",
            "switchSession empty",
            json!(["switchSession","/sessions/s.jsonl",{"with":false}]),
        ),
        (
            "C023",
            "newSession full",
            json!(["newSession",{"parent":"/sessions/parent.jsonl","setup":true,"with":true}]),
        ),
        (
            "C024",
            "fork full",
            json!(["fork","entry-1",{"position":"at","with":true}]),
        ),
        (
            "C025",
            "navigateTree full",
            json!(["navigateTree","target-1",{"summarize":true,"customInstructions":"focus","replaceInstructions":false,"label":"checkpoint"}]),
        ),
        (
            "C026",
            "switchSession full",
            json!(["switchSession","/sessions/s.jsonl",{"with":true}]),
        ),
    ]
}

/// Raw entry spelling and both explicit fork positions.
fn fork_cases() -> Vec<(&'static str, &'static str, Value)> {
    vec![
        (
            "C027",
            "fork raw",
            json!(["fork", "  e\u{feff}\u{85} ", null]),
        ),
        (
            "C028",
            "fork before",
            json!(["fork","entry-1",{"position":"before","with":false}]),
        ),
        (
            "C029",
            "fork at",
            json!(["fork","entry-1",{"position":"at","with":false}]),
        ),
    ]
}

/// Checks each native operand list independently of the guest's returned result.
pub(super) async fn observe(runner: &ExtensionRunner<Local>) {
    let state = Rc::new(State::default());
    bind(runner, &state);
    let mut failures = Vec::new();
    for (id, request, expected) in cases().into_iter().chain(fork_cases()) {
        let value = if request == "waitForIdle" || request == "reload" {
            Value::Null
        } else {
            json!({"cancelled":true})
        };
        super::check(
            &mut failures,
            id,
            &super::observe(runner, request).await,
            &Ok(json!({"value":value,"callbacks":0})),
        );
        super::check(
            &mut failures,
            id,
            &state.calls.borrow().clone(),
            &vec![expected],
        );
        state.calls.borrow_mut().clear();
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    errors(runner, &state).await;
}

/// Checks original typed failures before encoding, then exact catchable guest failures.
async fn errors(runner: &ExtensionRunner<Local>, state: &State) {
    state.error.set(Some("failed"));
    let context = runner.create_command_context();
    let native = [
        ("C081", "waitForIdle", context.wait_for_idle().await.err()),
        ("C082", "newSession", context.new_session(None).await.err()),
        (
            "C083",
            "fork",
            context.fork("entry-1".into(), None).await.err(),
        ),
        (
            "C084",
            "navigateTree",
            context.navigate_tree("target-1".into(), None).await.err(),
        ),
        (
            "C085",
            "switchSession",
            context
                .switch_session("/sessions/s.jsonl".into(), None)
                .await
                .err(),
        ),
        ("C086", "reload", context.reload().await.err()),
    ];
    state.calls.borrow_mut().clear();
    let mut failures = Vec::new();
    for (id, member, error) in native {
        check_error(id, &error.expect(id), state);
        super::check(
            &mut failures,
            id,
            &super::observe(runner, member).await,
            &Err(format!("{member} failed")),
        );
        assert_eq!(state.calls.borrow().len(), 1, "{id}");
        assert_eq!(state.calls.borrow()[0][0], member, "{id}");
        state.calls.borrow_mut().clear();
    }
    immediate_error(runner, state, &mut failures).await;
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A native callback can reject immediately, before its returned future is polled.
async fn immediate_error(
    runner: &ExtensionRunner<Local>,
    state: &State,
    failures: &mut Vec<String>,
) {
    state.error.set(Some("threw"));
    let error = runner
        .create_command_context()
        .fork("entry-1".into(), None)
        .await
        .expect_err("C087");
    check_error("C087", &error, state);
    state.calls.borrow_mut().clear();
    super::check(
        failures,
        "C087",
        &super::observe(runner, "fork").await,
        &Err("fork threw".into()),
    );
    assert_eq!(
        *state.calls.borrow(),
        [json!(["fork", "entry-1", null])],
        "C087"
    );
}

/// Observes the exact native marker without normalizing it into a new error.
fn check_error(id: &str, error: &EventError, state: &State) {
    let marker = error.downcast_ref::<Marker>().expect(id);
    assert!(
        Arc::ptr_eq(&marker.1, &state.marker),
        "{id}: original native error"
    );
}

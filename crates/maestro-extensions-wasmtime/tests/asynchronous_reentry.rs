//! Callback prefixes run inside the native caller; suspended tails settle afterward.
#![cfg(test)]
use maestro_extensions::{
    EventBus, EventError, ExtensionActions, ExtensionCommandContextActions,
    ExtensionContextActions, ExtensionExecution, ExtensionRunner, Local, create_event_bus_for,
    types::ProviderActions,
};
use std::{
    cell::RefCell,
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};
use tokio::sync::{mpsc, oneshot};

use super::retained_components::{loaded_on, view};

/// Ordered effects supplied by native actions and authored emissions.
type Trace = Rc<RefCell<Vec<String>>>;
/// An admitted native callback whose result can outlive its first poll.
type Work = Pin<Box<dyn Future<Output = Result<(), EventError>>>>;
/// The SDK operation retains its callback options until its explicit completion.
type SessionOperation = (
    maestro_extensions::types::NewSessionCommandOptions,
    oneshot::Sender<()>,
);

/// Original production owners with a caller-controlled native wait.
struct Fixture {
    /// Keeps the original registration owner alive.
    _owner: maestro_extensions::types::ExtensionHandle<Local>,
    /// Native consumer of the authored registrations.
    runner: Rc<ExtensionRunner<Local>>,
    /// Original bus used by the renderer's blocking import.
    events: EventBus<Local>,
    /// Observations at the real native action boundary.
    trace: Trace,
    /// Releases each actual command-context wait, without timers or scheduler assumptions.
    waits: mpsc::UnboundedReceiver<oneshot::Sender<()>>,
    /// Session callbacks retained by the SDK-supplied replacement operation.
    sessions: mpsc::UnboundedReceiver<SessionOperation>,
}

/// Runs a witness with the same local executor used by the production facade.
fn run(witness: impl Future<Output = ()>) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, witness);
}

impl Fixture {
    /// Loads the author fixture with its real event and action boundaries.
    async fn new() -> Self {
        let events = create_event_bus_for::<Local>(
            Rc::new(|_| panic!("unexpected listener tail")),
            Rc::new(|error| panic!("{error}")),
        )
        .for_extension();
        let (owner, runner, events) = loaded_on(events).await;
        let trace = Trace::default();
        let observed = trace.clone();
        runner
            .bind_core(
                ExtensionActions {
                    set_session_name: Rc::new(move |name| {
                        observed.borrow_mut().push(name);
                        Ok(())
                    }),
                    ..Local::unbound_actions()
                },
                ExtensionContextActions::default(),
                ProviderActions::default(),
            )
            .unwrap();
        let (entered, waits) = mpsc::unbounded_channel();
        let (session_started, sessions) = mpsc::unbounded_channel();
        runner.bind_command_context(Some(ExtensionCommandContextActions::new(operations(
            entered,
            session_started,
        ))));
        let register = runner
            .get_command("bridge-observe")
            .unwrap()
            .command
            .handler;
        register("register-prefixes".into(), runner.create_command_context())
            .await
            .unwrap();
        Self {
            _owner: owner,
            runner: Rc::new(runner),
            events,
            trace,
            waits,
            sessions,
        }
    }

    /// Polls once during a blocking import, then explicitly releases its retained tail.
    async fn pending_prefix(&mut self, kind: &str, call: impl Fn() -> Work + 'static) {
        let pending = Rc::new(RefCell::new(None));
        let (saved, observed) = (pending.clone(), self.trace.clone());
        let subscription = self.events.on(
            "renderer-created",
            Rc::new(move |_, _| {
                observed.borrow_mut().push("caller.before".into());
                let mut future = call();
                let result = future
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop()));
                assert!(result.is_pending(), "expected suspension: {result:?}");
                observed.borrow_mut().push("caller.pending".into());
                *saved.borrow_mut() = Some(future);
                observed.borrow_mut().push("native.after".into());
                Ok(None)
            }),
        );
        view(&self.runner, &self.events, "prefix-outer")
            .unwrap()
            .unwrap();
        subscription.unsubscribe();
        let future = pending.borrow_mut().take().unwrap();
        match futures_util::future::select(future, Box::pin(self.waits.recv())).await {
            futures_util::future::Either::Left((result, _)) => {
                panic!("callback ended before its controlled wait: {result:?}");
            }
            futures_util::future::Either::Right((release, future)) => {
                self.trace.borrow_mut().push("released".into());
                release.unwrap().send(()).unwrap();
                future.await.unwrap();
            }
        }
        assert_eq!(
            *self.trace.borrow(),
            [
                "caller.before".to_owned(),
                format!("{kind}.prefix"),
                "caller.pending".into(),
                "native.after".into(),
                "released".into(),
                format!("{kind}.tail"),
            ]
        );
    }
}

/// The actual native command operations keep their original closures and release signals.
fn operations(
    entered: mpsc::UnboundedSender<oneshot::Sender<()>>,
    sessions: mpsc::UnboundedSender<SessionOperation>,
) -> maestro_extensions::CommandContextOperations {
    maestro_extensions::CommandContextOperations {
        wait_for_idle: Rc::new(move || {
            let (release, ready) = oneshot::channel();
            entered.send(release).unwrap();
            Box::pin(async move {
                ready.await?;
                Ok(())
            })
        }),
        new_session: Rc::new(move |options| {
            let (finish, finished) = oneshot::channel();
            sessions.send((options.unwrap(), finish)).unwrap();
            Box::pin(async move {
                finished.await?;
                Ok(maestro_extensions::types::SessionChangeResult { cancelled: false })
            })
        }),
        ..super::context_actions::silent_command_operations()
    }
}

/// Calls retained session closures while their original SDK operation is still pending.
async fn session_prefix(kind: &str) {
    let mut fixture = Fixture::new().await;
    let command = fixture
        .runner
        .get_command("prefix-session")
        .unwrap()
        .command
        .handler;
    let invocation = command(String::new(), fixture.runner.create_command_context());
    let (options, finish, invocation) =
        match futures_util::future::select(invocation, Box::pin(fixture.sessions.recv())).await {
            futures_util::future::Either::Left((result, _)) => {
                panic!("session operation ended before retaining its callbacks: {result:?}");
            }
            futures_util::future::Either::Right((operation, invocation)) => {
                let (options, finish) = operation.unwrap();
                (options, finish, invocation)
            }
        };
    let call: Box<dyn Fn() -> Work> = if kind == "setup" {
        let setup = options.setup.unwrap();
        let (_, history) = super::session::runner_session("/setup-prefix");
        Box::new(move || setup(history.clone()))
    } else {
        let continuation = options.with_session.unwrap();
        let context = maestro_extensions::ReplacedSessionContext::new(
            fixture.runner.create_command_context(),
            Rc::new(|_, _| Box::pin(async { Ok(()) })),
            Rc::new(|_, _| Box::pin(async { Ok(()) })),
        );
        Box::new(move || continuation(context.clone()))
    };
    fixture.pending_prefix(kind, call).await;
    finish.send(()).unwrap();
    invocation.await.unwrap();
}

#[test]
fn routed_command_runs_its_prefix_before_its_first_poll_returns() {
    run(async {
        let mut fixture = Fixture::new().await;
        let command = fixture
            .runner
            .get_command("prefix-command")
            .unwrap()
            .command
            .handler;
        let context = fixture.runner.create_command_context();
        fixture
            .pending_prefix("command", move || command(String::new(), context.clone()))
            .await;
    });
}

#[test]
fn routed_ready_command_completes_before_its_first_poll_returns() {
    run(async {
        let fixture = Fixture::new().await;
        let command = fixture
            .runner
            .get_command("prefix-command")
            .unwrap()
            .command
            .handler;
        let context = fixture.runner.create_command_context();
        let observed = fixture.trace.clone();
        let subscription = fixture.events.on(
            "renderer-created",
            Rc::new(move |_, _| {
                observed.borrow_mut().push("caller.before".into());
                let mut future = command("ready".into(), context.clone());
                let result = future
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop()));
                observed.borrow_mut().push(match result {
                    Poll::Ready(Ok(())) => "caller.ready".into(),
                    other => format!("unexpected:{other:?}"),
                });
                observed.borrow_mut().push("native.after".into());
                Ok(None)
            }),
        );
        view(&fixture.runner, &fixture.events, "prefix-outer")
            .unwrap()
            .unwrap();
        subscription.unsubscribe();
        assert_eq!(
            *fixture.trace.borrow(),
            [
                "caller.before",
                "command.prefix",
                "command.tail",
                "caller.ready",
                "native.after",
            ]
        );
    });
}

#[test]
fn routed_completion_keeps_its_prefix_and_absent_result() {
    run(async {
        let mut fixture = Fixture::new().await;
        let completion = fixture
            .runner
            .get_command("prefix-command")
            .unwrap()
            .command
            .get_argument_completions
            .unwrap();
        fixture
            .pending_prefix("completion", move || {
                let result = completion("literal prefix".into());
                Box::pin(async move {
                    assert!(result.await?.is_none());
                    Ok(())
                })
            })
            .await;
    });
}

#[test]
fn routed_shortcut_runs_its_prefix_before_its_first_poll_returns() {
    run(async {
        let mut fixture = Fixture::new().await;
        let shortcut = fixture.runner.get_shortcuts(&Vec::new())["ctrl+alt+p"]
            .read()
            .unwrap()
            .handler
            .clone();
        let context = fixture.runner.create_context();
        fixture
            .pending_prefix("shortcut", move || shortcut(context.clone()))
            .await;
    });
}

#[test]
fn routed_tool_keeps_its_prefix_and_original_result() {
    run(async {
        let mut fixture = Fixture::new().await;
        let registered = fixture.runner.get_all_registered_tools().remove(0);
        let tool = maestro_extensions::wrap_registered_tool(&registered, fixture.runner.clone());
        fixture
            .pending_prefix("tool", move || {
                let result = (tool.execute)("literal id".into(), serde_json::json!({}), None, None);
                Box::pin(async move {
                    let result = result.await?;
                    assert!(result.content.is_empty());
                    assert_eq!(result.details, Some(serde_json::json!({})));
                    Ok(())
                })
            })
            .await;
    });
}

#[test]
fn routed_retained_setup_runs_its_prefix_before_its_first_poll_returns() {
    run(session_prefix("setup"));
}

#[test]
fn routed_retained_continuation_runs_its_prefix_before_its_first_poll_returns() {
    run(session_prefix("with-session"));
}

#[test]
fn routed_user_bash_is_ready_with_its_edited_event_and_returned_operations() {
    run(async {
        let fixture = Fixture::new().await;
        let (calls, observed) = (fixture.runner.clone(), fixture.trace.clone());
        let returned = Rc::new(RefCell::new(None));
        let saved = returned.clone();
        let subscription = fixture.events.on(
            "renderer-created",
            Rc::new(move |_, _| {
                observed.borrow_mut().push("caller.before".into());
                let mut event = maestro_extensions::UserBashEvent {
                    command: " literal event ".into(),
                    cwd: "/event".into(),
                    exclude_from_context: true,
                };
                let mut future = Box::pin(calls.emit_user_bash::<Rc<
                        dyn maestro_extensions::BashOperations<Local>,
                    >, maestro_extensions::BashResult>(
                        &mut event
                    ));
                let state = match future
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop()))
                {
                    Poll::Ready(Ok(Some(result))) => {
                        assert!(result.result.is_missing());
                        *saved.borrow_mut() = result.operations;
                        "caller.ready".into()
                    }
                    Poll::Ready(Err(error)) => format!("error:{error}"),
                    Poll::Ready(Ok(None)) => "caller.absent".into(),
                    Poll::Pending => "caller.pending".into(),
                };
                drop(future);
                observed.borrow_mut().push(state);
                observed.borrow_mut().push(event.command);
                observed.borrow_mut().push("native.after".into());
                Ok(None)
            }),
        );
        view(&fixture.runner, &fixture.events, "prefix-outer")
            .unwrap()
            .unwrap();
        subscription.unsubscribe();
        assert_eq!(
            *fixture.trace.borrow(),
            [
                "caller.before",
                "caller.ready",
                " literal event :edited",
                "native.after",
            ]
        );
        assert!(returned.borrow().is_some());
    });
}

/// Obtains the original authored operations through the native runner's event owner.
async fn shell_operations(
    runner: &ExtensionRunner<Local>,
) -> Rc<dyn maestro_extensions::BashOperations<Local>> {
    let mut event = maestro_extensions::UserBashEvent {
        command: " literal event ".into(),
        cwd: "/event".into(),
        exclude_from_context: true,
    };
    runner.emit_user_bash::<Rc<dyn maestro_extensions::BashOperations<Local>>, maestro_extensions::BashResult>(&mut event)
        .await.unwrap().unwrap().operations.unwrap()
}

#[test]
fn routed_returned_bash_is_ready_after_forwarding_each_original_byte_chunk() {
    run(async {
        let fixture = Fixture::new().await;
        let operations = shell_operations(&fixture.runner).await;
        let chunks = Rc::new(RefCell::new(Vec::new()));
        let exit_code = Rc::new(std::cell::Cell::new(None));
        let (observed, bytes, status) = (fixture.trace.clone(), chunks.clone(), exit_code.clone());
        let subscription = fixture.events.on(
            "renderer-created",
            Rc::new(move |_, _| {
                observed.borrow_mut().push("caller.before".into());
                let mut sink = |chunk: &[u8]| {
                    bytes.borrow_mut().push(chunk.to_vec());
                    observed.borrow_mut().push("on_data".into());
                };
                let signal = maestro_extensions::Cancellation::new();
                signal.abort();
                let mut future = operations.exec(
                    " literal shell ",
                    "./literal/../cwd",
                    maestro_extensions::BashExecOptions {
                        on_data: &mut sink,
                        signal: Some(&signal),
                        timeout: Some(2.5),
                        env: None,
                    },
                );
                let state = match future
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop()))
                {
                    Poll::Ready(Ok(result)) => {
                        status.set(result.exit_code);
                        "caller.ready".into()
                    }
                    Poll::Ready(Err(error)) => format!("error:{error}"),
                    Poll::Pending => "caller.pending".into(),
                };
                observed.borrow_mut().push(state);
                observed.borrow_mut().push("native.after".into());
                Ok(None)
            }),
        );
        view(&fixture.runner, &fixture.events, "prefix-outer")
            .unwrap()
            .unwrap();
        subscription.unsubscribe();
        assert_eq!(
            *fixture.trace.borrow(),
            [
                "caller.before",
                "on_data",
                "on_data",
                "caller.ready",
                "native.after",
            ]
        );
        assert_eq!(*chunks.borrow(), [vec![0, 255, 10], vec![]]);
        assert_eq!(exit_code.get(), Some(7.5));
    });
}

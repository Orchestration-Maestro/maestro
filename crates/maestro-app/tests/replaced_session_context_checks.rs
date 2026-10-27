//! Registered commands continue against replacements while their old frames become stale.
#![cfg(not(target_arch = "wasm32"))]

use crate::agent_session_runtime_events_checks::bind_command_runtime;
use crate::support::runtime_fixtures::runtime_for;
use crate::support::session_harness::{
    answer, corpus, delivery_message_view, on_task_set, script_responses,
};
use maestro_app::PromptOptions;
use maestro_extensions::{
    EventError, ExtensionAPI, ExtensionCommandContext, ExtensionFactory, ForkPosition,
    SessionShutdownEvent, SessionStartEvent,
    types::{
        ForkOptions, NewSessionCommandOptions, RegisteredCommandOptions, SwitchSessionOptions,
        WithSession,
    },
};
use maestro_models::{Local, UserContent};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// The command's replacement operation.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Replacement {
    /// Start a fresh session.
    New,
    /// Duplicate the active branch.
    Fork,
    /// Resume the supplied destination.
    Switch,
}

impl Replacement {
    /// The message sent by the continuation.
    fn message(self) -> &'static str {
        match self {
            Self::New => "new callback message",
            Self::Fork => "fork callback message",
            Self::Switch => "switch callback message",
        }
    }
}

/// What a command and the sessions it visits observed.
#[derive(Default)]
struct Observed {
    /// Active-session lifecycle and continuation order.
    order: Vec<String>,
    /// The command context refused stale reads inside the continuation.
    stale_ctx: bool,
    /// The old API refused to send a message inside the continuation.
    stale_api: bool,
    /// The stale read caught in the original command frame after replacement.
    after: String,
    /// File selected when the command began.
    previous_file: Option<String>,
    /// File seen by the replacement continuation.
    replacement_file: Option<String>,
}

/// Uses the replacement context after proving the old command's handles are invalid.
fn continuation(
    ctx: ExtensionCommandContext,
    api: ExtensionAPI<Local>,
    observed: Rc<RefCell<Observed>>,
    operation: Replacement,
    instance: usize,
) -> WithSession {
    Rc::new(move |replacement| {
        let (ctx, api, observed) = (ctx.clone(), api.clone(), Rc::clone(&observed));
        Box::pin(async move {
            let stale_ctx = ctx.session_manager().is_err();
            let stale_api = api
                .send_user_message(UserContent::Text("stale".into()), None)
                .is_err();
            let file = replacement
                .session_manager()?
                .borrow()
                .get_session_file()
                .map(str::to_owned);
            {
                let mut observed = observed.borrow_mut();
                observed.order.push(format!("with:{instance}"));
                observed.stale_ctx = stale_ctx;
                observed.stale_api = stale_api;
                observed.replacement_file = file;
            }
            replacement
                .send_user_message(UserContent::Text(operation.message().into()), None)
                .await
        })
    })
}

/// Executes a bound replacement, then catches a stale read in the original frame.
async fn execute(
    ctx: ExtensionCommandContext,
    api: ExtensionAPI<Local>,
    operation: Replacement,
    target: Option<String>,
    state: (Rc<RefCell<Observed>>, usize),
) -> Result<(), EventError> {
    let (observed, instance) = state;
    let previous = ctx
        .session_manager()?
        .borrow()
        .get_session_file()
        .map(str::to_owned);
    observed.borrow_mut().previous_file = previous.clone();
    let with_session = Some(continuation(
        ctx.clone(),
        api,
        Rc::clone(&observed),
        operation,
        instance,
    ));
    let result = match operation {
        Replacement::New => {
            ctx.new_session(Some(NewSessionCommandOptions {
                parent_session: previous,
                with_session,
                ..Default::default()
            }))
            .await?
        }
        Replacement::Fork => {
            let leaf = ctx
                .session_manager()?
                .borrow()
                .get_leaf_id()
                .unwrap()
                .to_owned();
            ctx.fork(
                leaf,
                Some(ForkOptions {
                    position: Some(ForkPosition::At),
                    with_session,
                }),
            )
            .await?
        }
        Replacement::Switch => {
            ctx.switch_session(target.unwrap(), Some(SwitchSessionOptions { with_session }))
                .await?
        }
    };
    assert!(!result.cancelled);
    observed.borrow_mut().after = ctx
        .session_manager()
        .err()
        .expect("old command frame must be stale")
        .to_string();
    Ok(())
}

/// Registers lifecycle observation and a real replacement command for each loaded session.
fn command_extension(
    operation: Replacement,
    target: Rc<RefCell<Option<String>>>,
    observed: Rc<RefCell<Observed>>,
) -> ExtensionFactory<Local> {
    let starts = Rc::new(Cell::new(0));
    Rc::new(move |api| {
        let instance = Rc::new(Cell::new(0));
        let (active, counter, log) = (
            Rc::clone(&instance),
            Rc::clone(&starts),
            Rc::clone(&observed),
        );
        let started = api.on::<SessionStartEvent, ()>(
            "session_start".into(),
            Rc::new(move |_, _| {
                counter.set(counter.get() + 1);
                active.set(counter.get());
                log.borrow_mut()
                    .order
                    .push(format!("start:{}", active.get()));
                Box::pin(std::future::ready(Ok(None)))
            }),
        );
        let (active, log) = (Rc::clone(&instance), Rc::clone(&observed));
        let shutdown = api.on::<SessionShutdownEvent, ()>(
            "session_shutdown".into(),
            Rc::new(move |_, _| {
                log.borrow_mut()
                    .order
                    .push(format!("shutdown:{}", active.get()));
                Box::pin(std::future::ready(Ok(None)))
            }),
        );
        let (target, observed, old_api) = (Rc::clone(&target), Rc::clone(&observed), api.clone());
        let registered = api.register_command(
            "replace".into(),
            RegisteredCommandOptions {
                description: None,
                get_argument_completions: None,
                handler: Rc::new(move |_, ctx| {
                    let target = target.borrow().clone();
                    Box::pin(execute(
                        ctx,
                        old_api.clone(),
                        operation,
                        target,
                        (Rc::clone(&observed), instance.get()),
                    ))
                }),
            },
        );
        Box::pin(std::future::ready(started.and(shutdown).and(registered)))
    })
}

/// Builds a distinct destination, returns to the original, and starts a fresh event slice.
async fn prepare_destination(
    runtime: &maestro_app::AgentSessionRuntime,
    target: &RefCell<Option<String>>,
    observed: &RefCell<Observed>,
) {
    let original = runtime.session().session_file().unwrap();
    assert!(
        !runtime
            .new_session(NewSessionCommandOptions::default())
            .await
            .unwrap()
            .cancelled
    );
    runtime
        .session()
        .prompt("target", PromptOptions::default())
        .await
        .unwrap();
    *target.borrow_mut() = runtime.session().session_file();
    assert!(
        !runtime
            .switch_session(&original, None, SwitchSessionOptions::default())
            .await
            .unwrap()
            .cancelled
    );
    observed.borrow_mut().order.clear();
}

/// Drives a registered command and records its exact conversation and lifetime observations.
async fn replacement_case(operation: Replacement) -> Value {
    let target = Rc::new(RefCell::new(None));
    let observed = Rc::new(RefCell::new(Observed::default()));
    let extension = command_extension(operation, Rc::clone(&target), Rc::clone(&observed));
    let (h, runtime, _) = runtime_for("commands", vec![(None, extension)]).await;
    drop(script_responses(
        &h,
        vec![
            answer("one", None, None),
            answer("two", None, None),
            answer("three", None, None),
        ],
    ));
    let runtime = Rc::new(runtime);
    bind_command_runtime(&runtime).await;
    if operation != Replacement::New {
        runtime
            .session()
            .prompt("seed", PromptOptions::default())
            .await
            .unwrap();
    }
    if operation == Replacement::Switch {
        prepare_destination(&runtime, &target, &observed).await;
    }
    runtime
        .session()
        .prompt("/replace", PromptOptions::default())
        .await
        .unwrap();
    let messages: Vec<Value> = runtime
        .session()
        .messages()
        .read()
        .unwrap()
        .iter()
        .map(|message| delivery_message_view(&serde_json::to_value(message).unwrap()))
        .collect();
    let result = {
        let seen = observed.borrow();
        assert_eq!(seen.replacement_file, runtime.session().session_file());
        json!({"order": seen.order, "staleCtx": seen.stale_ctx, "staleApi": seen.stale_api, "after": seen.after, "messages": messages,
            "isTarget": if operation == Replacement::Switch { runtime.session().session_file() == *target.borrow() } else { runtime.session().session_file() != seen.previous_file }})
    };
    runtime.dispose().await.unwrap();
    result
}

#[test]
fn new_session_rebinds_before_the_old_command_continues() {
    let expected = corpus("replacement");
    on_task_set(|_| async {
        assert_eq!(
            replacement_case(Replacement::New).await,
            expected["with_new"]
        );
    });
}

#[test]
fn fork_continuation_sends_into_the_replacement_branch() {
    let expected = corpus("replacement");
    on_task_set(|_| async {
        assert_eq!(
            replacement_case(Replacement::Fork).await,
            expected["with_fork"]
        );
    });
}

#[test]
fn switch_continuation_sends_into_the_destination_session() {
    let expected = corpus("replacement");
    on_task_set(|_| async {
        assert_eq!(
            replacement_case(Replacement::Switch).await,
            expected["with_switch"]
        );
    });
}

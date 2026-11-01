//! A connection's lifetime: binding each session, extension operations, shutdown on the
//! input's end, an extension's request or a signal, and the initial binding's dialogs.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::export_fixtures::published;
use crate::support::rpc_connection::{
    Client, Outcomes, connect, no_workspace, replacement_watch, response_to, select_on,
    startup_select, unspawned, user_entry,
};
use crate::support::runtime_fixtures::{runtime_for, stored_in};
use crate::support::session_harness::{
    hold_first_answer, on_paused_task_set, on_task_set, until_idle,
};
use maestro_app::{RpcConnectionOutcome, TerminationSignal};
use maestro_extensions::{
    ExtensionCommandContext, ExtensionContext, ExtensionFactory, SessionShutdownEvent,
    SessionStartEvent,
};
use maestro_models::Local;
use maestro_session::session_manager::ReadonlySessionManager;
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    pin::pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};
use tokio::sync::{Notify, mpsc};

/// Answers the dialog `request` with `value`.
fn choose(client: &Client, request: &Value, value: &str) {
    let response = json!({"type": "extension_ui_response", "id": request["id"], "value": value});
    client.send(&response.to_string());
}

/// The old session's events stop reaching the client once a replacement is bound, the
/// replacement's reach it once, and the protocol's interface stays bound; nothing is
/// written before the first answer.
#[test]
fn binding_order_releases_old_subscription() {
    on_task_set(|_| async {
        let (_h, runtime, _) = runtime_for("rebound", Vec::new()).await;
        let runtime = Rc::new(runtime);
        let mut client = connect(Rc::clone(&runtime));
        client.send(r#"{"id":"first","type":"get_state"}"#);
        assert_eq!(client.next().await["id"], "first", "no startup record");
        let old = runtime.session();
        client.ask(r#"{"type":"new_session"}"#).await;
        assert!(!Rc::ptr_eq(&old, &runtime.session()));
        old.set_session_name("old").unwrap();
        runtime.session().set_session_name("new").unwrap();
        assert_eq!(
            client.next().await,
            json!({"type": "session_info_changed", "name": "new"})
        );
        assert!(runtime.session().extension_runner().has_ui());
        let state = client.ask(r#"{"id":"probe","type":"get_state"}"#).await;
        assert_eq!(
            state["id"], "probe",
            "the old session's change was not streamed"
        );
    });
}

/// What each command-context operation answered, by operation.
type Answers = mpsc::UnboundedSender<(String, Result<Value, String>)>;

/// An extension command `act <operation> [argument]` that runs one command-context
/// operation and reports its answer.
fn actions_extension(answers: Answers) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let answers = answers.clone();
        let handler: maestro_extensions::types::CommandHandler<Local> =
            Rc::new(move |arguments: String, ctx: ExtensionCommandContext| {
                let answers = answers.clone();
                Box::pin(async move {
                    let (operation, argument) =
                        arguments.split_once(' ').unwrap_or((&arguments, ""));
                    let answer = act(&ctx, operation, argument.to_owned()).await;
                    answers
                        .send((operation.to_owned(), answer.map_err(|e| e.to_string())))
                        .unwrap();
                    Ok(())
                })
            });
        let options = maestro_extensions::types::RegisteredCommandOptions {
            description: None,
            get_argument_completions: None,
            handler,
        };
        Box::pin(std::future::ready(
            api.register_command("act".into(), options),
        ))
    })
}

/// Runs the command-context `operation` with `argument`.
async fn act(
    ctx: &ExtensionCommandContext,
    operation: &str,
    argument: String,
) -> Result<Value, maestro_extensions::EventError> {
    let cancelled = |result: maestro_extensions::types::SessionChangeResult| json!({"cancelled": result.cancelled});
    Ok(match operation {
        "wait" => ctx.wait_for_idle().await.map(|()| json!("idle"))?,
        "new" => cancelled(ctx.new_session(None).await?),
        "navigate" => cancelled(ctx.navigate_tree(argument, None).await?),
        "fork" => cancelled(ctx.fork(argument, None).await?),
        "switch" => cancelled(ctx.switch_session(argument, None).await?),
        "reload" => ctx.reload().await.map(|()| json!("reloaded"))?,
        other => panic!("unknown operation {other}"),
    })
}

/// Command-context operations reach the runtime's current owners: a fork or navigation
/// after a replacement uses the replacement, and a vetoed change reports its cancellation.
#[test]
fn extension_actions_keep_retained_runtime_calls() {
    on_task_set(|_| async {
        let (answers, mut answered) = mpsc::unbounded_channel();
        let starts = Rc::default();
        let veto = Rc::new(std::cell::Cell::new(false));
        let factories = vec![
            (None, actions_extension(answers)),
            (
                None,
                replacement_watch(Rc::clone(&starts), Rc::clone(&veto)),
            ),
        ];
        let (h, runtime, _) = runtime_for("actions", factories).await;
        let runtime = Rc::new(runtime);
        let client = connect(Rc::clone(&runtime));
        let mut run = async |line: String| {
            client.send(&json!({"type": "prompt", "message": line}).to_string());
            answered.recv().await.unwrap()
        };
        assert_eq!(
            run("/act wait".into()).await,
            ("wait".into(), Ok(json!("idle")))
        );
        let started = runtime.session().session_id();
        let answer = run("/act new".into()).await;
        assert_eq!(answer, ("new".into(), Ok(json!({"cancelled": false}))));
        assert_ne!(runtime.session().session_id(), started);
        let entry = user_entry(&runtime, "in the replacement");
        let answer = run(format!("/act navigate {entry}")).await;
        assert_eq!(answer, ("navigate".into(), Ok(json!({"cancelled": false}))));
        veto.set(true);
        let answer = run(format!("/act fork {entry}")).await;
        assert_eq!(answer, ("fork".into(), Ok(json!({"cancelled": true}))));
        veto.set(false);
        let answer = run(format!("/act fork {entry}")).await;
        assert_eq!(answer, ("fork".into(), Ok(json!({"cancelled": false}))));
        let root = h.scratch.path().display().to_string();
        let stored = stored_in(&format!("{root}/actions"), &format!("{root}/stored"), &root);
        let path = stored.get_session_file().unwrap().to_owned();
        let answer = run(format!("/act switch {path}")).await;
        assert_eq!(answer, ("switch".into(), Ok(json!({"cancelled": false}))));
        assert_eq!(
            runtime.session().session_file().as_deref(),
            Some(path.as_str())
        );
        assert_eq!(
            run("/act reload".into()).await,
            ("reload".into(), Ok(json!("reloaded")))
        );
        assert_eq!(
            *starts.borrow(),
            [
                json!("startup"),
                json!("new"),
                json!("fork"),
                json!("resume"),
                json!("reload")
            ]
        );
    });
}

/// An extension that keeps the context of its `session_start`.
fn context_keeper(kept: Rc<RefCell<Option<ExtensionContext>>>) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let kept = Rc::clone(&kept);
        let started = api.on::<SessionStartEvent, ()>(
            "session_start".into(),
            Rc::new(move |_, ctx| {
                *kept.borrow_mut() = Some(ctx);
                Box::pin(std::future::ready(Ok(None)))
            }),
        );
        Box::pin(std::future::ready(started))
    })
}

/// A shutdown an extension requests while idle waits for the next command, which is
/// answered before the connection ends; a dialog response or a parse failure is not one.
#[test]
fn extension_shutdown_waits_for_command_reply() {
    on_task_set(|_| async {
        let kept = Rc::default();
        let (_h, runtime, _) =
            runtime_for("requested", vec![(None, context_keeper(Rc::clone(&kept)))]).await;
        let mut client = connect(Rc::new(runtime));
        client.ask(r#"{"type":"get_state"}"#).await;
        kept.borrow().as_ref().unwrap().shutdown().unwrap();
        client.send(r#"{"type":"extension_ui_response","id":"unknown","value":"a"}"#);
        client.send("{");
        assert_eq!(client.next().await["command"], "parse");
        assert!(!client.server.is_finished());
        assert_eq!(
            client.ask(r#"{"id":"last","type":"get_state"}"#).await["id"],
            "last"
        );
        assert_eq!(
            client.outcome().await.unwrap(),
            RpcConnectionOutcome::Requested
        );
    });
}

/// Requests shutdown twice during compaction, then waits at the supplied gate.
fn compaction_shutdown(gate: Rc<Gate>) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let gate = Rc::clone(&gate);
        let registered = api
            .on::<maestro_app::SessionBeforeCompactEvent, maestro_app::SessionBeforeCompactResult>(
                "session_before_compact".into(),
                Rc::new(move |_, context| {
                    let gate = Rc::clone(&gate);
                    Box::pin(async move {
                        context.shutdown()?;
                        context.shutdown()?;
                        pass(Some(gate)).await;
                        Ok(Some(maestro_app::SessionBeforeCompactResult {
                            cancel: maestro_app::Presence::Present(true),
                            compaction: maestro_app::Presence::Missing,
                        }))
                    })
                }),
            );
        Box::pin(std::future::ready(registered))
    })
}

/// A shutdown requested during compaction waits for settlement without another command.
#[test]
fn extension_shutdown_during_compaction_waits_for_settle() {
    on_task_set(|_| async {
        let gate = Rc::new(Gate::default());
        let factory = compaction_shutdown(Rc::clone(&gate));
        let seen = Rc::default();
        let factories = vec![
            (None, factory),
            (None, shutdown_watch(Rc::clone(&seen), None)),
        ];
        let (h, runtime, _) = runtime_for("settle-shutdown", factories).await;
        let runtime = Rc::new(runtime);
        let mut client = connect(Rc::clone(&runtime));
        client.ask(r#"{"type":"get_state"}"#).await;
        let session = runtime.session();
        session.model().write().unwrap().context_window = 1.0;
        let (requested, release) = hold_first_answer(&h, "answer", &[]);
        let prompted = Rc::clone(&session);
        let run = tokio::task::spawn_local(async move {
            prompted
                .prompt("one", maestro_app::PromptOptions::default())
                .await
        });
        requested.notified().await;
        release.notify_one();
        gate.entered.notified().await;
        client.ask(r#"{"type":"get_state"}"#).await;
        assert_eq!(client.signals_stopped(), 0, "compaction still owns the run");
        assert!(
            seen.borrow().is_empty(),
            "a reply during compaction must not shut down"
        );
        assert!(!client.server.is_finished());
        gate.release.notify_one();
        run.await.unwrap().unwrap();
        assert_eq!(
            client.outcome().await.unwrap(),
            RpcConnectionOutcome::Requested
        );
        assert_eq!(seen.borrow().len(), 1);
    });
}

/// Notified when a handler starts waiting, and released by the test.
#[derive(Default)]
struct Gate {
    /// Notified once the handler waits.
    entered: Notify,
    /// Releases the handler.
    release: Notify,
}

/// Waits at `gate` until it is released, when there is one.
async fn pass(gate: Option<Rc<Gate>>) {
    if let Some(gate) = gate {
        gate.entered.notify_one();
        gate.release.notified().await;
    }
}

/// An extension that records what its `session_shutdown` handler saw as the session name,
/// then waits at `gate` when one is supplied.
fn shutdown_watch(
    seen: Rc<RefCell<Vec<Value>>>,
    gate: Option<Rc<Gate>>,
) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let (seen, gate) = (Rc::clone(&seen), gate.clone());
        let shutdown = api.on::<SessionShutdownEvent, ()>(
            "session_shutdown".into(),
            Rc::new(move |_, ctx| {
                let (seen, gate) = (Rc::clone(&seen), gate.clone());
                Box::pin(async move {
                    let name = ctx
                        .session_manager()?
                        .borrow()
                        .get_session_name()?
                        .map(str::to_owned);
                    seen.borrow_mut().push(json!(name));
                    pass(gate).await;
                    Ok(None)
                })
            }),
        );
        Box::pin(std::future::ready(shutdown))
    })
}

/// The session name recorded last in the session file at `path`.
fn stored_name(path: &str) -> Option<Value> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .rfind(|entry| entry["type"] == "session_info")
        .map(|entry| entry["name"].clone())
}

/// Lines read before the input's end are answered, and a final line without its LF is
/// answered while the connection shuts down: its effect is saved, its reply is written,
/// the session is disposed and the input's end returns.
#[test]
fn eof_tail_keeps_effects_and_correlated_reply() {
    on_task_set(|_| async {
        let seen = Rc::default();
        let (_h, runtime, _) = runtime_for(
            "tail-state",
            vec![(None, shutdown_watch(Rc::clone(&seen), None))],
        )
        .await;
        let mut client = connect(Rc::new(runtime));
        client.ask(r#"{"type":"get_state"}"#).await;
        // A command read just before the end is answered too.
        client.send(r#"{"id":"before","type":"get_state"}"#);
        client.end(Some(r#"{"id":"t","type":"get_state"}"#));
        for id in ["before", "t"] {
            let reply = client.next().await;
            assert_eq!(
                (&reply["id"], &reply["success"]),
                (&json!(id), &json!(true))
            );
        }
        assert_eq!(client.outcome().await.unwrap(), RpcConnectionOutcome::Eof);
        assert_eq!(seen.borrow().len(), 1, "the session was disposed");
        let (_h, runtime, _) = runtime_for("tail-name", Vec::new()).await;
        let file = runtime.session().session_file().unwrap();
        user_entry(&runtime, "saved");
        let mut client = connect(Rc::new(runtime));
        // The input ends while the session is still being bound: what came before is
        // answered once the binding finishes.
        client.send(r#"{"id":"early","type":"get_state"}"#);
        client.end(Some(
            r#"{"id":"n","type":"set_session_name","name":" tail "}"#,
        ));
        assert_eq!(client.next().await["id"], "early");
        assert_eq!(
            client.next().await,
            json!({"id": "n", "type": "response", "command": "set_session_name",
                "success": true}),
            "the name change is no longer streamed"
        );
        assert_eq!(client.outcome().await.unwrap(), RpcConnectionOutcome::Eof);
        assert_eq!(stored_name(&file), Some(json!("tail")));
    });
}

/// With a shutdown handler, the handler sees the name before the final line renames the
/// session, and the rename is still saved and answered.
#[test]
fn eof_shutdown_hook_sees_pre_tail_name() {
    on_task_set(|_| async {
        let seen = Rc::default();
        let (_h, runtime, _) = runtime_for(
            "hooked",
            vec![(None, shutdown_watch(Rc::clone(&seen), None))],
        )
        .await;
        let file = runtime.session().session_file().unwrap();
        user_entry(&runtime, "saved");
        runtime.session().set_session_name("old").unwrap();
        let mut client = connect(Rc::new(runtime));
        client.ask(r#"{"type":"get_state"}"#).await;
        client.end(Some(r#"{"id":"n","type":"set_session_name","name":"new"}"#));
        assert_eq!(client.next().await["success"], true);
        assert_eq!(client.outcome().await.unwrap(), RpcConnectionOutcome::Eof);
        assert_eq!(*seen.borrow(), [json!("old")]);
        assert_eq!(stored_name(&file), Some(json!("new")));
    });
}

/// A prompt still running when the input ends gets no reply beyond its acceptance.
#[test]
fn eof_does_not_invent_pending_replies() {
    on_task_set(|_| async {
        let (h, runtime, _) = runtime_for("pending", Vec::new()).await;
        let (requested, _release) = hold_first_answer(&h, "held", &[]);
        let mut client = connect(Rc::new(runtime));
        client.send(r#"{"id":"p","type":"prompt","message":"hold"}"#);
        requested.notified().await;
        client.end(None);
        let outcome = (&mut client.server).await.unwrap().unwrap();
        assert_eq!(outcome, RpcConnectionOutcome::Eof);
        let replies: Vec<Value> = client
            .written()
            .into_iter()
            .filter(response_to("prompt"))
            .collect();
        assert_eq!(
            replies,
            [json!({"id": "p", "type": "response", "command": "prompt", "success": true})]
        );
    });
}

/// The input ending while a signal's shutdown is still disposing the session returns at
/// once.
#[test]
fn eof_during_disposal_exits_without_wait() {
    on_task_set(|_| async {
        let gate = Rc::new(Gate::default());
        let watch = shutdown_watch(Rc::default(), Some(Rc::clone(&gate)));
        let (_h, runtime, _) = runtime_for("repeated", vec![(None, watch)]).await;
        let mut client = connect(Rc::new(runtime));
        client.ask(r#"{"type":"get_state"}"#).await;
        client.signal(TerminationSignal::Hangup);
        gate.entered.notified().await;
        client.end(None);
        assert_eq!(client.outcome().await.unwrap(), RpcConnectionOutcome::Eof);
    });
}

/// A failed read or write ends the connection with that failure through the same shutdown
/// as the input's end: the session is disposed and signal delivery stops.
#[test]
fn failed_reads_and_writes_still_shut_down() {
    on_task_set(|_| async {
        for (label, expected) in [("read", "read failed"), ("write", "write failed")] {
            let seen = Rc::default();
            let watch = shutdown_watch(Rc::clone(&seen), None);
            let (_h, runtime, _) = runtime_for(label, vec![(None, watch)]).await;
            let mut client = connect(Rc::new(runtime));
            client.ask(r#"{"type":"get_state"}"#).await;
            if label == "read" {
                client.fail_read();
            } else {
                client.fail_writes();
                client.send(r#"{"type":"get_state"}"#);
            }
            let stopped = (&mut client.server).await.unwrap().unwrap_err();
            assert_eq!(stopped.to_string(), expected);
            assert_eq!(seen.borrow().len(), 1, "{label}: the session was disposed");
            assert_eq!(
                client.signals_stopped(),
                1,
                "{label}: signal delivery stopped"
            );
        }
    });
}

/// A read failure or the input's end while a startup dialog waits settles the dialog as
/// cancelled, since no answer can arrive: the binding finishes, the session is disposed and
/// the connection ends with the failure or the end.
#[test]
fn lost_input_during_startup_dialog_still_shuts_down() {
    on_paused_task_set(|_| async {
        for label in ["read", "end"] {
            let (outcomes, seen): (Outcomes, _) = (Rc::default(), Rc::default());
            let factories = vec![
                (None, startup_select(Rc::clone(&outcomes))),
                (None, shutdown_watch(Rc::clone(&seen), None)),
            ];
            let (_h, runtime, _) = runtime_for(label, factories).await;
            let mut client = connect(Rc::new(runtime));
            assert_eq!(client.next().await["method"], "select");
            if label == "read" {
                client.fail_read();
            } else {
                client.end(None);
            }
            until_idle().await;
            assert!(client.server.is_finished(), "{label}: the connection ended");
            assert_eq!(*outcomes.borrow(), [Ok(None)], "{label}: cancelled");
            assert_eq!(seen.borrow().len(), 1, "{label}: the session was disposed");
            let outcome = client.outcome().await.map_err(|error| error.to_string());
            let expected = if label == "read" {
                Err("read failed".to_owned())
            } else {
                Ok(RpcConnectionOutcome::Eof)
            };
            assert_eq!(outcome, expected);
        }
    });
}

/// The input ending before the initial binding opens its first dialog settles that dialog
/// as cancelled without asking it: the binding finishes, the session is disposed once and
/// the connection ends.
#[test]
fn immediate_eof_before_first_startup_dialog() {
    on_paused_task_set(|_| async {
        let (outcomes, seen): (Outcomes, _) = (Rc::default(), Rc::default());
        let factories = vec![
            (None, startup_select(Rc::clone(&outcomes))),
            (None, shutdown_watch(Rc::clone(&seen), None)),
        ];
        let (_h, runtime, _) = runtime_for("immediate", factories).await;
        let mut client = connect(Rc::new(runtime));
        client.end(None);
        until_idle().await;
        assert_eq!(*outcomes.borrow(), [Ok(None)], "the dialog was cancelled");
        assert!(client.server.is_finished(), "the connection ended");
        assert_eq!(seen.borrow().len(), 1, "the session was disposed");
        assert_eq!(client.written(), Vec::<Value>::new(), "nothing was asked");
        assert_eq!(client.outcome().await.unwrap(), RpcConnectionOutcome::Eof);
    });
}

/// After the input ends during one startup dialog, a later startup handler's dialog settles
/// as cancelled at once instead of waiting for an answer that cannot arrive.
#[test]
fn consecutive_startup_dialogs_after_input_end() {
    on_paused_task_set(|_| async {
        let (outcomes, seen): (Outcomes, _) = (Rc::default(), Rc::default());
        let factories = vec![
            (None, startup_select(Rc::clone(&outcomes))),
            (None, startup_select(Rc::clone(&outcomes))),
            (None, shutdown_watch(Rc::clone(&seen), None)),
        ];
        let (_h, runtime, _) = runtime_for("consecutive", factories).await;
        let mut client = connect(Rc::new(runtime));
        assert_eq!(client.next().await["method"], "select");
        client.end(None);
        until_idle().await;
        assert_eq!(
            *outcomes.borrow(),
            [Ok(None), Ok(None)],
            "both were cancelled"
        );
        assert!(client.server.is_finished(), "the connection ended");
        assert_eq!(seen.borrow().len(), 1, "the session was disposed");
        assert_eq!(
            client.written(),
            Vec::<Value>::new(),
            "the second was not asked"
        );
        assert_eq!(client.outcome().await.unwrap(), RpcConnectionOutcome::Eof);
    });
}

/// A failed write followed by the input's end, both ready in one poll, does not report the
/// end before the shutdown the failure started has stopped signal delivery and begun
/// disposing; the failure is reported and the session is disposed once.
#[test]
fn failed_write_then_ready_eof_cannot_finish_before_close_starts() {
    on_paused_task_set(|_| async {
        let seen = Rc::default();
        let watch = shutdown_watch(Rc::clone(&seen), None);
        let (_h, runtime, _) = runtime_for("queued-close", vec![(None, watch)]).await;
        let (peer, mut server) = unspawned(Rc::new(runtime), no_workspace(), published());
        let context = &mut Context::from_waker(Waker::noop());
        assert!(server.as_mut().poll(context).is_pending());
        // The initial binding runs on its own task.
        until_idle().await;
        peer.fail_writes();
        peer.send(r#"{"type":"get_state"}"#);
        peer.end(None);
        let polled = server.as_mut().poll(context);
        let (stopped, disposed) = (peer.signals_stopped(), seen.borrow().len());
        assert!(
            polled.is_pending(),
            "ended with {polled:?} after {stopped} signal stops and {disposed} disposals"
        );
        let outcome = tokio::task::spawn_local(server).await.unwrap();
        assert_eq!(
            outcome.map_err(|error| error.to_string()),
            Err("write failed".to_owned())
        );
        assert_eq!(seen.borrow().len(), 1, "the session was disposed");
        assert_eq!(peer.signals_stopped(), 1, "signal delivery stopped");
    });
}

/// A dialog a shutdown handler asks after the input's end, once the session is bound, is
/// asked and stays unanswered: nothing settles it and disposal keeps waiting.
#[test]
fn shutdown_dialog_after_input_end_stays_pending() {
    on_paused_task_set(|_| async {
        let outcomes: Outcomes = Rc::default();
        let factory =
            select_on::<SessionShutdownEvent>("session_shutdown", "bye", Rc::clone(&outcomes));
        let (_h, runtime, _) = runtime_for("bye", vec![(None, factory)]).await;
        let mut client = connect(Rc::new(runtime));
        client.ask(r#"{"type":"get_state"}"#).await;
        client.end(None);
        assert_eq!(client.next().await["method"], "select");
        until_idle().await;
        assert_eq!(*outcomes.borrow(), [], "the dialog is unanswered");
        assert!(!client.server.is_finished(), "disposal still waits");
        client.server.abort();
    });
}

/// The input ending while a failure's shutdown disposes the session returns at once and
/// still reports the failure; the session is disposed once.
#[test]
fn failure_then_input_end_keeps_the_failure() {
    on_task_set(|_| async {
        let (gate, seen) = (Rc::new(Gate::default()), Rc::default());
        let watch = shutdown_watch(Rc::clone(&seen), Some(Rc::clone(&gate)));
        let (_h, runtime, _) = runtime_for("failed-end", vec![(None, watch)]).await;
        let mut client = connect(Rc::new(runtime));
        client.ask(r#"{"type":"get_state"}"#).await;
        client.fail_writes();
        client.send(r#"{"type":"get_state"}"#);
        gate.entered.notified().await;
        client.end(None);
        let outcome = client.outcome().await.map_err(|error| error.to_string());
        assert_eq!(outcome, Err("write failed".to_owned()));
        assert_eq!(seen.borrow().len(), 1);
    });
}

/// A failed write of a notice an extension sends through a retained interface after the
/// binding ends the connection with that failure.
#[test]
fn failed_notice_write_ends_the_connection() {
    on_paused_task_set(|_| async {
        let seen = Rc::default();
        let watch = shutdown_watch(Rc::clone(&seen), None);
        let (_h, runtime, _) = runtime_for("notice", vec![(None, watch)]).await;
        let runtime = Rc::new(runtime);
        let mut client = connect(Rc::clone(&runtime));
        client.ask(r#"{"type":"get_state"}"#).await;
        let ui = runtime.session().extension_runner().get_ui_context();
        client.fail_writes();
        let notified = ui
            .notify("n".into(), None)
            .map_err(|error| error.to_string());
        assert_eq!(notified, Err("write failed".to_owned()));
        until_idle().await;
        assert!(client.server.is_finished(), "the connection ended");
        assert_eq!(seen.borrow().len(), 1, "the session was disposed");
        let outcome = client.outcome().await.map_err(|error| error.to_string());
        assert_eq!(outcome, Err("write failed".to_owned()));
    });
}

/// Disposal does not settle a dialog nobody answered, and once the connection ends nothing
/// keeps the runtime alive.
#[test]
fn shutdown_keeps_unanswered_dialogs_pending() {
    on_task_set(|_| async {
        let (_h, runtime, _) = runtime_for("unanswered", Vec::new()).await;
        let runtime = Rc::new(runtime);
        let mut client = connect(Rc::clone(&runtime));
        client.ask(r#"{"type":"get_state"}"#).await;
        let ui = runtime.session().extension_runner().get_ui_context();
        let mut select = pin!(ui.select("t".into(), vec!["a".into()], None));
        client.next().await;
        client.end(None);
        assert_eq!(client.outcome().await.unwrap(), RpcConnectionOutcome::Eof);
        let waker = std::task::Waker::noop();
        let context = &mut std::task::Context::from_waker(waker);
        assert!(matches!(select.as_mut().poll(context), Poll::Pending));
        let released = Rc::downgrade(&runtime);
        drop((ui, runtime));
        assert!(
            released.upgrade().is_none(),
            "no ownership cycle keeps the runtime"
        );
        assert!(matches!(select.as_mut().poll(context), Poll::Pending));
    });
}

/// A `session_start` handler waiting on a dialog during the initial binding is answered
/// through the connection, and the binding then finishes.
#[test]
fn startup_dialog_response_finishes_initial_binding() {
    on_task_set(|_| async {
        let outcomes: Outcomes = Rc::default();
        let factory = startup_select(Rc::clone(&outcomes));
        let (_h, runtime, _) = runtime_for("startup", vec![(None, factory)]).await;
        let mut client = connect(Rc::new(runtime));
        let request = client.next().await;
        assert_eq!(request["method"], "select");
        choose(&client, &request, "a");
        let state = client.ask(r#"{"id":"bound","type":"get_state"}"#).await;
        assert_eq!(state["id"], "bound");
        assert_eq!(*outcomes.borrow(), [Ok(Some("a".to_owned()))]);
    });
}

/// The answer to a startup dialog can be the input's unterminated final line: it settles
/// the dialog, the binding finishes and the connection ends.
#[test]
fn startup_dialog_answered_by_final_line() {
    on_paused_task_set(|_| async {
        let outcomes: Outcomes = Rc::default();
        let factory = startup_select(Rc::clone(&outcomes));
        let (_h, runtime, _) = runtime_for("startup-tail", vec![(None, factory)]).await;
        let mut client = connect(Rc::new(runtime));
        let request = client.next().await;
        let response = json!({"type": "extension_ui_response", "id": request["id"], "value": "a"});
        client.end(Some(&response.to_string()));
        until_idle().await;
        assert_eq!(*outcomes.borrow(), [Ok(Some("a".to_owned()))]);
        assert_eq!(client.outcome().await.unwrap(), RpcConnectionOutcome::Eof);
    });
}

/// An extension command `ask` that asks a selection and reports the answer.
fn asking_command(answers: mpsc::UnboundedSender<Option<String>>) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let answers = answers.clone();
        let handler: maestro_extensions::types::CommandHandler<Local> =
            Rc::new(move |_, ctx: ExtensionCommandContext| {
                let answers = answers.clone();
                Box::pin(async move {
                    let select = ctx.ui()?.select("asked".into(), vec!["b".into()], None);
                    answers.send(select.await?).unwrap();
                    Ok(())
                })
            });
        let options = maestro_extensions::types::RegisteredCommandOptions {
            description: None,
            get_argument_completions: None,
            handler,
        };
        Box::pin(std::future::ready(
            api.register_command("ask".into(), options),
        ))
    })
}

/// After the initial binding, a dialog a command asks settles through the same input.
#[test]
fn command_dialog_still_resolves_after_binding() {
    on_task_set(|_| async {
        let (answers, mut answered) = mpsc::unbounded_channel();
        let outcomes: Outcomes = Rc::default();
        let factories = vec![
            (None, startup_select(Rc::clone(&outcomes))),
            (None, asking_command(answers)),
        ];
        let (_h, runtime, _) = runtime_for("command-dialog", factories).await;
        let mut client = connect(Rc::new(runtime));
        let request = client.next().await;
        choose(&client, &request, "a");
        client.send(r#"{"type":"prompt","message":"/ask"}"#);
        let request = client
            .until(|record| record["type"] == "extension_ui_request")
            .await;
        let request = request.last().unwrap();
        assert_eq!(request["title"], "asked");
        choose(&client, request, "b");
        assert_eq!(answered.recv().await.unwrap().as_deref(), Some("b"));
    });
}

/// An extension whose `session_start` handler waits on a selection, then records the
/// session name it reads.
fn startup_name_reader(names: Rc<RefCell<Vec<Value>>>) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let names = Rc::clone(&names);
        let started = api.on::<SessionStartEvent, ()>(
            "session_start".into(),
            Rc::new(move |_, ctx| {
                let names = Rc::clone(&names);
                Box::pin(async move {
                    ctx.ui()?
                        .select("start".into(), vec!["a".into()], None)
                        .await?;
                    let name = ctx
                        .session_manager()?
                        .borrow()
                        .get_session_name()?
                        .map(str::to_owned);
                    names.borrow_mut().push(json!(name));
                    Ok(None)
                })
            }),
        );
        Box::pin(std::future::ready(started))
    })
}

/// Commands that arrive during the initial binding wait for it, then run in arrival order
/// and overlap as usual: a quick one answers before a slow one that came first.
#[test]
fn early_commands_wait_and_keep_arrival_order() {
    on_task_set(|_| async {
        let names = Rc::default();
        let factory = startup_name_reader(Rc::clone(&names));
        let (_h, runtime, _) = runtime_for("early", vec![(None, factory)]).await;
        let mut client = connect(Rc::new(runtime));
        let request = client.next().await;
        client.send(r#"{"id":"one","type":"set_session_name","name":"early"}"#);
        client.send(r#"{"id":"two","type":"get_state"}"#);
        client.send(r#"{"id":"slow","type":"bash","command":"sleep 30"}"#);
        client.send(r#"{"id":"fast","type":"get_state"}"#);
        choose(&client, &request, "a");
        let mut ids = Vec::new();
        while ids.len() < 3 {
            let record = client.next().await;
            if record["type"] == "response" {
                ids.push(record["id"].clone());
            }
        }
        assert_eq!(ids, [json!("one"), json!("two"), json!("fast")]);
        assert_eq!(
            *names.borrow(),
            [json!(null)],
            "the rename waited for the binding"
        );
        client.send(r#"{"id":"stop","type":"abort_bash"}"#);
        let records = client.until(response_to("bash")).await;
        assert_eq!(records.last().unwrap()["id"], "slow");
    });
}

/// A connection without a session refuses session operations without constructing session
/// facilities, and still handles workspace commands on the same reader and writer.
#[test]
fn workspace_only_connections_refuse_session_commands() {
    on_task_set(|_| async {
        use crate::support::rpc_connection::ClosureWorkspace;
        use maestro_app::rpc_mode::{JsonlLineReader, RpcModeOptions};
        let lines = Rc::new(RefCell::new(Vec::new()));
        let written = Rc::clone(&lines);
        let output = maestro_app::rpc_mode::RpcOutput::synchronous(Rc::new(
            move |line: &maestro_app::rpc_mode::RpcLine| {
                written
                    .borrow_mut()
                    .push(serde_json::from_str::<Value>(line.as_str()).unwrap());
                Ok(())
            },
        ));
        let options = RpcModeOptions {
            session: None,
            workspace: Some(Rc::new(ClosureWorkspace(no_workspace()))),
        };
        let input = Box::new(JsonlLineReader::new(
            b"{\"id\":\"session\",\"type\":\"get_state\"}\n{\"id\":\"workspace\",\"type\":\"ping\"}\n".as_slice(),
        ));
        let outcome = maestro_app::run_rpc_connection(input, output, options)
            .await
            .unwrap();
        assert_eq!(outcome, RpcConnectionOutcome::Eof);
        let lines = lines.borrow();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0]["id"], "session");
        assert_eq!(lines[0]["command"], "get_state");
        assert!(
            lines[0]["error"]
                .as_str()
                .unwrap()
                .contains("open in a session")
        );
        assert!(lines[0].get("code").is_none());
        assert_eq!(lines[1]["id"], "workspace");
        assert_eq!(lines[1]["command"], "ping");
        assert_eq!(lines[1]["success"], false);
        assert_eq!(lines[1]["error"], "no workspace");
        assert_eq!(lines[1]["code"], "unsupported");
    });
}

/// A session-only connection uses the ordinary unknown-command response for workspace input.
#[test]
fn workspace_commands_without_provider_are_unknown() {
    on_task_set(|_| async {
        use maestro_app::rpc_mode::{JsonlLineReader, RpcModeOptions};
        let lines = Rc::new(RefCell::new(Vec::new()));
        let written = Rc::clone(&lines);
        let output = maestro_app::rpc_mode::RpcOutput::synchronous(Rc::new(
            move |line: &maestro_app::rpc_mode::RpcLine| {
                written
                    .borrow_mut()
                    .push(serde_json::from_str::<Value>(line.as_str()).unwrap());
                Ok(())
            },
        ));
        let input = Box::new(JsonlLineReader::new(
            b"{\"id\":\"workspace\",\"type\":\"ping\"}\n{\"id\":\"other\",\"type\":\"not_known\"}\n".as_slice(),
        ));
        let outcome = maestro_app::run_rpc_connection(
            input,
            output,
            RpcModeOptions {
                session: None,
                workspace: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(outcome, RpcConnectionOutcome::Eof);
        let lines = lines.borrow();
        assert_eq!(
            lines.as_slice(),
            [
                json!({"type":"response", "command":"ping", "success":false, "error":"Unknown command: ping"}),
                json!({"type":"response", "command":"not_known", "success":false, "error":"Unknown command: not_known"}),
            ]
        );
    });
}

//! Lifecycle payloads, cancellation boundaries and context invalidation order.
#![cfg(not(target_arch = "wasm32"))]

use crate::support::runtime_fixtures::runtime_for;
use crate::support::session_harness::{corpus, on_task_set, substituting};
use maestro_app::{AgentSessionRuntime, ExtensionBindings, PromptOptions, RebindSession};
use maestro_extensions::{
    ExtensionFactory, ForkPosition, SessionBeforeForkEvent, SessionBeforeForkResult,
    SessionBeforeSwitchEvent, SessionBeforeSwitchResult, SessionShutdownEvent, SessionStartEvent,
    types::{ForkOptions, NewSessionCommandOptions, SwitchSessionOptions},
};
use maestro_models::Local;
use maestro_session::Presence;
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// Binds commands and startup events initially and after each replacement, without a cycle.
pub(super) async fn bind_command_runtime(runtime: &Rc<AgentSessionRuntime>) {
    let weak = Rc::downgrade(runtime);
    let bind: RebindSession = Rc::new(move |session| {
        let actions = weak.upgrade().unwrap().command_context_actions();
        Box::pin(async move {
            session
                .bind_extensions(ExtensionBindings {
                    command_context_actions: Some(actions),
                    ..ExtensionBindings::default()
                })
                .await
        })
    });
    runtime.set_rebind_session(Some(Rc::clone(&bind)));
    bind(runtime.session()).await.unwrap();
}

/// Records a complete event before supplying its controlled result.
fn recording<V: serde::Serialize + 'static, R: 'static>(
    name: &'static str,
    log: Rc<RefCell<Vec<Value>>>,
    result: impl Fn() -> Option<R> + 'static,
) -> ExtensionFactory<Local> {
    crate::session_events::handling(name, move |event: &mut V| {
        let mut view = serde_json::to_value(event).unwrap();
        view["type"] = json!(name);
        log.borrow_mut().push(view);
        Ok(result())
    })
}

/// All lifecycle events, with independently controlled cancellation before replacement.
fn lifecycle(
    log: &Rc<RefCell<Vec<Value>>>,
    veto: &Rc<Cell<bool>>,
) -> Vec<(Option<&'static str>, ExtensionFactory<Local>)> {
    let switch_veto = Rc::clone(veto);
    let fork_veto = Rc::clone(veto);
    vec![
        (
            None,
            recording::<SessionStartEvent, ()>("session_start", Rc::clone(log), || None),
        ),
        (
            None,
            recording::<SessionShutdownEvent, ()>("session_shutdown", Rc::clone(log), || None),
        ),
        (
            None,
            recording::<SessionBeforeSwitchEvent, SessionBeforeSwitchResult>(
                "session_before_switch",
                Rc::clone(log),
                move || {
                    Some(SessionBeforeSwitchResult {
                        cancel: Presence::Present(switch_veto.get()),
                    })
                },
            ),
        ),
        (
            None,
            recording::<SessionBeforeForkEvent, SessionBeforeForkResult>(
                "session_before_fork",
                Rc::clone(log),
                move || {
                    Some(SessionBeforeForkResult {
                        cancel: Presence::Present(fork_veto.get()),
                        skip_conversation_restore: Presence::Missing,
                    })
                },
            ),
        ),
    ]
}

#[test]
fn new_and_resume_emit_complete_lifecycle_payloads() {
    let expected = corpus("replacement");
    on_task_set(|_| async {
        let log = Rc::new(RefCell::new(Vec::new()));
        let (_h, runtime, _) = runtime_for("events", lifecycle(&log, &Rc::default())).await;
        let runtime = Rc::new(runtime);
        bind_command_runtime(&runtime).await;
        assert_eq!(json!(*log.borrow()), expected["lifecycle"]["startup"]);
        runtime
            .session()
            .prompt("seed", PromptOptions::default())
            .await
            .unwrap();
        let first = runtime.session();
        let first_file = first.session_file().unwrap();
        log.borrow_mut().clear();
        let created = runtime
            .new_session(NewSessionCommandOptions::default())
            .await
            .unwrap();
        assert_eq!(
            json!({"cancelled": created.cancelled}),
            expected["lifecycle"]["created"]
        );
        assert!(!Rc::ptr_eq(&first, &runtime.session()));
        assert!(runtime.session().messages().read().unwrap().is_empty());
        let second_file = runtime.session().session_file().unwrap();
        let labels = [(&*first_file, "FIRST"), (&*second_file, "SECOND")];
        assert_eq!(
            substituting(json!(*log.borrow()), &labels),
            expected["lifecycle"]["fresh"]
        );
        log.borrow_mut().clear();
        let resumed = runtime
            .switch_session(&first_file, None, SwitchSessionOptions::default())
            .await
            .unwrap();
        assert_eq!(
            json!({"cancelled": resumed.cancelled}),
            expected["lifecycle"]["resumed"]
        );
        assert_eq!(
            substituting(json!(*log.borrow()), &labels),
            expected["lifecycle"]["resume"]
        );
        runtime.dispose().await.unwrap();
    });
}

#[test]
fn vetoed_new_and_resume_retain_the_original_session() {
    let expected = corpus("replacement");
    on_task_set(|_| async {
        let log = Rc::new(RefCell::new(Vec::new()));
        let veto = Rc::new(Cell::new(true));
        let (h, runtime, recorded) = runtime_for("veto", lifecycle(&log, &veto)).await;
        let first = runtime.session();
        let file = first.session_file();
        for resume in [false, true] {
            log.borrow_mut().clear();
            let absent = format!("{}/case-8/not-opened.jsonl", h.scratch.path().display());
            let outcome = if resume {
                runtime
                    .switch_session(&absent, None, SwitchSessionOptions::default())
                    .await
                    .unwrap()
            } else {
                runtime
                    .new_session(NewSessionCommandOptions::default())
                    .await
                    .unwrap()
            };
            let key = if resume { "veto_resume" } else { "veto_new" };
            assert_eq!(
                json!({"cancelled": outcome.cancelled}),
                expected[key]["result"]
            );
            assert_eq!(
                json!(Rc::ptr_eq(&first, &runtime.session())),
                expected[key]["sameSession"]
            );
            assert_eq!(runtime.session().session_file(), file);
            assert_eq!(
                json!(recorded.factory_calls.borrow().len()),
                expected[key]["builds"]
            );
            assert_eq!(
                crate::support::session_harness::with_placeholders(json!(*log.borrow()), &h),
                expected[key]["events"]
            );
        }
        runtime.dispose().await.unwrap();
    });
}

#[test]
fn shutdown_precedes_invalidation_and_rebinding() {
    let expected = corpus("replacement");
    on_task_set(|_| async {
        let phases = Rc::new(RefCell::new(Vec::new()));
        let seen = Rc::clone(&phases);
        let shutdown = crate::session_events::handling::<SessionShutdownEvent, ()>(
            "session_shutdown",
            move |_| {
                seen.borrow_mut().push("session_shutdown");
                Ok(None)
            },
        );
        let (_h, runtime, _) = runtime_for("invalidate", vec![(None, shutdown)]).await;
        let old = runtime.session().extension_runner().create_context();
        let before = old.clone();
        let cwd = runtime.cwd();
        let seen = Rc::clone(&phases);
        runtime.set_before_session_invalidate(Some(Rc::new(move || {
            assert_eq!(before.cwd().unwrap(), cwd);
            seen.borrow_mut().push("before_invalidate");
            Ok(())
        })));
        let seen = Rc::clone(&phases);
        runtime.set_rebind_session(Some(Rc::new(move |_| {
            seen.borrow_mut().push("rebind");
            Box::pin(std::future::ready(Ok(())))
        })));
        runtime
            .new_session(NewSessionCommandOptions::default())
            .await
            .unwrap();
        assert_eq!(json!(*phases.borrow()), expected["invalidation"]["phases"]);
        assert_eq!(
            json!(old.cwd().unwrap_err().to_string()),
            expected["invalidation"]["error"]
        );
        runtime.set_before_session_invalidate(None);
        runtime.set_rebind_session(None);
        runtime.dispose().await.unwrap();
    });
}

/// A veto prevents both valid and invalid entries from reaching construction.
async fn assert_fork_vetoes(
    runtime: &AgentSessionRuntime,
    recorded: &crate::support::runtime_fixtures::Recorded,
    log: &RefCell<Vec<Value>>,
    labels: &[(&str, &str)],
    expected: &Value,
) {
    let calls = recorded.factory_calls.borrow().len();
    for (entry, position, key, events) in [
        (labels[0].0, None, "valid", "validEvents"),
        (
            "missing-entry",
            Some(ForkPosition::At),
            "invalid",
            "invalidEvents",
        ),
    ] {
        log.borrow_mut().clear();
        let result = runtime
            .fork(
                entry,
                ForkOptions {
                    position,
                    with_session: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(json!({"cancelled": result.cancelled}), expected[key]);
        assert_eq!(substituting(json!(*log.borrow()), labels), expected[events]);
        assert_eq!(recorded.factory_calls.borrow().len(), calls);
    }
}

#[test]
fn fork_veto_precedes_entry_validation_and_replacement() {
    let expected = corpus("replacement");
    on_task_set(|_| async {
        let log = Rc::new(RefCell::new(Vec::new()));
        let veto = Rc::new(Cell::new(false));
        let (_h, runtime, recorded) = runtime_for("fork-events", lifecycle(&log, &veto)).await;
        let runtime = Rc::new(runtime);
        bind_command_runtime(&runtime).await;
        runtime
            .session()
            .prompt("hello", PromptOptions::default())
            .await
            .unwrap();
        let user = runtime.session().get_user_messages_for_forking()[0]
            .entry_id
            .clone();
        let old = runtime.session().session_file().unwrap();
        log.borrow_mut().clear();
        let result = runtime.fork(&user, ForkOptions::default()).await.unwrap();
        assert_eq!(
            json!({"cancelled": result.cancelled, "selectedText": result.selected_text}),
            expected["fork_events"]["result"]
        );
        let new = runtime.session().session_file().unwrap();
        let labels = [(&*user, "USER"), (&*old, "OLD"), (&*new, "NEW")];
        assert_eq!(
            substituting(json!(*log.borrow()), &labels),
            expected["fork_events"]["success"]
        );
        veto.set(true);
        assert_fork_vetoes(&runtime, &recorded, &log, &labels, &expected["fork_events"]).await;
        veto.set(false);
        assert_eq!(
            json!(
                runtime
                    .fork("missing-entry", ForkOptions::default())
                    .await
                    .unwrap_err()
                    .to_string()
            ),
            expected["fork_events"]["error"]
        );
        runtime.dispose().await.unwrap();
    });
}

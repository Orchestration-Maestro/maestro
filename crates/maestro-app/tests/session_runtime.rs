//! Replacing the session: the first runtime, a new session with its setup, a vetoed or
//! re-checked switch, forks before and at an entry, imports and the final shutdown,
//! compared with the recorded runtime cases.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::runtime_fixtures::{Recorded, runtime_for, stored_in};
use crate::support::session_harness::{
    Harness, answer, as_recorded, on_task_set, retry_started, script_responses, substituting,
};
use maestro_app::{
    AgentSessionRuntime, AgentSessionRuntimeError, CreateAgentSessionRuntimeOptions, PromptOptions,
    create_agent_session_runtime,
};
use maestro_extensions::types::{ForkOptions, NewSessionCommandOptions, SwitchSessionOptions};
use maestro_extensions::{
    ExtensionFactory, ForkPosition, MessageEndEvent, MessageEndEventResult, NativeAgentMessage,
    SessionBeforeSwitchEvent, SessionBeforeSwitchResult, SessionShutdownEvent,
};
use maestro_models::{Local, StopReason};
use maestro_session::session_manager::ReadonlySessionManager;
use maestro_session::{Presence, SessionManager};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc, sync::Arc};

/// The recorded runtime case `name`.
fn expected(name: &str) -> Value {
    crate::support::session_harness::corpus("sdk")[name]["ok"].clone()
}

/// `value` with the scratch root as `$ROOT` and each session file as its recorded label.
fn normalized(h: &Harness, value: &Value, files: &[(String, &str)]) -> Value {
    let labels: Vec<_> = files
        .iter()
        .map(|(file, label)| (file.as_str(), *label))
        .collect();
    let root = h.scratch.path().display().to_string();
    as_recorded(substituting(value.clone(), &labels), &[(&root, "$ROOT")])
}

/// A stored session whose working directory `<root>/gone` no longer exists.
fn stored_elsewhere(root: &str, sessions: &str) -> SessionManager {
    let gone = format!("{root}/gone");
    std::fs::create_dir_all(&gone).unwrap();
    let manager = stored_in(&gone, &format!("{root}/{sessions}"), root);
    std::fs::remove_dir_all(&gone).unwrap();
    manager
}

/// Records the invalidation, rebind and continuation steps of `runtime` into `recorded`.
fn record_steps(runtime: &AgentSessionRuntime, recorded: &Recorded) {
    let order = Rc::clone(&recorded.order);
    runtime.set_before_session_invalidate(Some(Rc::new(move || {
        order.borrow_mut().push("before_invalidate".into());
        Ok(())
    })));
    let order = Rc::clone(&recorded.order);
    runtime.set_rebind_session(Some(Rc::new(move |_| {
        order.borrow_mut().push("rebind".into());
        Box::pin(std::future::ready(Ok(())))
    })));
}

#[test]
fn creates_the_first_runtime_after_the_working_directory_check() {
    let expected = expected("runtime_new_session");
    on_task_set(|_| async {
        let (h, runtime, recorded) = runtime_for("new-session", Vec::new()).await;
        let first = runtime.session().session_file().unwrap();
        let calls = json!(recorded.factory_calls.borrow()[..1]);
        let files = [(first, "<SESSION_FILE_1>")];
        assert_eq!(
            normalized(&h, &calls, &files)[0],
            expected["factory_calls"][0]
        );
        assert_eq!(
            runtime.cwd(),
            format!("{}/new-session", h.scratch.path().display())
        );
    });
}

#[test]
fn refuses_a_first_runtime_whose_stored_directory_is_gone() {
    on_task_set(|_| async {
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let root = scratch.path().display().to_string();
        let cwd = format!("{root}/missing-cwd");
        std::fs::create_dir_all(&cwd).unwrap();
        let manager = stored_in(&cwd, &format!("{root}/sessions"), &root);
        let file = manager.get_session_file().unwrap().to_owned();
        std::fs::remove_dir_all(&cwd).unwrap();
        let manager = SessionManager::open(&file, None, None).unwrap();
        let calls = Rc::new(std::cell::Cell::new(0));
        let counted = Rc::clone(&calls);
        let created = create_agent_session_runtime(
            Rc::new(move |_| {
                counted.set(counted.get() + 1);
                Box::pin(std::future::ready(Err(
                    maestro_models::DiagnosticErrorInfo::error("factory must not run"),
                )))
            }),
            CreateAgentSessionRuntimeOptions {
                cwd: root.clone(),
                agent_dir: root.clone(),
                session_manager: Rc::new(RefCell::new(manager)),
                session_start_event: None,
            },
        )
        .await;
        let Err(AgentSessionRuntimeError::MissingSessionCwd(error)) = created else {
            panic!("runtime must reject the missing working directory before construction");
        };
        let expected = crate::support::session_harness::corpus("replacement");
        assert_eq!(json!(calls.get()), expected["missing_cwd"]["called"]);
        assert_eq!(json!(error.name()), expected["missing_cwd"]["name"]);
        let message = error
            .to_string()
            .replace(&file, "$ROOT/missing.jsonl")
            .replace(&root, "$ROOT");
        assert_eq!(json!(message), expected["missing_cwd"]["message"]);
    });
}

#[test]
fn replaces_the_session_and_rebinds_before_with_session() {
    let expected = expected("runtime_new_session");
    on_task_set(|_| async {
        let (_h, runtime, recorded) = runtime_for("new-session", Vec::new()).await;
        let first = runtime.session();
        record_steps(&runtime, &recorded);
        let order = Rc::clone(&recorded.order);
        let options = NewSessionCommandOptions {
            with_session: Some(Rc::new(move |context| {
                let order = Rc::clone(&order);
                Box::pin(async move {
                    context.send_message(probe_message(), None).await?;
                    order.borrow_mut().push("with_session:function".into());
                    Ok(())
                })
            })),
            ..NewSessionCommandOptions::default()
        };
        let outcome = runtime.new_session(options).await.unwrap();
        assert_eq!(json!({"cancelled": outcome.cancelled}), expected["outcome"]);
        assert_eq!(json!(*recorded.order.borrow()), expected["order"]);
        assert_eq!(
            json!(!Rc::ptr_eq(&first, &runtime.session())),
            expected["replaced"]
        );
        assert_eq!(custom_types(&runtime.session()), ["probe"]);
    });
}

/// A hidden custom message of type `probe`.
fn probe_message() -> maestro_session::messages::CustomMessageInput {
    maestro_session::messages::CustomMessageInput {
        custom_type: "probe".into(),
        content: maestro_models::UserContent::Text("probe".into()),
        display: false,
        details: Presence::Missing,
    }
}

/// The types of the custom messages `session` stored.
fn custom_types(session: &maestro_app::AgentSession<Local>) -> Vec<Value> {
    let manager = session.session_manager().borrow();
    manager
        .get_entries()
        .into_iter()
        .filter(|entry| entry["type"] == "custom_message")
        .map(|entry| entry["customType"].clone())
        .collect()
}

#[test]
fn suspended_rebind_keeps_its_continuation_on_the_rebound_session() {
    on_task_set(|_| async {
        let (_h, runtime, _recorded) = runtime_for("suspended-rebind", Vec::new()).await;
        let (release, resume) = tokio::sync::oneshot::channel::<()>();
        let resume = Rc::new(RefCell::new(Some(resume)));
        let target = Rc::new(RefCell::new(None));
        let rebound = Rc::clone(&target);
        runtime.set_rebind_session(Some(Rc::new(move |session| {
            let Some(resume) = resume.borrow_mut().take() else {
                return Box::pin(std::future::ready(Ok(())));
            };
            *rebound.borrow_mut() = Some(session);
            Box::pin(async move {
                resume.await.unwrap();
                Ok(())
            })
        })));
        let options = NewSessionCommandOptions {
            with_session: Some(Rc::new(|context| {
                Box::pin(async move { context.send_message(probe_message(), None).await })
            })),
            ..NewSessionCommandOptions::default()
        };
        let mut operation = std::pin::pin!(runtime.new_session(options));
        assert!(
            std::future::Future::poll(
                operation.as_mut(),
                &mut std::task::Context::from_waker(std::task::Waker::noop()),
            )
            .is_pending()
        );
        let target = target.borrow().as_ref().cloned().unwrap();
        runtime
            .new_session(NewSessionCommandOptions::default())
            .await
            .unwrap();
        let current = runtime.session();
        assert!(!Rc::ptr_eq(&target, &current));
        release.send(()).unwrap();
        operation.await.unwrap();
        assert_eq!(custom_types(&target), ["probe"]);
        assert!(custom_types(&current).is_empty());
        assert!(Rc::ptr_eq(&current, &runtime.session()));
    });
}

#[test]
fn suspended_setup_keeps_reload_and_continuation_on_the_created_session() {
    on_task_set(|_| async {
        let (_h, runtime, _recorded) = runtime_for("suspended-setup", Vec::new()).await;
        let (release, resume) = tokio::sync::oneshot::channel::<()>();
        let resume = Rc::new(RefCell::new(Some(resume)));
        let options = NewSessionCommandOptions {
            setup: Some(Rc::new(move |manager| {
                let resume = resume.borrow_mut().take().unwrap();
                Box::pin(async move {
                    resume.await.unwrap();
                    manager
                        .borrow_mut()
                        .append_message(json!({
                            "role": "user", "content": "setup history", "timestamp": 0
                        }))
                        .unwrap();
                    Ok(())
                })
            })),
            with_session: Some(Rc::new(|context| {
                Box::pin(async move { context.send_message(probe_message(), None).await })
            })),
            ..NewSessionCommandOptions::default()
        };
        let mut operation = std::pin::pin!(runtime.new_session(options));
        assert!(
            std::future::Future::poll(
                operation.as_mut(),
                &mut std::task::Context::from_waker(std::task::Waker::noop()),
            )
            .is_pending()
        );
        let target = runtime.session();
        assert!(target.messages().read().unwrap().is_empty());
        runtime
            .new_session(NewSessionCommandOptions::default())
            .await
            .unwrap();
        let current = runtime.session();
        assert!(!Rc::ptr_eq(&target, &current));
        release.send(()).unwrap();
        operation.await.unwrap();
        assert_eq!(custom_types(&target), ["probe"]);
        let messages = target.messages();
        let messages = messages.read().unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(
            serde_json::to_value(&messages[0]).unwrap()["content"],
            "setup history"
        );
        assert!(current.messages().read().unwrap().is_empty());
        assert!(custom_types(&current).is_empty());
        assert!(Rc::ptr_eq(&current, &runtime.session()));
    });
}

#[test]
fn applies_setup_entries_to_the_new_session() {
    let expected = expected("runtime_new_session_setup_applies_history");
    on_task_set(|_| async {
        let (_h, runtime, recorded) = runtime_for("new-session-setup", Vec::new()).await;
        let options = NewSessionCommandOptions {
            setup: Some(Rc::new(|manager| {
                manager
                    .borrow_mut()
                    .append_custom_entry("demo", Presence::Present(json!({"value": 1})))
                    .unwrap();
                Box::pin(std::future::ready(Ok(())))
            })),
            ..NewSessionCommandOptions::default()
        };
        let outcome = runtime.new_session(options).await.unwrap();
        assert_eq!(json!({"cancelled": outcome.cancelled}), expected["outcome"]);
        assert_eq!(json!(*recorded.order.borrow()), expected["order"]);
        let session = runtime.session();
        assert_eq!(
            json!(session.messages().read().unwrap().len()),
            expected["message_count"]
        );
        let manager = session.session_manager().borrow();
        let types: Vec<Value> = manager
            .get_entries()
            .into_iter()
            .map(|entry| entry["type"].clone())
            .collect();
        assert_eq!(json!(types), expected["entry_types"]);
    });
}

/// An extension cancelling every switch.
fn vetoing() -> ExtensionFactory<Local> {
    Rc::new(|api| {
        let registered = api.on::<SessionBeforeSwitchEvent, SessionBeforeSwitchResult>(
            "session_before_switch".into(),
            Rc::new(|_, _| {
                Box::pin(std::future::ready(Ok(Some(SessionBeforeSwitchResult {
                    cancel: Presence::Present(true),
                }))))
            }),
        );
        Box::pin(std::future::ready(registered))
    })
}

#[test]
fn reports_a_cancelled_switch() {
    on_task_set(|_| async {
        let (_h, runtime, recorded) = runtime_for("vetoed", vec![(None, vetoing())]).await;
        let first = runtime.session();
        let outcome = runtime
            .new_session(NewSessionCommandOptions::default())
            .await
            .unwrap();
        assert!(outcome.cancelled);
        assert!(Rc::ptr_eq(&first, &runtime.session()));
        assert_eq!(
            recorded.factory_calls.borrow().len(),
            1,
            "nothing was created"
        );
    });
}

#[test]
fn keeps_the_previous_session_file_in_the_start_event() {
    let expected = expected("runtime_new_session");
    on_task_set(|_| async {
        let (h, runtime, recorded) = runtime_for("new-session", Vec::new()).await;
        let previous = runtime.session().session_file().unwrap();
        runtime
            .new_session(NewSessionCommandOptions::default())
            .await
            .unwrap();
        let next = runtime.session().session_file().unwrap();
        let calls = json!(*recorded.factory_calls.borrow());
        let files = [(next, "<SESSION_FILE_2>"), (previous, "<SESSION_FILE_3>")];
        assert_eq!(
            normalized(&h, &calls[1], &files),
            expected["factory_calls"][1]
        );
    });
}

#[test]
fn rechecks_the_stored_working_directory_when_switching() {
    on_task_set(|_| async {
        let (h, runtime, _recorded) = runtime_for("switch", Vec::new()).await;
        let root = h.scratch.path().display().to_string();
        let other = stored_elsewhere(&root, "other");
        let path = other.get_session_file().unwrap().to_owned();
        let switched = runtime
            .switch_session(&path, None, SwitchSessionOptions::default())
            .await;
        assert!(matches!(
            switched,
            Err(AgentSessionRuntimeError::MissingSessionCwd(_))
        ));
    });
}

/// The file of a session stored beside the runtime's sessions in `cwd`.
fn stored_file(h: &Harness, cwd: &str) -> String {
    let root = h.scratch.path().display().to_string();
    let manager = stored_in(cwd, &format!("{cwd}/sessions"), &root);
    manager.get_session_file().unwrap().to_owned()
}

#[test]
fn switches_to_a_stored_session_with_the_previous_file_in_its_start_event() {
    on_task_set(|_| async {
        let (h, runtime, recorded) = runtime_for("switch-stored", Vec::new()).await;
        let previous = runtime.session().session_file().unwrap();
        let cwd = runtime
            .session()
            .session_manager()
            .borrow()
            .get_cwd()
            .to_owned();
        let target = stored_file(&h, &cwd);
        let outcome = runtime
            .switch_session(&target, None, SwitchSessionOptions::default())
            .await
            .unwrap();
        assert!(!outcome.cancelled);
        assert_eq!(runtime.session().session_file().unwrap(), target);
        assert_eq!(
            recorded.factory_calls.borrow()[1]["session_start_event"],
            json!({"type": "session_start", "reason": "resume", "previousSessionFile": previous})
        );
    });
}

#[test]
fn reports_a_vetoed_switch_to_a_stored_session() {
    on_task_set(|_| async {
        let (h, runtime, recorded) = runtime_for("switch-vetoed", vec![(None, vetoing())]).await;
        let first = runtime.session();
        let cwd = first.session_manager().borrow().get_cwd().to_owned();
        let target = stored_file(&h, &cwd);
        let outcome = runtime
            .switch_session(&target, None, SwitchSessionOptions::default())
            .await
            .unwrap();
        assert!(outcome.cancelled);
        assert!(Rc::ptr_eq(&first, &runtime.session()));
        assert_eq!(
            recorded.factory_calls.borrow().len(),
            1,
            "nothing was created"
        );
    });
}

#[test]
fn command_actions_forward_to_the_current_session_and_runtime() {
    on_task_set(|_| async {
        let (h, runtime, recorded) = runtime_for("commands", Vec::new()).await;
        drop(script_responses(&h, vec![answer("answer", None, None)]));
        let session = runtime.session();
        session
            .prompt("question", PromptOptions::default())
            .await
            .unwrap();
        let user_entry = {
            let manager = session.session_manager().borrow();
            let users: Vec<&Value> = manager
                .get_entries()
                .into_iter()
                .filter(|entry| entry["type"] == "message" && entry["message"]["role"] == "user")
                .collect();
            users.last().unwrap()["id"].as_str().unwrap().to_owned()
        };
        let runtime = Rc::new(runtime);
        let actions = runtime.command_context_actions();
        (actions.operations().wait_for_idle)().await.unwrap();
        (actions.operations().reload)().await.unwrap();
        let leaf = runtime
            .session()
            .session_manager()
            .borrow()
            .get_leaf_id()
            .map(str::to_owned)
            .unwrap();
        let navigated = (actions.operations().navigate_tree)(leaf, None)
            .await
            .unwrap();
        assert!(!navigated.cancelled);
        let forked = (actions.operations().fork)(user_entry, None).await.unwrap();
        assert!(!forked.cancelled);
        let created_at = recorded.factory_calls.borrow().len();
        let created = (actions.operations().new_session)(None).await.unwrap();
        assert!(!created.cancelled);
        assert_eq!(recorded.factory_calls.borrow().len(), created_at + 1);
        let cwd = runtime
            .session()
            .session_manager()
            .borrow()
            .get_cwd()
            .to_owned();
        let target = stored_file(&h, &cwd);
        let switched = (actions.operations().switch_session)(target.clone(), None)
            .await
            .unwrap();
        assert!(!switched.cancelled);
        assert_eq!(runtime.session().session_file().unwrap(), target);
        assert_eq!(
            recorded.factory_calls.borrow().len(),
            created_at + 2,
            "the switch created the replacement session"
        );
    });
}

#[test]
fn command_actions_project_cancelled_session_changes() {
    on_task_set(|_| async {
        let (h, runtime, _recorded) = runtime_for("commands-vetoed", vec![(None, vetoing())]).await;
        let first = runtime.session();
        let cwd = first.session_manager().borrow().get_cwd().to_owned();
        let target = stored_file(&h, &cwd);
        let runtime = Rc::new(runtime);
        let actions = runtime.command_context_actions();
        assert!(
            (actions.operations().new_session)(None)
                .await
                .unwrap()
                .cancelled
        );
        assert!(
            (actions.operations().switch_session)(target, None)
                .await
                .unwrap()
                .cancelled
        );
        assert!(
            Rc::ptr_eq(&first, &runtime.session()),
            "a cancelled change leaves the current session in place"
        );
    });
}

#[test]
fn held_command_actions_report_the_ended_runtime() {
    on_task_set(|_| async {
        let (_h, runtime, _recorded) = runtime_for("commands-ended", Vec::new()).await;
        let runtime = Rc::new(runtime);
        let actions = runtime.command_context_actions();
        drop(runtime);
        let error = (actions.operations().wait_for_idle)()
            .await
            .expect_err("the runtime has ended");
        assert_eq!(error.to_string(), "the session runtime has ended");
    });
}

/// Runs two prompts on the runtime's session and forks at the second user message.
async fn fork_case(label: &str, position: ForkPosition) -> (AgentSessionRuntime, Value) {
    let (h, runtime, _recorded) = runtime_for(label, Vec::new()).await;
    drop(script_responses(
        &h,
        vec![
            answer("first answer", None, None),
            answer("second answer", None, None),
        ],
    ));
    let session = runtime.session();
    session
        .prompt("first question", PromptOptions::default())
        .await
        .unwrap();
    session
        .prompt("second question", PromptOptions::default())
        .await
        .unwrap();
    let target = {
        let manager = session.session_manager().borrow();
        let users: Vec<&Value> = manager
            .get_entries()
            .into_iter()
            .filter(|entry| entry["type"] == "message" && entry["message"]["role"] == "user")
            .collect();
        users.last().unwrap()["id"].as_str().unwrap().to_owned()
    };
    let options = ForkOptions {
        position: Some(position),
        with_session: None,
    };
    let outcome = runtime.fork(&target, options).await.unwrap();
    let forked = runtime.session();
    let manager = forked.session_manager().borrow();
    let types: Vec<Value> = manager
        .get_entries()
        .into_iter()
        .map(|entry| entry["type"].clone())
        .collect();
    let roles: Vec<Value> = forked
        .messages()
        .read()
        .unwrap()
        .iter()
        .map(|message| serde_json::to_value(message).unwrap()["role"].clone())
        .collect();
    let mut view = json!({"cancelled": outcome.cancelled});
    if let Some(text) = outcome.selected_text {
        view["selectedText"] = json!(text);
    }
    drop(manager);
    (
        runtime,
        json!({"outcome": view, "order": [], "entry_types": types, "message_roles": roles}),
    )
}

#[test]
fn forks_before_a_user_message_and_selects_its_text() {
    on_task_set(|_| async {
        let (_runtime, observed) = fork_case("fork-before", ForkPosition::Before).await;
        assert_eq!(observed, expected("runtime_fork_before"));
    });
}

#[test]
fn forks_at_an_entry() {
    on_task_set(|_| async {
        let (_runtime, observed) = fork_case("fork-at", ForkPosition::At).await;
        assert_eq!(observed, expected("runtime_fork_at"));
    });
}

#[test]
fn refuses_an_unknown_fork_entry() {
    let expected = expected("runtime_fork_invalid_entry");
    on_task_set(|_| async {
        let (_h, runtime, _recorded) = runtime_for("fork-invalid", Vec::new()).await;
        let refused = runtime.fork("no-such-entry", ForkOptions::default()).await;
        let Err(error) = refused else {
            panic!("the fork was expected to fail");
        };
        assert_eq!(json!({"invalid": {"error": error.to_string()}}), expected);
    });
}

#[test]
fn reports_a_missing_import_file() {
    let expected = expected("runtime_import_jsonl");
    on_task_set(|_| async {
        let (h, runtime, _recorded) = runtime_for("import", Vec::new()).await;
        let root = h.scratch.path().display().to_string();
        let missing = runtime
            .import_from_jsonl(&format!("{root}/absent.jsonl"), None)
            .await;
        let Err(AgentSessionRuntimeError::SessionImportFileNotFound(error)) = missing else {
            panic!("the import was expected to miss its file");
        };
        let observed = json!({"error": error.to_string()});
        assert_eq!(normalized(&h, &observed, &[]), expected["missing"]);
    });
}

#[test]
fn copies_an_imported_file_into_the_session_directory() {
    let expected = expected("runtime_import_jsonl");
    on_task_set(|_| async {
        let (h, runtime, _recorded) = runtime_for("import", Vec::new()).await;
        let root = h.scratch.path().display().to_string();
        let source = format!("{root}/import-source.jsonl");
        std::fs::write(&source, "").unwrap();
        let imported = runtime.import_from_jsonl(&source, None).await.unwrap();
        assert_eq!(
            json!({"ok": {"cancelled": imported.cancelled}}),
            expected["empty"]
        );
        let session_dir = runtime
            .session()
            .session_manager()
            .borrow()
            .get_session_dir()
            .to_owned();
        let copied = std::path::Path::new(&format!("{session_dir}/import-source.jsonl")).exists();
        assert_eq!(json!(copied), expected["copied"]);
    });
}

/// An extension recording each shutdown reason into `shutdowns`.
fn shutdown_watch(shutdowns: Rc<RefCell<Vec<Value>>>) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let shutdowns = Rc::clone(&shutdowns);
        let registered = api.on::<SessionShutdownEvent, ()>(
            "session_shutdown".into(),
            Rc::new(move |event, _| {
                shutdowns.borrow_mut().push(json!({"reason": event.reason}));
                Box::pin(std::future::ready(Ok(None)))
            }),
        );
        Box::pin(std::future::ready(registered))
    })
}

#[test]
fn shuts_down_with_quit_and_invalidates_before_dispose() {
    let expected = expected("runtime_dispose");
    on_task_set(|_| async {
        let shutdowns = Rc::new(RefCell::new(Vec::new()));
        let watch = shutdown_watch(Rc::clone(&shutdowns));
        let (_h, runtime, recorded) =
            runtime_for("dispose", vec![(Some("<shutdown-watch>"), watch)]).await;
        let order = Rc::clone(&recorded.order);
        runtime.set_before_session_invalidate(Some(Rc::new(move || {
            order.borrow_mut().push("before_invalidate".into());
            Ok(())
        })));
        let context = runtime.session().extension_runner().create_context();
        runtime.dispose().await.unwrap();
        assert_eq!(
            json!({"shutdown": *shutdowns.borrow(), "order": *recorded.order.borrow()}),
            expected
        );
        assert!(
            context.cwd().is_err(),
            "the disposed session's contexts are stale"
        );
    });
}

/// An inline extension registering the `probe` tool, which counts its runs in `runs`, and
/// rewriting each finished answer's total cost to 7.
fn probing(runs: Rc<std::cell::Cell<usize>>) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let mut tool = crate::support::session_harness::tool_definition("probe", None, None);
        let counted = Rc::clone(&runs);
        tool.execute = Rc::new(move |_, _, _, _, _| {
            counted.set(counted.get() + 1);
            Box::pin(std::future::ready(Ok(maestro_agent::AgentToolResult {
                content: Vec::new(),
                details: None,
                terminate: None,
            })))
        });
        let registered = api.register_tool(tool).and_then(|()| {
            api.on::<MessageEndEvent, MessageEndEventResult<NativeAgentMessage>>(
                "message_end".into(),
                Rc::new(|event, _| Box::pin(std::future::ready(Ok(costed(&event.message))))),
            )
        });
        Box::pin(std::future::ready(registered))
    })
}

/// `message` with a total cost of 7 when it is an answer.
fn costed(message: &NativeAgentMessage) -> Option<MessageEndEventResult<NativeAgentMessage>> {
    let maestro_agent::AgentMessage::Assistant(answer) = message else {
        return None;
    };
    let mut rewritten = answer.read().unwrap().clone();
    rewritten.usage.cost.total = 7.0;
    Some(MessageEndEventResult {
        message: Presence::Present(maestro_agent::AgentMessage::Assistant(Arc::new(
            std::sync::RwLock::new(rewritten),
        ))),
    })
}

/// A scripted answer calling the `probe` tool.
fn probe_call() -> maestro_models::AssistantMessage {
    let call = maestro_models::faux_tool_call(
        "probe",
        serde_json::Map::new().into(),
        maestro_models::FauxToolCallOptions::default(),
    );
    maestro_models::faux_assistant_message(
        maestro_models::FauxAssistantContent::Block(maestro_models::AssistantContent::ToolCall(
            call,
        )),
        maestro_models::FauxAssistantMessageOptions {
            stop_reason: Some(maestro_models::StopReason::ToolUse),
            ..maestro_models::FauxAssistantMessageOptions::default()
        },
    )
}

/// The source paths of the `probe` tools `session` registers.
fn probe_source(session: &maestro_app::AgentSession<Local>) -> Vec<String> {
    session
        .get_all_tools()
        .into_iter()
        .filter(|tool| tool.name == "probe")
        .map(|tool| tool.source_info.path)
        .collect()
}

/// The total cost each answer `session` stored carries.
fn stored_costs(session: &maestro_app::AgentSession<Local>) -> Vec<Value> {
    let manager = session.session_manager().borrow();
    manager
        .get_entries()
        .into_iter()
        .filter(|entry| entry["message"]["role"] == "assistant")
        .map(|entry| entry["message"]["usage"]["cost"]["total"].clone())
        .collect()
}

#[test]
fn completes_a_tool_turn_then_reloads_and_replaces_with_fresh_contexts() {
    on_task_set(|_| async {
        let runs = Rc::new(std::cell::Cell::new(0));
        let probe = probing(Rc::clone(&runs));
        let (h, runtime, _recorded) = runtime_for("smoke", vec![(Some("<probe>"), probe)]).await;
        let session = runtime.session();
        drop(script_responses(
            &h,
            vec![probe_call(), answer("probed", None, None)],
        ));
        session
            .prompt("use the probe", PromptOptions::default())
            .await
            .unwrap();
        assert_eq!(runs.get(), 1, "the tool turn ran the tool");
        assert_eq!(stored_costs(&session), [json!(7.0), json!(7.0)]);
        assert!((session.get_session_stats().unwrap().cost - 14.0).abs() < f64::EPSILON);

        let before_reload = session.extension_runner().create_context();
        session.reload().await.unwrap();
        assert_eq!(
            probe_source(&session),
            ["<probe>"],
            "the reload kept the inline tool"
        );
        assert!(
            session
                .get_active_tool_names()
                .contains(&"probe".to_owned())
        );
        assert_eq!(
            before_reload.cwd().unwrap(),
            session.session_manager().borrow().get_cwd()
        );
        let before_replacement = session.extension_runner().create_context();

        let sent = Rc::new(std::cell::Cell::new(false));
        let marked = Rc::clone(&sent);
        let options = NewSessionCommandOptions {
            with_session: Some(Rc::new(move |context| {
                let marked = Rc::clone(&marked);
                Box::pin(async move {
                    context.send_message(probe_message(), None).await?;
                    marked.set(true);
                    Ok(())
                })
            })),
            ..NewSessionCommandOptions::default()
        };
        runtime.new_session(options).await.unwrap();
        assert!(sent.get(), "the fresh context sent its message");
        assert_eq!(custom_types(&runtime.session()), ["probe"]);
        let stale = before_replacement.cwd().unwrap_err().to_string();
        assert!(stale.starts_with("This extension ctx is stale"), "{stale}");
        assert_eq!(probe_source(&runtime.session()), ["<probe>"]);
    });
}

#[test]
fn command_wait_for_idle_returns_after_the_run_settles() {
    on_task_set(|_| async {
        let (h, runtime, _) = runtime_for("wait-settles", Vec::new()).await;
        let session = runtime.session();
        session.settings_manager().borrow_mut().apply_overrides(
            serde_json::from_value(json!({"retry": {"baseDelayMs": 30}})).unwrap(),
        );
        script_responses(
            &h,
            vec![
                answer("", Some(StopReason::Error), Some("overloaded_error")),
                answer("recovered", None, None),
            ],
        );
        let started = retry_started(&session);
        let prompted = Rc::clone(&session);
        let running = tokio::task::spawn_local(async move {
            prompted.prompt("question", PromptOptions::default()).await
        });
        started.notified().await;
        let runtime = Rc::new(runtime);
        let actions = runtime.command_context_actions();
        (actions.operations().wait_for_idle)().await.unwrap();
        assert!(
            !session.is_streaming(),
            "the wait outlasted the retry's backoff and the recovered answer"
        );
        running.await.unwrap().unwrap();
    });
}

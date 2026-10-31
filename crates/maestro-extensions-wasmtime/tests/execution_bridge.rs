//! Component callbacks invoked by the real registration loader and runner.
#![cfg(test)]
use maestro_extensions::{
    ExtensionRunner, Local, SessionStartEvent, SessionStartReason, create_event_bus_for,
    create_extension_runtime, load_extension_from_factory,
};
use maestro_session::Presence;
use std::{cell::RefCell, rc::Rc, sync::RwLock};

use super::{session, support};
use support::bind_prompt;

#[test]
fn real_contexts_reach_component_commands_and_hooks() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, composed_contexts());
}

/// Exercises actual callback registration, context selection and attributed hook errors.
async fn composed_contexts() {
    let path = std::env::var("MAESTRO_CONTEXT_COMPONENT").unwrap();
    let runtime = create_extension_runtime::<Local>();
    let bus = create_event_bus_for::<Local>(
        Rc::new(|_| panic!("this fixture registers no event tails")),
        Rc::new(|_| panic!("this fixture registers no bus listeners")),
    );
    let owner = load_extension_from_factory(
        crate::artifact(path.clone()),
        "/factory-only".into(),
        bus.for_extension(),
        runtime.clone(),
        Some(path.clone()),
    )
    .await
    .unwrap();
    {
        let registered = owner.read().unwrap();
        assert_eq!(
            registered.commands.keys().collect::<Vec<_>>(),
            [
                "builtin-read",
                "builtin-read-executable",
                "builtin-prepare-edit",
                "bridge-observe"
            ]
        );
        assert_eq!(
            registered.handlers.keys().collect::<Vec<_>>(),
            ["session_start"]
        );
        assert_eq!(registered.path, path);
        assert!(std::sync::Arc::ptr_eq(
            &registered.commands["bridge-observe"]
                .read()
                .unwrap()
                .source_info,
            &registered.source_info,
        ));
    }
    let owners = Rc::new(RwLock::new(vec![owner]));
    let runner = ExtensionRunner::<Local>::new(
        owners.clone(),
        runtime.clone(),
        session::runner_session("/command-domain").0,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    );
    bind_prompt(&runner, "command prompt");
    let hook_runner = ExtensionRunner::<Local>::new(
        owners,
        runtime,
        session::runner_session("/hook-domain").0,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    );
    bind_prompt(&hook_runner, "hook prompt");
    observe_callbacks(&runner, &hook_runner).await;
}

/// Rejects loading-time defaults and a command context accidentally reused for the hook.
async fn observe_callbacks(
    command_runner: &ExtensionRunner<Local>,
    hook_runner: &ExtensionRunner<Local>,
) {
    let command = command_runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    let observed = command(
        "literal args".into(),
        command_runner.create_command_context(),
    )
    .await
    .unwrap_err()
    .to_string();
    assert_eq!(
        observed,
        "command:literal args|/command-domain|command prompt"
    );
    let control = command("literal args".into(), hook_runner.create_command_context())
        .await
        .unwrap_err()
        .to_string();
    assert_eq!(control, "command:literal args|/hook-domain|hook prompt");
    assert_ne!(observed, control);
    let errors = Rc::new(RefCell::new(Vec::new()));
    let received = errors.clone();
    let _subscription = hook_runner.on_error(Rc::new(move |error| {
        received.borrow_mut().push((
            error.extension_path.clone(),
            error.event.clone(),
            error.error.clone(),
        ));
        Ok(())
    }));
    let mut event = SessionStartEvent {
        reason: SessionStartReason::Startup,
        previous_session_file: Presence::Missing,
    };
    assert!(hook_runner.emit(&mut event).await.unwrap().is_none());
    let errors = errors.borrow();
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].0,
        std::env::var("MAESTRO_CONTEXT_COMPONENT").unwrap()
    );
    assert_eq!(errors[0].1, "session_start");
    assert_eq!(errors[0].2, "hook:/hook-domain|hook prompt");
}

#[test]
fn production_factory_completes_a_positive_component_clock_wait() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = create_event_bus_for::<Local>(Rc::new(drop), Rc::new(|_| {}));
        let (_owner, runner, _events) =
            super::retained_components::loaded_on(bus.for_extension()).await;
        let command = runner.get_command("bridge-observe").unwrap().command;
        let result = (command.handler)("clock wait".into(), runner.create_command_context()).await;
        assert_eq!(result.unwrap_err().to_string(), "clock waited");
    });
}

#[test]
fn canonical_imports_reach_real_domain_owners() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, canonical_reads());
}

/// Runs both canonical context identities against populated native owners.
async fn canonical_reads() {
    use serde_json::json;
    let tails = Rc::new(RefCell::new(Vec::new()));
    let queued = tails.clone();
    let bus = create_event_bus_for::<Local>(
        Rc::new(move |tail| queued.borrow_mut().push(tail)),
        Rc::new(|_| panic!("no reports")),
    );
    let compactions = Rc::new(RefCell::new(Vec::new()));
    let received = compactions.clone();
    let _compaction_subscription = bus.for_extension().on(
        "canonical-compaction",
        Rc::new(move |data, _| {
            received.borrow_mut().push((*data).clone());
            Ok(None)
        }),
    );
    let credential_calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let (session, history) = session::runner_session_with_config(
        "/canonical-reads",
        maestro_models::SharedHandle::new(super::canonical_api::Credentials(
            credential_calls.clone(),
        )),
    );
    let root = seed_history(&mut history.borrow_mut());
    let model = super::model_support::model("literal model");
    *session.model_registry.get_all().write().unwrap() = vec![model.clone()];
    let (owner, runner, events) =
        super::retained_components::loaded_in(bus.for_extension(), session).await;
    let runner = Rc::new(runner);
    let effects = Rc::new(RefCell::new(Vec::new()));
    let compact = bind_reads(&runner, model.clone(), &effects);
    let descriptor = model.read().unwrap().clone();
    let expected = json!({
        "cwd": "/canonical-reads", "prompt": "canonical imports", "hasUi": false,
        "idle": false, "pending": true, "aborted": true,
        "usage": {"tokens": 12.5, "contextWindow": 100.0, "percent": 12.5},
        "model": descriptor, "history": expected_history(&*history.borrow(), &root),
        "catalog": {"all": [descriptor.clone()], "available": [], "found": descriptor, "missing": null},
    });
    observe_reads(&runner, &expected).await;
    complete_compactions(&compact, &compactions);
    super::canonical_commands::observe(&runner).await;
    super::canonical_api::observe(&owner, &runner, &events, &history, &tails).await;
    assert_eq!(*credential_calls.lock().unwrap(), ["CANONICAL_API_KEY"]);
    assert_eq!(
        *effects.borrow(),
        ["abort", "shutdown", "abort", "shutdown"]
    );
}

/// Publishes a labeled root and populated arbitrary entry payloads.
fn seed_history(history: &mut maestro_session::SessionManager) -> String {
    let root = history
        .append_custom_entry(
            "literal root",
            Presence::Present(serde_json::json!({"n": 2.5})),
        )
        .unwrap();
    history
        .append_label_change(&root, Presence::Present("literal label"))
        .unwrap();
    seed_payloads(history);
    history.append_session_info("literal name").unwrap();
    root
}

/// Binds visible effects and nondefault values to the actual runner.
fn bind_reads(
    runner: &ExtensionRunner<Local>,
    model: std::sync::Arc<RwLock<maestro_models::Model>>,
    effects: &Rc<RefCell<Vec<&'static str>>>,
) -> Rc<RefCell<Vec<maestro_extensions::CompactOptions>>> {
    use maestro_extensions::{ExtensionContextActions, ExtensionExecution, types::ProviderActions};
    let signal = maestro_models::Cancellation::new();
    signal.abort();
    let aborted = effects.clone();
    let shutdown = effects.clone();
    let compactions = Rc::new(RefCell::new(Vec::new()));
    let received = compactions.clone();
    runner
        .bind_core(
            Local::unbound_actions(),
            ExtensionContextActions {
                get_model: Rc::new(move || Ok(Some(model.clone()))),
                compact: Rc::new(move |options| {
                    received.borrow_mut().push(options.unwrap());
                    Ok(())
                }),
                is_idle: Rc::new(|| Ok(false)),
                get_signal: Rc::new(move || Ok(Some(signal.clone()))),
                abort: Rc::new(move || {
                    aborted.borrow_mut().push("abort");
                    Ok(())
                }),
                has_pending_messages: Rc::new(|| Ok(true)),
                shutdown: Rc::new(move || {
                    shutdown.borrow_mut().push("shutdown");
                    Ok(())
                }),
                get_context_usage: Rc::new(|| {
                    Ok(Some(maestro_extensions::ContextUsage {
                        tokens: Some(12.5),
                        context_window: 100.0,
                        percent: Some(12.5),
                    }))
                }),
                get_system_prompt: Rc::new(|| Ok("canonical imports".into())),
            },
            ProviderActions::default(),
        )
        .unwrap();
    compactions
}

/// Calls actual retained guest notifications after the originating command and hook end.
fn complete_compactions(
    options: &Rc<RefCell<Vec<maestro_extensions::CompactOptions>>>,
    received: &Rc<RefCell<Vec<serde_json::Value>>>,
) {
    let options = std::mem::take(&mut *options.borrow_mut());
    assert_eq!(options.len(), 2);
    let payload = serde_json::json!({"summary":"literal completion", "firstKeptEntryId":"literal kept", "tokensBefore":12.5, "details":{"z":[false,null], "a":2.5}});
    for option in options {
        assert_eq!(
            option.custom_instructions.as_deref(),
            Some("literal compact instructions")
        );
        option.on_complete.unwrap()(serde_json::from_value(payload.clone()).unwrap()).unwrap();
        option.on_error.unwrap()("literal compact failure".into()).unwrap();
    }
    let error = serde_json::json!({"error":"literal compact failure"});
    assert_eq!(
        *received.borrow(),
        [payload.clone(), error.clone(), payload, error]
    );
}

/// Reads the populated owner directly, independently of the component conversions.
fn expected_history(
    reader: &dyn maestro_session::session_manager::ReadonlySessionManager,
    root: &str,
) -> serde_json::Value {
    use serde_json::json;
    let mut expected_history = json!({
        "cwd": "/canonical-reads", "dir": reader.get_session_dir(),
        "id": reader.get_session_id(), "file": null,
        "leafId": reader.get_leaf_id(), "leaf": reader.get_leaf_entry(),
        "entry": reader.get_entry(root), "missingEntry": null,
        "label": "literal label", "missingLabel": null,
        "branch": reader.get_branch(None).unwrap(),
        "rootBranch": reader.get_branch(Some(root)).unwrap(),
        "header": reader.get_header(), "entries": reader.get_entries(),
        "name": "literal name", "tree": native_tree(reader.get_tree()),
    });
    // The shared header represents its numeric version as f64.
    expected_history["header"]["version"] = json!(3.0);
    expected_history
}

/// Verifies command and event delivery share values without sharing resource identities.
async fn observe_reads(runner: &ExtensionRunner<Local>, expected: &serde_json::Value) {
    let command = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    let response = command("imports".into(), runner.create_command_context())
        .await
        .unwrap_err()
        .to_string();
    let observed: serde_json::Value = serde_json::from_str(&response)
        .unwrap_or_else(|_| panic!("expected canonical read report, got {response}"));
    assert_eq!(&observed, expected);
    let errors = Rc::new(RefCell::new(Vec::new()));
    let reports = errors.clone();
    let _subscription = runner.on_error(Rc::new(move |error| {
        reports.borrow_mut().push(error.error.clone());
        Ok(())
    }));
    let mut event = SessionStartEvent {
        reason: SessionStartReason::Startup,
        previous_session_file: Presence::Missing,
    };
    runner.emit(&mut event).await.unwrap();
    assert_eq!(errors.borrow().len(), 1);
    let observed: serde_json::Value = serde_json::from_str(&errors.borrow()[0]).unwrap();
    assert_eq!(&observed, expected);
}

/// Projects borrowed native tree data independently of the component resource traversal.
fn native_tree(
    nodes: Vec<maestro_session::session_manager::SessionTreeNode<'_>>,
) -> Vec<serde_json::Value> {
    nodes
        .into_iter()
        .map(|mut node| {
            serde_json::json!({
                "entry": node.entry, "label": node.label, "labelTimestamp": node.label_timestamp,
                "children": native_tree(std::mem::take(&mut node.children)),
            })
        })
        .collect()
}

/// Populates all opaque entry fields through their native writer, not a synthetic reader.
fn seed_payloads(history: &mut maestro_session::SessionManager) {
    use maestro_session::session_manager::{
        BranchSummaryOptions, CompactionOptions, ReadonlySessionManager,
    };
    use serde_json::json;
    for payload in [
        Presence::Present(json!({"z": {"b": [false, null, "Ω"]}, "a": 1})),
        Presence::Present(json!([null, true, {"z": 1, "a": 2}])),
        Presence::Present(json!(2.5)),
        Presence::Present(json!(9_007_199_254_740_993_u64)),
        Presence::Present(json!("literal \"text\"")),
        Presence::Null,
        Presence::Missing,
    ] {
        history
            .append_custom_entry("payload", payload.clone())
            .unwrap();
        history
            .append_compaction(
                "summary",
                "keep",
                2.5,
                CompactionOptions {
                    details: payload.clone(),
                    ..Default::default()
                },
            )
            .unwrap();
        let parent = history.get_leaf_id().unwrap().to_owned();
        history
            .branch_with_summary(
                Some(&parent),
                "summary",
                BranchSummaryOptions {
                    details: payload.clone(),
                    ..Default::default()
                },
            )
            .unwrap();
        history
            .append_custom_message_entry(
                "payload",
                maestro_models::UserContent::Text("content".into()),
                true,
                payload,
            )
            .unwrap();
    }
}

#[test]
fn retained_command_context_resumes_then_reports_stale() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, super::retained_contexts::retained(false));
}

#[test]
fn caught_import_error_keeps_same_store_work_usable() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, super::retained_contexts::retained(true));
}

#[test]
fn failed_factory_initialization_releases_admissions() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, super::startup::failed());
}

#[test]
fn retained_capabilities_release_after_their_final_consumer() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, super::lifetimes::release());
}

//! Per-member context observations through the real author component and runner.
#![cfg(test)]
use maestro_extensions::{
    ExtensionRunner, Local, SessionStartEvent, SessionStartReason, create_event_bus_for,
    create_extension_runtime,
};
use maestro_session::Presence;
use maestro_session::session_manager::ReadonlySessionManager;
use serde_json::{Value, json};
use std::rc::Rc;

mod bound;
mod commands;

#[test]
fn native_context_bound_actions_preserve_values_and_errors() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = create_event_bus_for::<Local>(Rc::new(drop), Rc::new(|error| panic!("{error}")));
        let (_, runner, _) = super::retained_components::loaded_in(
            bus.for_extension(),
            super::session::runner_session("/work").0,
        )
        .await;
        bound::observe(&runner, &bus.for_extension()).await;
        commands::observe(&runner).await;
    });
}

#[test]
fn native_context_unbound_defaults_match_source() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = create_event_bus_for::<Local>(Rc::new(drop), Rc::new(|error| panic!("{error}")));
        let (session, history) = super::session::runner_session("/work");
        let identity = json!({"cwd":"/work","id":history.borrow().get_session_id()});
        let model = super::model_support::model("unbound registry identity");
        let descriptor = serde_json::to_value(&*model.read().unwrap()).unwrap();
        *session.model_registry.get_all().write().unwrap() = vec![model];
        let (_, runner, _) = super::retained_components::unbound_with_runtime(
            bus.for_extension(),
            session,
            "/different-factory-directory",
            create_extension_runtime::<Local>(),
        )
        .await;
        let mut failures = Vec::new();
        for (id, request, expected) in unbound_cases(identity, &descriptor) {
            let expected = if id.starts_with('C') {
                json!({"value":expected,"callbacks":0})
            } else {
                expected
            };
            check(
                &mut failures,
                id,
                &observe(&runner, request).await,
                &Ok(expected),
            );
        }
        check(
            &mut failures,
            "B016",
            &runner.shutdown().map_err(|e| e.to_string()),
            &Ok(()),
        );
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    });
}

/// Case identifiers map literal expected values to independently executed component imports.
fn unbound_cases(history: Value, model: &Value) -> Vec<(&'static str, &'static str, Value)> {
    vec![
        ("B001", "ui", Value::Null),
        ("B002", "hasUI", json!(false)),
        ("B003", "cwd", json!("/work")),
        ("B004", "sessionManager", history),
        ("B005", "modelRegistry", json!([model])),
        ("B006", "model", Value::Null),
        ("B007", "isIdle", json!(true)),
        ("B008", "signal", Value::Null),
        ("B009", "abort", Value::Null),
        ("B010", "hasPendingMessages", json!(false)),
        ("B011", "shutdown", Value::Null),
        ("B012", "getContextUsage", Value::Null),
        ("B013", "compact", Value::Null),
        ("B014", "getSystemPrompt", json!("")),
        ("B015", "compact full", Value::Null),
        ("C001", "waitForIdle", Value::Null),
        ("C002", "newSession", json!({"cancelled": false})),
        ("C003", "fork", json!({"cancelled": false})),
        ("C004", "navigateTree", json!({"cancelled": false})),
        ("C005", "switchSession", json!({"cancelled": false})),
        ("C006", "reload", Value::Null),
        ("C007", "waitForIdle full", Value::Null),
        ("C008", "newSession full", json!({"cancelled": false})),
        ("C009", "fork full", json!({"cancelled": false})),
        ("C010", "navigateTree full", json!({"cancelled": false})),
        ("C011", "switchSession full", json!({"cancelled": false})),
        ("C012", "reload full", Value::Null),
    ]
}

/// Uses the command handler or typed hook with a fresh native context on every call.
async fn observe(runner: &ExtensionRunner<Local>, request: &str) -> Result<Value, String> {
    let (member, _) = request.split_once(' ').unwrap_or((request, "none"));
    let answer = if matches!(
        member,
        "waitForIdle" | "newSession" | "fork" | "navigateTree" | "switchSession" | "reload"
    ) {
        let command = runner.get_command("bridge-observe").unwrap().command;
        (command.handler)(
            format!("context matrix {request}"),
            runner.create_command_context(),
        )
        .await
        .unwrap_err()
        .to_string()
    } else {
        observe_hook(runner, request).await
    };
    serde_json::from_str(&answer).unwrap_or_else(|error| panic!("{request}: {error}: {answer}"))
}

/// Selects an ordinary context through the actual session-start event dispatcher.
async fn observe_hook(runner: &ExtensionRunner<Local>, request: &str) -> String {
    let errors = Rc::new(std::cell::RefCell::new(Vec::new()));
    let received = errors.clone();
    let unsubscribe = runner.on_error(Rc::new(move |error| {
        received.borrow_mut().push((
            error.event.clone(),
            error.extension_path.clone(),
            error.error.clone(),
        ));
        Ok(())
    }));
    let mut event = SessionStartEvent {
        reason: SessionStartReason::Startup,
        previous_session_file: Presence::Present(format!("context matrix {request}")),
    };
    assert!(runner.emit(&mut event).await.unwrap().is_none());
    unsubscribe();
    let mut errors = errors.borrow_mut();
    assert_eq!(errors.len(), 1, "{request}: attributed hook result");
    let (event, path, answer) = errors.pop().unwrap();
    assert_eq!(event, "session_start");
    assert_eq!(path, std::env::var("MAESTRO_CONTEXT_COMPONENT").unwrap());
    answer
}

/// Keeps all case failures visible when qualifying a table of independent operations.
fn check<T: PartialEq + std::fmt::Debug>(
    failures: &mut Vec<String>,
    id: &str,
    actual: &T,
    expected: &T,
) {
    if actual != expected {
        failures.push(format!("{id}: expected {expected:?}, received {actual:?}"));
    }
}

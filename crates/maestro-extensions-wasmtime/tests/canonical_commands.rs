//! Canonical command imports execute against the native application bindings.
#![cfg(test)]
use maestro_extensions::{ExtensionCommandContextActions, ExtensionRunner, Local};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// Ordered application invocations, not adapter-generated observations.
type Trace = Rc<RefCell<Vec<String>>>;
/// The native local continuation future.
type Work = std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<(), maestro_extensions::EventError>>>,
>;
/// Callback supplied by the real command operation.
type Continuation = Rc<dyn Fn(maestro_extensions::ReplacedSessionContext) -> Work>;

/// Extends the canonical import witness without constructing another component.
pub(super) async fn observe(runner: &ExtensionRunner<Local>) {
    use maestro_session::session_manager::ReadonlySessionManager;
    let trace: Trace = Rc::default();
    let (_, history) = super::session::runner_session("/canonical-setup");
    let file = std::env::temp_dir().join(format!(
        "maestro-canonical-{}.jsonl",
        history.borrow().get_session_id()
    ));
    assert!(!file.exists());
    bind(runner, &trace, history.clone());
    let handler = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    let answer = handler(
        format!("command imports:{}", file.display()),
        runner.create_command_context(),
    )
    .await
    .unwrap_err()
    .to_string();
    let answer: Value =
        serde_json::from_str(&answer).unwrap_or_else(|error| panic!("{error}: {answer}"));
    assert_eq!(
        answer,
        json!({"new": false, "fork": true, "navigate": true, "switch": false, "reload": "literal reload failure"})
    );
    assert_eq!(
        *trace.borrow(),
        [
            "wait",
            "setup",
            "user:new:/replacement-new",
            "custom:new",
            "user:fork:/replacement-fork",
            "custom:fork",
            "navigate",
            "user:switch:/replacement-switch",
            "custom:switch",
            "reload"
        ]
    );
    check_history(&history.borrow());
    assert!(!file.exists(), "memory-only setup must not create a file");
}

/// Supplies real application operations and invokes callbacks with their actual owners.
fn bind(
    runner: &ExtensionRunner<Local>,
    trace: &Trace,
    history: Rc<RefCell<maestro_session::SessionManager>>,
) {
    let (wait, new, fork, navigate, switch, reload) = (
        trace.clone(),
        trace.clone(),
        trace.clone(),
        trace.clone(),
        trace.clone(),
        trace.clone(),
    );
    runner.bind_command_context(Some(ExtensionCommandContextActions::new(
        maestro_extensions::CommandContextOperations {
            wait_for_idle: Rc::new(move || {
                wait.borrow_mut().push("wait".into());
                Box::pin(async { Ok(()) })
            }),
            new_session: Rc::new(move |options| {
                let options = options.unwrap();
                assert_eq!(options.parent_session.as_deref(), Some("literal parent"));
                let (trace, history) = (new.clone(), history.clone());
                Box::pin(async move {
                    options.setup.unwrap()(history).await?;
                    trace.borrow_mut().push("setup".into());
                    with_replacement(options.with_session.unwrap(), "new", &trace).await?;
                    Ok(maestro_extensions::types::SessionChangeResult { cancelled: false })
                })
            }),
            fork: Rc::new(move |id, options| {
                assert_eq!(id, "literal fork");
                let options = options.unwrap();
                assert_eq!(
                    options.position,
                    Some(maestro_extensions::ForkPosition::Before)
                );
                let trace = fork.clone();
                Box::pin(async move {
                    with_replacement(options.with_session.unwrap(), "fork", &trace).await?;
                    Ok(maestro_extensions::types::SessionChangeResult { cancelled: true })
                })
            }),
            navigate_tree: navigation(navigate),
            switch_session: Rc::new(move |path, options| {
                assert_eq!(path, " literal session ");
                let with = options.unwrap().with_session.unwrap();
                let trace = switch.clone();
                Box::pin(async move {
                    with_replacement(with, "switch", &trace).await?;
                    Ok(maestro_extensions::types::SessionChangeResult { cancelled: false })
                })
            }),
            reload: Rc::new(move || {
                reload.borrow_mut().push("reload".into());
                Box::pin(async { Err("literal reload failure".into()) })
            }),
        },
    )));
}

/// Checks navigation operands at their real application consumer.
fn navigation(navigate: Trace) -> maestro_extensions::types::NavigateTreeHandler {
    Rc::new(move |id, options| {
        assert_eq!(id, "literal target");
        let options = options.unwrap();
        assert_eq!(options.summarize, Some(true));
        assert_eq!(
            options.custom_instructions.as_deref(),
            Some(" literal instructions ")
        );
        assert_eq!(options.replace_instructions, Some(false));
        assert_eq!(options.label.as_deref(), Some("literal navigation label"));
        navigate.borrow_mut().push("navigate".into());
        Box::pin(async { Ok(maestro_extensions::types::SessionChangeResult { cancelled: true }) })
    })
}

/// Runs a supplied continuation against a fresh real command context and message effects.
async fn with_replacement(
    with: Continuation,
    operation: &'static str,
    trace: &Trace,
) -> Result<(), maestro_extensions::EventError> {
    let cwd = format!("/replacement-{operation}");
    let fresh = ExtensionRunner::<Local>::new(
        Rc::new(std::sync::RwLock::new(Vec::new())),
        maestro_extensions::create_extension_runtime::<Local>(),
        super::session::runner_session(&cwd).0,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    );
    let (custom, user) = (trace.clone(), trace.clone());
    let replaced = maestro_extensions::ReplacedSessionContext::new(
        fresh.create_command_context(),
        Rc::new(move |message, options| {
            assert!(options.is_none());
            assert_eq!(
                serde_json::to_value(message).unwrap(),
                json!({
                    "customType": operation, "content": format!("/replacement-{operation}"),
                    "display": false, "details": {"n": 2.5},
                })
            );
            custom.borrow_mut().push(format!("custom:{operation}"));
            Box::pin(async { Ok(()) })
        }),
        Rc::new(move |content, options| {
            assert!(options.is_none());
            assert_eq!(
                serde_json::to_value(content).unwrap(),
                json!(format!("{operation}:/replacement-{operation}"))
            );
            user.borrow_mut()
                .push(format!("user:{operation}:/replacement-{operation}"));
            Box::pin(async { Ok(()) })
        }),
    );
    with(replaced).await
}

/// Checks literal records and projections independently at the native session owner.
fn check_history(history: &maestro_session::SessionManager) {
    use maestro_session::session_manager::ReadonlySessionManager;
    let entries: Vec<Value> = history.get_entries().into_iter().cloned().collect();
    assert_eq!(entries.len(), 4);
    assert_eq!(
        entries[0]["message"],
        json!({"role": "user", "content": "literal setup message", "timestamp": 12.5})
    );
    assert_eq!(entries[1]["summary"], "literal branch summary");
    assert_eq!(entries[1]["details"], json!({"s": 2.5}));
    assert_eq!(entries[1]["fromHook"], false);
    assert_eq!(entries[2]["label"], "literal setup label");
    let report = &entries[3]["data"];
    assert_eq!(report["header"]["id"], "literal setup id");
    assert_eq!(report["header"]["parentSession"], "literal setup parent");
    let written = report["entries"].as_array().unwrap();
    check_written(written);
    assert_eq!(report["children"], json!([written[1], written[9]]));
    assert_eq!(
        report["context"],
        serde_json::to_value(history.build_session_context().unwrap()).unwrap()
    );
}

/// Verifies each written entry before the selected-path extraction.
fn check_written(written: &[Value]) {
    assert_eq!(
        written
            .iter()
            .map(|entry| entry["type"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "message",
            "message",
            "thinking_level_change",
            "model_change",
            "compaction",
            "custom",
            "custom_message",
            "label",
            "session_info",
            "branch_summary"
        ]
    );
    assert_eq!(
        written[1]["message"]["details"],
        json!({"nested": [2.5, null]})
    );
    assert_eq!(written[2]["thinkingLevel"], "high");
    assert_eq!(written[3]["provider"], "literal provider");
    assert_eq!(written[3]["modelId"], "literal model");
    assert_eq!(written[4]["summary"], "literal compact");
    assert_eq!(written[4]["tokensBefore"], 12.5);
    assert_eq!(written[4]["details"], json!({"z": [false, null], "a": 2.5}));
    assert_eq!(written[4]["fromHook"], true);
    assert_eq!(written[5]["data"], json!({"ready": true}));
    assert_eq!(written[6]["details"], json!([2.5, null]));
    assert_eq!(written[6]["display"], false);
    assert_eq!(written[8]["name"], "literal setup name");
}

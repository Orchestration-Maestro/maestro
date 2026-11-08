//! Retained context reads and continued messages keep their own owners.
#![cfg(test)]
use maestro_extensions::{
    CommandContextOperations, ContextUsage, CustomMessageInput, ExtensionCommandContextActions,
    ExtensionContextActions, ExtensionExecution, ExtensionRunner, Local, ReplacedSessionContext,
    create_event_bus_for,
    types::{ReloadHandler, SessionChangeResult},
};
use maestro_models::UserContent;
use maestro_session::message_records::{SendMessageOptions, SendUserMessageOptions};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{Arc, RwLock},
};

use crate::components::local;
use crate::retained_contexts::STALE;

/// Live values the supplied base and command bindings answer with.
struct Bindings {
    /// Values the first base binding supplies.
    first: Value,
    /// Values the second base binding supplies, deliberately unequal.
    second: Value,
    /// Native command operations this run invoked.
    calls: Rc<Cell<usize>>,
}

/// Loads the component over a seeded history, catalog and model.
async fn loaded(
    cwd: &str,
) -> (
    ExtensionRunner<Local>,
    Rc<RefCell<maestro_session::SessionManager>>,
) {
    let bus = create_event_bus_for::<Local>(Rc::new(|_| {}), Rc::new(|_| {}));
    let (session, history) = super::session::runner_session(cwd);
    *session.model_registry.get_all().write().unwrap() =
        vec![super::model_support::model("registry model")];
    let (_, runner, _) = super::retained_components::loaded_in(bus.for_extension(), session).await;
    (runner, history)
}

/// A UI context whose only supplied capability is the live theme.
struct Ui;
impl maestro_extensions::types::ExtensionUIContextPort for Ui {
    fn theme(&self) -> maestro_theme::LiveTheme {
        maestro_theme::LiveTheme::default()
    }
}

/// A suspension the driver releases once it has changed the bindings.
fn suspended(
    entered: tokio::sync::mpsc::UnboundedSender<tokio::sync::oneshot::Sender<()>>,
) -> ReloadHandler {
    Rc::new(move || {
        let (release, done) = tokio::sync::oneshot::channel();
        entered.send(release).unwrap();
        Box::pin(async move {
            done.await?;
            Ok(())
        })
    })
}

/// Supplies every base read from one distinguishable value set.
fn base_actions(
    model: &Arc<RwLock<maestro_models::Model>>,
    values: &Value,
) -> ExtensionContextActions {
    let model = model.clone();
    let values = values.clone();
    let read = move |member: &str| values[member].clone();
    let (idle, signal, pending, tokens, prompt) = (
        read("isIdle"),
        read("signal"),
        read("hasPendingMessages"),
        read("getContextUsage"),
        read("getSystemPrompt"),
    );
    let cancellation = maestro_extensions::Cancellation::new();
    if signal.as_bool() == Some(true) {
        cancellation.abort();
    }
    ExtensionContextActions {
        get_model: Rc::new(move || Ok(Some(model.clone()))),
        is_idle: Rc::new(move || Ok(idle.as_bool().unwrap())),
        get_signal: Rc::new(move || {
            Ok(signal
                .as_bool()
                .filter(|aborted| *aborted)
                .map(|_| cancellation.clone()))
        }),
        has_pending_messages: Rc::new(move || Ok(pending.as_bool().unwrap())),
        get_context_usage: Rc::new(move || {
            Ok(Some(ContextUsage {
                tokens: Some(tokens.as_f64().unwrap()),
                context_window: 1000.0,
                percent: None,
            }))
        }),
        get_system_prompt: Rc::new(move || Ok(prompt.as_str().unwrap().to_owned())),
        ..ExtensionContextActions::default()
    }
}

/// Supplies the five session-replacing operations with one distinguishable result.
fn command_actions(
    calls: &Rc<Cell<usize>>,
    cancelled: bool,
    wait: ReloadHandler,
) -> ExtensionCommandContextActions {
    let counter = calls.clone();
    let reloaded = move || {
        counter.set(counter.get() + 1);
    };
    let (start, fork_at, navigate_to, switch_to) =
        (calls.clone(), calls.clone(), calls.clone(), calls.clone());
    ExtensionCommandContextActions::new(CommandContextOperations {
        wait_for_idle: wait,
        new_session: Rc::new(move |_| {
            start.set(start.get() + 1);
            Box::pin(async move { Ok(SessionChangeResult { cancelled }) })
        }),
        fork: Rc::new(move |_, _| {
            fork_at.set(fork_at.get() + 1);
            Box::pin(async move { Ok(SessionChangeResult { cancelled }) })
        }),
        navigate_tree: Rc::new(move |_, _| {
            navigate_to.set(navigate_to.get() + 1);
            Box::pin(async move { Ok(SessionChangeResult { cancelled }) })
        }),
        switch_session: Rc::new(move |_, _| {
            switch_to.set(switch_to.get() + 1);
            Box::pin(async move { Ok(SessionChangeResult { cancelled }) })
        }),
        reload: Rc::new(move || {
            reloaded();
            Box::pin(async { Ok(()) })
        }),
    })
}

#[test]
fn retained_context_reads_stay_lazy_and_reject_staleness() {
    local(async {
        let (runner, history) = loaded("/retained-lazy").await;
        let models = (
            super::model_support::model("captured model"),
            super::model_support::model("rebound model"),
        );
        let bindings = Bindings {
            first: json!({"isIdle":false,"signal":false,"hasPendingMessages":true,
                "getContextUsage":1.5,"getSystemPrompt":"prompt one"}),
            second: json!({"isIdle":true,"signal":true,"hasPendingMessages":false,
                "getContextUsage":2.5,"getSystemPrompt":"prompt two"}),
            calls: Rc::default(),
        };
        runner
            .bind_core(
                Local::unbound_actions(),
                base_actions(&models.0, &bindings.first),
                maestro_extensions::types::ProviderActions::default(),
            )
            .unwrap();
        runner.set_ui_context(Some(maestro_extensions::ExtensionUIContext::new(Rc::new(
            Ui,
        ))));
        let (entered, mut releases) = tokio::sync::mpsc::unbounded_channel();
        runner.bind_command_context(Some(command_actions(
            &bindings.calls,
            true,
            suspended(entered.clone()),
        )));
        let handler = runner
            .get_command("bridge-observe")
            .unwrap()
            .command
            .handler;
        let command = handler("retention lazy".into(), runner.create_command_context());
        let (answer, ()) = futures_util::future::join(command, async {
            for phase in 0..4 {
                let release = releases.recv().await.unwrap();
                rebind(&runner, &bindings, &models, phase, entered.clone());
                release.send(()).unwrap();
            }
        })
        .await;
        let report: Value = serde_json::from_str(&answer.unwrap_err().to_string()).unwrap();
        check_phases(&report, &history);
        assert_eq!(bindings.calls.get(), 20, "every stale read rejected early");
    });
}

/// Applies the binding change the released suspension makes visible.
fn rebind(
    runner: &ExtensionRunner<Local>,
    bindings: &Bindings,
    models: &(
        Arc<RwLock<maestro_models::Model>>,
        Arc<RwLock<maestro_models::Model>>,
    ),
    phase: usize,
    entered: tokio::sync::mpsc::UnboundedSender<tokio::sync::oneshot::Sender<()>>,
) {
    match phase {
        0 => {
            runner
                .bind_core(
                    Local::unbound_actions(),
                    base_actions(&models.1, &bindings.second),
                    maestro_extensions::types::ProviderActions::default(),
                )
                .unwrap();
            runner.set_ui_context(None);
        }
        1 => runner.bind_command_context(Some(command_actions(
            &bindings.calls,
            false,
            suspended(entered),
        ))),
        2 => {
            runner
                .bind_core(
                    Local::unbound_actions(),
                    base_actions(&models.0, &bindings.first),
                    maestro_extensions::types::ProviderActions::default(),
                )
                .unwrap();
            runner.set_ui_context(Some(maestro_extensions::ExtensionUIContext::new(Rc::new(
                Ui,
            ))));
        }
        _ => runner.invalidate(None),
    }
}

/// Checks every phase's per-member and per-operation result.
fn check_phases(report: &Value, history: &Rc<RefCell<maestro_session::SessionManager>>) {
    use maestro_session::session_manager::ReadonlySessionManager;
    let history = history.borrow();
    let id = history.get_session_id();
    let phases = report.as_array().unwrap();
    assert_eq!(phases.len(), 5);
    let identity = json!({"cwd":"/retained-lazy","id":id});
    let registry = json!(["registry model"]);
    let first_members = json!([
        {"name":"ui","ok":null}, {"name":"hasUI","ok":true},
        {"name":"cwd","ok":"/retained-lazy"},
        {"name":"sessionManager","ok":identity},
        {"name":"modelRegistry","ok":registry},
        {"name":"model","ok":"captured model"},
        {"name":"isIdle","ok":false}, {"name":"signal","ok":null},
        {"name":"hasPendingMessages","ok":true},
        {"name":"getContextUsage","ok":1.5},
        {"name":"getSystemPrompt","ok":"prompt one"},
    ]);
    // The model reader is the one captured at creation, so the second binding's
    // model never appears, while every other bound reader follows it.
    let second_members = json!([
        {"name":"ui","ok":null}, {"name":"hasUI","ok":false},
        {"name":"cwd","ok":"/retained-lazy"},
        {"name":"sessionManager","ok":identity},
        {"name":"modelRegistry","ok":registry},
        {"name":"model","ok":"captured model"},
        {"name":"isIdle","ok":true}, {"name":"signal","ok":true},
        {"name":"hasPendingMessages","ok":false},
        {"name":"getContextUsage","ok":2.5},
        {"name":"getSystemPrompt","ok":"prompt two"},
    ]);
    assert_eq!(phases[0]["members"], first_members, "phase one");
    assert_eq!(phases[1]["members"], second_members, "phase two");
    assert_eq!(phases[2]["members"], second_members, "phase three");
    assert_eq!(phases[3]["members"], first_members, "phase four");
    assert!(operations(&phases[0], true), "first commands");
    assert!(operations(&phases[1], true), "rebound base");
    assert!(operations(&phases[2], false), "rebound commands");
    assert!(operations(&phases[3], false), "rebound base again");
    assert!(stale_members(&phases[4]), "stale members");
    assert_eq!(phases[4]["wait"], json!(STALE), "stale wait");
}

/// Checks one phase's five operation results against the bound cancellation.
fn operations(phase: &Value, cancelled: bool) -> bool {
    let expected: Vec<Value> = [
        "newSession",
        "fork",
        "navigateTree",
        "switchSession",
        "reload",
    ]
    .into_iter()
    .map(|name| {
        if name == "reload" {
            json!({"name":name,"ok":null})
        } else {
            json!({"name":name,"ok":{"cancelled":cancelled}})
        }
    })
    .collect();
    phase["operations"] == json!(expected)
}

/// Checks that every guarded member reports the branded stale guidance.
fn stale_members(phase: &Value) -> bool {
    let members = phase["members"].as_array().unwrap();
    let report = members
        .iter()
        .all(|member| member["err"] == json!(STALE) && member.get("ok").is_none());
    let operations = phase["operations"].as_array().unwrap();
    report
        && operations
            .iter()
            .all(|member| member["err"] == json!(STALE))
}

/// Records the replacement messages a supplied session receives.
#[derive(Default)]
struct Received {
    /// Every message with the delivery options it carried, in arrival order.
    calls: RefCell<Vec<Value>>,
}

impl Received {
    /// Records one custom message as its record-shaped wire form.
    fn custom(&self, message: &CustomMessageInput, options: Option<SendMessageOptions>) {
        self.calls.borrow_mut().push(json!({
            "message": serde_json::to_value(message).unwrap(),
            "options": options.map(|options| serde_json::to_value(options).unwrap()),
        }));
    }
    /// Records one user message as its wire form.
    fn user(&self, content: &UserContent, options: Option<SendUserMessageOptions>) {
        self.calls.borrow_mut().push(json!({
            "message": serde_json::to_value(content).unwrap(),
            "options": options.map(|options| serde_json::to_value(options).unwrap()),
        }));
    }
}

/// Builds the replacement context whose messages keep reaching the creating session.
fn replacement(
    received: &Rc<Received>,
    replacement: &Rc<RefCell<Option<Rc<ExtensionRunner<Local>>>>>,
) -> ReplacedSessionContext {
    let session = super::session::runner_session("/messages-replacement").0;
    let runner = Rc::new(ExtensionRunner::<Local>::new(
        Rc::new(RwLock::new(Vec::new())),
        maestro_extensions::create_extension_runtime::<Local>(),
        session,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    ));
    *replacement.borrow_mut() = Some(runner.clone());
    let (custom, user) = (received.clone(), received.clone());
    ReplacedSessionContext::new(
        runner.create_command_context(),
        Rc::new(move |message, options| {
            if message.custom_type == "after-5" {
                return Box::pin(async { Err("literal receiver failure".into()) });
            }
            custom.custom(&message, options);
            Box::pin(async { Ok(()) })
        }),
        Rc::new(move |content, options| {
            user.user(&content, options);
            Box::pin(async { Ok(()) })
        }),
    )
}

#[test]
fn replaced_messages_retain_their_session_after_invalidation() {
    local(async {
        let (runner, _) = loaded("/messages-origin").await;
        let received = Rc::new(Received::default());
        let replacement_runner: Rc<RefCell<Option<Rc<ExtensionRunner<Local>>>>> =
            Rc::new(RefCell::new(None));
        let (entered, mut releases) = tokio::sync::mpsc::unbounded_channel();
        let (custom, slot) = (received.clone(), replacement_runner.clone());
        let new_session: maestro_extensions::NewSessionHandler = Rc::new(move |options| {
            let options = options.expect("present new-session options");
            assert_eq!(
                options.parent_session.as_deref(),
                Some("/sessions/parent-messages.jsonl")
            );
            let replaced = replacement(&custom, &slot);
            let continuation = options.with_session.expect("present continuation");
            Box::pin(async move {
                continuation(replaced).await?;
                Ok(SessionChangeResult { cancelled: false })
            })
        });
        runner.bind_command_context(Some(ExtensionCommandContextActions::new(
            CommandContextOperations {
                new_session,
                wait_for_idle: suspended(entered),
                ..super::context_actions::silent_command_operations()
            },
        )));
        let handler = runner
            .get_command("bridge-observe")
            .unwrap()
            .command
            .handler;
        let command = handler(
            "retention replaced-messages".into(),
            runner.create_command_context(),
        );
        let (answer, ()) = futures_util::future::join(command, async {
            let release = releases.recv().await.unwrap();
            replacement_runner
                .borrow()
                .as_ref()
                .unwrap()
                .invalidate(None);
            release.send(()).unwrap();
        })
        .await;
        let report: Value = serde_json::from_str(&answer.unwrap_err().to_string()).unwrap();
        assert_eq!(report["resolved"]["cancelled"], json!(false));
        assert_eq!(
            report["resolved"]["trace"],
            json!(["continuation:/messages-replacement"])
        );
        assert_eq!(report["guarded"], json!({"err":STALE}));
        assert_eq!(report["retained"], json!(true));
        check_received(&received, &report["after"]);
        assert!(
            runner.create_command_context().cwd().is_ok(),
            "one invalidated alias must not revoke the describing session"
        );
    });
}

/// Checks the receiver's content, options, details domain and numeric sign.
fn check_received(received: &Received, after: &Value) {
    let calls = received.calls.borrow();
    assert_eq!(calls.len(), 20, "both phases plus the retained probe");
    check_domains(&calls);
    check_user_messages(&calls, after);
}

/// Checks every details domain before and after the invalidation.
fn check_domains(calls: &[Value]) {
    for (index, details) in domain_values().into_iter().enumerate() {
        assert_eq!(
            calls[index],
            sent("before", index, details.clone()),
            "{index}"
        );
        if index != 5 {
            let offset = if index > 5 { index - 1 } else { index };
            assert_eq!(
                calls[10 + offset],
                sent("after", index, details),
                "after {index}"
            );
        }
    }
    let zero = calls[3]["message"]["details"].as_f64().unwrap();
    assert!(zero.is_sign_negative(), "signed zero survives the boundary");
}

/// Checks the authored user messages and the retained probe.
fn check_user_messages(calls: &[Value], after: &Value) {
    assert_eq!(calls[7], sent_user("before"));
    assert_eq!(calls[8], sent_blocks("before"));
    assert_eq!(calls[9], sent_steer("before"));
    assert_eq!(calls[16], sent_user("after"));
    assert_eq!(calls[17], sent_blocks("after"));
    assert_eq!(calls[18], sent_steer("after"));
    assert_eq!(
        calls[19]["message"],
        json!({"customType":"retained-probe","content":"retained probe","display":false})
    );
    assert_eq!(
        after[5],
        json!({"name":"message5","err":"literal receiver failure"})
    );
    assert_eq!(after[0], json!({"name":"message0","ok":null}));
    assert_eq!(after[7], json!({"name":"user-text","ok":null}));
    assert_eq!(after[8], json!({"name":"user-blocks","ok":null}));
    assert_eq!(after[9], json!({"name":"user-steer","ok":null}));
}

/// The details domains every accepted value set is exercised through.
fn domain_values() -> Vec<Value> {
    vec![
        Value::Null,
        json!(null),
        json!(" literal details "),
        json!(-0.0),
        json!(true),
        json!({"z":1,"a":[2.5,null]}),
        json!([1, "two", null]),
    ]
}

/// Builds the exact wire record of one numbered message.
fn sent(phase: &str, index: usize, details: Value) -> Value {
    let mut message = json!({
        "customType": format!("{phase}-{index}"),
        "content": format!("{phase} text {index}"),
        "display": index.is_multiple_of(2),
    });
    if index != 0 {
        message["details"] = details;
    }
    let options = match index % 4 {
        1 => json!({"triggerTurn":false,"deliverAs":"steer"}),
        2 => json!({"triggerTurn":true,"deliverAs":"followUp"}),
        3 => json!({"triggerTurn":false,"deliverAs":"nextTurn"}),
        _ => Value::Null,
    };
    json!({ "message": message, "options": options })
}

/// Builds the wire record of one authored text user message.
fn sent_user(phase: &str) -> Value {
    json!({ "message": format!("{phase} user text"), "options": {} })
}

/// Builds the wire record of the authored ordered user blocks.
fn sent_blocks(phase: &str) -> Value {
    json!({
        "message": [
            {"type":"text","text":format!("{phase} first")},
            {"type":"text","text":""},
            {"type":"text","text":format!("{phase} last")},
        ],
        "options": {"deliverAs":"followUp"},
    })
}

/// Builds the wire record of the authored steered user message.
fn sent_steer(phase: &str) -> Value {
    json!({ "message": format!("{phase} user steer"), "options": {"deliverAs":"steer"} })
}

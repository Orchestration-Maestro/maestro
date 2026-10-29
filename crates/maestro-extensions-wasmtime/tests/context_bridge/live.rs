//! Retained contexts select current actions without exchanging their captured model readers.
#![cfg(test)]
use maestro_extensions::{
    CommandContextOperations, ExtensionCommandContext, ExtensionCommandContextActions,
    ExtensionContextActions, ExtensionExecution, ExtensionRunner, ExtensionUIContext, Local,
    types::{ExtensionUIContextPort, ProviderActions, SessionChangeResult},
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};

/// Actual operation and UI consumers, independent of component observations.
pub(super) type Calls = Rc<RefCell<Vec<String>>>;

/// Local native command result with the public operation's concrete future shape.
type Work<T> =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, maestro_extensions::EventError>>>>;

/// Mutable selection behind the captured model reader.
type ModelReader = Rc<RefCell<Arc<RwLock<maestro_models::Model>>>>;

/// Distinguishes the installed host from the headless fallback.
struct Ui(Calls);
impl ExtensionUIContextPort for Ui {
    fn theme(&self) -> maestro_theme::LiveTheme {
        maestro_theme::LiveTheme::default()
    }
    fn notify(
        &self,
        message: String,
        _: Option<maestro_extensions::types::NotificationType>,
    ) -> Result<(), maestro_extensions::EventError> {
        self.0.borrow_mut().push(message);
        Ok(())
    }
}

/// Installs one identifiable native UI whose retained handles keep recording after reset.
pub(super) fn install(runner: &ExtensionRunner<Local>, calls: &Calls) {
    runner.set_ui_context(Some(ExtensionUIContext::new(Rc::new(Ui(calls.clone())))));
}

/// Supplies unequal typed core results and a mutable model through the original reader.
pub(super) fn core(
    runner: &ExtensionRunner<Local>,
    name: &str,
    idle: bool,
    calls: &Calls,
) -> ModelReader {
    let model = Rc::new(RefCell::new(super::super::model_support::model(name)));
    let captured = model.clone();
    let shutdown = calls.clone();
    runner
        .bind_core(
            Local::unbound_actions(),
            ExtensionContextActions {
                get_model: Rc::new(move || Ok(Some(captured.borrow().clone()))),
                is_idle: Rc::new(move || Ok(idle)),
                get_system_prompt: Rc::new(move || {
                    Ok(if idle { "B-prompt" } else { "A-prompt" }.into())
                }),
                shutdown: Rc::new(move || {
                    shutdown.borrow_mut().push("shutdown".into());
                    Ok(())
                }),
                ..Default::default()
            },
            ProviderActions::default(),
        )
        .unwrap();
    model
}

/// Records which command binding actually starts each invocation.
pub(super) fn commands(runner: &ExtensionRunner<Local>, tag: &'static str, calls: &Calls) {
    macro_rules! action {
        ($member:literal, $value:expr) => {{
            let calls = calls.clone();
            Rc::new(move || {
                calls.borrow_mut().push(format!("{tag}:{}", $member));
                Box::pin(std::future::ready(Ok($value))) as Work<_>
            })
        }};
    }
    let new = action!(
        "newSession",
        SessionChangeResult {
            cancelled: tag != "B"
        }
    );
    let fork = action!(
        "fork",
        SessionChangeResult {
            cancelled: tag != "B"
        }
    );
    let nav = action!(
        "navigateTree",
        SessionChangeResult {
            cancelled: tag != "B"
        }
    );
    let switch = action!(
        "switchSession",
        SessionChangeResult {
            cancelled: tag != "B"
        }
    );
    runner.bind_command_context(Some(ExtensionCommandContextActions::new(
        CommandContextOperations {
            wait_for_idle: action!("waitForIdle", ()),
            reload: action!("reload", ()),
            new_session: Rc::new(move |_| new()),
            fork: Rc::new(move |_, _| fork()),
            navigate_tree: Rc::new(move |_, _| nav()),
            switch_session: Rc::new(move |_, _| switch()),
        },
    )));
}

/// Calls the real registered command with an explicitly retained native command context.
pub(super) async fn command(
    runner: &ExtensionRunner<Local>,
    context: &ExtensionCommandContext,
    request: &str,
) -> Result<Value, String> {
    let handler = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    let answer = handler(format!("context matrix {request}"), context.clone())
        .await
        .unwrap_err()
        .to_string();
    serde_json::from_str(&answer).unwrap_or_else(|e| panic!("{request}: {e}: {answer}"))
}

/// Observes the ordinary hook's earlier context, not the new dispatch frame.
pub(super) async fn retained(
    runner: &ExtensionRunner<Local>,
    request: &str,
) -> Result<Value, String> {
    serde_json::from_str(&super::observe_hook(runner, &format!("retained {request}")).await)
        .unwrap()
}

/// Compares an owned model projection without pretending its wire representation is a pointer.
fn name(result: Result<Value, String>) -> Result<Value, String> {
    result.map(|value| {
        if value.is_null() {
            Value::Null
        } else {
            value["name"].clone()
        }
    })
}

/// Loads without a core binding so creation-time model capture can be distinguished.
pub(super) async fn load(cwd: &str) -> ExtensionRunner<Local> {
    let bus = maestro_extensions::create_event_bus_for::<Local>(
        Rc::new(drop),
        Rc::new(|e| panic!("{e}")),
    );
    super::super::retained_components::unbound_with_runtime(
        bus.for_extension(),
        super::super::session::runner_session(cwd).0,
        "/factory",
        maestro_extensions::create_extension_runtime::<Local>(),
    )
    .await
    .1
}

/// Qualifies captured model readers, live UI and both sibling-binding directions.
pub(super) async fn observe() {
    let runner = load("/work").await;
    let before = runner.create_command_context();
    assert_eq!(retained(&runner, "save before").await, Ok(Value::Null));
    let calls = Calls::default();
    let mut failures = Vec::new();
    let bound = models(&runner, &calls, &mut failures).await;
    ui(&runner, &calls, &mut failures).await;
    lazy(&runner, &before, &bound, &mut failures).await;
    binding(&runner, &before, &calls, &mut failures).await;
    independent(&runner, &mut failures).await;
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The captured reader is re-evaluated, rather than freezing its first model value.
async fn models(
    runner: &ExtensionRunner<Local>,
    calls: &Calls,
    failures: &mut Vec<String>,
) -> ExtensionCommandContext {
    let a = core(runner, "A", false, calls);
    let bound = runner.create_command_context();
    assert_eq!(retained(runner, "save bound").await, Ok(Value::Null));
    for (id, slot, member, expected) in [
        ("B035", "before", "model", Value::Null),
        ("B036", "before", "isIdle", json!(false)),
    ] {
        let actual = retained(runner, &format!("{slot} {member}")).await;
        super::check(failures, id, &actual, &Ok(expected));
    }
    *a.borrow_mut() = super::super::model_support::model("A:second");
    super::check(
        failures,
        "B037",
        &name(retained(runner, "bound model").await),
        &Ok(json!("A:second")),
    );
    later_models(runner, calls, failures).await;
    bound
}

/// Later core bindings change live readers but do not replace captured model readers.
async fn later_models(runner: &ExtensionRunner<Local>, calls: &Calls, failures: &mut Vec<String>) {
    core(runner, "B", true, calls);
    for (id, request, expected) in [
        ("B038", "bound model", json!("A:second")),
        ("B039", "bound isIdle", json!(true)),
        ("B040", "before model", Value::Null),
    ] {
        let result = retained(runner, request).await;
        let result = if request.ends_with("model") {
            name(result)
        } else {
            result
        };
        super::check(failures, id, &result, &Ok(expected));
    }
    assert_eq!(retained(runner, "save later").await, Ok(Value::Null));
    super::check(
        failures,
        "B041",
        &name(retained(runner, "later model").await),
        &Ok(json!("B")),
    );
}

/// Retaining a UI capability preserves its original host after acquisition is reset.
async fn ui(runner: &ExtensionRunner<Local>, calls: &Calls, failures: &mut Vec<String>) {
    install(runner, calls);
    super::check(
        failures,
        "B042",
        &retained(runner, "before ui").await,
        &Ok(Value::Null),
    );
    super::check(
        failures,
        "B042",
        &calls.borrow().clone(),
        &vec!["matrix notification".to_owned()],
    );
    calls.borrow_mut().clear();
    super::check(
        failures,
        "B043",
        &retained(runner, "before hasUI").await,
        &Ok(json!(true)),
    );
    super::check(
        failures,
        "B046",
        &retained(runner, "before saveUI").await,
        &Ok(Value::Null),
    );
    runner.set_ui_context(None);
    super::check(
        failures,
        "B044",
        &retained(runner, "before ui").await,
        &Ok(Value::Null),
    );
    super::check(failures, "B044", &calls.borrow().len(), &0);
    super::check(
        failures,
        "B045",
        &retained(runner, "before hasUI").await,
        &Ok(json!(false)),
    );
    super::check(
        failures,
        "B046",
        &retained(runner, "before retainedUI").await,
        &Ok(Value::Null),
    );
    super::check(
        failures,
        "B046",
        &calls.borrow().clone(),
        &vec!["retained notification".to_owned()],
    );
    calls.borrow_mut().clear();
}

/// Command inheritance stays lazy while its model reader remains fixed at creation.
async fn lazy(
    runner: &ExtensionRunner<Local>,
    before: &ExtensionCommandContext,
    bound: &ExtensionCommandContext,
    failures: &mut Vec<String>,
) {
    let calls = Calls::default();
    core(runner, "A", false, &calls);
    let a = runner.create_command_context();
    install(runner, &calls);
    for (id, request, expected) in [
        ("C049", "isIdle", json!(false)),
        ("C050", "ui", Value::Null),
        ("C051", "hasUI", json!(true)),
        ("C052", "model", Value::Null),
    ] {
        super::check(
            failures,
            id,
            &command(runner, before, request).await,
            &Ok(expected),
        );
    }
    super::check(
        failures,
        "C050",
        &calls.borrow().clone(),
        &vec!["matrix notification".to_owned()],
    );
    core(runner, "B", true, &calls);
    super::check(
        failures,
        "C053",
        &name(command(runner, &a, "model").await),
        &Ok(json!("A")),
    );
    super::check(
        failures,
        "C054",
        &command(runner, bound, "isIdle").await,
        &Ok(json!(true)),
    );
    super::check(
        failures,
        "C055",
        &command(runner, bound, "getSystemPrompt").await,
        &Ok(json!("B-prompt")),
    );
    reset_command_ui(runner, before, &calls, failures).await;
}

/// The inherited UI getter observes reset without invoking the old installed host.
async fn reset_command_ui(
    runner: &ExtensionRunner<Local>,
    before: &ExtensionCommandContext,
    calls: &Calls,
    failures: &mut Vec<String>,
) {
    runner.set_ui_context(None);
    calls.borrow_mut().clear();
    super::check(
        failures,
        "C056",
        &command(runner, before, "ui").await,
        &Ok(Value::Null),
    );
    super::check(failures, "C056", &calls.borrow().len(), &0);
    super::check(
        failures,
        "C057",
        &command(runner, before, "cwd").await,
        &Ok(json!("/work")),
    );
    super::check(
        failures,
        "C057",
        &command(runner, before, "reload").await,
        &Ok(json!({"value":null,"callbacks":0})),
    );
}

/// Both spellings of absence map to the same native Option; neither keeps old actions.
async fn binding(
    runner: &ExtensionRunner<Local>,
    before: &ExtensionCommandContext,
    calls: &Calls,
    failures: &mut Vec<String>,
) {
    unbinding(runner, before, calls, failures).await;
    commands(runner, "A", calls);
    commands(runner, "B", calls);
    for member in [
        "waitForIdle",
        "newSession",
        "fork",
        "navigateTree",
        "switchSession",
        "reload",
    ] {
        let result = command(runner, before, member).await;
        super::check(failures, "C045", &result.is_ok(), &true);
    }
    super::check(
        failures,
        "C045",
        &calls.borrow().clone(),
        &vec![
            "B:waitForIdle",
            "B:newSession",
            "B:fork",
            "B:navigateTree",
            "B:switchSession",
            "B:reload",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>(),
    );
    calls.borrow_mut().clear();
    commands(runner, "A", calls);
    super::check(
        failures,
        "C046",
        &command(runner, before, "switchSession").await,
        &Ok(json!({"value":{"cancelled":true},"callbacks":0})),
    );
    super::check(
        failures,
        "C046",
        &calls.borrow().clone(),
        &vec!["A:switchSession".to_owned()],
    );
    calls.borrow_mut().clear();
    admitted(runner, before, calls, failures).await;
    siblings(runner, before, calls, failures).await;
}

/// Unbinding restores defaults for each command without calling old application operations.
async fn unbinding(
    runner: &ExtensionRunner<Local>,
    before: &ExtensionCommandContext,
    calls: &Calls,
    failures: &mut Vec<String>,
) {
    for first in [31, 38] {
        commands(runner, "A", calls);
        runner.bind_command_context(None);
        for (offset, member) in [
            "waitForIdle",
            "newSession",
            "fork",
            "navigateTree",
            "switchSession",
            "reload",
        ]
        .iter()
        .enumerate()
        {
            let value = if matches!(*member, "waitForIdle" | "reload") {
                Value::Null
            } else {
                json!({"cancelled":false})
            };
            super::check(
                failures,
                &format!("C{:03}", first + offset),
                &command(runner, before, member).await,
                &Ok(json!({"value":value,"callbacks":0})),
            );
            super::check(
                failures,
                &format!("C{:03}", first + offset),
                &calls.borrow().len(),
                &0,
            );
        }
        super::check(
            failures,
            &format!("C{:03}", first + 6),
            &calls.borrow().len(),
            &0,
        );
    }
}

/// The selected operation owns its suspended work despite a later rebind.
async fn admitted(
    runner: &ExtensionRunner<Local>,
    context: &ExtensionCommandContext,
    calls: &Calls,
    failures: &mut Vec<String>,
) {
    let (entered, mut admitted) = tokio::sync::mpsc::unbounded_channel();
    let fork_calls = calls.clone();
    runner.bind_command_context(Some(ExtensionCommandContextActions::new(
        CommandContextOperations {
            fork: Rc::new(move |_, _| {
                fork_calls.borrow_mut().push("A:fork".into());
                let (resume, done) = tokio::sync::oneshot::channel();
                entered.send(resume).unwrap();
                Box::pin(async move {
                    done.await.unwrap();
                    Ok(SessionChangeResult { cancelled: true })
                })
            }),
            ..super::super::context_actions::silent_command_operations()
        },
    )));
    let (result, ()) = futures_util::future::join(command(runner, context, "fork"), async {
        let resume = admitted.recv().await.unwrap();
        commands(runner, "B", calls);
        resume.send(()).unwrap();
    })
    .await;
    super::check(
        failures,
        "C047",
        &result,
        &Ok(json!({"value":{"cancelled":true},"callbacks":0})),
    );
    super::check(
        failures,
        "C047",
        &calls.borrow().clone(),
        &vec!["A:fork".to_owned()],
    );
    super::check(
        failures,
        "C048",
        &command(runner, context, "fork").await,
        &Ok(json!({"value":{"cancelled":false},"callbacks":0})),
    );
    super::check(
        failures,
        "C048",
        &calls.borrow().clone(),
        &vec!["A:fork".to_owned(), "B:fork".to_owned()],
    );
    calls.borrow_mut().clear();
}

/// Core publication leaves command actions intact; command publication leaves core intact.
async fn siblings(
    runner: &ExtensionRunner<Local>,
    context: &ExtensionCommandContext,
    calls: &Calls,
    failures: &mut Vec<String>,
) {
    commands(runner, "A", calls);
    core(runner, "B", true, calls);
    super::check(
        failures,
        "C088",
        &command(runner, context, "isIdle").await,
        &Ok(json!(true)),
    );
    super::check(
        failures,
        "C088",
        &command(runner, context, "switchSession").await,
        &Ok(json!({"value":{"cancelled":true},"callbacks":0})),
    );
    super::check(
        failures,
        "C088",
        &calls.borrow().clone(),
        &vec!["A:switchSession".to_owned()],
    );
    calls.borrow_mut().clear();
    commands(runner, "C", calls);
    for (member, expected) in [
        ("isIdle", json!(true)),
        ("getSystemPrompt", json!("B-prompt")),
        (
            "switchSession",
            json!({"value":{"cancelled":true},"callbacks":0}),
        ),
    ] {
        super::check(
            failures,
            "C089",
            &command(runner, context, member).await,
            &Ok(expected),
        );
    }
    super::check(
        failures,
        "C089",
        &calls.borrow().clone(),
        &vec!["C:switchSession".to_owned()],
    );
}

/// Staleness stays runner-local while unguarded metadata remains available.
async fn independent(runner: &ExtensionRunner<Local>, failures: &mut Vec<String>) {
    let other = load("/b").await;
    runner.invalidate(None);
    let stale = Err(super::super::retained_contexts::STALE.to_owned());
    super::check(
        failures,
        "B083",
        &retained(runner, "before cwd").await,
        &stale,
    );
    super::check(
        failures,
        "B084",
        &super::observe(&other, "cwd").await,
        &Ok(json!("/b")),
    );
    super::check(
        failures,
        "B085",
        &super::observe(&other, "isIdle").await,
        &Ok(json!(true)),
    );
    assert_eq!(retained(runner, "save second").await, Ok(Value::Null));
    super::check(
        failures,
        "B086",
        &retained(runner, "second cwd").await,
        &stale,
    );
    super::check(
        failures,
        "B087",
        &runner
            .get_flags()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        &vec![
            "fail-start",
            "load-time-inputs",
            "provider-operations",
            "catalog-construction",
            "constructor-auth-path",
            "constructor-model-path",
        ],
    );
    super::check(
        failures,
        "B088",
        &runner.get_extension_paths(),
        &vec![std::env::var("MAESTRO_CONTEXT_COMPONENT").unwrap()],
    );
}

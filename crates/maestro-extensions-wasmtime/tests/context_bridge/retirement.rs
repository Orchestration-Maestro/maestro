//! Catchable retirement errors leave admitted work and independently retained owners intact.
#![cfg(test)]
use super::live::{Calls, command, retained};
use maestro_extensions::{
    ExtensionAPI, ExtensionCommandContextActions, ExtensionContextActions, ExtensionExecution,
    ExtensionRunner, Local, ReplacedSessionContext,
    types::{ProviderActions, RegisteredCommandOptions, SessionChangeResult},
};
use serde_json::{Value, json};
use std::{rc::Rc, sync::Arc};

/// Complete branded guidance is compared byte-for-byte across the component boundary.
const STALE: &str = super::super::retained_contexts::STALE;

/// Loads the existing real fixture while retaining its native registration facade.
async fn load() -> (
    ExtensionRunner<Local>,
    ExtensionAPI<Local>,
    Rc<maestro_extensions::ExtensionRuntime<Local>>,
) {
    let bus = maestro_extensions::create_event_bus_for::<Local>(
        Rc::new(drop),
        Rc::new(|e| panic!("{e}")),
    );
    let runtime = maestro_extensions::create_extension_runtime::<Local>();
    let (owner, runner, events) = super::super::retained_components::unbound_with_runtime(
        bus.for_extension(),
        super::super::session::runner_session("/work").0,
        "/factory",
        runtime.clone(),
    )
    .await;
    let api = ExtensionAPI::new(&owner, runtime.clone(), events, "/factory".into());
    (runner, api, runtime)
}

/// Counts every guarded core operation so errors cannot conceal accidental application calls.
fn bind(runner: &ExtensionRunner<Local>, calls: &Calls) {
    macro_rules! action {
        ($member:literal, $value:expr) => {{
            let calls = calls.clone();
            Rc::new(move || {
                calls.borrow_mut().push($member.into());
                Ok($value)
            })
        }};
    }
    let compact = calls.clone();
    runner
        .bind_core(
            Local::unbound_actions(),
            ExtensionContextActions {
                get_model: action!("model", None),
                is_idle: action!("isIdle", false),
                get_signal: action!("signal", None),
                abort: action!("abort", ()),
                has_pending_messages: action!("hasPendingMessages", true),
                shutdown: action!("shutdown", ()),
                get_context_usage: action!("getContextUsage", None),
                get_system_prompt: action!("getSystemPrompt", "bound".into()),
                compact: Rc::new(move |_| {
                    compact.borrow_mut().push("compact".into());
                    Ok(())
                }),
            },
            ProviderActions::default(),
        )
        .unwrap();
    super::live::commands(runner, "A", calls);
    super::live::install(runner, calls);
}

/// Qualifies guarded base and command members, new stale contexts and admitted completion.
pub(super) async fn observe() {
    let (runner, api, _) = load().await;
    let calls = Calls::default();
    bind(&runner, &calls);
    let context = runner.create_command_context();
    assert_eq!(retained(&runner, "save old").await, Ok(Value::Null));
    let mut failures = Vec::new();
    admitted(&runner, &context, &calls, &mut failures).await;
    guarded(&runner, &context, &calls, &mut failures).await;
    native_raw(&runner, &api, &calls, &mut failures).await;
    messages(&mut failures).await;
    replacements(&mut failures).await;
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    super::super::retained_contexts::retained(true).await;
}

/// Every guarded member rejects retirement before invoking application operations or callbacks.
async fn guarded(
    runner: &ExtensionRunner<Local>,
    context: &maestro_extensions::ExtensionCommandContext,
    calls: &Calls,
    failures: &mut Vec<String>,
) {
    let stale = Err(STALE.to_owned());
    for (offset, member) in [
        "ui",
        "hasUI",
        "cwd",
        "sessionManager",
        "modelRegistry",
        "model",
        "isIdle",
        "signal",
        "abort",
        "hasPendingMessages",
        "shutdown",
        "getContextUsage",
        "compact",
        "getSystemPrompt",
    ]
    .iter()
    .enumerate()
    {
        super::check(
            failures,
            &format!("B{:03}", 47 + offset),
            &retained(runner, &format!("old {member}")).await,
            &stale,
        );
        super::check(
            failures,
            &format!("C{:03}", 64 + offset),
            &command(runner, context, member).await,
            &stale,
        );
    }
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
        super::check(
            failures,
            &format!("C{:03}", 58 + offset),
            &command(runner, context, &format!("{member} full checked")).await,
            &Ok(json!({"result":{"Err":STALE},"callbacks":0})),
        );
    }
    super::check(failures, "B047-B060/C058-C077", &calls.borrow().len(), &0);
}

/// Creation remains possible; runner UI, shutdown and registration keep their distinct rules.
async fn native_raw(
    runner: &ExtensionRunner<Local>,
    api: &ExtensionAPI<Local>,
    calls: &Calls,
    failures: &mut Vec<String>,
) {
    let stale = Err(STALE.to_owned());
    super::check(
        failures,
        "B061",
        &retained(runner, "save B061").await,
        &Ok(Value::Null),
    );
    super::check(
        failures,
        "B062",
        &retained(runner, "B061 cwd").await,
        &stale,
    );
    super::check(
        failures,
        "C079",
        &command(runner, &runner.create_command_context(), "reload").await,
        &stale,
    );
    runner
        .get_ui_context()
        .notify("raw runner UI".into(), None)
        .unwrap();
    super::check(
        failures,
        "B064",
        &calls.borrow().clone(),
        &vec!["raw runner UI".to_owned()],
    );
    calls.borrow_mut().clear();
    runner.shutdown().unwrap();
    super::check(
        failures,
        "B065",
        &calls.borrow().clone(),
        &vec!["shutdown".to_owned()],
    );
    let registration = api.register_command(
        "late".into(),
        RegisteredCommandOptions {
            description: None,
            get_argument_completions: None,
            handler: Rc::new(|_, _| Box::pin(async { Ok(()) })),
        },
    );
    super::check(
        failures,
        "B066",
        &registration.map_err(|e| e.to_string()),
        &Err(STALE.to_owned()),
    );
}

/// Retiring the runner after operation admission does not revoke the selected future.
async fn admitted(
    runner: &ExtensionRunner<Local>,
    context: &maestro_extensions::ExtensionCommandContext,
    calls: &Calls,
    failures: &mut Vec<String>,
) {
    let (entered, mut admitted) = tokio::sync::mpsc::unbounded_channel();
    runner.bind_command_context(Some(ExtensionCommandContextActions::new(
        maestro_extensions::CommandContextOperations {
            fork: Rc::new(move |_, _| {
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
    let (answer, ()) = futures_util::future::join(command(runner, context, "fork"), async {
        let resume = admitted.recv().await.unwrap();
        runner.invalidate(None);
        super::live::commands(runner, "B", calls);
        resume.send(()).unwrap();
    })
    .await;
    super::check(
        failures,
        "C078",
        &answer,
        &Ok(json!({"value":{"cancelled":true},"callbacks":0})),
    );
}

/// Presence, not truthiness, selects the first invalidation message, including empty text.
async fn messages(failures: &mut Vec<String>) {
    for (first, later, base_id, runtime_id) in [
        ("first", "second", "B067", "B068"),
        ("", "later", "B069", "B070"),
    ] {
        let (runner, _, runtime) = load().await;
        assert_eq!(retained(&runner, "save old").await, Ok(Value::Null));
        let context = runner.create_command_context();
        runner.invalidate(Some(first.into()));
        super::check(
            failures,
            base_id,
            &retained(&runner, "old cwd").await,
            &Err(first.to_owned()),
        );
        super::check(
            failures,
            runtime_id,
            &runtime.assert_active().map_err(|e| e.to_string()),
            &Err(first.to_owned()),
        );
        runner.invalidate(Some(later.into()));
        if first.is_empty() {
            super::check(
                failures,
                "B071",
                &retained(&runner, "old cwd").await,
                &Err(String::new()),
            );
            super::check(
                failures,
                "B072",
                &runtime.assert_active().map_err(|e| e.to_string()),
                &Err(String::new()),
            );
        } else {
            super::check(
                failures,
                "B067",
                &retained(&runner, "old cwd").await,
                &Err(first.to_owned()),
            );
            super::check(
                failures,
                "B068",
                &runtime.assert_active().map_err(|e| e.to_string()),
                &Err(first.to_owned()),
            );
            super::check(
                failures,
                "C080",
                &command(&runner, &context, "waitForIdle").await,
                &Err(first.to_owned()),
            );
        }
    }
}

/// Typed error identity is checked before encoding; only its exact message crosses the wire.
#[derive(Debug)]
struct Marker(Arc<()>);
impl std::fmt::Display for Marker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("send failed")
    }
}
impl std::error::Error for Marker {}

/// Supplies independent message operations to a stale real replacement context.
fn replacement(
    runner: &ExtensionRunner<Local>,
    calls: &Calls,
    marker: Option<Arc<()>>,
) -> ReplacedSessionContext {
    let (custom, user) = (calls.clone(), calls.clone());
    let other = marker.clone();
    ReplacedSessionContext::new(
        runner.create_command_context(),
        Rc::new(move |message, options| {
            assert_eq!(message.custom_type, "retired");
            assert!(options.is_none());
            custom.borrow_mut().push("custom".into());
            let marker = marker.clone();
            Box::pin(async move {
                marker.map_or(Ok(()), |marker| {
                    Err(Box::new(Marker(marker)) as maestro_extensions::EventError)
                })
            })
        }),
        Rc::new(move |content, options| {
            assert_eq!(serde_json::to_value(content).unwrap(), json!("note"));
            assert!(options.is_none());
            user.borrow_mut().push("user".into());
            let marker = other.clone();
            Box::pin(async move {
                marker.map_or(Ok(()), |marker| {
                    Err(Box::new(Marker(marker)) as maestro_extensions::EventError)
                })
            })
        }),
    )
}

/// Authored continuations observe stale reads but still reach their original message owners.
async fn replacements(failures: &mut Vec<String>) {
    let outer = super::live::load("/work").await;
    let fresh = super::live::load("/replacement").await;
    fresh.invalidate(None);
    for fail in [false, true] {
        let calls = Calls::default();
        let marker = fail.then(|| Arc::new(()));
        let replaced = replacement(&fresh, &calls, marker.clone());
        if let Some(marker) = marker {
            original_errors(&replaced, &marker).await;
            calls.borrow_mut().clear();
        }
        outer.bind_command_context(Some(ExtensionCommandContextActions::new(
            maestro_extensions::CommandContextOperations {
                new_session: Rc::new(move |options| {
                    let replaced = replaced.clone();
                    Box::pin(async move {
                        options.unwrap().with_session.unwrap()(replaced).await?;
                        Ok(SessionChangeResult::default())
                    })
                }),
                ..super::super::context_actions::silent_command_operations()
            },
        )));
        let answer = command(
            &outer,
            &outer.create_command_context(),
            "newSession retired",
        )
        .await
        .unwrap_err();
        let answer: Value = serde_json::from_str(&answer).unwrap();
        super::check(failures, "C107", &answer["cwd"], &json!({"Err":STALE}));
        super::check(failures, "C108", &answer["wait"], &json!({"Err":STALE}));
        super::check(
            failures,
            "C109",
            &calls.borrow().clone(),
            &vec!["custom".to_owned(), "user".to_owned()],
        );
        for (id, member) in [("C110", "custom"), ("C111", "user")] {
            let expected = if fail {
                json!({"Err":"send failed"})
            } else {
                json!({"Ok":null})
            };
            super::check(failures, id, &answer[member], &expected);
        }
    }
}

/// Confirms that both native message errors retain the application's actual typed payload.
async fn original_errors(context: &ReplacedSessionContext, marker: &Arc<()>) {
    let message =
        serde_json::from_value(json!({"customType":"retired","content":"note","display":false}))
            .unwrap();
    for (id, result) in [
        ("C110", context.send_message(message, None).await),
        (
            "C111",
            context
                .send_user_message(maestro_models::UserContent::Text("note".into()), None)
                .await,
        ),
    ] {
        let error = result.unwrap_err();
        assert!(
            Arc::ptr_eq(&error.downcast_ref::<Marker>().expect(id).0, marker),
            "{id}"
        );
        assert_eq!(error.to_string(), "send failed", "{id}");
    }
}

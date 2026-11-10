//! Replacement operands and genuine callback failures cross the real authored frame.
#![cfg(test)]
use maestro_extensions::{
    CommandContextOperations, CustomMessageInput, EventBus, ExtensionCommandContextActions,
    ExtensionRunner, Local, ReplacedSessionContext, create_event_bus_for,
    types::SessionChangeResult,
};
use maestro_models::UserContent;
use maestro_session::message_records::{SendMessageOptions, SendUserMessageOptions};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use super::{components::local, retained_components::report};

/// Everything the supplied host observed while it drove one replacement operation.
#[derive(Default)]
struct Host {
    /// Exact operands of each replacement operation, in arrival order.
    calls: RefCell<Vec<Value>>,
    /// The writable session every supplied setup received.
    history: RefCell<Option<Rc<RefCell<maestro_session::SessionManager>>>>,
    /// Supplied continuations this run invoked.
    continuations: Rc<Cell<usize>>,
    /// Whether the supplied operations answer that the replacement was cancelled.
    cancelled: Cell<bool>,
}

/// Loads the component, binds the supplied operations and returns them.
async fn loaded() -> (ExtensionRunner<Local>, Rc<Host>) {
    let bus = create_event_bus_for::<Local>(Rc::new(|_| {}), Rc::new(|_| {}));
    loaded_on(bus.for_extension()).await
}

/// Loads the component over the caller's bus and binds the supplied operations.
async fn loaded_on(events: EventBus<Local>) -> (ExtensionRunner<Local>, Rc<Host>) {
    let (session, history) = super::session::runner_session("/replacement-origin");
    let (_, runner, _) = super::retained_components::loaded_in(events, session).await;
    let host = Rc::new(Host::default());
    *host.history.borrow_mut() = Some(history);
    bind(&runner, &host);
    (runner, host)
}

/// Supplies the replacement operations the authored requests consume.
fn bind(runner: &ExtensionRunner<Local>, host: &Rc<Host>) {
    let (new, fork, switch) = (host.clone(), host.clone(), host.clone());
    runner.bind_command_context(Some(ExtensionCommandContextActions::new(
        CommandContextOperations {
            new_session: new_session(&new),
            fork: fork_from(&fork),
            switch_session: switch_to(&switch),
            ..super::context_actions::silent_command_operations()
        },
    )));
}

/// Drives the supplied setup and continuation of one new-session request.
fn new_session(host: &Rc<Host>) -> maestro_extensions::NewSessionHandler {
    let host = host.clone();
    Rc::new(move |options| {
        let host = host.clone();
        Box::pin(async move {
            let record = options.as_ref().map(|options| {
                json!({
                    "parent": options.parent_session,
                    "setup": options.setup.is_some(),
                    "with": options.with_session.is_some(),
                })
            });
            host.calls
                .borrow_mut()
                .push(json!({ "type": "newSession", "options": record }));
            let Some(options) = options else {
                return Ok(SessionChangeResult { cancelled: false });
            };
            if let Some(setup) = options.setup {
                let history = host.history.borrow().clone().expect("supplied history");
                setup(history).await?;
            }
            if let Some(with) = options.with_session {
                with(replaced("new")).await?;
                host.continuations.set(host.continuations.get() + 1);
            }
            Ok(SessionChangeResult { cancelled: false })
        })
    })
}

/// Records the authored fork operands and drives its optional continuation.
fn fork_from(host: &Rc<Host>) -> maestro_extensions::ForkHandler {
    let host = host.clone();
    Rc::new(move |entry, options| {
        let host = host.clone();
        Box::pin(async move {
            host.calls.borrow_mut().push(json!({
                "type": "fork",
                "entry": entry,
                "options": options.as_ref().map(|options| json!({
                    "position": options.position,
                    "with": options.with_session.is_some(),
                })),
            }));
            let cancelled = host.cancelled.get();
            if let Some(with) = options.and_then(|options| options.with_session)
                && !cancelled
            {
                with(replaced("fork")).await?;
            }
            Ok(SessionChangeResult { cancelled })
        })
    })
}

/// Drives the authored session path and its optional continuation.
fn switch_to(host: &Rc<Host>) -> maestro_extensions::SwitchSessionHandler {
    let host = host.clone();
    Rc::new(move |path, options| {
        let host = host.clone();
        Box::pin(async move {
            host.calls.borrow_mut().push(json!({
                "type": "switch",
                "path": path,
                "options": options.as_ref().map(|options| options.with_session.is_some()),
            }));
            if let Some(with) = options.and_then(|options| options.with_session) {
                with(replaced("switch")).await?;
                host.continuations.set(host.continuations.get() + 1);
            }
            Ok(SessionChangeResult { cancelled: false })
        })
    })
}

/// Builds the replacement context a supplied continuation receives.
fn replaced(label: &str) -> ReplacedSessionContext {
    let session = super::session::runner_session(&format!("/replacement-{label}")).0;
    let runner = ExtensionRunner::<Local>::new(
        Rc::new(std::sync::RwLock::new(Vec::new())),
        maestro_extensions::create_extension_runtime::<Local>(),
        session,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    );
    ReplacedSessionContext::new(
        runner.create_command_context(),
        Rc::new(|_: CustomMessageInput, _: Option<SendMessageOptions>| Box::pin(async { Ok(()) })),
        Rc::new(|_: UserContent, _: Option<SendUserMessageOptions>| Box::pin(async { Ok(()) })),
    )
}

#[test]
fn new_session_forwards_parent_setup_and_continuation() {
    local(async {
        for (combo, trace, parent) in presence_cases() {
            let (runner, host) = loaded().await;
            let answer = report(&runner, &format!("retention new-session {combo}")).await;
            let continuation = "continuation:presence:/replacement-new".to_owned();
            let expected: Vec<String> = match trace {
                "none" => vec![],
                "setup" => vec!["setup".into()],
                "with" => vec![continuation.clone()],
                _ => vec!["setup".into(), continuation.clone()],
            };
            assert_eq!(answer["cancelled"], json!(false), "{combo}");
            assert_eq!(answer["trace"], json!(expected), "{combo}");
            let record = match combo {
                "none" => Value::Null,
                _ => json!({
                    "parent": parent,
                    "setup": setup_ran(trace),
                    "with": continuation_count(trace) == 1,
                }),
            };
            assert_eq!(
                *host.calls.borrow(),
                vec![json!({ "type": "newSession", "options": record })],
                "{combo}"
            );
            assert_eq!(
                host.continuations.get(),
                continuation_count(trace),
                "{combo}"
            );
            assert_eq!(
                setup_name(&host),
                setup_ran(trace).then(|| "literal setup name".to_owned()),
                "{combo}"
            );
        }
    });
}

/// The new-session presence cases with their callback trace and parent operand.
fn presence_cases() -> Vec<(&'static str, &'static str, Value)> {
    vec![
        ("none", "none", Value::Null),
        ("empty", "none", Value::Null),
        (
            "parent",
            "none",
            json!("  /sessions/\u{feff}padded parent.jsonl "),
        ),
        ("setup", "setup", Value::Null),
        ("with", "with", Value::Null),
        ("both", "both", Value::Null),
        (
            "full",
            "full",
            json!("  /sessions/\u{feff}padded parent.jsonl "),
        ),
    ]
}

/// Counts the continuations a presence case runs.
fn continuation_count(trace: &str) -> usize {
    usize::from(matches!(trace, "with" | "both" | "full"))
}

/// Whether a presence case carries the one-shot setup.
fn setup_ran(trace: &str) -> bool {
    matches!(trace, "setup" | "both" | "full")
}

/// Returns the name the guest's setup wrote to the supplied writable session.
fn setup_name(host: &Host) -> Option<String> {
    use maestro_session::session_manager::ReadonlySessionManager;
    match host.history.borrow().as_ref() {
        Some(history) => history
            .borrow()
            .get_session_name()
            .unwrap()
            .map(str::to_owned),
        None => None,
    }
}

#[test]
fn switch_forwards_authored_path_and_continuation() {
    local(async {
        let (runner, host) = loaded().await;
        for (case, path, options) in switch_cases() {
            host.calls.borrow_mut().clear();
            let answer = report(&runner, &format!("retention switch {case}")).await;
            assert_eq!(answer["cancelled"], json!(false), "{case}");
            assert_eq!(answer["path"], json!(path), "{case}");
            assert_eq!(
                answer["trace"],
                json!(match case {
                    "continuation" => vec!["continuation:switch:/replacement-switch"],
                    _ => vec![],
                }),
                "{case}"
            );
            assert_eq!(
                *host.calls.borrow(),
                vec![json!({ "type": "switch", "path": path, "options": options })],
                "{case}"
            );
        }
    });
}

/// The authored paths with their independently optional continuation.
fn switch_cases() -> Vec<(&'static str, &'static str, Value)> {
    vec![
        ("absent", "  /sessions/ padded name.jsonl  ", Value::Null),
        ("empty", "/sessions/\u{feff}bom.jsonl", json!(false)),
        (
            "continuation",
            "/sessions/\u{85}next-line.jsonl",
            json!(true),
        ),
    ]
}

#[test]
fn fork_forwards_position_without_adding_selected_text() {
    local(async {
        let (runner, host) = loaded().await;
        for cancelled in [false, true] {
            host.cancelled.set(cancelled);
            for case in fork_cases() {
                fork_case(&runner, &host, case, cancelled).await;
            }
        }
    });
}

/// Forks once and checks the guest's cancelled-only result and the operands the host saw.
async fn fork_case(
    runner: &ExtensionRunner<Local>,
    host: &Host,
    (case, position, continued): (&str, Value, bool),
    cancelled: bool,
) {
    host.calls.borrow_mut().clear();
    let answer = report(runner, &format!("retention fork {case}")).await;
    let entry = format!("entry-{case}");
    let ran = continued && !cancelled;
    let trace: &[&str] = if ran {
        &["continuation:fork:/replacement-fork"]
    } else {
        &[]
    };
    let label = format!("{case} cancelled={cancelled}");
    assert_eq!(
        answer,
        json!({ "cancelled": cancelled, "trace": trace, "entry": entry }),
        "{label}"
    );
    let options = match case {
        "absent" => Value::Null,
        _ => json!({ "position": position, "with": continued }),
    };
    assert_eq!(
        *host.calls.borrow(),
        vec![json!({ "type": "fork", "entry": entry, "options": options })],
        "{label}"
    );
}

/// The fork option cases with the position and continuation each one sends.
fn fork_cases() -> [(&'static str, Value, bool); 7] {
    [
        ("absent", Value::Null, false),
        ("empty", Value::Null, false),
        ("before", json!("before"), false),
        ("at", json!("at"), false),
        ("continuation", Value::Null, true),
        ("before-continuation", json!("before"), true),
        ("at-continuation", json!("at"), true),
    ]
}

#[test]
fn setup_writer_uses_real_session_persistence() {
    local(async {
        use maestro_session::session_manager::ReadonlySessionManager;
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let directory = scratch.path().to_string_lossy().into_owned();
        let persisted = Rc::new(RefCell::new(
            maestro_session::SessionManager::create("/persisted-cwd", Some(&directory), &directory)
                .unwrap(),
        ));
        let steps = Rc::new(RefCell::new(Vec::new()));
        let bus = create_event_bus_for::<Local>(Rc::new(|_| {}), Rc::new(|_| {}));
        let events = bus.for_extension();
        let (history, seen) = (persisted.clone(), steps.clone());
        drop(events.on(
            "persistence-step",
            Rc::new(move |step, _| {
                let file = history.borrow().get_session_file().map(str::to_owned);
                seen.borrow_mut().push((
                    step.as_str().unwrap().to_owned(),
                    published(file.as_deref()),
                ));
                Ok(None)
            }),
        ));
        let (runner, host) = loaded_on(events).await;
        *host.history.borrow_mut() = Some(persisted);
        let answer = report(&runner, "retention persistence").await;
        assert_eq!(answer, json!({ "cancelled": false, "trace": [] }));
        let header = |extra: &[&str]| -> Vec<String> {
            ["session", "message", "message"]
                .iter()
                .chain(extra)
                .map(|kind| (*kind).to_owned())
                .collect()
        };
        assert_eq!(
            *steps.borrow(),
            [
                ("user".to_owned(), vec![]),
                ("assistant".to_owned(), header(&[])),
                ("later".to_owned(), header(&["session_info"])),
            ]
        );
    });
}

/// The record kinds the session file holds, or none while nothing is published.
fn published(file: Option<&str>) -> Vec<String> {
    let Some(text) = file.and_then(|path| std::fs::read_to_string(path).ok()) else {
        return Vec::new();
    };
    text.lines()
        .map(|line| {
            let record: Value = serde_json::from_str(line).unwrap();
            record["type"].as_str().unwrap().to_owned()
        })
        .collect()
}

#[test]
fn guest_setup_and_continuation_errors_return_to_the_old_frame() {
    local(async {
        let cases = [
            ("none", None, vec!["setup", "continuation:/replacement-new"]),
            ("setup", Some("literal setup failure"), vec!["setup"]),
            (
                "continuation",
                Some("literal continuation failure"),
                vec!["setup", "continuation:/replacement-new"],
            ),
        ];
        for (mode, error, trace) in cases {
            let (runner, host) = loaded().await;
            let answer = report(&runner, &format!("retention errors {mode}")).await;
            match error {
                None => assert_eq!(answer["cancelled"], json!(false), "{mode}"),
                Some(error) => assert_eq!(answer["err"], json!(error), "{mode}"),
            }
            assert_eq!(answer["trace"], json!(trace), "{mode}");
            assert_eq!(
                setup_name(&host),
                (mode != "setup").then(|| "literal setup effect".to_owned()),
                "{mode}: completed effects are preserved"
            );
        }
    });
}

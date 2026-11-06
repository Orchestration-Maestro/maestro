//! Replacement operands and genuine callback failures cross the real authored frame.
#![cfg(test)]
use maestro_extensions::{
    CommandContextOperations, CustomMessageInput, ExtensionCommandContextActions, ExtensionRunner,
    Local, ReplacedSessionContext, create_event_bus_for, types::SessionChangeResult,
};
use maestro_models::UserContent;
use maestro_session::messages::{SendMessageOptions, SendUserMessageOptions};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// Everything the supplied host observed while it drove one replacement operation.
#[derive(Default)]
struct Host {
    /// Exact operands of each replacement operation, in arrival order.
    calls: RefCell<Vec<Value>>,
    /// The writable session every supplied setup received.
    history: RefCell<Option<Rc<RefCell<maestro_session::SessionManager>>>>,
    /// Supplied continuations this run invoked.
    continuations: Rc<Cell<usize>>,
}

/// Runs one authored request through the real command frame of a loaded component.
fn local<T>(work: impl std::future::Future<Output = T>) -> T {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, work)
}

/// Loads the component, binds the supplied operations and returns them.
async fn loaded() -> (ExtensionRunner<Local>, Rc<Host>) {
    let bus = create_event_bus_for::<Local>(Rc::new(|_| {}), Rc::new(|_| {}));
    let (session, history) = super::session::runner_session("/replacement-origin");
    let (_, runner, _) = super::retained_components::loaded_in(bus.for_extension(), session).await;
    let host = Rc::new(Host::default());
    *host.history.borrow_mut() = Some(history);
    bind(&runner, &host);
    (runner, host)
}

/// Runs one authored request that answers with its own JSON either way.
async fn report(runner: &ExtensionRunner<Local>, request: &str) -> Value {
    let handler = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    let answer = handler(request.into(), runner.create_command_context())
        .await
        .expect_err("retained request")
        .to_string();
    serde_json::from_str(&answer).unwrap_or_else(|error| panic!("{error}: {answer}"))
}

/// Supplies the replacement operations the authored requests consume.
fn bind(runner: &ExtensionRunner<Local>, host: &Rc<Host>) {
    let (new, switch) = (host.clone(), host.clone());
    runner.bind_command_context(Some(ExtensionCommandContextActions::new(
        CommandContextOperations {
            new_session: new_session(&new),
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
                    "with": withs(trace) == 1,
                }),
            };
            assert_eq!(
                *host.calls.borrow(),
                vec![json!({ "type": "newSession", "options": record })],
                "{combo}"
            );
            assert_eq!(host.continuations.get(), withs(trace), "{combo}");
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
fn withs(trace: &str) -> usize {
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

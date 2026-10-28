//! Session lookups, stored-history choices and their opened results.

use crate::support::startup;
use maestro_app::{Presence, ReadonlySessionManager, SessionManager};
use maestro_cli::startup::main::sessions::{
    ResolvedSession, SessionOperation, SessionSelection, open_selected_session,
    resolve_session_path, select_session_operation,
};
use maestro_test_directory::Scratch;
use serde_json::Value;

/// The first header record of a stored history.
fn header(path: &str) -> Value {
    let text = std::fs::read_to_string(path).expect("stored history");
    serde_json::from_str(text.lines().next().expect("header line")).expect("header record")
}

/// The authored paths one lookup must answer with itself.
const AUTHORED: [&str; 3] = ["/direct/session.jsonl", "sub/session", "back\\slash"];

#[test]
fn session_locators_choose_first_local_then_global() {
    let scratch = Scratch::new().unwrap();
    let agent_dir = scratch.path().join("agent");
    let cwd = scratch.path().join("project");
    std::fs::create_dir_all(&cwd).unwrap();
    let local_dir = scratch.path().join("local");
    let local = startup::stored_history(&local_dir, "local-one", "project-session", path(&cwd));
    let other = scratch
        .path()
        .join("agent")
        .join("sessions")
        .join("other-project");
    let global = startup::stored_history(&other, "global-one", "global-456", "/other/project");
    let ambiguous = ["amb-1", "amb-2"]
        .iter()
        .map(|id| startup::stored_history(&local_dir, id, id, path(&cwd)))
        .collect::<Vec<_>>();

    startup::runtime().block_on(async {
        let here = (path(&cwd), path(&local_dir), path(&agent_dir));
        assert_eq!(
            lookup("project", here).await.unwrap(),
            ResolvedSession::Local(local.clone())
        );
        assert_eq!(
            lookup("global", here).await.unwrap(),
            ResolvedSession::Global {
                path: global.clone(),
                cwd: "/other/project".to_owned(),
            }
        );
        assert_eq!(
            lookup("nothing", here).await.unwrap(),
            ResolvedSession::NotFound("nothing".to_owned())
        );
        assert_eq!(
            lookup("UPPER.JSONL", here).await.unwrap(),
            ResolvedSession::NotFound("UPPER.JSONL".to_owned()),
            "an uppercase suffix stays a session identifier"
        );
        for authored in AUTHORED {
            assert_eq!(
                lookup(authored, here).await.unwrap(),
                ResolvedSession::Path(authored.to_owned()),
                "{authored}"
            );
        }
        let listed = listed_prefix(&local_dir, &agent_dir, &cwd, "amb-").await;
        assert!(ambiguous.contains(&listed));
        assert_eq!(
            lookup("amb-", here).await.unwrap(),
            ResolvedSession::Local(listed),
            "an ambiguous prefix keeps the listing order"
        );
    });
}

#[test]
fn a_local_match_alone_answers_without_the_global_root() {
    let scratch = Scratch::new().unwrap();
    let cwd = scratch.path().join("project");
    std::fs::create_dir_all(&cwd).unwrap();
    let local_dir = scratch.path().join("local");
    let local = startup::stored_history(&local_dir, "local-one", "project-session", path(&cwd));
    let broken = scratch.path().join("broken").join("agent");
    std::fs::create_dir_all(&broken).unwrap();
    std::fs::write(broken.join("sessions"), "not a directory").unwrap();

    startup::runtime().block_on(async {
        let here = (path(&cwd), path(&local_dir), path(&broken));
        assert_eq!(
            lookup("project", here).await.unwrap(),
            ResolvedSession::Local(local)
        );
        assert_eq!(
            lookup("global", here).await.unwrap(),
            ResolvedSession::NotFound("global".to_owned()),
            "only a missing local match reads the global root"
        );
    });
}

/// Resolve one argument against the supplied directories.
async fn lookup(argument: &str, here: (&str, &str, &str)) -> std::io::Result<ResolvedSession> {
    resolve_session_path(argument, here.0, Some(here.1), here.2).await
}

/// The first listed session of the given prefix, in the owner's order.
async fn listed_prefix(
    local_dir: &std::path::Path,
    agent_dir: &std::path::Path,
    cwd: &std::path::Path,
    prefix: &str,
) -> String {
    let listed = SessionManager::list(path(cwd), Some(path(local_dir)), path(agent_dir), None)
        .await
        .unwrap();
    listed
        .into_iter()
        .find(|session| {
            matches!(&session.id, Presence::Present(Value::String(id)) if id.starts_with(prefix))
        })
        .expect("an ambiguous local match")
        .path
}

/// The authored text of a path.
fn path(value: &std::path::Path) -> &str {
    value.to_str().expect("authored path")
}

#[test]
fn explicit_foreign_fork_copies_in_without_confirming() {
    let scratch = Scratch::new().unwrap();
    let agent_dir = scratch.path().join("agent");
    let cwd = scratch.path().join("project");
    std::fs::create_dir_all(&cwd).unwrap();
    let local_dir = scratch.path().join("local");
    let other = scratch
        .path()
        .join("agent")
        .join("sessions")
        .join("other-project");
    let global = startup::stored_history(&other, "global-one", "global-456", "/other/project");
    let here = (path(&cwd), path(&agent_dir), path(&local_dir));

    assert_eq!(
        select(&["--fork", "global"], here).unwrap(),
        SessionSelection::Selected(SessionOperation::Fork {
            path: global.clone(),
        }),
        "an explicit foreign fork copies the session in without asking"
    );
    let forked = open_selected_session(
        &SessionOperation::Fork {
            path: global.clone(),
        },
        here.0,
        Some(here.2),
        here.1,
    )
    .unwrap();
    let forked_path = forked
        .get_session_file()
        .expect("a forked filename")
        .to_owned();
    assert_ne!(
        forked_path, global,
        "a foreign fork copies into this project"
    );
    assert_eq!(
        header(&forked_path)["parentSession"].as_str(),
        Some(global.as_str())
    );
    assert_eq!(
        select(&["--session", "global"], here).unwrap(),
        SessionSelection::ConfirmFork {
            path: global,
            cwd: "/other/project".to_owned(),
        },
        "only an explicit session confirms a foreign match"
    );
}

#[test]
fn session_operation_precedence_is_explicit() {
    let scratch = Scratch::new().unwrap();
    let agent_dir = scratch.path().join("agent");
    let cwd = scratch.path().join("project");
    std::fs::create_dir_all(&cwd).unwrap();
    let session_dir = scratch.path().join("sessions");
    let first = startup::stored_history(&session_dir, "first", "first-1", path(&cwd));
    let second = startup::stored_history(&session_dir, "second", "second-2", path(&cwd));
    startup::set_modified(&first, 1_000_000);
    startup::set_modified(&second, 2_000_000);
    let here = (path(&cwd), path(&agent_dir), path(&session_dir));

    let cases: Vec<(Vec<&str>, SessionSelection)> = vec![
        (
            vec![
                "--no-session",
                "--session",
                "first",
                "--resume",
                "--continue",
            ],
            SessionSelection::Selected(SessionOperation::InMemory),
        ),
        (
            vec![
                "--fork",
                "first",
                "--session",
                "second",
                "--resume",
                "--continue",
            ],
            SessionSelection::Selected(SessionOperation::Fork {
                path: first.clone(),
            }),
        ),
        (
            vec!["--session", "first", "--resume", "--continue"],
            SessionSelection::Selected(SessionOperation::Open {
                path: first.clone(),
            }),
        ),
        (vec!["--resume", "--continue"], SessionSelection::Resume),
        (
            vec!["--continue"],
            SessionSelection::Selected(SessionOperation::ContinueRecent),
        ),
        (vec![], SessionSelection::Selected(SessionOperation::Create)),
    ];
    for (tokens, expected) in &cases {
        assert_eq!(&select(tokens, here).unwrap(), expected, "{tokens:?}");
    }
    for tokens in [vec!["--fork", "nothing"], vec!["--session", "nothing"]] {
        assert_eq!(
            select(&tokens, here).unwrap(),
            SessionSelection::NotFound("nothing".to_owned()),
            "{tokens:?}"
        );
    }

    opened_histories(here, &first, &second);
}

/// Assert the history each selected operation opens.
fn opened_histories(here: (&str, &str, &str), first: &str, second: &str) {
    let opened = |tokens: &[&str]| {
        open_selected_session(&operation(tokens, here), here.0, Some(here.2), here.1).unwrap()
    };
    let memory = opened(&["--no-session"]);
    assert!(
        !memory.is_persisted(),
        "an unsaved session publishes nothing"
    );

    let continued = opened(&["--continue"]);
    assert_eq!(
        continued.get_session_file(),
        Some(second),
        "continue opens the most recently active history"
    );

    let created = opened(&[]);
    let created_path = created.get_session_file().expect("a selected filename");
    assert_ne!(created_path, first);
    assert_ne!(created_path, second);

    let in_place = opened(&["--session", "first"]);
    let before = std::fs::read_to_string(first).unwrap();
    assert_eq!(in_place.get_session_file(), Some(first));
    assert_eq!(
        std::fs::read_to_string(first).unwrap(),
        before,
        "opening a history rewrites nothing"
    );

    let forked = opened(&["--fork", "first"]);
    let forked_path = forked
        .get_session_file()
        .expect("a forked filename")
        .to_owned();
    assert_ne!(forked_path, first);
    assert_eq!(
        header(&forked_path)["parentSession"].as_str(),
        Some(first),
        "a fork records its source"
    );
    assert_eq!(header(first)["id"].as_str(), Some("first-1"));
}

/// The operation the supplied arguments select.
fn select(tokens: &[&str], here: (&str, &str, &str)) -> std::io::Result<SessionSelection> {
    startup::runtime().block_on(select_session_operation(
        &startup::args(tokens),
        here.0,
        Some(here.2),
        here.1,
    ))
}

/// The selected operation of the supplied arguments.
fn operation(tokens: &[&str], here: (&str, &str, &str)) -> SessionOperation {
    match select(tokens, here).unwrap() {
        SessionSelection::Selected(operation) => operation,
        other => panic!("{tokens:?} selected {other:?}"),
    }
}

#[test]
fn confirmation_keeps_untrimmed_answers_and_eof() {
    use maestro_cli::startup::main::inputs::prompt_confirm;
    for (bytes, expected) in [
        (&b"Y\n"[..], Some(true)),
        (&b"YeS\r\n"[..], Some(true)),
        (&b"yes\r"[..], Some(true)),
        (&b" yes\n"[..], Some(false)),
        (&b"yes \n"[..], Some(false)),
        (&b"no\n"[..], Some(false)),
        (&b"\n"[..], Some(false)),
        (&b"yes"[..], None),
        (&b""[..], None),
    ] {
        let mut input = bytes;
        let mut output = Vec::new();
        assert_eq!(
            startup::runtime()
                .block_on(prompt_confirm("Fork?", &mut input, &mut output))
                .unwrap(),
            expected
        );
        assert_eq!(output, b"Fork? [y/N] ");
    }
    for (refusal, expected) in [(Some(false), 5), (Some(true), 6), (None, 7)] {
        let flushed = std::rc::Rc::new(std::cell::Cell::new(false));
        let mut reader = QuestionInput(std::rc::Rc::clone(&flushed));
        let mut writer = QuestionOutput {
            bytes: Vec::new(),
            flushed,
            refusal,
        };
        let error = startup::runtime()
            .block_on(prompt_confirm("Fork?", &mut reader, &mut writer))
            .unwrap_err();
        assert_eq!(error.raw_os_error(), Some(expected));
    }
}

/// Output that records flushing and can refuse either output phase.
struct QuestionOutput {
    bytes: Vec<u8>,
    flushed: std::rc::Rc<std::cell::Cell<bool>>,
    refusal: Option<bool>,
}
impl std::io::Write for QuestionOutput {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.refusal == Some(false) {
            return Err(std::io::Error::from_raw_os_error(5));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        if self.refusal == Some(true) {
            return Err(std::io::Error::from_raw_os_error(6));
        }
        self.flushed.set(true);
        Ok(())
    }
}

/// Input checking that the question was flushed before reading.
struct QuestionInput(std::rc::Rc<std::cell::Cell<bool>>);
impl tokio::io::AsyncRead for QuestionInput {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        _: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        unreachable!("confirmation uses buffered input")
    }
}
impl tokio::io::AsyncBufRead for QuestionInput {
    fn poll_fill_buf(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<&[u8]>> {
        assert!(self.0.get(), "the question must be visible before reading");
        std::task::Poll::Ready(Err(std::io::Error::from_raw_os_error(7)))
    }
    fn consume(self: std::pin::Pin<&mut Self>, _: usize) {
        unreachable!("refused input has no bytes");
    }
}

#[test]
fn missing_cwd_continue_cancel_and_repeat_settle_once() {
    use crate::support::missing_cwd::{Dialog, isolated};
    use maestro_cli::startup::main::presentation::prompt_for_missing_session_cwd;
    use std::{
        rc::Rc,
        task::{Context, Waker},
    };
    if isolated("missing_cwd_continue_cancel_and_repeat_settle_once") {
        return;
    }
    for selected in [Some("Continue"), Some("Cancel"), Some("other"), None] {
        let dialog = Dialog::new();
        let issue = Dialog::issue();
        let mut operation = Box::pin(prompt_for_missing_session_cwd(
            &issue,
            Rc::clone(&dialog.settings),
            dialog.presentation(),
        ));
        let mut cx = Context::from_waker(Waker::noop());
        assert!(operation.as_mut().poll(&mut cx).is_pending());
        let request = dialog.captured.borrow_mut().take().unwrap();
        if let Some(selected) = selected {
            (request.on_select)(selected);
        } else {
            (request.on_cancel)();
        }
        (request.on_select)("Continue");
        (request.on_cancel)();
        let result = match operation.as_mut().poll(&mut cx) {
            std::task::Poll::Ready(result) => result.unwrap(),
            std::task::Poll::Pending => panic!("settled dialog"),
        };
        assert_eq!(
            result.as_deref(),
            (selected == Some("Continue")).then_some("/fallback")
        );
        assert_eq!(dialog.harness.terminal.events(), ["start", "stop"]);
    }
    let dialog = Dialog::new();
    let issue = Dialog::issue();
    let mut operation = Box::pin(prompt_for_missing_session_cwd(
        &issue,
        Rc::clone(&dialog.settings),
        dialog.presentation(),
    ));
    assert!(
        operation
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    let cancel = dialog.captured.borrow_mut().take().unwrap().on_cancel;
    drop(operation);
    cancel();
    assert_eq!(dialog.harness.terminal.events(), ["start", "stop"]);
}

#[test]
fn missing_cwd_focus_cursor_cleanup_follow_real_writer() {
    use crate::support::missing_cwd::{Dialog, isolated};
    use maestro_cli::startup::main::presentation::prompt_for_missing_session_cwd;
    use maestro_tui::Component;
    use std::{
        rc::Rc,
        task::{Context, Poll, Waker},
    };
    if isolated("missing_cwd_focus_cursor_cleanup_follow_real_writer") {
        return;
    }
    for (hardware, clear) in [(false, false), (false, true), (true, true), (true, false)] {
        let dialog = Dialog::new();
        let issue = Dialog::issue();
        let mut operation = Box::pin(prompt_for_missing_session_cwd(
            &issue,
            Rc::clone(&dialog.settings),
            dialog.policy_presentation((hardware, clear)),
        ));
        let mut cx = Context::from_waker(Waker::noop());
        assert!(operation.as_mut().poll(&mut cx).is_pending());
        dialog.harness.frames();
        let presented_ui = dialog.captured.borrow().as_ref().unwrap().tui.clone();
        assert_eq!(
            presented_ui.render(80).len(),
            1,
            "the supplied writer contains the presented component"
        );
        assert_eq!(
            dialog.frames.borrow().first().map(String::as_str),
            Some("render:true")
        );
        let cancel = dialog.captured.borrow().as_ref().unwrap().on_cancel.clone();
        cancel();
        cancel();
        assert!(matches!(
            operation.as_mut().poll(&mut cx),
            Poll::Ready(Ok(None))
        ));
        assert_eq!(dialog.harness.terminal.events(), ["start", "stop"]);
    }
}

#[test]
fn missing_cwd_failures_release_created_writer() {
    use crate::support::missing_cwd::{Dialog, isolated};
    use maestro_cli::startup::main::presentation::prompt_for_missing_session_cwd;
    use std::{
        io,
        rc::Rc,
        task::{Context, Poll, Waker},
    };
    if isolated("missing_cwd_failures_release_created_writer") {
        return;
    }
    for phase in ["theme", "writer", "component", "start"] {
        let (dialog, issue) = (Dialog::new(), Dialog::issue());
        let mut presentation = dialog.presentation();
        match phase {
            "theme" => {
                presentation.theme = Rc::new(maestro_theme::ThemeState::new(
                    maestro_theme::ThemeDirectories {
                        themes_dir: "/nonexistent-themes".into(),
                        custom_themes_dir: "/nonexistent-custom".into(),
                    },
                ));
            }
            "writer" => presentation.create_ui = Rc::new(|_| Err(io::Error::from_raw_os_error(5))),
            "component" => {
                presentation.presenter = Rc::new(|_| Err(io::Error::from_raw_os_error(5)));
            }
            "start" => dialog
                .harness
                .terminal
                .set_hide_error(Some(io::ErrorKind::PermissionDenied)),
            _ => unreachable!(),
        }
        let mut operation = Box::pin(prompt_for_missing_session_cwd(
            &issue,
            Rc::clone(&dialog.settings),
            presentation,
        ));
        let Poll::Ready(Err(error)) = operation
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        else {
            panic!("the phase must fail");
        };
        if phase == "start" {
            assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        } else if phase != "theme" {
            assert_eq!(error.raw_os_error(), Some(5));
        }
        assert!(
            dialog
                .harness
                .terminal
                .events()
                .iter()
                .filter(|event| **event == "stop")
                .count()
                <= 1
        );
    }
    crate::support::missing_cwd::keybinding_defaults();
}

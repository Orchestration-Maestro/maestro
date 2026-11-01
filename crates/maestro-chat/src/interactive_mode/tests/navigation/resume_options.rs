//! The resume operation's options, missing-directory recovery and failure boundaries.
use super::super::{Setup, World, actions::shows, isolated, isolated_output, start};
use super::{append_user, resume::saved, tree::panel, tree_choices::choose};
use crate::ExtensionSelectorComponent;
use maestro_app::{
    AgentSessionRuntime, ExtensionFactory, Local, Presence, ReadonlySessionManager,
    SessionBeforeSwitchEvent, SessionBeforeSwitchResult, SessionChangeResult, SwitchSessionOptions,
};
use maestro_tui::Component;
use serde_json::json;
use std::{
    cell::{Cell, RefCell},
    error::Error,
    future::Future,
    path::Path,
    rc::{Rc, Weak},
    task::{Context, Waker},
};

/// The session file each continuation saw as current, in call order.
type Continued = Rc<RefCell<Vec<Option<String>>>>;

/// An extension cancelling every switch from its `from`th onwards, counting from zero.
fn cancelling_switches(from: usize) -> ExtensionFactory<Local> {
    let seen = Rc::new(Cell::new(0));
    Rc::new(move |api| {
        let seen = Rc::clone(&seen);
        api.on::<SessionBeforeSwitchEvent, SessionBeforeSwitchResult>(
            "session_before_switch".into(),
            Rc::new(move |_, _| {
                let cancel = seen.get() >= from;
                seen.set(seen.get() + 1);
                let result = SessionBeforeSwitchResult {
                    cancel: Presence::Present(cancel),
                };
                Box::pin(std::future::ready(Ok(Some(result))))
            }),
        )
        .unwrap();
        Box::pin(std::future::ready(Ok(())))
    })
}

/// Options whose continuation, when `retained`, records the runtime's current file.
fn options(
    host: &Rc<AgentSessionRuntime>,
    retained: bool,
    seen: &Continued,
) -> SwitchSessionOptions {
    let host: Weak<AgentSessionRuntime> = Rc::downgrade(host);
    let seen = Rc::clone(seen);
    if !retained {
        return SwitchSessionOptions::default();
    }
    SwitchSessionOptions {
        with_session: Some(Rc::new(move |_| {
            let file = host
                .upgrade()
                .and_then(|host| host.session().session_file());
            seen.borrow_mut().push(file);
            Box::pin(std::future::ready(Ok(())))
        })),
    }
}

/// A replacement factory that always fails.
fn failing_factory() -> maestro_app::CreateAgentSessionRuntimeFactory {
    Rc::new(|_| {
        Box::pin(std::future::ready(Err(
            maestro_app::DiagnosticErrorInfo::new(None, "factory failed"),
        )))
    })
}

/// Resumes `path`, answering the missing-directory prompt with the `answer`th choice.
async fn resumed(
    world: &World,
    path: &str,
    options: SwitchSessionOptions,
    answer: Option<usize>,
) -> Result<SessionChangeResult, Box<dyn Error>> {
    let mut resuming = std::pin::pin!(world.mode.handle_resume_session(path.to_owned(), options));
    if let Some(answer) = answer {
        let mut context = Context::from_waker(Waker::noop());
        assert!(resuming.as_mut().poll(&mut context).is_pending());
        let prompt = panel::<ExtensionSelectorComponent>(world).expect("the directory prompt");
        let shown = maestro_app::strip_ansi(&prompt.render(120).join("\n")).unwrap();
        assert!(shown.contains("Session cwd not found"), "{shown}");
        choose(world, answer);
    }
    resuming.await.map_err(|error| -> Box<dyn Error> { error })
}

/// One resume case: its setup, prompt answer, expected outcome and status.
struct Case {
    /// The replacement factory fails.
    failing: bool,
    /// The extension cancels switches from this one onwards.
    cancel_from: Option<usize>,
    /// The saved session's directory no longer exists.
    missing: bool,
    /// The prompt choice.
    answer: Option<usize>,
    /// The expected outcome: cancelled, or an error when absent.
    cancelled: Option<bool>,
    /// The status shown, if any.
    status: Option<&'static str>,
}

/// Starts the mode `case` runs in and saves the session it resumes, returning its path.
async fn prepared(directory: &Path, case: &Case) -> (World, String) {
    let world = Setup {
        extension: case.cancel_from.map(cancelling_switches),
        create: case.failing.then(failing_factory),
        ..Setup::default()
    }
    .build(directory);
    start(&world).await;
    append_user(&world, "current question");
    let cwd = if case.missing {
        json!(directory.join("gone").to_str().unwrap())
    } else {
        json!(directory.to_str().unwrap())
    };
    let path = saved(&directory.join("saved"), "other", &cwd, "resumed question");
    (world, path)
}

/// Checks that exactly the status `case` expects is shown.
fn shown_status(world: &World, case: &Case, label: &str) {
    for status in [
        "Resumed session",
        "Resume cancelled",
        "Resumed session in current cwd",
    ] {
        let expected = case.status.is_some_and(|shown| shown.starts_with(status));
        assert_eq!(
            shows(&world.mode.chat_container, status),
            expected,
            "{label}: {status}"
        );
    }
}

/// Runs `case`, with or without a retained continuation.
async fn run(directory: &Path, case: &Case, retained: bool) {
    let (world, path) = prepared(directory, case).await;
    let previous = world.host.session().session_file();
    let seen = Continued::default();
    let outcome = resumed(
        &world,
        &path,
        options(&world.host, retained, &seen),
        case.answer,
    )
    .await;
    let label = format!("{:?} {retained}", case.status);
    assert_eq!(
        outcome.as_ref().ok().map(|result| result.cancelled),
        case.cancelled,
        "{label}"
    );
    shown_status(&world, case, &label);
    let switched = case.cancelled == Some(false);
    let current = world.host.session().session_file();
    assert_eq!(
        current,
        if switched {
            Some(path.clone())
        } else {
            previous
        },
        "{label}"
    );
    assert_eq!(
        shows(&world.mode.chat_container, "resumed question"),
        switched
    );
    let expected: Vec<_> = (retained && switched)
        .then(|| Some(path.clone()))
        .into_iter()
        .collect();
    assert_eq!(
        *seen.borrow(),
        expected,
        "{label}: one continuation on the replacement"
    );
    if switched && case.missing {
        let session = world.host.session();
        assert_eq!(
            session.session_manager().borrow().get_cwd(),
            directory.to_str().unwrap()
        );
    }
}

/// Every non-fatal outcome: success, veto, declined, accepted, vetoed retry, failed retry.
fn cases() -> [Case; 6] {
    let plain = Case {
        failing: false,
        cancel_from: None,
        missing: false,
        answer: None,
        cancelled: Some(false),
        status: Some("Resumed session"),
    };
    [
        Case { ..plain },
        Case {
            cancel_from: Some(0),
            cancelled: Some(true),
            status: None,
            ..plain
        },
        Case {
            missing: true,
            answer: Some(1),
            cancelled: Some(true),
            status: Some("Resume cancelled"),
            ..plain
        },
        Case {
            missing: true,
            answer: Some(0),
            status: Some("Resumed session in current cwd"),
            ..plain
        },
        Case {
            cancel_from: Some(1),
            missing: true,
            answer: Some(0),
            cancelled: Some(true),
            status: None,
            ..plain
        },
        Case {
            failing: true,
            missing: true,
            answer: Some(0),
            cancelled: None,
            status: None,
            ..plain
        },
    ]
}

/// Checks in a child process that an unreadable session ends it with status 1.
fn fatal_resume_ends_the_process(test: &str) {
    for (case, retained) in [("fatal", false), ("fatal-retained", true)] {
        let output = isolated_output(test, case, &[], "trap '' XFSZ", async move |directory| {
            let world = Setup::default().build(directory);
            start(&world).await;
            let path = saved(&directory.join("saved"), "broken", &json!(5), "broken");
            let seen = Continued::default();
            println!("\nPROBE resume started");
            drop(resumed(&world, &path, options(&world.host, retained, &seen), None).await);
            println!("\nPROBE resume survived");
        });
        if let Some(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains("PROBE resume started"), "{case}: {stdout}");
            assert!(
                !stdout.contains("PROBE resume survived"),
                "{case}: {stdout}"
            );
            assert_eq!(output.status.code(), Some(1), "{case}: {output:?}");
        }
    }
}

#[test]
fn resume_preserves_switch_options_missing_cwd_and_error_boundaries() {
    let test = "resume_preserves_switch_options_missing_cwd_and_error_boundaries";
    fatal_resume_ends_the_process(test);
    isolated(test, async |directory| {
        for case in &cases() {
            for retained in [false, true] {
                run(directory, case, retained).await;
            }
        }
    });
}

#[test]
fn navigation_rebuild_uses_the_replacement_session_after_callbacks() {
    isolated(
        "navigation_rebuild_uses_the_replacement_session_after_callbacks",
        async |directory| {
            let world = Setup::default().build(directory);
            start(&world).await;
            append_user(&world, "current question");
            world.mode.render_current_session_state().unwrap();
            let cwd = json!(directory.to_str().unwrap());
            let path = saved(&directory.join("saved"), "other", &cwd, "resumed question");
            let host = Rc::downgrade(&world.host);
            let chat = Rc::clone(&world.mode.chat_container);
            let replayed_early = Rc::new(Cell::new(true));
            let observed = Rc::clone(&replayed_early);
            let options = SwitchSessionOptions {
                with_session: Some(Rc::new(move |_| {
                    let host = host.upgrade().unwrap();
                    observed.set(shows(&chat, "resumed question"));
                    append_user_to(&host, "continued question");
                    Box::pin(std::future::ready(Ok(())))
                })),
            };
            let continuation = options.with_session.clone().unwrap();
            let result = resumed(&world, &path, options, None).await.unwrap();
            assert!(!result.cancelled);
            assert!(!replayed_early.get(), "replay waits for the continuation");
            assert!(shows(&world.mode.chat_container, "resumed question"));
            assert!(shows(&world.mode.chat_container, "continued question"));
            assert!(!shows(&world.mode.chat_container, "current question"));
            assert_eq!(
                Rc::strong_count(&continuation),
                1,
                "the operation releases it"
            );
        },
    );
}

/// Appends a user message to the runtime's current session.
fn append_user_to(host: &AgentSessionRuntime, text: &str) {
    host.session()
        .session_manager()
        .borrow_mut()
        .append_message(super::super::transcript::user_record(text))
        .unwrap();
}

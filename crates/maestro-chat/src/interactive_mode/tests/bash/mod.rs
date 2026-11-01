//! Shell input through the retained rows, the session and its extension runner.
mod lifecycle;

use super::super::bash::component_address;
use super::typed::{recalled, submit};
use super::{
    Setup, ShellOperations, World, active, admitted, driving, echoing_shell, holding, isolated,
    scripted, start_shells, started, transcript_positions, until_complete, until_exists,
};
use crate::BashExecutionComponent;
use maestro_app::{ReadonlySessionManager, SettingsManager};
use std::{
    cell::RefCell,
    error::Error,
    future::Future,
    path::Path,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};

/// A started mode whose shell runs `script`.
pub(super) async fn shelling(directory: &Path, script: &str) -> World {
    let script = script.to_owned();
    let world = Setup {
        configure: Box::new(move |settings: &mut SettingsManager| {
            settings.set_shell_path(Some(script));
        }),
        ..Setup::default()
    }
    .build(directory);
    world.mode.init().await.unwrap();
    world.runtime.settle().unwrap();
    world
}

/// Polls `future` once with a waker that observes nothing.
fn poll_once<F: Future + Unpin>(future: &mut F) -> Option<F::Output> {
    match Pin::new(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(output) => Some(output),
        Poll::Pending => None,
    }
}

/// Submits `text` while a waiting input observes it, and returns what reached that input.
fn delivered(world: &World, text: &str) -> Vec<String> {
    let mut waiter: Pin<Box<dyn Future<Output = String>>> = Box::pin(world.mode.get_user_input());
    assert!(poll_once(&mut waiter).is_none());
    submit(&world.mode, text);
    poll_once(&mut waiter).into_iter().collect()
}

/// One submitted text, where it goes, and what the ordinary input received.
struct Case {
    /// The submitted text.
    text: &'static str,
    /// Whether a shell command runs instead of ordinary input.
    shell: bool,
    /// What the waiting input received.
    delivered: &'static [&'static str],
    /// The trimmed text the editor recorded.
    history: &'static str,
}

/// The prefix, trimming and fall-through cases of one submitted text.
const CASES: &[Case] = &[
    Case {
        text: "!",
        shell: false,
        delivered: &["!"],
        history: "!",
    },
    Case {
        text: "!!",
        shell: false,
        delivered: &["!!"],
        history: "!!",
    },
    Case {
        text: "!   ",
        shell: false,
        delivered: &["!"],
        history: "!",
    },
    Case {
        text: "!!!",
        shell: true,
        delivered: &[],
        history: "!!!",
    },
    Case {
        text: " ! ls ",
        shell: true,
        delivered: &[],
        history: "! ls",
    },
    Case {
        text: "!\tls\u{a0}",
        shell: true,
        delivered: &[],
        history: "!\tls",
    },
    Case {
        text: "\u{feff}!ls",
        shell: true,
        delivered: &[],
        history: "!ls",
    },
    Case {
        text: "\u{85}!ls",
        shell: false,
        delivered: &["\u{85}!ls"],
        history: "\u{85}!ls",
    },
    Case {
        text: "\u{200b}!ls",
        shell: false,
        delivered: &["\u{200b}!ls"],
        history: "\u{200b}!ls",
    },
];

#[test]
fn command_submission_keeps_exact_prefix_and_whitespace_rules() {
    isolated(
        "command_submission_keeps_exact_prefix_and_whitespace_rules",
        async |directory| {
            let script = scripted(directory, "shell", "printf 'ran\\n'");
            let world = shelling(directory, &script).await;
            let ((), errors) = (driving(&world, async {
                let mut rows = 0;
                for case in CASES {
                    assert!(active(&world).is_none(), "{:?}", case.text);
                    let delivered = delivered(&world, case.text);
                    assert_eq!(delivered, case.delivered, "{:?}", case.text);
                    assert_eq!(recalled(&world.mode.editor)[0], case.history);
                    rows += usize::from(case.shell);
                    until_complete(&world, rows).await;
                }
                assert_eq!(world.mode.chat_container.children().borrow().len(), rows);
            }))
            .await;
            assert!(errors.is_empty(), "{errors:?}");
        },
    );
}

/// The busy command's exact warning.
const BUSY: &str = "A bash command is already running. Press Esc to cancel it first.";

#[test]
fn busy_shell_restores_selected_input_without_history() {
    isolated(
        "busy_shell_restores_selected_input_without_history",
        async |directory| {
            let started = directory.join("started");
            let release = directory.join("release");
            let script = scripted(
                directory,
                "shell",
                &format!(
                    ": > {}\nwhile [ ! -e {} ]; do :; done\nprintf 'ran\\n'",
                    started.display(),
                    release.display()
                ),
            );
            let world = shelling(directory, &script).await;
            let ((), errors) = (driving(&world, async {
                submit(&world.mode, "!first");
                until_exists(&started).await;
                assert!(world.host.session().is_bash_running());
                for text in ["!second", "!!second"] {
                    submit(&world.mode, text);
                    assert_eq!(world.mode.editor.get_text(), text, "{text}");
                    assert!(!recalled(&world.mode.editor).contains(&text.to_owned()));
                }
                assert_eq!(warnings(&world), 2);
                std::fs::write(&release, "").unwrap();
                // The shell row, and a spacer and a text row for each warning.
                until_complete(&world, 5).await;
            }))
            .await;
            assert!(errors.is_empty(), "{errors:?}");
            assert!(started.exists() && release.exists());
        },
    );
}

/// The number of rendered transcript rows that hold the busy warning.
fn warnings(world: &World) -> usize {
    world
        .mode
        .chat_container
        .walk()
        .flat_map(|child| child.render(80))
        .filter(|row| row.contains(BUSY))
        .count()
}

/// The events the registered `user_bash` handlers saw, in order.
type Seen = Rc<RefCell<Vec<maestro_app::UserBashEvent>>>;

/// What one `user_bash` handler answers, from the event it is given.
type Answer = Rc<dyn Fn(&mut maestro_app::UserBashEvent) -> Option<Interception>>;

/// One intercepted answer: operations, a whole result, or both.
struct Interception {
    /// The operations to run the command through.
    operations: Option<ShellOperations>,
    /// The whole result, which replaces execution.
    result: Option<maestro_app::BashResult>,
}

impl Interception {
    /// The typed result the runner carries.
    fn typed(&self) -> maestro_app::UserBashEventResult<ShellOperations, maestro_app::BashResult> {
        maestro_app::UserBashEventResult {
            operations: self.operations.clone(),
            result: self
                .result
                .as_ref()
                .map_or(maestro_app::Presence::Missing, |result| {
                    maestro_app::Presence::Present(result.clone())
                }),
        }
    }
}

/// An extension answering `user_bash` through `answer` and recording each event in `seen`.
fn intercepting(seen: &Seen, answer: Answer) -> maestro_app::ExtensionFactory<maestro_app::Local> {
    let seen = Rc::clone(seen);
    Rc::new(move |api| {
        let answer = Rc::clone(&answer);
        let seen = Rc::clone(&seen);
        api.on::<maestro_app::UserBashEvent, _>(
            "user_bash".to_owned(),
            Rc::new(move |event: &mut maestro_app::UserBashEvent, _| {
                seen.borrow_mut().push(event.clone());
                let produced = answer(&mut *event).map(|answer| answer.typed());
                Box::pin(std::future::ready(Ok(produced)))
            }),
        )
        .unwrap();
        Box::pin(std::future::ready(Ok(())))
    })
}

/// A started mode with `answer` registered, whose shell runs `script`.
async fn intercepting_world(directory: &Path, script: &str, answer: Answer) -> (World, Seen) {
    let seen: Seen = Rc::default();
    let script = script.to_owned();
    let extension = intercepting(&seen, answer);
    let world = Setup {
        configure: Box::new(move |settings: &mut SettingsManager| {
            settings.set_shell_path(Some(script));
        }),
        extension: Some(extension),
        ..Setup::default()
    }
    .build(directory);
    world.mode.init().await.unwrap();
    world.runtime.settle().unwrap();
    (world, seen)
}

/// The transcript's rendered rows at width 80.
fn transcript(world: &World) -> Vec<String> {
    world
        .mode
        .chat_container
        .walk()
        .flat_map(|child| child.render(80))
        .collect()
}

/// The shell results the session recorded, in order.
fn recorded(world: &World) -> Vec<serde_json::Value> {
    world
        .host
        .session()
        .session_manager()
        .borrow()
        .get_entries()
        .into_iter()
        .filter(|entry| entry["message"]["role"] == "bashExecution")
        .map(|entry| entry["message"].clone())
        .collect()
}

/// A whole shell result with the supplied output and status.
fn result(output: &str, exit_code: Option<f64>) -> maestro_app::BashResult {
    maestro_app::BashResult {
        output: output.to_owned(),
        exit_code,
        cancelled: false,
        truncated: false,
        full_output_path: None,
    }
}

/// Waits for the submitted shell row to appear and then to settle.
async fn until_command_settles(world: &World) {
    for _ in 0..10_000 {
        if active(world).is_some() {
            break;
        }
        tokio::task::yield_now().await;
    }
    for _ in 0..10_000 {
        if active(world).is_none() {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("the shell command settled");
}

/// Submits `text` in a fresh mode answering `user_bash` through `answer`, and settles it.
async fn settled(
    directory: &Path,
    script: &str,
    answer: Answer,
    text: &str,
) -> (World, Seen, Vec<Box<dyn Error>>) {
    let (world, seen) = intercepting_world(directory, script, answer).await;
    let errors = async {
        let ((), errors) = driving(&world, async {
            submit(&world.mode, text);
            until_command_settles(&world).await;
        })
        .await;
        errors
    }
    .await;
    (world, seen, errors)
}

/// An answer that replaces execution with `output` and status `code`.
fn whole(output: &'static str, code: Option<f64>) -> Answer {
    Rc::new(move |_event: &mut maestro_app::UserBashEvent| {
        Some(Interception {
            operations: None,
            result: Some(result(output, code)),
        })
    })
}

#[test]
fn shell_interception_and_operations_share_one_retained_row_and_settle() {
    isolated(
        "shell_interception_and_operations_share_one_retained_row_and_settle",
        async |directory| {
            whole_result_keeps_one_recorded_row(directory).await;
            excluded_result_records_its_exclusion(directory).await;
            supplied_operations_run_the_command(directory).await;
            rejected_execution_fails_the_row(directory).await;
            streaming_placement(directory, ("!echo p", false)).await;
            streaming_placement(directory, ("!!echo p", true)).await;
        },
    );
}

/// A whole intercepted result shows its output and records it once, without a process.
async fn whole_result_keeps_one_recorded_row(directory: &Path) {
    let marker = directory.join("whole-ran");
    let script = scripted(
        directory,
        "shell",
        &format!(": > {}\nprintf 'process output\\n'", marker.display()),
    );
    let (world, seen, errors) = settled(
        directory,
        &script,
        whole("whole output", Some(2.0)),
        "!echo x",
    )
    .await;
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(seen.borrow().len(), 1);
    assert_eq!(seen.borrow()[0].command, "echo x");
    assert!(!seen.borrow()[0].exclude_from_context);
    let rows = transcript(&world);
    assert!(rows.iter().any(|row| row.contains("$ echo x")), "{rows:?}");
    assert!(
        rows.iter().any(|row| row.contains("whole output")),
        "{rows:?}"
    );
    assert!(!marker.exists(), "no process runs");
    let records = recorded(&world);
    assert_eq!(records.len(), 1, "{records:?}");
    assert_eq!(records[0]["output"], "whole output");
    assert_eq!(records[0]["exitCode"], 2.0);
}

/// A double prefix keeps the intercepted output out of the context.
async fn excluded_result_records_its_exclusion(directory: &Path) {
    let script = scripted(directory, "shell", "printf 'unused\\n'");
    let (world, seen, errors) =
        settled(directory, &script, whole("kept out", None), "!!echo y").await;
    assert!(errors.is_empty(), "{errors:?}");
    assert!(seen.borrow()[0].exclude_from_context);
    let records = recorded(&world);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["excludeFromContext"], true);
}

/// Operations an extension supplies run the command in place of the local shell.
async fn supplied_operations_run_the_command(directory: &Path) {
    let marker = directory.join("operations-ran");
    let script = scripted(
        directory,
        "shell",
        &format!(": > {}\nprintf 'process output\\n'", marker.display()),
    );
    let operations =
        maestro_app::local::create_local_bash_operations(String::new(), Some(script.clone()));
    let answer: Answer = Rc::new(move |_event: &mut maestro_app::UserBashEvent| {
        Some(Interception {
            operations: Some(operations.clone()),
            result: None,
        })
    });
    let (world, _, errors) = settled(directory, &script, answer, "!echo z").await;
    assert!(errors.is_empty(), "{errors:?}");
    assert!(marker.exists(), "the supplied operations ran the command");
    let rows = transcript(&world);
    assert!(
        rows.iter().any(|row| row.contains("process output")),
        "{rows:?}"
    );
    assert_eq!(recorded(&world).len(), 1);
}

/// A rejected execution completes the row as failed and records nothing.
async fn rejected_execution_fails_the_row(directory: &Path) {
    let script = scripted(directory, "shell", "printf 'unused\\n'");
    let missing = maestro_app::local::create_local_bash_operations(
        String::new(),
        Some("/nonexistent/shell".to_owned()),
    );
    let answer: Answer = Rc::new(move |_event: &mut maestro_app::UserBashEvent| {
        Some(Interception {
            operations: Some(missing.clone()),
            result: None,
        })
    });
    let (world, _, errors) = settled(directory, &script, answer, "!echo w").await;
    assert!(errors.is_empty(), "{errors:?}");
    let rows = transcript(&world);
    assert!(
        rows.iter().any(|row| row.contains("Bash command failed: ")),
        "{rows:?}"
    );
    assert!(
        recorded(&world).is_empty(),
        "a failed command records no result"
    );
}

/// An answer that leaves the command to the session's selected operations.
fn default_answer() -> Answer {
    Rc::new(|_event: &mut maestro_app::UserBashEvent| {
        Some(Interception {
            operations: None,
            result: None,
        })
    })
}

/// An answer that runs the command through `operations`.
fn through(operations: ShellOperations) -> Answer {
    Rc::new(move |_event: &mut maestro_app::UserBashEvent| {
        Some(Interception {
            operations: Some(operations.clone()),
            result: None,
        })
    })
}

/// An answer that runs the command through a shell that does not exist.
fn failing() -> Answer {
    through(maestro_app::local::create_local_bash_operations(
        String::new(),
        Some("/nonexistent/shell".to_owned()),
    ))
}

/// The transcribed rows of `row` at width 80.
fn row_rows(row: &Rc<BashExecutionComponent>) -> String {
    maestro_tui::Component::render(&**row, 80).join("\n")
}

/// One streaming placement case: the answer the extension gives, the submitted text and
/// the output the row paints, when the command produces one.
struct Placement {
    /// The `user_bash` answer.
    answer: Answer,
    /// The submitted text.
    text: &'static str,
    /// Whether the text keeps the output out of the context.
    excluded: bool,
    /// The output the row paints once, absent when execution fails.
    painted: Option<&'static str>,
}

/// Submits both prefix forms while a response streams: the command keeps one retained row
/// in the pending area, never in the transcript, through every interception branch.
async fn streaming_placement(directory: &Path, (text, excluded): (&'static str, bool)) {
    let script = scripted(directory, "streaming-shell", "printf 'ran\n'");
    let supplied =
        maestro_app::local::create_local_bash_operations(String::new(), Some(script.clone()));
    let cases = [
        Placement {
            answer: default_answer(),
            text,
            excluded,
            painted: Some("ran"),
        },
        Placement {
            answer: through(supplied),
            text,
            excluded,
            painted: Some("ran"),
        },
        Placement {
            answer: whole("whole output", Some(0.0)),
            text,
            excluded,
            painted: Some("whole output"),
        },
        Placement {
            answer: failing(),
            text,
            excluded,
            painted: None,
        },
    ];
    for case in cases {
        placement_case(directory, &script, &case).await;
    }
}

/// Runs one streaming placement case and checks the retained row it left.
async fn placement_case(directory: &Path, script: &str, case: &Placement) {
    let (world, seen) = intercepting_world(directory, script, Rc::clone(&case.answer)).await;
    let errors = async {
        holding(&world, |_| async {
            let ((), errors) = driving(&world, async {
                submit(&world.mode, case.text);
                until_command_settles(&world).await;
            })
            .await;
            errors
        })
        .await
    }
    .await;
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(seen.borrow().len(), 1, "{}", case.text);
    assert_eq!(
        seen.borrow()[0].exclude_from_context,
        case.excluded,
        "{}",
        case.text
    );
    let retained = world.mode.pending_shells.borrow().clone();
    assert_eq!(retained.len(), 1, "one retained shell row: {}", case.text);
    let row = Rc::clone(&retained[0]);
    let shown: Vec<usize> = world
        .mode
        .pending_messages_container
        .children()
        .borrow()
        .iter()
        .map(component_address)
        .collect();
    assert_eq!(shown, vec![component_address(&row)], "the row is pending");
    let transcript = transcript(&world).join("\n");
    assert!(
        !transcript.contains("$ "),
        "the row is not transcribed: {transcript}"
    );
    assert!(
        active(&world).is_none(),
        "completion releases the active claim"
    );
    let rendered = row_rows(&row);
    match case.painted {
        Some(painted) => assert_eq!(
            rendered.matches(painted).count(),
            1,
            "the row paints {painted:?} once for {}: {rendered}",
            case.text
        ),
        // A failed execution completes the row and reports through a status line.
        None => assert!(transcript.contains("Bash command failed: "), "{transcript}"),
    }
    assert_eq!(world.mode.pending_shells.borrow().len(), 1, "{}", case.text);
}

#[test]
fn shell_hooks_preserve_selected_arguments_and_read_current_session_after_await() {
    isolated(
        "shell_hooks_preserve_selected_arguments_and_read_current_session_after_await",
        async |directory| {
            hook_reads_the_selected_arguments(directory).await;
            hook_cannot_rewrite_the_selection(directory).await;
            replaced_session_during_interception_records_there(directory).await;
        },
    );
}

/// The handler sees the trimmed selection and the session's own directory.
async fn hook_reads_the_selected_arguments(directory: &Path) {
    let script = scripted(directory, "shell", "printf 'ran\\n'");
    let cwd = directory.to_str().unwrap().to_owned();
    let answer: Answer = Rc::new(|event: &mut maestro_app::UserBashEvent| {
        assert_eq!(event.command, "echo kept");
        None
    });
    let (world, seen, errors) = settled(directory, &script, answer, "! echo kept ").await;
    assert!(errors.is_empty(), "{errors:?}");
    let events = seen.borrow();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].command, "echo kept");
    assert_eq!(events[0].cwd, cwd, "the hook reads the session's directory");
    assert!(!events[0].exclude_from_context);
    drop(events);
    let rows = transcript(&world);
    assert!(
        rows.iter().any(|row| row.contains("$ echo kept")),
        "{rows:?}"
    );
    let records = recorded(&world);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["command"], "echo kept");
    assert_eq!(records[0]["excludeFromContext"], false);
}

/// A handler that rewrites the event cannot change what runs or what is recorded.
async fn hook_cannot_rewrite_the_selection(directory: &Path) {
    let script = scripted(directory, "shell", "printf 'ran\\n'");
    let rewrite: Answer = Rc::new(|event: &mut maestro_app::UserBashEvent| {
        event.command = "mutated".to_owned();
        event.exclude_from_context = false;
        event.cwd = "/elsewhere".to_owned();
        Some(Interception {
            operations: None,
            result: Some(result("rewritten", Some(0.0))),
        })
    });
    let (world, seen, errors) = settled(directory, &script, rewrite, "!!echo kept").await;
    assert!(errors.is_empty(), "{errors:?}");
    assert!(seen.borrow()[0].exclude_from_context);
    let records = recorded(&world);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["command"], "echo kept");
    assert_eq!(records[0]["excludeFromContext"], true);
    let rows = transcript(&world);
    assert!(
        rows.iter().any(|row| row.contains("$ echo kept")),
        "{rows:?}"
    );
}

/// An extension answering `user_bash` through `answer` once `resume` is notified, reporting
/// `reached` first and recording each event in `seen`.
fn awaiting_interception(
    seen: &Seen,
    reached: Rc<tokio::sync::Notify>,
    resume: Rc<tokio::sync::Notify>,
    answer: Answer,
) -> maestro_app::ExtensionFactory<maestro_app::Local> {
    let seen = Rc::clone(seen);
    Rc::new(move |api| {
        let answer = Rc::clone(&answer);
        let seen = Rc::clone(&seen);
        let reached = Rc::clone(&reached);
        let resume = Rc::clone(&resume);
        api.on::<maestro_app::UserBashEvent, _>(
            "user_bash".to_owned(),
            Rc::new(move |event: &mut maestro_app::UserBashEvent, _| {
                seen.borrow_mut().push(event.clone());
                let produced = answer(&mut *event).map(|answer| answer.typed());
                let reached = Rc::clone(&reached);
                let resume = Rc::clone(&resume);
                Box::pin(async move {
                    reached.notify_one();
                    resume.notified().await;
                    Ok(produced)
                })
            }),
        )
        .unwrap();
        Box::pin(std::future::ready(Ok(())))
    })
}

/// A replacement during a pending hook records the answer into the session current after it.
async fn replaced_session_during_interception_records_there(directory: &Path) {
    let script = scripted(directory, "shell", "printf 'not run\\n'");
    let reached = Rc::new(tokio::sync::Notify::new());
    let resume = Rc::new(tokio::sync::Notify::new());
    let seen: Seen = Rc::default();
    let extension = awaiting_interception(
        &seen,
        Rc::clone(&reached),
        Rc::clone(&resume),
        whole("hook output", Some(0.0)),
    );
    let world = Setup {
        configure: Box::new(move |settings: &mut SettingsManager| {
            settings.set_shell_path(Some(script));
        }),
        extension: Some(extension),
        replacing: true,
        ..Setup::default()
    }
    .build(directory);
    world.mode.init().await.unwrap();
    world.runtime.settle().unwrap();
    let retired = world.host.session();
    let errors = async {
        let ((), errors) = driving(&world, async {
            submit(&world.mode, "!echo kept");
            reached.notified().await;
            let replaced = world
                .host
                .new_session(maestro_app::NewSessionCommandOptions::default())
                .await
                .expect("the replacement session starts");
            assert!(!replaced.cancelled);
            assert!(!Rc::ptr_eq(&retired, &world.host.session()));
            resume.notify_one();
            until_command_settles(&world).await;
        })
        .await;
        errors
    }
    .await;
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(seen.borrow().len(), 1);
    assert_eq!(
        recorded(&world).len(),
        1,
        "the session current after the hook records the answer"
    );
    assert!(
        recorded_in(&retired).is_empty(),
        "the retired session records nothing"
    );
}

/// One whole result the mode shows without running a process.
struct Fields {
    /// The output the extension reports.
    output: &'static str,
    /// The status the extension reports.
    exit_code: Option<f64>,
    /// Whether the extension reports a cancellation.
    cancelled: bool,
    /// Whether the extension reports truncated output.
    truncated: bool,
    /// The artifact path the extension reports.
    path: Option<&'static str>,
}

/// The whole-result cases of the completion projection, in order.
const FIELD_CASES: &[Fields] = &[
    Fields {
        output: "",
        exit_code: Some(0.0),
        cancelled: false,
        truncated: false,
        path: None,
    },
    Fields {
        output: "   ",
        exit_code: Some(0.0),
        cancelled: false,
        truncated: false,
        path: None,
    },
    Fields {
        output: "\u{1b}[31mred\u{1b}[0m\r\nsecond \u{263a}",
        exit_code: None,
        cancelled: false,
        truncated: false,
        path: None,
    },
    Fields {
        output: "custom status",
        exit_code: Some(2.5),
        cancelled: false,
        truncated: false,
        path: None,
    },
    Fields {
        output: "cancelled output",
        exit_code: None,
        cancelled: true,
        truncated: false,
        path: None,
    },
    Fields {
        output: "truncated output",
        exit_code: Some(0.0),
        cancelled: false,
        truncated: true,
        path: Some("/tmp/full-output.txt"),
    },
];

#[test]
fn shell_result_fields_reach_completion_without_projection_loss() {
    isolated(
        "shell_result_fields_reach_completion_without_projection_loss",
        async |directory| {
            let script = scripted(directory, "shell", "printf 'not run\n'");
            for case in FIELD_CASES {
                let (world, _, errors) =
                    settled(directory, &script, fields(case), "!cat file").await;
                assert!(errors.is_empty(), "{errors:?}");
                assert_recorded(&world, case);
                assert_painted(&world, case);
            }
        },
    );
}

/// An answer that reports `case` whole.
fn fields(case: &Fields) -> Answer {
    let output = case.output.to_owned();
    let path = case.path.map(str::to_owned);
    let (exit_code, cancelled, truncated) = (case.exit_code, case.cancelled, case.truncated);
    Rc::new(move |_event: &mut maestro_app::UserBashEvent| {
        Some(Interception {
            operations: None,
            result: Some(maestro_app::BashResult {
                output: output.clone(),
                exit_code,
                cancelled,
                truncated,
                full_output_path: path.clone(),
            }),
        })
    })
}

/// Asserts the session recorded `case` exactly once, field by field.
fn assert_recorded(world: &World, case: &Fields) {
    let records = recorded(world);
    assert_eq!(records.len(), 1, "{records:?}");
    let record = &records[0];
    assert_eq!(record["output"], case.output);
    assert_eq!(record["cancelled"], case.cancelled);
    assert_eq!(record["truncated"], case.truncated);
    assert_eq!(
        record["exitCode"],
        case.exit_code
            .map_or(serde_json::Value::Null, std::convert::Into::into)
    );
}

/// Asserts the transcript painted `case`.
fn assert_painted(world: &World, case: &Fields) {
    let painted = transcript(world).join("\n");
    assert!(painted.contains("$ cat file"), "{painted}");
    match case.exit_code {
        Some(code) if code != 0.0 => assert!(
            painted.contains(&format!("(exit {code})")),
            "the status is painted: {painted}"
        ),
        _ => assert!(!painted.contains("(exit "), "{painted}"),
    }
    assert_eq!(painted.contains("(cancelled)"), case.cancelled, "{painted}");
    if let Some(path) = case.path {
        assert!(painted.contains(path), "{painted}");
    }
}

/// A started mode whose shell runs `script`, replacing the session when one is replaced.
async fn replacing(directory: &Path, script: &str) -> World {
    let script = script.to_owned();
    let world = Setup {
        configure: Box::new(move |settings: &mut SettingsManager| {
            settings.set_shell_path(Some(script));
        }),
        replacing: true,
        ..Setup::default()
    }
    .build(directory);
    world.mode.init().await.unwrap();
    world.runtime.settle().unwrap();
    world
}

/// Starts a session replacement that finishes once the running shell releases it.
fn start_replacement(world: &World) -> tokio::task::JoinHandle<()> {
    let host = Rc::clone(&world.host);
    tokio::task::spawn_local(async move {
        // The replacement starts a fresh session in the same directory.
        drop(
            host.new_session(maestro_app::NewSessionCommandOptions::default())
                .await,
        );
    })
}

/// The shell outputs the session over `session` recorded, in order.
fn recorded_in(session: &Rc<maestro_app::AgentSession>) -> Vec<String> {
    session
        .session_manager()
        .borrow()
        .get_entries()
        .into_iter()
        .filter(|entry| entry["message"]["role"] == "bashExecution")
        .map(|entry| {
            entry["message"]["output"]
                .as_str()
                .unwrap_or_default()
                .to_owned()
        })
        .collect()
}

/// Yields until `session` runs no shell command.
async fn until_idle(session: &maestro_app::AgentSession) {
    while session.is_bash_running() {
        tokio::task::yield_now().await;
    }
}

#[test]
fn shell_completion_from_replaced_session_is_retired() {
    isolated(
        "shell_completion_from_replaced_session_is_retired",
        async |directory| {
            let script = scripted(
                directory,
                "shell",
                ": > \"$2-started\"\nwhile [ ! -e \"$2-release\" ]; do :; done\nprintf '%s output\\n' \"$2\"",
            );
            let world = replacing(directory, &script).await;
            async {
                let ((), errors) = driving(&world, async {
                    submit(&world.mode, "!old");
                    until_exists(&directory.join("old-started")).await;
                    let old_session = world.host.session();
                    let replacement = start_replacement(&world);
                    std::fs::write(directory.join("old-release"), "").unwrap();
                    replacement.await.unwrap();
                    assert!(!Rc::ptr_eq(&old_session, &world.host.session()));
                    until_command_settles(&world).await;
                    let rows = transcript(&world).join("\n");
                    assert!(rows.contains("old output"), "the old row completes: {rows}");
                    assert!(
                        recorded(&world).is_empty(),
                        "the replacement session records nothing"
                    );
                    assert_eq!(
                        recorded_in(&old_session),
                        ["old output\n"],
                        "the old command recorded into its own session"
                    );
                    // A new session typed in the editor disposes the running command's session
                    // and resets: neither its row nor its output survives the release.
                    submit(
                        &world.mode,
                        "!: > second-started; while [ ! -e second-release ]; do :; done; \
                         printf 'second output\\n'",
                    );
                    until_exists(&directory.join("second-started")).await;
                    let second_session = world.host.session();
                    submit(&world.mode, "/new");
                    assert!(admitted(&world.runtime).await.is_empty());
                    let rows = transcript(&world).join("\n");
                    assert!(rows.contains("New session started"), "{rows}");
                    std::fs::write(directory.join("second-release"), "").unwrap();
                    until_idle(&second_session).await;
                    assert!(active(&world).is_none());
                    submit(&world.mode, "later");
                    let rows = transcript(&world).join("\n");
                    assert!(!rows.contains("output"), "nothing old survives: {rows}");
                    assert!(recorded_in(&world.host.session()).is_empty());
                    submit(&world.mode, "!printf 'third output\\n'");
                    until_command_settles(&world).await;
                    let rows = transcript(&world).join("\n");
                    assert!(rows.contains("third output"), "{rows}");
                    assert_eq!(recorded_in(&world.host.session()), ["third output\n"]);
                })
                .await;
                assert!(errors.is_empty(), "{errors:?}");
            }
            .await;
        },
    );
}

#[test]
fn intercepted_shell_recording_failure_releases_active_row() {
    isolated(
        "intercepted_shell_recording_failure_releases_active_row",
        async |directory| {
            use std::os::unix::fs::PermissionsExt;
            let history = directory.join("history");
            std::fs::create_dir(&history).unwrap();
            let script = scripted(directory, "shell", "printf 'not run\n'");
            let seen: Seen = Rc::default();
            let world = Setup {
                configure: Box::new(move |settings: &mut SettingsManager| {
                    settings.set_shell_path(Some(script));
                }),
                extension: Some(intercepting(&seen, whole("kept output", Some(0.0)))),
                session_history: Some(history),
                ..Setup::default()
            }
            .build(directory);
            world.mode.init().await.unwrap();
            world.runtime.settle().unwrap();
            let session = world.host.session();
            let manager = Rc::clone(session.session_manager());
            // A history publishes only once it holds an assistant message, so the file is
            // written when the shell result is recorded.
            manager
                .borrow_mut()
                .append_message(serde_json::json!({
                    "role": "assistant",
                    "content": [],
                    "timestamp": 0
                }))
                .unwrap();
            let file = manager
                .borrow()
                .get_session_file()
                .map(str::to_owned)
                .expect("the session file");
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o400)).unwrap();
            let errors = async {
                let ((), errors) = driving(&world, async {
                    submit(&world.mode, "!echo kept");
                    until_command_settles(&world).await;
                })
                .await;
                errors
            }
            .await;
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
            assert_eq!(errors.len(), 1, "the recording failure is preserved");
            let message = errors[0].to_string();
            assert!(
                message.contains("denied") || message.contains("ermission"),
                "{message}"
            );
            assert!(
                active(&world).is_none(),
                "the completed row's claim is released"
            );
            let rows = transcript(&world).join("\n");
            assert!(rows.contains("kept output"), "{rows}");
            assert!(!rows.contains("Error: "), "no extra notice: {rows}");
        },
    );
}

#[test]
fn native_shell_output_paints_chunks_once() {
    isolated(
        "native_shell_output_paints_chunks_once",
        async |directory| {
            let script = scripted(directory, "shell", "printf 'chunk\\n'");
            let world = shelling(directory, &script).await;
            let errors = async {
                let ((), errors) = driving(&world, async {
                    submit(&world.mode, "!true");
                    until_complete(&world, 1).await;
                })
                .await;
                errors
            }
            .await;
            assert!(errors.is_empty(), "{errors:?}");
            let rows = transcript(&world);
            assert_eq!(
                rows.iter().filter(|row| row.contains("chunk")).count(),
                1,
                "the streamed output is painted once: {rows:?}"
            );
            let records = recorded(&world);
            assert_eq!(records.len(), 1, "{records:?}");
            assert_eq!(records[0]["output"], "chunk\n");
        },
    );
}

#[test]
fn overlapping_shell_rows_release_only_their_own_claim() {
    isolated(
        "overlapping_shell_rows_release_only_their_own_claim",
        async |directory| {
            let script = scripted(
                directory,
                "shell",
                ": > \"$2-started\"\nwhile [ ! -e \"$2-release\" ]; do :; done\nprintf '%s output\\n' \"$2\"",
            );
            let world = shelling(directory, &script).await;
            let errors = async {
                let ((), errors) = driving(&world, async {
                    let older = start_command(&world, "older");
                    until_exists(&directory.join("older-started")).await;
                    let newer = start_command(&world, "newer");
                    until_exists(&directory.join("newer-started")).await;
                    let newer_row = active(&world).expect("the newer row produces output");
                    std::fs::write(directory.join("older-release"), "").unwrap();
                    older.await.unwrap().unwrap();
                    assert!(
                        active(&world).is_some_and(|row| Rc::ptr_eq(&row, &newer_row)),
                        "the older row's release leaves the newer row's claim"
                    );
                    std::fs::write(directory.join("newer-release"), "").unwrap();
                    newer.await.unwrap().unwrap();
                    assert!(active(&world).is_none(), "the last row gives up its claim");
                })
                .await;
                errors
            }
            .await;
            assert!(errors.is_empty(), "{errors:?}");
            let rows = transcript(&world).join("\n");
            assert!(rows.contains("older output"), "{rows}");
            assert!(rows.contains("newer output"), "{rows}");
        },
    );
}

/// Starts the mode's own shell operation for `command`, as a submission does.
fn start_command(
    world: &World,
    command: &'static str,
) -> tokio::task::JoinHandle<Result<(), Box<dyn Error>>> {
    let mode = Rc::clone(&world.mode);
    tokio::task::spawn_local(async move { mode.handle_bash_command(command, false).await })
}

#[test]
fn shell_paints_streamed_chunks_in_order_before_its_row_completes() {
    isolated(
        "shell_paints_streamed_chunks_in_order_before_its_row_completes",
        async |directory| {
            let started = directory.join("drain-started");
            let release = directory.join("drain-release");
            let script = scripted(
                directory,
                "shell",
                &format!(
                    "printf 'first\\n'\n: > {}\nwhile [ ! -e {} ]; do :; done\nprintf 'second\\n'",
                    started.display(),
                    release.display()
                ),
            );
            let world = shelling(directory, &script).await;
            let errors = async {
                let ((), errors) = driving(&world, async {
                    submit(&world.mode, "!drain");
                    until_exists(&started).await;
                    let row = active(&world).expect("the row produces output");
                    until_painted(&row, "first").await;
                    let painted = maestro_tui::Component::render(&*row, 80).join("\n");
                    assert!(
                        !painted.contains("second"),
                        "the command still runs: {painted}"
                    );
                    assert!(
                        painted.contains("Running..."),
                        "the row is not complete: {painted}"
                    );
                    std::fs::write(&release, "").unwrap();
                    until_complete(&world, 1).await;
                })
                .await;
                errors
            }
            .await;
            assert!(errors.is_empty(), "{errors:?}");
            let rows = transcript(&world);
            let first = rows.iter().position(|row| row.contains("first"));
            let second = rows.iter().position(|row| row.contains("second"));
            assert!(first < second, "the chunks keep their order: {rows:?}");
            assert_eq!(
                rows.iter().filter(|row| row.contains("first")).count(),
                1,
                "the first chunk is painted once: {rows:?}"
            );
            assert_eq!(recorded(&world).len(), 1);
            assert_eq!(recorded(&world)[0]["output"], "first\nsecond\n");
        },
    );
}

/// Waits until the row paints `chunk`, yielding so the delivery reaches it.
async fn until_painted(row: &BashExecutionComponent, chunk: &str) {
    for _ in 0..10_000 {
        if painted(row).contains(chunk) {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("the row paints {chunk}");
}

/// The painted rows of one shell row at width 80.
fn painted(row: &BashExecutionComponent) -> String {
    maestro_tui::Component::render(row, 80).join("\n")
}

#[test]
fn cancelled_shell_settles_its_row_and_records_the_cancellation() {
    isolated(
        "cancelled_shell_settles_its_row_and_records_the_cancellation",
        async |directory| {
            let started = directory.join("cancel-started");
            let script = scripted(
                directory,
                "shell",
                &format!(": > {}\nwhile :; do :; done", started.display()),
            );
            let world = shelling(directory, &script).await;
            let errors = async {
                let ((), errors) = driving(&world, async {
                    submit(&world.mode, "!cancel");
                    until_exists(&started).await;
                    world.host.session().abort_bash();
                    until_complete(&world, 1).await;
                })
                .await;
                errors
            }
            .await;
            assert!(errors.is_empty(), "{errors:?}");
            let rows = transcript(&world).join("\n");
            assert!(rows.contains("(cancelled)"), "{rows}");
            let records = recorded(&world);
            assert_eq!(records.len(), 1, "{records:?}");
            assert_eq!(records[0]["cancelled"], true);
        },
    );
}

#[test]
fn pending_shell_flush_uses_linear_identity_membership() {
    isolated(
        "pending_shell_flush_uses_linear_identity_membership",
        async |directory| {
            for count in [3, 64, 128, 256] {
                membership_case(directory, count).await;
            }
            streaming_flush(directory).await;
        },
    );
}

/// Flushes `count` equal-looking pending rows recorded in another order than the container
/// holds them, and checks the transcript order a second flush leaves alone.
async fn membership_case(directory: &Path, count: usize) {
    let world = started(directory).await;
    let (rows, neighbours) = pending_area(&world, count);
    world.mode.flush_pending_bash_components();
    assert_pending_flush(&world, &rows, &neighbours);
    world.mode.flush_pending_bash_components();
    assert_eq!(
        world.mode.chat_container.children().borrow().len(),
        count,
        "a second flush moves nothing"
    );
    assert_eq!(
        world
            .mode
            .pending_messages_container
            .children()
            .borrow()
            .len(),
        neighbours.len()
    );
}

/// Rows two real shell commands produced while a response streamed move into the transcript
/// in submission order under their own identities.
async fn streaming_flush(directory: &Path) {
    let world = echoing_shell(directory).await;
    let rows = async {
        holding(&world, |_| async {
            let (rows, errors) = driving(&world, async {
                start_shells(&world, 2).await;
                world.mode.pending_shells.borrow().clone()
            })
            .await;
            assert!(errors.is_empty(), "{errors:?}");
            rows
        })
        .await
    }
    .await;
    assert_eq!(rows.len(), 2, "both commands kept their row");
    let baseline = world.mode.chat_container.children().borrow().len();
    world.mode.flush_pending_bash_components();
    assert_eq!(
        transcript_positions(&world, &rows),
        vec![baseline, baseline + 1],
        "the rows move once, in submission order"
    );
    assert!(
        world
            .mode
            .pending_messages_container
            .children()
            .borrow()
            .is_empty(),
        "nothing stays pending"
    );
}

/// Fills the pending area with text neighbours around `count` equal-looking shell rows,
/// recording the row handles in another order than the container holds them.
fn pending_area(
    world: &World,
    count: usize,
) -> (
    Vec<Rc<BashExecutionComponent>>,
    Vec<Rc<dyn maestro_tui::Component>>,
) {
    let rows: Vec<_> = std::iter::repeat_n("!echo same", count)
        .map(|command| {
            Rc::new(BashExecutionComponent::new(
                command.to_owned(),
                (*world.mode.ui).clone(),
                world.themes.theme(),
                false,
            ))
        })
        .collect();
    let neighbours: Vec<Rc<dyn maestro_tui::Component>> = ["before", "between"]
        .iter()
        .map(|text| {
            let text = Rc::new(maestro_tui::Text::new((*text).to_owned(), 0, 0, None));
            text as Rc<dyn maestro_tui::Component>
        })
        .collect();
    let mut children: Vec<Rc<dyn maestro_tui::Component>> = vec![Rc::clone(&neighbours[0])];
    children.extend(
        rows.iter()
            .cloned()
            .map(|row| row as Rc<dyn maestro_tui::Component>),
    );
    children.push(Rc::clone(&neighbours[1]));
    world
        .mode
        .pending_messages_container
        .set_children(Rc::new(RefCell::new(children)));
    let mut membership = rows.clone();
    membership.rotate_left(count.saturating_sub(1));
    *world.mode.pending_shells.borrow_mut() = membership;
    (rows, neighbours)
}

/// Asserts the neighbours kept their order and every row moved once under its identity.
fn assert_pending_flush(
    world: &World,
    rows: &[Rc<BashExecutionComponent>],
    neighbours: &[Rc<dyn maestro_tui::Component>],
) {
    let kept = world.mode.pending_messages_container.children();
    let kept: Vec<_> = kept.borrow().iter().map(Rc::clone).collect();
    assert_eq!(kept.len(), neighbours.len());
    for (position, neighbour) in neighbours.iter().enumerate() {
        assert!(
            Rc::ptr_eq(&kept[position], neighbour),
            "neighbour {position} stays"
        );
    }
    let moved = world.mode.chat_container.children();
    let moved: Vec<_> = moved.borrow().iter().map(Rc::clone).collect();
    assert_eq!(moved.len(), rows.len());
    for (position, row) in rows.iter().enumerate() {
        let expected = Rc::clone(row) as Rc<dyn maestro_tui::Component>;
        assert!(
            Rc::ptr_eq(&moved[position], &expected),
            "row {position} moves once under its own identity"
        );
    }
}

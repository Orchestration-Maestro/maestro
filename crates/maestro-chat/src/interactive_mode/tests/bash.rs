//! Shell input through the retained rows, the session and its extension runner.
use super::typed::{recalled, submit};
use super::{Setup, World, isolated};
use crate::BashExecutionComponent;
use crate::components::session_selector::list::tests::session_picker::manual_runtime::ManualRuntime;
use maestro_app::{ReadonlySessionManager, SettingsManager};
use maestro_tui::tui::LocalFuture;
use std::{
    cell::{Cell, RefCell},
    error::Error,
    future::Future,
    path::Path,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};

/// The failures the mode's work reported.
type Failures = Rc<RefCell<Vec<Box<dyn Error>>>>;

/// Runs the work the mode handed its terminal runtime beside `body`, collecting the
/// failures that work reported.
///
/// The mode's own futures are awaited to completion, so the session's shell execution and
/// its chunk delivery interleave with `body`, which stops the driver when it is done.
async fn driving<T>(world: &World, body: impl Future<Output = T>) -> (T, Vec<Box<dyn Error>>) {
    let runtime = world.runtime.clone();
    let finished = Rc::new(Cell::new(false));
    let errors: Failures = Rc::default();
    let driver = {
        let finished = Rc::clone(&finished);
        let errors = Rc::clone(&errors);
        tokio::task::spawn_local(async move { drive(&runtime, &finished, &errors).await })
    };
    let value = body.await;
    finished.set(true);
    driver.await.unwrap();
    let errors = Rc::try_unwrap(errors).unwrap().into_inner();
    (value, errors)
}

/// Drives the mode's futures until the caller reports it is done.
async fn drive(runtime: &ManualRuntime, finished: &Rc<Cell<bool>>, errors: &Failures) {
    loop {
        dispatch(runtime, errors);
        if finished.get() {
            return;
        }
        tokio::task::yield_now().await;
    }
}

/// Hands every future the mode admitted to the task set running beside the caller.
fn dispatch(runtime: &ManualRuntime, errors: &Failures) {
    for future in runtime.take_futures() {
        let errors = Rc::clone(errors);
        tokio::task::spawn_local(async move { await_work(future, &errors).await });
    }
}

/// Awaits one future the mode admitted, recording its failure.
async fn await_work(future: LocalFuture, errors: &Failures) {
    if let Err(error) = future.await {
        errors.borrow_mut().push(error);
    }
}

/// Runs `body` inside a current-thread runtime and task set, where the mode's work and the
/// session's own tasks make progress.
fn on_tasks<T>(body: impl Future<Output = T>) -> T {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, body)
}

/// Waits until `path` exists, yielding so the work that creates it runs.
async fn until_exists(path: &Path) {
    for _ in 0..10_000 {
        if path.exists() {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("{} appeared", path.display());
}

/// Waits until no shell row is producing output and the transcript holds `rows` children.
async fn until_complete(world: &World, rows: usize) {
    for _ in 0..10_000 {
        if active(world).is_none() && world.mode.chat_container.children().borrow().len() == rows {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!(
        "the shell command completed: active={} rows={}",
        active(world).is_some(),
        world.mode.chat_container.children().borrow().len()
    );
}

/// Writes an executable shell script answering every command with `body`.
fn scripted(directory: &Path, name: &str, body: &str) -> String {
    use std::os::unix::fs::PermissionsExt;
    let path = directory.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    path.to_str().unwrap().to_owned()
}

/// A started mode whose shell runs `script`.
fn shelling(directory: &Path, script: &str) -> World {
    let script = script.to_owned();
    let world = Setup {
        configure: Box::new(move |settings: &mut SettingsManager| {
            settings.set_shell_path(Some(script));
        }),
        ..Setup::default()
    }
    .build(directory);
    world.mode.init().unwrap();
    world.runtime.settle().unwrap();
    world
}

/// The shell row the mode still reports as producing output.
fn active(world: &World) -> Option<Rc<BashExecutionComponent>> {
    world.mode.active_shell.borrow().clone()
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
    submit(world, text);
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
        |directory| {
            let script = scripted(directory, "shell", "printf 'ran\\n'");
            let world = shelling(directory, &script);
            let ((), errors) = on_tasks(driving(&world, async {
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
            }));
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
        |directory| {
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
            let world = shelling(directory, &script);
            let ((), errors) = on_tasks(driving(&world, async {
                submit(&world, "!first");
                until_exists(&started).await;
                assert!(world.host.session().is_bash_running());
                for text in ["!second", "!!second"] {
                    submit(&world, text);
                    assert_eq!(world.mode.editor.get_text(), text, "{text}");
                    assert!(!recalled(&world.mode.editor).contains(&text.to_owned()));
                }
                assert_eq!(warnings(&world), 2);
                std::fs::write(&release, "").unwrap();
                // The shell row, and a spacer and a text row for each warning.
                until_complete(&world, 5).await;
            }));
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

/// The operations an extension supplies for a shell command.
type ShellOperations = std::sync::Arc<dyn maestro_app::BashOperations>;

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
fn intercepting_world(directory: &Path, script: &str, answer: Answer) -> (World, Seen) {
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
    world.mode.init().unwrap();
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
fn settled(
    directory: &Path,
    script: &str,
    answer: Answer,
    text: &str,
) -> (World, Seen, Vec<Box<dyn Error>>) {
    let (world, seen) = intercepting_world(directory, script, answer);
    let errors = on_tasks(async {
        let ((), errors) = driving(&world, async {
            submit(&world, text);
            until_command_settles(&world).await;
        })
        .await;
        errors
    });
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
        |directory| {
            whole_result_keeps_one_recorded_row(directory);
            excluded_result_records_its_exclusion(directory);
            supplied_operations_run_the_command(directory);
            rejected_execution_fails_the_row(directory);
        },
    );
}

/// A whole intercepted result shows its output and records it once, without a process.
fn whole_result_keeps_one_recorded_row(directory: &Path) {
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
    );
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
fn excluded_result_records_its_exclusion(directory: &Path) {
    let script = scripted(directory, "shell", "printf 'unused\\n'");
    let (world, seen, errors) = settled(directory, &script, whole("kept out", None), "!!echo y");
    assert!(errors.is_empty(), "{errors:?}");
    assert!(seen.borrow()[0].exclude_from_context);
    let records = recorded(&world);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["excludeFromContext"], true);
}

/// Operations an extension supplies run the command in place of the local shell.
fn supplied_operations_run_the_command(directory: &Path) {
    let marker = directory.join("operations-ran");
    let script = scripted(
        directory,
        "shell",
        &format!(": > {}\nprintf 'process output\\n'", marker.display()),
    );
    let operations = maestro_app::create_local_bash_operations(String::new(), Some(script.clone()));
    let answer: Answer = Rc::new(move |_event: &mut maestro_app::UserBashEvent| {
        Some(Interception {
            operations: Some(operations.clone()),
            result: None,
        })
    });
    let (world, _, errors) = settled(directory, &script, answer, "!echo z");
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
fn rejected_execution_fails_the_row(directory: &Path) {
    let script = scripted(directory, "shell", "printf 'unused\\n'");
    let missing = maestro_app::create_local_bash_operations(
        String::new(),
        Some("/nonexistent/shell".to_owned()),
    );
    let answer: Answer = Rc::new(move |_event: &mut maestro_app::UserBashEvent| {
        Some(Interception {
            operations: Some(missing.clone()),
            result: None,
        })
    });
    let (world, _, errors) = settled(directory, &script, answer, "!echo w");
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

#[test]
fn shell_hooks_preserve_selected_arguments_and_read_current_session_after_await() {
    isolated(
        "shell_hooks_preserve_selected_arguments_and_read_current_session_after_await",
        |directory| {
            hook_reads_the_selected_arguments(directory);
            hook_cannot_rewrite_the_selection(directory);
        },
    );
}

/// The handler sees the trimmed selection and the session's own directory.
fn hook_reads_the_selected_arguments(directory: &Path) {
    let script = scripted(directory, "shell", "printf 'ran\\n'");
    let cwd = directory.to_str().unwrap().to_owned();
    let answer: Answer = Rc::new(|event: &mut maestro_app::UserBashEvent| {
        assert_eq!(event.command, "echo kept");
        None
    });
    let (world, seen, errors) = settled(directory, &script, answer, "! echo kept ");
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
fn hook_cannot_rewrite_the_selection(directory: &Path) {
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
    let (world, seen, errors) = settled(directory, &script, rewrite, "!!echo kept");
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
        |directory| {
            let script = scripted(directory, "shell", "printf 'not run\n'");
            for case in FIELD_CASES {
                let (world, _, errors) = settled(directory, &script, fields(case), "!cat file");
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
fn replacing(directory: &Path, script: &str) -> World {
    let script = script.to_owned();
    let world = Setup {
        configure: Box::new(move |settings: &mut SettingsManager| {
            settings.set_shell_path(Some(script));
        }),
        replacing: true,
        ..Setup::default()
    }
    .build(directory);
    world.mode.init().unwrap();
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

#[test]
fn shell_completion_from_replaced_session_is_retired() {
    isolated(
        "shell_completion_from_replaced_session_is_retired",
        |directory| {
            let script = scripted(
                directory,
                "shell",
                ": > \"$2-started\"\nwhile [ ! -e \"$2-release\" ]; do :; done\nprintf '%s output\\n' \"$2\"",
            );
            let world = replacing(directory, &script);
            on_tasks(async {
                let ((), errors) = driving(&world, async {
                    submit(&world, "!old");
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
                })
                .await;
                assert!(errors.is_empty(), "{errors:?}");
            });
        },
    );
}

#[test]
fn intercepted_shell_recording_failure_releases_active_row() {
    isolated(
        "intercepted_shell_recording_failure_releases_active_row",
        |directory| {
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
            world.mode.init().unwrap();
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
            let errors = on_tasks(async {
                let ((), errors) = driving(&world, async {
                    submit(&world, "!echo kept");
                    until_command_settles(&world).await;
                })
                .await;
                errors
            });
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

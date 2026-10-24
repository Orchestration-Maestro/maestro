//! Shell input through the retained rows, the session and its extension runner.
use super::typed::{recalled, submit};
use super::{Setup, World, isolated};
use crate::BashExecutionComponent;
use crate::components::session_selector::list::tests::session_picker::manual_runtime::ManualRuntime;
use maestro_app::SettingsManager;
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

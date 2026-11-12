//! The published print-mode surface over the application's own runtime.
#![cfg(not(target_arch = "wasm32"))]
#[cfg(unix)]
use crate::startup_runtime::SHUTDOWN_UNDER_WAY;
use crate::startup_runtime::{Fixture, run};
use crate::support::startup;
use maestro_app::AgentSessionRuntime;
use maestro_catalog::ModelResolverOutput;
use maestro_cli::startup::entrypoint::initialization::create_runtime;
use maestro_cli::{PrintMode, PrintModeOptions, run_print_mode};
use maestro_models::ImageContent;
use serde_json::json;
use std::cell::RefCell;
use std::rc::Rc;

/// The shutdown report the signalled child writes the events it delivered to.
#[cfg(unix)]
const SHUTDOWN_REPORT: &str = "MAESTRO_SHUTDOWN_REPORT";

/// The line the child writes once its request waits on the scripted stream.
#[cfg(unix)]
const WAITING_FOR_THE_REQUEST: &str = "print run waiting in its model stream";

/// The shutdown case a signalled child runs.
#[cfg(unix)]
#[derive(Clone, Copy)]
enum SignallingCase {
    /// One termination signal, disposing while the request still waits.
    Termination,
    /// A hangup arriving while the termination's disposal still waits.
    Hangup,
    /// One termination signal whose disposal the host rejects.
    Rejected,
}

#[cfg(unix)]
impl SignallingCase {
    /// Every case, in the order the parent may select them.
    const ALL: [Self; 3] = [Self::Termination, Self::Hangup, Self::Rejected];

    /// The case `spelled` names, as the parent hands it to its child.
    fn parse(spelled: Option<&str>) -> Self {
        Self::ALL
            .into_iter()
            .find(|case| Some(case.spelled()) == spelled)
            .unwrap_or_else(|| panic!("the signalled run has no case: {spelled:?}"))
    }

    /// The spelling the parent hands its child for this case.
    fn spelled(self) -> &'static str {
        match self {
            Self::Termination => "termination",
            Self::Hangup => "hangup",
            Self::Rejected => "rejected",
        }
    }
}

#[test]
fn library_exports_accept_existing_runtime() {
    // The exports compile against the application's own runtime and typed inputs.
    let options = PrintModeOptions {
        mode: PrintMode::Text,
        messages: vec!["prompt".to_owned()],
        initial_message: None,
        initial_images: Some(vec![ImageContent {
            mime_type: "image/png".to_owned(),
            data: "first".to_owned(),
        }]),
    };
    assert_eq!(options.mode, PrintMode::Text);
    assert_eq!(options.initial_images.expect("images").len(), 1);
    let entry: fn(Rc<AgentSessionRuntime>, PrintModeOptions) -> _ = run_print_mode;
    let _ = entry;
}

#[test]
fn a_print_run_shuts_the_session_down_with_quit() {
    run(
        "a_print_run_shuts_the_session_down_with_quit",
        |local| async move {
            let shutdowns = Rc::new(RefCell::new(Vec::new()));
            let fixture = Fixture::with_shutdown_watch(
                Rc::clone(&local),
                startup::settings(&[]),
                Rc::clone(&shutdowns),
            )
            .await;
            let created = create_runtime(
                &startup::args(&["--model", "startup/first"]),
                fixture.creation(),
                &mut ModelResolverOutput {
                    stdout: &mut Vec::new(),
                    stderr: &mut Vec::new(),
                    color: false,
                },
            )
            .await
            .expect("the runtime");
            let runtime = Rc::new(AgentSessionRuntime::new(
                created,
                Rc::new(|_| panic!("a print run never replaces its session")),
            ));

            let status = run_print_mode(
                Rc::clone(&runtime),
                PrintModeOptions {
                    mode: PrintMode::Text,
                    messages: Vec::new(),
                    initial_message: None,
                    initial_images: None,
                },
            )
            .await;

            assert_eq!(status, Ok(0));
            assert_eq!(
                *shutdowns.borrow(),
                [json!({"reason": "quit"})],
                "the run's shutdown reaches the extension an actual session binds"
            );
        },
    );
}

/// A real termination signal disposes the run while its request is still pending, and the
/// extension an actual session binds hears `quit` before the process ends.
#[cfg(unix)]
#[test]
fn a_termination_signal_disposes_the_run_and_reports_quit() {
    let name = "a_termination_signal_disposes_the_run_and_reports_quit";
    if maestro_test_process::selection().is_child() {
        signalling_child();
        return;
    }
    let root = maestro_test_directory::Scratch::new().unwrap();
    let report = root.path().join("shutdown.json");
    let (mut child, mut diagnostics) =
        signalling_case(name, SignallingCase::Termination, root.path(), &report);

    await_announcement(&mut child, &mut diagnostics, WAITING_FOR_THE_REQUEST);
    signal(&child, rustix::process::Signal::TERM);
    let status = child.wait().unwrap();

    assert_eq!(status.code(), Some(143), "the run's termination status");
    assert_eq!(
        shutdown_report(&report),
        [json!({"reason": "quit"})],
        "the run disposed and the bound extension heard quit exactly once"
    );
}

/// A hangup arriving while the first signal's disposal still waits ends the run with the
/// hangup status, without disposing the session a second time.
#[cfg(unix)]
#[test]
fn a_hangup_after_a_termination_ends_the_run_and_reports_quit_once() {
    let name = "a_hangup_after_a_termination_ends_the_run_and_reports_quit_once";
    if maestro_test_process::selection().is_child() {
        signalling_child();
        return;
    }
    let root = maestro_test_directory::Scratch::new().unwrap();
    let report = root.path().join("shutdown.json");
    let (mut child, mut diagnostics) =
        signalling_case(name, SignallingCase::Hangup, root.path(), &report);

    await_announcement(&mut child, &mut diagnostics, WAITING_FOR_THE_REQUEST);
    signal(&child, rustix::process::Signal::TERM);
    await_announcement(&mut child, &mut diagnostics, SHUTDOWN_UNDER_WAY);
    signal(&child, rustix::process::Signal::HUP);
    let status = child.wait().unwrap();

    assert_eq!(status.code(), Some(129), "the run's hangup status");
    assert_eq!(
        shutdown_report(&report),
        [json!({"reason": "quit"})],
        "the run reported quit once and the hangup disposed nothing further"
    );
}

/// A disposal the host rejects still ends the run with the termination status, after the
/// extension the session binds heard `quit`.
#[cfg(unix)]
#[test]
fn a_rejected_disposal_ends_the_run_and_reports_quit() {
    let name = "a_rejected_disposal_ends_the_run_and_reports_quit";
    if maestro_test_process::selection().is_child() {
        signalling_child();
        return;
    }
    let root = maestro_test_directory::Scratch::new().unwrap();
    let report = root.path().join("shutdown.json");
    let (mut child, mut diagnostics) =
        signalling_case(name, SignallingCase::Rejected, root.path(), &report);

    await_announcement(&mut child, &mut diagnostics, WAITING_FOR_THE_REQUEST);
    signal(&child, rustix::process::Signal::TERM);
    let status = child.wait().unwrap();

    assert_eq!(
        status.code(),
        Some(143),
        "a rejected disposal still ends the run with the termination status"
    );
    assert_eq!(
        shutdown_report(&report),
        [json!({"reason": "quit"})],
        "the bound extension heard quit before the host rejected the disposal"
    );
}

/// The signalled child running `case`, with its diagnostics.
#[cfg(unix)]
fn signalling_case(
    name: &str,
    case: SignallingCase,
    root: &std::path::Path,
    report: &std::path::Path,
) -> (
    std::process::Child,
    std::io::BufReader<std::process::ChildStderr>,
) {
    let mut child = crate::support::missing_cwd::command(name, case.spelled(), root)
        .env(SHUTDOWN_REPORT, report)
        .quiet()
        .nocapture()
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .command()
        .unwrap()
        .spawn()
        .unwrap();
    let diagnostics = std::io::BufReader::new(child.stderr.take().unwrap());
    (child, diagnostics)
}

/// Waits until `child` announces `expected`, ending it when it announces anything else.
#[cfg(unix)]
fn await_announcement(
    child: &mut std::process::Child,
    diagnostics: &mut impl std::io::BufRead,
    expected: &str,
) {
    let mut line = String::new();
    let announced = diagnostics.read_line(&mut line).unwrap();
    if announced == 0 || line.trim_end() != expected {
        let _ = child.kill();
        let _ = child.wait();
        panic!("the child announced {line:?} instead of {expected:?}");
    }
}

/// Delivers `signal` to `child`.
#[cfg(unix)]
fn signal(child: &std::process::Child, signal: rustix::process::Signal) {
    let pid = rustix::process::Pid::from_raw(i32::try_from(child.id()).unwrap()).unwrap();
    rustix::process::kill_process(pid, signal).unwrap();
}

/// The shutdown events the run reported at `report`.
#[cfg(unix)]
fn shutdown_report(report: &std::path::Path) -> Vec<serde_json::Value> {
    serde_json::from_str(&std::fs::read_to_string(report).expect("the run reported no shutdown"))
        .unwrap()
}

/// Runs the print run the parent signals, reporting what its disposal delivered.
#[cfg(unix)]
fn signalling_child() {
    let report = std::env::var_os(SHUTDOWN_REPORT).expect("a shutdown report path");
    let local = Rc::new(tokio::task::LocalSet::new());
    let running = {
        let local = Rc::clone(&local);
        async move {
            let case = SignallingCase::parse(maestro_test_process::selection().case());
            let fixture =
                signalling_fixture(Rc::clone(&local), std::path::Path::new(&report), case).await;
            let created = create_runtime(
                &startup::args(&["--model", "startup/first"]),
                fixture.creation(),
                &mut ModelResolverOutput {
                    stdout: &mut Vec::new(),
                    stderr: &mut Vec::new(),
                    color: false,
                },
            )
            .await
            .expect("the runtime");
            let runtime = Rc::new(AgentSessionRuntime::new(
                created,
                Rc::new(|_| panic!("a print run never replaces its session")),
            ));
            if let SignallingCase::Rejected = case {
                reject_disposal(&runtime);
            }
            *runtime.session().agent().stream_fn.write().unwrap() = Some(Rc::new(|_, _, _, _| {
                eprintln!("{WAITING_FOR_THE_REQUEST}");
                Box::pin(std::future::pending())
            }));
            let status = run_print_mode(
                Rc::clone(&runtime),
                PrintModeOptions {
                    mode: PrintMode::Text,
                    messages: vec!["pending".to_owned()],
                    initial_message: None,
                    initial_images: None,
                },
            )
            .await;
            panic!("the run ended without its signal: {status:?}");
        }
    };
    startup::runtime().block_on(local.run_until(running));
}

/// Rejects the disposal the run's signal path starts, as a host's teardown can.
#[cfg(unix)]
fn reject_disposal(runtime: &AgentSessionRuntime) {
    runtime.set_before_session_invalidate(Some(Rc::new(|| {
        Err("the host could not invalidate the session".into())
    })));
}

/// The fixture the signalled run starts from, chosen by its case.
#[cfg(unix)]
async fn signalling_fixture(
    local: Rc<tokio::task::LocalSet>,
    report: &std::path::Path,
    case: SignallingCase,
) -> Fixture {
    let settings = startup::settings(&[]);
    match case {
        SignallingCase::Hangup => {
            Fixture::with_pending_shutdown_report(local, settings, report).await
        }
        SignallingCase::Termination | SignallingCase::Rejected => {
            Fixture::with_shutdown_report(local, settings, report).await
        }
    }
}

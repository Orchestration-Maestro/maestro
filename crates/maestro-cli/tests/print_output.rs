//! The published print-mode surface over the application's own runtime.
#![cfg(not(target_arch = "wasm32"))]
use crate::startup_runtime::{Fixture, run};
use crate::support::startup;
use maestro_app::{AgentSessionRuntime, ImageContent, ModelResolverOutput};
use maestro_cli::startup::main::runtime::create_runtime;
use maestro_cli::{PrintMode, PrintModeOptions, run_print_mode};
use serde_json::json;
use std::cell::RefCell;
use std::rc::Rc;

/// The shutdown report the signalled child writes the events it delivered to.
#[cfg(unix)]
const SHUTDOWN_REPORT: &str = "MAESTRO_SHUTDOWN_REPORT";

/// The line the child writes once its request waits on the scripted stream.
#[cfg(unix)]
const WAITING_FOR_THE_REQUEST: &str = "print run waiting in its model stream";

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
    use std::io::BufRead;
    let name = "a_termination_signal_disposes_the_run_and_reports_quit";
    if maestro_test_process::selection().is_child() {
        signalling_child();
        return;
    }
    let root = maestro_test_directory::Scratch::new().unwrap();
    let report = root.path().join("shutdown.json");

    let mut child = crate::support::missing_cwd::command(name, "termination", root.path())
        .env(SHUTDOWN_REPORT, &report)
        .quiet()
        .nocapture()
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .command()
        .unwrap()
        .spawn()
        .unwrap();
    let mut diagnostics = std::io::BufReader::new(child.stderr.take().unwrap());
    let mut waiting = String::new();
    let announced = diagnostics.read_line(&mut waiting).unwrap();
    if announced == 0 || waiting.trim_end() != WAITING_FOR_THE_REQUEST {
        let _ = child.kill();
        let _ = child.wait();
        panic!("the child reached its model stream before the signal: {waiting}");
    }

    let pid = rustix::process::Pid::from_raw(i32::try_from(child.id()).unwrap()).unwrap();
    rustix::process::kill_process(pid, rustix::process::Signal::TERM).unwrap();
    let status = child.wait().unwrap();

    assert_eq!(status.code(), Some(143), "the run's termination status");
    let reported: Vec<serde_json::Value> = serde_json::from_str(
        &std::fs::read_to_string(&report).expect("the run reported no shutdown"),
    )
    .unwrap();
    assert_eq!(
        reported,
        [json!({"reason": "quit"})],
        "the run disposed and the bound extension heard quit exactly once"
    );
}

/// Runs the print run the parent signals, reporting what its disposal delivered.
#[cfg(unix)]
fn signalling_child() {
    use std::path::Path;
    let report = std::env::var_os(SHUTDOWN_REPORT).expect("a shutdown report path");
    let local = Rc::new(tokio::task::LocalSet::new());
    let running = {
        let local = Rc::clone(&local);
        async move {
            let fixture = Fixture::with_shutdown_report(
                Rc::clone(&local),
                startup::settings(&[]),
                Path::new(&report),
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

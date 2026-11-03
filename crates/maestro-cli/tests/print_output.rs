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

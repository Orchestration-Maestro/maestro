//! The published print-mode surface over the application's own runtime.
#![cfg(not(target_arch = "wasm32"))]
use maestro_app::{AgentSessionRuntime, ImageContent};
use maestro_cli::{PrintMode, PrintModeOptions, run_print_mode};
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

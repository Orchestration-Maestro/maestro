//! Command-line input preparation, account commands and single-shot output.
//!
//! The crate turns authored arguments into retained input, prepares file and image
//! attachments, assembles the first message, renders model tables and runs the
//! configuration and session presentation lifecycles through supplied presenters. It
//! also presents the standalone account command's help, provider lists and login, and
//! presents one session run as its final text or as event records.

pub mod input;
pub mod print;

#[cfg(not(target_arch = "wasm32"))]
pub mod model_login;
pub mod packages;

pub use print::print_mode::{
    FinalMessage, NativeSignals, PrintEffects, PrintFuture, PrintMode, PrintModeOptions,
    PrintRebind, PrintRuntime, PrintSession, SignalPort, run_print_mode, run_print_mode_with,
};

//! Command-line input preparation and single-shot output.
//!
//! The crate turns authored arguments into retained input, prepares file and image
//! attachments, assembles the first message, renders model tables, runs the configuration and
//! session presentation lifecycles through supplied presenters, and presents one session run
//! as its final text or as event records.

pub mod input;
pub mod print;

pub use print::print_mode::{
    FinalMessage, NativeSignals, PrintEffects, PrintFuture, PrintMode, PrintModeOptions,
    PrintRebind, PrintRuntime, PrintSession, SignalPort, run_print_mode, run_print_mode_with,
};

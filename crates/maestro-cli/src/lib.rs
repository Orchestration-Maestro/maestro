//! Command-line input preparation, account commands and single-shot output.
//!
//! The crate turns authored arguments into retained input, prepares file and image
//! attachments, assembles the first message, renders model tables, selects what a run
//! does before its frontend starts, and runs the configuration and session presentation
//! lifecycles through supplied presenters. It also presents the standalone account
//! command's help, provider lists and login, and presents one session run as its final
//! text or as event records.

pub mod input;

#[cfg(not(target_arch = "wasm32"))]
pub mod model_login;
pub mod packages;
mod print;

pub use print::print_mode::{PrintMode, PrintModeOptions, run_print_mode};

#[cfg(not(target_arch = "wasm32"))]
pub mod startup;
#[cfg(not(target_arch = "wasm32"))]
pub use startup::main::{MainOptions, main};

#[cfg(not(target_arch = "wasm32"))]
pub mod workspace_commands;

#[cfg(not(target_arch = "wasm32"))]
pub mod completion;

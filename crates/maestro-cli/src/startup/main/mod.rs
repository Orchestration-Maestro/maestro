//! Argument, session and model selection performed before a frontend starts.

pub mod bootstrap;
pub mod inputs;
pub mod options;
pub mod presentation;
pub mod runtime;
pub mod sessions;

pub mod dispatch;

mod entry;
mod factory;
mod selected_sessions;
mod writers;

pub use entry::{MainExport, MainHost, MainOptions, main};

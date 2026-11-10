//! Argument, session and model selection performed before a frontend starts.

pub mod bootstrap;
pub mod configuration;
pub mod feedback;
pub mod initialization;
pub mod inputs;
pub mod sessions;

pub mod routing;

mod entry;
mod factory;
mod selected_sessions;
mod writers;

pub use entry::{MainExport, MainHost, MainOptions, main};

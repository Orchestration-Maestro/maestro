//! Server mode: the workspace daemon's loop with its API socket, ready message and log.
mod run;
mod server;

pub use run::run_server;

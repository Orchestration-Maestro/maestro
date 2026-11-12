//! Test support: the application's scripted session harness with the runtime and export
//! fixtures built on it, a protocol connection over memory, and a child process that serves
//! one runtime over its standard streams.
#[path = "../../../maestro-app/tests/support/export_fixtures.rs"]
pub mod export_fixtures;
#[path = "../../../maestro-app/tests/support/no_commands.rs"]
pub mod no_commands;
pub mod rpc_connection;
#[path = "../../../maestro-app/tests/support/runtime_fixtures.rs"]
#[allow(
    dead_code,
    reason = "The protocol tests use only part of the application's runtime fixtures."
)]
pub mod runtime_fixtures;
#[cfg(unix)]
mod served;
#[path = "../../../maestro-app/tests/support/session_harness.rs"]
#[allow(
    dead_code,
    reason = "The protocol tests use only part of the application's session harness."
)]
pub mod session_harness;
#[path = "../../../maestro-app/tests/support/runtime.rs"]
#[allow(
    dead_code,
    reason = "The protocol tests use only part of the application's test runtimes."
)]
mod tokio_runtimes;

#[cfg(unix)]
pub use served::{Child, Served, on_local, options, runtime, serve};
pub use tokio_runtimes::io_runtime;

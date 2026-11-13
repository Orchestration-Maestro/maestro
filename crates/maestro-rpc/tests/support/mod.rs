//! Test support: a protocol connection over memory, and a child process that serves one
//! runtime over its standard streams.
pub mod rpc_connection;
#[cfg(unix)]
mod served;

#[cfg(unix)]
pub use served::{Child, Served, on_local, options, runtime, serve};

/// The application's scripted session with the owner of its disposable root.
pub type Harness = maestro_app::test_support::Harness<maestro_test_directory::Scratch>;

//! Shared support compiled only for library tests.

#[path = "../tests/support/runtime.rs"]
#[allow(dead_code, reason = "The library tests use only the I/O runtime.")]
pub(crate) mod runtime;
pub(crate) use runtime::io_runtime;

#[path = "../tests/support/no_commands.rs"]
pub(crate) mod no_commands;

#[path = "../tests/support/session_harness.rs"]
#[allow(
    dead_code,
    reason = "The library tests use only part of the session harness."
)]
pub(crate) mod session_harness;

mod session_recovery;

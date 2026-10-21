//! Shared support compiled only for library tests.

#[path = "../tests/support/runtime.rs"]
#[allow(dead_code, reason = "The library tests use only the I/O runtime.")]
pub(crate) mod runtime;

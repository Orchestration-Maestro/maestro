//! Shared support compiled only for library tests.

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use crate::test_support::{ftp_server, runtime, socks, tls};
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use maestro_test_process::loopback;

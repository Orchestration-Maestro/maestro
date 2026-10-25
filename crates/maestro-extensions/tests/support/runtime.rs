//! Test-owned Tokio runtimes shared by the integration tests.
#![cfg(test)]

/// Build one current-thread runtime without I/O or time drivers.
pub fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
}

/// Build one current-thread runtime with its I/O and time drivers.
#[cfg(unix)]
pub fn io_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

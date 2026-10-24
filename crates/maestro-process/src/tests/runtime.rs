//! Current-thread runtimes shared by native process behavior tests.
#![cfg(test)]

/// Builds the same I/O runtime as the synchronous process bridges.
pub(crate) fn runtime() -> tokio::runtime::Runtime {
    crate::children::runtime().unwrap()
}

/// Pauses the clock before the timer driver's origin is created.
pub(crate) fn paused_runtime() -> tokio::runtime::Runtime {
    crate::children::runtime_builder()
        .start_paused(true)
        .build()
        .unwrap()
}

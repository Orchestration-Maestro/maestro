//! Native effects exposed without moving their lifecycle into the component adapter.

/// Cancellable native delay owned by the tools runtime.
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_tools::process::sleep::sleep;

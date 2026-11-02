#![doc = include_str!("../../../docs/terminal/native-terminal.md")]

#[cfg(any(unix, windows))]
pub mod adapter;
#[cfg(any(unix, windows))]
pub use adapter::ProcessTerminal;
#[cfg(not(target_arch = "wasm32"))]
pub mod scheduling;
#[cfg(not(target_arch = "wasm32"))]
pub use scheduling::ProcessTuiRuntime;

mod output_errors;

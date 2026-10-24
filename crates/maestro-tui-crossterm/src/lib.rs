#![doc = include_str!("../../../docs/terminal/native-terminal.md")]

#[cfg(unix)]
pub mod adapter;
#[cfg(unix)]
pub use adapter::ProcessTerminal;
#[cfg(not(target_arch = "wasm32"))]
pub mod scheduling;
#[cfg(not(target_arch = "wasm32"))]
pub use scheduling::ProcessTuiRuntime;

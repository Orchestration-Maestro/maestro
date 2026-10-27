#![doc = include_str!("../../../docs/terminal/native-terminal.md")]

#[cfg(any(unix, windows))]
pub mod terminal;
#[cfg(any(unix, windows))]
pub use terminal::ProcessTerminal;
#[cfg(not(target_arch = "wasm32"))]
pub mod runtime;
#[cfg(not(target_arch = "wasm32"))]
pub use runtime::ProcessTuiRuntime;

mod output_errors;

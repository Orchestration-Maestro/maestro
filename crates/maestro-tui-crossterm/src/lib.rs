#![doc = include_str!("../../../docs/terminal/native-terminal.md")]

#[cfg(unix)]
pub mod terminal;
#[cfg(unix)]
pub use terminal::ProcessTerminal;
#[cfg(not(target_arch = "wasm32"))]
pub mod runtime;
#[cfg(not(target_arch = "wasm32"))]
pub use runtime::ProcessTuiRuntime;

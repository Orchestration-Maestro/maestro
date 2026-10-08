#![doc = include_str!("../../../docs/terminal/native-terminal.md")]

#[cfg(unix)]
pub mod terminal;
#[cfg(unix)]
pub use terminal::ProcessTerminal;

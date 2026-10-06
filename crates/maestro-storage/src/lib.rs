#![doc = include_str!("../../../docs/transcript.md")]

mod types;
pub use types::{Dirent, DirentKind, Storage};

#[cfg(not(target_arch = "wasm32"))]
mod session_manager;
#[cfg(not(target_arch = "wasm32"))]
pub use session_manager::FileStorage;

pub mod conformance;

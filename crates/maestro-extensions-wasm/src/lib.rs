//! Author-side contract for native-async extension components.
//!
//! The canonical interface files in `wit/` define every record and resource. Generated
//! records are re-exported under their domain names; this crate adds only callback and
//! ownership ergonomics around them. The same author code runs against the generated
//! component adapter and against a controlled adapter that implements the same ports.

#[macro_use]
mod port;
mod agent;
mod bash_executor;
#[doc(hidden)]
pub mod bindings;
mod compaction;
mod component_adapter;
mod event_bus;
mod loader;
mod messages;
mod model_registry;
mod models;
mod session_manager;
mod skills;
mod slash_commands;
mod source_info;
mod system_prompt;
#[cfg(test)]
mod tests;
mod tools;
mod tui;
mod types;

pub use agent::*;
pub use bash_executor::*;
pub use compaction::*;
#[doc(hidden)]
pub use component_adapter::Glue;
pub use event_bus::*;
pub use loader::{Extension, ExtensionFactory, load_extension_from_factory};
pub use messages::*;
pub use model_registry::*;
pub use models::*;
pub use session_manager::*;
pub use skills::*;
pub use slash_commands::*;
pub use source_info::*;
pub use system_prompt::*;
pub use tools::*;
pub use tui::*;
pub use types::*;

#[cfg(test)]
extern crate self as maestro_extensions_wasm;

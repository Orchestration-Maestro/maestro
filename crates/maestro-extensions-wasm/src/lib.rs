//! Author-side contract for native-async extension components.
//!
//! The canonical interface files in `wit/` define every record and resource. Generated
//! records are re-exported under their domain names; this crate adds only callback and
//! ownership ergonomics around them. The component adapter is written against the host's
//! imports, so its functions run unchanged in a component and against an in-process host.

#[macro_use]
mod port;
#[doc(hidden)]
pub mod bindings;
mod compaction;
#[cfg(any(test, target_arch = "wasm32"))]
mod component_adapter;
mod loader;
#[cfg(test)]
mod tests;
mod types;

pub use compaction::*;
#[cfg(target_arch = "wasm32")]
#[doc(hidden)]
pub use component_adapter::Glue;
pub use loader::{Extension, ExtensionFactory, load_extension_from_factory};
pub use types::*;

#[cfg(test)]
extern crate self as maestro_extensions_wasm;

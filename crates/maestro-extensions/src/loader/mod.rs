//! Owned registration facade implementations.
mod registration;
pub(crate) use registration::STALE_MESSAGE;

mod actions;
mod construction;
pub use construction::CatalogConstruction;
#[cfg(not(target_arch = "wasm32"))]
mod tool_construction;
#[cfg(not(target_arch = "wasm32"))]
pub use tool_construction::ToolConstruction;
pub(crate) mod runtime;

#[cfg(not(target_arch = "wasm32"))]
mod discovery;
mod factory;
#[cfg(not(target_arch = "wasm32"))]
mod paths;
#[cfg(not(target_arch = "wasm32"))]
pub use discovery::discover_and_load_extensions;
pub use factory::load_extension_from_factory;
#[cfg(not(target_arch = "wasm32"))]
pub use paths::{load_extensions, load_extensions_with_runtime};

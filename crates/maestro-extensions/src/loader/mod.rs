//! Owned registration facade implementations.
mod registration;
mod ui_errors;
pub(crate) use registration::STALE_MESSAGE;

mod actions;
mod construction;
pub use construction::CatalogConstruction;
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

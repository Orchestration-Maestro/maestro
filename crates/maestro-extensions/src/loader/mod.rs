//! Owned registration facade implementations.
mod registration;

mod actions;
pub(crate) mod runtime;

mod factory;
pub use factory::load_extension_from_factory;

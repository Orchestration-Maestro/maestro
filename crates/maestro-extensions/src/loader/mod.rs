//! Owned registration facade implementations.
mod registration;
pub(crate) use registration::STALE_MESSAGE;

mod actions;
pub(crate) mod runtime;

mod factory;
pub use factory::load_extension_from_factory;

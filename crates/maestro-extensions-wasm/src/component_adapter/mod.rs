//! Component adapter: serves the exports of the extension world and forwards registrations
//! and actions to the host imports. All logic is written against [`Imports`], so a component
//! and an in-process host run the same functions.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

mod callbacks;
mod contexts;
mod exports;
#[cfg(target_arch = "wasm32")]
mod generated;
mod host_api;
mod imports;

pub(crate) use callbacks::release;
pub(crate) use exports::Exports;
#[cfg(target_arch = "wasm32")]
pub use generated::Glue;
pub(crate) use imports::Imports;

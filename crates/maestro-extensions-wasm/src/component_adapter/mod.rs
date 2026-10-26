//! Component adapter: serves the exports of the extension world and forwards registrations
//! and actions to the host imports. All logic is written against [`Imports`], so a component
//! and an in-process host run the same functions.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

mod callbacks;
mod compaction;
mod event_bus;
pub(crate) mod renderers;
#[cfg(test)]
pub(crate) use event_bus::events;
pub(crate) mod bash;
mod contexts;
mod signals;
#[cfg(test)]
pub(crate) use signals::signal;
mod exports;
pub(crate) mod factories;
#[cfg(target_arch = "wasm32")]
mod generated;
mod host_api;
mod imports;
mod model_registry;
mod schema;
mod session;
mod setup;
mod theme;
mod tools;
#[cfg(test)]
pub(crate) use tools::ToolCapabilities;

pub(crate) use callbacks::release;
pub(crate) use exports::{Capabilities, Exports};
#[cfg(test)]
pub(crate) use exports::{EventOutcome, outcome};
#[cfg(target_arch = "wasm32")]
pub use generated::Glue;
#[cfg(test)]
pub(crate) use imports::Dialog;
pub(crate) use imports::Imports;

pub(crate) use imports::ToolListing;

/// Decode a complete JSON payload with the nesting admitted by authored schemas.
pub(crate) use maestro_request::parameter_schema::transport::decode_json;

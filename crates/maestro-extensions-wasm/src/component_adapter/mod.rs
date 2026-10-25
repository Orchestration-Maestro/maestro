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
mod exports;
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
pub(crate) use imports::Imports;

pub(crate) use imports::ToolListing;

/// Decode a complete JSON payload with the nesting admitted by authored schemas.
pub(crate) fn decode_json<T: serde::de::DeserializeOwned>(text: &str) -> crate::ExtensionResult<T> {
    let mut decoder = serde_json::Deserializer::from_str(text);
    decoder.disable_recursion_limit();
    let value = T::deserialize(&mut decoder).map_err(|error| error.to_string())?;
    decoder.end().map_err(|error| error.to_string())?;
    Ok(value)
}

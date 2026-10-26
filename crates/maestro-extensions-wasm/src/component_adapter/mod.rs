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
mod editors;
mod signals;
pub(crate) mod streams;
#[cfg(target_arch = "wasm32")]
pub(crate) use generated::model_streams;
#[cfg(target_arch = "wasm32")]
pub(crate) use generated::new_event_stream;
#[cfg(test)]
pub(crate) use signals::signal;
mod exports;
pub(crate) mod factories;
#[cfg(target_arch = "wasm32")]
mod generated;
mod host_api;
mod imports;
mod keys;
mod lists;
mod model_registry;
mod schema;
mod session;
mod setup;
mod theme;
pub(crate) mod tool_renders;
mod tools;
mod ui;
mod widget_imports;
mod widgets;
pub(crate) use lists::invoke_widget;
#[cfg(test)]
pub(crate) use tools::ToolCapabilities;
pub(crate) use widget_imports::{
    ChildKey, ChildRef, EditorAnswer, EditorRequest, EditorStyles, HandlerSlot, HighlightRef,
    HostMarkdownTheme, HostSettingsTheme, KeyHint, KeybindingsAnswer, KeybindingsRequest,
    LayoutRef, MarkdownStyles, RowStyleRef, SettingRef, SettingsStyles, StyleRef, TextStyle,
    WidgetAnswer, WidgetEvent, WidgetImports,
};
#[cfg(test)]
pub(crate) use widgets::authored_component;
pub(crate) use widgets::{Builder, install};
pub(crate) use widgets::{invoke_highlight, invoke_style, widget_host};

pub(crate) use callbacks::release;
pub(crate) use exports::{Capabilities, Exports};
#[cfg(test)]
pub(crate) use exports::{EventOutcome, outcome};
#[cfg(target_arch = "wasm32")]
pub use generated::Glue;
#[cfg(test)]
pub(crate) use imports::Dialog;
pub(crate) use imports::Imports;
#[cfg(test)]
pub(crate) use imports::ToolCallbacks;

pub(crate) use imports::ToolListing;

/// Decode a complete JSON payload with the nesting admitted by authored schemas.
pub(crate) use maestro_request::parameter_schema::transport::decode_json;

pub(crate) mod auth_storage;

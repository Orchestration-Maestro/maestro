//! Private declaration fragments for the application interface.

/// Existing browser storage re-exports.
mod browser_storage;
/// Existing browser tools re-exports.
mod browser_tools;
/// Existing clipboard re-exports.
mod clipboard;
/// Existing configuration re-exports.
mod configuration;
/// Existing credentials re-exports.
mod credentials;
/// Named daemon records and profiling operations for frontends.
#[cfg(not(target_arch = "wasm32"))]
#[path = "daemon.rs"]
mod daemon_exports;
/// Existing extensions re-exports.
mod extensions;
/// Existing json re-exports.
mod json;
/// Named Markdown records and parsing operations for frontends.
#[path = "markdown.rs"]
mod markdown_exports;
/// Existing models re-exports.
mod models;
/// Existing packages re-exports.
mod packages;
/// Existing presentation data re-exports.
mod presentation_data;
/// Existing process re-exports.
mod process;
/// Existing resources re-exports.
mod resources;
/// The JSONL protocol records and server.
#[cfg(not(target_arch = "wasm32"))]
mod rpc;
/// Existing session re-exports.
mod session;
/// Existing settings re-exports.
mod settings;
/// Named terminal emulator records for the pane renderer.
#[cfg(not(target_arch = "wasm32"))]
#[path = "terminal.rs"]
mod terminal_exports;
/// Existing text re-exports.
mod text;
/// Existing theme re-exports.
#[cfg(not(target_arch = "wasm32"))]
mod theme;
/// Existing tools re-exports.
mod tools;

pub use browser_storage::*;
pub use browser_tools::*;
#[cfg(not(target_arch = "wasm32"))]
pub use clipboard::*;
pub use configuration::*;
pub use credentials::*;
#[cfg(not(target_arch = "wasm32"))]
pub use daemon_exports::daemon;
pub use extensions::*;
pub use json::*;
pub use markdown_exports::markdown;
pub use models::*;
#[cfg(not(target_arch = "wasm32"))]
pub use packages::*;
pub use presentation_data::*;
#[cfg(not(target_arch = "wasm32"))]
pub use process::*;
pub use resources::*;
#[cfg(not(target_arch = "wasm32"))]
pub use rpc::*;
pub use session::*;
pub use settings::*;
#[cfg(not(target_arch = "wasm32"))]
pub use terminal_exports::terminal;
pub use text::*;
#[cfg(not(target_arch = "wasm32"))]
pub use theme::*;
pub use tools::*;

/// Changelog utility re-exports.
#[cfg(not(target_arch = "wasm32"))]
pub use crate::utils::changelog::{
    ChangelogEntry, compare_versions, get_changelog_path, get_new_entries, parse_changelog,
};

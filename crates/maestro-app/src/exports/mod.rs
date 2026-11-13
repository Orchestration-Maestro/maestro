//! Private declaration fragments for the application interface.

/// Clipboard copy helpers.
mod clipboard;
/// Application name, version, directory names and environment variable names.
mod configuration;
/// Credential records and the credential storage backends.
mod credentials;
/// Extension runtime, context, event and registration types.
mod extensions;
/// The model registry and command-line model resolution.
mod models;
/// The package manager and its resolved resource records.
mod packages;
/// Footer data provider, keybinding manager and slash command records.
mod presentation_data;
/// Resource loader, its options, diagnostics and collisions.
mod resources;
/// Session manager, session entry records and skill block parsing.
mod session;
/// The settings manager and its compaction, image and retry records.
mod settings;
/// Theme records, highlighting and markdown theme helpers.
#[cfg(not(target_arch = "wasm32"))]
mod theme;
/// Built-in tool constructors, options and execution records.
mod tools;

#[cfg(not(target_arch = "wasm32"))]
pub use clipboard::*;
pub use configuration::*;
pub use credentials::*;
pub use extensions::*;
pub use models::*;
#[cfg(not(target_arch = "wasm32"))]
pub use packages::*;
pub use presentation_data::*;
pub use resources::*;
pub use session::*;
pub use settings::*;
#[cfg(not(target_arch = "wasm32"))]
pub use theme::*;
pub use tools::*;

/// Changelog utility re-exports.
#[cfg(not(target_arch = "wasm32"))]
pub use crate::utils::release_notes::{
    ChangelogEntry, compare_versions, get_changelog_path, get_new_entries, parse_changelog,
};

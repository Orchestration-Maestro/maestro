//! Existing presentation data owners exposed at the application root.

pub use crate::presentation_data::status_line::{ExtensionStatuses, ReadonlyFooterDataProvider};

/// Configurable application action identifiers and their shared manager.
pub use crate::presentation_data::bindings::{AppKeybinding, KeybindingsManager};

pub use crate::presentation_data::slash_commands::{SlashCommandInfo, SlashCommandSource};

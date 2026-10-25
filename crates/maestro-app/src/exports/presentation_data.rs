//! Existing presentation data owners exposed at the application root.

pub use crate::presentation_data::footer_data_provider::{
    ExtensionStatuses, ReadonlyFooterDataProvider,
};

/// Configurable application action identifiers and their shared manager.
pub use crate::presentation_data::keybindings::{AppKeybinding, KeybindingsManager};

pub use crate::presentation_data::slash_commands::{SlashCommandInfo, SlashCommandSource};

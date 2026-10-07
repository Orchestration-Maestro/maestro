//! Reusable terminal presentation without native terminal IO.
#![doc = include_str!("../../../docs/terminal.md")]
#![doc = include_str!("../../../docs/terminal/components.md")]
/// Presentation components.
pub mod components {
    /// Editor capabilities.
    pub mod editor_component;
    /// Synchronous composition.
    pub mod tui;
}
pub use components::tui::{CURSOR_MARKER, Component, Container, Focusable, is_focusable};
/// Terminal text operations.
pub mod text {
    /// Retained first-line text.
    pub mod truncated_text;
    /// Cell operations.
    pub mod utils;
}
pub use text::{
    truncated_text::TruncatedText,
    utils::{truncate_to_width, visible_width},
};

pub use text::utils::wrap_text_with_ansi;

/// Caller-supplied terminal lifecycle and IO contract.
pub mod terminal;
/// Editor contracts.
pub mod editor {
    /// Completion contracts.
    pub mod completion {
        /// Completion records and provider operations.
        pub mod autocomplete;
    }
}
pub use components::{
    editor_component::EditorComponent,
    tui::{OverlayAnchor, OverlayHandle, OverlayMargin, OverlayOptions, SizeValue},
};
pub use editor::completion::autocomplete::{
    AutocompleteItem, AutocompleteProvider, AutocompleteSuggestions, CursorPosition, SlashCommand,
};
pub use terminal::Terminal;

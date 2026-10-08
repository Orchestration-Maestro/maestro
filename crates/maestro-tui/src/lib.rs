#![doc = include_str!("../../../docs/terminal/components.md")]

pub mod autocomplete;
pub mod components;
pub mod editor_component;
pub mod terminal;
pub mod text;
pub mod tui;

pub use autocomplete::{
    AutocompleteItem, AutocompleteProvider, AutocompleteSuggestions, SlashCommand,
};
pub use components::TruncatedText;
pub use editor_component::EditorComponent;
pub use terminal::Terminal;
pub use text::utils::{
    AnsiCode, ColumnSlice, ExtractedSegments, TruncateOptions, apply_background_to_line,
    extract_ansi_code, extract_segments, get_segmenter, is_punctuation_char, is_whitespace_char,
    normalize_terminal_output, slice_by_column, slice_with_width, truncate_to_width, visible_width,
    wrap_text_with_ansi,
};
pub use tui::{
    CURSOR_MARKER, Component, Container, Focusable, OverlayAnchor, OverlayHandle, OverlayMargin,
    OverlayOptions, SizeValue, is_focusable,
};

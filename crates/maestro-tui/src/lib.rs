#![doc = include_str!("../../../docs/terminal/components.md")]

pub mod autocomplete;
pub mod components;
pub mod editor_component;
pub mod images;
pub mod keybindings;
pub mod keys;
pub mod stdin_buffer;
pub mod terminal;
pub mod text;
pub mod tui;

pub use autocomplete::{
    AutocompleteItem, AutocompleteProvider, AutocompleteSuggestions, SlashCommand,
};
pub use components::{Image, ImageOptions, ImageTheme, TruncatedText};
pub use editor_component::EditorComponent;
pub use keys::{
    Key, KeyEventType, KeyId, decode_kitty_printable, is_key_release, is_key_repeat,
    is_kitty_protocol_active, matches_key, parse_key, set_kitty_protocol_active,
};
pub use stdin_buffer::{StdinBuffer, StdinBufferEventMap, StdinBufferInput, StdinBufferOptions};
pub use terminal::Terminal;
pub use text::utils::{
    AnsiCode, ColumnSlice, ExtractedSegments, TruncateOptions, apply_background_to_line,
    extract_ansi_code, extract_segments, get_segmenter, is_punctuation_char, is_whitespace_char,
    normalize_terminal_output, slice_by_column, slice_with_width, truncate_to_width, visible_width,
    wrap_text_with_ansi,
};
pub use tui::{
    CURSOR_MARKER, Component, Container, FocusFlag, Focusable, OverlayAnchor, OverlayHandle,
    OverlayMargin, OverlayOptions, SizeValue, TUI, is_focusable,
};

pub use images::terminal_image::{
    CellDimensions, ITerm2Options, ImageDimensions, ImageProtocol, ImageRenderOptions, ImageSize,
    KittyOptions, RenderedImage, TerminalCapabilities, TerminalImage, calculate_image_rows,
    delete_all_kitty_images, delete_kitty_image, encode_i_term2, encode_kitty, get_gif_dimensions,
    get_image_dimensions, get_jpeg_dimensions, get_png_dimensions, get_webp_dimensions, hyperlink,
    image_fallback, is_image_line,
};

pub use keybindings::{
    Keybinding, KeybindingConflict, KeybindingDefinition, KeybindingDefinitions, KeybindingKeys,
    KeybindingsConfig, KeybindingsManager, TUI_KEYBINDINGS, get_keybindings, set_keybindings,
};

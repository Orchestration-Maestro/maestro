#![doc = include_str!("../../../docs/terminal/components.md")]

mod autocomplete;
mod keybindings;
mod kill_ring;
pub use kill_ring::*;
pub mod components;
pub mod editor_component;
mod fuzzy;
pub mod images;
pub mod keys;
mod stdin_buffer;
pub mod terminal;
pub mod text;
mod tui;
pub mod undo_stack;

pub use autocomplete::*;
pub use components::{
    Box, CancellableLoader, DefaultTextStyle, Editor, EditorOptions, EditorTheme, Image,
    ImageOptions, ImageTheme, Input, Loader, LoaderIndicatorOptions, Markdown, MarkdownOptions,
    MarkdownTheme, Spacer, Text, TextDecoration, TruncatedText,
};
pub use editor_component::EditorComponent;
pub use keys::{
    Key, KeyEventType, KeyId, decode_kitty_printable, is_key_release, is_key_repeat,
    is_kitty_protocol_active, matches_key, parse_key, set_kitty_protocol_active,
};
pub use stdin_buffer::*;
pub use terminal::Terminal;
pub use text::utils::{
    AnsiCode, ColumnSlice, ExtractedSegments, TruncateOptions, apply_background_to_line,
    extract_ansi_code, extract_segments, get_segmenter, is_punctuation_char, is_whitespace_char,
    normalize_terminal_output, slice_by_column, slice_with_width, truncate_to_width, visible_width,
    wrap_text_with_ansi,
};
pub use tui::*;

pub use images::terminal_image::{
    CellDimensions, ITerm2Options, ImageCellSize, ImageDimensions, ImageProtocol,
    ImageRenderOptions, ImageSize, KittyOptions, PANE_TERM_PROGRAM, RenderedImage, TERM_PROGRAM,
    TerminalCapabilities, TerminalImage, calculate_image_cell_size, calculate_image_rows,
    delete_all_kitty_images, delete_kitty_image, encode_i_term2, encode_kitty, get_gif_dimensions,
    get_image_dimensions, get_jpeg_dimensions, get_png_dimensions, get_webp_dimensions, hyperlink,
    image_fallback, is_image_line,
};

pub use keybindings::*;

pub use fuzzy::*;

pub use components::{
    SelectItem, SelectList, SelectListLayoutOptions, SelectListTheme,
    SelectListTruncatePrimaryContext, SettingItem, SettingsList, SettingsListTheme,
};

pub use maestro_cancellation::Cancellation;

pub use text::expand_tabs;

/// Host terminal colour queries and OSC reply parsing.
pub mod terminal_theme;

/// Opt-in helpers for controlled tests.
#[cfg(feature = "test-support")]
pub mod test_support;

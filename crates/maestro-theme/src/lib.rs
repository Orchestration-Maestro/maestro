//! Typed terminal themes: color construction, loading, registration, discovery, live publication, export colors, text decorations, code highlighting and component styles.
//!
//! See `docs/theme.md` for the file format, color modes and errors.

pub mod palette;
pub use palette::watch_with_error_handler;
pub use palette::{BrandFont, BrandGlyph, BrandMark, BrandMode, BrandPack, BrandPresentation};
pub use palette::{BrandType, load_brand_pack};
#[cfg(not(target_arch = "wasm32"))]
pub use palette::{Collator, NativeThemeWatchOperations};
pub use palette::{ColorMode, ColorValue, Theme, ThemeBg, ThemeColor, ThemeError, ThemeOptions};
pub use palette::{
    HighlightProfile, get_settings_list_theme, highlight_auto, highlight_code, highlight_css,
    highlight_html,
};
pub use palette::{
    LiveTheme, ThemeChangeResult, ThemeExportColors, ansi256_to_hex, is_light_theme,
};
pub use palette::{SyntaxHighlighter, SyntaxSpan, SyntectHighlighter, get_language_from_path};
pub use palette::{ThemeDirectories, ThemeInfo, ThemeState, load_theme_from_path};
pub use palette::{ThemeReloadTimer, ThemeWatchOperations, ThemeWatcher, close_watcher};
pub use palette::{get_editor_theme, get_markdown_theme, get_select_list_theme};

pub use palette::paint;

mod highlight;

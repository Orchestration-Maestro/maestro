//! Typed terminal themes: color construction, loading, registration, discovery, live publication, export colors, text decorations, code highlighting and component styles.
//!
//! See `docs/theme.md` for the file format, color modes and errors.

pub mod theme;
pub use theme::watch_with_error_handler;
pub use theme::{BrandFont, BrandGlyph, BrandMark, BrandMode, BrandPack, BrandPresentation};
pub use theme::{BrandType, load_brand_pack};
pub use theme::{ColorMode, ColorValue, Theme, ThemeBg, ThemeColor, ThemeError, ThemeOptions};
pub use theme::{LiveTheme, ThemeChangeResult, ThemeExportColors, is_light_theme};
#[cfg(not(target_arch = "wasm32"))]
pub use theme::{NativeThemeOperations, NativeThemeWatchOperations};
pub use theme::{SyntaxHighlighter, SyntaxSpan, SyntectHighlighter, get_language_from_path};
pub use theme::{ThemeDirectories, ThemeInfo, ThemeOperations, ThemeState, load_theme_from_path};
pub use theme::{ThemeReloadTimer, ThemeWatchOperations, ThemeWatcher, close_watcher};
pub use theme::{get_editor_theme, get_markdown_theme, get_select_list_theme};
pub use theme::{get_settings_list_theme, highlight_code, highlight_html};

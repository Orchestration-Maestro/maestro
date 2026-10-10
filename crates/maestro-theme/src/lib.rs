//! Typed terminal themes: color construction, loading, registration and discovery.
//!
//! See `docs/theme.md` for the file format, color modes and errors.

pub mod theme;
#[cfg(not(target_arch = "wasm32"))]
pub use theme::NativeThemeOperations;
pub use theme::{ColorMode, ColorValue, Theme, ThemeBg, ThemeColor, ThemeError, ThemeOptions};
pub use theme::{ThemeDirectories, ThemeInfo, ThemeOperations, ThemeState, load_theme_from_path};

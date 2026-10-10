//! Typed terminal themes: color construction and loading from theme files.
//!
//! See `docs/theme.md` for the file format, color modes and errors.

pub mod theme;
#[cfg(not(target_arch = "wasm32"))]
pub use theme::NativeThemeOperations;
pub use theme::{ColorMode, ColorValue, Theme, ThemeBg, ThemeColor, ThemeError, ThemeOptions};
pub use theme::{ThemeOperations, load_theme_from_path};

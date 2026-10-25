//! Theme files, environments and documents shared by the theme tests.
pub mod environment;
pub mod highlighting;

use maestro_test_directory::Scratch;
use maestro_theme::{ColorMode, Theme, ThemeColor, ThemeError, load_theme_from_path};
use serde_json::Value;

/// The shipped dark theme document.
const DARK: &str = include_str!("../../assets/theme/dark.json");
/// The shipped dark theme file, readable from children whose environment is cleared.
pub const DARK_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/theme/dark.json");
/// The published editor schema.
const PUBLISHED_SCHEMA: &str = include_str!("../../assets/theme/theme-schema.json");

/// A file named `fixture` in a fresh directory that is removed on drop.
pub struct ThemeFile {
    /// Owner of the directory holding the file.
    pub directory: Scratch,
    /// The file's native path.
    pub path: String,
}

impl ThemeFile {
    /// Write `content` to the file.
    ///
    /// # Panics
    /// Panics if the directory or the file cannot be created.
    #[must_use]
    pub fn new(content: impl AsRef<[u8]>) -> Self {
        let directory = Scratch::new().unwrap();
        let path = directory.path().join("fixture");
        std::fs::write(&path, content).unwrap();
        Self {
            path: path.to_str().unwrap().to_owned(),
            directory,
        }
    }

    /// Load the file through the public loader.
    ///
    /// # Errors
    /// Returns the theme loader failure.
    pub fn load(&self, mode: Option<ColorMode>) -> Result<Theme, ThemeError> {
        load_theme_from_path(&self.path, mode)
    }
}

/// Parse the shipped dark theme.
///
/// # Panics
/// Panics if the bundled document is invalid.
#[must_use]
pub fn dark() -> Value {
    serde_json::from_str(DARK).unwrap()
}

/// The published schema's required color names in authored order.
///
/// # Panics
/// Panics if the bundled schema has an invalid required-color list.
#[must_use]
pub fn required_colors() -> Vec<String> {
    let schema: Value = serde_json::from_str(PUBLISHED_SCHEMA).unwrap();
    let required = schema["properties"]["colors"]["required"]
        .as_array()
        .unwrap();
    required
        .iter()
        .map(|name| name.as_str().unwrap().to_owned())
        .collect()
}

/// Load theme text written to a fresh file through the public loader.
///
/// # Errors
/// Returns the theme loader failure.
pub fn load(text: &str, mode: Option<ColorMode>) -> Result<Theme, ThemeError> {
    ThemeFile::new(text).load(mode)
}

/// The foreground prefix stored under a literal key.
///
/// # Panics
/// Panics if the theme does not contain the requested key.
pub fn fg(theme: &Theme, name: &str) -> String {
    theme
        .get_fg_ansi(&ThemeColor::Named(name.into()))
        .unwrap()
        .to_owned()
}

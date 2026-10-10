//! Controlled theme operations and documents shared by the theme tests.
use maestro_theme::{
    ColorMode, Theme, ThemeColor, ThemeError, ThemeOperations, load_theme_from_path,
};
use serde_json::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io;

/// The shipped dark theme document.
const DARK: &str = include_str!("../../assets/theme/dark.json");
/// The published editor schema.
const PUBLISHED_SCHEMA: &str = include_str!("../../assets/theme/theme-schema.json");

/// Serves fixed theme text and environment while recording each effect in order.
pub struct Controlled {
    /// Theme text, or `None` for a missing file.
    pub content: Option<String>,
    /// Environment variables that are set.
    env: HashMap<String, String>,
    /// `read:<path>` and `env:<name>` entries in call order.
    pub log: RefCell<Vec<String>>,
}

impl Controlled {
    /// Serve `content` with the given environment.
    pub fn new(content: &str, env: &[(&str, &str)]) -> Self {
        Self {
            content: Some(content.to_owned()),
            env: env
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
            log: RefCell::default(),
        }
    }
}

impl ThemeOperations for Controlled {
    fn read_to_string(&self, path: &str) -> io::Result<String> {
        self.log.borrow_mut().push(format!("read:{path}"));
        self.content
            .clone()
            .ok_or_else(|| io::ErrorKind::NotFound.into())
    }

    fn environment(&self, name: &str) -> Option<String> {
        self.log.borrow_mut().push(format!("env:{name}"));
        self.env.get(name).cloned()
    }
}

/// Parse the shipped dark theme.
pub fn dark() -> Value {
    serde_json::from_str(DARK).unwrap()
}

/// The published schema's required color names in authored order.
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

/// Load theme text through the public loader with an empty environment.
pub fn load(text: &str, mode: Option<ColorMode>) -> Result<Theme, ThemeError> {
    load_theme_from_path("fixture", mode, &Controlled::new(text, &[]))
}

/// The foreground prefix stored under a literal key.
pub fn fg(theme: &Theme, name: &str) -> String {
    theme
        .get_fg_ansi(&ThemeColor::Named(name.into()))
        .unwrap()
        .to_owned()
}

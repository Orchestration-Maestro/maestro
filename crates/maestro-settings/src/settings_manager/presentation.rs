//! Terminal, image, editor and session-directory preferences.

use std::path::{Component, Path, PathBuf};

use serde_json::Value;

use super::SettingsManager;
use super::conversion::number;
use super::records::{ImageSettings, MarkdownSettings, TerminalSettings};
use super::vocabulary::{DoubleEscapeAction, TreeFilterMode};

/// Environment variable that enables clearing empty rows when none is configured.
const CLEAR_ON_SHRINK_ENV: &str = "MAESTRO_CLEAR_ON_SHRINK";
/// Environment variable that shows the hardware cursor when none is configured.
const HARDWARE_CURSOR_ENV: &str = "MAESTRO_HARDWARE_CURSOR";

/// Whether an environment variable is set to exactly `1`.
fn env_enabled(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| value == "1")
}

impl SettingsManager {
    /// Returns the session directory; an exact `~` or `~/` prefix expands to the home directory.
    #[must_use]
    pub fn get_session_dir(&self) -> Option<PathBuf> {
        let configured = self.text("sessionDir")?;
        let expanded = if configured == "~" {
            std::env::home_dir()
        } else {
            let suffix = configured.strip_prefix("~/");
            suffix
                .and_then(|suffix| std::env::home_dir().map(|home| join_normalized(&home, suffix)))
        };
        Some(expanded.unwrap_or_else(|| PathBuf::from(configured)))
    }

    /// Returns whether inline images are shown (default true).
    #[must_use]
    pub fn get_show_images(&self) -> bool {
        self.record::<TerminalSettings>("terminal")
            .show_images
            .unwrap_or(true)
    }

    /// Shows or hides inline images.
    pub fn set_show_images(&mut self, value: bool) {
        self.set_global_key("terminal", "showImages", value);
    }

    /// Returns the inline image width in cells (default 60, at least 1).
    #[must_use]
    pub fn get_image_width_cells(&self) -> f64 {
        self.record::<TerminalSettings>("terminal")
            .image_width_cells
            .map_or(60.0, |width| width.floor().max(1.0))
    }

    /// Sets the inline image width in cells, rounded down to at least 1.
    pub fn set_image_width_cells(&mut self, value: f64) {
        let width = value.floor();
        let width = if width.is_nan() {
            width
        } else {
            width.max(1.0)
        };
        self.set_global_key("terminal", "imageWidthCells", number(width));
    }

    /// Returns whether empty rows are cleared when content shrinks. A configured
    /// value, including `null`, wins over the environment.
    #[must_use]
    pub fn get_clear_on_shrink(&self) -> bool {
        let configured = self
            .effective
            .get("terminal")
            .and_then(|terminal| terminal.get("clearOnShrink"));
        match configured {
            Some(Value::Bool(value)) => *value,
            Some(Value::Null) => false,
            _ => env_enabled(CLEAR_ON_SHRINK_ENV),
        }
    }

    /// Enables or disables clearing empty rows when content shrinks.
    pub fn set_clear_on_shrink(&mut self, value: bool) {
        self.set_global_key("terminal", "clearOnShrink", value);
    }

    /// Returns whether terminal progress indicators are shown (default false).
    #[must_use]
    pub fn get_show_terminal_progress(&self) -> bool {
        self.record::<TerminalSettings>("terminal")
            .show_terminal_progress
            .unwrap_or(false)
    }

    /// Shows or hides terminal progress indicators.
    pub fn set_show_terminal_progress(&mut self, value: bool) {
        self.set_global_key("terminal", "showTerminalProgress", value);
    }

    /// Returns whether images are resized for model compatibility (default true).
    #[must_use]
    pub fn get_image_auto_resize(&self) -> bool {
        self.record::<ImageSettings>("images")
            .auto_resize
            .unwrap_or(true)
    }

    /// Enables or disables image resizing.
    pub fn set_image_auto_resize(&mut self, value: bool) {
        self.set_global_key("images", "autoResize", value);
    }

    /// Returns whether images are withheld from model providers (default false).
    #[must_use]
    pub fn get_block_images(&self) -> bool {
        self.record::<ImageSettings>("images")
            .block_images
            .unwrap_or(false)
    }

    /// Blocks or allows images for model providers.
    pub fn set_block_images(&mut self, value: bool) {
        self.set_global_key("images", "blockImages", value);
    }

    /// Returns the double-escape action (default tree).
    #[must_use]
    pub fn get_double_escape_action(&self) -> DoubleEscapeAction {
        self.text("doubleEscapeAction")
            .map_or(DoubleEscapeAction::Tree, |text| {
                DoubleEscapeAction::from_text(&text)
            })
    }

    /// Sets the double-escape action.
    pub fn set_double_escape_action(&mut self, value: DoubleEscapeAction) {
        self.set_global("doubleEscapeAction", value);
    }

    /// Returns the tree filter mode; unrecognized values read as the default.
    #[must_use]
    pub fn get_tree_filter_mode(&self) -> TreeFilterMode {
        self.text("treeFilterMode")
            .and_then(|text| TreeFilterMode::from_text(&text))
            .unwrap_or(TreeFilterMode::Default)
    }

    /// Sets the tree filter mode.
    pub fn set_tree_filter_mode(&mut self, value: TreeFilterMode) {
        self.set_global("treeFilterMode", value.as_str());
    }

    /// Returns whether the hardware cursor is shown; an unset or `null` value
    /// falls back to the environment.
    #[must_use]
    pub fn get_show_hardware_cursor(&self) -> bool {
        self.flag("showHardwareCursor")
            .unwrap_or_else(|| env_enabled(HARDWARE_CURSOR_ENV))
    }

    /// Shows or hides the hardware cursor.
    pub fn set_show_hardware_cursor(&mut self, value: bool) {
        self.set_global("showHardwareCursor", value);
    }

    /// Returns the editor's horizontal padding (default 0).
    #[must_use]
    pub fn get_editor_padding_x(&self) -> f64 {
        if self.not_a_number.editor_padding_x {
            return f64::NAN;
        }
        self.effective
            .get("editorPaddingX")
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
    }

    /// Sets the editor's horizontal padding, rounded down into 0 to 3.
    pub fn set_editor_padding_x(&mut self, value: f64) {
        let padding = clamp_floor(value, 0.0, 3.0);
        self.not_a_number.editor_padding_x = padding.is_nan();
        self.set_global("editorPaddingX", number(padding));
    }

    /// Returns the visible autocomplete rows (default 5).
    #[must_use]
    pub fn get_autocomplete_max_visible(&self) -> f64 {
        if self.not_a_number.autocomplete_max_visible {
            return f64::NAN;
        }
        self.effective
            .get("autocompleteMaxVisible")
            .and_then(Value::as_f64)
            .unwrap_or(5.0)
    }

    /// Sets the visible autocomplete rows, rounded down into 3 to 20.
    pub fn set_autocomplete_max_visible(&mut self, value: f64) {
        let rows = clamp_floor(value, 3.0, 20.0);
        self.not_a_number.autocomplete_max_visible = rows.is_nan();
        self.set_global("autocompleteMaxVisible", number(rows));
    }

    /// Returns the code block indentation (default two spaces).
    #[must_use]
    pub fn get_code_block_indent(&self) -> String {
        self.record::<MarkdownSettings>("markdown")
            .code_block_indent
            .unwrap_or_else(|| "  ".to_owned())
    }
}

/// Rounds down and clamps into `[low, high]`, keeping `NaN` and mapping `-0` to `0`.
fn clamp_floor(value: f64, low: f64, high: f64) -> f64 {
    let floored = value.floor();
    if floored.is_nan() {
        return floored;
    }
    floored.clamp(low, high) + 0.0
}

/// Joins `suffix` onto `home`, resolving empty, `.` and `..` segments and keeping a trailing separator.
fn join_normalized(home: &Path, suffix: &str) -> PathBuf {
    let mut parts: Vec<Component<'_>> = home.components().collect();
    for segment in suffix.split('/') {
        match segment {
            "" | "." => {}
            ".." => match parts.last() {
                Some(Component::Normal(_)) => {
                    parts.pop();
                }
                Some(Component::RootDir | Component::Prefix(_)) => {}
                _ => parts.push(Component::ParentDir),
            },
            name => parts.push(Component::Normal(name.as_ref())),
        }
    }
    let mut path: PathBuf = parts.into_iter().collect();
    if suffix.ends_with('/') {
        path.push("");
    }
    path
}

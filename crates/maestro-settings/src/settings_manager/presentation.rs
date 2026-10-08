//! Terminal, image, editor and session-directory preferences.

use std::path::{Component, Path, PathBuf};

use serde_json::Value;

use super::SettingsManager;
use super::conversion::number;
use super::paths::normalized;
use super::vocabulary::{DoubleEscapeAction, TreeFilterMode};

/// Environment variable that enables clearing empty rows when the preference is
/// neither a boolean nor `null`.
const CLEAR_ON_SHRINK_ENV: &str = "MAESTRO_CLEAR_ON_SHRINK";
/// Environment variable that shows the hardware cursor when the preference is
/// not a boolean.
const HARDWARE_CURSOR_ENV: &str = "MAESTRO_HARDWARE_CURSOR";

/// Whether an environment variable is set to exactly `1`.
fn env_enabled(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| value == "1")
}

impl SettingsManager {
    /// Returns the session directory. An exact `~` reads as the home directory and
    /// a `~/` prefix is joined onto it; any other text, and any text while the
    /// home directory is unknown, is returned as written.
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
        self.nested("terminal", "showImages").unwrap_or(true)
    }

    /// Shows or hides inline images.
    pub fn set_show_images(&mut self, value: bool) {
        self.set_global_key("terminal", "showImages", value);
    }

    /// Returns the inline image width in cells: a stored number rounded down with a
    /// minimum of 1, or 60 when it is unset or not a number.
    #[must_use]
    pub fn get_image_width_cells(&self) -> f64 {
        self.nested("terminal", "imageWidthCells")
            .map_or(60.0, |width: f64| width.floor().max(1.0))
    }

    /// Sets the inline image width in cells, rounded down with a minimum of 1;
    /// `NaN` and positive infinity are stored as `null`.
    pub fn set_image_width_cells(&mut self, value: f64) {
        let width = value.floor();
        let width = if width.is_nan() {
            width
        } else {
            width.max(1.0)
        };
        self.set_global_key("terminal", "imageWidthCells", number(width));
    }

    /// Returns whether empty rows are cleared when content shrinks: a stored
    /// boolean, `false` for a stored `null`, and otherwise the environment.
    #[must_use]
    pub fn get_clear_on_shrink(&self) -> bool {
        let configured = self
            .object("terminal")
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
        self.nested("terminal", "showTerminalProgress")
            .unwrap_or(false)
    }

    /// Shows or hides terminal progress indicators.
    pub fn set_show_terminal_progress(&mut self, value: bool) {
        self.set_global_key("terminal", "showTerminalProgress", value);
    }

    /// Returns whether images are resized for model compatibility (default true).
    #[must_use]
    pub fn get_image_auto_resize(&self) -> bool {
        self.nested("images", "autoResize").unwrap_or(true)
    }

    /// Enables or disables image resizing.
    pub fn set_image_auto_resize(&mut self, value: bool) {
        self.set_global_key("images", "autoResize", value);
    }

    /// Returns whether images are withheld from model providers (default false).
    #[must_use]
    pub fn get_block_images(&self) -> bool {
        self.nested("images", "blockImages").unwrap_or(false)
    }

    /// Blocks or allows images for model providers.
    pub fn set_block_images(&mut self, value: bool) {
        self.set_global_key("images", "blockImages", value);
    }

    /// Returns the double-escape action (default tree); unrecognized text is retained.
    #[must_use]
    pub fn get_double_escape_action(&self) -> DoubleEscapeAction {
        self.text("doubleEscapeAction")
            .map_or(DoubleEscapeAction::Tree, DoubleEscapeAction::from_text)
    }

    /// Sets the double-escape action.
    pub fn set_double_escape_action(&mut self, value: DoubleEscapeAction) {
        self.set_global("doubleEscapeAction", value);
    }

    /// Returns the tree filter mode; unrecognized or wrong-typed values read as the
    /// default.
    #[must_use]
    pub fn get_tree_filter_mode(&self) -> TreeFilterMode {
        self.text("treeFilterMode")
            .and_then(TreeFilterMode::from_text)
            .unwrap_or(TreeFilterMode::Default)
    }

    /// Sets the tree filter mode.
    pub fn set_tree_filter_mode(&mut self, value: TreeFilterMode) {
        self.set_global("treeFilterMode", value.as_str());
    }

    /// Returns whether the hardware cursor is shown; an unset, `null` or
    /// wrong-typed value falls back to the environment.
    #[must_use]
    pub fn get_show_hardware_cursor(&self) -> bool {
        self.flag("showHardwareCursor")
            .unwrap_or_else(|| env_enabled(HARDWARE_CURSOR_ENV))
    }

    /// Shows or hides the hardware cursor.
    pub fn set_show_hardware_cursor(&mut self, value: bool) {
        self.set_global("showHardwareCursor", value);
    }

    /// Returns the editor's horizontal padding: `NaN` while a `NaN` setter result
    /// is not superseded, otherwise the stored number, or 0 when it is unset or not
    /// a number.
    #[must_use]
    pub fn get_editor_padding_x(&self) -> f64 {
        if self.effective_nan.editor_padding_x {
            return f64::NAN;
        }
        self.effective
            .get("editorPaddingX")
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
    }

    /// Sets the editor's horizontal padding, rounded down into 0 to 3; `NaN` is
    /// kept as the typed result and stored as `null`.
    pub fn set_editor_padding_x(&mut self, value: f64) {
        let padding = clamp_floor(value, 0.0, 3.0);
        self.accepted_nan.editor_padding_x = padding.is_nan();
        self.set_global("editorPaddingX", number(padding));
    }

    /// Returns the visible autocomplete rows: `NaN` while a `NaN` setter result is
    /// not superseded, otherwise the stored number, or 5 when it is unset or not a
    /// number.
    #[must_use]
    pub fn get_autocomplete_max_visible(&self) -> f64 {
        if self.effective_nan.autocomplete_max_visible {
            return f64::NAN;
        }
        self.effective
            .get("autocompleteMaxVisible")
            .and_then(Value::as_f64)
            .unwrap_or(5.0)
    }

    /// Sets the visible autocomplete rows, rounded down into 3 to 20; `NaN` is kept
    /// as the typed result and stored as `null`.
    pub fn set_autocomplete_max_visible(&mut self, value: f64) {
        let rows = clamp_floor(value, 3.0, 20.0);
        self.accepted_nan.autocomplete_max_visible = rows.is_nan();
        self.set_global("autocompleteMaxVisible", number(rows));
    }

    /// Returns the code block indentation (two spaces when unset or not a string).
    #[must_use]
    pub fn get_code_block_indent(&self) -> String {
        self.nested("markdown", "codeBlockIndent")
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

/// Joins `suffix` onto `home` with the platform's separators: `.` and `..` fold
/// lexically in the whole path and a trailing separator is kept.
fn join_normalized(home: &Path, suffix: &str) -> PathBuf {
    let relative: PathBuf = Path::new(suffix)
        .components()
        .filter(|part| matches!(part, Component::Normal(_) | Component::ParentDir))
        .collect();
    let mut path = normalized(&home.join(relative));
    if suffix.ends_with(std::path::is_separator) {
        path.push("");
    }
    path
}

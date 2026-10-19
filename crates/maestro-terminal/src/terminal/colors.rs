//! Terminal palette defaults and effective dynamic colours.

use super::Terminal;
use crate::color::{Palette, Rgb};

impl Terminal {
    /// Replace the foreground default while retaining its explicit override.
    pub fn set_default_foreground_color(&mut self, color: Option<Rgb>) {
        self.colors.foreground.default = color;
        self.flags.dirty.palette = true;
    }

    /// Replace the background default while retaining its explicit override.
    pub fn set_default_background_color(&mut self, color: Option<Rgb>) {
        self.colors.background.default = color;
        self.flags.dirty.palette = true;
    }

    /// Replace the cursor default while retaining its explicit override.
    pub fn set_default_cursor_color(&mut self, color: Option<Rgb>) {
        self.colors.cursor.default = color;
        self.flags.dirty.palette = true;
    }

    /// Replace palette defaults while retaining explicit index overrides.
    pub fn set_default_palette(&mut self, palette: &Palette) {
        self.colors.palette.change_default(*palette);
        self.flags.dirty.palette = true;
    }

    /// Palette defaults without explicit index overrides.
    #[must_use]
    pub fn default_palette(&self) -> Palette {
        self.colors.palette.original
    }

    /// Foreground override, or its independent default when no override exists.
    #[must_use]
    pub fn effective_foreground_color(&self) -> Option<Rgb> {
        self.colors.foreground.get()
    }

    /// Cursor override, or its independent default when no override exists.
    #[must_use]
    pub fn effective_cursor_color(&self) -> Option<Rgb> {
        self.colors.cursor.get()
    }
}

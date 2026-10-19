//! Cursor attributes and page-local style ownership.

use super::Screen;
use crate::{
    page_list::IncreaseCapacity,
    sgr::Attribute,
    style::{Color, Style},
};

impl Screen {
    /// Apply a decoded rendition attribute, retaining the old style on failure.
    ///
    /// # Errors
    /// Returns a page capacity error when the style cannot be admitted.
    pub fn set_attribute(
        &mut self,
        attribute: Attribute<'_>,
    ) -> Result<(), crate::page_list::IncreaseCapacityError> {
        let old = self.cursor.style;
        apply(&mut self.cursor.style, attribute);
        if self.cursor.style == old {
            return Ok(());
        }
        if let Err(error) = self.manual_style_update() {
            self.cursor.style = old;
            if self.manual_style_update().is_err() {
                self.cursor.style = Style::default();
                self.cursor.style_id = 0;
            }
            return Err(error);
        }
        Ok(())
    }

    /// Reconcile a directly edited style with its cursor-page reference.
    ///
    /// # Errors
    /// Returns a capacity error after growth and splitting cannot admit the style.
    /// The style ID is cleared on failure; the concrete style remains available.
    pub fn manual_style_update(&mut self) -> Result<(), crate::page_list::IncreaseCapacityError> {
        use crate::page_list::IncreaseCapacityError as Error;
        let pin = self.cursor_pin()?;
        let page = self.pages.page_mut(pin.node).ok_or(Error::InvalidNode)?;
        if self.cursor.style_id != 0 {
            page.styles.release_multiple(self.cursor.style_id, 1);
        }
        self.cursor.style_id = 0;
        if self.cursor.style.is_default() {
            return Ok(());
        }
        if let Ok(id) = page.styles.add(self.cursor.style) {
            self.cursor.style_id = id;
            return Ok(());
        }
        match self.increase_capacity(pin.node, Some(IncreaseCapacity::Styles)) {
            Ok(_) => {}
            Err(Error::OutOfSpace) => self.split_for_capacity(self.cursor_pin()?)?,
            Err(error) => return Err(error),
        }
        let pin = self.cursor_pin()?;
        self.cursor.style_id = self
            .pages
            .page_mut(pin.node)
            .ok_or(Error::InvalidNode)?
            .styles
            .add(self.cursor.style)
            .map_err(|_| Error::OutOfSpace)?;
        Ok(())
    }
}

/// Update only the fields owned by the decoded attribute.
fn apply(style: &mut Style, attribute: Attribute<'_>) {
    use Attribute as A;
    match attribute {
        A::Unset => *style = Style::default(),
        A::Bold => style.flags.set_bold(true),
        A::ResetBold => {
            style.flags.set_bold(false);
            style.flags.set_faint(false);
        }
        A::Italic => style.flags.set_italic(true),
        A::ResetItalic => style.flags.set_italic(false),
        A::Faint => style.flags.set_faint(true),
        A::Underline(value) => style.flags.set_underline(value),
        A::UnderlineColor(value) => style.underline_color = Color::Rgb(value),
        A::Palette256UnderlineColor(value) => style.underline_color = Color::Palette(value),
        A::ResetUnderlineColor => style.underline_color = Color::None,
        A::Overline => style.flags.set_overline(true),
        A::ResetOverline => style.flags.set_overline(false),
        A::Blink => style.flags.set_blink(true),
        A::ResetBlink => style.flags.set_blink(false),
        A::Inverse => style.flags.set_inverse(true),
        A::ResetInverse => style.flags.set_inverse(false),
        A::Invisible => style.flags.set_invisible(true),
        A::ResetInvisible => style.flags.set_invisible(false),
        A::Strikethrough => style.flags.set_strikethrough(true),
        A::ResetStrikethrough => style.flags.set_strikethrough(false),
        A::DirectColorFg(value) => style.fg_color = Color::Rgb(value),
        A::DirectColorBg(value) => style.bg_color = Color::Rgb(value),
        A::Color8Fg(value) | A::Color8BrightFg(value) => {
            style.fg_color = Color::Palette(value.0);
        }
        A::Color8Bg(value) | A::Color8BrightBg(value) => {
            style.bg_color = Color::Palette(value.0);
        }
        A::Palette256Fg(value) => style.fg_color = Color::Palette(value),
        A::Palette256Bg(value) => style.bg_color = Color::Palette(value),
        A::ResetFg => style.fg_color = Color::None,
        A::ResetBg => style.bg_color = Color::None,
        A::Unknown(_) => {}
    }
}

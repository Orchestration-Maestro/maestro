//! Word-wrapped text with padding and an optional background.

use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::background::{BackgroundFn, fill_row};
use crate::text::utils::{
    TruncateOptions, is_whitespace_scalar, truncate_to_width, visible_width, wrap_text_with_ansi,
};
use crate::tui::Component;

/// Wrapped multi-line text between horizontal and vertical padding.
pub struct Text {
    /// Text to wrap; whitespace-only text renders nothing.
    content: RefCell<Rc<str>>,
    /// Columns of padding on each side, reduced when the viewport is narrow.
    padding_x: usize,
    /// Blank rows above and below the text.
    padding_y: usize,
    /// Styles every row, padding rows included.
    custom_bg_fn: RefCell<Option<BackgroundFn>>,
    /// Rows of the last render, kept until the text, background or width changes.
    cache: RefCell<Option<Rendered>>,
    /// Count of invalidations, so a render can tell that one happened while it ran.
    generation: Cell<u64>,
}

/// Rows kept from a render.
struct Rendered {
    /// Viewport width the rows were laid out for.
    width: usize,
    /// The rows.
    lines: Vec<String>,
}

/// The whole graphemes of `line` that fit `width` cells; a row that already fits is untouched.
fn clip(line: &str, width: usize) -> Cow<'_, str> {
    if visible_width(line) <= width {
        return Cow::Borrowed(line);
    }
    let options = TruncateOptions {
        ellipsis: "",
        pad: false,
    };
    Cow::Owned(truncate_to_width(line, width, options))
}

impl Text {
    /// Creates a component for `text` with padding and an optional row background.
    #[must_use]
    pub fn new(
        text: String,
        padding_x: usize,
        padding_y: usize,
        custom_bg_fn: Option<BackgroundFn>,
    ) -> Self {
        Self {
            content: RefCell::new(text.into()),
            padding_x,
            padding_y,
            custom_bg_fn: RefCell::new(custom_bg_fn),
            cache: RefCell::new(None),
            generation: Cell::new(0),
        }
    }

    /// Replaces the text and drops the cached rows, also when the text is equal.
    pub fn set_text(&self, text: String) {
        *self.content.borrow_mut() = text.into();
        self.invalidate();
    }

    /// Replaces or removes the row background and drops the cached rows.
    pub fn set_custom_bg_fn(&self, custom_bg_fn: Option<BackgroundFn>) {
        let replaced = self.custom_bg_fn.replace(custom_bg_fn);
        self.invalidate();
        drop(replaced);
    }

    /// The background in effect now, cloned so that no borrow is held while it runs.
    fn background(&self) -> Option<BackgroundFn> {
        self.custom_bg_fn.borrow().clone()
    }

    /// One row padded to `width` and styled with the current background.
    fn row(&self, line: &str, width: usize) -> String {
        fill_row(line, width, self.background().as_deref())
    }

    /// Lays `text` out for `width`: wrapped rows between the padding rows.
    fn layout(&self, text: &str, width: usize) -> Vec<String> {
        let padding = self.padding_x.min(width / 2);
        let margin = " ".repeat(padding);
        let normalized = text.replace('\t', "   ");
        let content_width = width - 2 * padding;
        let content: Vec<String> = wrap_text_with_ansi(&normalized, content_width.max(1))
            .iter()
            .map(|line| {
                let shown = clip(line, content_width);
                self.row(&format!("{margin}{shown}{margin}"), width)
            })
            .collect();
        let blank: Vec<String> = (0..self.padding_y)
            .map(|_| self.row(&" ".repeat(width), width))
            .collect();
        [blank.clone(), content, blank].concat()
    }
}

impl Default for Text {
    /// Empty text with one column and one row of padding and no background.
    fn default() -> Self {
        Self::new(String::new(), 1, 1, None)
    }
}

impl Component for Text {
    /// Renders the wrapped text, or nothing when it has no non-whitespace scalar. Rows are
    /// reused until the text, the background or the width changes, or the component is
    /// invalidated; a setter or invalidation called while rendering keeps this render's rows
    /// out of the cache.
    fn render(&self, width: usize) -> Vec<String> {
        if let Some(rendered) = &*self.cache.borrow()
            && rendered.width == width
        {
            return rendered.lines.clone();
        }
        let generation = self.generation.get();
        let text = Rc::clone(&self.content.borrow());
        if text.chars().all(is_whitespace_scalar) {
            return Vec::new();
        }
        let lines = self.layout(&text, width);
        if self.generation.get() == generation {
            *self.cache.borrow_mut() = Some(Rendered {
                width,
                lines: lines.clone(),
            });
        }
        lines
    }

    fn invalidate(&self) {
        self.generation.set(self.generation.get().wrapping_add(1));
        self.cache.take();
    }
}

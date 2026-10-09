//! A container that pads its children and paints a background behind them.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::background::{BackgroundFn, fill_row};
use crate::tui::{ChildArray, Component, ComponentHandle, Container};

/// Text handed to the background to detect that it now styles differently.
const BACKGROUND_SAMPLE: &str = "test";

/// Children laid out inside horizontal and vertical padding, over an optional background.
///
/// Every render renders the children again; when they produce rows, it samples any background.
/// The composed rows are reused only when the width, the children's rows and the sample are
/// all unchanged. Children are held in a shared array: whoever keeps the handle from
/// [`Box::children`] edits the array the box renders, and a render in progress stays on the
/// array it started on when [`Box::clear`] or [`Box::set_children`] installs another.
pub struct Box {
    /// The children, in render order.
    children: Container,
    /// Columns of padding on the left of every child row, reduced when the viewport is narrow.
    padding_x: usize,
    /// Blank rows above and below the children.
    padding_y: usize,
    /// Styles every row, padding rows included.
    bg_fn: RefCell<Option<BackgroundFn>>,
    /// The last composed rows and the inputs they were composed from.
    cache: RefCell<Option<Rendered>>,
    /// Count of cache drops, so a render can tell that the box changed while it ran.
    generation: Cell<u64>,
}

/// Composed rows with the inputs that produced them.
struct Rendered {
    /// Viewport width the rows were composed for.
    width: usize,
    /// The children's rows with their left padding.
    child_lines: Vec<String>,
    /// What the background returned for the sample text, if there was one.
    bg_sample: Option<String>,
    /// The composed rows.
    lines: Vec<String>,
}

impl Box {
    /// Creates a box with the given padding and optional row background.
    #[must_use]
    pub fn new(padding_x: usize, padding_y: usize, bg_fn: Option<BackgroundFn>) -> Self {
        Self {
            children: Container::new(),
            padding_x,
            padding_y,
            bg_fn: RefCell::new(bg_fn),
            cache: RefCell::new(None),
            generation: Cell::new(0),
        }
    }

    /// The array of children in render order, shared with the box.
    #[must_use]
    pub fn children(&self) -> ChildArray {
        self.children.children()
    }

    /// Makes `children` the array the box renders. The previous array is left to its holders.
    pub fn set_children(&self, children: ChildArray) {
        self.children.set_children(children);
    }

    /// Appends a child and drops the composed rows.
    pub fn add_child(&self, component: ComponentHandle) {
        self.children.add_child(component);
        self.drop_cache();
    }

    /// Removes the first occurrence of `component` and drops the composed rows; a missing
    /// child is ignored and keeps them.
    pub fn remove_child(&self, component: &ComponentHandle) {
        let present = self
            .children()
            .borrow()
            .iter()
            .any(|child| Rc::ptr_eq(child, component));
        if present {
            self.children.remove_child(component);
            self.drop_cache();
        }
    }

    /// Replaces the children with a new empty array and drops the composed rows.
    pub fn clear(&self) {
        self.children.clear();
        self.drop_cache();
    }

    /// Replaces or removes the row background and drops the composed rows.
    pub fn set_bg_fn(&self, bg_fn: Option<BackgroundFn>) {
        let replaced = self.bg_fn.replace(bg_fn);
        self.drop_cache();
        drop(replaced);
    }

    /// Drops the composed rows and keeps a render that is running from caching its rows.
    fn drop_cache(&self) {
        self.generation.set(self.generation.get().wrapping_add(1));
        self.cache.take();
    }

    /// The background in effect now, cloned so that no borrow is held while it runs.
    fn background(&self) -> Option<BackgroundFn> {
        self.bg_fn.borrow().clone()
    }

    /// One row padded to `width` and styled with the background in effect now.
    fn row(&self, line: &str, width: usize) -> String {
        fill_row(line, width, self.background().as_deref())
    }

    /// The padding rows, the child rows and the padding rows again, each styled in turn.
    fn compose(&self, child_lines: &[String], width: usize) -> Vec<String> {
        let mut lines = Vec::with_capacity(child_lines.len() + 2 * self.padding_y);
        lines.extend((0..self.padding_y).map(|_| self.row("", width)));
        lines.extend(child_lines.iter().map(|line| self.row(line, width)));
        lines.extend((0..self.padding_y).map(|_| self.row("", width)));
        lines
    }
}

impl Default for Box {
    /// A box with one column and one row of padding and no background.
    fn default() -> Self {
        Self::new(1, 1, None)
    }
}

impl Component for Box {
    /// Renders nothing when no child produces a row. Otherwise the children render again,
    /// the background, if any, is sampled, and the composed rows are reused when width,
    /// child rows and sample are unchanged.
    fn render(&self, width: usize) -> Vec<String> {
        let generation = self.generation.get();
        let padding = self.padding_x.min(width / 2);
        let margin = " ".repeat(padding);
        let child_lines: Vec<String> = self
            .children
            .render(width - 2 * padding)
            .iter()
            .map(|line| format!("{margin}{line}"))
            .collect();
        if child_lines.is_empty() {
            return Vec::new();
        }
        let bg_sample = self
            .background()
            .map(|background| background(BACKGROUND_SAMPLE));
        if let Some(rendered) = &*self.cache.borrow()
            && rendered.width == width
            && rendered.bg_sample == bg_sample
            && rendered.child_lines == child_lines
        {
            return rendered.lines.clone();
        }
        let lines = self.compose(&child_lines, width);
        if self.generation.get() == generation {
            *self.cache.borrow_mut() = Some(Rendered {
                width,
                child_lines,
                bg_sample,
                lines: lines.clone(),
            });
        }
        lines
    }

    /// Drops the composed rows, then invalidates the children.
    fn invalidate(&self) {
        self.drop_cache();
        self.children.invalidate();
    }
}

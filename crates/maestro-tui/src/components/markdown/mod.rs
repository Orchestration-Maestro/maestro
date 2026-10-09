#![doc = include_str!("../../../../../docs/terminal/markdown.md")]

mod autolinks;
mod blocks;
mod inline;
mod lists;
mod parse;

use super::background::fill_row;
use crate::text::utils::{is_whitespace_scalar, wrap_text_with_ansi};
use crate::{Component, TerminalImage};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// Foreground or row styling callback.
type ColorFn = dyn Fn(&str) -> String;
/// Code-row highlighting callback.
type HighlightFn = dyn Fn(&str, Option<&str>) -> Vec<String>;
/// A requested base-text decoration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextDecoration {
    /// Bold text.
    Bold,
    /// Italic text.
    Italic,
    /// Struck-through text.
    Strikethrough,
    /// Underlined text.
    Underline,
}

/// Base foreground and decorations; background is applied during final layout.
#[derive(Default)]
pub struct DefaultTextStyle {
    /// Optional foreground callback.
    pub color: Option<Box<ColorFn>>,
    /// Optional row background callback.
    pub bg_color: Option<Box<ColorFn>>,
    /// Decorations applied in bold, italic, strikethrough, underline order.
    pub decorations: Vec<TextDecoration>,
}

/// Caller-supplied formatting for terminal Markdown elements.
pub struct MarkdownTheme {
    /// Heading foreground.
    pub heading: Box<dyn Fn(&str) -> String>,
    /// Link label.
    pub link: Box<dyn Fn(&str) -> String>,
    /// Printed link destination.
    pub link_url: Box<dyn Fn(&str) -> String>,
    /// Inline code.
    pub code: Box<dyn Fn(&str) -> String>,
    /// Unhighlighted code line.
    pub code_block: Box<dyn Fn(&str) -> String>,
    /// Code fence.
    pub code_block_border: Box<dyn Fn(&str) -> String>,
    /// Quote foreground.
    pub quote: Box<dyn Fn(&str) -> String>,
    /// Quote border.
    pub quote_border: Box<dyn Fn(&str) -> String>,
    /// Horizontal rule.
    pub hr: Box<dyn Fn(&str) -> String>,
    /// List marker.
    pub list_bullet: Box<dyn Fn(&str) -> String>,
    /// Bold decoration.
    pub bold: Box<dyn Fn(&str) -> String>,
    /// Italic decoration.
    pub italic: Box<dyn Fn(&str) -> String>,
    /// Strikethrough decoration.
    pub strikethrough: Box<dyn Fn(&str) -> String>,
    /// Underline decoration.
    pub underline: Box<dyn Fn(&str) -> String>,
    /// Returns code rows for the complete code text and optional full information string.
    pub highlight_code: Option<Box<HighlightFn>>,
    /// Code-row prefix; absence selects two spaces.
    pub code_block_indent: Option<String>,
}

/// Padding and base style supplied to a Markdown component.
pub struct MarkdownOptions {
    /// Requested margin on each side.
    pub padding_x: usize,
    /// Blank rows above and below nonempty content.
    pub padding_y: usize,
    /// Optional base styling.
    pub default_text_style: Option<DefaultTextStyle>,
}

/// Retained terminal Markdown content.
pub struct Markdown {
    /// Current text.
    text: RefCell<Rc<str>>,
    /// Caller layout and styling.
    options: MarkdownOptions,
    /// Shared immutable callback record.
    theme: Rc<MarkdownTheme>,
    /// Terminal capability state.
    terminal_image: TerminalImage,
    /// Finalized rows and their width.
    cache: RefCell<Option<(usize, Vec<String>)>>,
    /// Invalidations observed during callbacks.
    generation: Cell<u64>,
}

/// Callback context for one layout; no retained component state is borrowed.
struct Renderer<'a> {
    /// Supplied styles.
    theme: &'a MarkdownTheme,
    /// Optional message styling.
    default_style: Option<&'a DefaultTextStyle>,
    /// Shared terminal capabilities.
    terminal_image: &'a TerminalImage,
    /// Lazily sampled prefix shared by this layout's message spans.
    default_prefix: std::cell::OnceCell<String>,
}

impl Markdown {
    /// Retains text, layout options, callbacks and terminal capability state.
    #[must_use]
    pub fn new(
        text: String,
        options: MarkdownOptions,
        theme: Rc<MarkdownTheme>,
        terminal_image: TerminalImage,
    ) -> Self {
        Self {
            text: RefCell::new(text.into()),
            options,
            theme,
            terminal_image,
            cache: RefCell::new(None),
            generation: Cell::new(0),
        }
    }
    /// Replaces text and invalidates cached rows, including when the text is equal.
    pub fn set_text(&self, text: String) {
        *self.text.borrow_mut() = text.into();
        self.invalidate();
    }
    /// Wraps rows before applying requested margins and background.
    fn layout(&self, text: &str, width: usize) -> Vec<String> {
        let margin = " ".repeat(self.options.padding_x);
        let content_width = width
            .saturating_sub(self.options.padding_x.saturating_mul(2))
            .max(1);
        let background = self
            .options
            .default_text_style
            .as_ref()
            .and_then(|style| style.bg_color.as_deref());
        let renderer = Renderer {
            theme: &self.theme,
            default_style: self.options.default_text_style.as_ref(),
            terminal_image: &self.terminal_image,
            default_prefix: std::cell::OnceCell::new(),
        };
        let content: Vec<String> = renderer
            .blocks(&parse::parse(text), content_width, inline::Style::Default)
            .into_iter()
            .flat_map(|line| {
                if crate::is_image_line(&line) {
                    vec![line]
                } else {
                    wrap_text_with_ansi(&line, content_width)
                }
            })
            .map(|row| {
                if crate::is_image_line(&row) {
                    row
                } else {
                    fill_row(&format!("{margin}{row}{margin}"), width, background)
                }
            })
            .collect();
        let blank: Vec<String> = (0..self.options.padding_y)
            .map(|_| fill_row("", width, background))
            .collect();
        let mut lines = blank.clone();
        lines.extend(content);
        lines.extend(blank);
        if lines.is_empty() {
            lines.push(String::new());
        }
        lines
    }
}
impl Component for Markdown {
    fn render(&self, width: usize) -> Vec<String> {
        if let Some((cached_width, lines)) = &*self.cache.borrow()
            && *cached_width == width
        {
            return lines.clone();
        }
        let generation = self.generation.get();
        let text = Rc::clone(&self.text.borrow());
        let lines = if text.chars().all(is_whitespace_scalar) {
            Vec::new()
        } else {
            self.layout(&text, width)
        };
        if generation == self.generation.get() {
            *self.cache.borrow_mut() = Some((width, lines.clone()));
        }
        lines
    }
    fn invalidate(&self) {
        self.generation.set(self.generation.get().wrapping_add(1));
        self.cache.take();
    }
}

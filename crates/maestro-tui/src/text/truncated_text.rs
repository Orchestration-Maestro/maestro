//! Retained first-line text.
use crate::{Component, text::utils::truncate_to_width};

/// Single-line text that truncates to fit viewport width.
pub struct TruncatedText {
    text: String,
    padding_x: usize,
    padding_y: usize,
}
impl TruncatedText {
    /// Retain text and optional padding, defaulting each padding to zero.
    #[must_use]
    pub fn new(text: String, padding_x: Option<usize>, padding_y: Option<usize>) -> Self {
        Self {
            text,
            padding_x: padding_x.unwrap_or(0),
            padding_y: padding_y.unwrap_or(0),
        }
    }
}
impl Component for TruncatedText {
    fn invalidate(&mut self) {}
    fn render(&mut self, width: usize) -> Vec<String> {
        let left = self.padding_x.min(width);
        let right = self.padding_x.min(width - left);
        let first_line = self.text.split('\n').next().unwrap_or_default();
        let content = truncate_to_width(first_line, width - left - right, None, Some(true));
        let line = " ".repeat(left) + &content + &" ".repeat(right);
        let padding = std::iter::repeat_n(" ".repeat(width), self.padding_y);
        padding
            .clone()
            .chain(std::iter::once(line))
            .chain(padding)
            .collect()
    }
}

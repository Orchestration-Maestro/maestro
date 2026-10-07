//! Retained first-line text.
use crate::{
    Component,
    text::utils::{truncate_to_width, visible_width},
};
/// Single-line text that truncates to fit viewport width.
pub struct TruncatedText {
    text: String,
    padding_x: usize,
    padding_y: usize,
}
impl TruncatedText {
    /// Retain text and optional padding, defaulting each padding to zero.
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
        let empty = " ".repeat(width);
        let mut result = Vec::new();
        result.extend(std::iter::repeat_n(empty.clone(), self.padding_y));
        let left_padding = self.padding_x.min(width);
        let right_padding = self.padding_x.min(width - left_padding);
        let available = width - left_padding - right_padding;
        let text = self.text.split('\n').next().unwrap_or("");
        let display = truncate_to_width(text, available, None, None);
        let line = " ".repeat(left_padding) + &display + &" ".repeat(right_padding);
        let padding = " ".repeat(width.saturating_sub(visible_width(&line)));
        result.push(line + &padding);
        result.extend(std::iter::repeat_n(empty.clone(), self.padding_y));
        result
    }
}

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
        let mut i = 0;
        while i < self.padding_y {
            result.push(empty.clone());
            i += 1;
        }
        let available = width.saturating_sub(self.padding_x * 2).max(1);
        let text = self.text.split('\n').next().unwrap_or("");
        let display = truncate_to_width(text, available, None, None);
        let line = " ".repeat(self.padding_x) + &display + &" ".repeat(self.padding_x);
        let padding = " ".repeat(width.saturating_sub(visible_width(&line)));
        result.push(line + &padding);
        let mut i = 0;
        while i < self.padding_y {
            result.push(empty.clone());
            i += 1;
        }
        result
    }
}

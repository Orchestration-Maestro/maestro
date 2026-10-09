//! Row filling shared by the padded widgets.

use std::rc::Rc;

use crate::text::utils::{apply_background_to_line, visible_width};

/// A caller-supplied function that styles one row, for example with a background colour.
pub(super) type BackgroundFn = Rc<dyn Fn(&str) -> String>;

/// Pads `line` with spaces up to `width` cells (a wider line is left as it is), then styles
/// the result with `background` when one is given.
pub(super) fn fill_row(
    line: &str,
    width: usize,
    background: Option<&dyn Fn(&str) -> String>,
) -> String {
    match background {
        Some(background) => apply_background_to_line(line, width, background),
        None => format!(
            "{line}{}",
            " ".repeat(width.saturating_sub(visible_width(line)))
        ),
    }
}

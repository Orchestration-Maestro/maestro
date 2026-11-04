//! Completion styles for demonstration callers.
use super::color::{Color, color, decorate};
use maestro_tui::SelectListTheme;
use std::{io, rc::Rc};
/// Completion styles retain selection emphasis and secondary dim treatment.
pub(super) fn default_select_list_theme() -> io::Result<SelectListTheme> {
    let accent = color("accent")?;
    let selected = Rc::clone(&accent);
    let muted = color("muted")?;
    let dim: Color = Rc::new(move |text| decorate(&muted(text), "\x1b[2m", "\x1b[22m"));
    Ok(SelectListTheme {
        selected_prefix: accent,
        selected_text: Rc::new(move |text| decorate(&selected(text), "\x1b[1m", "\x1b[22m")),
        description: Rc::clone(&dim),
        scroll_info: Rc::clone(&dim),
        no_match: dim,
    })
}

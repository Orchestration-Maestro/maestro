//! Editor styles for demonstration callers.
use super::{
    color::{color, decorate},
    default_select_list_theme::default_select_list_theme,
};
use maestro_tui::EditorTheme;
use std::{io, rc::Rc};
/// Editor border and completion styles for the dark demonstration surface.
pub(super) fn default_editor_theme() -> io::Result<EditorTheme> {
    let border = color("border")?;
    Ok(EditorTheme {
        border_color: Rc::new(move |text| decorate(&border(text), "\x1b[2m", "\x1b[22m")),
        select_list: default_select_list_theme()?,
    })
}

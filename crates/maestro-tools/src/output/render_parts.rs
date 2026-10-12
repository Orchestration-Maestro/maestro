//! Building blocks shared by the native tool renderers.

use crate::{
    keybinding_hints::{EXPAND, key_hint},
    utils::diagnostics::failure,
};
use maestro_models::DiagnosticErrorInfo;
use maestro_theme::{LiveTheme, Theme, ThemeColor, ThemeError};
use maestro_tui::{Text, tui::ComponentHandle};
use std::{any::Any, rc::Rc};

/// Format a line number or count the way the model has always seen it.
pub(crate) fn num(value: f64) -> String {
    ryu_js::Buffer::new().format(value).to_owned()
}

/// Convert a length to a float, which holds every realistic length exactly.
pub(crate) fn float(len: usize) -> f64 {
    num_traits::ToPrimitive::to_f64(&len).unwrap_or(f64::MAX)
}

/// Carry a theme failure to the renderer's caller.
pub(crate) fn themed<T>(result: Result<T, ThemeError>) -> Result<T, DiagnosticErrorInfo> {
    result.map_err(|error| failure(&error.to_string()))
}

/// `text` in `color`.
pub(crate) fn paint(
    theme: &Theme,
    color: &ThemeColor,
    text: &str,
) -> Result<String, DiagnosticErrorInfo> {
    themed(theme.fg(color, text))
}

/// Put `text` in the previous row component when it is a `Text`, otherwise in a new one.
pub(crate) fn into_text(previous: Option<&ComponentHandle>, text: String) -> ComponentHandle {
    if let Some(handle) = previous {
        let component: &dyn Any = &**handle;
        if let Some(existing) = component.downcast_ref::<Text>() {
            existing.set_text(text);
            return Rc::clone(handle);
        }
    }
    Rc::new(Text::new(text, 0, 0, None))
}

/// The muted count of rows left out of a preview, followed by the expand key hint.
pub(crate) fn expand_hint(
    remaining: usize,
    theme: &Theme,
    live_theme: &LiveTheme,
) -> Result<String, DiagnosticErrorInfo> {
    let more = paint(
        theme,
        &ThemeColor::Muted,
        &format!("\n... ({remaining} more lines,"),
    )?;
    let hint = themed(key_hint(EXPAND, "to expand", live_theme))?;
    Ok(format!("{more} {hint})"))
}

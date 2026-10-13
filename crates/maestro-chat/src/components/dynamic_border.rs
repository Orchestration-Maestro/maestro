//! A rule recolored at each render.
use maestro_theme::{LiveTheme, ThemeColor, paint};
use maestro_tui::{Component, editor_component::BorderColor};
use std::rc::Rc;

/// A viewport-wide rule with a live or supplied painter.
pub struct DynamicBorder {
    /// Retained painter, invoked once per render.
    color: BorderColor,
}
impl DynamicBorder {
    /// Uses the live border color unless a painter is supplied.
    #[must_use]
    pub fn new(theme: LiveTheme, color: Option<BorderColor>) -> Self {
        let color = color.unwrap_or_else(|| {
            Rc::new(move |text| paint(theme.get().ok().as_deref(), &ThemeColor::Border, text))
        });
        Self { color }
    }
}
impl Component for DynamicBorder {
    fn render(&self, width: usize) -> Vec<String> {
        vec![(self.color)(&"─".repeat(width.max(1)))]
    }
}

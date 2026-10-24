//! Supplied thinking levels in a bordered selection list.
use super::bordered_select_list;
use maestro_app::ThinkingLevel;
use maestro_theme::LiveTheme;
use maestro_tui::{Component, Container, SelectItem, SelectList};
use std::rc::Rc;

/// A thinking-level picker retaining its focusable list.
pub struct ThinkingSelectorComponent {
    /// Shared child-array owner.
    container: Container,
    /// Focus target retained independently of the chrome.
    select_list: Rc<SelectList>,
}
impl ThinkingSelectorComponent {
    /// Keeps supplied order and duplicates, selecting the first current match.
    #[must_use]
    pub fn new(
        current_level: ThinkingLevel,
        available_levels: Vec<ThinkingLevel>,
        on_select: Rc<dyn Fn(ThinkingLevel)>,
        on_cancel: Rc<dyn Fn()>,
        theme: LiveTheme,
    ) -> Self {
        let current = available_levels
            .iter()
            .position(move |level| *level == current_level);
        let items: Vec<_> = available_levels
            .into_iter()
            .map(|level| thinking_item(&level))
            .collect();
        let count = items.len();
        let (container, select_list) = bordered_select_list(items, count, theme);
        if let Some(index) = current.and_then(|index| isize::try_from(index).ok()) {
            select_list.set_selected_index(index);
        }
        select_list.set_on_select(Some(Rc::new(move |item| {
            if let Ok(level) = item.value.parse() {
                on_select(level);
            }
        })));
        select_list.set_on_cancel(Some(on_cancel));
        Self {
            container,
            select_list,
        }
    }
    /// Returns the same focus target held in the children.
    #[must_use]
    pub fn get_select_list(&self) -> &Rc<SelectList> {
        &self.select_list
    }
}
component_children!(ThinkingSelectorComponent, Container, container);
impl Component for ThinkingSelectorComponent {
    fn render(&self, width: usize) -> Vec<String> {
        self.container.render(width)
    }
    fn invalidate(&self) {
        self.container.invalidate();
    }
}

/// The single name and description owner for reasoning levels.
pub(crate) fn thinking_text(level: &ThinkingLevel) -> (&'static str, &'static str) {
    match level {
        ThinkingLevel::Off => ("off", "No reasoning"),
        ThinkingLevel::Minimal => ("minimal", "Very brief reasoning (~1k tokens)"),
        ThinkingLevel::Low => ("low", "Light reasoning (~2k tokens)"),
        ThinkingLevel::Medium => ("medium", "Moderate reasoning (~8k tokens)"),
        ThinkingLevel::High => ("high", "Deep reasoning (~16k tokens)"),
        ThinkingLevel::Xhigh => ("xhigh", "Maximum reasoning (~32k tokens)"),
    }
}
/// Builds one reasoning choice from the shared text mapping.
pub(super) fn thinking_item(level: &ThinkingLevel) -> SelectItem {
    let (name, description) = thinking_text(level);
    SelectItem {
        value: name.into(),
        label: name.into(),
        description: Some(description.into()),
    }
}

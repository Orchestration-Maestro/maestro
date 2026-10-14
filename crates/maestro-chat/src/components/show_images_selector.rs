//! Inline-image display choices over the shared list owner.
use super::bordered_select_list;
use maestro_theme::LiveTheme;
use maestro_tui::{Component, Container, SelectItem, SelectList};
use std::rc::Rc;

/// A bordered Yes/No picker for inline images.
pub struct ShowImagesSelectorComponent {
    /// Child-array owner.
    container: Container,
    /// Retained focus target.
    select_list: Rc<SelectList>,
}
impl ShowImagesSelectorComponent {
    /// Selects the supplied bool, reporting the confirmed choice.
    #[must_use]
    pub fn new(
        current_value: bool,
        on_select: Rc<dyn Fn(bool)>,
        on_cancel: Rc<dyn Fn()>,
        theme: LiveTheme,
    ) -> Self {
        let items = [
            ("yes", "Yes", "Show images inline in terminal"),
            ("no", "No", "Show text placeholder instead"),
        ]
        .into_iter()
        .map(|(value, label, description)| SelectItem {
            value: value.into(),
            label: label.into(),
            description: Some(description.into()),
        })
        .collect();
        let (container, select_list) = bordered_select_list(items, 5, theme);
        select_list.set_selected_index(isize::from(!current_value));
        select_list.set_on_select(Some(Rc::new(move |item| on_select(item.value == "yes"))));
        select_list.set_on_cancel(Some(on_cancel));
        Self {
            container,
            select_list,
        }
    }
    /// Returns the retained focus target.
    #[must_use]
    pub fn get_select_list(&self) -> &Rc<SelectList> {
        &self.select_list
    }
}
component_children!(ShowImagesSelectorComponent, Container, container);
impl Component for ShowImagesSelectorComponent {
    fn render(&self, width: usize) -> Vec<String> {
        self.container.render(width)
    }
    fn invalidate(&self) {
        self.container.invalidate();
    }
}

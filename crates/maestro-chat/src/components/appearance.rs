//! Theme inventory selection with navigation-driven preview.
use super::bordered_select_list;
use maestro_theme::{ThemeError, ThemeState};
use maestro_tui::{Component, Container, SelectItem, SelectList};
use std::rc::Rc;

/// A bordered theme picker whose caller owns preview restoration.
pub struct ThemeSelectorComponent {
    /// Shared child-array owner.
    container: Container,
    /// Retained focus target.
    select_list: Rc<SelectList>,
}
impl ThemeSelectorComponent {
    /// Discovers names through the supplied registry and previews on navigation.
    ///
    /// # Errors
    /// Returns the registry's discovery failure unchanged.
    pub fn new(
        current_theme: &str,
        on_select: Rc<dyn Fn(&str)>,
        on_cancel: Rc<dyn Fn()>,
        on_preview: Rc<dyn Fn(&str)>,
        themes: &ThemeState,
    ) -> Result<Self, ThemeError> {
        let names = themes.get_available_themes()?;
        let current = names.iter().position(|name| name == current_theme);
        let items = names
            .into_iter()
            .map(|name| SelectItem {
                description: (name == current_theme).then(|| "(current)".into()),
                label: name.clone(),
                value: name,
            })
            .collect();
        let (container, select_list) = bordered_select_list(items, 10, themes.theme());
        if let Some(index) = current.and_then(|index| isize::try_from(index).ok()) {
            select_list.set_selected_index(index);
        }
        select_list.set_on_select(Some(Rc::new(move |item| on_select(&item.value))));
        select_list.set_on_cancel(Some(on_cancel));
        select_list.set_on_selection_change(Some(Rc::new(move |item| on_preview(&item.value))));
        Ok(Self {
            container,
            select_list,
        })
    }
    /// Returns the retained focus target.
    #[must_use]
    pub fn get_select_list(&self) -> &Rc<SelectList> {
        &self.select_list
    }
}
component_children!(ThemeSelectorComponent, Container, container);
impl Component for ThemeSelectorComponent {
    fn render(&self, width: usize) -> Vec<String> {
        self.container.render(width)
    }
    fn invalidate(&self) {
        self.container.invalidate();
    }
}

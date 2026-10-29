//! Warning edits and bounded nested option lists.
use crate::components::bounded_select_list;
use maestro_app::WarningSettings;
use maestro_theme::{LiveTheme, Theme, ThemeColor, paint};
use maestro_tui::components::settings_list::SettingsListOptions;
use maestro_tui::tui::InputHandler;
use maestro_tui::{
    Component, Container, SelectItem, SelectList, SettingItem, SettingsList, Spacer, Text,
};
use std::{cell::RefCell, rc::Rc};

/// A one-row warning editor retaining its own supplied snapshot.
pub(super) struct WarningSettingsSubmenu {
    /// Child-array owner.
    container: Container,
    /// Input target.
    list: Rc<SettingsList>,
}
impl WarningSettingsSubmenu {
    /// Commits warning state before notifying the caller.
    pub(super) fn new(
        initial: WarningSettings,
        change: Rc<dyn Fn(WarningSettings)>,
        cancel: Rc<dyn Fn()>,
        theme: LiveTheme,
    ) -> Self {
        let enabled = initial
            .0
            .get("anthropicExtraUsage")
            .filter(|value| !value.is_null())
            .is_none_or(maestro_app::is_truthy);
        let state = RefCell::new(initial);
        let item = SettingItem {
            id: "anthropic-extra-usage".into(),
            label: "Anthropic extra usage".into(),
            description: Some(
                "Warn when Anthropic subscription auth may use paid extra usage".into(),
            ),
            current_value: enabled.to_string(),
            values: Some(vec!["true".into(), "false".into()]),
            submenu: None,
        };
        let list = Rc::new(SettingsList::new(
            vec![item],
            1,
            maestro_theme::get_settings_list_theme(theme),
            (
                Rc::new(move |_, value| {
                    let snapshot = {
                        let mut state = state.borrow_mut();
                        state.set_anthropic_extra_usage(Some(value == "true"));
                        state.clone()
                    };
                    change(snapshot);
                    true
                }),
                cancel,
            ),
            SettingsListOptions::default(),
        ));
        let container = Container::new();
        container.add_child(list.clone());
        Self { container, list }
    }
}
component_children!(WarningSettingsSubmenu, Container, container);
impl Component for WarningSettingsSubmenu {
    fn render(&self, width: usize) -> Vec<String> {
        self.container.render(width)
    }
    fn invalidate(&self) {
        self.container.invalidate();
    }
    fn input_handler(&self) -> Option<&dyn InputHandler> {
        Some(self)
    }
}
impl InputHandler for WarningSettingsSubmenu {
    fn handle_input(&self, data: &str) {
        self.list.handle_input(data);
    }
}
/// Supplied nested choices with captured heading text.
pub(super) struct Selection {
    /// Heading.
    pub title: &'static str,
    /// Explanation.
    pub description: &'static str,
    /// Ordered choices.
    pub options: Vec<SelectItem>,
    /// Current value.
    pub current: String,
}
/// Immediate selection notifications.
pub(super) struct SelectionCallbacks {
    /// Confirmation.
    pub select: Rc<dyn Fn(&str)>,
    /// Cancellation.
    pub cancel: Rc<dyn Fn()>,
    /// Optional navigation preview.
    pub preview: Option<super::ThemeCallback>,
}
/// A bounded nested list with captured heading and help styling.
pub(super) struct SelectSubmenu {
    /// Child-array owner.
    container: Container,
    /// Input target.
    list: Rc<SelectList>,
}
impl SelectSubmenu {
    /// Captures chrome while list callbacks retain the live palette.
    pub(super) fn new(
        selection: Selection,
        callbacks: SelectionCallbacks,
        theme: LiveTheme,
    ) -> Self {
        let container = Container::new();
        let palette = theme.get().ok();
        let title = paint(palette.as_deref(), &ThemeColor::Accent, selection.title);
        container.add_child(Rc::new(Text::new(Theme::bold(&title, true), 0, 0, None)));
        container.add_child(Rc::new(Spacer::new(1)));
        container.add_child(Rc::new(Text::new(
            paint(
                palette.as_deref(),
                &ThemeColor::Muted,
                selection.description,
            ),
            0,
            0,
            None,
        )));
        container.add_child(Rc::new(Spacer::new(1)));
        let index = selection
            .options
            .iter()
            .position(|item| item.value == selection.current);
        let visible = selection.options.len().min(10);
        let list = bounded_select_list(selection.options, visible, theme);
        if let Some(index) = index.and_then(|index| isize::try_from(index).ok()) {
            list.set_selected_index(index);
        }
        list.set_on_select(Some(Rc::new(move |item| (callbacks.select)(&item.value))));
        list.set_on_cancel(Some(callbacks.cancel));
        if let Some(preview) = callbacks.preview {
            list.set_on_selection_change(Some(Rc::new(move |item| preview(&item.value))));
        }
        container.add_child(list.clone());
        container.add_child(Rc::new(Spacer::new(1)));
        container.add_child(Rc::new(Text::new(
            paint(
                palette.as_deref(),
                &ThemeColor::Dim,
                "  Enter to select · Esc to go back",
            ),
            0,
            0,
            None,
        )));
        Self { container, list }
    }
}
component_children!(SelectSubmenu, Container, container);
impl Component for SelectSubmenu {
    fn render(&self, width: usize) -> Vec<String> {
        self.container.render(width)
    }
    fn invalidate(&self) {
        self.container.invalidate();
    }
    fn input_handler(&self) -> Option<&dyn InputHandler> {
        Some(self)
    }
}
impl InputHandler for SelectSubmenu {
    fn handle_input(&self, data: &str) {
        self.list.handle_input(data);
    }
}

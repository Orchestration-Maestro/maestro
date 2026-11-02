//! Clamped extension selection over caller-supplied string options.
use super::{countdown_timer::CountdownTimer, dialog_chrome, dialog_countdown};
use maestro_theme::{LiveTheme, ThemeColor, ThemeError, paint};
use maestro_tui::tui::{InputHandler, TuiRuntime};
use maestro_tui::{Component, Container, TUI, Text, get_keybindings};
use std::{cell::Cell, rc::Rc};

/// Presentation and optional countdown hosts for a selection dialog.
pub struct ExtensionSelectorOptions {
    /// Writer used to request countdown frames.
    pub tui: Option<TUI>,
    /// Optional deadline in milliseconds.
    pub timeout: Option<f64>,
    /// Scheduling owner.
    pub runtime: Rc<dyn TuiRuntime>,
    /// Live presentation slot.
    pub theme: LiveTheme,
    /// Flips tool-output expansion on the tool-expand key; without it the key does nothing.
    pub on_toggle_tools_expanded: Option<Rc<dyn Fn()>>,
}
/// A selection dialog whose navigation clamps without scrolling.
pub struct ExtensionSelectorComponent {
    /// Editable chrome owner.
    container: Container,
    /// Retained list independently of chrome replacement.
    list: Rc<Container>,
    /// Supplied option order and values.
    options: Vec<String>,
    /// Position used for lookup; an empty list never notifies selection.
    selected: Cell<usize>,
    /// Selection notification.
    on_select: Rc<dyn Fn(&str)>,
    /// Cancellation notification.
    on_cancel: Rc<dyn Fn()>,
    /// Slot read when rebuilding the list.
    theme: LiveTheme,
    /// Owned countdown, if admitted.
    countdown: Option<CountdownTimer>,
    /// Flips tool-output expansion without moving focus or the selection.
    on_toggle_tools_expanded: Option<Rc<dyn Fn()>>,
}
impl ExtensionSelectorComponent {
    /// Composes a dialog retaining supplied option order and duplicate values.
    ///
    /// # Errors
    /// Returns the existing hint theme failure.
    pub fn new(
        title: String,
        options: Vec<String>,
        on_select: Rc<dyn Fn(&str)>,
        on_cancel: Rc<dyn Fn()>,
        opts: ExtensionSelectorOptions,
    ) -> Result<Self, ThemeError> {
        let ExtensionSelectorOptions {
            tui,
            timeout,
            runtime,
            theme,
            on_toggle_tools_expanded,
        } = opts;
        let list = Rc::new(Container::new());
        let hint = maestro_app::raw_key_hint("↑↓", "navigate", &theme)?
            + "  "
            + &maestro_app::key_hint("tui.select.confirm", "select", &theme)?
            + "  "
            + &maestro_app::key_hint("tui.select.cancel", "cancel", &theme)?;
        let (container, update) = dialog_chrome(title, list.clone(), theme.clone(), hint, true);
        let countdown = dialog_countdown(timeout, tui.as_ref(), runtime, update, on_cancel.clone());
        let result = Self {
            container,
            list,
            options,
            selected: Cell::new(0),
            on_select,
            on_cancel,
            theme,
            countdown,
            on_toggle_tools_expanded,
        };
        result.update_list();
        Ok(result)
    }
    /// Reconstructs option rows using the currently published colors.
    fn update_list(&self) {
        self.list.clear();
        let theme = self.theme.get().ok();
        for (index, option) in self.options.iter().enumerate() {
            let text = if index == self.selected.get() {
                paint(theme.as_deref(), &ThemeColor::Accent, "→ ")
                    + &paint(theme.as_deref(), &ThemeColor::Accent, option)
            } else {
                "  ".to_owned() + &paint(theme.as_deref(), &ThemeColor::Text, option)
            };
            self.list.add_child(Rc::new(Text::new(text, 1, 0, None)));
        }
    }
    /// Stops countdown work without notifying or disabling subsequent input.
    pub fn dispose(&self) {
        if let Some(countdown) = &self.countdown {
            countdown.dispose();
        }
    }
}
component_children!(ExtensionSelectorComponent, Container, container);
impl InputHandler for ExtensionSelectorComponent {
    fn handle_input(&self, data: &str) {
        let bindings = get_keybindings();
        if bindings.matches(data, "app.tools.expand") {
            if let Some(toggle) = &self.on_toggle_tools_expanded {
                toggle();
            }
        } else if bindings.matches(data, "tui.select.up") || data == "k" {
            self.selected.set(self.selected.get().saturating_sub(1));
            self.update_list();
        } else if bindings.matches(data, "tui.select.down") || data == "j" {
            self.selected
                .set((self.selected.get() + 1).min(self.options.len().saturating_sub(1)));
            self.update_list();
        } else if bindings.matches(data, "tui.select.confirm") || data == "\n" {
            if let Some(selected) = self
                .options
                .get(self.selected.get())
                .filter(|value| !value.is_empty())
            {
                (self.on_select)(selected);
            }
        } else if bindings.matches(data, "tui.select.cancel") {
            (self.on_cancel)();
        }
    }
}
impl Component for ExtensionSelectorComponent {
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

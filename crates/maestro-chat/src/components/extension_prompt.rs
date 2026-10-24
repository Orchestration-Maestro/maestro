//! Extension text submission over the toolkit's retained input editor.
use super::{countdown_timer::CountdownTimer, dialog_chrome, dialog_countdown};
use maestro_theme::{LiveTheme, ThemeError};
use maestro_tui::interface::{InputHandler, TuiRuntime};
use maestro_tui::{Component, Container, FocusFlag, Focusable, Input, TUI, get_keybindings};
use std::rc::Rc;

/// Presentation and optional countdown hosts for a text dialog.
pub struct ExtensionInputOptions {
    /// Writer used to request countdown frames.
    pub tui: Option<TUI>,
    /// Optional deadline in milliseconds.
    pub timeout: Option<f64>,
    /// Scheduling owner.
    pub runtime: Rc<dyn TuiRuntime>,
    /// Live presentation slot.
    pub theme: LiveTheme,
}
/// A text dialog forwarding focus immediately to its retained editor.
pub struct ExtensionInputComponent {
    /// Editable chrome owner.
    container: Container,
    /// Retained editor, independent of child-array replacement.
    input: Rc<Input>,
    /// Parent state and its synchronous child forwarding target.
    focus: FocusFlag,
    /// Submission notification.
    on_submit: Rc<dyn Fn(&str)>,
    /// Cancellation notification.
    on_cancel: Rc<dyn Fn()>,
    /// Owned countdown, if admitted.
    countdown: Option<CountdownTimer>,
}
impl ExtensionInputComponent {
    /// Composes an empty editor; the accepted placeholder is intentionally unused.
    ///
    /// # Errors
    /// Returns the existing hint theme failure.
    pub fn new(
        title: String,
        _placeholder: Option<&str>,
        on_submit: Rc<dyn Fn(&str)>,
        on_cancel: Rc<dyn Fn()>,
        opts: ExtensionInputOptions,
    ) -> Result<Self, ThemeError> {
        let ExtensionInputOptions {
            tui,
            timeout,
            runtime,
            theme,
        } = opts;
        let input = Rc::new(Input::new());
        let hint = maestro_app::key_hint("tui.select.confirm", "submit", &theme)?
            + "  "
            + &maestro_app::key_hint("tui.select.cancel", "cancel", &theme)?;
        let (container, update) = dialog_chrome(title, input.clone(), theme, hint, false);
        let countdown = dialog_countdown(timeout, tui.as_ref(), runtime, update, on_cancel.clone());
        let focus = FocusFlag::forwarding(input.clone());
        Ok(Self {
            container,
            input,
            focus,
            on_submit,
            on_cancel,
            countdown,
        })
    }
    /// Stops countdown work without notifying or disabling subsequent input.
    pub fn dispose(&self) {
        if let Some(countdown) = &self.countdown {
            countdown.dispose();
        }
    }
}
component_children!(ExtensionInputComponent, Container, container);
impl InputHandler for ExtensionInputComponent {
    fn handle_input(&self, data: &str) {
        let bindings = get_keybindings();
        if bindings.matches(data, "tui.select.confirm") || data == "\n" {
            (self.on_submit)(&self.input.get_value());
        } else if bindings.matches(data, "tui.select.cancel") {
            (self.on_cancel)();
        } else {
            self.input.handle_input(data);
        }
    }
}
impl Focusable for ExtensionInputComponent {
    fn focus_flag(&self) -> &FocusFlag {
        &self.focus
    }
}
impl Component for ExtensionInputComponent {
    fn render(&self, width: usize) -> Vec<String> {
        self.container.render(width)
    }
    fn invalidate(&self) {
        self.container.invalidate();
    }
    fn input_handler(&self) -> Option<&dyn InputHandler> {
        Some(self)
    }
    fn focusable(&self) -> Option<&dyn Focusable> {
        Some(self)
    }
}

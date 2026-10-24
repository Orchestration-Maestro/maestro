//! Editor actions: the shell-mode border.
use super::InteractiveMode;
use crate::components::thinking_selector::thinking_text;
use maestro_app::is_whitespace_scalar;
use maestro_theme::ThemeError;
use std::rc::Rc;

impl InteractiveMode {
    /// Installs the editor's change callback without keeping the mode alive: text whose
    /// first non-whitespace character is `!` selects the shell border, and only a change
    /// of mode repaints it.
    pub(super) fn setup_editor_actions(self: &Rc<Self>) {
        let mode = Rc::downgrade(self);
        self.editor.set_on_change(Some(Rc::new(move |text| {
            let Some(mode) = mode.upgrade() else {
                return;
            };
            let shell = text
                .trim_start_matches(is_whitespace_scalar)
                .starts_with('!');
            if mode.is_bash_mode.replace(shell) != shell {
                mode.report(mode.update_editor_border_color().map_err(Into::into));
            }
        })));
    }

    /// Paints the editor border with the shell colour in shell mode, otherwise with the
    /// colour of the session's thinking level, and schedules a frame.
    ///
    /// # Errors
    /// Returns the failure to read the published theme.
    pub(super) fn update_editor_border_color(&self) -> Result<(), ThemeError> {
        let theme = self.themes.theme().get()?;
        let border = if self.is_bash_mode.get() {
            theme.get_bash_mode_border_color()
        } else {
            let level = self.runtime_host.session().thinking_level();
            theme.get_thinking_border_color(thinking_text(&level).0)
        };
        self.editor.set_border_color(border);
        self.ui.request_render(false);
        Ok(())
    }

    /// Hands a failed editor callback's error to the host, which reports it.
    fn report(&self, result: Result<(), Box<dyn std::error::Error>>) {
        if let Err(error) = result {
            self.host_runtime
                .spawn_local(Box::pin(std::future::ready(Err(error))));
        }
    }
}

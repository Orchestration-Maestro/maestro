//! The startup header, its help text and the labelled transcript notices.

use super::InteractiveMode;
use maestro_app::configuration::config::{APP_NAME, VERSION};
use maestro_theme::{Theme, ThemeColor, ThemeError};
use maestro_tui::{Component, Spacer, Text};
use std::{error::Error, rc::Rc};

#[cfg(test)]
#[cfg(unix)]
mod tests;

impl InteractiveMode {
    /// Empties the editor and schedules a frame, or reports its original write failure.
    pub fn clear_editor(&self) {
        if let Err(error) = self.current_editor().try_set_text("") {
            return self.report(Err(error));
        }
        self.ui.request_render(false);
    }

    /// Appends an error notice followed by a blank row to the transcript and schedules a
    /// frame.
    ///
    /// # Errors
    /// Returns the failure of the published theme's error colour.
    pub fn show_error(&self, message: &str) -> Result<(), ThemeError> {
        self.notice(&ThemeColor::Error, &format!("Error: {message}"))?;
        self.chat_container.add_child(Rc::new(Spacer::new(1)));
        Ok(())
    }

    /// Appends a warning notice to the transcript and schedules a frame.
    ///
    /// # Errors
    /// Returns the failure of the published theme's warning colour.
    pub fn show_warning(&self, message: &str) -> Result<(), ThemeError> {
        self.notice(&ThemeColor::Warning, &format!("Warning: {message}"))
    }

    /// Appends a spacer and one coloured text row, then schedules a frame.
    ///
    /// # Errors
    /// Returns the failure of the published theme's colour.
    pub(super) fn notice(&self, color: &ThemeColor, text: &str) -> Result<(), ThemeError> {
        self.add_text(self.themes.theme().get()?.fg(color, text)?, 0);
        Ok(())
    }

    /// Appends a spacer and one text, padded by one column and `padding_y` rows, then
    /// schedules a frame.
    pub(super) fn add_text(&self, text: String, padding_y: usize) {
        self.chat_container.add_child(Rc::new(Spacer::new(1)));
        self.chat_container
            .add_child(Rc::new(Text::new(text, 1, padding_y, None)));
        self.ui.request_render(false);
    }

    /// Adds the startup help to the header container: nothing when startup is quiet
    /// without verbose, otherwise the compact or the expanded instructions.
    ///
    /// # Errors
    /// Returns the failure of the published theme's colours.
    pub(super) fn build_header(&self) -> Result<(), Box<dyn Error>> {
        let session = self.runtime_host.session();
        if !self.options.verbose && session.settings_manager().borrow().get_quiet_startup() {
            let header: Rc<dyn Component> = Rc::new(Text::new(String::new(), 0, 0, None));
            self.built_in_header
                .replace(Some(header.clone() as Rc<dyn Component>));
            self.header_container.add_child(header);
            return Ok(());
        }
        let live = self.themes.theme();
        let theme = live.get()?;
        let hint = |action, text| maestro_app::key_hint(action, text, &live);
        let raw = |key: &str, text| maestro_app::raw_key_hint(key, text, &live);
        let key = maestro_app::key_text;
        let expanded = expanded_instructions(&live)?;
        let compact = [
            hint("app.interrupt", "interrupt")?,
            raw(
                &format!("{}/{}", key("app.clear"), key("app.exit")),
                "clear/exit",
            )?,
            raw("/", "commands")?,
            raw("!", "bash")?,
            hint("app.tools.expand", "more")?,
        ]
        .join(&theme.fg(&ThemeColor::Muted, " · ")?);
        let logo = Theme::bold(&theme.fg(&ThemeColor::Accent, APP_NAME)?, true)
            + &theme.fg(&ThemeColor::Dim, &format!(" v{VERSION}"))?;
        let onboarding = theme.fg(&ThemeColor::Dim, "Maestro can explain its own features and look up its docs. Ask it how to use or extend Maestro.")?;
        let compact_onboarding = theme.fg(
            &ThemeColor::Dim,
            &format!(
                "Press {} to show full startup help and loaded resources.",
                key("app.tools.expand")
            ),
        )?;
        let collapsed = format!("{logo}\n{compact}\n{compact_onboarding}\n\n{onboarding}");
        let expanded = format!("{logo}\n{expanded}\n\n{onboarding}");
        let header = Rc::new(ExpandableText::new(
            Box::new(move || Ok(collapsed.clone())),
            Box::new(move || Ok(expanded.clone())),
            self.get_startup_expansion_state(),
            1,
            0,
        )?);
        self.built_in_header.replace(Some(header.clone()));
        self.header_container.add_child(Rc::new(Spacer::new(1)));
        self.header_container.add_child(header);
        self.header_container.add_child(Rc::new(Spacer::new(1)));
        Ok(())
    }
}

/// The expanded startup instructions, one styled hint per line.
///
/// # Errors
/// Returns the failure of the published theme's colours.
fn expanded_instructions(
    live: &maestro_theme::LiveTheme,
) -> Result<String, maestro_theme::ThemeError> {
    let hint = |action, text| maestro_app::key_hint(action, text, live);
    let raw = |key: &str, text| maestro_app::raw_key_hint(key, text, live);
    let key = maestro_app::key_text;
    Ok([
        hint("app.interrupt", "to interrupt")?,
        hint("app.clear", "to clear")?,
        raw(&format!("{} twice", key("app.clear")), "to exit")?,
        hint("app.exit", "to exit (empty)")?,
        hint("app.suspend", "to suspend")?,
        hint("tui.editor.deleteToLineEnd", "to delete to end")?,
        hint("app.thinking.cycle", "to cycle thinking level")?,
        raw(
            &format!(
                "{}/{}",
                key("app.model.cycleForward"),
                key("app.model.cycleBackward")
            ),
            "to cycle models",
        )?,
        hint("app.model.select", "to select model")?,
        hint("app.tools.expand", "to expand tools")?,
        hint("app.thinking.toggle", "to expand thinking")?,
        hint("app.editor.external", "for external editor")?,
        raw("/", "for commands")?,
        raw("!", "to run bash")?,
        raw("!!", "to run bash (no context)")?,
        hint("app.message.followUp", "to queue follow-up")?,
        hint("app.message.dequeue", "to edit all queued messages")?,
        hint("app.clipboard.pasteImage", "to paste image")?,
        raw("drop files", "to attach")?,
    ]
    .join("\n"))
}

/// Builds one rendering of an expandable text when it is selected.
pub(super) type TextFactory = Box<dyn Fn() -> Result<String, ThemeError>>;

/// One text with a collapsed and an expanded rendering, each built when selected.
pub(super) struct ExpandableText {
    /// The text owner that paints, wraps and clips the current rendering.
    text: Text,
    /// Builds the compact rendering.
    collapsed: TextFactory,
    /// Builds the expanded rendering.
    expanded: TextFactory,
}

impl ExpandableText {
    /// Builds the component and applies the initial expansion.
    ///
    /// # Errors
    /// Returns the failure of the selected rendering.
    pub(super) fn new(
        collapsed: TextFactory,
        expanded: TextFactory,
        expanded_state: bool,
        padding_x: usize,
        padding_y: usize,
    ) -> Result<Self, ThemeError> {
        let value = Self {
            text: Text::new(String::new(), padding_x, padding_y, None),
            collapsed,
            expanded,
        };
        value.set_expanded(expanded_state)?;
        Ok(value)
    }

    /// Builds the selected rendering and replaces the text with it.
    ///
    /// # Errors
    /// Returns the failure of the selected rendering, leaving the text unchanged.
    pub(super) fn set_expanded(&self, expanded: bool) -> Result<(), ThemeError> {
        let factory = if expanded {
            &self.expanded
        } else {
            &self.collapsed
        };
        self.text.set_text(factory()?);
        Ok(())
    }
}

impl Component for ExpandableText {
    /// Renders the current rendering at the supplied width.
    fn render(&self, width: usize) -> Vec<String> {
        self.text.render(width)
    }

    /// Discards the text owner's cached lines.
    fn invalidate(&self) {
        self.text.invalidate();
    }
}

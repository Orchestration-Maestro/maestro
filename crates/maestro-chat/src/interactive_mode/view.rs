//! A read-only view of what the interactive mode shows, for controlled tests.
use super::InteractiveMode;
use maestro_tui::Component;

/// One of the eight regions the mode lays out, from top to bottom.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Region {
    /// The startup header, built-in or supplied by an extension.
    Header,
    /// The transcript.
    Chat,
    /// The queued messages that wait for delivery.
    PendingMessages,
    /// The status line and the working row.
    Status,
    /// The extension widgets above the editor.
    WidgetsAbove,
    /// The editor, or the dialog shown in its place.
    Editor,
    /// The extension widgets below the editor.
    WidgetsBelow,
    /// The footer, built-in or supplied by an extension.
    Footer,
}

/// What an [`InteractiveMode`] shows, read without changing it.
pub struct ModeView<'a> {
    /// The mode read.
    mode: &'a InteractiveMode,
}

impl ModeView<'_> {
    /// The rows `region` renders at the terminal's current width.
    #[must_use]
    pub fn region(&self, region: Region) -> Vec<String> {
        let mode = self.mode;
        let width = mode.ui.terminal().borrow().columns();
        match region {
            Region::Header => mode.header_container.render(width),
            Region::Chat => mode.chat_container.render(width),
            Region::PendingMessages => mode.pending_messages_container.render(width),
            Region::Status => mode.status_container.render(width),
            Region::WidgetsAbove => mode.widget_container_above.render(width),
            Region::Editor => mode.editor_container.render(width),
            Region::WidgetsBelow => mode.widget_container_below.render(width),
            Region::Footer => match mode.custom_footer.borrow().as_ref() {
                Some(footer) => footer.component.render(width),
                None => mode.footer.render(width),
            },
        }
    }

    /// The text the current editor holds.
    #[must_use]
    pub fn editor_text(&self) -> String {
        self.mode.current_editor().get_text()
    }
}

impl InteractiveMode {
    /// The read-only view of this mode's regions and editor text.
    #[must_use]
    pub fn view(&self) -> ModeView<'_> {
        ModeView { mode: self }
    }
}

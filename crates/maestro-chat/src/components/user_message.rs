//! User text in a live themed panel with prompt-zone markers.
use maestro_theme::{LiveTheme, ThemeBg, ThemeColor};
use maestro_tui::{Box, Component, Container, MarkdownTheme, TerminalImage};
use std::rc::Rc;

/// An editable user-message container.
pub struct UserMessageComponent {
    /// Owns the padded message panel.
    container: Container,
}
impl UserMessageComponent {
    /// Builds a user panel using supplied Markdown callbacks and terminal capabilities.
    #[must_use]
    pub fn new(
        text: String,
        theme: LiveTheme,
        markdown_theme: Rc<MarkdownTheme>,
        terminal_image: TerminalImage,
    ) -> Self {
        let panel = Box::new(
            1,
            1,
            Some(super::background(theme.clone(), ThemeBg::UserMessageBg)),
        );
        let rendering = super::MessageRendering::new(theme, markdown_theme, terminal_image);
        panel.add_child(rendering.markdown(text, 0, Some(ThemeColor::UserMessageText), false));
        let container = Container::new();
        container.add_child(Rc::new(panel));
        Self { container }
    }
}
message_children!(UserMessageComponent, Container, container);
impl Component for UserMessageComponent {
    fn render(&self, width: usize) -> Vec<String> {
        super::mark_zone(self.container.render(width))
    }
    fn invalidate(&self) {
        self.container.invalidate();
    }
}

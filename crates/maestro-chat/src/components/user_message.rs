//! User text as one padded Markdown layer on its live background, with prompt-zone
//! markers.
use maestro_theme::{LiveTheme, ThemeBg, ThemeColor};
use maestro_tui::{Component, Container, MarkdownOptions, MarkdownTheme, TerminalImage};
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
        let background = super::background(theme.clone(), ThemeBg::UserMessageBg);
        let rendering = super::MessageRendering::new(theme, markdown_theme, terminal_image);
        let mut style = rendering
            .text_style(Some(ThemeColor::UserMessageText), false)
            .unwrap_or_default();
        style.bg_color = Some(Box::new(move |text| background(text)));
        let message = rendering.markdown_with(
            text,
            MarkdownOptions {
                padding_x: 1,
                padding_y: 1,
                default_text_style: Some(style),
                clean_content: false,
            },
        );
        let container = Container::new();
        container.add_child(message);
        Self { container }
    }
}
component_children!(UserMessageComponent, Container, container);
impl Component for UserMessageComponent {
    fn render(&self, width: usize) -> Vec<String> {
        super::mark_zone(self.container.render(width))
    }
    fn invalidate(&self) {
        self.container.invalidate();
    }
}

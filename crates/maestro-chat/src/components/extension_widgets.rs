//! The host widgets an extension builds, over the toolkit and application widgets.
use super::dynamic_border::DynamicBorder;
use maestro_app::ExtensionWidgets;
use maestro_theme::{LiveTheme, SyntaxHighlighter, get_markdown_theme};
use maestro_tui::{
    Markdown, MarkdownOptions, MarkdownTheme, TerminalImage, editor_component::BorderColor,
    tui::ComponentHandle,
};
use std::rc::Rc;

/// Builds extension widgets with the live theme, the code highlighting and the terminal's
/// capabilities.
pub struct ExtensionWidgetHost {
    /// The live theme the styles read.
    theme: LiveTheme,
    /// The code highlighting of the Markdown styles.
    highlighter: Rc<dyn SyntaxHighlighter>,
    /// The terminal's capabilities Markdown is laid out for.
    terminal: TerminalImage,
}

impl ExtensionWidgetHost {
    /// Retains the host's theme, highlighting and terminal capabilities.
    #[must_use]
    pub fn new(
        theme: LiveTheme,
        highlighter: Rc<dyn SyntaxHighlighter>,
        terminal: TerminalImage,
    ) -> Self {
        Self {
            theme,
            highlighter,
            terminal,
        }
    }
}

impl ExtensionWidgets for ExtensionWidgetHost {
    fn markdown(
        &self,
        text: String,
        options: MarkdownOptions,
        theme: Rc<MarkdownTheme>,
    ) -> Rc<Markdown> {
        Rc::new(Markdown::new(text, options, theme, self.terminal.clone()))
    }
    fn markdown_theme(&self) -> MarkdownTheme {
        get_markdown_theme(self.theme.clone(), self.highlighter.clone(), true)
    }
    fn dynamic_border(&self, color: Option<BorderColor>) -> ComponentHandle {
        Rc::new(DynamicBorder::new(self.theme.clone(), color))
    }
}

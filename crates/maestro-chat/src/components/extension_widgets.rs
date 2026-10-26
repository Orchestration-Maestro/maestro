//! The host widgets an extension builds, over the toolkit and application widgets.
use super::bordered_loader::{BorderedLoader, BorderedLoaderOptions};
use super::dynamic_border::DynamicBorder;
use maestro_app::{ExtensionLoader, ExtensionWidgets};
use maestro_theme::{
    LiveTheme, SyntaxHighlighter, Theme, get_markdown_theme, get_select_list_theme,
    get_settings_list_theme,
};
use maestro_tui::{
    Cancellation, Container, Markdown, MarkdownOptions, MarkdownTheme, SelectListTheme,
    SettingsListTheme, TUI, TerminalImage, editor_component::BorderColor, tui::ComponentHandle,
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
    fn select_list_theme(&self) -> SelectListTheme {
        get_select_list_theme(self.theme.clone())
    }
    fn settings_list_theme(&self) -> SettingsListTheme {
        get_settings_list_theme(self.theme.clone())
    }
    fn bordered_loader(
        &self,
        tui: &TUI,
        theme: Rc<Theme>,
        message: String,
        cancellable: bool,
    ) -> Result<Rc<dyn ExtensionLoader>, String> {
        let options = BorderedLoaderOptions { cancellable };
        let loader = BorderedLoader::new(tui, theme, message, options, &self.theme);
        loader
            .map(|loader| Rc::new(loader) as Rc<dyn ExtensionLoader>)
            .map_err(|error| error.to_string())
    }
}

impl ExtensionLoader for BorderedLoader {
    fn signal(&self) -> Cancellation {
        BorderedLoader::signal(self).clone()
    }
    fn set_on_abort(&self, on_abort: Option<Rc<dyn Fn()>>) {
        BorderedLoader::set_on_abort(self, on_abort);
    }
    fn dispose(&self) {
        BorderedLoader::dispose(self);
    }
    fn rows(&self) -> &Container {
        self
    }
}

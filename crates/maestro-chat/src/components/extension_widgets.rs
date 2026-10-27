//! The host widgets an extension builds, over the toolkit and application widgets.
use super::bordered_loader::{BorderedLoader, BorderedLoaderOptions};
use super::custom_editor::CustomEditor;
use super::dynamic_border::DynamicBorder;
use maestro_app::{
    ExtensionEditor, ExtensionKeybindings, ExtensionLoader, ExtensionWidgets, KeybindingsManager,
};
use maestro_theme::{
    LiveTheme, SyntaxHighlighter, Theme, get_editor_theme, get_markdown_theme,
    get_select_list_theme, get_settings_list_theme,
};
use maestro_tui::{
    Cancellation, Container, EditorTheme, KeybindingsConfig, Markdown, MarkdownOptions,
    MarkdownTheme, SelectListTheme, SettingsListTheme, TUI, TerminalImage,
    editor_component::BorderColor, tui::ComponentHandle,
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
    fn theme(&self) -> LiveTheme {
        self.theme.clone()
    }
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
    fn keybindings(
        &self,
        user_bindings: KeybindingsConfig,
        config_path: Option<String>,
    ) -> ExtensionKeybindings {
        keybindings(KeybindingsManager::new(user_bindings, config_path))
    }
    fn create_keybindings(
        &self,
        agent_dir: Option<String>,
    ) -> Result<ExtensionKeybindings, String> {
        #[cfg(not(target_arch = "wasm32"))]
        let manager =
            KeybindingsManager::create(agent_dir.as_deref()).map_err(|error| error.to_string())?;
        // The browser has no keybindings file, so it keeps the default bindings.
        #[cfg(target_arch = "wasm32")]
        let manager = {
            drop(agent_dir);
            KeybindingsManager::default()
        };
        Ok(keybindings(manager))
    }
    fn editor_theme(&self) -> EditorTheme {
        get_editor_theme(self.theme.clone())
    }
    fn key_hint(&self, keybinding: &str, description: &str) -> Result<String, String> {
        maestro_app::key_hint(keybinding, description, &self.theme)
            .map_err(|error| error.to_string())
    }
    fn raw_key_hint(&self, key: &str, description: &str) -> Result<String, String> {
        maestro_app::raw_key_hint(key, description, &self.theme).map_err(|error| error.to_string())
    }
    fn custom_editor(
        &self,
        tui: &TUI,
        theme: EditorTheme,
        keybindings: maestro_tui::KeybindingsManager,
        options: maestro_tui::EditorOptions,
    ) -> Rc<dyn ExtensionEditor> {
        Rc::new(CustomEditor::new(tui, theme, keybindings, options))
    }
}

/// The application manager's shared bindings and, on native targets, its reload.
pub(crate) fn keybindings(manager: KeybindingsManager) -> ExtensionKeybindings {
    let shared = (*manager).clone();
    #[cfg(not(target_arch = "wasm32"))]
    let reload: Option<Rc<dyn Fn()>> = Some(Rc::new(move || manager.reload()));
    #[cfg(target_arch = "wasm32")]
    let reload = {
        drop(manager);
        None
    };
    ExtensionKeybindings {
        manager: shared,
        reload,
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

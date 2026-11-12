//! Toolkit widgets over a live theme, for a host with no terminal image support.

use std::rc::Rc;

use maestro_theme::{
    LiveTheme, SyntaxHighlighter, SyntectHighlighter, Theme, ThemeColor, get_editor_theme,
    get_markdown_theme, get_select_list_theme, get_settings_list_theme, paint,
};
use maestro_tui::{
    CancellableLoader, Cancellation, Component, ComponentHandle, Container, Editor, EditorOptions,
    EditorTheme, InputHandler, KeybindingsConfig, KeybindingsManager, Loader, Markdown,
    MarkdownOptions, MarkdownTheme, SelectListTheme, SettingsListTheme, TUI, TUI_KEYBINDINGS,
    TerminalImage, editor_component::BorderColor,
};

use super::editor::CustomEditor;
use crate::types::{ExtensionEditor, ExtensionKeybindings, ExtensionLoader, ExtensionWidgets};

/// Toolkit widgets over a live theme with code highlighting; Markdown is laid out for a
/// terminal without image support, and the rule paints like the application's rule.
///
/// A keybindings manager built with a configuration path reloads the bindings set by
/// [`ToolkitWidgets::reloading`], standing in for the file an application rereads.
pub struct ToolkitWidgets {
    /// The live theme the styles read.
    theme: LiveTheme,
    /// The code highlighting of the Markdown styles.
    highlighter: Rc<dyn SyntaxHighlighter>,
    /// The user bindings a manager with a configuration path reloads.
    reloaded: KeybindingsConfig,
}

impl ToolkitWidgets {
    /// Widgets over `theme` whose managers reload no user bindings.
    ///
    /// # Errors
    /// Returns the highlighter's failure to load its syntax definitions.
    pub fn new(theme: LiveTheme) -> Result<Self, String> {
        let highlighter = SyntectHighlighter::new().map_err(|error| error.to_string())?;
        Ok(Self {
            theme,
            highlighter: Rc::new(highlighter),
            reloaded: Vec::new(),
        })
    }

    /// The same widgets, whose managers with a configuration path reload `bindings`.
    #[must_use]
    pub fn reloading(self, bindings: KeybindingsConfig) -> Self {
        Self {
            reloaded: bindings,
            ..self
        }
    }
}

impl ExtensionWidgets for ToolkitWidgets {
    fn theme(&self) -> LiveTheme {
        self.theme.clone()
    }
    fn markdown(
        &self,
        text: String,
        options: MarkdownOptions,
        theme: Rc<MarkdownTheme>,
    ) -> Rc<Markdown> {
        let terminal = TerminalImage::new(|_| None, || 1);
        Rc::new(Markdown::new(text, options, theme, terminal))
    }
    fn markdown_theme(&self) -> MarkdownTheme {
        get_markdown_theme(self.theme.clone(), self.highlighter.clone(), true)
    }
    fn dynamic_border(&self, color: Option<BorderColor>) -> ComponentHandle {
        Rc::new(Rule(color, self.theme.clone()))
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
        let color = move |key: ThemeColor| -> BorderColor {
            let theme = theme.clone();
            Rc::new(move |text: &str| paint(Some(&theme), &key, text))
        };
        let loader = Loader::new(
            tui.clone(),
            color(ThemeColor::Accent),
            color(ThemeColor::Muted),
            Some(message),
            None,
        );
        let loader = Rc::new(CancellableLoader::new(loader));
        let rows = Container::new();
        rows.add_child(loader.clone());
        Ok(Rc::new(BorderlessLoader {
            rows,
            inner: loader,
            cancellable,
            signal: Cancellation::new(),
        }))
    }
    fn keybindings(
        &self,
        user_bindings: KeybindingsConfig,
        config_path: Option<String>,
    ) -> ExtensionKeybindings {
        let manager = KeybindingsManager::new(TUI_KEYBINDINGS.clone(), user_bindings);
        let reloaded = manager.clone();
        let bindings = self.reloaded.clone();
        ExtensionKeybindings {
            manager,
            reload: config_path.map(|_| {
                Rc::new(move || reloaded.set_user_bindings(bindings.clone())) as Rc<dyn Fn()>
            }),
        }
    }
    fn create_keybindings(
        &self,
        agent_dir: Option<String>,
    ) -> Result<ExtensionKeybindings, String> {
        let path = agent_dir.unwrap_or_else(|| "/agent".to_owned());
        Ok(self.keybindings(Vec::new(), Some(format!("{path}/keybindings.json"))))
    }
    fn editor_theme(&self) -> EditorTheme {
        get_editor_theme(self.theme.clone())
    }
    fn key_hint(&self, keybinding: &str, description: &str) -> Result<String, String> {
        let keys = maestro_tools::key_text(keybinding);
        Ok(format!("[{keys}] {description}"))
    }
    fn raw_key_hint(&self, key: &str, description: &str) -> Result<String, String> {
        Ok(format!("[{key}] {description}"))
    }
    fn custom_editor(
        &self,
        tui: &TUI,
        theme: EditorTheme,
        keybindings: KeybindingsManager,
        options: EditorOptions,
    ) -> Rc<dyn ExtensionEditor> {
        Rc::new(CustomEditor::new(
            Editor::new(tui, theme, options),
            keybindings,
        ))
    }
}

/// A loader between no borders: input cancels it only when it is cancellable.
struct BorderlessLoader {
    /// The rows it renders.
    rows: Container,
    /// The cancellable loader among them.
    inner: Rc<CancellableLoader>,
    /// Whether input can cancel it.
    cancellable: bool,
    /// The signal of a loader input cannot cancel.
    signal: Cancellation,
}

impl Component for BorderlessLoader {
    fn render(&self, width: usize) -> Vec<String> {
        self.rows.render(width)
    }
    fn input_handler(&self) -> Option<&dyn InputHandler> {
        Some(self)
    }
}

impl InputHandler for BorderlessLoader {
    fn handle_input(&self, data: &str) {
        if self.cancellable {
            InputHandler::handle_input(&*self.inner, data);
        }
    }
}

impl ExtensionLoader for BorderlessLoader {
    fn signal(&self) -> Cancellation {
        if self.cancellable {
            self.inner.signal().clone()
        } else {
            self.signal.clone()
        }
    }
    fn set_on_abort(&self, on_abort: Option<Rc<dyn Fn()>>) {
        self.inner.set_on_abort(on_abort);
    }
    fn dispose(&self) {
        self.inner.dispose();
    }
    fn rows(&self) -> &Container {
        &self.rows
    }
}

/// A rule across the width, painted by its color or the live border color.
struct Rule(Option<BorderColor>, LiveTheme);

impl Component for Rule {
    fn render(&self, width: usize) -> Vec<String> {
        let rule = "─".repeat(width.max(1));
        vec![match &self.0 {
            Some(color) => color(&rule),
            None => paint(self.1.get().ok().as_deref(), &ThemeColor::Border, &rule),
        }]
    }
}

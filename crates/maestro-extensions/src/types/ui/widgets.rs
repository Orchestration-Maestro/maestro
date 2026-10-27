//! The host capability that builds the extension widgets whose look depends on the host.
use maestro_theme::Theme;
use maestro_tui::{
    Cancellation, Component, Container, Editor, EditorOptions, EditorTheme, KeybindingsConfig,
    KeybindingsManager, Markdown, MarkdownOptions, MarkdownTheme, SelectListTheme,
    SettingsListTheme, TUI, editor_component::BorderColor, tui::ComponentHandle,
};

/// A keybindings manager an extension holds, with the reload of an application manager that
/// has a file.
#[derive(Clone)]
pub struct ExtensionKeybindings {
    /// The manager; clones share its bindings.
    pub manager: KeybindingsManager,
    /// Reloads the user bindings from the manager's file.
    pub reload: Option<Rc<dyn Fn()>>,
}
use std::{ops::Deref, rc::Rc};

impl ExtensionKeybindings {
    /// Reloads through the supplied application manager, or does nothing without a reload.
    pub fn reload(&self) {
        if let Some(reload) = &self.reload {
            reload();
        }
    }

    /// Returns owned, resolved bindings for known actions in definition order.
    #[must_use]
    pub fn get_effective_config(&self) -> KeybindingsConfig {
        self.manager.get_resolved_bindings()
    }
}

impl Deref for ExtensionKeybindings {
    type Target = KeybindingsManager;

    fn deref(&self) -> &Self::Target {
        &self.manager
    }
}

/// The application's bordered loader, as an extension holds it.
pub trait ExtensionLoader: Component {
    /// The signal input cancels; a loader that cannot be cancelled keeps its own.
    fn signal(&self) -> Cancellation;
    /// Replaces or removes the notification a cancellation runs.
    fn set_on_abort(&self, on_abort: Option<Rc<dyn Fn()>>);
    /// Stops the animation without cancelling the signal.
    fn dispose(&self);
    /// The rows the loader renders, which an extension may extend.
    fn rows(&self) -> &Container;
}

/// The application's custom editor, as an extension holds it.
pub trait ExtensionEditor: super::editor::CustomEditorComponent {
    /// The toolkit editor it extends, for the editor operations beyond the editor component's.
    fn editor(&self) -> &Editor;
}

/// Builds the widgets whose look depends on the host: Markdown laid out for its terminal, the
/// live theme's Markdown styles and the application's dynamic border.
///
/// Every mode supplies one, so building a widget never needs an interactive UI; only showing
/// one goes through the UI context. Text, boxes, containers and spacers need nothing from the
/// host and are built from the toolkit directly.
pub trait ExtensionWidgets {
    /// The live theme shared by the host's widgets, even without an interactive UI.
    fn theme(&self) -> maestro_theme::LiveTheme;
    /// Markdown laid out with the host terminal's capabilities.
    fn markdown(
        &self,
        text: String,
        options: MarkdownOptions,
        theme: Rc<MarkdownTheme>,
    ) -> Rc<Markdown>;
    /// Markdown styles that read the live theme when they run, with the host's code
    /// highlighting.
    fn markdown_theme(&self) -> MarkdownTheme;
    /// A rule across the full width, painted by `color` or by the live border color.
    fn dynamic_border(&self, color: Option<BorderColor>) -> ComponentHandle;
    /// Selection list styles that read the live theme when they run.
    fn select_list_theme(&self) -> SelectListTheme;
    /// Settings list styles that read the live theme, with the cursor colored now.
    fn settings_list_theme(&self) -> SettingsListTheme;
    /// The application's bordered loader on `tui`, colored by `theme`.
    ///
    /// # Errors
    /// Returns the theme failure of the loader's cancellation hint.
    fn bordered_loader(
        &self,
        tui: &TUI,
        theme: Rc<Theme>,
        message: String,
        cancellable: bool,
    ) -> Result<Rc<dyn ExtensionLoader>, String>;
    /// A manager of the application's keybindings with `user_bindings`, reloading from
    /// `config_path`.
    fn keybindings(
        &self,
        user_bindings: KeybindingsConfig,
        config_path: Option<String>,
    ) -> ExtensionKeybindings;
    /// A manager of the application's keybindings loaded from `agent_dir`, or from the host's
    /// agent directory.
    ///
    /// # Errors
    /// Returns the failure to find the host's agent directory.
    fn create_keybindings(&self, agent_dir: Option<String>)
    -> Result<ExtensionKeybindings, String>;
    /// Editor styles that read the live theme when they run.
    fn editor_theme(&self) -> EditorTheme;
    /// The keys of `keybinding` and `description`, styled with the live theme.
    ///
    /// # Errors
    /// Returns the theme's failure, such as a theme that is not published yet.
    fn key_hint(&self, keybinding: &str, description: &str) -> Result<String, String>;
    /// A literal `key` and `description`, styled with the live theme.
    ///
    /// # Errors
    /// Returns the theme's failure, such as a theme that is not published yet.
    fn raw_key_hint(&self, key: &str, description: &str) -> Result<String, String>;
    /// The application's custom editor on `tui`, intercepting the actions `keybindings` binds.
    fn custom_editor(
        &self,
        tui: &TUI,
        theme: EditorTheme,
        keybindings: KeybindingsManager,
        options: EditorOptions,
    ) -> Rc<dyn ExtensionEditor>;
}

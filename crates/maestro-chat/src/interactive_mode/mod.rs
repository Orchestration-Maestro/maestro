//! The interactive terminal mode: its retained containers, startup help, loaded
//! resources, notices and typed input.

use crate::{CustomEditor, FooterComponent};
use maestro_app::presentation_data::footer_data_provider::{FooterDataProvider, FooterOperations};
use maestro_app::{
    AgentSession, AgentSessionRuntime, KeybindingsManager, ReadonlyFooterDataProvider,
    ReadonlySessionManager, SettingsManager,
};
use maestro_theme::{Collator, LiveTheme, ThemeState, get_editor_theme};
use maestro_tui::tui::{TerminalHandle, TuiRuntime};
use maestro_tui::{Container, EditorOptions, TUI, TerminalImage};
use std::{
    cell::{Cell, RefCell},
    error::Error,
    rc::Rc,
};

mod resources;
mod startup;
mod submit;

#[cfg(test)]
#[cfg(unix)]
mod tests;

/// How the interactive mode starts.
#[derive(Default)]
pub struct InteractiveModeOptions {
    /// Forces the expanded startup help, even when startup is quiet.
    pub verbose: bool,
}

/// The retained interactive presentation over the current session.
pub struct InteractiveMode {
    /// The runtime the current session is read from.
    runtime_host: Rc<AgentSessionRuntime>,
    /// How the mode starts.
    options: InteractiveModeOptions,
    /// The terminal user interface this mode owns.
    ui: TUI,
    /// The theme registry and its published theme.
    themes: Rc<ThemeState>,
    /// The startup header container.
    header_container: Rc<Container>,
    /// The transcript container notices are appended to.
    chat_container: Rc<Container>,
    /// The pending-messages container.
    pending_messages_container: Rc<Container>,
    /// The status container.
    status_container: Rc<Container>,
    /// The widget container above the editor.
    widget_container_above: Rc<Container>,
    /// The widget container below the editor.
    widget_container_below: Rc<Container>,
    /// The container the editor is added through.
    editor_container: Rc<Container>,
    /// The input editor.
    editor: Rc<CustomEditor>,
    /// The footer.
    footer: Rc<FooterComponent>,
    /// The footer metadata and its branch watch.
    footer_data_provider: FooterDataProvider,
    /// Whether the presentation has started.
    is_initialized: Cell<bool>,
    /// The only currently waiting input delivery.
    on_input_callback: Rc<RefCell<Option<tokio::sync::oneshot::Sender<String>>>>,
    /// Whether tool output, and with it the startup text, is expanded.
    tool_output_expanded: Cell<bool>,
}

impl InteractiveMode {
    /// Builds the retained presentation over the supplied terminal, themes and footer
    /// effects, then registers the session's loaded themes and selects the configured one.
    ///
    /// # Errors
    /// Returns the failure that stopped the keybinding manager, the footer's collator or
    /// the theme selection.
    pub fn new(
        runtime_host: Rc<AgentSessionRuntime>,
        options: InteractiveModeOptions,
        terminal: (TerminalHandle, Rc<dyn TuiRuntime>),
        themes: Rc<ThemeState>,
        operations: Rc<dyn FooterOperations>,
    ) -> Result<Rc<Self>, Box<dyn Error>> {
        let session = runtime_host.session();
        let ui = create_ui(&session, terminal);
        let keys = KeybindingsManager::create(None)?;
        maestro_tui::set_keybindings((*keys).clone());
        let editor = create_editor(
            &ui,
            themes.theme(),
            (*keys).clone(),
            session.settings_manager(),
        );
        let editor_container = Rc::new(Container::new());
        editor_container.add_child(editor.clone());
        let footer_data_provider = FooterDataProvider::new(
            session.session_manager().borrow().get_cwd().to_owned(),
            operations.clone(),
        );
        let footer = create_footer(session.clone(), &footer_data_provider, themes.theme())?;
        footer.set_auto_compact_enabled(session.auto_compaction_enabled());
        themes.set_registered_themes(session.resource_loader().get_themes().themes.clone());
        let selected_theme = session.settings_manager().borrow().get_theme();
        themes.init_theme(selected_theme.as_deref(), Some(operations))?;
        Ok(Rc::new(Self {
            runtime_host,
            options,
            ui,
            themes,
            header_container: Rc::new(Container::new()),
            chat_container: Rc::new(Container::new()),
            pending_messages_container: Rc::new(Container::new()),
            status_container: Rc::new(Container::new()),
            widget_container_above: Rc::new(Container::new()),
            widget_container_below: Rc::new(Container::new()),
            editor_container,
            editor,
            footer,
            footer_data_provider,
            is_initialized: Cell::new(false),
            on_input_callback: Rc::default(),
            tool_output_expanded: Cell::new(false),
        }))
    }

    /// Builds the header, adds every container in order, focuses the editor, installs
    /// editor submission, starts the terminal and lists the loaded resources with their
    /// diagnostics, once;
    /// theme and branch changes then request fresh frames.
    ///
    /// # Errors
    /// Returns the failure of the header's theme colours, of the terminal start or of
    /// the resource listing.
    pub fn init(self: &Rc<Self>) -> Result<(), Box<dyn Error>> {
        if self.is_initialized.get() {
            return Ok(());
        }
        self.build_header()?;
        for child in [
            &self.header_container,
            &self.chat_container,
            &self.pending_messages_container,
            &self.status_container,
            &self.widget_container_above,
            &self.editor_container,
            &self.widget_container_below,
        ] {
            self.ui.add_child(child.clone());
        }
        self.ui.add_child(self.footer.clone());
        self.ui.set_focus(Some(self.editor.clone()));
        self.setup_editor_submit_handler();
        self.ui.start()?;
        self.is_initialized.set(true);
        let mode = Rc::downgrade(self);
        self.themes.on_theme_change(Rc::new(move || {
            if let Some(mode) = mode.upgrade() {
                mode.ui.invalidate();
                mode.ui.request_render(false);
            }
            Ok(())
        }));
        // The subscription lasts as long as the provider; its removal is not needed.
        drop(
            self.footer_data_provider
                .on_branch_change(self.ui.weak_render_request()),
        );
        self.show_loaded_resources(resources::ShowLoadedResourcesOptions {
            show_diagnostics_when_quiet: true,
            ..resources::ShowLoadedResourcesOptions::default()
        })
    }
}

/// Builds the frame writer with the configured hardware-cursor and clear-on-shrink policies.
fn create_ui(session: &AgentSession, terminal: (TerminalHandle, Rc<dyn TuiRuntime>)) -> TUI {
    let settings = session.settings_manager().borrow();
    let ui = TUI::new(
        terminal.0,
        terminal.1,
        TerminalImage::default(),
        Some(settings.get_show_hardware_cursor()),
    );
    ui.set_clear_on_shrink(settings.get_clear_on_shrink());
    ui
}

/// Builds the editor with the configured padding and completion limit.
fn create_editor(
    ui: &TUI,
    theme: LiveTheme,
    keys: maestro_tui::KeybindingsManager,
    settings: &Rc<RefCell<SettingsManager>>,
) -> Rc<CustomEditor> {
    let options = {
        let settings = settings.borrow();
        EditorOptions {
            padding_x: Some(settings.get_editor_padding_x()),
            autocomplete_max_visible: Some(settings.get_autocomplete_max_visible()),
        }
    };
    Rc::new(CustomEditor::new(
        ui,
        get_editor_theme(theme),
        keys,
        options,
    ))
}

/// Builds the footer over the session, its metadata and the published theme.
///
/// # Errors
/// Returns the failure of the collator reading the locale environment.
fn create_footer(
    session: Rc<AgentSession>,
    data: &FooterDataProvider,
    theme: LiveTheme,
) -> Result<Rc<FooterComponent>, Box<dyn Error>> {
    Ok(Rc::new(FooterComponent::new(
        session,
        Rc::new(data.clone()),
        theme,
        Rc::new(Collator::new(&|key| std::env::var(key).ok())?),
        Rc::new(|key| std::env::var(key).ok()),
    )))
}

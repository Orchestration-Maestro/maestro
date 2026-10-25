//! The interactive terminal mode: its retained containers, startup help, loaded
//! resources, notices, transcript shortcuts, editor actions, typed input, queued
//! input and session commands.

use crate::{AssistantMessageComponent, BashExecutionComponent, CustomEditor, FooterComponent};
use maestro_app::presentation_data::footer_data_provider::{FooterDataProvider, FooterOperations};
use maestro_app::{
    AgentSession, AgentSessionRuntime, KeybindingsManager, ReadonlyFooterDataProvider,
    ReadonlySessionManager, SettingsManager, SharedAssistantMessage, ToolRenderEnvironment,
};
use maestro_theme::{Collator, LiveTheme, SyntectHighlighter, ThemeState, get_editor_theme};
use maestro_tui::tui::{TerminalHandle, TuiRuntime};
use maestro_tui::{Component, Container, EditorOptions, TUI, TerminalImage, Text};
use std::{
    cell::{Cell, RefCell},
    error::Error,
    rc::{Rc, Weak},
};

mod actions;
mod authentication;
mod bash;
mod debug;
mod events;
mod info;
mod key_display;
mod models;
mod notices;
mod queues;
mod replay;
mod resources;
mod selectors;
mod share;
mod startup;
mod submit;

mod extension_ui;
mod toggles;
mod transcript;
mod transfer;
mod working;

#[cfg(test)]
#[cfg(unix)]
mod tests;

/// The spacer and text of a status line.
type StatusLine = (Rc<dyn Component>, Rc<Text>);

/// The assistant row a response streams into and the message it shows.
type StreamingMessage = (Rc<AssistantMessageComponent>, SharedAssistantMessage);

/// How the interactive mode starts.
#[derive(Default)]
pub struct InteractiveModeOptions {
    /// Forces the expanded startup help, even when startup is quiet.
    pub verbose: bool,
    /// Providers whose credentials were migrated at startup, warned about in order.
    pub migrated_providers: Vec<String>,
    /// The warning shown when the session model could not be restored.
    pub model_fallback_message: Option<String>,
    /// The first prompt sent at startup.
    pub initial_message: Option<String>,
    /// Images attached to the first startup prompt.
    pub initial_images: Option<Vec<maestro_app::ImageContent>>,
    /// Prompts sent after the first startup prompt, in order.
    pub initial_messages: Vec<String>,
}

/// The retained interactive presentation over the current session.
pub struct InteractiveMode {
    /// Live keyed widgets above the editor.
    extension_widgets_above: maestro_app::Live<String, Rc<maestro_app::ExtensionComponent>>,
    /// Live keyed widgets below the editor.
    extension_widgets_below: maestro_app::Live<String, Rc<maestro_app::ExtensionComponent>>,
    /// This mode, for callbacks that must not keep it alive.
    this: Weak<InteractiveMode>,
    /// The runtime the current session is read from.
    runtime_host: Rc<AgentSessionRuntime>,
    /// How the mode starts.
    options: InteractiveModeOptions,
    /// The terminal user interface this mode owns, shared with dialogs that keep it weakly.
    ui: Rc<TUI>,
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
    /// The supplied watch, timer and Git process effects, reused for named theme watching.
    operations: Rc<dyn FooterOperations>,
    /// Whether the presentation has started.
    is_initialized: Cell<bool>,
    /// The expandable startup help, absent when startup is quiet.
    built_in_header: RefCell<Option<Rc<startup::ExpandableText>>>,
    /// The runtime, theme, highlighter and image support shared by transcript rows.
    tool_environment: Rc<ToolRenderEnvironment>,
    /// Whether tool output and summaries, and with them the startup text, are expanded.
    tool_output_expanded: Cell<bool>,
    /// Whether assistant thinking is replaced by its label.
    hide_thinking_block: Cell<bool>,
    /// The spacer and text of the latest status line.
    last_status: RefCell<Option<StatusLine>>,
    /// Tool rows waiting for their results, by call id in call order.
    pending_tools: RefCell<replay::PendingTools>,
    /// Whether the editor holds a shell command.
    is_bash_mode: Cell<bool>,
    /// The shell row that is still producing output, when one is attached.
    active_shell: RefCell<Option<Rc<BashExecutionComponent>>>,
    /// Shell rows shown in the pending area until the next ordinary submission.
    pending_shells: RefCell<Vec<Rc<BashExecutionComponent>>>,
    /// The only currently waiting input delivery.
    on_input_callback: Rc<RefCell<Option<tokio::sync::oneshot::Sender<String>>>>,
    /// Whether this mode has shown the subscription billing notice.
    anthropic_subscription_warning_shown: Cell<bool>,
    /// Messages typed while the session compacts, in the order they were typed.
    compaction_queued_messages: RefCell<Vec<queues::CompactionQueuedMessage>>,
    /// The session subscription that delivers agent and session events.
    unsubscribe: RefCell<Option<Box<dyn Fn()>>>,
    /// The working row and its extension-controlled presentation.
    working: working::WorkingState,
    /// The assistant row of the response streaming now.
    streaming: RefCell<Option<StreamingMessage>>,
}

impl InteractiveMode {
    /// Builds the retained presentation over the supplied terminal, themes and footer
    /// effects, then registers the session's loaded themes and selects the configured one.
    ///
    /// # Errors
    /// Returns the failure that stopped the documentation path, the highlighter, the
    /// keybinding manager, the footer's collator or the theme selection.
    pub fn new(
        runtime_host: Rc<AgentSessionRuntime>,
        options: InteractiveModeOptions,
        terminal: (TerminalHandle, Rc<dyn TuiRuntime>),
        themes: Rc<ThemeState>,
        operations: Rc<dyn FooterOperations>,
    ) -> Result<Rc<Self>, Box<dyn Error>> {
        let session = runtime_host.session();
        let tool_environment = tool_environment(terminal.1.clone(), themes.theme())?;
        let ui = create_ui(&session, terminal, tool_environment.terminal_image.clone());
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
        let settings = session.settings_manager();
        let selected_theme = settings.borrow().get_theme();
        themes.init_theme(selected_theme.as_deref(), Some(operations.clone()))?;
        let hide_thinking_block = settings.borrow().get_hide_thinking_block();
        Ok(Rc::new_cyclic(|this| Self {
            this: this.clone(),
            extension_widgets_above: maestro_app::Live::new(),
            extension_widgets_below: maestro_app::Live::new(),
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
            operations,
            is_initialized: Cell::new(false),
            built_in_header: RefCell::default(),
            tool_environment,
            tool_output_expanded: Cell::new(false),
            hide_thinking_block: Cell::new(hide_thinking_block),
            last_status: RefCell::default(),
            pending_tools: RefCell::default(),
            is_bash_mode: Cell::new(false),
            active_shell: RefCell::default(),
            pending_shells: RefCell::default(),
            on_input_callback: Rc::default(),
            anthropic_subscription_warning_shown: Cell::new(false),
            compaction_queued_messages: RefCell::default(),
            unsubscribe: RefCell::default(),
            working: working::WorkingState::default(),
            streaming: RefCell::default(),
        }))
    }

    /// Builds the header, adds every container in order, focuses the editor, binds the
    /// transcript shortcuts, installs editor submission and the global debug key, starts
    /// the terminal, lists the loaded resources with their diagnostics and replays the
    /// session, once; theme changes then refresh the editor border, and theme and branch
    /// changes request fresh frames.
    /// Finally the footer learns how many providers the model candidates cover.
    ///
    /// # Errors
    /// Returns the failure of the header's theme colours, of the terminal start, of the
    /// resource listing, of the replay or of the catalog refresh.
    pub fn init(self: &Rc<Self>) -> Result<(), Box<dyn Error>> {
        if self.is_initialized.get() {
            return Ok(());
        }
        self.build_header()?;
        self.render_widgets();
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
        self.setup_key_handlers();
        self.setup_queue_key_handlers();
        self.setup_editor_submit_handler();
        self.setup_debug_handler();
        self.ui.start()?;
        self.is_initialized.set(true);
        self.subscribe_to_agent();
        self.update_terminal_title()?;
        let mode = Rc::downgrade(self);
        self.themes.on_theme_change(Rc::new(move || {
            if let Some(mode) = mode.upgrade() {
                mode.ui.invalidate();
                mode.update_editor_border_color()?;
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
        })?;
        self.render_initial_messages()?;
        self.update_available_provider_count()
    }
}

/// The runtime, theme, highlighter and image support shared by the transcript rows.
///
/// # Errors
/// Returns the failure of the documentation path or of the highlighter.
fn tool_environment(
    runtime: Rc<dyn TuiRuntime>,
    live_theme: LiveTheme,
) -> Result<Rc<ToolRenderEnvironment>, Box<dyn Error>> {
    Ok(Rc::new(ToolRenderEnvironment {
        runtime,
        readme_path: maestro_app::configuration::config::get_readme_path()?,
        live_theme,
        highlighter: Rc::new(
            SyntectHighlighter::new().map_err(|error| -> Box<dyn Error> { error })?,
        ),
        terminal_image: TerminalImage::default(),
    }))
}

/// Builds the frame writer with the configured hardware-cursor and clear-on-shrink policies,
/// sharing the transcript rows' image support.
fn create_ui(
    session: &AgentSession,
    terminal: (TerminalHandle, Rc<dyn TuiRuntime>),
    image: TerminalImage,
) -> Rc<TUI> {
    let settings = session.settings_manager().borrow();
    let ui = TUI::new(
        terminal.0,
        terminal.1,
        image,
        Some(settings.get_show_hardware_cursor()),
    );
    ui.set_clear_on_shrink(settings.get_clear_on_shrink());
    Rc::new(ui)
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

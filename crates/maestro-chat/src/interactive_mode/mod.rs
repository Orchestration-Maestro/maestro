//! The interactive terminal mode: its retained containers, startup help, loaded
//! resources, notices, transcript shortcuts, editor actions, typed input, queued
//! input and session commands.

use crate::{AssistantMessageComponent, BashExecutionComponent, CustomEditor, FooterComponent};
use maestro_app::presentation_data::footer_data_provider::{FooterDataProvider, FooterOperations};
use maestro_app::{
    AgentSession, AgentSessionRuntime, AutocompleteProviderFactory, EventError, KeybindingsManager,
    ReadonlyFooterDataProvider, ReadonlySessionManager, SettingsManager, SharedAssistantMessage,
    ToolRenderEnvironment,
};
use maestro_theme::{Collator, LiveTheme, SyntectHighlighter, ThemeState, get_editor_theme};
use maestro_tui::tui::{TerminalHandle, TuiRuntime};
use maestro_tui::{Component, Container, EditorOptions, TUI, TerminalImage, Text};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    error::Error,
    rc::{Rc, Weak},
};

mod actions;
mod authentication;
mod autocomplete;
mod bash;
mod command_context_actions;
mod commands;
mod compaction_queue;
mod debug;
mod dispatch;
mod events;
mod extensions;
mod hotkeys;
mod info;
mod key_display;
mod lifecycle;
mod models;
mod navigation;
mod notices;
mod progress;
mod queues;
mod reload;
mod replay;
mod resources;
mod selectors;
mod settings;
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

pub use authentication::is_api_key_login_provider;
pub use maestro_app::presentation_data::mode_dispatch::InteractiveModeOptions;

/// The retained interactive presentation over the current session.
pub struct InteractiveMode {
    /// Whether an extension requested graceful shutdown after the agent finishes.
    pub(crate) shutdown_requested: Cell<bool>,
    /// Retained exit state and native registration ownership.
    lifecycle: lifecycle::LifecycleState,
    /// Bound extension input registrations and their cleanup operations.
    extension_input: extension_ui::input::ExtensionInput,
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
    /// The default input editor, which keeps the application's actions and callbacks.
    editor: Rc<CustomEditor>,
    /// The current editor, the extension editor factory and the completion provider.
    editor_state: extension_ui::editor::EditorState,
    /// The extension custom views that have not settled.
    custom_views: extension_ui::custom::CustomViews,
    /// The latest extension dialog of each kind.
    dialogs: extension_ui::dialogs::DialogSlots,
    /// The keybindings the editor reads, and the configured keys extension shortcuts use.
    keybindings: KeybindingsManager,
    /// The footer.
    footer: Rc<FooterComponent>,
    /// The footer metadata and its branch watch.
    footer_data_provider: FooterDataProvider,
    /// The supplied watch, timer and Git process effects, reused for named theme watching.
    operations: Rc<dyn FooterOperations>,
    /// Selected release history for this startup.
    changelog_markdown: RefCell<Option<String>>,
    /// Whether this mode has handled its startup notice.
    startup_notices_shown: Cell<bool>,
    /// Whether the presentation has started.
    is_initialized: Cell<bool>,
    /// Events waiting for the presentation to initialize, in arrival order.
    pending_events: RefCell<VecDeque<maestro_app::AgentSessionEvent>>,
    /// Whether the one initialization continuation is still delivering events.
    event_initialization_pending: Cell<bool>,
    /// The built-in header, absent until the header is built.
    built_in_header: RefCell<Option<maestro_tui::tui::ComponentHandle>>,
    /// The extension header shown in place of the built-in one.
    custom_header: RefCell<Option<Rc<maestro_app::ExtensionComponent>>>,
    /// The extension footer shown in place of the built-in one.
    custom_footer: RefCell<Option<Rc<maestro_app::ExtensionComponent>>>,
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
    /// Wall time of the latest unpaired interrupt press on the empty editor.
    last_escape_time: Cell<i64>,
    /// The shell row that is still producing output, when one is attached.
    active_shell: RefCell<Option<Rc<BashExecutionComponent>>>,
    /// Shell rows shown in the pending area until the next ordinary submission.
    pending_shells: RefCell<Vec<Rc<BashExecutionComponent>>>,
    /// The only currently waiting input delivery.
    on_input_callback: Rc<RefCell<Option<tokio::sync::oneshot::Sender<String>>>>,
    /// The selected search program for file completion, absent until startup prepares it.
    fd_path: RefCell<Option<String>>,
    /// Bound factories in registration order, including repeated handles.
    autocomplete_provider_wrappers: RefCell<Vec<AutocompleteProviderFactory>>,
    /// Whether this mode has shown the subscription billing notice.
    anthropic_subscription_warning_shown: Cell<bool>,
    /// Messages typed while the session compacts, in the order they were typed.
    compaction_queued_messages: RefCell<Vec<queues::CompactionQueuedMessage>>,
    /// The progress presentation's loaders, countdown and interrupt callbacks.
    progress: progress::Retained,
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
        let (tool_environment, ui) = create_presentation(&session, terminal, themes.theme())?;
        let (keys, editor, editor_container) = create_input(&session, &ui, themes.theme())?;
        let (footer, footer_data_provider) = create_footer(&session, &operations, themes.theme())?;
        let hide_thinking = init_theme(&session, &themes, &operations)?;
        let mode = Rc::new_cyclic(|this| Self {
            shutdown_requested: Cell::new(false),
            this: this.clone(),
            lifecycle: lifecycle::LifecycleState::default(),
            extension_input: extension_ui::input::ExtensionInput::default(),
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
            editor_state: extension_ui::editor::EditorState::new(editor.clone()),
            custom_views: Rc::default(),
            dialogs: extension_ui::dialogs::DialogSlots::default(),
            editor,
            keybindings: keys,
            footer,
            footer_data_provider,
            operations,
            changelog_markdown: RefCell::default(),
            startup_notices_shown: Cell::new(false),
            is_initialized: Cell::new(false),
            pending_events: RefCell::default(),
            event_initialization_pending: Cell::new(false),
            built_in_header: RefCell::default(),
            custom_header: RefCell::default(),
            custom_footer: RefCell::default(),
            tool_environment,
            fd_path: RefCell::default(),
            autocomplete_provider_wrappers: RefCell::default(),
            tool_output_expanded: Cell::new(false),
            hide_thinking_block: Cell::new(hide_thinking),
            last_status: RefCell::default(),
            pending_tools: RefCell::default(),
            is_bash_mode: Cell::new(false),
            last_escape_time: Cell::new(0),
            active_shell: RefCell::default(),
            pending_shells: RefCell::default(),
            on_input_callback: Rc::default(),
            anthropic_subscription_warning_shown: Cell::new(false),
            compaction_queued_messages: RefCell::default(),
            progress: progress::Retained::default(),
            unsubscribe: RefCell::default(),
            working: working::WorkingState::default(),
            streaming: RefCell::default(),
        });
        extensions::bind_runtime_hooks(&mode);
        Ok(mode)
    }

    /// Installs lifecycle handlers, selects unseen release history, prepares search
    /// programs, then builds the header, adds every container in order, focuses the
    /// editor, binds the transcript shortcuts, installs editor submission and the global
    /// debug key, starts the terminal, binds the current session as `rebind_current_session`
    /// does and replays the session; theme changes then refresh the editor border, and
    /// theme and branch changes request fresh frames.
    /// Finally the footer learns how many providers the model candidates cover.
    /// Calls while initialized do nothing; after `stop`, initialization runs again.
    /// Unix initialization needs a caller-driven Tokio local executor with signals enabled.
    ///
    /// # Errors
    /// Returns a failure to register the termination or output-error handlers, or a failure
    /// of the managed binary directory, the header's theme colours, terminal start, session
    /// binding, replay or catalog refresh.
    pub async fn init(self: &Rc<Self>) -> Result<(), Box<dyn Error>> {
        if self.is_initialized.get() {
            return Ok(());
        }
        self.register_signal_handlers()?;
        *self.changelog_markdown.borrow_mut() = self.get_changelog_for_display()?;
        self.prepare_search_tools().await?;
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
        self.ui
            .set_focus(Some(self.current_editor() as Rc<dyn Component>));
        self.setup_key_handlers();
        self.setup_queue_key_handlers();
        self.setup_editor_submit_handler();
        self.setup_debug_handler();
        self.ui.start()?;
        self.is_initialized.set(true);
        self.rebind_current_session().await?;
        self.render_initial_messages()
            .map_err(|error| -> Box<dyn Error> { error })?;
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
        self.update_available_provider_count()
    }

    /// Clears the transcript, the pending-message rows and the shell rows waiting there, then
    /// replays the current session with the initial replay's footer, history and compaction
    /// effects.
    ///
    /// # Errors
    /// Returns the failure of reading the session context, of a stored record, of a row's
    /// rendering or of the published theme.
    pub fn render_current_session_state(&self) -> Result<(), EventError> {
        self.chat_container.clear();
        self.pending_messages_container.clear();
        self.compaction_queued_messages.borrow_mut().clear();
        self.pending_shells.borrow_mut().clear();
        drop(self.active_shell.take());
        drop(self.streaming.take());
        self.render_initial_messages()
    }

    /// Reports a fatal runtime failure as its transcript notice, draws that frame before
    /// the stop that would cancel it, closes the theme watch and stops the terminal, so
    /// nothing repaints while the process ends.
    fn report_fatal_runtime_error(&self, prefix: &str, error: &dyn Error) {
        let _ = self.show_error(&format!("{prefix}: {error}"));
        let _ = self.ui.render_pending_frame();
        self.themes.stop_theme_watcher();
        let _ = self.ui.stop();
    }

    /// Reports a fatal runtime failure and ends the process with status one.
    pub fn handle_fatal_runtime_error(&self, prefix: &str, error: &dyn Error) -> ! {
        self.report_fatal_runtime_error(prefix, error);
        std::process::exit(1);
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

/// Builds the transcript environment and frame writer with shared image support.
///
/// # Errors
/// Returns the failure of the transcript environment.
fn create_presentation(
    session: &AgentSession,
    terminal: (TerminalHandle, Rc<dyn TuiRuntime>),
    theme: LiveTheme,
) -> Result<(Rc<ToolRenderEnvironment>, Rc<TUI>), Box<dyn Error>> {
    let environment = tool_environment(terminal.1.clone(), theme)?;
    let ui = create_ui(session, terminal, environment.terminal_image.clone());
    Ok((environment, ui))
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

/// The configured keybindings, input editor and its presentation container.
type ConfiguredInput = (KeybindingsManager, Rc<CustomEditor>, Rc<Container>);

/// Builds the configured input editor and its container over the same keybindings.
///
/// # Errors
/// Returns a failure to read the keybinding configuration.
fn create_input(
    session: &AgentSession,
    ui: &TUI,
    theme: LiveTheme,
) -> Result<ConfiguredInput, Box<dyn Error>> {
    let keys = KeybindingsManager::create(None)?;
    maestro_tui::set_keybindings((*keys).clone());
    let editor = create_editor(ui, theme, (*keys).clone(), session.settings_manager());
    let container = Rc::new(Container::new());
    container.add_child(editor.clone());
    Ok((keys, editor, container))
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

/// Builds the footer metadata over the session's directory and `operations`, then the
/// footer over the session, that metadata and the published theme.
///
/// # Errors
/// Returns the failure of the collator reading the locale environment.
fn create_footer(
    session: &Rc<AgentSession>,
    operations: &Rc<dyn FooterOperations>,
    theme: LiveTheme,
) -> Result<(Rc<FooterComponent>, FooterDataProvider), Box<dyn Error>> {
    let data = FooterDataProvider::new(
        session.session_manager().borrow().get_cwd().to_owned(),
        operations.clone(),
    );
    let auto_compact = session.auto_compaction_enabled();
    let footer = Rc::new(FooterComponent::new(
        session.clone(),
        Rc::new(data.clone()),
        theme,
        Rc::new(Collator::new(&|key| std::env::var(key).ok())?),
        Rc::new(|key| std::env::var(key).ok()),
    ));
    footer.set_auto_compact_enabled(auto_compact);
    Ok((footer, data))
}

/// Registers loaded themes, selects the configured theme, then reads thinking visibility.
///
/// # Errors
/// Returns the theme selection failure.
fn init_theme(
    session: &AgentSession,
    themes: &Rc<ThemeState>,
    operations: &Rc<dyn FooterOperations>,
) -> Result<bool, Box<dyn Error>> {
    themes.set_registered_themes(session.resource_loader().get_themes().themes.clone());
    let settings = session.settings_manager();
    let selected = settings.borrow().get_theme();
    themes.init_theme(selected.as_deref(), Some(operations.clone()))?;
    Ok(settings.borrow().get_hide_thinking_block())
}

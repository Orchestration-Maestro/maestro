//! Terminal UI capabilities offered to extension callbacks.
mod callbacks;
mod editor;
mod widgets;
use crate::{EventError, footer_data_provider::ReadonlyFooterDataProvider};
pub use callbacks::{UIHostCall, WeakUIHostCall};
pub use editor::{
    ActionHandlers, CustomEditorComponent, EditorFactoryOutcome, ExtensionShortcutCallback,
};
use maestro_models::{Cancellation, Execution, Local};
use maestro_theme::{LiveTheme, Theme, ThemeInfo};
use maestro_tui::{
    AutocompleteProvider, EditorTheme, OverlayHandle, OverlayOptions, TUI,
    tui::{ComponentHandle, InputListenerResult},
};
use std::{
    cell::{Cell, RefCell},
    ops::Deref,
    rc::Rc,
};
pub use widgets::{ExtensionEditor, ExtensionKeybindings, ExtensionLoader, ExtensionWidgets};

/// A local UI operation that settles with its result or the host's error.
pub type ExtensionUIFuture<T> = <Local as Execution>::Future<'static, Result<T, EventError>>;

/// Options for extension UI dialogs.
#[derive(Clone, Default)]
pub struct ExtensionUIDialogOptions {
    /// Signal that dismisses the dialog programmatically.
    pub signal: Option<Cancellation>,
    /// Timeout in milliseconds, passed to the host unchanged.
    pub timeout: Option<f64>,
}

/// Placement for extension widgets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WidgetPlacement {
    /// Above the editor, the default.
    #[default]
    AboveEditor,
    /// Below the editor.
    BelowEditor,
}

/// Options for extension widgets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExtensionWidgetOptions {
    /// Where the widget is rendered; absent means [`WidgetPlacement::AboveEditor`].
    pub placement: Option<WidgetPlacement>,
}

/// Working indicator configuration for the streaming loader.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorkingIndicatorOptions {
    /// Animation frames, rendered verbatim; an empty list hides the indicator.
    pub frames: Option<Vec<String>>,
    /// Frame interval in milliseconds for animated indicators.
    pub interval_ms: Option<f64>,
}

/// Notification severity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NotificationType {
    /// Informational.
    Info,
    /// A warning.
    Warning,
    /// An error.
    Error,
}

/// A theme selected by name or supplied as an instance.
#[derive(Clone)]
pub enum ThemeSelection {
    /// A theme name to load.
    Name(String),
    /// A theme instance, read when the setter consumes it.
    Theme(LiveTheme),
}

/// The outcome of a theme selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeSetResult {
    /// Whether the theme was applied.
    pub success: bool,
    /// The failure message, when there is one.
    pub error: Option<String>,
}

/// Terminal input listener bound to the facade that authored it.
pub type TerminalInputHandler = UIHostCall<String, InputListenerResult>;
/// Removes a subscription; dropping it does not.
pub type UIUnsubscribe = Rc<dyn Fn() -> Result<(), EventError>>;
/// Explicit disposal of an extension component.
pub type ComponentDisposer = UIHostCall<(), ()>;

/// A component an extension built, with its optional disposal, expansion and width.
pub struct ExtensionComponent {
    /// The retained component.
    pub component: ComponentHandle,
    /// Called once by the host when it removes the component.
    pub dispose: Option<ComponentDisposer>,
    /// Expands or collapses the component, when it is expandable.
    pub set_expanded: Option<UIHostCall<bool, ()>>,
    /// The width an overlay falls back to when no overlay options are supplied.
    pub width: Option<f64>,
}

/// Builds a component from the terminal and the live theme.
pub type ComponentFactory = UIHostCall<(TUI, LiveTheme), ExtensionComponent>;
/// Builds a footer that may read the footer metadata.
pub type FooterFactory =
    UIHostCall<(TUI, LiveTheme, Rc<dyn ReadonlyFooterDataProvider>), ExtensionComponent>;

/// Widget content: fixed lines or a component factory.
#[derive(Clone)]
pub enum WidgetContent {
    /// Lines rendered as they are.
    Lines(Vec<String>),
    /// A component built by the host.
    Factory(ComponentFactory),
}

/// Wraps the current autocomplete provider with additional behavior.
pub type AutocompleteProviderFactory = UIHostCall<
    Rc<dyn AutocompleteProvider<Signal = Cancellation>>,
    Rc<dyn AutocompleteProvider<Signal = Cancellation>>,
>;
/// Builds a replacement editor with the application keybindings.
pub type EditorFactory = UIHostCall<(TUI, EditorTheme, ExtensionKeybindings), EditorFactoryOutcome>;

/// Resolves overlay options each time the host asks for them.
pub type OverlayOptionsResolver = UIHostCall<(), Rc<RefCell<OverlayOptions>>>;
/// Static or resolved overlay options.
#[derive(Clone)]
pub enum OverlayOptionsSource {
    /// Options retained as supplied.
    Static(Rc<RefCell<OverlayOptions>>),
    /// Options resolved by the host when needed.
    Resolver(OverlayOptionsResolver),
}
/// Receives the shown overlay's handle, shared with the host.
pub type OverlayHandleCallback = UIHostCall<Rc<RefCell<Box<dyn OverlayHandle>>>, ()>;

/// Options for a custom component.
#[derive(Clone, Default)]
pub struct CustomUIOptions {
    /// Whether the component is shown as an overlay.
    pub overlay: Option<bool>,
    /// Overlay positioning and sizing.
    pub overlay_options: Option<OverlayOptionsSource>,
    /// Called with the overlay handle after the overlay is shown.
    pub on_handle: Option<OverlayHandleCallback>,
}

/// Completes a typed custom component with its result.
pub type CustomDone<T> = Rc<dyn Fn(T) -> Result<(), EventError>>;
/// Closes a custom component: `true` on the first successful close, `false` once closed.
pub type CustomClose = Rc<dyn Fn() -> Result<bool, EventError>>;

/// A custom component built immediately or asynchronously.
pub enum CustomFactoryOutcome {
    /// The component is ready.
    Ready(ExtensionComponent),
    /// The component settles later.
    Async(ExtensionUIFuture<ExtensionComponent>),
}
/// Builds a typed custom component with the application keybindings.
pub type CustomFactory<T> =
    UIHostCall<(TUI, LiveTheme, ExtensionKeybindings, CustomDone<T>), CustomFactoryOutcome>;
/// Builds a host-closed custom component with the application keybindings.
pub type CustomComponentFactory =
    UIHostCall<(TUI, LiveTheme, ExtensionKeybindings, CustomClose), CustomFactoryOutcome>;

/// Settles at once with `value`.
fn ready<T: 'static>(value: T) -> ExtensionUIFuture<T> {
    Box::pin(std::future::ready(Ok(value)))
}

/// Interactive UI a host provides; each default is the defined headless result.
///
/// Installed hosts override only the operations they support.
pub trait ExtensionUIContextPort {
    /// Shows a selector and returns the chosen option.
    fn select(
        &self,
        _title: String,
        _options: Vec<String>,
        _opts: Option<ExtensionUIDialogOptions>,
    ) -> ExtensionUIFuture<Option<String>> {
        ready(None)
    }
    /// Shows a confirmation dialog.
    fn confirm(
        &self,
        _title: String,
        _message: String,
        _opts: Option<ExtensionUIDialogOptions>,
    ) -> ExtensionUIFuture<bool> {
        ready(false)
    }
    /// Shows a text input dialog.
    fn input(
        &self,
        _title: String,
        _placeholder: Option<String>,
        _opts: Option<ExtensionUIDialogOptions>,
    ) -> ExtensionUIFuture<Option<String>> {
        ready(None)
    }
    /// Shows a notification.
    ///
    /// # Errors
    /// Returns the host's failure.
    fn notify(&self, _message: String, _kind: Option<NotificationType>) -> Result<(), EventError> {
        Ok(())
    }
    /// Listens to raw terminal input; the result removes the listener.
    ///
    /// # Errors
    /// Returns the host's failure.
    fn on_terminal_input(
        &self,
        _handler: TerminalInputHandler,
    ) -> Result<UIUnsubscribe, EventError> {
        Ok(Rc::new(|| Ok(())))
    }
    /// Sets or clears footer status text.
    ///
    /// # Errors
    /// Returns the host's failure.
    fn set_status(&self, _key: String, _text: Option<String>) -> Result<(), EventError> {
        Ok(())
    }
    /// Sets the working message, or restores the default with `None`.
    ///
    /// # Errors
    /// Returns the host's failure.
    fn set_working_message(&self, _message: Option<String>) -> Result<(), EventError> {
        Ok(())
    }
    /// Shows or hides the working loader row.
    ///
    /// # Errors
    /// Returns the host's failure.
    fn set_working_visible(&self, _visible: bool) -> Result<(), EventError> {
        Ok(())
    }
    /// Configures the working indicator, or restores the default with `None`.
    ///
    /// # Errors
    /// Returns the host's failure.
    fn set_working_indicator(
        &self,
        _options: Option<WorkingIndicatorOptions>,
    ) -> Result<(), EventError> {
        Ok(())
    }
    /// Sets the hidden-thinking label, or restores the default with `None`.
    ///
    /// # Errors
    /// Returns the host's failure.
    fn set_hidden_thinking_label(&self, _label: Option<String>) -> Result<(), EventError> {
        Ok(())
    }
    /// Sets or removes a widget.
    ///
    /// # Errors
    /// Returns the host's or factory's failure.
    fn set_widget(
        &self,
        _key: String,
        _content: Option<WidgetContent>,
        _options: Option<ExtensionWidgetOptions>,
    ) -> Result<(), EventError> {
        Ok(())
    }
    /// Sets a custom footer, or restores the built-in one with `None`.
    ///
    /// # Errors
    /// Returns the host's or factory's failure.
    fn set_footer(&self, _factory: Option<FooterFactory>) -> Result<(), EventError> {
        Ok(())
    }
    /// Sets a custom header, or restores the built-in one with `None`.
    ///
    /// # Errors
    /// Returns the host's or factory's failure.
    fn set_header(&self, _factory: Option<ComponentFactory>) -> Result<(), EventError> {
        Ok(())
    }
    /// Sets the terminal title.
    ///
    /// # Errors
    /// Returns the host's failure.
    fn set_title(&self, _title: String) -> Result<(), EventError> {
        Ok(())
    }
    /// Shows a custom component with keyboard focus; settles when it closes.
    fn custom_component(
        &self,
        _factory: CustomComponentFactory,
        _options: Option<CustomUIOptions>,
    ) -> ExtensionUIFuture<()> {
        ready(())
    }
    /// Pastes text into the editor.
    ///
    /// # Errors
    /// Returns the host's failure.
    fn paste_to_editor(&self, _text: String) -> Result<(), EventError> {
        Ok(())
    }
    /// Sets the editor text.
    ///
    /// # Errors
    /// Returns the host's failure.
    fn set_editor_text(&self, _text: String) -> Result<(), EventError> {
        Ok(())
    }
    /// Returns the editor text.
    ///
    /// # Errors
    /// Returns the host's failure.
    fn get_editor_text(&self) -> Result<String, EventError> {
        Ok(String::new())
    }
    /// Shows a multi-line editor.
    fn editor(
        &self,
        _title: String,
        _prefill: Option<String>,
    ) -> ExtensionUIFuture<Option<String>> {
        ready(None)
    }
    /// Stacks behavior on top of the current autocomplete provider.
    ///
    /// # Errors
    /// Returns the host's or factory's failure.
    fn add_autocomplete_provider(
        &self,
        _factory: AutocompleteProviderFactory,
    ) -> Result<(), EventError> {
        Ok(())
    }
    /// Sets a custom editor factory, or restores the default with `None`.
    ///
    /// # Errors
    /// Returns the host's or factory's failure.
    fn set_editor_component(&self, _factory: Option<EditorFactory>) -> Result<(), EventError> {
        Ok(())
    }
    /// Returns the custom editor factory, if one is set.
    ///
    /// # Errors
    /// Returns the host's failure.
    fn get_editor_component(&self) -> Result<Option<EditorFactory>, EventError> {
        Ok(None)
    }
    /// The live theme; reading it before publication fails.
    fn theme(&self) -> LiveTheme;
    /// Returns every available theme name and path.
    ///
    /// # Errors
    /// Returns the host's failure.
    fn get_all_themes(&self) -> Result<Vec<ThemeInfo>, EventError> {
        Ok(Vec::new())
    }
    /// Loads a theme by name without switching to it.
    ///
    /// # Errors
    /// Returns the host's failure.
    fn get_theme(&self, _name: String) -> Result<Option<Rc<Theme>>, EventError> {
        Ok(None)
    }
    /// Selects a theme by name or instance.
    ///
    /// # Errors
    /// Returns the host's failure.
    fn set_theme(&self, _theme: ThemeSelection) -> Result<ThemeSetResult, EventError> {
        Ok(ThemeSetResult {
            success: false,
            error: Some("UI not available".into()),
        })
    }
    /// Returns the tool output expansion state.
    ///
    /// # Errors
    /// Returns the host's failure.
    fn get_tools_expanded(&self) -> Result<bool, EventError> {
        Ok(false)
    }
    /// Sets the tool output expansion state.
    ///
    /// # Errors
    /// Returns the host's failure.
    fn set_tools_expanded(&self, _expanded: bool) -> Result<(), EventError> {
        Ok(())
    }
}

/// Completes through the host's close and commits the value only when that close is accepted.
fn committing_done<T: 'static>(close: CustomClose, slot: Rc<Cell<Option<T>>>) -> CustomDone<T> {
    Rc::new(move |value| {
        if close()? {
            slot.set(Some(value));
        }
        Ok(())
    })
}

/// How an installed UI host is retained by a session and its runner.
#[derive(Clone)]
pub enum ExtensionUIContextBinding {
    /// An independently retained SDK host.
    Owned(ExtensionUIContext),
    /// A host whose containing owner already retains the session.
    Borrowed {
        /// The non-owning host reference.
        port: std::rc::Weak<dyn ExtensionUIContextPort>,
        /// Whether this was the headless fallback.
        fallback: bool,
    },
}
impl ExtensionUIContextBinding {
    /// Borrows a host without retaining its containing owner.
    #[must_use]
    pub fn borrowed(context: &ExtensionUIContext) -> Self {
        Self::Borrowed {
            port: Rc::downgrade(&context.port),
            fallback: context.fallback,
        }
    }
    /// Obtains an independently retained context while its host is still live.
    pub(crate) fn get(&self) -> Option<ExtensionUIContext> {
        match self {
            Self::Owned(context) => Some(context.clone()),
            Self::Borrowed { port, fallback } => port.upgrade().map(|port| ExtensionUIContext {
                port,
                fallback: *fallback,
                error_reporter: None,
            }),
        }
    }
    /// Whether the installed host is live and is not a headless fallback.
    pub(crate) fn has_ui(&self) -> bool {
        match self {
            Self::Owned(context) => !context.fallback,
            Self::Borrowed { port, fallback } => !fallback && port.strong_count() != 0,
        }
    }
}
impl From<ExtensionUIContext> for ExtensionUIContextBinding {
    fn from(context: ExtensionUIContext) -> Self {
        Self::Owned(context)
    }
}

/// An owned handle to the UI host that was current when it was obtained.
#[derive(Clone)]
pub struct ExtensionUIContext {
    /// The retained host.
    pub(crate) port: Rc<dyn ExtensionUIContextPort>,
    /// Whether this is the headless fallback rather than an installed host.
    pub(crate) fallback: bool,
    /// Attributed errors use the originating runner's live listener owner.
    pub(crate) error_reporter: Option<super::ExtensionErrorListener>,
}

impl Deref for ExtensionUIContext {
    type Target = dyn ExtensionUIContextPort;
    fn deref(&self) -> &Self::Target {
        &*self.port
    }
}

impl ExtensionUIContext {
    /// Retains an installed host.
    #[must_use]
    pub fn new(port: Rc<dyn ExtensionUIContextPort>) -> Self {
        Self {
            port,
            fallback: false,
            error_reporter: None,
        }
    }
    /// Shows a component using the host's accepted-close operation.
    ///
    /// The returned future settles when the host closes the component or reports its failure.
    #[must_use]
    pub fn custom_component(
        &self,
        factory: CustomComponentFactory,
        options: Option<CustomUIOptions>,
    ) -> ExtensionUIFuture<()> {
        self.port.custom_component(factory, options)
    }

    /// Shows a custom component; settles with the first committed result.
    ///
    /// `done` commits only after the host accepts the first close. A committed
    /// value wins over a later failure; otherwise the host's error is returned,
    /// or `None` when the component closed without a value.
    pub fn custom<T: 'static>(
        &self,
        factory: CustomFactory<T>,
        options: Option<CustomUIOptions>,
    ) -> ExtensionUIFuture<Option<T>> {
        let slot = Rc::new(Cell::new(None));
        let result = Rc::clone(&slot);
        let component: CustomComponentFactory =
            UIHostCall::from_host(move |(tui, theme, keybindings, close)| {
                let done = committing_done(close, Rc::clone(&slot));
                factory.call((tui, theme, keybindings, done))
            });
        let closed = self.custom_component(component, options);
        Box::pin(async move {
            let outcome = closed.await;
            match result.take() {
                Some(value) => Ok(Some(value)),
                None => outcome.map(|()| None),
            }
        })
    }
}

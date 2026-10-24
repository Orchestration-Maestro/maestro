//! Terminal UI capabilities offered to extension callbacks.
use crate::{CallbackEmitter, EventError, footer_data_provider::ReadonlyFooterDataProvider};
use maestro_models::{Cancellation, Execution, Local};
use maestro_theme::{LiveTheme, Theme, ThemeInfo};
use maestro_tui::{
    AutocompleteProvider, EditorComponent, EditorTheme, KeybindingsManager, OverlayHandle,
    OverlayOptions, TUI,
    tui::{ComponentHandle, InputListenerResult},
};
use std::{
    cell::{Cell, RefCell},
    ops::Deref,
    rc::Rc,
};

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

/// Raw terminal input listener.
pub type TerminalInputHandler = Rc<
    dyn for<'a> Fn(String, CallbackEmitter<'a, Local>) -> Result<InputListenerResult, EventError>,
>;
/// Removes a subscription; dropping it does not.
pub type UIUnsubscribe = Rc<dyn Fn() -> Result<(), EventError>>;
/// Explicit disposal of an extension component.
pub type ComponentDisposer =
    Rc<dyn for<'a> Fn(CallbackEmitter<'a, Local>) -> Result<(), EventError>>;

/// A component an extension built, with its optional disposal.
pub struct ExtensionComponent {
    /// The retained component.
    pub component: ComponentHandle,
    /// Called once by the host when it removes the component.
    pub dispose: Option<ComponentDisposer>,
}

/// Builds a component from the terminal and the live theme.
pub type ComponentFactory = Rc<
    dyn for<'a> Fn(
        TUI,
        LiveTheme,
        CallbackEmitter<'a, Local>,
    ) -> Result<ExtensionComponent, EventError>,
>;
/// Builds a footer that may read the footer metadata.
pub type FooterFactory = Rc<
    dyn for<'a> Fn(
        TUI,
        LiveTheme,
        Rc<dyn ReadonlyFooterDataProvider>,
        CallbackEmitter<'a, Local>,
    ) -> Result<ExtensionComponent, EventError>,
>;

/// Widget content: fixed lines or a component factory.
#[derive(Clone)]
pub enum WidgetContent {
    /// Lines rendered as they are.
    Lines(Vec<String>),
    /// A component built by the host.
    Factory(ComponentFactory),
}

/// Wraps the current autocomplete provider with additional behavior.
pub type AutocompleteProviderFactory = Rc<
    dyn for<'a> Fn(
        Rc<dyn AutocompleteProvider<Signal = Cancellation>>,
        CallbackEmitter<'a, Local>,
    ) -> Result<Rc<dyn AutocompleteProvider<Signal = Cancellation>>, EventError>,
>;
/// Builds a replacement editor.
pub type EditorFactory = Rc<
    dyn for<'a> Fn(
        TUI,
        EditorTheme,
        KeybindingsManager,
        CallbackEmitter<'a, Local>,
    ) -> Result<Rc<dyn EditorComponent<Signal = Cancellation>>, EventError>,
>;

/// Resolves overlay options each time the host asks for them.
pub type OverlayOptionsResolver = Rc<
    dyn for<'a> Fn(CallbackEmitter<'a, Local>) -> Result<Rc<RefCell<OverlayOptions>>, EventError>,
>;
/// Static or resolved overlay options.
#[derive(Clone)]
pub enum OverlayOptionsSource {
    /// Options retained as supplied.
    Static(Rc<RefCell<OverlayOptions>>),
    /// Options resolved by the host when needed.
    Resolver(OverlayOptionsResolver),
}
/// Receives the shown overlay's handle, shared with the host.
pub type OverlayHandleCallback = Rc<
    dyn for<'a> Fn(
        Rc<RefCell<Box<dyn OverlayHandle>>>,
        CallbackEmitter<'a, Local>,
    ) -> Result<(), EventError>,
>;

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
/// Builds a typed custom component.
pub type CustomFactory<T> = Rc<
    dyn for<'a> Fn(
        TUI,
        LiveTheme,
        KeybindingsManager,
        CustomDone<T>,
        CallbackEmitter<'a, Local>,
    ) -> Result<CustomFactoryOutcome, EventError>,
>;
/// Builds a custom component that the host closes.
pub type CustomComponentFactory = Rc<
    dyn for<'a> Fn(
        TUI,
        LiveTheme,
        KeybindingsManager,
        CustomClose,
        CallbackEmitter<'a, Local>,
    ) -> Result<CustomFactoryOutcome, EventError>,
>;

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

/// An owned handle to the UI host that was current when it was obtained.
#[derive(Clone)]
pub struct ExtensionUIContext {
    /// The retained host.
    pub(crate) port: Rc<dyn ExtensionUIContextPort>,
    /// Whether this is the headless fallback rather than an installed host.
    pub(crate) fallback: bool,
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
        }
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
            Rc::new(move |tui, theme, keybindings, close, events| {
                let done = committing_done(close, Rc::clone(&slot));
                factory(tui, theme, keybindings, done, events)
            });
        let closed = self.port.custom_component(component, options);
        Box::pin(async move {
            let outcome = closed.await;
            match result.take() {
                Some(value) => Ok(Some(value)),
                None => outcome.map(|()| None),
            }
        })
    }
}

//! Real terminal objects, themes and rejecting callbacks for UI capability tests.
#![cfg(test)]
use crate::local_support::{LocalBus, Released, Trace, loaded, mark};
use crate::manual_runtime::ManualRuntime;
use crate::recording_terminal::RecordingTerminal;
use maestro_extensions::footer_data_provider::live::Live;
use maestro_extensions::footer_data_provider::{ExtensionStatuses, ReadonlyFooterDataProvider};
use maestro_extensions::types::{
    ComponentFactory, CustomFactory, ExtensionComponent, ExtensionUIContextPort, FooterFactory,
    OverlayHandleCallback, OverlayOptionsResolver, UIUnsubscribe,
};
use maestro_extensions::{
    AutocompleteProviderFactory, EditorFactory, EventError, ExtensionAPI, TerminalInputHandler,
};
use maestro_models::{Cancellation, Local};
use maestro_theme::LiveTheme;
use maestro_tui::autocomplete::{
    CompletionError, CompletionOptions, CompletionResult, CursorPosition,
};
use maestro_tui::tui::{InputListener, InputListenerResult};
use maestro_tui::{
    AutocompleteItem, AutocompleteProvider, AutocompleteSuggestions, TUI, TerminalImage,
};
use std::{cell::RefCell, future::Future, pin::Pin, rc::Rc};

/// A frame writer over a recording terminal and a manual runtime.
pub fn screen() -> (TUI, RecordingTerminal) {
    let terminal = RecordingTerminal::new(40, 12);
    let tui = TUI::new(
        terminal.handle(),
        ManualRuntime::new().handle(),
        TerminalImage::default(),
        Some(false),
    );
    (tui, terminal)
}

/// Builds a component through the callback's own originating capability.
pub fn build(
    tui: &TUI,
    theme: &LiveTheme,
    factory: &ComponentFactory,
) -> Result<ExtensionComponent, EventError> {
    factory.call((tui.clone(), theme.clone()))
}

/// Disposes a removed component through its bound callback.
pub fn dispose(component: Option<ExtensionComponent>) {
    if let Some(dispose) = component.and_then(|component| component.dispose) {
        dispose.call(()).unwrap();
    }
}

/// Callbacks that record any invocation and reject it.
pub struct Rejecting {
    /// Recorded invocation labels.
    pub trace: Trace,
    /// The facade every rejecting callback is bound to.
    api: ExtensionAPI<Local>,
}

impl Rejecting {
    /// Rejecting callbacks bound to a fresh local facade over its own bus.
    pub fn new(trace: Trace) -> Self {
        let bus = LocalBus::new();
        let api = loaded("/rejecting", bus.controller.for_extension()).api;
        Self { trace, api }
    }
    /// A rejecting callback recording `label`.
    fn reject(&self, label: &'static str) -> impl Fn() -> EventError + 'static {
        let trace = self.trace.clone();
        move || {
            mark(&trace, label);
            "rejected".into()
        }
    }
    /// A rejecting component factory.
    pub fn component(&self) -> ComponentFactory {
        let reject = self.reject("component");
        self.api.ui_callback(move |(_, _), _| Err(reject()))
    }
    /// A rejecting footer factory.
    pub fn footer(&self) -> FooterFactory {
        let reject = self.reject("footer");
        self.api.ui_callback(move |(_, _, _), _| Err(reject()))
    }
    /// A rejecting editor factory.
    pub fn editor(&self) -> EditorFactory {
        let reject = self.reject("editor");
        self.api.ui_callback(move |(_, _, _), _| Err(reject()))
    }
    /// A rejecting autocomplete wrapper.
    pub fn autocomplete(&self) -> AutocompleteProviderFactory {
        let reject = self.reject("autocomplete");
        self.api.ui_callback(move |_, _| Err(reject()))
    }
    /// A rejecting terminal input handler.
    pub fn input(&self) -> TerminalInputHandler {
        let reject = self.reject("input");
        self.api.ui_callback(move |_, _| Err(reject()))
    }
    /// A rejecting custom component factory.
    pub fn custom(&self) -> CustomFactory<String> {
        let reject = self.reject("custom");
        self.api.ui_callback(move |(_, _, _, _), _| Err(reject()))
    }
    /// A rejecting overlay option resolver.
    pub fn resolver(&self) -> OverlayOptionsResolver {
        let reject = self.reject("resolver");
        self.api.ui_callback(move |(), _| Err(reject()))
    }
    /// A rejecting overlay handle callback.
    pub fn on_handle(&self) -> OverlayHandleCallback {
        let reject = self.reject("on_handle");
        self.api.ui_callback(move |_, _| Err(reject()))
    }
}

/// The base provider: absence, empty and ordered suggestions, and cancellation.
pub struct Words;
impl AutocompleteProvider for Words {
    type Signal = Cancellation;
    fn get_suggestions<'a>(
        &'a self,
        lines: &'a [String],
        _: CursorPosition,
        options: CompletionOptions<'a, Cancellation>,
    ) -> Pin<Box<dyn Future<Output = Result<Option<AutocompleteSuggestions>, CompletionError>> + 'a>>
    {
        let item = |value: &str| AutocompleteItem {
            value: value.into(),
            label: value.into(),
            description: None,
        };
        let result = match lines[0].as_str() {
            _ if options.signal.is_aborted() => Err("cancelled".into()),
            "" => Ok(None),
            "none" => Ok(Some(AutocompleteSuggestions {
                items: Vec::new(),
                prefix: "none".into(),
            })),
            word => Ok(Some(AutocompleteSuggestions {
                items: vec![item("b"), item("a")],
                prefix: word.into(),
            })),
        };
        Box::pin(std::future::ready(result))
    }
    fn apply_completion(
        &self,
        _: &[String],
        _: CursorPosition,
        item: &AutocompleteItem,
        prefix: &str,
    ) -> CompletionResult {
        CompletionResult {
            lines: vec![format!("{prefix}->{}", item.value)],
            cursor_line: 0,
            cursor_col: 0,
        }
    }
}

/// Footer metadata whose removers keep its watch owner alive, as the application owner does.
pub struct Metadata {
    /// The live status texts.
    pub statuses: ExtensionStatuses,
    /// Branch-change callbacks by identity.
    pub callbacks: Live<usize, Rc<dyn Fn()>>,
    /// The watch lifecycle retained by removers.
    watch: Rc<Released>,
}
impl Metadata {
    /// Metadata over fresh live statuses and a watch owner released with its removers.
    pub fn new(watch: Rc<Released>) -> Self {
        Self {
            statuses: ExtensionStatuses::new(),
            callbacks: Live::default(),
            watch,
        }
    }
}
impl ReadonlyFooterDataProvider for Metadata {
    fn get_git_branch(&self) -> Option<String> {
        Some("main".into())
    }
    fn get_extension_statuses(&self) -> ExtensionStatuses {
        self.statuses.clone()
    }
    fn get_available_provider_count(&self) -> f64 {
        2.5
    }
    fn on_branch_change(&self, callback: Rc<dyn Fn()>) -> Box<dyn Fn()> {
        let identity = Rc::as_ptr(&callback).addr();
        self.callbacks.insert(identity, callback);
        let (callbacks, watch) = (self.callbacks.clone(), Rc::clone(&self.watch));
        Box::new(move || {
            let _ = &watch;
            callbacks.remove(&identity);
        })
    }
}

/// A host routing extension input listeners through the writer's own listeners.
pub struct Keyboard {
    /// The real writer.
    pub tui: TUI,
    /// Original handler failures the host received.
    pub errors: Rc<RefCell<Vec<EventError>>>,
}
impl Keyboard {
    /// A host over `tui` recording handler failures.
    pub fn new(tui: TUI) -> Self {
        Self {
            tui,
            errors: Rc::default(),
        }
    }
}
impl ExtensionUIContextPort for Keyboard {
    fn on_terminal_input(
        &self,
        handler: TerminalInputHandler,
    ) -> Result<UIUnsubscribe, EventError> {
        let errors = self.errors.clone();
        let listener: InputListener = Rc::new(move |data: &str| {
            handler.call(data.to_owned()).unwrap_or_else(|error| {
                errors.borrow_mut().push(error);
                InputListenerResult::Pass
            })
        });
        let remove = self.tui.add_input_listener(listener);
        Ok(Rc::new(move || {
            remove();
            Ok(())
        }))
    }
    fn theme(&self) -> LiveTheme {
        LiveTheme::default()
    }
}

//! Real terminal objects, themes and rejecting callbacks for UI capability tests.
#![cfg(test)]
use crate::local_support::{Trace, mark};
use crate::manual_runtime::ManualRuntime;
use crate::recording_terminal::RecordingTerminal;
use maestro_extensions::types::{
    ComponentFactory, CustomFactory, ExtensionComponent, FooterFactory, OverlayHandleCallback,
    OverlayOptionsResolver,
};
use maestro_extensions::{
    AutocompleteProviderFactory, EditorFactory, EventBus, EventError, TerminalInputHandler,
};
use maestro_models::Local;
use maestro_theme::LiveTheme;
use maestro_tui::{TUI, TerminalImage};
use std::rc::Rc;

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

/// Builds a component through the extension's callback boundary.
pub fn build(
    events: &EventBus<Local>,
    tui: &TUI,
    theme: &LiveTheme,
    factory: &ComponentFactory,
) -> Result<ExtensionComponent, EventError> {
    events.invoke_callback(|emitter| factory(tui.clone(), theme.clone(), emitter))
}

/// Disposes a removed component through the extension's callback boundary.
pub fn dispose(events: &EventBus<Local>, component: Option<ExtensionComponent>) {
    if let Some(dispose) = component.and_then(|component| component.dispose) {
        events.invoke_callback(|emitter| dispose(emitter)).unwrap();
    }
}

/// Callbacks that record any invocation and reject it.
pub struct Rejecting(pub Trace);

impl Rejecting {
    /// A rejecting callback recording `label`.
    fn reject(&self, label: &'static str) -> impl Fn() -> EventError + 'static {
        let trace = self.0.clone();
        move || {
            mark(&trace, label);
            "rejected".into()
        }
    }
    /// A rejecting component factory.
    pub fn component(&self) -> ComponentFactory {
        let reject = self.reject("component");
        Rc::new(move |_, _, _| Err(reject()))
    }
    /// A rejecting footer factory.
    pub fn footer(&self) -> FooterFactory {
        let reject = self.reject("footer");
        Rc::new(move |_, _, _, _| Err(reject()))
    }
    /// A rejecting editor factory.
    pub fn editor(&self) -> EditorFactory {
        let reject = self.reject("editor");
        Rc::new(move |_, _, _, _| Err(reject()))
    }
    /// A rejecting autocomplete wrapper.
    pub fn autocomplete(&self) -> AutocompleteProviderFactory {
        let reject = self.reject("autocomplete");
        Rc::new(move |_, _| Err(reject()))
    }
    /// A rejecting terminal input handler.
    pub fn input(&self) -> TerminalInputHandler {
        let reject = self.reject("input");
        Rc::new(move |_, _| Err(reject()))
    }
    /// A rejecting custom component factory.
    pub fn custom(&self) -> CustomFactory<String> {
        let reject = self.reject("custom");
        Rc::new(move |_, _, _, _, _| Err(reject()))
    }
    /// A rejecting overlay option resolver.
    pub fn resolver(&self) -> OverlayOptionsResolver {
        let reject = self.reject("resolver");
        Rc::new(move |_| Err(reject()))
    }
    /// A rejecting overlay handle callback.
    pub fn on_handle(&self) -> OverlayHandleCallback {
        let reject = self.reject("on_handle");
        Rc::new(move |_, _| Err(reject()))
    }
}

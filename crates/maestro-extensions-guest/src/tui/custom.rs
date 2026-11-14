//! Custom components retain typed guest results without serializing them through the host.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::{ExtensionComponent, KeybindingsManager, OverlayHandle, OverlayOptionsHandle, TUI};
use crate::{CallbackEmitter, ExtensionFuture, ExtensionResult, ExtensionUIContext, Theme};
use std::{cell::Cell, rc::Rc};

/// Commits a typed result after the host accepts closing the custom component.
pub type CustomDone<T> = Rc<dyn Fn(T) -> ExtensionResult<()>>;
/// Closes the component, returning true only for the first accepted close.
pub type CustomClose = Rc<dyn Fn() -> ExtensionResult<bool>>;

/// A component built now or by an independently owned future.
pub enum CustomFactoryOutcome {
    /// A completed synchronous factory.
    Ready(ExtensionComponent),
    /// A future that owns its captures, without retaining the borrowed callback emitter.
    Async(ExtensionFuture<'static, ExtensionComponent>),
}

/// Builds a component using a typed completion function and callback-scoped emission.
pub type CustomFactory<T> = Rc<
    dyn for<'a> Fn(
        TUI,
        Theme,
        KeybindingsManager,
        CustomDone<T>,
        CallbackEmitter<'a>,
    ) -> ExtensionResult<CustomFactoryOutcome>,
>;
/// Builds a component using the host's close operation.
pub type CustomComponentFactory = Rc<
    dyn for<'a> Fn(
        TUI,
        Theme,
        KeybindingsManager,
        CustomClose,
        CallbackEmitter<'a>,
    ) -> ExtensionResult<CustomFactoryOutcome>,
>;

/// Resolves an overlay's placement when the host shows it, with callback-scoped emission.
pub type OverlayOptionsResolver =
    Rc<dyn for<'a> Fn(CallbackEmitter<'a>) -> ExtensionResult<OverlayOptionsHandle>>;
/// Receives a shown overlay's handle, with callback-scoped emission.
pub type OverlayHandleCallback =
    Rc<dyn for<'a> Fn(OverlayHandle, CallbackEmitter<'a>) -> ExtensionResult<()>>;

/// Overlay placement: a retained record, or one resolved when the overlay is shown.
#[derive(Clone)]
pub enum OverlayOptionsSource {
    /// Placement shared with the host as supplied.
    Static(OverlayOptionsHandle),
    /// Placement the host asks for after the component is built.
    Resolver(OverlayOptionsResolver),
}

/// Host-owned presentation options for a custom component.
#[derive(Clone, Default)]
pub struct CustomUIOptions {
    /// Whether to show an overlay rather than replacing the editor.
    pub overlay: Option<bool>,
    /// Overlay placement, shared with the host.
    pub overlay_options: Option<OverlayOptionsSource>,
    /// Called with the overlay's handle after the overlay is shown.
    pub on_handle: Option<OverlayHandleCallback>,
}

impl ExtensionUIContext {
    /// Shows a custom component and returns its first committed guest value.
    ///
    /// The value stays in the guest, so it need not be serializable. A committed value wins
    /// over a later host failure. Closing without a value returns `None`.
    pub fn custom<T: 'static>(
        &self,
        factory: CustomFactory<T>,
        options: Option<CustomUIOptions>,
    ) -> ExtensionFuture<'static, Option<T>> {
        let value = Rc::new(Cell::new(None));
        let result = value.clone();
        let component: CustomComponentFactory = Rc::new(move |tui, theme, keys, close, emitter| {
            let done = committing_done(close, value.clone());
            factory(tui, theme, keys, done, emitter)
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

/// Commits a guest value only after the original host accepts closing the component.
fn committing_done<T: 'static>(close: CustomClose, value: Rc<Cell<Option<T>>>) -> CustomDone<T> {
    Rc::new(move |result| {
        if close()? {
            value.set(Some(result));
        }
        Ok(())
    })
}

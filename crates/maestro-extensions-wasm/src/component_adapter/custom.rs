//! Custom factory prefixes and their separately owned asynchronous continuations.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::{
    Exports, Imports, callbacks, callbacks::Scoped, factories::Terminal, lists::WidgetHandler,
    renderers::with_emitter, widget_imports::WidgetImports,
};
use crate::{
    CustomComponentFactory, CustomFactoryOutcome, CustomUIOptions, ExtensionResult,
    OverlayOptionsSource,
};
use std::rc::Rc;

/// A placement operand: the shared record, or the identity of an authored resolver.
pub(crate) enum OptionsSourceRef<'a, I: Imports> {
    /// Shared static placement.
    Placement(&'a I::OverlayOptions),
    /// An authored resolver, staged for the operation.
    Resolver(&'a I::Callback),
}

/// Presentation operands borrow the existing retained options owner.
pub(crate) struct CustomOptionsRef<'a, I: Imports> {
    /// Whether this is an overlay.
    pub(crate) overlay: Option<bool>,
    /// Placement, preserving its absence.
    pub(crate) overlay_options: Option<OptionsSourceRef<'a, I>>,
    /// The identity of the authored handle receiver.
    pub(crate) on_handle: Option<&'a I::Callback>,
}

/// Placement retained for one custom operation.
enum Placement<I: Imports> {
    /// The host's shared record.
    Shared(Rc<super::overlays::Options<I>>),
    /// An authored resolver under an operation-scoped identity.
    Resolver(Scoped<I>),
}

/// Presentation options retained, with their authored callbacks staged, until the custom
/// operation ends.
pub(crate) struct Retained<I: Imports> {
    /// Whether this is an overlay.
    overlay: Option<bool>,
    /// Placement, preserving its absence.
    placement: Option<Placement<I>>,
    /// The authored handle receiver.
    on_handle: Option<Scoped<I>>,
}

impl<I: Imports> Retained<I> {
    /// Stages the callbacks of `options` and finds the host record a static placement names.
    ///
    /// # Errors
    /// Returns a message when the placement belongs to another host.
    pub(crate) fn new(imports: &I, options: CustomUIOptions) -> ExtensionResult<Self> {
        let placement = match options.overlay_options {
            Some(OverlayOptionsSource::Static(options)) => Some(Placement::Shared(
                options
                    .0
                    .host::<super::overlays::Options<I>>()
                    .ok_or_else(|| "overlay options belong to another host".to_owned())?,
            )),
            Some(OverlayOptionsSource::Resolver(resolve)) => Some(Placement::Resolver(
                Scoped::new(imports, Rc::new(WidgetHandler::OverlayOptions(resolve))),
            )),
            None => None,
        };
        let on_handle = options
            .on_handle
            .map(|receive| Scoped::new(imports, Rc::new(WidgetHandler::OverlayHandle(receive))));
        Ok(Self {
            overlay: options.overlay,
            placement,
            on_handle,
        })
    }
    /// The operands naming the retained options.
    pub(crate) fn operands(&self) -> CustomOptionsRef<'_, I> {
        CustomOptionsRef {
            overlay: self.overlay,
            overlay_options: self.placement.as_ref().map(|placement| match placement {
                Placement::Shared(options) => OptionsSourceRef::Placement(&options.resource),
                Placement::Resolver(resolve) => OptionsSourceRef::Resolver(&resolve.handle),
            }),
            on_handle: self.on_handle.as_ref().map(|receive| &receive.handle),
        }
    }
}

/// The capabilities lent to one custom factory prefix.
pub(crate) struct Lent<I: WidgetImports> {
    /// Original terminal.
    pub(crate) tui: I::Tui,
    /// Live theme.
    pub(crate) theme: I::Theme,
    /// Original keybindings manager.
    pub(crate) keybindings: I::Keybindings,
    /// Accepted-close operation.
    pub(crate) close: I::CustomClose,
}

impl<I: WidgetImports> Exports<I> {
    /// Runs only the synchronous factory prefix under its borrowed emission frame.
    pub(crate) fn invoke_custom_factory(
        &self,
        handler: u32,
        lent: Lent<I>,
        emitter: &I::Emitter,
    ) -> ExtensionResult<CustomFactoryOutcome> {
        let tui = Terminal::lent(&self.imports, lent.tui);
        let theme = super::theme::theme(self.imports.clone(), lent.theme);
        let keys = super::keys::lend(&self.imports, lent.keybindings);
        let imports = self.imports.clone();
        let close = Rc::new(lent.close);
        let close = Rc::new(move || imports.custom_close(&close));
        let factory = callbacks::find::<CustomComponentFactory>(handler)?;
        with_emitter(&self.imports, emitter, |emitter| {
            factory(tui, theme, keys, close, emitter)
        })
    }
}

/// Awaits an owned continuation without keeping its prefix's borrowed emitter.
#[cfg(target_arch = "wasm32")]
pub(crate) async fn invoke_custom_future(
    handler: u32,
) -> ExtensionResult<crate::ExtensionComponent> {
    callbacks::take::<crate::ExtensionFuture<'static, crate::ExtensionComponent>>(handler)?.await
}

/// Transfers the completed component or the one-shot owned continuation.
#[cfg(target_arch = "wasm32")]
pub(super) fn transfer<
    I: Imports<Callback = crate::bindings::maestro::extension::host::Callback>,
>(
    imports: &I,
    outcome: CustomFactoryOutcome,
) -> crate::bindings::maestro::extension::host::CustomBuilt {
    use crate::bindings::maestro::extension::host::CustomBuilt;
    match outcome {
        CustomFactoryOutcome::Ready(built) => {
            CustomBuilt::Ready(super::factories::transfer(imports, built))
        }
        CustomFactoryOutcome::Async(future) => {
            CustomBuilt::Pending(callbacks::Identity::<I>::new(imports).transfer(future))
        }
    }
}

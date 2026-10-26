//! Component factories and disposals the host runs with a lent terminal and theme.
use std::rc::Rc;

use super::renderers::with_emitter;
use super::{Exports, Imports, callbacks};
use crate::{ComponentFactory, ExtensionComponent, ExtensionResult, TUI, TuiPort};

/// Host terminal behind the facade.
struct Terminal<I: Imports> {
    /// The host the terminal belongs to.
    _imports: I,
    /// The lent host terminal.
    _tui: I::Tui,
}

impl<I: Imports> TuiPort for Terminal<I> {}

impl<I: Imports> Exports<I> {
    /// Runs the factory under this identity with the lent terminal and theme.
    pub(crate) fn invoke_component_factory(
        &self,
        handler: u32,
        tui: I::Tui,
        theme: I::Theme,
        emitter: &I::Emitter,
    ) -> ExtensionResult<ExtensionComponent> {
        let factory = callbacks::find::<ComponentFactory>(handler)?;
        let tui = TUI::new(Rc::new(Terminal {
            _imports: self.imports.clone(),
            _tui: tui,
        }));
        let theme = super::theme::theme(self.imports.clone(), theme);
        with_emitter(&self.imports, emitter, |emitter| {
            factory(tui, theme, emitter)
        })
    }

    /// Runs the disposal under this identity once, with its borrowed emitter; a disposal
    /// shares the one-shot table kind of a cancellation listener, whose type it has.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn invoke_dispose(&self, handler: u32, emitter: &I::Emitter) -> ExtensionResult<()> {
        let dispose = callbacks::take::<crate::ComponentDisposer>(handler)?;
        with_emitter(&self.imports, emitter, |emitter| dispose(emitter))
    }
}

/// Transfers a built component and its disposal to the host as their identities.
#[cfg(target_arch = "wasm32")]
pub(super) fn transfer<
    I: Imports<Callback = crate::bindings::maestro::extension::host::Callback>,
>(
    imports: &I,
    built: ExtensionComponent,
) -> crate::bindings::maestro::extension::host::FactoryComponent {
    let component = callbacks::Identity::<I>::new(imports).transfer(built.component);
    let dispose = built
        .dispose
        .map(|dispose| callbacks::Identity::<I>::new(imports).transfer(dispose));
    crate::bindings::maestro::extension::host::FactoryComponent { component, dispose }
}

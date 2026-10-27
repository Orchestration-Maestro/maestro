//! Component factories and disposals the host runs with a lent terminal and theme.
use std::rc::Rc;

use super::renderers::with_emitter;
use super::{Exports, Imports, callbacks, widget_imports::WidgetImports};
use crate::{
    BranchChangeListener, ComponentFactory, ExtensionComponent, ExtensionResult, FooterData,
    FooterDataPort, FooterFactory, Subscription, TUI,
};

/// Host frame writer behind the facade.
pub(super) struct Terminal<I: WidgetImports> {
    /// The adapter serving this frame writer.
    pub(super) imports: I,
    /// The lent host frame writer.
    pub(super) tui: I::Tui,
    /// The frame writer as a container widget, built on first use.
    pub(super) writer: std::cell::OnceCell<Rc<super::widgets::HostWidget<I>>>,
    /// Authored components lent as focus or overlay targets, held weakly, with their
    /// identities.
    pub(super) lent: std::cell::RefCell<Vec<(std::rc::Weak<dyn crate::ComponentView>, u32)>>,
    /// Input listeners added through this frame writer, each once, with their host removers.
    pub(super) listeners: Rc<super::writer::InputListeners>,
}

impl<I: WidgetImports> Terminal<I> {
    /// The facade of a lent host frame writer.
    pub(super) fn lent(imports: &I, tui: I::Tui) -> TUI {
        TUI::supplied(Rc::new(Self {
            imports: imports.clone(),
            tui,
            writer: std::cell::OnceCell::new(),
            lent: std::cell::RefCell::default(),
            listeners: Rc::default(),
        }))
    }
}

/// What a factory's host lends it: the terminal, the live theme and, for a footer, its
/// metadata.
pub(crate) struct Lent<I: Imports> {
    /// The lent terminal.
    pub(crate) tui: I::Tui,
    /// The lent live theme.
    pub(crate) theme: I::Theme,
    /// The lent footer metadata of a footer factory.
    pub(crate) footer: Option<I::FooterData>,
}

/// Host footer metadata behind the facade.
struct Footer<I: Imports> {
    /// The host the metadata belongs to.
    imports: I,
    /// The lent host metadata.
    footer: I::FooterData,
}

impl<I: Imports> FooterDataPort for Footer<I> {
    fn get_git_branch(&self) -> ExtensionResult<Option<String>> {
        self.imports.footer_get_git_branch(&self.footer)
    }
    fn get_extension_statuses(&self) -> ExtensionResult<Vec<(String, String)>> {
        self.imports.footer_get_extension_statuses(&self.footer)
    }
    fn get_available_provider_count(&self) -> ExtensionResult<f64> {
        self.imports
            .footer_get_available_provider_count(&self.footer)
    }
    fn on_branch_change(&self, listener: BranchChangeListener) -> ExtensionResult<Subscription> {
        let identity = callbacks::Identity::new(&self.imports);
        let remover = self
            .imports
            .footer_on_branch_change(&self.footer, &identity.handle)?;
        identity.keep(listener);
        Ok(super::event_bus::subscription(
            self.imports.clone(),
            remover,
        ))
    }
}

impl<I: WidgetImports> Exports<I> {
    /// Runs the factory under this identity with the lent terminal and theme, and with the
    /// footer metadata when the factory builds a footer.
    pub(crate) fn invoke_component_factory(
        &self,
        handler: u32,
        lent: Lent<I>,
        emitter: &I::Emitter,
    ) -> ExtensionResult<ExtensionComponent> {
        let tui = Terminal::lent(&self.imports, lent.tui);
        let theme = super::theme::theme(self.imports.clone(), lent.theme);
        let Some(footer) = lent.footer else {
            let factory = callbacks::find::<ComponentFactory>(handler)?;
            return with_emitter(&self.imports, emitter, |emitter| {
                factory(tui, theme, emitter)
            });
        };
        let factory = callbacks::find::<FooterFactory>(handler)?;
        let footer = FooterData::new(Rc::new(Footer {
            imports: self.imports.clone(),
            footer,
        }));
        with_emitter(&self.imports, emitter, |emitter| {
            factory(tui, theme, footer, emitter)
        })
    }
}

impl<I: Imports> Exports<I> {
    /// Runs the retained expansion callback with this invocation's borrowed emitter.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn invoke_set_expanded(
        &self,
        handler: u32,
        expanded: bool,
        emitter: &I::Emitter,
    ) -> ExtensionResult<()> {
        let handler = callbacks::find::<crate::ComponentExpansionHandler>(handler)?;
        with_emitter(&self.imports, emitter, |emitter| handler(expanded, emitter))
    }

    /// Runs the disposal under this identity once, with its borrowed emitter; a disposal
    /// shares the one-shot table kind of a cancellation listener, whose type it has.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn invoke_dispose(&self, handler: u32, emitter: &I::Emitter) -> ExtensionResult<()> {
        let dispose = callbacks::take::<crate::ComponentDisposer>(handler)?;
        with_emitter(&self.imports, emitter, |emitter| dispose(emitter))
    }
}

/// Transfers a built component and its callbacks to the host as owned identities.
#[cfg(target_arch = "wasm32")]
pub(super) fn transfer<
    I: Imports<Callback = crate::bindings::maestro::extension::host::Callback>,
>(
    imports: &I,
    built: ExtensionComponent,
) -> crate::bindings::maestro::extension::host::FactoryComponent {
    let component = callbacks::Identity::<I>::new(imports).transfer(built.component);
    let dispose = super::renderers::retain(imports, built.dispose);
    let set_expanded = super::renderers::retain(imports, built.set_expanded);
    crate::bindings::maestro::extension::host::FactoryComponent {
        component,
        dispose,
        set_expanded,
        width: built.width,
    }
}

/// Runs the branch change listener under this identity.
#[cfg(any(test, target_arch = "wasm32"))]
pub(crate) fn invoke_branch_change(handler: u32) -> ExtensionResult<()> {
    callbacks::find::<BranchChangeListener>(handler)?()
}

//! Completion providers the host lends, and the authored providers and provider factories the
//! host calls.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use super::renderers::with_emitter;
use super::signals::{Mirror, Signal};
use super::widget_imports::WidgetImports;
use super::{Exports, callbacks};
use crate::{
    AutocompleteItem, AutocompleteProvider, AutocompleteProviderFactory, AutocompleteSuggestions,
    CompletionOptions, CompletionResult, CursorPosition, ExtensionFuture, ExtensionResult,
};

/// The authored editor and completion provider requests of the host at the wire level.
pub(crate) trait EditorImports: WidgetImports {
    /// An editor factory another extension set.
    type EditorFactory: 'static;
    /// Builds an editor with another extension's factory, answering its widget and whether it
    /// extends the custom editor; the factory may call back into this instance first.
    fn editor_factory_build(
        &self,
        factory: &Self::EditorFactory,
        tui: &Self::Tui,
        theme: super::widget_imports::EditorStyles<'_, Self>,
        keybindings: &Self::Keybindings,
    ) -> ExtensionResult<(Self::Widget, bool)>;
    /// One editor operation on an editor another extension's factory built, running the
    /// nested calls the host makes first.
    fn foreign_editor(
        &self,
        widget: &Self::Widget,
        request: super::foreign_editors::ForeignRequest<'_, Self>,
    ) -> ExtensionResult<super::foreign_editors::ForeignAnswer<Self>>;
    /// Lends an editor's border style.
    fn widget_border_color(&self, widget: &Self::Widget) -> ExtensionResult<Self::Style>;
    /// A text callback the host lent to an authored editor.
    type TextCallback: 'static;
    /// Runs a lent text callback, running the nested calls the host makes first.
    fn text_callback(&self, callback: &Self::TextCallback, text: &str) -> ExtensionResult<()>;
    /// Replaces an editor's completion provider with the authored provider staged under
    /// `provider`.
    fn widget_set_autocomplete_provider(
        &self,
        widget: &Self::Widget,
        provider: &Self::Callback,
    ) -> ExtensionResult<()>;
    /// A completion provider the host lent.
    type Provider: 'static;
    /// Requests suggestions from a lent provider with a host signal.
    fn provider_suggestions<'a>(
        &'a self,
        provider: &'a Self::Provider,
        request: (&'a [String], crate::CursorPosition, Option<bool>),
        signal: &'a Self::Signal,
    ) -> crate::ExtensionFuture<'a, Option<crate::AutocompleteSuggestions>>;
    /// Applies a completion through a lent provider, running the nested calls the host makes
    /// first.
    fn provider_apply(
        &self,
        provider: &Self::Provider,
        at: (&[String], crate::CursorPosition),
        item: &crate::AutocompleteItem,
        prefix: &str,
    ) -> ExtensionResult<crate::CompletionResult>;
    /// Reads a lent provider's file-completion decision, running the nested calls the host
    /// makes first.
    fn provider_trigger(
        &self,
        provider: &Self::Provider,
        at: (&[String], crate::CursorPosition),
    ) -> ExtensionResult<Option<bool>>;
}

/// A completion provider the host lent.
pub(super) struct HostProvider<I: EditorImports> {
    /// The host the provider belongs to.
    imports: I,
    /// The host resource.
    provider: I::Provider,
}

/// Wraps a lent host provider.
pub(super) fn lend<I: EditorImports>(
    imports: &I,
    provider: I::Provider,
) -> Rc<dyn AutocompleteProvider> {
    Rc::new(HostProvider {
        imports: imports.clone(),
        provider,
    })
}

impl<I: EditorImports> AutocompleteProvider for HostProvider<I> {
    fn get_suggestions<'a>(
        &'a self,
        lines: &'a [String],
        cursor: CursorPosition,
        options: CompletionOptions<'a>,
    ) -> ExtensionFuture<'a, Option<AutocompleteSuggestions>> {
        Box::pin(async move {
            let request = (lines, cursor, options.force);
            if let Some(lent) = options.signal.host::<Signal<I>>() {
                return self
                    .imports
                    .provider_suggestions(&self.provider, request, &lent.signal)
                    .await;
            }
            let mirror = Mirror::new(&self.imports, Some(options.signal))?;
            let signal = mirror
                .signal()
                .ok_or("a suggestion request needs its signal")?;
            let suggestions = self
                .imports
                .provider_suggestions(&self.provider, request, signal)
                .await;
            mirror.settle(suggestions)
        })
    }

    fn apply_completion(
        &self,
        lines: &[String],
        cursor: CursorPosition,
        item: &AutocompleteItem,
        prefix: &str,
    ) -> ExtensionResult<CompletionResult> {
        self.imports
            .provider_apply(&self.provider, (lines, cursor), item, prefix)
    }

    fn should_trigger_file_completion(
        &self,
        lines: &[String],
        cursor: CursorPosition,
    ) -> ExtensionResult<Option<bool>> {
        self.imports
            .provider_trigger(&self.provider, (lines, cursor))
    }
}

/// One host request on an authored completion provider.
pub(crate) enum ProviderRequest {
    /// Suggestions for the lines and cursor, and whether completion was forced.
    Suggestions(Vec<String>, CursorPosition, Option<bool>),
    /// Applies a chosen item in place of its prefix.
    Apply(Vec<String>, CursorPosition, AutocompleteItem, String),
    /// Whether explicit completion should offer files.
    Trigger(Vec<String>, CursorPosition),
}

/// What an authored completion provider answered.
pub(crate) enum ProviderAnswer {
    /// Suggestions, absent when there are none.
    Suggestions(Option<AutocompleteSuggestions>),
    /// The edited lines and cursor.
    Applied(CompletionResult),
    /// The provider's decision, absent without one.
    Trigger(Option<bool>),
}

impl<I: EditorImports> Exports<I> {
    /// Runs one host request on the authored provider under this identity; suggestions receive
    /// the host's signal.
    pub(crate) async fn invoke_provider(
        &self,
        handler: u32,
        request: ProviderRequest,
        signal: Option<I::Signal>,
    ) -> ExtensionResult<ProviderAnswer> {
        let provider = callbacks::find::<Rc<dyn AutocompleteProvider>>(handler)?;
        Ok(match request {
            ProviderRequest::Suggestions(lines, cursor, force) => {
                let signal = signal.ok_or("a suggestion request needs its signal")?;
                let signal = super::signals::signal(&self.imports, signal);
                let options = CompletionOptions {
                    signal: &signal,
                    force,
                };
                let suggestions = provider.get_suggestions(&lines, cursor, options).await?;
                ProviderAnswer::Suggestions(suggestions)
            }
            ProviderRequest::Apply(lines, cursor, item, prefix) => {
                ProviderAnswer::Applied(provider.apply_completion(&lines, cursor, &item, &prefix)?)
            }
            ProviderRequest::Trigger(lines, cursor) => {
                ProviderAnswer::Trigger(provider.should_trigger_file_completion(&lines, cursor)?)
            }
        })
    }

    /// Runs the provider factory under this identity on the lent current provider.
    pub(crate) fn invoke_provider_factory(
        &self,
        handler: u32,
        current: I::Provider,
        emitter: &I::Emitter,
    ) -> ExtensionResult<Rc<dyn AutocompleteProvider>> {
        let factory = callbacks::find::<AutocompleteProviderFactory>(handler)?;
        let current = lend(&self.imports, current);
        with_emitter(&self.imports, emitter, |emitter| factory(current, emitter))
    }
}

/// Keeps an authored provider under a fresh identity whose owner handle the host receives.
pub(super) fn stage<I: WidgetImports>(
    imports: &I,
    provider: Rc<dyn AutocompleteProvider>,
) -> callbacks::Identity<I> {
    let identity = callbacks::Identity::new(imports);
    identity.stage(provider);
    identity
}

/// Keeps a provider a factory returned under a fresh identity, as a test host holds it.
#[cfg(test)]
pub(crate) fn keep_provider<I: WidgetImports>(
    imports: &I,
    provider: Rc<dyn AutocompleteProvider>,
) -> u32 {
    super::editor_factories::keep(imports, provider)
}

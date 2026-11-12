//! Canonical imports of the completion providers the host lends to the guest.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::bindings::maestro::extension::host::{AutocompleteItem, HostCompletionProvider};
use crate::callback_imports::CallbackView;
use crate::context_imports::add_resource;
use crate::guest_calls::providers::{Cursor, ProviderEndpoint};
use crate::import_route::{Route, push};
use crate::lifecycle::State;
use crate::local_resources::SignalEndpoint;
use crate::read_imports::{endpoint, forwards, release};
use wasmtime::component::{LinkerInstance, Resource};

/// The operands of a suggestion request.
type SuggestionOperands = (
    Resource<ProviderEndpoint>,
    Vec<String>,
    Cursor,
    Resource<SignalEndpoint>,
    Option<bool>,
);

/// Installs the lent provider resource and its methods.
pub(crate) fn add_to_linker<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    host.func_wrap_concurrent(
        "[method]completion-provider.get-suggestions",
        |accessor, (this, lines, cursor, signal, force): SuggestionOperands| {
            Box::pin(async move {
                let (provider, signal) = accessor.with(|mut access| {
                    let table = &access.get().ctx().table;
                    Ok::<_, wasmtime::Error>((
                        table.get(&this)?.clone(),
                        table.get(&signal)?.clone(),
                    ))
                })?;
                Ok((provider.suggestions(lines, cursor, signal, force).await,))
            })
        },
    )
}

impl HostCompletionProvider for State {
    fn apply_completion(
        &mut self,
        this: Resource<ProviderEndpoint>,
        lines: Vec<String>,
        cursor: Cursor,
        item: AutocompleteItem,
        prefix: String,
    ) -> wasmtime::Result<Resource<Route>> {
        let item = maestro_extensions::AutocompleteItem {
            value: item.value,
            label: item.label,
            description: item.description,
        };
        let route = endpoint(self, &this)?.apply(lines, cursor, item, prefix);
        push(self, route)
    }

    fn should_trigger_file_completion(
        &mut self,
        this: Resource<ProviderEndpoint>,
        lines: Vec<String>,
        cursor: Cursor,
    ) -> wasmtime::Result<Resource<Route>> {
        let route = endpoint(self, &this)?.should_trigger_file_completion(lines, cursor);
        push(self, route)
    }

    fn drop(&mut self, this: Resource<ProviderEndpoint>) -> wasmtime::Result<()> {
        release(self, this)
    }
}

/// Registers the lent provider resource with its synchronous methods forwarded to the trait.
pub(crate) fn add_provider(host: &mut LinkerInstance<'_, State>) -> wasmtime::Result<()> {
    add_resource::<State, ProviderEndpoint>(host, "completion-provider")?;
    forwards! { host, "completion-provider", HostCompletionProvider, ProviderEndpoint,
        "apply-completion" => apply_completion(
            lines: Vec<String>,
            cursor: Cursor,
            item: AutocompleteItem,
            prefix: String
        );
        "should-trigger-file-completion" => should_trigger_file_completion(
            lines: Vec<String>,
            cursor: Cursor
        );
    }
    Ok(())
}

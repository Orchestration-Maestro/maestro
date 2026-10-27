//! Canonical imports of the completion providers the host lends to the guest.
use crate::callback_imports::CallbackView;
use crate::guest_calls::providers::{Cursor, Lent, ProviderEndpoint};
use crate::guest_calls::widget_calls::Item;
use crate::import_route::push;
use crate::local_resources::SignalEndpoint;
use wasmtime::{
    StoreContextMut,
    component::{LinkerInstance, Resource},
};

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
    crate::context_imports::add_resource::<T, ProviderEndpoint>(host, "completion-provider")?;
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
    )?;
    host.func_wrap(
        "[method]completion-provider.apply-completion",
        |mut store: StoreContextMut<'_, T>,
         (this, lines, cursor, item, prefix): (
            Resource<ProviderEndpoint>,
            Vec<String>,
            Cursor,
            Item,
            String,
        )| {
            let provider = store.data_mut().ctx().table.get(&this)?.clone();
            let route = provider.route(|reply| Lent::Apply(lines, cursor, item, prefix, reply));
            Ok((push(store.data_mut(), route)?,))
        },
    )?;
    host.func_wrap(
        "[method]completion-provider.should-trigger-file-completion",
        |mut store: StoreContextMut<'_, T>,
         (this, lines, cursor): (Resource<ProviderEndpoint>, Vec<String>, Cursor)| {
            let provider = store.data_mut().ctx().table.get(&this)?.clone();
            let route = provider.route(|reply| Lent::Trigger(lines, cursor, reply));
            Ok((push(store.data_mut(), route)?,))
        },
    )
}

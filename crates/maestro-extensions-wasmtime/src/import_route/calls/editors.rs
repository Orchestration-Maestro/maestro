//! Nested editor factories, authored editor requests, completion providers and provider
//! factories preserve their original retained capabilities.
use super::{Call, Callback, LocalCall, LocalResult};
use crate::local_resources::effects::EmitterEndpoint;
use wasmtime::component::{ComponentType, Lower, Resource, ResourceTable};

/// Owned operands of one nested editor factory.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(in super::super) struct LocalEditorFactory {
    /// Authored identity of the factory.
    target: u32,
    /// Original terminal, transferred.
    tui: Resource<crate::local_resources::effects::TuiEndpoint>,
    /// Lent editor styles, transferred.
    theme: crate::keys_imports::EditorTheme,
    /// Original keybindings manager, transferred.
    keybindings: Resource<crate::keys_reads::KeysEndpoint>,
    /// Independent native invocation frame.
    emitter: Resource<EmitterEndpoint>,
}

/// Owned operands of one nested request on an authored editor.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(in super::super) struct LocalEditor {
    /// Authored identity of the editor.
    target: u32,
    /// The request.
    request: crate::guest_calls::editors::WireRequest,
    /// Independent native invocation frame.
    emitter: Resource<EmitterEndpoint>,
}

/// Owned operands of one nested request on an authored completion provider.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(in super::super) struct LocalAutocomplete {
    /// Authored identity of the provider.
    target: u32,
    /// The request.
    request: crate::guest_calls::providers::Request,
    /// The caller's signal of a suggestion request, transferred.
    signal: Option<Resource<crate::local_resources::SignalEndpoint>>,
}

/// Owned operands of one nested completion provider factory.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(in super::super) struct LocalAutocompleteFactory {
    /// Authored identity of the factory.
    target: u32,
    /// The current provider, lent.
    current: Resource<crate::guest_calls::providers::ProviderEndpoint>,
    /// Independent native invocation frame.
    emitter: Resource<EmitterEndpoint>,
}

/// Marshals one editor factory, editor request, provider request or provider factory.
pub(super) fn lower(
    table: &mut ResourceTable,
    call: &Call<Callback>,
) -> wasmtime::Result<LocalCall> {
    Ok(match call {
        Call::EditorFactory(call) => LocalCall::EditorFactory(LocalEditorFactory {
            target: call.handler.id,
            tui: table.push(call.tui.clone())?,
            theme: crate::keys_imports::lend(table, call.theme.clone())?,
            keybindings: table.push(call.keybindings.clone())?,
            emitter: table.push(call.emitter.clone())?,
        }),
        Call::Editor(call) => LocalCall::Editor(LocalEditor {
            target: call.handler.id,
            request: call.request.lower(table)?,
            emitter: table.push(call.emitter.clone())?,
        }),
        Call::Autocomplete(call) => LocalCall::Autocomplete(LocalAutocomplete {
            target: call.handler.id,
            request: call.request.clone(),
            signal: call
                .signal
                .clone()
                .map(|signal| table.push(signal))
                .transpose()?,
        }),
        Call::AutocompleteFactory(call) => {
            LocalCall::AutocompleteFactory(LocalAutocompleteFactory {
                target: call.handler.id,
                current: table.push(call.current.clone())?,
                emitter: table.push(call.emitter.clone())?,
            })
        }
        _ => {
            return Err(wasmtime::format_err!(
                "expected an editor or provider callback"
            ));
        }
    })
}

/// Delivers a nested editor factory, editor request, provider request or provider factory's
/// result.
pub(super) fn settle<T: crate::callback_imports::CallbackView>(
    store: &mut T,
    call: Call<Callback>,
    value: LocalResult,
) -> wasmtime::Result<()> {
    match (call, value) {
        (Call::EditorFactory(call), LocalResult::EditorFactory(value)) => {
            let value = match value {
                Ok(built) => Ok(crate::guest_calls::editors::retained(store, built)?),
                Err(error) => Err(error),
            };
            let _ = call.reply.send(value);
        }
        (Call::Editor(call), LocalResult::Editor(value)) => {
            let value = match value {
                Ok(answer) => Ok(crate::guest_calls::editors::answered(store, answer)?),
                Err(error) => Err(error),
            };
            let _ = call.reply.send(value);
        }
        (Call::Autocomplete(call), LocalResult::Autocomplete(value)) => {
            let _ = call.reply.send(value);
        }
        (Call::AutocompleteFactory(call), LocalResult::AutocompleteFactory(value)) => {
            let value = match value {
                Ok(provider) => Ok(crate::callback_imports::transferred(store, provider)?),
                Err(error) => Err(error),
            };
            let _ = call.reply.send(value);
        }
        _ => {
            return Err(wasmtime::format_err!(
                "extension route result kind mismatch"
            ));
        }
    }
    Ok(())
}

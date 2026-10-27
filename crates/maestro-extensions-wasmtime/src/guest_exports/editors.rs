//! The canonical editor factory and authored editor exports, entered with the invocation frame.
use super::arguments;
use crate::guest_calls::{
    Callback,
    editors::{Answer, Built, EditorCall, FactoryCall, WireRequest, answered, retained},
};
use crate::keys_reads::KeysEndpoint;
use crate::local_resources::effects::{EmitterEndpoint, TuiEndpoint};
use wasmtime::{
    Store,
    component::{Accessor, ComponentExportIndex, Instance, Resource, TypedFunc},
};

/// Exact canonical editor factory signature.
type Factory = TypedFunc<
    (
        Resource<Callback>,
        Resource<TuiEndpoint>,
        crate::keys_imports::EditorTheme,
        Resource<KeysEndpoint>,
        Resource<EmitterEndpoint>,
    ),
    (Result<Built, String>,),
>;

/// Exact canonical authored editor request signature.
type Editor = TypedFunc<
    (Resource<Callback>, WireRequest, Resource<EmitterEndpoint>),
    (Result<Answer, String>,),
>;

/// The editor factory and authored editor exports of the canonical guest instance.
pub(super) struct Exports(Factory, Editor);

impl Exports {
    /// Resolves the real guest exports.
    pub(super) fn new<T: 'static>(
        store: &mut Store<T>,
        instance: &Instance,
        guest: &ComponentExportIndex,
    ) -> wasmtime::Result<Self> {
        Ok(Self(
            super::typed(store, instance, guest, "invoke-editor-factory")?,
            super::typed(store, instance, guest, "invoke-editor")?,
        ))
    }

    /// Transfers the terminal, styles and keybindings, borrows the callback and frame, and
    /// retains what the factory built.
    pub(super) async fn factory<T: crate::callback_imports::CallbackView + 'static>(
        &self,
        accessor: &Accessor<T>,
        call: FactoryCall<Callback>,
    ) -> wasmtime::Result<()> {
        let (callback, tui) = arguments(accessor, &call.handler, call.tui)?;
        let (theme, keybindings, emitter) = accessor.with(|mut access| {
            let table = access.get().ctx().table;
            Ok::<_, wasmtime::Error>((
                crate::keys_imports::lend(table, call.theme)?,
                table.push(call.keybindings)?,
                table.push(call.emitter)?,
            ))
        })?;
        let operands = (
            Resource::new_borrow(callback.rep()),
            tui,
            theme,
            keybindings,
            Resource::new_borrow(emitter.rep()),
        );
        let result = self.0.call_concurrent(accessor, operands).await;
        let built = accessor.with(|mut access| {
            let table = access.get().ctx().table;
            table.delete(callback)?;
            table.delete(emitter)?;
            match result?.0 {
                Ok(built) => Ok(Ok(retained(access.get(), built)?)),
                Err(error) => Ok::<_, wasmtime::Error>(Err(error)),
            }
        })?;
        let _ = call.reply.send(built);
        Ok(())
    }

    /// Runs one request, deleting its temporary borrows before replying.
    pub(super) async fn request<T: crate::callback_imports::CallbackView + 'static>(
        &self,
        accessor: &Accessor<T>,
        call: EditorCall<Callback>,
    ) -> wasmtime::Result<()> {
        let (callback, emitter) = arguments(accessor, &call.handler, call.emitter)?;
        let request = accessor.with(|mut access| call.request.lower(access.get().ctx().table))?;
        let operands = (
            Resource::new_borrow(callback.rep()),
            request,
            Resource::new_borrow(emitter.rep()),
        );
        let result = self.1.call_concurrent(accessor, operands).await;
        let answer = accessor.with(|mut access| {
            let table = access.get().ctx().table;
            table.delete(callback)?;
            table.delete(emitter)?;
            match result?.0 {
                Ok(answer) => Ok::<_, wasmtime::Error>(Ok(answered(access.get(), answer)?)),
                Err(error) => Ok(Err(error)),
            }
        })?;
        let _ = call.reply.send(answer);
        Ok(())
    }
}

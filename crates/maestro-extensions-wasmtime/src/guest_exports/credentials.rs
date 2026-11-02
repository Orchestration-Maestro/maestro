//! Typed credential exports are the same bodies used by nested same-instance routes.
use super::typed;
use crate::{
    guest_calls::{
        Callback,
        credentials::{BackendCall, FallbackCall},
    },
    local_resources::credential_scope::Endpoint,
};
use wasmtime::{
    Store,
    component::{Accessor, ComponentExportIndex, Instance, Resource, TypedFunc},
};
/// A backend receives a borrowed update, synchronous or awaited.
type Backend = TypedFunc<(Resource<Callback>, Resource<Endpoint>, bool), (Result<(), String>,)>;
/// A fallback returns absent, empty, nonempty or its supplied error distinctly.
type Fallback = TypedFunc<(Resource<Callback>, String), (Result<Option<String>, String>,)>;
/// Canonical credential callback exports of the original instance.
pub(crate) struct Exports {
    /// Actual authored backend entry.
    backend: Backend,
    /// Actual authored resolver entry.
    fallback: Fallback,
}
impl Exports {
    /// Calls the original credential export selected by the native owner.
    pub(crate) async fn invoke<T: crate::callback_imports::CallbackView + 'static>(
        &self,
        accessor: &Accessor<T>,
        call: crate::guest_calls::credentials::CredentialCall<Callback>,
    ) -> wasmtime::Result<()> {
        match call {
            crate::guest_calls::credentials::CredentialCall::Backend(call) => {
                self.backend(accessor, call).await
            }
            crate::guest_calls::credentials::CredentialCall::Fallback(call) => {
                self.fallback(accessor, call).await
            }
        }
    }
    /// Resolves only this component's canonical exports.
    pub(crate) fn new<T: 'static>(
        store: &mut Store<T>,
        instance: &Instance,
        guest: &ComponentExportIndex,
    ) -> wasmtime::Result<Self> {
        Ok(Self {
            backend: typed(store, instance, guest, "invoke-auth-backend")?,
            fallback: typed(store, instance, guest, "invoke-auth-fallback")?,
        })
    }
    /// Calls the author's selected backend method and keeps the operand alive until it settles.
    pub(crate) async fn backend<T: crate::callback_imports::CallbackView + 'static>(
        &self,
        accessor: &Accessor<T>,
        call: BackendCall<Callback>,
    ) -> wasmtime::Result<()> {
        let (handler, update) = accessor.with(|mut access| {
            let table = access.get().ctx().table;
            Ok::<_, wasmtime::Error>((table.push(call.handler)?, table.push(call.update)?))
        })?;
        let result = self
            .backend
            .call_concurrent(
                accessor,
                (
                    Resource::new_borrow(handler.rep()),
                    Resource::new_borrow(update.rep()),
                    call.asynchronous,
                ),
            )
            .await;
        accessor.with(|mut access| {
            let table = access.get().ctx().table;
            table.delete(handler)?;
            table.delete(update)?;
            Ok::<_, wasmtime::Error>(())
        })?;
        let _ = call.reply.send(result?.0);
        Ok(())
    }
    /// Returns the exact resolver value before a native synchronous caller continues.
    pub(crate) async fn fallback<T: crate::callback_imports::CallbackView + 'static>(
        &self,
        accessor: &Accessor<T>,
        call: FallbackCall<Callback>,
    ) -> wasmtime::Result<()> {
        let handler = accessor.with(|mut access| access.get().ctx().table.push(call.handler))?;
        let result = self
            .fallback
            .call_concurrent(
                accessor,
                (Resource::new_borrow(handler.rep()), call.provider),
            )
            .await;
        accessor.with(|mut access| access.get().ctx().table.delete(handler))?;
        let _ = call.reply.send(result?.0);
        Ok(())
    }
}

//! Canonical JSON access and data-only schema export admission.
use crate::{
    callback_imports::CallbackView,
    guest_calls::Callback,
    schema_reads::{Endpoint, Request},
    schema_wire::{Binding, Listing},
};
use wasmtime::{
    StoreContextMut,
    component::{Accessor, LinkerInstance, Resource},
};
/// The current borrowed native operand.
fn node<T: CallbackView>(
    store: &mut T,
    resource: &Resource<Endpoint>,
) -> wasmtime::Result<Endpoint> {
    Ok(store.ctx().table.get(resource)?.clone())
}
/// Install canonical JSON views and live tool enumeration.
pub(crate) fn add_to_linker<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    crate::context_imports::add_resource::<T, Endpoint>(host, "json-node")?;
    host.func_wrap(
        "[method]json-node.kind",
        |mut s: StoreContextMut<'_, T>, (n, p): (Resource<Endpoint>, String)| {
            Ok((node(s.data_mut(), &n)?.read(|r| Request::Kind(p, r)),))
        },
    )?;
    host.func_wrap(
        "[method]json-node.read",
        |mut s: StoreContextMut<'_, T>, (n, p): (Resource<Endpoint>, String)| {
            Ok((node(s.data_mut(), &n)?.read(|r| Request::Read(p, r)),))
        },
    )?;
    host.func_wrap(
        "[method]json-node.keys",
        |mut s: StoreContextMut<'_, T>, (n, p): (Resource<Endpoint>, String)| {
            Ok((node(s.data_mut(), &n)?.read(|r| Request::Keys(p, r)),))
        },
    )?;
    host.func_wrap(
        "[method]json-node.set",
        |mut s: StoreContextMut<'_, T>,
         (n, p, k, v): (Resource<Endpoint>, String, String, String)| {
            Ok((node(s.data_mut(), &n)?.read(|r| Request::Set(p, k, v, r)),))
        },
    )?;
    host.func_wrap(
        "[method]json-node.delete",
        |mut s: StoreContextMut<'_, T>, (n, p, k): (Resource<Endpoint>, String, String)| {
            Ok((node(s.data_mut(), &n)?.read(|r| Request::Delete(p, k, r)),))
        },
    )?;
    host.func_wrap(
        "[method]json-node.splice",
        |mut s: StoreContextMut<'_, T>,
         (n, p, range, v): (Resource<Endpoint>, String, (u32, u32), Vec<String>)| {
            Ok((node(s.data_mut(), &n)?.read(|r| Request::Splice(p, range, v, r)),))
        },
    )?;
    host.func_wrap_concurrent("get-all-tools", |a, (): ()| {
        Box::pin(async move { listing(a).await })
    })?;
    Ok(())
}
/// Retain this guest's callback bindings before returning live declarations.
async fn listing<T: CallbackView + 'static>(
    a: &Accessor<T>,
) -> wasmtime::Result<(Result<Listing, String>,)> {
    let (api, exports) = a.with(|mut access| {
        let store = access.get();
        Ok::<_, wasmtime::Error>((
            store.api().clone(),
            store.exports().map_err(wasmtime::Error::msg)?,
        ))
    })?;
    let (metadata, bindings) = match api.schema_listing() {
        Ok(v) => v,
        Err(e) => return Ok((Err(e),)),
    };
    let mut schemas = Vec::new();
    for (tool, pointer, handle) in bindings {
        let handler = a.with(|mut access| {
            let store = access.get();
            let callback = store.callbacks().allocate().map_err(wasmtime::Error::msg)?;
            Ok::<_, wasmtime::Error>(store.ctx().table.push(callback)?)
        })?;
        // Both aliases must refer to the same retained identity.
        let source =
            a.with(|mut access| access.get().ctx().table.push(Callback { id: handle.id() }))?;
        let owned = a.with(|mut access| {
            let id = access.get().ctx().table.get(&handler)?.id;
            access.get().ctx().table.push(Callback { id })
        })?;
        let result = exports
            .schema
            .retain(a, Resource::new_borrow(source.rep()), owned)
            .await;
        a.with(|mut access| access.get().ctx().table.delete(source))?;
        if let Err(e) = result? {
            return Ok((Err(e.message),));
        }
        schemas.push(Binding {
            tool,
            pointer,
            handler,
        });
    }
    Ok((Ok(Listing { metadata, schemas }),))
}

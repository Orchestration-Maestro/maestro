//! Canonical JSON access and retained local or foreign schema admission.
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
    crate::local_resources::schema::link(host)?;
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
    for (tool, pointer, shape, origin) in bindings {
        let mut callback = a.with(|mut access| {
            access
                .get()
                .callbacks()
                .allocate()
                .map_err(wasmtime::Error::msg)
        })?;
        let local = match origin {
            crate::schema_registration::Origin::Local(handle) => {
                if let Err(error) = retain(a, &exports, handle.id(), callback.id).await? {
                    return Ok((Err(error.message),));
                }
                true
            }
            crate::schema_registration::Origin::Foreign(endpoint) => {
                callback.schema = Some(endpoint);
                false
            }
        };
        let handler = a.with(|mut access| access.get().ctx().table.push(callback))?;
        schemas.push(Binding {
            tool,
            pointer,
            handler,
            shape,
            local,
        });
    }
    Ok((Ok(Listing { metadata, schemas }),))
}

/// Copy a local author identity before handing its independently owned alias to the guest.
async fn retain<T: CallbackView + 'static>(
    accessor: &Accessor<T>,
    exports: &crate::guest_exports::Exports,
    source_id: u32,
    retained_id: u32,
) -> wasmtime::Result<Result<(), crate::schema_wire::Error>> {
    let (source, retained) = accessor.with(|mut access| {
        let table = access.get().ctx().table;
        Ok::<_, wasmtime::Error>((
            table.push(Callback {
                id: source_id,
                schema: None,
            })?,
            table.push(Callback {
                id: retained_id,
                schema: None,
            })?,
        ))
    })?;
    let result = exports
        .schema
        .retain(accessor, Resource::new_borrow(source.rep()), retained)
        .await;
    accessor.with(|mut access| access.get().ctx().table.delete(source))?;
    result
}

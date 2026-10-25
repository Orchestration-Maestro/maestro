//! Actual canonical renderer and retained component methods.
use super::arguments;
use crate::{
    guest_calls::{
        Callback,
        renderers::{ComponentCall, ComponentMethod, RenderRequest, RendererCall},
    },
    local_resources::effects::EmitterEndpoint,
};
use wasmtime::{
    Store,
    component::{Accessor, ComponentExportIndex, Instance, Resource, ResourceAny, TypedFunc},
};
use wasmtime_wasi::WasiView;

/// Original renderer entry point transfers an optional real guest component.
type Create = TypedFunc<
    (Resource<Callback>, RenderRequest, Resource<EmitterEndpoint>),
    (Result<Option<ResourceAny>, String>,),
>;
/// Original component rendering method.
type Render =
    TypedFunc<(ResourceAny, u32, Resource<EmitterEndpoint>), (Result<Vec<String>, String>,)>;
/// Original component input method.
type Input = TypedFunc<(ResourceAny, String, Resource<EmitterEndpoint>), (Result<(), String>,)>;
/// Original component invalidation method.
type Invalidate = TypedFunc<(ResourceAny, Resource<EmitterEndpoint>), (Result<(), String>,)>;
/// Original component key-release preference.
type WantsKeyRelease = TypedFunc<(ResourceAny, Resource<EmitterEndpoint>), (Result<bool, String>,)>;

/// All methods belong to the same canonical component instance.
pub(super) struct Exports {
    /// Actual renderer callback.
    create: Create,
    /// Actual component render method.
    render: Render,
    /// Actual component input method.
    input: Input,
    /// Actual component cache invalidation method.
    invalidate: Invalidate,
    /// Actual component key-release preference method.
    wants_key_release: WantsKeyRelease,
}
impl Exports {
    /// Resolves required canonical methods without a fallback implementation.
    pub(super) fn new<T: 'static>(
        store: &mut Store<T>,
        instance: &Instance,
        guest: &ComponentExportIndex,
    ) -> wasmtime::Result<Self> {
        Ok(Self {
            create: super::typed(store, instance, guest, "invoke-renderer")?,
            render: super::typed(store, instance, guest, "[method]component-view.render")?,
            input: super::typed(
                store,
                instance,
                guest,
                "[method]component-view.handle-input",
            )?,
            invalidate: super::typed(store, instance, guest, "[method]component-view.invalidate")?,
            wants_key_release: super::typed(
                store,
                instance,
                guest,
                "[method]component-view.wants-key-release",
            )?,
        })
    }
    /// Transfers the original theme and borrows only the callback and invocation frame.
    pub(super) async fn create<T: WasiView + 'static>(
        &self,
        accessor: &Accessor<T>,
        call: RendererCall<Callback>,
    ) -> wasmtime::Result<()> {
        let (callback, theme) = arguments(accessor, &call.handler, call.theme)?;
        let emitter = accessor.with(|mut access| access.get().ctx().table.push(call.emitter))?;
        let result = self
            .create
            .call_concurrent(
                accessor,
                (
                    Resource::new_borrow(callback.rep()),
                    RenderRequest {
                        message: call.message,
                        expanded: call.expanded,
                        theme,
                    },
                    Resource::new_borrow(emitter.rep()),
                ),
            )
            .await;
        accessor.with(|mut access| {
            let table = access.get().ctx().table;
            table.delete(callback)?;
            table.delete(emitter)?;
            Ok::<_, wasmtime::Error>(())
        })?;
        let _ = call.reply.send(result?.0);
        Ok(())
    }
    /// Borrows the retained guest owner; its final drop remains the driver's responsibility.
    pub(super) async fn component<T: WasiView + 'static>(
        &self,
        accessor: &Accessor<T>,
        call: ComponentCall<ResourceAny>,
    ) -> wasmtime::Result<()> {
        match call {
            ComponentCall::Render(owner, width, emitter, reply) => {
                let emitter = insert(accessor, emitter)?;
                let result = self
                    .render
                    .call_concurrent(
                        accessor,
                        (owner, width, Resource::new_borrow(emitter.rep())),
                    )
                    .await;
                delete(accessor, emitter)?;
                let _ = reply.send(result?.0);
            }
            ComponentCall::Unit(owner, method, emitter, reply) => {
                let emitter = insert(accessor, emitter)?;
                let borrow = Resource::new_borrow(emitter.rep());
                let result = match method {
                    ComponentMethod::Input(data) => {
                        self.input
                            .call_concurrent(accessor, (owner, data, borrow))
                            .await
                    }
                    ComponentMethod::Invalidate => {
                        self.invalidate
                            .call_concurrent(accessor, (owner, borrow))
                            .await
                    }
                };
                delete(accessor, emitter)?;
                let _ = reply.send(result?.0);
            }
            ComponentCall::WantsKeyRelease(owner, emitter, reply) => {
                let emitter = insert(accessor, emitter)?;
                let result = self
                    .wants_key_release
                    .call_concurrent(accessor, (owner, Resource::new_borrow(emitter.rep())))
                    .await;
                delete(accessor, emitter)?;
                let _ = reply.send(result?.0);
            }
        }
        Ok(())
    }
}
/// Inserts only this call's original native invocation frame.
fn insert<T: WasiView + 'static>(
    accessor: &Accessor<T>,
    emitter: EmitterEndpoint,
) -> wasmtime::Result<Resource<EmitterEndpoint>> {
    Ok(accessor.with(|mut access| access.get().ctx().table.push(emitter))?)
}
/// Retires the temporary canonical borrow before exposing the native reply.
fn delete<T: WasiView + 'static>(
    accessor: &Accessor<T>,
    emitter: Resource<EmitterEndpoint>,
) -> wasmtime::Result<()> {
    accessor.with(|mut access| access.get().ctx().table.delete(emitter))?;
    Ok(())
}

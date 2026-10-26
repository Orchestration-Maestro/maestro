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
    component::{Accessor, ComponentExportIndex, Instance, Resource, TypedFunc},
};
use wasmtime_wasi::WasiView;

/// Original renderer entry point transfers an optional real guest component.
type Create = TypedFunc<
    (Resource<Callback>, RenderRequest, Resource<EmitterEndpoint>),
    (Result<Option<Resource<Callback>>, String>,),
>;
/// Original component rendering method.
type Render =
    TypedFunc<(Resource<Callback>, u32, Resource<EmitterEndpoint>), (Result<Vec<String>, String>,)>;
/// Original component input method.
type Input =
    TypedFunc<(Resource<Callback>, String, Resource<EmitterEndpoint>), (Result<(), String>,)>;
/// Original component invalidation method.
type Invalidate = TypedFunc<(Resource<Callback>, Resource<EmitterEndpoint>), (Result<(), String>,)>;
/// Retained expansion callback.
type SetExpanded =
    TypedFunc<(Resource<Callback>, bool, Resource<EmitterEndpoint>), (Result<(), String>,)>;
/// Original component key-release preference.
type WantsKeyRelease =
    TypedFunc<(Resource<Callback>, Resource<EmitterEndpoint>), (Result<bool, String>,)>;

/// Optional focus state of an authored component.
type Focused =
    TypedFunc<(Resource<Callback>, Resource<EmitterEndpoint>), (Result<Option<bool>, String>,)>;

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
    /// Actual expansion callback.
    set_expanded: SetExpanded,
    /// Actual component key-release preference method.
    wants_key_release: WantsKeyRelease,
    /// The component's optional focus state.
    focused: Focused,
    /// The component's synchronous focus setter.
    set_focused: SetExpanded,
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
            render: super::typed(store, instance, guest, "component-render")?,
            input: super::typed(store, instance, guest, "component-handle-input")?,
            invalidate: super::typed(store, instance, guest, "component-invalidate")?,
            set_expanded: super::typed(store, instance, guest, "invoke-set-expanded")?,
            wants_key_release: super::typed(store, instance, guest, "component-wants-key-release")?,
            focused: super::typed(store, instance, guest, "component-focused")?,
            set_focused: super::typed(store, instance, guest, "component-set-focused")?,
        })
    }
    /// Transfers the original theme and borrows only the callback and invocation frame.
    pub(super) async fn create<T: crate::callback_imports::CallbackView + 'static>(
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
        let result = accessor.with(|mut access| {
            let table = access.get().ctx().table;
            table.delete(callback)?;
            table.delete(emitter)?;
            let component = match result?.0 {
                Ok(component) => component,
                Err(error) => return Ok::<_, wasmtime::Error>(Err(error)),
            };
            let component = component
                .map(|component| crate::callback_imports::transferred(access.get(), component));
            Ok(Ok(component.transpose()?))
        })?;
        let _ = call.reply.send(result);
        Ok(())
    }
    /// Borrows the retained authored identity; its final release remains the driver's.
    pub(super) async fn component<T: WasiView + 'static>(
        &self,
        accessor: &Accessor<T>,
        call: ComponentCall<Callback>,
    ) -> wasmtime::Result<()> {
        match call {
            ComponentCall::Render(owner, width, emitter, reply) => {
                let (owner, emitter) = insert(accessor, owner, emitter)?;
                let result = self
                    .render
                    .call_concurrent(accessor, (borrow(&owner), width, borrow(&emitter)))
                    .await;
                delete(accessor, owner, emitter)?;
                let _ = reply.send(result?.0);
            }
            ComponentCall::Unit(owner, method, emitter, reply) => {
                let (owner, emitter) = insert(accessor, owner, emitter)?;
                let result = self
                    .unit(accessor, borrow(&owner), borrow(&emitter), method)
                    .await;
                delete(accessor, owner, emitter)?;
                let _ = reply.send(result?.0);
            }
            ComponentCall::Focused(owner, emitter, reply) => {
                let (owner, emitter) = insert(accessor, owner, emitter)?;
                let result = self
                    .focused
                    .call_concurrent(accessor, (borrow(&owner), borrow(&emitter)))
                    .await;
                delete(accessor, owner, emitter)?;
                let _ = reply.send(result?.0);
            }
            ComponentCall::WantsKeyRelease(owner, emitter, reply) => {
                let (owner, emitter) = insert(accessor, owner, emitter)?;
                let result = self
                    .wants_key_release
                    .call_concurrent(accessor, (borrow(&owner), borrow(&emitter)))
                    .await;
                delete(accessor, owner, emitter)?;
                let _ = reply.send(result?.0);
            }
        }
        Ok(())
    }
    /// Runs one unit-returning component operation with its original borrowed operands.
    async fn unit<T: WasiView + 'static>(
        &self,
        accessor: &Accessor<T>,
        view: Resource<Callback>,
        frame: Resource<EmitterEndpoint>,
        method: ComponentMethod,
    ) -> wasmtime::Result<(Result<(), String>,)> {
        match method {
            ComponentMethod::Input(data) => {
                self.input
                    .call_concurrent(accessor, (view, data, frame))
                    .await
            }
            ComponentMethod::SetExpanded(expanded) => {
                self.set_expanded
                    .call_concurrent(accessor, (view, expanded, frame))
                    .await
            }
            ComponentMethod::SetFocused(focused) => {
                self.set_focused
                    .call_concurrent(accessor, (view, focused, frame))
                    .await
            }
            ComponentMethod::Invalidate => {
                self.invalidate
                    .call_concurrent(accessor, (view, frame))
                    .await
            }
        }
    }
}
/// Inserts the retained identity and this call's original native invocation frame.
fn insert<T: WasiView + 'static>(
    accessor: &Accessor<T>,
    owner: Callback,
    emitter: EmitterEndpoint,
) -> wasmtime::Result<(Resource<Callback>, Resource<EmitterEndpoint>)> {
    accessor.with(|mut access| {
        let table = access.get().ctx().table;
        Ok((table.push(owner)?, table.push(emitter)?))
    })
}
/// Lends a temporary table entry to one canonical call.
fn borrow<R: 'static>(resource: &Resource<R>) -> Resource<R> {
    Resource::new_borrow(resource.rep())
}
/// Retires both temporary canonical borrows before exposing the native reply.
fn delete<T: WasiView + 'static>(
    accessor: &Accessor<T>,
    owner: Resource<Callback>,
    emitter: Resource<EmitterEndpoint>,
) -> wasmtime::Result<()> {
    accessor.with(|mut access| {
        let table = access.get().ctx().table;
        table.delete(owner)?;
        table.delete(emitter)?;
        Ok(())
    })
}

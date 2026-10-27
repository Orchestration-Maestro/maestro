//! The canonical styling and highlighting exports.
use crate::guest_calls::{Callback, paints::PaintCall};
use wasmtime::{
    Store,
    component::{Accessor, ComponentExportIndex, Instance, Resource, TypedFunc},
};
use wasmtime_wasi::WasiView;

/// Exact canonical styling signature.
type Style = TypedFunc<(Resource<Callback>, String), (Result<String, String>,)>;
/// Exact canonical highlighting signature.
type Highlight =
    TypedFunc<(Resource<Callback>, String, Option<String>), (Result<Vec<String>, String>,)>;

/// The styling and highlighting exports of the canonical guest instance.
pub(super) struct Exports {
    /// Styles text.
    style: Style,
    /// Highlights code.
    highlight: Highlight,
}

impl Exports {
    /// Resolves the real guest exports.
    pub(super) fn new<T: 'static>(
        store: &mut Store<T>,
        instance: &Instance,
        guest: &ComponentExportIndex,
    ) -> wasmtime::Result<Self> {
        Ok(Self {
            style: super::typed(store, instance, guest, "invoke-style")?,
            highlight: super::typed(store, instance, guest, "invoke-highlight")?,
        })
    }

    /// Runs one authored function, deleting its temporary borrow before replying.
    pub(super) async fn invoke<T: WasiView + 'static>(
        &self,
        accessor: &Accessor<T>,
        call: PaintCall<Callback>,
    ) -> wasmtime::Result<()> {
        let id = call.handler().id;
        let callback = accessor
            .with(|mut access| access.get().ctx().table.push(Callback { id, schema: None }))?;
        let handler = Resource::new_borrow(callback.rep());
        match call {
            PaintCall::Style(_, text, reply) => {
                let result = self.style.call_concurrent(accessor, (handler, text)).await;
                accessor.with(|mut access| access.get().ctx().table.delete(callback))?;
                let _ = reply.send(result?.0);
            }
            PaintCall::Highlight(_, code, lang, reply) => {
                let operands = (handler, code, lang);
                let result = self.highlight.call_concurrent(accessor, operands).await;
                accessor.with(|mut access| access.get().ctx().table.delete(callback))?;
                let _ = reply.send(result?.0);
            }
        }
        Ok(())
    }
}

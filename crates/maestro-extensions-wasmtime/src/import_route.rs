//! A native operation in progress hands its nested same-instance calls to its own guest.
//!
//! The Store thread cannot enter an instance that is already waiting inside a synchronous
//! import, so a call raised from that import's native code travels back through the import as
//! a typed step and runs the same guest body the ordinary export runs.
use crate::{
    guest_calls::{Call, Callback},
    lifecycle::completion::Completion,
};
use std::sync::mpsc::{Receiver, Sender};
use wasmtime::{
    StoreContextMut,
    component::{ComponentType, LinkerInstance, Lower, Resource},
};

mod calls;
mod values;
use calls::LocalResult;
pub(crate) use values::Value;

/// A nested same-instance call; its strong ownership travels in its completion.
pub(crate) struct Nested {
    /// The call, its operands and its native reply.
    pub(crate) call: Call<Callback>,
    /// Releases the call's admitted operands once it settles.
    _completion: Completion,
    /// Releases an asynchronous caller once the call's prefix ran.
    prefix: Option<crate::local_resources::Completion>,
}

impl Nested {
    /// Admits a call's operands and hands it to the native operation's route.
    pub(crate) fn send(
        route: &Sender<Self>,
        call: crate::guest_calls::Call,
        prefix: Option<crate::local_resources::Completion>,
    ) -> Result<(), String> {
        let completion = Completion::local();
        call.before(&completion);
        let nested = Self {
            call: call.admitted(&completion),
            _completion: completion,
            prefix,
        };
        route
            .send(nested)
            .map_err(|_| "extension native operation ended".to_owned())
    }

    /// Lets an asynchronous caller continue now that the call's prefix ran.
    fn started(&mut self) {
        if let Some(prefix) = self.prefix.take() {
            prefix.complete(Ok(()));
        }
    }

    /// Delivers the call's result, then releases a caller still waiting for its prefix.
    fn settle<T: crate::callback_imports::CallbackView>(
        mut self,
        store: &mut T,
        value: LocalResult,
    ) -> wasmtime::Result<()> {
        let settled = calls::settle(store, self.call, value);
        if let Some(prefix) = self.prefix.take() {
            prefix.complete(Ok(()));
        }
        settled
    }
}

/// Store-side ticket for one native operation in progress.
pub(crate) struct Route {
    /// Structured schema outcome, present only for a schema import.
    pub(crate) schema: Option<Receiver<crate::local_resources::schema::Result>>,
    /// Nested calls of the operation, closed when the operation returns.
    nested: Receiver<Nested>,
    /// The operation's own result, sent before its nested calls close.
    result: Box<dyn FnOnce() -> Result<Value, String> + Send>,
    /// Outstanding nested call, settled exactly once by the guest.
    pending: Option<Nested>,
}

impl Route {
    /// Opens the nested-call channel beside the operation's own reply.
    pub(crate) fn new<T: Into<Value> + Send + 'static>(
        result: Receiver<Result<T, String>>,
    ) -> (Sender<Nested>, Self) {
        let (send, nested) = std::sync::mpsc::channel();
        let route = Self {
            schema: None,
            nested,
            result: Box::new(move || {
                result
                    .recv()
                    .unwrap_or_else(|_| Err("extension resource reply ended".into()))
                    .map(Into::into)
            }),
            pending: None,
        };
        (send, route)
    }

    /// Returns a catchable failure discovered before native admission.
    pub(crate) fn failed(error: String) -> Self {
        let (send, result) = std::sync::mpsc::channel();
        let (_, route) = Self::new::<()>(result);
        let _ = send.send(Err(error));
        route
    }

    /// Waits for the next nested call, or for the operation's own result once it returned.
    pub(crate) fn next(&mut self) -> Result<Nested, Result<Value, String>> {
        self.nested.recv().map_err(|_| {
            let result = std::mem::replace(
                &mut self.result,
                Box::new(|| Err("extension route already completed".into())),
            );
            result()
        })
    }

    /// Takes the nested call the guest is running.
    fn take(&mut self) -> wasmtime::Result<Nested> {
        self.pending
            .take()
            .ok_or_else(|| wasmtime::format_err!("extension route has no invocation"))
    }
}

#[cfg(test)]
impl Route {
    /// Waits for an operation that makes no nested call into this instance.
    pub(crate) fn result(mut self) -> Result<(), String> {
        match self.next() {
            Ok(_) => Err("unexpected nested call".to_owned()),
            Err(result) => result.and_then(|value| match value {
                Value::Unit => Ok(()),
                _ => Err("unexpected non-unit result".into()),
            }),
        }
    }
}

/// A nested asynchronous call whose prefix ran and whose result the guest delivers later.
pub(crate) struct Settlement(Option<Nested>);

/// One step of a native operation, as the guest receives it.
#[derive(ComponentType, Lower)]
#[component(variant)]
enum RouteStep {
    /// A nested same-instance call to run now.
    #[component(name = "invoke")]
    Invoke(calls::LocalCall),
    /// The operation's own final result.
    #[component(name = "done")]
    Done(Result<values::Transferred, String>),
}

/// Installs the progress ticket and the later settlement on the original host interface.
pub(crate) fn link<T: crate::callback_imports::CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    crate::context_imports::add_resource::<T, Route>(host, "import-route")?;
    crate::context_imports::add_resource::<T, Settlement>(host, "local-settlement")?;
    host.func_wrap("[method]import-route.schema-result", schema_result::<T>)?;
    host.func_wrap(
        "[method]import-route.next",
        |mut store: StoreContextMut<'_, T>, (resource,): (Resource<Route>,)| {
            let next = store.data_mut().ctx().table.get_mut(&resource)?.next();
            let step = match next {
                Ok(nested) => {
                    let table = store.data_mut().ctx().table;
                    let local = calls::lower(table, &nested.call)?;
                    table.get_mut(&resource)?.pending = Some(nested);
                    RouteStep::Invoke(local)
                }
                Err(result) => RouteStep::Done(match result {
                    Ok(value) => Ok(value.transfer(store.data_mut().ctx().table)?),
                    Err(error) => Err(error),
                }),
            };
            Ok((step,))
        },
    )?;
    host.func_wrap(
        "[method]import-route.resume",
        |mut store: StoreContextMut<'_, T>, (resource, value): (Resource<Route>, LocalResult)| {
            let nested = store.data_mut().ctx().table.get_mut(&resource)?.take()?;
            nested.settle(store.data_mut(), value)
        },
    )?;
    host.func_wrap(
        "[method]import-route.defer",
        |mut store: StoreContextMut<'_, T>, (resource,): (Resource<Route>,)| {
            let mut nested = store.data_mut().ctx().table.get_mut(&resource)?.take()?;
            nested.started();
            Ok((store
                .data_mut()
                .ctx()
                .table
                .push(Settlement(Some(nested)))?,))
        },
    )?;
    host.func_wrap(
        "[method]local-settlement.finish",
        |mut store: StoreContextMut<'_, T>,
         (resource, value): (Resource<Settlement>, LocalResult)| {
            let nested = store.data_mut().ctx().table.get_mut(&resource)?.0.take();
            let nested =
                nested.ok_or_else(|| wasmtime::format_err!("extension call already settled"))?;
            nested.settle(store.data_mut(), value)
        },
    )?;
    Ok(())
}

/// Pushes a native operation's ticket for the guest to drive.
pub(crate) fn push<T: wasmtime_wasi::WasiView>(
    store: &mut T,
    route: Route,
) -> wasmtime::Result<Resource<Route>> {
    Ok(store.ctx().table.push(route)?)
}

/// Take the structured schema outcome only after its native import route completes.
fn schema_result<T: crate::callback_imports::CallbackView>(
    mut store: StoreContextMut<'_, T>,
    (resource,): (Resource<Route>,),
) -> wasmtime::Result<(crate::local_resources::schema::Result,)> {
    let receiver = store
        .data_mut()
        .ctx()
        .table
        .get_mut(&resource)?
        .schema
        .take()
        .ok_or_else(|| wasmtime::format_err!("extension route has no schema result"))?;
    let result = receiver
        .recv()
        .map_err(|_| wasmtime::format_err!("extension schema reply ended"))?;
    Ok((result,))
}

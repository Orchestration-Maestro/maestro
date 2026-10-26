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
use calls::LocalResult;

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
    /// Nested calls of the operation, closed when the operation returns.
    nested: Receiver<Nested>,
    /// The operation's own result, sent before its nested calls close.
    result: Returns,
    /// Outstanding nested call, settled exactly once by the guest.
    pending: Option<Nested>,
}

impl Route {
    /// Opens the nested-call channel beside the operation's own reply.
    pub(crate) fn new(result: Receiver<Result<(), String>>) -> (Sender<Nested>, Self) {
        let (send, nested) = std::sync::mpsc::channel();
        let route = Self {
            nested,
            result: Returns::Unit(result),
            pending: None,
        };
        (send, route)
    }

    /// Opens the same nested-call route for a native value result.
    pub(crate) fn value(result: Receiver<Result<Outcome, String>>) -> (Sender<Nested>, Self) {
        let (send, nested) = std::sync::mpsc::channel();
        (
            send,
            Self {
                nested,
                result: Returns::Value(result),
                pending: None,
            },
        )
    }

    /// Returns a catchable failure discovered before native admission.
    pub(crate) fn failed(error: String) -> Self {
        let (send, result) = std::sync::mpsc::channel();
        let (_, route) = Self::new(result);
        let _ = send.send(Err(error));
        route
    }

    /// Waits for the next nested call, or for the operation's own result once it returned.
    pub(crate) fn next(&mut self) -> Result<Nested, Result<Outcome, String>> {
        self.nested.recv().map_err(|_| self.result.receive())
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
                Outcome::Unit => Ok(()),
                _ => Err("extension route result kind mismatch".into()),
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
    Done(Result<Value, String>),
}

/// Installs the progress ticket and the later settlement on the original host interface.
pub(crate) fn link<T: crate::callback_imports::CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    crate::context_imports::add_resource::<T, Route>(host, "import-route")?;
    crate::context_imports::add_resource::<T, Settlement>(host, "local-settlement")?;
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
                    Ok(value) => Ok(value.lower(store.data_mut().ctx().table)?),
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

/// Native result ownership shares the original route, never a second dispatcher.
pub(crate) enum Outcome {
    /// An operation with no value.
    Unit,
    /// Original independently retained component.
    Component(crate::native_renders::ComponentEndpoint),
    /// Exact component output lines.
    Lines(Vec<String>),
    /// Native key-release preference.
    Key(bool),
}
/// Result channels keep existing unit callers unchanged.
enum Returns {
    /// Existing native unit operation.
    Unit(Receiver<Result<(), String>>),
    /// Native rendering or component operation.
    Value(Receiver<Result<Outcome, String>>),
}
impl Returns {
    /// Reads exactly one result after the nested sender closes.
    fn receive(&self) -> Result<Outcome, String> {
        match self {
            Self::Unit(result) => result.recv().map(|value| value.map(|()| Outcome::Unit)),
            Self::Value(result) => result.recv(),
        }
        .unwrap_or_else(|_| Err("extension resource reply ended".into()))
    }
}
/// Canonical result values transfer their original resource ownership.
#[derive(ComponentType, Lower)]
#[component(variant)]
enum Value {
    /// No value.
    #[component(name = "unit")]
    Unit,
    /// Original native component.
    #[component(name = "component")]
    Component(Resource<crate::native_renders::ComponentEndpoint>),
    /// Component painting.
    #[component(name = "lines")]
    Lines(Vec<String>),
    /// Key-release preference.
    #[component(name = "key")]
    Key(bool),
}
impl Outcome {
    /// Transfers original resources only at the canonical return boundary.
    fn lower(self, table: &mut wasmtime::component::ResourceTable) -> wasmtime::Result<Value> {
        Ok(match self {
            Self::Unit => Value::Unit,
            Self::Component(component) => Value::Component(table.push(component)?),
            Self::Lines(lines) => Value::Lines(lines),
            Self::Key(key) => Value::Key(key),
        })
    }
}

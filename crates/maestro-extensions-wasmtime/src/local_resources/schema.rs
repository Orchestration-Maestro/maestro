//! Foreign schema calls borrow the operand on the original synchronous wait stack.
use crate::{
    guest_calls::Callback,
    local_resources::{Capability, Lease, Message, Registry},
    schema_reads::Request,
    schema_wire::{Error, Operation},
};
use maestro_extensions::{DiagnosticErrorInfo, SchemaNode};
use serde_json::Value;
use std::sync::{Arc, mpsc};
use wasmtime::{
    StoreContextMut,
    component::{LinkerInstance, Resource},
};

/// One schema result, separate from ordinary extension string errors.
pub(crate) type Result = std::result::Result<String, Error>;
/// One independently retained node in the receiving guest's existing registry.
#[derive(Clone)]
pub(crate) struct Endpoint(Arc<Lease>);
/// A method requested against the scoped operand, with both original completions.
pub(crate) struct Invocation {
    /// Node identity in the receiving guest's native registry.
    pub(crate) schema: u64,
    /// Method selected by the caller.
    pub(crate) operation: Operation,
    /// Selected view within the borrowed operand.
    pub(crate) path: String,
    /// Original structured schema result.
    pub(crate) result: mpsc::Sender<Result>,
    /// Completion of the native import route.
    pub(crate) done: mpsc::Sender<std::result::Result<(), String>>,
}
impl Invocation {
    /// Invoke against the original value, never a serialized tree copy.
    pub(crate) fn answer(self, registry: &Registry, value: &mut Value) {
        let result = registry.schema_invoke(self.schema, &self.operation, &self.path, value);
        let _ = self.result.send(result.map_err(Error::from));
        let _ = self.done.send(Ok(()));
    }
    /// Settle both completions when the operand ended before the request arrived.
    pub(crate) fn reject(self) {
        let _ = self.result.send(Err(DiagnosticErrorInfo::error(
            "extension JSON operand ended",
        )
        .into()));
        let _ = self.done.send(Ok(()));
    }
}
impl Registry {
    /// Retain native or foreign author captures without adding another callback registry.
    pub(crate) fn schema(&self, node: SchemaNode) -> std::result::Result<Endpoint, String> {
        self.insert(Capability::Schema(node)).map(Endpoint)
    }
    /// Clone captures outside the registry borrow before running supplied callbacks.
    fn schema_invoke(
        &self,
        id: u64,
        operation: &Operation,
        path: &str,
        value: &mut Value,
    ) -> std::result::Result<String, DiagnosticErrorInfo> {
        let capability = self.capability(id);
        let Some(Capability::Schema(node)) = capability.as_deref() else {
            return Err(DiagnosticErrorInfo::error(
                "extension schema resource ended",
            ));
        };
        let value = value
            .pointer_mut(path)
            .ok_or_else(|| DiagnosticErrorInfo::error("JSON operand is absent"))?;
        let result = run(node, operation, value)?;
        serde_json::to_string(&result)
            .map_err(|error| DiagnosticErrorInfo::error(error.to_string()))
    }
}
/// Select only the requested native method, retaining conversion's null distinction.
fn run(
    node: &SchemaNode,
    operation: &Operation,
    value: &mut Value,
) -> std::result::Result<Value, DiagnosticErrorInfo> {
    let base = || {
        node.base
            .as_ref()
            .ok_or_else(|| DiagnosticErrorInfo::error("schema has no Base"))
    };
    let refinement = |index: u32| {
        node.refinements
            .get(index as usize)
            .ok_or_else(|| DiagnosticErrorInfo::error("schema has no refinement at this index"))
    };
    match operation {
        Operation::Convert => {
            serde_json::to_value(base()?.convert.call(value)?.map(|value| (value,)))
        }
        Operation::Check => serde_json::to_value(base()?.check.call(value)?),
        Operation::Errors => serde_json::to_value(base()?.errors.call(value)?),
        Operation::Refine(index) => serde_json::to_value(refinement(*index)?.check.call(value)?),
        Operation::Message(index) => serde_json::to_value(refinement(*index)?.error.call(value)?),
    }
    .map_err(|error| DiagnosticErrorInfo::error(error.to_string()))
}
/// Install the synchronous routed method on the existing callback resource.
pub(crate) fn link<T: crate::callback_imports::CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    host.func_wrap(
        "[method]callback.schema-invoke",
        |mut store: StoreContextMut<'_, T>,
         (callback, operation, node, path): (
            Resource<Callback>,
            Operation,
            Resource<crate::schema_reads::Endpoint>,
            String,
        )| {
            let table = store.data_mut().ctx().table;
            let schema = table
                .get(&callback)?
                .schema
                .clone()
                .ok_or_else(|| wasmtime::format_err!("schema callback is local"))?;
            let node = table.get(&node)?.clone();
            let (send, result) = mpsc::channel();
            let mut route = schema.0.route(|id, done| {
                Message::Json(
                    node.id,
                    Request::Invoke(Invocation {
                        schema: id,
                        operation,
                        path,
                        result: send,
                        done,
                    }),
                )
            });
            route.schema = Some(result);
            Ok((table.push(route)?,))
        },
    )?;
    Ok(())
}

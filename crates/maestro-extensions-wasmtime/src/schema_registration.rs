//! Shared schema records retain callbacks under the existing local identity owner.
use crate::{
    guest_calls::Handle,
    local_resources::Registry,
    schema_wire::{Operation, Shape},
};
use maestro_extensions::{
    Base, DiagnosticErrorInfo, ExtensionAPI, Local, Refinement, SchemaCallback, SchemaCode,
    SchemaNode,
};
use serde_json::Value;
use std::rc::{Rc, Weak};
/// The shared native acceptance callback signature.
pub(crate) type Check = dyn Fn(&mut Value) -> Result<bool, DiagnosticErrorInfo>;
/// Weak capture and weak Store ownership for an actual native callable.
pub(crate) type Identity = (Weak<Check>, crate::guest_calls::WeakHandle);
/// Live metadata with schema origins selected by callback identity.
pub(crate) type ToolListing = (String, Vec<(u32, String, Shape, Origin)>);
/// Local author identity or a foreign node retained by the existing registry.
pub(crate) enum Origin {
    /// This instance can clone its own callbacks locally.
    Local(Handle),
    /// Another guest or native author retains its original callbacks.
    Foreign(crate::local_resources::schema::Endpoint),
}
/// The independently optional preparation, execution and schema callbacks.
pub(crate) type ToolCallbacks = (Option<Handle>, Handle, Schemas);
/// Described guest callbacks keyed by authored schema pointer.
pub(crate) type Schemas = Vec<(String, Shape, Handle)>;
/// Adapt one guest schema method through the request-owned capture record.
fn callback<R: serde::de::DeserializeOwned + 'static>(
    handle: Handle,
    registry: Registry,
    api: ExtensionAPI<Local>,
    operation: Operation,
) -> SchemaCallback<R> {
    SchemaCallback::new(move |value| {
        api.assert_active()
            .map_err(|error| DiagnosticErrorInfo::error(error.to_string()))?;
        handle.schema_call(&registry, &api.events, operation.clone(), value)
    })
}
/// Adapt and identify an acceptance callback without a second origin registry.
fn check(
    handle: &Handle,
    registry: &Registry,
    api: &ExtensionAPI<Local>,
    operation: Operation,
) -> SchemaCallback<bool> {
    let h = handle.clone();
    let r = registry.clone();
    let api = api.clone();
    let cb: Rc<Check> = Rc::new(move |value| {
        api.assert_active()
            .map_err(|error| DiagnosticErrorInfo::error(error.to_string()))?;
        h.schema_call(&r, &api.events, operation.clone(), value)
    });
    registry.schema_identity(&cb, handle);
    SchemaCallback::from_callback(cb)
}
/// Build native schema nodes through their existing callback owner.
pub(crate) fn code(schemas: Schemas, registry: &Registry, api: &ExtensionAPI<Local>) -> SchemaCode {
    schemas
        .into_iter()
        .map(|(pointer, shape, handle)| {
            let base = shape.base.then(|| Base {
                convert: {
                    let h = handle.clone();
                    let r = registry.clone();
                    let api = api.clone();
                    SchemaCallback::new(move |value| {
                        api.assert_active()
                            .map_err(|error| DiagnosticErrorInfo::error(error.to_string()))?;
                        h.schema_call::<Option<(Value,)>>(
                            &r,
                            &api.events,
                            Operation::Convert,
                            value,
                        )
                        .map(|v| v.map(|(v,)| v))
                    })
                },
                check: check(&handle, registry, api, Operation::Check),
                errors: callback(
                    handle.clone(),
                    registry.clone(),
                    api.clone(),
                    Operation::Errors,
                ),
            });
            let refinements = (0..shape.refinements)
                .map(|i| Refinement {
                    check: check(&handle, registry, api, Operation::Refine(i)),
                    error: callback(
                        handle.clone(),
                        registry.clone(),
                        api.clone(),
                        Operation::Message(i),
                    ),
                })
                .collect();
            (pointer, SchemaNode { base, refinements })
        })
        .collect()
}
impl Registry {
    /// Associate an actual callable with its weak original guest ownership.
    fn schema_identity(&self, callback: &Rc<Check>, handle: &Handle) {
        self.resources.borrow_mut().identity(
            Rc::as_ptr(callback).cast::<()>().addr(),
            crate::local_resources::Identity::Schema((Rc::downgrade(callback), handle.downgrade())),
        );
    }
    /// Resolve only this Store's live callback allocation.
    pub(crate) fn schema_origin(&self, node: &SchemaNode) -> Option<Handle> {
        let cb = node
            .base
            .as_ref()
            .map(|base| &base.check)
            .or_else(|| node.refinements.first().map(|r| &r.check))?;
        let key = std::ptr::from_ref(&**cb).cast::<()>().addr();
        let resources = self.resources.borrow();
        match resources.identities.get(&key) {
            Some(crate::local_resources::Identity::Schema((weak, handle)))
                if weak.strong_count() > 0 =>
            {
                handle.upgrade()
            }
            _ => None,
        }
    }
    /// Select live schema origins through the actual native declarations.
    pub(crate) fn schema_listing(
        &self,
        api: &maestro_extensions::ExtensionAPI<Local>,
    ) -> Result<ToolListing, String> {
        let tools = api.get_all_tools().map_err(|e| e.to_string())?;
        let metadata = maestro_extensions::schema_transport::encode_tools(&tools)?;
        let mut bindings = Vec::new();
        for (index, tool) in tools.iter().enumerate() {
            for (pointer, node) in tool.parameters.code() {
                let shape = Shape {
                    base: node.base.is_some(),
                    refinements: u32::try_from(node.refinements.len())
                        .map_err(|e| e.to_string())?,
                };
                let origin = match self.schema_origin(node) {
                    Some(handle) => Origin::Local(handle),
                    None => Origin::Foreign(self.schema(node.clone())?),
                };
                bindings.push((
                    u32::try_from(index).map_err(|e| e.to_string())?,
                    pointer.clone(),
                    shape,
                    origin,
                ));
            }
        }
        Ok((metadata, bindings))
    }
}

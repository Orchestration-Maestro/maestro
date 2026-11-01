//! Originating schema callbacks and borrowed invocation resources.
use super::{Exports, Imports, callbacks};
use crate::ExtensionResult;
use crate::bindings::maestro::extension::host::{JsonKind, SchemaOperation, SchemaShape};
use crate::schema_code::{DiagnosticErrorInfo, SchemaResult};
use crate::schema_code::{Handler, JsonPort, JsonView, SchemaNode};
use serde_json::Value;
use std::rc::Rc;

/// Retain all methods at a single authored schema node in the existing callback table.
pub(super) fn handler(node: SchemaNode) -> Handler {
    Rc::new(node)
}

/// Copy locally retained callbacks without changing their author captures or affinity.
pub(super) fn node(handler: &Handler) -> SchemaNode {
    handler.as_ref().clone()
}

/// The live operand and emitter supplied by the active invocation.
struct Port<'a, I: Imports> {
    /// The runtime holding the resources.
    imports: &'a I,
    /// A borrowed invocation-local node, not a checker-root path.
    handle: &'a I::JsonNode,
}
impl<I: Imports> JsonPort for Port<'_, I> {
    fn resource(&self) -> &dyn std::any::Any {
        self.handle
    }

    fn kind(&self, path: &str) -> ExtensionResult<Option<JsonKind>> {
        self.imports.json_kind(self.handle, path)
    }
    fn read(&self, path: &str) -> ExtensionResult<Option<Value>> {
        self.imports
            .json_read(self.handle, path)?
            .map(|text| super::decode_json(&text))
            .transpose()
    }
    fn keys(&self, path: &str) -> ExtensionResult<Vec<String>> {
        self.imports.json_keys(self.handle, path)
    }
    fn set(&self, path: &str, key: &str, value: &Value) -> ExtensionResult<()> {
        self.imports.json_set(
            self.handle,
            path,
            key,
            &serde_json::to_string(value).map_err(|error| error.to_string())?,
        )
    }
    fn delete(&self, path: &str, key: &str) -> ExtensionResult<()> {
        self.imports.json_delete(self.handle, path, key)
    }
    fn splice(
        &self,
        path: &str,
        start: u32,
        delete: u32,
        values: &[Value],
    ) -> ExtensionResult<Vec<Value>> {
        let values = values
            .iter()
            .map(serde_json::to_string)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        self.imports
            .json_splice(self.handle, path, (start, delete), &values)?
            .into_iter()
            .map(|text| super::decode_json(&text))
            .collect()
    }
}
impl<I: Imports> Exports<I> {
    /// Describe the actual callbacks at their originating schema node.
    pub(crate) fn describe_schema(handler: u32) -> SchemaResult<SchemaShape> {
        let node = callbacks::find::<Handler>(handler).map_err(DiagnosticErrorInfo::error)?;
        Ok(SchemaShape {
            base: node.base.is_some(),
            refinements: u32::try_from(node.refinements.len())
                .map_err(|error| DiagnosticErrorInfo::error(error.to_string()))?,
        })
    }
    /// Retain callbacks in the same registry under an independently owned identity.
    pub(crate) fn retain_schema(&self, handler: u32, retained: I::Callback) -> SchemaResult<()> {
        let node = callbacks::find::<Handler>(handler).map_err(DiagnosticErrorInfo::error)?;
        callbacks::Identity::from_handle(&self.imports, retained).keep(node);
        Ok(())
    }
    /// Invoke one method at its originating node without requesting unrelated callbacks.
    pub(crate) fn invoke_schema(
        &self,
        handler: u32,
        operation: SchemaOperation,
        node: &I::JsonNode,
        emitter: &I::Emitter,
    ) -> SchemaResult<String> {
        let node_callbacks =
            callbacks::find::<Handler>(handler).map_err(DiagnosticErrorInfo::error)?;
        let port = Port {
            imports: &self.imports,
            handle: node,
        };
        let mut value = JsonView::new(&port);
        let emission = super::event_bus::Emitter {
            imports: &self.imports,
            handle: emitter,
        };
        let emitter = crate::CallbackEmitter::new(&emission);
        let result = run(&node_callbacks, operation, &mut value, emitter)?;
        serde_json::to_string(&result)
            .map_err(|error| DiagnosticErrorInfo::error(error.to_string()))
    }
}

/// Dispatch a schema method while keeping its author error intact.
fn run(
    node: &SchemaNode,
    operation: SchemaOperation,
    value: &mut JsonView<'_>,
    emitter: crate::CallbackEmitter<'_>,
) -> SchemaResult<Value> {
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
        // The tuple keeps a null replacement distinct from no replacement.
        SchemaOperation::Convert => {
            serde_json::to_value(base()?.convert.call(value, emitter)?.map(|value| (value,)))
        }
        SchemaOperation::Check => serde_json::to_value(base()?.check.call(value, emitter)?),
        SchemaOperation::Errors => serde_json::to_value(base()?.errors.call(value, emitter)?),
        SchemaOperation::Refine(index) => {
            serde_json::to_value(refinement(index)?.check.call(value, emitter)?)
        }
        SchemaOperation::Message(index) => {
            serde_json::to_value(refinement(index)?.error.call(value, emitter)?)
        }
    }
    .map_err(|error| DiagnosticErrorInfo::error(error.to_string()))
}

/// Rebuild a foreign node's shape while retaining the original capability.
pub(super) fn foreign<I: Imports>(
    imports: &I,
    identity: callbacks::Scoped<I>,
    shape: SchemaShape,
) -> SchemaNode {
    use crate::schema_code::{Base, Refinement};
    let identity = Rc::new(identity);
    let base = shape.base.then(|| Base {
        convert: {
            let callback = forwarded::<I, Option<(Value,)>>(
                imports.clone(),
                identity.clone(),
                SchemaOperation::Convert,
            );
            crate::schema_code::SchemaCallback::new(move |value, emitter| {
                callback
                    .call(value, emitter)
                    .map(|value| value.map(|(value,)| value))
            })
        },
        check: forwarded(imports.clone(), identity.clone(), SchemaOperation::Check),
        errors: forwarded(imports.clone(), identity.clone(), SchemaOperation::Errors),
    });
    let refinements = (0..shape.refinements)
        .map(|index| Refinement {
            check: forwarded(
                imports.clone(),
                identity.clone(),
                SchemaOperation::Refine(index),
            ),
            error: forwarded(
                imports.clone(),
                identity.clone(),
                SchemaOperation::Message(index),
            ),
        })
        .collect();
    SchemaNode { base, refinements }
}
/// Forward only the selected operation and borrowed view, never the complete operand.
fn forwarded<I: Imports, R: serde::de::DeserializeOwned + 'static>(
    imports: I,
    identity: Rc<callbacks::Scoped<I>>,
    operation: SchemaOperation,
) -> crate::schema_code::SchemaCallback<R> {
    crate::schema_code::SchemaCallback::new(move |value, _| {
        let node = value
            .port
            .resource()
            .downcast_ref::<I::JsonNode>()
            .ok_or_else(|| {
                DiagnosticErrorInfo::error("schema operand belongs to another runtime")
            })?;
        let text = imports.schema_invoke(&identity.handle, operation, node, &value.path)?;
        super::decode_json(&text).map_err(DiagnosticErrorInfo::error)
    })
}

#[cfg(test)]
mod tests {
    use super::{handler, node};
    use crate::schema_code::{Base, Refinement, SchemaCallback, SchemaNode};
    use serde_json::json;

    /// A node supplying every Base method and two ordered refinements.
    fn supplied_node() -> SchemaNode {
        SchemaNode {
            base: Some(Base {
                convert: SchemaCallback::new(|_, _| Ok(Some(json!({"converted":true})))),
                check: SchemaCallback::new(|_, _| Ok(true)),
                errors: SchemaCallback::new(|_, _| Ok(vec![json!({"message":"base detail"})])),
            }),
            refinements: vec![
                Refinement {
                    check: SchemaCallback::new(|_, _| Ok(false)),
                    error: SchemaCallback::new(|_, _| Ok("first".into())),
                },
                Refinement {
                    check: SchemaCallback::new(|_, _| Ok(false)),
                    error: SchemaCallback::new(|_, _| Ok("second".into())),
                },
            ],
        }
    }

    /// A local retained node keeps the Base its author supplied.
    #[test]
    fn schema_node_roundtrip_retains_the_supplied_base() {
        let restored = node(&handler(supplied_node()));
        assert!(
            restored.base.is_some(),
            "a relayed executable schema keeps its Base"
        );
    }

    /// A local retained node keeps every refinement slot its author supplied.
    #[test]
    fn schema_node_roundtrip_retains_every_supplied_refinement() {
        let restored = node(&handler(supplied_node()));
        assert_eq!(
            restored.refinements.len(),
            2,
            "a relayed schema keeps both refinement slots"
        );
    }
}

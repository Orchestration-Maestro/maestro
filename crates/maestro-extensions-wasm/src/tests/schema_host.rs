//! The shared invocation-local JSON resource used by both component test adapters.
use crate::bindings::maestro::extension::host::JsonKind;
use serde_json::Value;
use std::sync::{Arc, Mutex};

/// A native JSON operand lent to one synchronous schema invocation.
#[derive(Clone)]
pub struct JsonNode(pub Arc<Mutex<Value>>);
impl JsonNode {
    /// Read only the requested kind, preserving absence separately from null.
    pub fn kind(&self, path: &str) -> Result<Option<JsonKind>, String> {
        let root = self.0.lock().map_err(|error| error.to_string())?;
        Ok(root.pointer(path).map(|value| match value {
            Value::Null => JsonKind::Null,
            Value::Bool(_) => JsonKind::Boolean,
            Value::Number(_) => JsonKind::Number,
            Value::String(_) => JsonKind::String,
            Value::Array(_) => JsonKind::Array,
            Value::Object(_) => JsonKind::Object,
        }))
    }
    /// Copy a value only when the author explicitly asks to read it.
    pub fn read(&self, path: &str) -> Result<Option<String>, String> {
        let value = self
            .0
            .lock()
            .map_err(|error| error.to_string())?
            .pointer(path)
            .cloned();
        value
            .map(|value| serde_json::to_string(&value).map_err(|error| error.to_string()))
            .transpose()
    }
    /// Enumerate keys without cloning any child value.
    pub fn keys(&self, path: &str) -> Result<Vec<String>, String> {
        let root = self.0.lock().map_err(|error| error.to_string())?;
        let object = root
            .pointer(path)
            .and_then(Value::as_object)
            .ok_or("JSON operand is not an object")?;
        Ok(maestro_json::canonical_entries(object)
            .into_iter()
            .map(|(key, _)| key.clone())
            .collect())
    }
    /// Write a parsed value to an object member or array position.
    pub fn set(&self, path: &str, key: &str, value: &str) -> Result<(), String> {
        let value = crate::component_adapter::decode_json(value)?;
        let mut root = self.0.lock().map_err(|error| error.to_string())?;
        match root.pointer_mut(path).ok_or("JSON operand is absent")? {
            Value::Object(object) => {
                object.insert(key.to_owned(), value);
            }
            Value::Array(array) => {
                let index = key.parse::<usize>().map_err(|error| error.to_string())?;
                if index >= array.len() {
                    array.resize(
                        index.checked_add(1).ok_or("array index overflow")?,
                        Value::Null,
                    );
                }
                array[index] = value;
            }
            _ => return Err("JSON operand is not a container".into()),
        }
        Ok(())
    }
    /// Delete an object member, or retain an array hole as JSON null.
    pub fn delete(&self, path: &str, key: &str) -> Result<(), String> {
        let mut root = self.0.lock().map_err(|error| error.to_string())?;
        match root.pointer_mut(path).ok_or("JSON operand is absent")? {
            Value::Object(object) => {
                object.shift_remove(key);
            }
            Value::Array(array) => {
                let index = key.parse::<usize>().map_err(|error| error.to_string())?;
                if let Some(value) = array.get_mut(index) {
                    *value = Value::Null;
                }
            }
            _ => return Err("JSON operand is not a container".into()),
        }
        Ok(())
    }
    /// Apply a range edit, serializing removed values only after releasing the native guard.
    pub fn splice(
        &self,
        path: &str,
        range: (u32, u32),
        values: &[String],
    ) -> Result<Vec<String>, String> {
        let values = values
            .iter()
            .map(|text| crate::component_adapter::decode_json(text))
            .collect::<Result<Vec<Value>, _>>()?;
        let removed: Vec<Value> = {
            let mut root = self.0.lock().map_err(|error| error.to_string())?;
            let array = root
                .pointer_mut(path)
                .and_then(Value::as_array_mut)
                .ok_or("JSON operand is not an array")?;
            let start = (range.0 as usize).min(array.len());
            let end = start.saturating_add(range.1 as usize).min(array.len());
            array.splice(start..end, values).collect()
        };
        removed
            .into_iter()
            .map(|value| serde_json::to_string(&value).map_err(|error| error.to_string()))
            .collect()
    }
}

use super::host::{Identity, Imports, State, borrow};
use crate::bindings::host_side::exports::maestro::extension::guest::Guest;
use crate::bindings::host_side::maestro::extension::host;
use wasmtime::component::{Accessor, Resource};

/// The capability at which a callback was originally registered.
#[derive(Clone)]
pub enum Origin {
    /// The originating guest instance in this store.
    Guest(Arc<std::sync::OnceLock<Guest>>),
}
/// Read only the name, leaving deeply nested schema metadata to its existing reader.
#[derive(serde::Deserialize)]
struct Named {
    /// Registration identity.
    name: String,
}
/// Read a registration's name without decoding its parameters again.
pub fn tool_name(text: &str) -> Result<String, serde_json::Error> {
    serde_json::from_str::<Named>(text).map(|tool| tool.name)
}
/// The per-tool source identities selected by an enumeration fixture.
type Listing = (String, Vec<(u32, String, u32)>);
/// Carry each matching schema binding with its own metadata record.
pub fn listing(observed: &super::observed::Observed) -> Result<Listing, String> {
    let text = observed.actions.record_for("getAllTools")?;
    let names = serde_json::from_str::<Vec<Named>>(&text).unwrap_or_default();
    let mut bindings = Vec::new();
    for (index, tool) in names.into_iter().enumerate() {
        for (name, pointer, handle) in &observed.schemas {
            if *name == tool.name {
                bindings.push((
                    u32::try_from(index).map_err(|error| error.to_string())?,
                    pointer.clone(),
                    *handle,
                ));
            }
        }
    }
    Ok((text, bindings))
}

/// Convert only the generated kind representation, not its meaning.
fn host_kind(kind: JsonKind) -> host::JsonKind {
    match kind {
        JsonKind::Null => host::JsonKind::Null,
        JsonKind::Boolean => host::JsonKind::Boolean,
        JsonKind::Number => host::JsonKind::Number,
        JsonKind::String => host::JsonKind::String,
        JsonKind::Array => host::JsonKind::Array,
        JsonKind::Object => host::JsonKind::Object,
    }
}
impl host::HostJsonNode for State {
    fn kind(
        &mut self,
        node: Resource<JsonNode>,
        path: String,
    ) -> wasmtime::Result<Result<Option<host::JsonKind>, String>> {
        self.observed.schema_checks += 1;
        self.observed.schema_request_bytes += 4 + 8 + path.len();
        self.observed.schema_response_bytes += 3;
        Ok(self
            .table
            .get(&node)?
            .kind(&path)
            .map(|kind| kind.map(host_kind)))
    }
    fn read(
        &mut self,
        node: Resource<JsonNode>,
        path: String,
    ) -> wasmtime::Result<Result<Option<String>, String>> {
        self.observed.schema_request_bytes += 12 + path.len();
        let result = self.table.get(&node)?.read(&path);
        self.observed.schema_response_bytes += match &result {
            Ok(Some(value)) => 10 + value.len(),
            Ok(None) => 2,
            Err(error) => 9 + error.len(),
        };
        Ok(result)
    }
    fn keys(
        &mut self,
        node: Resource<JsonNode>,
        path: String,
    ) -> wasmtime::Result<Result<Vec<String>, String>> {
        Ok(self.table.get(&node)?.keys(&path))
    }
    fn set(
        &mut self,
        node: Resource<JsonNode>,
        path: String,
        key: String,
        value: String,
    ) -> wasmtime::Result<Result<(), String>> {
        self.observed.schema_request_bytes += 4 + 8 + path.len() + 8 + key.len() + 8 + value.len();
        self.observed.schema_response_bytes += 1;
        Ok(self.table.get(&node)?.set(&path, &key, &value))
    }
    fn delete(
        &mut self,
        node: Resource<JsonNode>,
        path: String,
        key: String,
    ) -> wasmtime::Result<Result<(), String>> {
        Ok(self.table.get(&node)?.delete(&path, &key))
    }
    fn splice(
        &mut self,
        node: Resource<JsonNode>,
        path: String,
        range: (u32, u32),
        values: Vec<String>,
    ) -> wasmtime::Result<Result<Vec<String>, String>> {
        Ok(self.table.get(&node)?.splice(&path, range, &values))
    }
    fn drop(&mut self, node: Resource<JsonNode>) -> wasmtime::Result<()> {
        self.table.delete(node)?;
        Ok(())
    }
}

/// Resolve the originating resource before calling author code outside the accessor.
fn origin(
    accessor: &Accessor<State, Imports>,
    callback: &Resource<Identity>,
) -> wasmtime::Result<Origin> {
    accessor.with(|mut access| {
        let state = access.get();
        state
            .table
            .get(callback)?
            .1
            .clone()
            .or_else(|| state.schema_origin.clone().map(Origin::Guest))
            .ok_or_else(|| wasmtime::format_err!("schema origin is unavailable"))
    })
}

/// Retain an independent identity in the originating registry.
async fn retain(
    accessor: &Accessor<State, Imports>,
    callback: Resource<Identity>,
) -> wasmtime::Result<Result<Resource<Identity>, host::DiagnosticErrorInfo>> {
    let source = origin(accessor, &callback)?;
    let (id, retained) = accessor.with(|mut access| {
        let state = access.get();
        let id = state.observed.next_identity();
        let retained = state.table.push(Identity(id, Some(source.clone())))?;
        Ok::<_, wasmtime::Error>((id, retained))
    })?;
    match source {
        Origin::Guest(source) => {
            let guest = source
                .get()
                .cloned()
                .ok_or_else(|| wasmtime::format_err!("schema origin is unavailable"))?;
            let owned = accessor.with(|mut access| {
                access
                    .get()
                    .table
                    .push(Identity(id, Some(Origin::Guest(Arc::clone(&source)))))
            })?;
            let function = accessor.with(|access| guest.func_retain_schema().func().typed::<(Resource<Identity>,Resource<Identity>), (Result<(),host::DiagnosticErrorInfo>,)>(access))?;
            match function.call_concurrent(accessor, (callback, owned)).await {
                Ok((Ok(()),)) => Ok(Ok(retained)),
                Ok((Err(error),)) => {
                    accessor.with(|mut access| access.get().table.delete(retained))?;
                    Ok(Err(error))
                }
                Err(error) => {
                    accessor.with(|mut access| access.get().table.delete(retained))?;
                    Err(error)
                }
            }
        }
    }
}

/// Enumeration retains callbacks before handing their independent resources to the guest.
pub(super) async fn get_all_tools(
    accessor: &Accessor<State, Imports>,
) -> wasmtime::Result<Result<host::ToolListing, String>> {
    let (metadata, bindings) = match accessor.with(|mut access| listing(&access.get().observed)) {
        Ok(listing) => listing,
        Err(error) => return Ok(Err(error)),
    };
    let mut schemas = Vec::new();
    for (tool, pointer, callback) in bindings {
        let local = accessor.with(|mut access| {
            let state = access.get();
            let Some(Origin::Guest(source)) = &state.table.get(&borrow(callback))?.1 else {
                return Ok::<_, wasmtime::Error>(false);
            };
            Ok(state
                .schema_origin
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(source, current)))
        })?;
        if !local {
            continue;
        }
        let handler = match retain(accessor, borrow(callback)).await? {
            Ok(handler) => handler,
            Err(error) => return Ok(Err(error.message)),
        };
        let guest = accessor.with(|mut access| {
            access
                .get()
                .schema_origin
                .as_ref()
                .unwrap()
                .get()
                .unwrap()
                .clone()
        });
        let function = accessor.with(|access| guest.func_describe_schema().func().typed::<(Resource<Identity>,), (Result<host::SchemaShape, host::DiagnosticErrorInfo>,)>(access))?;
        let (shape,) = function
            .call_concurrent(accessor, (borrow(callback),))
            .await?;
        let shape = shape.map_err(|error| wasmtime::format_err!("{}", error.message))?;
        schemas.push(host::SchemaBinding {
            tool,
            pointer,
            handler,
            shape,
            local: true,
        });
    }
    Ok(Ok(host::ToolListing { metadata, schemas }))
}

/// Borrowed object enumeration puts canonical numeric keys before ordinary keys.
#[test]
fn borrowed_json_keys_use_canonical_numeric_order() {
    let node = JsonNode(Arc::new(Mutex::new(
        serde_json::json!({"10":10,"2":2,"02":20,"first":1,"second":2}),
    )));
    assert_eq!(node.keys("").unwrap(), ["2", "10", "02", "first", "second"]);
}

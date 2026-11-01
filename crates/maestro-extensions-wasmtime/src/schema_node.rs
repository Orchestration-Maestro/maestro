//! Invocation-local operations on the validator's borrowed JSON operand.
use crate::schema_wire::JsonKind;
use serde_json::Value;

/// A native JSON operand lent to one synchronous schema invocation.
pub(crate) struct JsonNode<'a>(pub(crate) &'a mut Value);
impl JsonNode<'_> {
    /// Read only the requested kind, preserving absence separately from null.
    pub fn kind(&self, path: &str) -> Option<JsonKind> {
        let root = &*self.0;
        root.pointer(path).map(|value| match value {
            Value::Null => JsonKind::Null,
            Value::Bool(_) => JsonKind::Boolean,
            Value::Number(_) => JsonKind::Number,
            Value::String(_) => JsonKind::String,
            Value::Array(_) => JsonKind::Array,
            Value::Object(_) => JsonKind::Object,
        })
    }
    /// Copy a value only when the author explicitly asks to read it.
    pub fn read(&self, path: &str) -> Result<Option<String>, String> {
        let value = self.0.pointer(path);
        value
            .map(|value| serde_json::to_string(&value).map_err(|error| error.to_string()))
            .transpose()
    }
    /// Enumerate keys without cloning any child value.
    pub fn keys(&self, path: &str) -> Result<Vec<String>, String> {
        let root = &*self.0;
        let object = root
            .pointer(path)
            .and_then(Value::as_object)
            .ok_or("JSON operand is not an object")?;
        Ok(
            maestro_extensions::schema_transport::canonical_entries(object)
                .into_iter()
                .map(|(key, _)| key.clone())
                .collect(),
        )
    }
    /// Write a parsed value to an object member or array position.
    pub fn set(&mut self, path: &str, key: &str, value: &str) -> Result<(), String> {
        let value = maestro_extensions::schema_transport::decode_json(value)?;
        let root = &mut *self.0;
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
    pub fn delete(&mut self, path: &str, key: &str) -> Result<(), String> {
        let root = &mut *self.0;
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
        &mut self,
        path: &str,
        range: (u32, u32),
        values: &[String],
    ) -> Result<Vec<String>, String> {
        let values = values
            .iter()
            .map(|text| maestro_extensions::schema_transport::decode_json(text))
            .collect::<Result<Vec<Value>, _>>()?;
        let removed: Vec<Value> = {
            let root = &mut *self.0;
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

//! Structured schema conversion shared by every content-generation endpoint.

use serde_json::{Map, Value};

use super::{Failure, Projection, malformed};
use maestro_json::{canonical_entries, is_truthy};

/// The schema type names the service declares.
const TYPES: [&str; 8] = [
    "TYPE_UNSPECIFIED",
    "STRING",
    "NUMBER",
    "INTEGER",
    "BOOLEAN",
    "ARRAY",
    "OBJECT",
    "NULL",
];

/// Whether a value is a schema authored for the JSON-schema alternative of a field.
pub(super) fn is_json_schema(value: &Value) -> bool {
    value
        .as_object()
        .is_some_and(|schema| schema.contains_key("$schema"))
}

/// Move a `$schema`-bearing source to its JSON alternative when that destination is false-like.
pub(super) fn route_json_schema(members: &mut Map<String, Value>, source: &str, destination: &str) {
    if members.get(source).is_some_and(is_json_schema)
        && !members.get(destination).is_some_and(is_truthy)
        && let Some(schema) = members.shift_remove(source)
    {
        members.insert(destination.to_owned(), schema);
    }
}

/// Convert a structured schema: type names, `items`, `anyOf` and `properties` are rewritten
/// and `additionalProperties` is dropped at every converted node.
pub(super) fn convert(schema: Value) -> Projection<Value> {
    match schema {
        Value::Object(members) => process(members).map(Value::Object),
        _ => Err(malformed("schema must be an object")),
    }
}

/// The declared spelling of a type name, or the unspecified type for an unknown name.
fn type_name(name: &str) -> Value {
    let upper = name.to_uppercase();
    Value::String(if TYPES.contains(&upper.as_str()) {
        upper
    } else {
        "TYPE_UNSPECIFIED".to_owned()
    })
}

/// Convert one schema node.
fn process(source: Map<String, Value>) -> Projection<Map<String, Value>> {
    let populated = |key| source.get(key).is_some_and(is_truthy);
    if populated("type") && populated("anyOf") {
        return Err(Failure::error("type and anyOf cannot be both populated."));
    }
    let mut converted = Map::new();
    let source = unwrap_nullable(source, &mut converted)?;
    if let Some(Value::Array(names)) = source.get("type") {
        flatten_types(names, &mut converted)?;
    }
    for (name, value) in canonical_entries(source) {
        if value.is_null() {
            continue;
        }
        match name.as_str() {
            "type" => {
                if let Some(declared) = declared_type(&value)? {
                    converted.insert(name, declared);
                }
            }
            "items" => {
                converted.insert(name, convert(value)?);
            }
            "anyOf" => union(value, &mut converted)?,
            "properties" => {
                converted.insert(name, properties(value)?);
            }
            "additionalProperties" => {}
            _ => {
                converted.insert(name, value);
            }
        }
    }
    Ok(converted)
}

/// Whether the schema is the null type.
fn is_null_type(schema: &Value) -> bool {
    schema.get("type").and_then(Value::as_str) == Some("null")
}

/// Replace a pure two-arm nullable wrapper by its other arm and mark the result nullable.
///
/// A parent holding any other non-null member is retained for ordinary schema conversion
/// through the general union path instead.
fn unwrap_nullable(
    mut source: Map<String, Value>,
    converted: &mut Map<String, Value>,
) -> Projection<Map<String, Value>> {
    if source
        .iter()
        .any(|(key, value)| key != "anyOf" && !value.is_null())
    {
        return Ok(source);
    }
    let kept = match source
        .get("anyOf")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
    {
        Some([first, _]) if is_null_type(first) => 1,
        Some([_, second]) if is_null_type(second) => 0,
        _ => return Ok(source),
    };
    let Some(Value::Array(arms)) = source.shift_remove("anyOf") else {
        return Ok(source);
    };
    converted.insert("nullable".to_owned(), Value::Bool(true));
    match arms.into_iter().nth(kept) {
        Some(Value::Object(inner)) => Ok(inner),
        _ => Err(malformed("schema must be an object")),
    }
}

/// Spell a type array as `nullable` plus one type, or plus a union of single-type schemas.
fn flatten_types(names: &[Value], converted: &mut Map<String, Value>) -> Projection<()> {
    let names = names
        .iter()
        .map(|name| {
            name.as_str()
                .ok_or_else(|| malformed("schema type names must be text"))
        })
        .collect::<Projection<Vec<_>>>()?;
    if names.contains(&"null") {
        converted.insert("nullable".to_owned(), Value::Bool(true));
    }
    let rest: Vec<&str> = names.into_iter().filter(|name| *name != "null").collect();
    if let [only] = rest[..] {
        converted.insert("type".to_owned(), type_name(only));
    } else {
        let arms = rest
            .into_iter()
            .map(|name| Value::Object(Map::from_iter([("type".to_owned(), type_name(name))])));
        converted.insert("anyOf".to_owned(), Value::Array(arms.collect()));
    }
    Ok(())
}

/// The converted `type` member; an array was already spread and null cannot stand alone.
fn declared_type(value: &Value) -> Projection<Option<Value>> {
    match value {
        Value::String(name) if name == "null" => Err(Failure::error(
            "type: null can not be the only possible type for the field.",
        )),
        Value::String(name) => Ok(Some(type_name(name))),
        Value::Array(_) => Ok(None),
        _ => Err(malformed("schema type must be text")),
    }
}

/// Convert each alternative; a null-type alternative only marks the schema nullable.
fn union(alternatives: Value, converted: &mut Map<String, Value>) -> Projection<()> {
    let Value::Array(items) = alternatives else {
        return Err(malformed("anyOf must be a list"));
    };
    let mut arms = Vec::new();
    for item in items {
        if is_null_type(&item) {
            converted.insert("nullable".to_owned(), Value::Bool(true));
        } else {
            arms.push(convert(item)?);
        }
    }
    converted.insert("anyOf".to_owned(), Value::Array(arms));
    Ok(())
}

/// Convert each property schema, visiting properties in canonical key order.
fn properties(value: Value) -> Projection<Value> {
    let Value::Object(members) = value else {
        return Err(malformed("properties must be an object"));
    };
    canonical_entries(members)
        .into_iter()
        .map(|(name, schema)| Ok((name, convert(schema)?)))
        .collect::<Projection<Map<_, _>>>()
        .map(Value::Object)
}

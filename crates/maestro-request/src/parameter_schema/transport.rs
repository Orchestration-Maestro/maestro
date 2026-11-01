//! Internal component metadata keeps authored schema kinds beside ordinary JSON.
use super::{ParameterSchema, SchemaKind};
use crate::tool_info::ToolInfo;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::HashMap;

/// Member carrying declared kinds beside the plain payload.
const SCHEMA_KINDS: &str = "schemaKinds";

/// Encode metadata with the kinds its author declared.
///
/// # Errors
/// Returns serialization failure or an error when metadata is not an object.
pub fn encode_metadata<T: Serialize, C: Default>(
    metadata: &T,
    parameters: &ParameterSchema<C>,
) -> Result<String, String> {
    let mut value = serde_json::to_value(metadata).map_err(|error| error.to_string())?;
    let kinds =
        serde_json::to_value(parameters.conversions()).map_err(|error| error.to_string())?;
    match value.as_object_mut() {
        Some(object) => {
            object.insert(SCHEMA_KINDS.to_owned(), kinds);
        }
        None => return Err("tool metadata is not an object".into()),
    }
    serde_json::to_string(&value).map_err(|error| error.to_string())
}

/// Declared schema kinds carried by one transport record.
///
/// Other members are skipped without building a value, keeping decoding metadata-local.
#[derive(Default, Deserialize)]
struct CarriedKinds {
    /// Authored kinds by JSON pointer.
    #[serde(rename = "schemaKinds", default)]
    schema_kinds: HashMap<String, SchemaKind>,
}

/// Decode enumerated tools, restoring their carried schema kinds.
///
/// # Errors
/// Returns invalid metadata or trailing-input errors.
pub fn decode_tools<C: Default>(text: &str) -> Result<Vec<ToolInfo<ParameterSchema<C>>>, String> {
    let tools = decode_json::<Vec<ToolInfo<ParameterSchema<C>>>>(text)?;
    let carried = serde_json::from_str::<Vec<CarriedKinds>>(text).unwrap_or_default();
    if carried.len() != tools.len() {
        return Ok(tools);
    }
    Ok(tools
        .into_iter()
        .zip(carried)
        .map(|(mut tool, carried)| {
            tool.parameters =
                ParameterSchema::authored(tool.parameters.into_json(), carried.schema_kinds);
            tool
        })
        .collect())
}

/// Metadata fields beside the authored schema, read without a second schema owner.
#[derive(Deserialize)]
#[serde(bound(deserialize = "C: Default"))]
struct CarriedSchema<C: Default> {
    /// Supplied ordinary JSON Schema.
    parameters: ParameterSchema<C>,
    /// Declared node kinds, independent of ordinary JSON serialization.
    #[serde(flatten)]
    kinds: CarriedKinds,
}

/// Decode registration metadata and its authored parameter schema.
///
/// # Errors
/// Returns malformed metadata or trailing-input errors.
pub fn decode_metadata<T: DeserializeOwned, C: Default>(
    text: &str,
) -> Result<(T, ParameterSchema<C>), String> {
    let metadata = decode_json::<T>(text)?;
    let schema = decode_json::<CarriedSchema<C>>(text)?;
    Ok((
        metadata,
        ParameterSchema::authored(schema.parameters.into_json(), schema.kinds.schema_kinds),
    ))
}

/// Encode enumeration through the same metadata transport used for registration.
///
/// # Errors
/// Returns metadata serialization errors.
pub fn encode_tools<C: Default>(tools: &[ToolInfo<ParameterSchema<C>>]) -> Result<String, String> {
    let records = tools
        .iter()
        .map(|tool| encode_metadata(tool, &tool.parameters))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!("[{}]", records.join(",")))
}

/// Decode one complete component JSON document without a fixed nesting limit.
///
/// # Errors
/// Returns malformed JSON or trailing input.
pub fn decode_json<T: DeserializeOwned>(text: &str) -> Result<T, String> {
    let mut deserializer = serde_json::Deserializer::from_str(text);
    deserializer.disable_recursion_limit();
    let value = T::deserialize(&mut deserializer).map_err(|error| error.to_string())?;
    deserializer.end().map_err(|error| error.to_string())?;
    Ok(value)
}

/// Canonical object traversal used by component JSON views.
pub use maestro_json::canonical_entries;

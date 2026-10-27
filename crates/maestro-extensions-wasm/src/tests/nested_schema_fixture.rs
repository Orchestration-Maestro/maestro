//! Supplied nested declarations shared by the author and its host witnesses.
use maestro_request::parameter_schema::{ParameterSchema, SchemaKind};
use serde::Deserialize;
use std::collections::HashMap;

/// One declaration supplied to the author.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Case {
    /// Selected declaration name.
    pub(crate) id: String,
    /// Supplied schema text.
    schema_json: String,
    /// The kinds declared beside it.
    kinds: HashMap<String, SchemaKind>,
}

/// Build the selected owned declaration from its supplied text and kinds.
pub(crate) fn parameters(id: &str) -> Result<ParameterSchema, String> {
    let case = cases()?
        .into_iter()
        .find(|case| case.id == id)
        .ok_or("unknown schema")?;
    Ok(ParameterSchema::authored(
        value(&case.schema_json)?,
        case.kinds,
    ))
}

/// Read the complete supplied declaration or metadata observation.
pub(crate) fn value(text: &str) -> Result<serde_json::Value, String> {
    let mut decoder = serde_json::Deserializer::from_str(text);
    decoder.disable_recursion_limit();
    let schema = serde_json::Value::deserialize(&mut decoder).map_err(|e| e.to_string())?;
    decoder.end().map_err(|e| e.to_string())?;
    Ok(schema)
}

/// Read the strict declaration inventory shared by the author and native witnesses.
pub(crate) fn cases() -> Result<Vec<Case>, String> {
    serde_json::from_str(include_str!("nested_schema_corpus.json"))
        .map_err(|error| error.to_string())
}

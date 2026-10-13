//! Projection of a post-hook content-generation request onto its wire body.
//!
//! The projector selects fields from open JSON in the source order of the public endpoint,
//! so the hook may add any supported field. Key order and number spelling of the final text
//! belong to the [JSON owner](maestro_json).

use serde_json::{Map, Value};

use crate::DiagnosticErrorInfo;

mod config;
mod content;
mod schema;
mod tools;

/// A failed projection.
type Failure = DiagnosticErrorInfo;
/// The result of one projection step.
type Projection<T> = Result<T, Failure>;
/// Conversion applied to a selected member.
type Convert = fn(Value) -> Projection<Value>;

/// The selected model resource and wire body of a content-generation request.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectedContentRequest {
    /// Normalized model resource path, not a full endpoint URL.
    pub model: String,
    /// Selected wire object, without the model or any client-only control.
    pub body: Map<String, Value>,
}

/// One decision of a record's member selection, taken in the order the plan lists them.
enum Step {
    /// Copy a non-null member unchanged.
    Keep(&'static str),
    /// Fail when the member is present, whatever its value.
    Reject(&'static str),
    /// Convert a non-null member.
    Convert(&'static str, Convert),
    /// Convert a non-null member into the enclosing body instead of the record.
    Body(&'static str, Convert),
}

/// Select the members a plan names from `source`, moving body-level members into `body`.
///
/// A source that is not an object has no members; unplanned members are discarded.
fn select(
    source: Value,
    plan: &[Step],
    body: &mut Map<String, Value>,
) -> Projection<Map<String, Value>> {
    let mut source = match source {
        Value::Object(members) => members,
        _ => Map::new(),
    };
    let mut selected = Map::new();
    for step in plan {
        match *step {
            Step::Reject(key) if source.contains_key(key) => return Err(unsupported(key)),
            Step::Reject(_) => {}
            Step::Keep(key) => {
                if let Some(value) = take(&mut source, key) {
                    selected.insert(key.to_owned(), value);
                }
            }
            Step::Convert(key, convert) => {
                if let Some(value) = take(&mut source, key) {
                    selected.insert(key.to_owned(), convert(value)?);
                }
            }
            Step::Body(key, convert) => {
                if let Some(value) = take(&mut source, key) {
                    body.insert(key.to_owned(), convert(value)?);
                }
            }
        }
    }
    Ok(selected)
}

/// Select a record that has no body-level members.
fn record(source: Value, plan: &[Step]) -> Projection<Value> {
    select(source, plan, &mut Map::new()).map(Value::Object)
}

/// Remove a member and return it unless it is null.
fn take(source: &mut Map<String, Value>, key: &str) -> Option<Value> {
    source.shift_remove(key).filter(|value| !value.is_null())
}

/// The rejection of a member the public endpoint does not accept.
fn unsupported(key: &str) -> Failure {
    Failure::error(format!("{key} parameter is not supported in Gemini API."))
}

/// A rejection of input whose shape no wire field can hold.
fn malformed(reason: &str) -> Failure {
    Failure::error(reason)
}

/// The diagnostic type name of a value that is neither a string nor an object.
fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Bool(_) => "boolean",
        _ => "number",
    }
}

/// Project the value a payload hook returned onto the model path and wire body.
///
/// The model is normalized and checked first, then contents, then configuration; the first
/// rejection wins. Selected record members omit null; unsupported record members fail on
/// presence even when null. Nothing is sent and no hook is called.
///
/// # Errors
/// Fails with the authored message of the first unsupported or invalid member, or with a
/// description of a value whose shape cannot be projected.
pub fn project_content_request(
    params: Value,
) -> Result<ProjectedContentRequest, DiagnosticErrorInfo> {
    let Value::Object(mut params) = params else {
        return Err(model_missing());
    };
    let model = model_path(take(&mut params, "model"))?;
    let mut body = Map::new();
    if let Some(contents) = take(&mut params, "contents") {
        body.insert("contents".to_owned(), content::contents(contents)?);
    }
    if let Some(config) = take(&mut params, "config") {
        let generation = config::generation(config, &mut body)?;
        body.insert("generationConfig".to_owned(), Value::Object(generation));
    }
    Ok(ProjectedContentRequest { model, body })
}

/// The rejection of a missing or non-text model.
fn model_missing() -> Failure {
    Failure::error("model is required and must be a string")
}

/// The model resource path: qualified names are kept and others gain the `models/` prefix.
fn model_path(model: Option<Value>) -> Projection<String> {
    let Some(Value::String(id)) = model.filter(|id| id != "") else {
        return Err(model_missing());
    };
    if id.contains("..") || id.contains('?') || id.contains('&') {
        return Err(Failure::error("invalid model parameter"));
    }
    Ok(
        if id.starts_with("models/") || id.starts_with("tunedModels/") {
            id
        } else {
            format!("models/{id}")
        },
    )
}

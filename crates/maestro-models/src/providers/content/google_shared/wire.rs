//! Members of one response item, read with typed reads at the branch that consumes them.

use serde::de::{DeserializeOwned, Error};
use serde_json::Value;
use serde_json::value::RawValue;

use super::ThoughtMetadata;
use crate::arguments::replay::nullish_default;
use crate::providers::json_text::{is_truthy, json_value, member, raw_json};
use crate::providers::selected_fields::{count, field, native, required};
use crate::{DiagnosticErrorInfo, JsonObject, Usage, UsageCost};
use maestro_request::presence::Presence;

/// The members of a part that decide which updates it makes.
pub(super) struct Part<'a> {
    /// The text, when the part carries one.
    pub(super) text: Option<String>,
    /// The thought flag and signature.
    pub(super) metadata: ThoughtMetadata,
    /// The function call, when the part carries one.
    pub(super) call: Option<&'a RawValue>,
}

/// The members of a function call.
pub(super) struct Call<'a> {
    /// The identifier the service gave the call.
    pub(super) id: Option<String>,
    /// The name of the function.
    pub(super) name: Option<String>,
    /// The raw arguments, absent when the call has none.
    pub(super) arguments: Option<&'a RawValue>,
}

/// Decode a member that may be absent or null; both read as absent.
pub(super) fn nullable<T: DeserializeOwned>(
    raw: &RawValue,
    name: &str,
) -> Result<Option<T>, DiagnosticErrorInfo> {
    field::<Option<T>>(raw, name).map(Option::flatten)
}

/// Parse one framed response item.
pub(super) fn item(input: &str) -> Result<&RawValue, DiagnosticErrorInfo> {
    required(Some(raw_json(input).map_err(|error| native(&error))?))
}

/// Decode an array into its raw elements.
fn elements(raw: &RawValue) -> Result<Vec<&RawValue>, DiagnosticErrorInfo> {
    serde_json::from_str(raw.get()).map_err(|error| native(&error))
}

/// Require an object where a branch reads its members.
fn object(raw: &RawValue) -> Result<&RawValue, DiagnosticErrorInfo> {
    if raw.get().starts_with('{') {
        Ok(raw)
    } else {
        Err(native(&serde_json::Error::custom("expected an object")))
    }
}

/// The first candidate, which is the only one that contributes.
pub(super) fn candidate(item: &RawValue) -> Result<Option<&RawValue>, DiagnosticErrorInfo> {
    let Some(candidates) = member(item, "candidates").filter(|raw| raw.get() != "null") else {
        return Ok(None);
    };
    Ok(elements(candidates)?.first().copied())
}

/// The parts of the candidate content.
pub(super) fn parts(candidate: &RawValue) -> Result<Vec<&RawValue>, DiagnosticErrorInfo> {
    member(candidate, "content")
        .and_then(|content| member(content, "parts"))
        .filter(|raw| raw.get() != "null")
        .map_or_else(|| Ok(Vec::new()), elements)
}

/// The candidate's finish reason, absent when null or empty.
pub(super) fn finish_reason(candidate: &RawValue) -> Result<Option<String>, DiagnosticErrorInfo> {
    Ok(nullable::<String>(candidate, "finishReason")?.filter(|reason| !reason.is_empty()))
}

/// Read the text, thought flag and signature of a part that is not null.
pub(super) fn part(raw: &RawValue) -> Result<Part<'_>, DiagnosticErrorInfo> {
    let raw = required(Some(raw))?;
    Ok(Part {
        text: field(raw, "text")?,
        metadata: ThoughtMetadata {
            thought: nullable(raw, "thought")?,
            thought_signature: nullable(raw, "thoughtSignature")?,
        },
        call: member(raw, "functionCall").filter(|call| is_truthy(call)),
    })
}

/// Read the identifier and name of a function call; the arguments stay raw.
pub(super) fn call(raw: &RawValue) -> Result<Call<'_>, DiagnosticErrorInfo> {
    let raw = object(raw)?;
    Ok(Call {
        id: nullable(raw, "id")?,
        name: nullable(raw, "name")?,
        arguments: member(raw, "args").filter(|args| args.get() != "null"),
    })
}

/// Materialize call arguments of any root kind; missing or null arguments are an empty
/// object.
pub(super) fn arguments(raw: Option<&RawValue>) -> Result<Value, DiagnosticErrorInfo> {
    let Some(raw) = raw else {
        return Ok(JsonObject::new().into());
    };
    let value = json_value(raw).map_err(|error| native(&error))?;
    Ok(nullish_default(value))
}

/// Read a usage report; the cost is left for the model's rates to fill.
pub(super) fn usage(raw: &RawValue) -> Result<Usage, DiagnosticErrorInfo> {
    let raw = object(raw)?;
    let prompt = count(raw, "promptTokenCount")?;
    let cached = count(raw, "cachedContentTokenCount")?;
    let candidates = count(raw, "candidatesTokenCount")?;
    let thoughts = count(raw, "thoughtsTokenCount")?;
    Ok(Usage {
        input: prompt - cached,
        output: candidates + thoughts,
        cache_read: cached,
        cache_write: 0.0,
        total_tokens: Presence::Present(count(raw, "totalTokenCount")?),
        cost: UsageCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total: 0.0,
        },
    })
}
